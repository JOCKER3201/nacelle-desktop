//! The Wayland colour pipeline: telling the compositor what colour
//! space this program's surface is in, via the Color Management
//! protocol.
//!
//! Exists only in a native Wayland session — the protocol IS the
//! session's compositor speaking. Under gamescope or X11 there is
//! nobody to talk to, the module never starts, and the COLOR section is
//! painted shut with the reason written under it.
//!
//! What is NOT this module's, and used to be shut in here with it: the
//! swapchain bit depth and the grading LUT. Those are the renderer's,
//! nobody is asked about them, and they go on working in a session with
//! no colour manager at all (`main.rs`, `apply_color!`).
//!
//! The connection here is a second one to the SAME display winit
//! holds: libwayland is built for that (one socket, many queues), and
//! the surface being described is winit's own, wrapped from the raw
//! pointer. Only requests about colour travel on this queue, so the
//! two event loops never trip over each other.

use std::collections::HashSet;
use std::os::fd::{AsFd, AsRawFd};
use std::time::{Duration, Instant};

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::color_management::v1::client::{
    wp_color_management_surface_v1::WpColorManagementSurfaceV1,
    wp_color_manager_v1::{self, WpColorManagerV1},
    wp_image_description_creator_icc_v1::WpImageDescriptionCreatorIccV1,
    wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1,
    wp_image_description_v1::{self, WpImageDescriptionV1},
};

/// What the compositor said it can do, plus the fate of the image
/// description being created right now.
#[derive(Default)]
struct State {
    manager: Option<(u32, u32)>,
    features: HashSet<u32>,
    tf_named: HashSet<u32>,
    primaries_named: HashSet<u32>,
    /// Whether the manager has sent `done` — the event whose ONLY job is
    /// to say that the three sets above are complete. See
    /// [`ColorMgr::start`] for why a roundtrip is not that answer.
    caps_done: bool,
    /// Some(true) = ready, Some(false) = failed with the reason kept.
    desc_done: Option<bool>,
    /// The compositor's own words, and the protocol's four-way reason
    /// beside them. Two fields because they answer different questions:
    /// the cause is a category this program can put into a sentence a
    /// user can act on, the message is free text a compositor may leave
    /// empty — and an empty message used to become the whole reason.
    desc_cause: Option<wp_image_description_v1::Cause>,
    desc_error: Option<String>,
}

pub struct ColorMgr {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: WpColorManagerV1,
    surface: WpColorManagementSurfaceV1,
    /// The manager never finished listing what it can do, so
    /// "this compositor does not offer X" would be a claim this program
    /// has not earned. Remembered so the sentence under the SPACE list
    /// can say which of the two it is.
    caps_partial: bool,
}

/// How long anything on this queue may go unanswered before the page
/// says so.
///
/// A CLOCK, AND THE PREVIOUS CODE HAD NEITHER A CLOCK NOR A BOUND.
/// It read `for _ in 0..10 { queue.roundtrip(..) }` under a comment
/// promising that "a couple of roundtrips is all the waiting there is",
/// which reads as a bound and is not one: a roundtrip waits for the
/// compositor's answer to `wl_display.sync` and waits with
/// `poll(.., None)` — no timeout, forever. The ten counted ANSWERED
/// roundtrips, so it bounded the wait only on a compositor that was
/// answering, which is the one case where no bound is needed. A
/// compositor that went quiet — wedged, or stopped by a debugger — hung
/// this call, and this call is made from the frame loop's own thread:
/// the whole desktop stops, with the settings window half-drawn.
///
/// Half a second because both ends are on one machine over a unix
/// socket, where an answer is microseconds away; anything this program
/// waits for here it waits for with the picture frozen, so the number
/// has to be small enough to read as a stutter rather than as a hang.
const ANSWER_WAIT: Duration = Duration::from_millis(500);

/// The largest ICC profile this program will hand to a compositor.
/// Display profiles are tens of kilobytes; a file this size is a file
/// picked by mistake, and the point of refusing it here is to say so
/// with a sentence instead of by whatever the compositor does with it.
const ICC_MAX: u64 = 32 * 1024 * 1024;

