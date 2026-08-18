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
use std::os::fd::AsFd;

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
    /// Some(true) = ready, Some(false) = failed with the message kept.
    desc_done: Option<bool>,
    desc_error: Option<String>,
}

pub struct ColorMgr {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: WpColorManagerV1,
    surface: WpColorManagementSurfaceV1,
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
        let manager: WpColorManagerV1 = registry.bind(
            name,
            version.min(WpColorManagerV1::interface().version),
            &qh,
            (),
        );
        // The manager announces its abilities right after binding.
        queue.roundtrip(&mut state).ok()?;

        let sid = unsafe {
            ObjectId::from_ptr(WlSurface::interface(), surface.cast()).ok()?
        };
        let wl_surface = WlSurface::from_id(&conn, sid).ok()?;
        let surface = manager.get_surface(&wl_surface, &qh, ());
        conn.flush().ok()?;
        eprintln!("nacelle-desktop: colour management at the compositor's, v{version}");
        Some(ColorMgr { conn, queue, state, manager, surface })
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
        if let Some(path) = icc {
            if self.apply_icc(path) {
                // Said out loud, because this is the one case where a
                // control the user just turned is deliberately ignored:
                // the SPACE list is live, its mark moved, and the
                // picture is answering to a file instead.
                return format!(
                    "the ICC profile {} is in force — it overrides {space}",
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                );
            }
        }
        self.apply_space(space)
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
            eprintln!(
                "nacelle-desktop: the compositor does not offer '{space}' — leaving its default"
            );
            return format!("this compositor does not offer {space}");
        };
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorParamsV1 =
            self.manager.create_parametric_creator(&qh, ());
        creator.set_primaries_named(prim);
        creator.set_tf_named(tf);
        let desc = creator.create(&qh, ());
        if self.finish(desc, space) {
            format!("{space} is in force")
        } else {
            match self.state.desc_done {
                Some(false) => format!(
                    "the compositor refused {space}: {}",
                    self.state.desc_error.as_deref().unwrap_or("no reason given")
                ),
                // Neither ready nor failed inside the roundtrips: the
                // description was built and never answered for. Named
                // as its own outcome and not folded into "refused",
                // because the two ask for different things — a refusal
                // is the compositor's answer, silence is a question
                // about this program.
                _ => format!("no answer from the compositor about {space}"),
            }
        }
    }

    fn apply_icc(&mut self, path: &std::path::Path) -> bool {
        if !self
            .state
            .features
            .contains(&(wp_color_manager_v1::Feature::IccV2V4 as u32))
        {
            eprintln!("nacelle-desktop: the compositor does not take ICC profiles");
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else {
            eprintln!("nacelle-desktop: cannot open ICC profile {}", path.display());
            return false;
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len == 0 || len > 32 * 1024 * 1024 {
            eprintln!("nacelle-desktop: ICC profile {} has a nonsense size", path.display());
            return false;
        }
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorIccV1 = self.manager.create_icc_creator(&qh, ());
        creator.set_icc_file(file.as_fd(), 0, len as u32);
        let desc = creator.create(&qh, ());
        self.finish(desc, &path.display().to_string())
    }

    /// Waits for the description to be ready and sets it. The ready
    /// event follows the create over the same socket, so a couple of
    /// roundtrips is all the waiting there is.
    fn finish(&mut self, desc: WpImageDescriptionV1, label: &str) -> bool {
        self.state.desc_done = None;
        self.state.desc_error = None;
        for _ in 0..10 {
            if self.queue.roundtrip(&mut self.state).is_err() {
                return false;
            }
            match self.state.desc_done {
                Some(true) => {
                    self.surface.set_image_description(
                        &desc,
                        wp_color_manager_v1::RenderIntent::Perceptual,
                    );
                    // Immutable once set; the compositor keeps what it
                    // needs and the object may go.
                    desc.destroy();
                    let _ = self.conn.flush();
                    eprintln!("nacelle-desktop: surface colour set to {label}");
                    return true;
                }
                Some(false) => {
                    eprintln!(
                        "nacelle-desktop: the compositor refused {label}: {}",
                        self.state.desc_error.as_deref().unwrap_or("no reason given")
                    );
                    desc.destroy();
                    let _ = self.conn.flush();
                    return false;
                }
                None => {}
            }
        }
        eprintln!("nacelle-desktop: no answer about {label}; leaving colours as they are");
        desc.destroy();
        let _ = self.conn.flush();
        false
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
            _ => {}
        }
    }
}

/// What one event from an image description says about its fate: `Ok`
/// for ready, `Err(reason)` for failed, `None` for anything that is not
/// about the fate at all.
///
/// **BOTH ready events, and this is the whole bug.** The protocol has
/// two: `ready` carries a 32-bit identity and `ready2` a 64-bit one, and
/// the XML says of the first — "Starting from interface version 2, the
/// 'ready2' event is sent instead of this event." The version an object
/// speaks is inherited from the one that made it, all the way down from
/// the bind, so on a compositor announcing version 2 or 3 (this build
/// asks for up to 3) EVERY image description answers with `ready2` and
/// with nothing else. A reader that knew only `ready` therefore heard
/// silence, gave up after its roundtrips, and returned without ever
/// sending `set_image_description` — leaving the surface exactly as it
/// was. That is the "changing HDR and the colour space does nothing"
/// the owner reported: not a wrong picture, no picture at all.
///
/// A free function because it is the only part of this module a test can
/// reach. Everything else here needs a compositor on the other end of a
/// socket; this needs an event, and an event is data.
fn desc_outcome(event: &wp_image_description_v1::Event) -> Option<Result<(), String>> {
    use wp_image_description_v1::Event;
    match event {
        Event::Ready { .. } | Event::Ready2 { .. } => Some(Ok(())),
        Event::Failed { msg, .. } => Some(Err(msg.clone())),
        _ => None,
    }
}

impl Dispatch<WpImageDescriptionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpImageDescriptionV1,
        event: wp_image_description_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match desc_outcome(&event) {
            Some(Ok(())) => state.desc_done = Some(true),
            Some(Err(msg)) => {
                state.desc_done = Some(false);
                state.desc_error = Some(msg);
            }
            None => {}
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
    use super::{desc_outcome, preset};
    use wayland_protocols::wp::color_management::v1::client::wp_image_description_v1 as desc;

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

    /// **A compositor speaking version 2 or later says `ready2`, and it
    /// has to count.**
    ///
    /// This is the regression the owner reported. Nothing about it is
    /// visible from the outside: the description IS built, the compositor
    /// DOES answer, and the answer is thrown away — so the picture stays
    /// as it was and every control on the page keeps its new mark.
    #[test]
    fn both_ready_events_mean_ready_and_failed_carries_its_reason() {
        assert_eq!(
            desc_outcome(&desc::Event::Ready { identity: 7 }),
            Some(Ok(())),
            "a version-1 compositor's answer stopped counting"
        );
        assert_eq!(
            desc_outcome(&desc::Event::Ready2 { identity_hi: 0, identity_lo: 7 }),
            Some(Ok(())),
            "a version-2 compositor's answer was not heard — the colour \
             space is built, accepted, and never set on the surface"
        );
        assert_eq!(
            desc_outcome(&desc::Event::Failed {
                cause: wayland_client::WEnum::Value(desc::Cause::Unsupported),
                msg: "no".to_string(),
            }),
            Some(Err("no".to_string())),
            "a refusal must carry the compositor's own words to the window"
        );
    }
}