/// Pumps `queue` until `answered` holds or `limit` runs out; answers
/// whether it was answered.
///
/// A FREE FUNCTION because both waits in this module need it and one of
/// them ([`ColorMgr::start`]) happens before a `ColorMgr` exists.
///
/// The shape is the one libwayland documents for a client with more
/// than one event source: dispatch what is already buffered, flush,
/// `prepare_read`, wait on the fd, read. The only thing added is that
/// the wait on the fd has a deadline — which is the entire difference
/// between this and a roundtrip.
fn wait_until(
    conn: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    limit: Duration,
    answered: fn(&State) -> bool,
) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        // What already arrived, first and every time round: an answer
        // sitting in the buffer must never be made to wait on a socket
        // that has nothing more to say.
        if queue.dispatch_pending(state).is_err() {
            return false;
        }
        if answered(state) {
            return true;
        }
        // Checked HERE and not only around the poll, so that every path
        // through this loop is bounded by the same clock — including the
        // `prepare_read` retry below, which does no waiting at all and
        // could otherwise spin.
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        if conn.flush().is_err() {
            return false;
        }
        let Some(guard) = conn.prepare_read() else {
            // Another queue's events are buffered; dispatching them is
            // what makes a read possible again.
            continue;
        };
        let fd = guard.connection_fd().as_raw_fd();
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let ms = left.as_millis().min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut pfd, 1, ms) };
        if ready < 0 {
            // A signal is not an answer and not a failure. Dropping the
            // guard cancels the read libwayland was holding open, which
            // is what makes going round again legal.
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return false;
        }
        if ready == 0 {
            return false;
        }
        if guard.read().is_err() {
            return false;
        }
    }
}

/// The compositor's four-way reason, in a sentence rather than a name.
///
/// FOUR CAUSES, FOUR ANSWERS, and they are four because the protocol
/// says so — `wp_image_description_v1.cause`. Until this existed the
/// argument was destructured away (`Event::Failed { msg, .. }`) and the
/// whole of the reason was whatever free text the compositor happened to
/// attach. That text is documented as an "ad hoc human-readable
/// explanation": optional in practice, in the compositor's own language,
/// and written for a log rather than for the person holding the mouse.
/// The cause is the part that is always there and always means the same
/// thing, and it is the part that tells a user what to do next — an old
/// compositor is a thing to upgrade, a missing screen is a thing to
/// plug back in, and neither reads like the other.
fn cause_words(cause: Option<wp_image_description_v1::Cause>) -> &'static str {
    use wp_image_description_v1::Cause;
    match cause {
        Some(Cause::LowVersion) => {
            "its colour management is too old to be asked for this"
        }
        Some(Cause::Unsupported) => "it cannot show this combination",
        Some(Cause::OperatingSystem) => {
            "the system refused, for a reason outside this program"
        }
        Some(Cause::NoOutput) => "the screen it was meant for is gone",
        // Every other value, and no value at all. A compositor sending a
        // cause this build does not know is not a compositor to argue
        // with: the refusal itself is still true and still worth saying.
        _ => "it gave no reason this program understands",
    }
}

/// The ONE sentence a refusal turns into, wherever the refusal came
/// from.
///
/// The compositor's own words are kept when there are any, in quotation
/// marks and AFTER the cause, so the sentence is complete before the
/// quotation starts. An empty message is treated as no message at all —
/// which is exactly what the code before this could not do: it read
/// `desc_error.unwrap_or("no reason given")`, and `Some("")` is not
/// `None`, so a compositor that refused with an empty explanation put
/// "the compositor refused bt2020 pq: " on the page and stopped.
fn refusal_line(
    label: &str,
    cause: Option<wp_image_description_v1::Cause>,
    msg: Option<&str>,
) -> String {
    let words = cause_words(cause);
    match msg.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => format!("the compositor refused {label}: {words} — it said \u{201c}{m}\u{201d}"),
        None => format!("the compositor refused {label}: {words}"),
    }
}

/// What one space name asks the protocol for: its named primaries and a
/// LIST of transfer functions, best first.
///
/// A list and not one curve, because compositors disagree about the sRGB
/// one (KWin, for one, offers gamma 2.2 and not the piecewise sRGB), and
/// refusing over that would grey the whole feature out on the very
/// compositor it was built for.
///
/// A free function so that asking "can this be shown?"
/// ([`ColorMgr::supports`]) and asking for it ([`ColorMgr::apply_space`])
/// read ONE table. The settings window hides what the answer is no for,
/// so the two must not be able to disagree: a space missing from the
/// list would still be applicable, and one offered but unapplicable
/// would be a control that does nothing.
fn preset(
    space: &str,
) -> Option<(
    wp_color_manager_v1::Primaries,
    &'static [wp_color_manager_v1::TransferFunction],
)> {
    use wp_color_manager_v1::{Primaries, TransferFunction as Tf};
    match space {
        "srgb" => Some((Primaries::Srgb, &[Tf::Srgb, Tf::Gamma22, Tf::Bt1886])),
        "display p3" => {
            Some((Primaries::DisplayP3, &[Tf::Srgb, Tf::Gamma22, Tf::Bt1886]))
        }
        "adobe rgb" => Some((Primaries::AdobeRgb, &[Tf::Gamma22, Tf::Srgb])),
        "bt2020 pq" => Some((Primaries::Bt2020, &[Tf::St2084Pq])),
        "bt2020 hlg" => Some((Primaries::Bt2020, &[Tf::Hlg])),
        "scrgb linear" => Some((Primaries::Srgb, &[Tf::ExtLinear])),
        _ => None,
    }
}

impl ColorMgr {
    /// Attaches to winit's display and surface, both as raw pointers
    /// from the window handle. None when the compositor does not speak
    /// the protocol — which is the whole "greyed out" story.
    pub fn start(display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void) -> Option<ColorMgr> {
        if display.is_null() || surface.is_null() {
            return None;
        }
        let backend = unsafe { Backend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);
        let mut queue: EventQueue<State> = conn.new_event_queue();
        let qh = queue.handle();
        let mut state = State::default();
        let _registry = conn.display().get_registry(&qh, ());
        queue.roundtrip(&mut state).ok()?;
        let (name, version) = state.manager?;
        let registry = conn.display().get_registry(&qh, ());
        // The version actually SPOKEN, kept in a name of its own because
        // it is the version everything below inherits — the creators, and
        // through them every image description — and it decides which of
        // `ready`/`ready2` the answers come back as. The line at the
        // bottom used to print the version the compositor ADVERTISED,
        // which on a compositor newer than this build is a different
        // number, in the one log line anybody would consult about that.
        let bound = version.min(WpColorManagerV1::interface().version);
        let manager: WpColorManagerV1 = registry.bind(name, bound, &qh, ());
        // The manager announces its abilities right after binding, and
        // says `done` when it has announced them all.
        //
        // WAITED FOR, NOT ASSUMED — the same shape of mistake as the one
        // in the description handler below, in the one other place this
        // module reads state an event is still filling in. A roundtrip
        // returns when `wl_display.sync` is answered, which orders it
        // after everything the compositor had already queued but says
        // NOTHING about a list still being written; a compositor that
        // sends its capabilities from an idle callback instead of
        // straight out of the bind would leave this build holding a short
        // list, and a short list is silent: `supports` would answer no,
        // and the SPACE list — built once, at startup, from exactly those
        // answers — would simply be missing entries, with nothing said
        // anywhere. `done` is the event that exists to answer this and
        // nobody was listening to it.
        let caps_partial =
            !wait_until(&conn, &mut queue, &mut state, ANSWER_WAIT, |s| s.caps_done);
        if caps_partial {
            eprintln!(
                "nacelle-desktop: the colour manager never finished listing what it \
                 offers — the COLOR page may be short of spaces this machine can show"
            );
        }

        let sid = unsafe {
            ObjectId::from_ptr(WlSurface::interface(), surface.cast()).ok()?
        };
        let wl_surface = WlSurface::from_id(&conn, sid).ok()?;
        let surface = manager.get_surface(&wl_surface, &qh, ());
        conn.flush().ok()?;
        eprintln!("nacelle-desktop: colour management at the compositor's, v{bound}");
        Some(ColorMgr { conn, queue, state, manager, surface, caps_partial })
    }

    /// Applies the preferences: an ICC profile when one is chosen (the
    /// more specific wish wins), a named colour space otherwise, and
    /// the compositor's own default for "auto".
    ///
    /// Answers with the ONE LINE the settings window shows under the
    /// controls that asked. Every outcome below used to be a line on
    /// stderr and nothing else — and a desktop session has nowhere to
    /// show a stderr, which is the same reason the ADDONS page carries
    /// the loader's complaints. A space the compositor would not take
    /// left the picture exactly as it was, under a list that had just
    /// moved its mark: a control pretending to have worked.
    pub fn apply(&mut self, space: &str, icc: Option<&std::path::Path>) -> String {
        let Some(path) = icc else { return self.apply_space(space) };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match self.apply_icc(path) {
            // Said out loud, because this is the one case where a
            // control the user just turned is deliberately ignored:
            // the SPACE list is live, its mark moved, and the
            // picture is answering to a file instead.
            Ok(()) => format!("the ICC profile {name} is in force — it overrides {space}"),
            // AND THE OTHER CASE, which used to be no case at all. A
            // profile that could not be taken fell through this `if`
            // silently and the page went on to report the SPACE alone —
            // so a user who had chosen a profile read a line about a
            // colour space, saw nothing wrong, and never learnt that the
            // file they picked had been dropped. Both halves are said
            // now: why the profile is not in force, and what the picture
            // is answering to instead.
            Err(why) => {
                eprintln!(
                    "nacelle-desktop: the ICC profile {} was not taken: {why}",
                    path.display()
                );
                let fell_back = self.apply_space(space);
                format!("the ICC profile {name} was not taken — {why}; {fell_back}")
            }
        }
    }

    /// Whether this compositor can be asked for `space` at all.
    ///
    /// The SAME test [`ColorMgr::apply_space`] makes before it builds a
    /// description, asked ahead of time: the settings window leaves out
    /// what this machine cannot show rather than offering it and
    /// printing a line into the log when it is picked. "auto" is always
    /// answerable — it asks for nothing.
    pub fn supports(&self, space: &str) -> bool {
        let Some((prim, tfs)) = preset(space) else { return true };
        tfs.iter().any(|tf| self.state.tf_named.contains(&(*tf as u32)))
            && self
                .state
                .features
                .contains(&(wp_color_manager_v1::Feature::Parametric as u32))
            && self.state.primaries_named.contains(&(prim as u32))
    }

    fn apply_space(&mut self, space: &str) -> String {
        let Some((prim, tfs)) = preset(space) else {
            // "auto": the compositor's preference stands.
            self.surface.unset_image_description();
            let _ = self.conn.flush();
            return "the compositor's own choice".to_string();
        };
        let tf = tfs
            .iter()
            .copied()
            .find(|tf| self.state.tf_named.contains(&(*tf as u32)));
        let (Some(tf), true, true) = (
            tf,
            self.state
                .features
                .contains(&(wp_color_manager_v1::Feature::Parametric as u32)),
            self.state.primaries_named.contains(&(prim as u32)),
        ) else {
            // TWO SENTENCES, because there are two states here and only
            // one of them is a fact about the compositor. "Does not
            // offer" is read off a list the compositor finished giving;
            // if it never finished, the same words would be this program
            // reporting its own short read as the machine's limitation,
            // which is the kind of sentence that sends somebody hunting
            // for a monitor that is not the problem.
            let line = if self.caps_partial {
                format!(
                    "this compositor never finished saying what it offers, \
                     and {space} was not in what it did say"
                )
            } else {
                format!("this compositor does not offer {space}")
            };
            eprintln!("nacelle-desktop: {line} — leaving its default");
            return line;
        };
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorParamsV1 =
            self.manager.create_parametric_creator(&qh, ());
        creator.set_primaries_named(prim);
        creator.set_tf_named(tf);
        let desc = creator.create(&qh, ());
        match self.finish(desc, space) {
            Ok(()) => format!("{space} is in force"),
            Err(why) => why,
        }
    }

    /// Hands the compositor an ICC profile, or says why it could not be.
    ///
    /// `Result` and not `bool`, because every one of these returns is a
    /// sentence somebody has to read. As a bool each of them was an
    /// `eprintln!` and a `false`, the caller could not tell them apart,
    /// and a desktop session has nowhere to show a stderr — so choosing
    /// a profile that turned out to be a JPEG looked exactly like
    /// choosing one that worked.
    fn apply_icc(&mut self, path: &std::path::Path) -> Result<(), String> {
        if !self
            .state
            .features
            .contains(&(wp_color_manager_v1::Feature::IccV2V4 as u32))
        {
            return Err("this compositor does not take ICC profiles".to_string());
        }
        let file = std::fs::File::open(path)
            .map_err(|e| format!("the file cannot be opened ({e})"))?;
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len == 0 {
            return Err("the file is empty".to_string());
        }
        if len > ICC_MAX {
            return Err(format!(
                "the file is {} MiB, past the {} MiB limit for a profile",
                len / (1024 * 1024),
                ICC_MAX / (1024 * 1024)
            ));
        }
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorIccV1 = self.manager.create_icc_creator(&qh, ());
        creator.set_icc_file(file.as_fd(), 0, len as u32);
        let desc = creator.create(&qh, ());
        self.finish(desc, &path.display().to_string())
    }

    /// Waits for the description to answer for itself and sets it on the
    /// surface, or answers with the sentence the page is to show.
    ///
    /// The ready event follows the create over the same socket, so the
    /// wait is normally microseconds — but it is a WAIT and not a
    /// counted number of roundtrips, and [`ANSWER_WAIT`] says why the
    /// difference is not cosmetic.
    fn finish(
        &mut self,
        desc: WpImageDescriptionV1,
        label: &str,
    ) -> Result<(), String> {
        self.state.desc_done = None;
        self.state.desc_cause = None;
        self.state.desc_error = None;
        wait_until(&self.conn, &mut self.queue, &mut self.state, ANSWER_WAIT, |s| {
            s.desc_done.is_some()
        });
        let out = match self.state.desc_done {
            Some(true) => {
                self.surface.set_image_description(
                    &desc,
                    wp_color_manager_v1::RenderIntent::Perceptual,
                );
                eprintln!("nacelle-desktop: surface colour set to {label}");
                Ok(())
            }
            Some(false) => {
                let line = refusal_line(
                    label,
                    self.state.desc_cause,
                    self.state.desc_error.as_deref(),
                );
                eprintln!("nacelle-desktop: {line}");
                Err(line)
            }
            // Neither ready nor failed before the clock ran out: the
            // description was built and never answered for. Named as its
            // own outcome and not folded into "refused", because the two
            // ask for different things — a refusal is the compositor's
            // answer, silence is a question about this program.
            None => {
                let line = format!(
                    "no answer from the compositor about {label} in {} ms",
                    ANSWER_WAIT.as_millis()
                );
                eprintln!("nacelle-desktop: {line}; leaving colours as they are");
                Err(line)
            }
        };
        // Immutable once set; the compositor keeps what it needs and the
        // object may go. Destroyed on every path — a description that
        // was refused, or never answered for, is still an object of ours
        // on the compositor's side.
        desc.destroy();
        let _ = self.conn.flush();
        out
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == WpColorManagerV1::interface().name {
                state.manager = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpColorManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpColorManagerV1,
        event: wp_color_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wp_color_manager_v1::Event;
        match event {
            Event::SupportedFeature { feature } => {
                if let Ok(f) = feature.into_result() {
                    state.features.insert(f as u32);
                }
            }
            Event::SupportedTfNamed { tf } => {
                if let Ok(tf) = tf.into_result() {
                    state.tf_named.insert(tf as u32);
                }
            }
            Event::SupportedPrimariesNamed { primaries } => {
                if let Ok(p) = primaries.into_result() {
                    state.primaries_named.insert(p as u32);
                }
            }
            // "all features have been sent" — the end of the three lists
            // above, and the only thing in the protocol that says they
            // are complete. It was falling into the arm below with
            // everything else this handler does not use, and
            // [`ColorMgr::start`] read the lists on a roundtrip's word
            // instead.
            Event::Done => state.caps_done = true,
            _ => {}
        }
    }
}

/// The fate of an image description, written into the state
/// [`ColorMgr::finish`] is waiting on.
///
/// **BOTH ready events, and this is the whole bug.** The protocol has
/// two: `ready` carries a 32-bit identity and `ready2` a 64-bit one, and
/// the XML says of the first — "Starting from interface version 2, the
/// 'ready2' event is sent instead of this event." The version an object
/// speaks is inherited from the one that made it, all the way down from
/// the bind, so on a compositor announcing version 2 or 3 (this build
/// asks for up to 3) EVERY image description answers with `ready2` and
/// with nothing else. A reader that knew only `ready` therefore heard
/// silence, waited out the whole wait, and returned without ever
/// sending `set_image_description` — leaving the surface exactly as it
/// was. That is the "changing HDR and the colour space does nothing"
/// the owner reported: not a wrong picture, no picture at all.
///
/// **AND IT WAS NOT THE ONLY DEAF HANDLER.** The manager's own `done`,
/// two impls up, was falling through the same kind of catch-all arm —
/// so `start` read the capability lists on a roundtrip's word rather
/// than on the event that declares them complete. Same shape, quieter
/// symptom: no missing picture, just spaces missing from a list.
///
/// The match sits HERE, in the handler itself, and not in a pure helper
/// beside it. A helper would be the easier thing to test and would test
/// the wrong thing: the bug was never in a table of events, it was in
/// this handler being deaf, and a test that reads a helper would pass
/// with the handler put back the way it was. The three arguments this
/// body ignores are the reason it looked untestable — a proxy, a
/// connection and a queue handle — and none of them needs a compositor:
/// a socket pair and a null object id make all three (see the tests).
impl Dispatch<WpImageDescriptionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpImageDescriptionV1,
        event: wp_image_description_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wp_image_description_v1::Event;
        match event {
            Event::Ready { .. } | Event::Ready2 { .. } => state.desc_done = Some(true),
            // BOTH ARGUMENTS. The cause was destructured away here
            // (`{ msg, .. }`), so the only reason that ever reached the
            // page was the compositor's free text — which the protocol
            // calls "ad hoc" and permits to be empty, and which an empty
            // string turned into a sentence ending in a colon. The cause
            // is the four-way answer that is always there; `cause_words`
            // turns it into the sentence.
            Event::Failed { cause, msg } => {
                state.desc_done = Some(false);
                state.desc_cause = cause.into_result().ok();
                state.desc_error = Some(msg);
            }
            _ => {}
        }
    }
}

impl Dispatch<WpColorManagementSurfaceV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpColorManagementSurfaceV1,
        _: <WpColorManagementSurfaceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpImageDescriptionCreatorParamsV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpImageDescriptionCreatorParamsV1,
        _: <WpImageDescriptionCreatorParamsV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpImageDescriptionCreatorIccV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpImageDescriptionCreatorIccV1,
        _: <WpImageDescriptionCreatorIccV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use wayland_protocols::wp::color_management::v1::client::wp_image_description_v1 as desc;

    /// Everything the description handler is handed and does not look
    /// at: a proxy, a connection and a queue handle. None of it needs a
    /// compositor.
    ///
    /// A SOCKET PAIR AND A NULL ID, and both halves are deliberate.
    /// `Connection::from_socket` performs no handshake — the display
    /// object is made on this side — so the far end of the pair is never
    /// written to and never read; it is held open only so the near end
    /// is not a hung-up socket. And `Proxy::from_id` accepts the null id
    /// by design (`!same_interface(..) && !id.is_null()`), which is what
    /// makes a proxy without a server possible at all.
    ///
    /// This is the difference between testing the fix and testing a
    /// paraphrase of it: the assertions below run THE HANDLER, so the
    /// handler going deaf again is a failure and not a silent
    /// regression.
    struct Wire {
        conn: Connection,
        queue: EventQueue<State>,
        qh: QueueHandle<State>,
        desc: WpImageDescriptionV1,
        mgr: WpColorManagerV1,
        _far_end: UnixStream,
    }

    impl Wire {
        fn new() -> Wire {
            let (near, far) = UnixStream::pair().expect("a socket pair");
            let conn = Connection::from_socket(near).expect("a connection to nobody");
            let queue: EventQueue<State> = conn.new_event_queue();
            let qh = queue.handle();
            let desc = WpImageDescriptionV1::from_id(&conn, ObjectId::null())
                .expect("a null proxy");
            let mgr =
                WpColorManagerV1::from_id(&conn, ObjectId::null()).expect("a null proxy");
            Wire { conn, queue, qh, desc, mgr, _far_end: far }
        }

        /// One event through `Dispatch::event`, exactly as the queue
        /// would deliver it.
        fn deliver(&self, state: &mut State, event: desc::Event) {
            <State as Dispatch<WpImageDescriptionV1, ()>>::event(
                state,
                &self.desc,
                event,
                &(),
                &self.conn,
                &self.qh,
            );
        }

        /// The same, for the manager's own handler.
        fn deliver_mgr(&self, state: &mut State, event: wp_color_manager_v1::Event) {
            <State as Dispatch<WpColorManagerV1, ()>>::event(
                state,
                &self.mgr,
                event,
                &(),
                &self.conn,
                &self.qh,
            );
        }
    }

    /// **Every space the window offers can be turned into a request.**
    ///
    /// The two tables are written in two files — the names and their
    /// ranges in `config::model`, the primaries and transfer functions
    /// here — and only this test holds them together. A name that fell
    /// out of step would not fail to compile and would not log anything:
    /// [`preset`] answers None for a name it does not know, `apply_space`
    /// reads None as "auto", and the surface would go back to the
    /// compositor's own choice. The user would see a list where every
    /// entry does the same nothing.
    ///
    /// "auto" is the one name that means no request, and it is checked
    /// for exactly that.
    #[test]
    fn every_space_the_window_offers_reaches_the_protocol() {
        for &(name, _) in crate::config::model::COLOR_SPACE_TABLE.iter() {
            if name == crate::config::model::ColorConf::SPACE {
                assert!(
                    preset(name).is_none(),
                    "'{name}' names no space and must ask for nothing"
                );
                continue;
            }
            let (_, tfs) = preset(name).unwrap_or_else(|| {
                panic!(
                    "the COLOR page offers '{name}' and the protocol layer \
                     has no primaries or transfer function for it — picking \
                     it would silently hand the surface back to the compositor"
                )
            });
            assert!(
                !tfs.is_empty(),
                "'{name}' has no transfer function to try at all"
            );
        }
    }

    /// **A compositor speaking version 2 or later says `ready2`, and the
    /// handler has to hear it.**
    ///
    /// This is the regression the owner reported, and it is measured
    /// where it lived: the event goes through `Dispatch::event` and the
    /// assertion is on `State`, the same field `apply_space` reads after
    /// its roundtrips. Nothing about the bug is visible from the outside
    /// — the description IS built, the compositor DOES answer, and the
    /// answer is dropped on the floor — so the picture stays as it was
    /// while every control on the page keeps its new mark.
    ///
    /// Both ready events, because a version-1 compositor still says the
    /// old one and the fix must not trade one deafness for another.
    #[test]
    fn the_handler_hears_both_ready_events() {
        let wire = Wire::new();

        let mut old = State::default();
        wire.deliver(&mut old, desc::Event::Ready { identity: 7 });
        assert_eq!(
            old.desc_done,
            Some(true),
            "a version-1 compositor's answer stopped counting"
        );

        let mut new = State::default();
        wire.deliver(
            &mut new,
            desc::Event::Ready2 { identity_hi: 0, identity_lo: 7 },
        );
        assert_eq!(
            new.desc_done,
            Some(true),
            "a version-2 compositor's answer was not heard — the colour \
             space is built, accepted, and never set on the surface, which \
             is exactly what the owner saw"
        );
    }

    /// **A refusal carries the compositor's own words, and silence is a
    /// third thing.**
    ///
    /// Three outcomes and not two: ready, refused, and never answered
    /// for. `apply_space` tells them apart to write three different
    /// lines under the SPACE list — "in force", "the compositor refused
    /// X: <its words>", "no answer about X" — so a state nobody has told
    /// must stay `None`, and a refusal must arrive with its message
    /// intact. Dropping the message would leave the page saying "refused
    /// … no reason given" over a compositor that gave one.
    #[test]
    fn a_refusal_reaches_the_state_with_its_reason() {
        let wire = Wire::new();

        let untold = State::default();
        assert_eq!(
            untold.desc_done, None,
            "silence has to be its own answer, not a refusal"
        );

        let mut refused = State::default();
        wire.deliver(
            &mut refused,
            desc::Event::Failed {
                cause: wayland_client::WEnum::Value(desc::Cause::Unsupported),
                msg: "no such transfer function".to_string(),
            },
        );
        assert_eq!(refused.desc_done, Some(false), "a refusal read as a success");
        assert_eq!(
            refused.desc_error.as_deref(),
            Some("no such transfer function"),
            "a refusal must carry the compositor's own words to the window"
        );
        assert_eq!(
            refused.desc_cause,
            Some(desc::Cause::Unsupported),
            "the protocol's own four-way reason was destructured away, and it \
             is the half that is always there"
        );
    }

    /// **The four causes are four different sentences.**
    ///
    /// The protocol's `cause` enum has exactly four entries and they do
    /// not mean the same thing to the person reading the page: an
    /// interface too old is something to upgrade, a missing output is
    /// something to plug back in, an operating-system refusal is not
    /// this program's to explain, and "unsupported" is the only one of
    /// the four that says anything about the space that was picked.
    /// Collapsing any two of them into one line would send somebody
    /// looking in the wrong place, so the test is that no two are equal
    /// and none of them is the fallback.
    #[test]
    fn each_of_the_four_causes_says_its_own_thing() {
        use desc::Cause;
        let four = [
            Cause::LowVersion,
            Cause::Unsupported,
            Cause::OperatingSystem,
            Cause::NoOutput,
        ];
        let unknown = cause_words(None);
        let mut said: Vec<&'static str> = Vec::new();
        for c in four {
            let words = cause_words(Some(c));
            assert!(!words.is_empty(), "{c:?} has no sentence at all");
            assert_ne!(
                words, unknown,
                "{c:?} is a reason the compositor gave — it must not read as no reason"
            );
            assert!(
                !said.contains(&words),
                "{c:?} says exactly what another cause says, so the page cannot \
                 tell the user which of the two happened"
            );
            said.push(words);
        }
    }

    /// **An empty explanation is not an explanation.**
    ///
    /// The protocol calls the message an "ad hoc human-readable
    /// explanation" and does not require one, so a compositor may refuse
    /// with `msg: ""`. The line before this read
    /// `desc_error.unwrap_or("no reason given")` — and `Some("")` is not
    /// `None`, so an empty message won that fallback and the page said
    /// "the compositor refused bt2020 pq: " with nothing after the
    /// colon. Both shapes are checked here: the sentence has to stand on
    /// the cause when there are no words, and has to carry the words
    /// when there are.
    #[test]
    fn a_refusal_with_no_words_still_says_why() {
        use desc::Cause;
        let bare = refusal_line("bt2020 pq", Some(Cause::NoOutput), Some(""));
        assert!(
            bare.contains(cause_words(Some(Cause::NoOutput))),
            "an empty message left the sentence with no reason in it: {bare}"
        );
        assert!(
            !bare.trim_end().ends_with(':'),
            "a line that stops at the colon is a page saying nothing loudly: {bare}"
        );
        assert_eq!(
            bare,
            refusal_line("bt2020 pq", Some(Cause::NoOutput), None),
            "an empty message and no message are the same thing to a reader"
        );

        let told = refusal_line(
            "bt2020 pq",
            Some(Cause::Unsupported),
            Some("no such transfer function"),
        );
        assert!(
            told.contains("no such transfer function"),
            "the compositor's own words were dropped: {told}"
        );
        assert!(
            told.contains("bt2020 pq"),
            "the sentence has to name what was refused: {told}"
        );
    }

    /// **The manager's `done` is heard.**
    ///
    /// The second deaf handler in this module, and the same shape as the
    /// `ready2` one above: the compositor sends the event whose only job
    /// is to say "the list is complete", and it fell through into the
    /// arm that ignores everything. `ColorMgr::start` read the three
    /// capability sets after a roundtrip instead — which orders them
    /// after a `wl_display.sync` and says nothing at all about a list
    /// still being written. The cost of being wrong is silent: `supports`
    /// answers no for a space the compositor does offer, and the SPACE
    /// list is built once, at startup, out of exactly those answers.
    #[test]
    fn the_manager_handler_hears_the_end_of_the_list() {
        let wire = Wire::new();

        let mut s = State::default();
        assert!(!s.caps_done, "nobody has said anything yet");

        wire.deliver_mgr(
            &mut s,
            wp_color_manager_v1::Event::SupportedFeature {
                feature: wayland_client::WEnum::Value(
                    wp_color_manager_v1::Feature::Parametric,
                ),
            },
        );
        assert!(
            !s.caps_done,
            "one capability is not the end of the list, and reading it as one is \
             the whole mistake"
        );

        wire.deliver_mgr(&mut s, wp_color_manager_v1::Event::Done);
        assert!(
            s.caps_done,
            "the manager said it had finished and this build did not hear it"
        );
    }

    /// **The wait gives up by itself.**
    ///
    /// This is the bug in `finish` measured directly. It looked bounded
    /// — `for _ in 0..10` — and was not: a `roundtrip` waits for the
    /// compositor's answer to `wl_display.sync` and waits on
    /// `poll(.., None)`, with no timeout, so the ten counted ANSWERED
    /// roundtrips and bounded nothing on a compositor that had gone
    /// quiet. `apply` is called from the frame loop's own thread, so the
    /// price was the whole desktop stopping.
    ///
    /// The connection here is the same socket pair the other tests use:
    /// a far end that is never read and never written, which is exactly
    /// a compositor that will never answer. The old code, handed this,
    /// does not fail — it never returns.
    #[test]
    fn the_wait_gives_up_on_a_compositor_that_never_answers() {
        let mut wire = Wire::new();
        let mut state = State::default();

        let limit = Duration::from_millis(120);
        let started = Instant::now();
        let answered = wait_until(&wire.conn, &mut wire.queue, &mut state, limit, |s| {
            s.desc_done.is_some()
        });
        let took = started.elapsed();

        assert!(
            !answered,
            "nobody answered, so the only honest return is 'no' — a true here \
             would mean the page claims a colour space that was never set"
        );
        assert!(
            took < Duration::from_secs(5),
            "the wait ran for {took:?} against a limit of {limit:?}; anything \
             unbounded here freezes the frame loop"
        );
        // And it was the CLOCK that ended it. Without this the test would
        // pass just as happily on a `wait_until` that fell straight out of
        // its first error path, which returns the same `false` and would
        // give up on a compositor that was about to answer.
        assert!(
            took >= limit / 2,
            "the wait was over in {took:?} of a {limit:?} limit — it gave up on \
             something other than the clock"
        );
        assert_eq!(
            state.desc_done, None,
            "giving up must leave the state untold, so `finish` can tell silence \
             apart from a refusal"
        );
    }
}
