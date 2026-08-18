//! What the machine's screens are: winit's monitors joined with what
//! each monitor says about itself, because three decisions here depend
//! on the SCREEN rather than on pixels — a chassis screen is "10 inches
//! or less" whatever its resolution, a desktop spanning several
//! monitors needs to know which one is which, and one of them carries
//! the MAIN SCREEN role.
//!
//! Two sources answer, and they answer different questions.
//!
//! RandR (x11rb) gives the physical millimetres and the connector each
//! output hangs off; it covers both X11 sessions and XWayland, gamescope
//! included. A pure Wayland session without an X socket simply reports
//! nothing here.
//!
//! The monitor's own description block ([`edid`]) gives its maker,
//! model and serial number, and its picture size in centimetres. It is
//! read from the kernel and so answers in EVERY session, X11 or not.
//!
//! WHICH SCREEN IS WHICH IS THE MONITOR'S ANSWER, NOT THE SOCKET'S.
//! The connector was this program's idea of a screen's identity until
//! 2026-08-18, and it is the wrong one: `DP-1` names a socket on the
//! graphics card, so moving one plug re-points every setting written
//! under that name at a different monitor, and swapping two cables
//! swaps two desktops. What the firmware says travels with the screen.
//! So the connector is now a LABEL — worked out when there is something
//! to show a person, never stored — and [`ScreenId`] is the identity.

pub mod edid;

use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;

/// What marks a configuration key as a monitor's own name rather than a
/// socket's.
///
/// A prefix and not a shape, because the two vocabularies would
/// otherwise overlap: `DEL-41B2-0123ABCD` is dash-separated letters and
/// digits and would pass for a connector under [`connector_of`]'s test.
/// A colon is in no connector name any display server produces, so one
/// glance at a key says which kind it is — to this program and to
/// whoever is editing the file by hand.
pub const EDID_PREFIX: &str = "edid:";

/// The names one screen answers to, most stable first.
///
/// Both may be present, and when they are they mean different things: the
/// identity is THIS MONITOR wherever it is plugged in, the connector is
/// WHATEVER IS PLUGGED IN HERE. Both are legal keys in the configuration
/// and the identity is looked up first, so a rule about a particular
/// monitor beats a rule about a socket — which is the order anybody
/// writing both meant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScreenId {
    /// The body of the monitor's own key, without [`EDID_PREFIX`] —
    /// `DEL-41B2-0123ABCD`. `None` when the monitor published no
    /// description block, or one nothing could be built from.
    pub edid: Option<String>,
    /// The socket this screen hangs off — `DP-1`, `HDMI-A-1`, `eDP-1`.
    /// `None` when nothing names it: a display server reporting only a
    /// marketing string, a headless output.
    pub connector: Option<String>,
}

impl ScreenId {
    /// What one screen is called, given the socket it is on and what
    /// its firmware said.
    pub fn of(connector: Option<&str>, block: Option<&edid::Edid>) -> ScreenId {
        ScreenId {
            edid: block.and_then(|e| e.identity()),
            connector: connector.map(str::to_string),
        }
    }

    /// A screen known only by its socket — what a monitor with no
    /// readable description block gets, and what the tests of the rules
    /// below are written in terms of.
    ///
    /// `allow(dead_code)`: the shipped paths build an id from what the
    /// kernel said ([`ScreenId::of`]), so the only callers are tests —
    /// which is where a rule about sockets has to be stated anyway,
    /// there being no socket to plug a monitor into in a test binary.
    #[allow(dead_code)]
    pub fn of_connector(connector: &str) -> ScreenId {
        ScreenId { edid: None, connector: Some(connector.to_string()) }
    }

    /// Every key this screen may be written under, IN LOOKUP ORDER.
    pub fn keys(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(2);
        if let Some(e) = &self.edid {
            out.push(format!("{EDID_PREFIX}{e}"));
        }
        if let Some(c) = &self.connector {
            out.push(c.clone());
        }
        out
    }

    /// The key a WRITE goes under: the monitor's own name when it has
    /// one, the socket when it has not. `None` for a screen nothing
    /// names, which therefore cannot be given a setting of its own and
    /// simply takes whatever every unnamed screen takes.
    pub fn key(&self) -> Option<String> {
        self.keys().into_iter().next()
    }

    /// Whether one of this screen's keys is the given text. Case is not
    /// significant: RandR says `eDP-1` and a user typing `edp-1` means
    /// that same screen, and the same forgiveness is owed to a monitor's
    /// key, which is longer and worse to type.
    pub fn answers_to(&self, key: &str) -> bool {
        let key = key.trim();
        self.keys().iter().any(|k| k.eq_ignore_ascii_case(key))
    }
}

/// The connector name a display name carries, or None.
///
/// Both display servers are asked the same question and answer it
/// differently: RandR names the output outright (`DP-1`), while a
/// Wayland compositor may hand winit the connector, `DP-1 Dell Inc.
/// U2720Q`, or nothing but the make and model. So the first
/// whitespace-separated word is taken — the convention the EDID
/// lookup reads names by — and kept only if it LOOKS like a connector:
/// letters and digits in dash-separated parts, `HDMI-A-1` as much as
/// `eDP-1`.
///
/// The shape test is what keeps a make and model out: `Dell` is a
/// perfectly good word and a hopeless screen name, because the second
/// Dell on the desk answers to it too.
pub fn connector_of(name: &str) -> Option<String> {
    let token = name.split_whitespace().next()?;
    let parts: Vec<&str> = token.split('-').collect();
    let shaped = parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()));
    shaped.then(|| token.to_string())
}

/// The key a piece of configuration text may name a screen by, or
/// `None` when it names no screen at all.
///
/// THE one statement of the vocabulary: a monitor's own name behind
/// [`EDID_PREFIX`], or a connector. Everything that reads a key and
/// everything that writes one asks here, so a key this program writes
/// is a key it can read back, and a key nothing could ever be matched
/// to is worth neither writing nor reading.
///
/// A monitor key comes back upper-cased, which is how one is generated;
/// a hand-typed `edid:del-41b2` and a written one are then the same
/// line in the file rather than two entries for one screen. A connector
/// is returned exactly as it was typed, because that is what it always
/// did and the display servers disagree about case (`eDP-1`).
pub fn screen_key(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(body) = text.strip_prefix(EDID_PREFIX) {
        return edid_key_body(body).map(|b| format!("{EDID_PREFIX}{b}"));
    }
    connector_of(text).filter(|c| c == text)
}

/// The shape of a monitor key's body: a maker's three letters, a model,
/// and at most one serial, dash-separated.
///
/// A shape test rather than a parse, for the same reason [`connector_of`]
/// is one: what matters is that the text could only ever have come from
/// a monitor, so that nothing else can be written into that half of the
/// file by accident.
fn edid_key_body(body: &str) -> Option<String> {
    let parts: Vec<&str> = body.split('-').collect();
    let shaped = (2..=3).contains(&parts.len())
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()))
        && parts[0].len() == 3
        && parts[0].chars().all(|c| c.is_ascii_alphabetic());
    shaped.then(|| body.to_ascii_uppercase())
}

/// What to write on a screen where a person will read it: in the log,
/// and in the list the settings window will offer.
///
/// WORKED OUT, NEVER STORED. The monitor's model is what a person
/// recognises — they bought a U2720Q, not a `DEL-41B2` — and the socket
/// is what tells two of the same model apart, so both go in when both
/// are known. `None` when neither is: the caller then has the winit
/// name to fall back on, which this function cannot see.
pub fn label(model: Option<&str>, connector: Option<&str>) -> Option<String> {
    match (model, connector) {
        (Some(m), Some(c)) => Some(format!("{m} ({c})")),
        (Some(m), None) => Some(m.to_string()),
        (None, Some(c)) => Some(c.to_string()),
        (None, None) => None,
    }
}

/// WHAT THE MAIN SCREEN ROLE MEANS. Four duties, one screen, one
/// setting — written down here because a role with no definition
/// collects a different one from every reader.
///
/// Which screen carries it was the display server's answer alone until
/// 2026-08-18 (RandR's primary output, or winit's, or whichever monitor
/// sits at the origin). That answer is still the DEFAULT, and it is now
/// only a default: the configuration names a screen by its identity and
/// that screen carries the role — see [`main_screen`].
///
/// A duty listed here is a duty of THE ROLE, whether or not the program
/// has grown the thing that discharges it yet. Two of the four are
/// live; the other two name work that does not exist, and they are
/// written down anyway so that when it is built it is built onto this
/// setting instead of inventing a second one beside it.
///
/// `allow(dead_code)`: this type is a DEFINITION and is meant to be
/// one. Nothing matches on it — the two live duties are discharged by
/// code that takes the first screen of a list sorted by the role, which
/// is the cheapest way to make a role mean something and the reason it
/// was possible to have the role without ever writing down what it is.
/// The day a duty needs to be named in code (a notification asking
/// where to go, a window asking where to open) it is named from here.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainScreenDuty {
    /// **Where virtual desktop 1 sits after start.** NOT LIVE: virtual
    /// desktops do not exist in this program yet.
    ///
    /// And when they do, they are a thing of their own. A VIRTUAL
    /// DESKTOP is a set of windows the user switches between; a BOARD
    /// (`BoardWorld`, `BoardId`) is a face of the cube one screen
    /// turns, and every screen has its own. They are two independent
    /// mechanisms and must not be merged in the model or in the names —
    /// this duty is about the first of them and says nothing about the
    /// second.
    VirtualDesktopOne,
    /// **Where a new window goes when nothing says where.** NOT LIVE:
    /// this program does not place other applications' windows yet, and
    /// will when it is the compositor.
    ///
    /// It is the role's duty rather than the window manager's policy,
    /// so that "the main screen" means the same thing to the person
    /// setting it as to the window that lands there.
    UnplacedWindow,
    /// **Where the main panel stands.** LIVE. The first element of the
    /// surveyed list is the screen the desktop treats as its own: the
    /// theme engine's viewport is taken from it, the layaut editor
    /// edits its screen, and the gamescope handshake is made on its
    /// window. Sorting the list by this role is what makes all three
    /// follow the setting.
    MainPanel,
    /// **Where notifications appear.** LIVE IN PART: what the desktop
    /// has today is the rescue notice and the settings window's own
    /// reports, which are drawn on the main screen because that is the
    /// screen the desktop draws its own furniture on. A notification
    /// service for other applications does not exist.
    Notification,
}

/// Every duty the role carries. Derived from nothing and read by the
/// test that keeps this list and the type above in step: a duty added
/// to one and not the other is a duty nobody has to think about, which
/// is how a role quietly comes to mean four different things.
#[allow(dead_code)]
pub const MAIN_SCREEN_DUTIES: [MainScreenDuty; 4] = [
    MainScreenDuty::VirtualDesktopOne,
    MainScreenDuty::UnplacedWindow,
    MainScreenDuty::MainPanel,
    MainScreenDuty::Notification,
];

/// Which screen carries the role, as an index into the surveyed list.
///
/// Three answers in order of authority: the screen the configuration
/// names, then the display server's own (whatever the caller worked
/// out), then the first screen there is — somebody must be the main
/// screen, and a desktop with no answer at all would have nowhere to
/// put its own furniture.
///
/// Pure, and handed everything it judges by, because this decision has
/// to be testable on a machine that has no screens.
///
/// A configuration naming a screen that is not plugged in today is not
/// an error and not a reason to fall back loudly: the monitor is simply
/// off or elsewhere, the setting is still theirs, and the machine picks
/// up where the display server says until it comes back.
pub fn main_screen(
    ids: &[ScreenId],
    chosen: Option<&str>,
    server_said: Option<usize>,
) -> Option<usize> {
    if let Some(key) = chosen {
        if let Some(i) = ids.iter().position(|id| id.answers_to(key)) {
            return Some(i);
        }
    }
    server_said
        .filter(|&i| i < ids.len())
        .or_else(|| (!ids.is_empty()).then_some(0))
}

pub struct Screen {
    pub monitor: MonitorHandle,
    /// The socket this screen hangs off — a LABEL, and no longer an
    /// identity: [`Screen::id`] is what the configuration keys by.
    pub connector: Option<String>,
    /// What this monitor said about itself, if anything.
    pub edid: Option<edid::Edid>,
    /// Physical diagonal in inches, when anything says.
    pub diagonal_in: Option<f32>,
    /// Whether this screen carries the MAIN SCREEN role — see
    /// [`MainScreenDuty`]. The surveyed list is sorted by it, so the
    /// screen holding the role is always index 0.
    pub primary: bool,
}

impl Screen {
    /// What the configuration calls this screen.
    pub fn id(&self) -> ScreenId {
        ScreenId::of(self.connector.as_deref(), self.edid.as_ref())
    }

    /// What a person calls it. Falls back to the display server's own
    /// name for the monitor, which is all there is left to say.
    pub fn label(&self) -> String {
        label(self.edid.as_ref().and_then(|e| e.model()).as_deref(), self.connector.as_deref())
            .or_else(|| self.monitor.name())
            .unwrap_or_else(|| "?".into())
    }
}

/// What the configuration calls the screen on one connector, read from
/// the kernel.
///
/// Remembered per connector: a monitor does not change its firmware
/// while it is plugged in, and this is asked once by the survey and
/// again by every window that comes up on that screen.
pub fn identify(connector: Option<&str>) -> ScreenId {
    let Some(c) = connector else { return ScreenId::default() };
    ScreenId::of(Some(c), read_edid(c).as_ref())
}

/// The description block for one connector, remembered.
pub fn read_edid(connector: &str) -> Option<edid::Edid> {
    thread_local! {
        static SEEN: std::cell::RefCell<std::collections::HashMap<String, Option<edid::Edid>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    if let Some(known) = SEEN.with(|c| c.borrow().get(connector).cloned()) {
        return known;
    }
    let block = edid::read(connector);
    SEEN.with(|c| c.borrow_mut().insert(connector.to_string(), block.clone()));
    block
}

/// Every monitor the event loop can see, joined with what it says about
/// itself and with RandR's physical sizes, THE MAIN SCREEN FIRST.
///
/// Which one is main is answered by [`main_screen`] over three sources,
/// most authoritative first: the configuration, then the display server
/// (winit's own answer, which X11 has; then RandR's primary output
/// matched by geometry, because a native Wayland winit answers None
/// while the session's XWayland still knows), then the monitor at the
/// origin.
///
/// This is also where the configuration is brought up to date with what
/// the machine can see, and it has to be here: rewriting a key that
/// names a socket into one that names a monitor takes knowing which
/// monitor is on that socket, and this is the one moment the program
/// learns it. Nothing is written when nothing needs moving.
pub fn survey(el: &EventLoop<()>) -> Vec<Screen> {
    let winit_primary = el.primary_monitor();
    let (physical, randr_primary) = randr_info().unwrap_or((Vec::new(), None));
    let mut out: Vec<Screen> = el
        .available_monitors()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            let phys = physical
                .iter()
                .find(|p| {
                    p.x == pos.x && p.y == pos.y && p.px_w == size.width && p.px_h == size.height
                })
                .or_else(|| {
                    // A second chance by pixel size alone: a compositor
                    // may shift logical positions while CRTCs stay put.
                    physical
                        .iter()
                        .find(|p| p.px_w == size.width && p.px_h == size.height)
                });
            // RandR names the output for what it is, so it is asked
            // first; winit's own name is the answer on a session with
            // no X socket, where it is all there is.
            let connector = phys
                .and_then(|p| connector_of(&p.name))
                .or_else(|| m.name().as_deref().and_then(connector_of));
            let block = connector.as_deref().and_then(read_edid);
            // RandR's millimetres first, the monitor's own centimetres
            // behind them. The second reading is coarser — whole
            // centimetres — but it is the only one a pure Wayland
            // session has, and a chassis screen that measures nothing
            // is a chassis screen this program walks past.
            let diagonal_in = phys
                .and_then(|p| diagonal(p.mm_w, p.mm_h))
                .or_else(|| block.as_ref().and_then(|e| e.diagonal_in()));
            let primary = winit_primary
                .as_ref()
                .map(|p| *p == m)
                .unwrap_or_else(|| {
                    phys.map(|p| Some(p.output) == randr_primary).unwrap_or(false)
                });
            Screen { monitor: m, connector, edid: block, diagonal_in, primary }
        })
        .collect();
    // What the display server made of it, before the configuration is
    // asked: its own answer, or the monitor at the origin.
    let server_said = out.iter().position(|s| s.primary).or_else(|| {
        out.iter()
            .position(|s| s.monitor.position() == winit::dpi::PhysicalPosition::new(0, 0))
    });
    let ids: Vec<ScreenId> = out.iter().map(Screen::id).collect();
    // The configuration is brought up to date BEFORE the role is read,
    // so a role written against a socket-keyed file is read out of the
    // document the migration has just moved it into.
    crate::config::migrate_screen_identities(&ids);
    let chosen = crate::config::main_screen_key();
    let holder = main_screen(&ids, chosen.as_deref(), server_said);
    for (i, s) in out.iter_mut().enumerate() {
        s.primary = Some(i) == holder;
    }
    // The main screen first, so the role's live duties — the viewport,
    // the editor's screen, the desktop's own furniture — all follow it
    // by indexing 0.
    out.sort_by_key(|s| !s.primary);
    for s in &out {
        // The KEY is said out loud beside the label, because it is what
        // the user has to write down to give this screen a layaut or the
        // main role of its own, and working it out from a monitor's
        // model is not something anybody should have to do.
        eprintln!(
            "nacelle-desktop: screen '{}' {}x{}{}{} \u{2014} key {}",
            s.label(),
            s.monitor.size().width,
            s.monitor.size().height,
            s.diagonal_in.map(|d| format!(" {d:.1}\"")).unwrap_or_default(),
            if s.primary { " \u{2014} MAIN SCREEN" } else { "" },
            s.id().key().unwrap_or_else(|| "(none \u{2014} unnamed screen)".into()),
        );
    }
    out
}

/// The smallest screen at or under the chassis threshold — the little
/// panel in a computer case this program makes a fine face for.
pub fn chassis(screens: &[Screen]) -> Option<&Screen> {
    screens
        .iter()
        .filter(|s| s.diagonal_in.map(|d| d <= 10.0).unwrap_or(false))
        .min_by(|a, b| {
            a.diagonal_in
                .partial_cmp(&b.diagonal_in)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn diagonal(mm_w: u32, mm_h: u32) -> Option<f32> {
    // RandR reports 0x0 for projectors, headless outputs and anything
    // whose EDID keeps quiet — no measurement, not a tiny screen.
    if mm_w == 0 || mm_h == 0 {
        return None;
    }
    let (w, h) = (mm_w as f32, mm_h as f32);
    Some((w * w + h * h).sqrt() / 25.4)
}

struct PhysOutput {
    output: u32,
    /// The connector RandR calls this output, verbatim.
    name: String,
    x: i32,
    y: i32,
    px_w: u32,
    px_h: u32,
    mm_w: u32,
    mm_h: u32,
}

/// Connected RandR outputs with their CRTC geometry and physical size,
/// plus the server's primary output. Any failure — no X socket, no
/// RandR, a racing disconnect — is an empty answer.
fn randr_info() -> Option<(Vec<PhysOutput>, Option<u32>)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let res = conn.randr_get_screen_resources_current(root).ok()?.reply().ok()?;
    let primary = conn
        .randr_get_output_primary(root)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.output)
        .filter(|&o| o != x11rb::NONE);
    let mut out = Vec::new();
    for &o in &res.outputs {
        let Ok(info) = conn.randr_get_output_info(o, res.config_timestamp) else { continue };
        let Ok(info) = info.reply() else { continue };
        if info.crtc == x11rb::NONE {
            continue; // not driving anything
        }
        let Ok(crtc) = conn.randr_get_crtc_info(info.crtc, res.config_timestamp) else { continue };
        let Ok(crtc) = crtc.reply() else { continue };
        out.push(PhysOutput {
            output: o,
            name: String::from_utf8_lossy(&info.name).into_owned(),
            x: crtc.x as i32,
            y: crtc.y as i32,
            px_w: crtc.width as u32,
            px_h: crtc.height as u32,
            mm_w: info.mm_width,
            mm_h: info.mm_height,
        });
    }
    Some((out, primary))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name a screen is configured by. Every form the two display
    /// servers hand over resolves to the same connector, and anything
    /// that would not identify a screen tomorrow resolves to nothing —
    /// a screen with no name takes the default layaut rather than
    /// borrowing another screen's.
    #[test]
    fn a_screen_is_named_by_its_connector_or_not_at_all() {
        for name in ["DP-1", "HDMI-A-1", "eDP-1", "DVI-I-1", "Virtual-1"] {
            assert_eq!(connector_of(name).as_deref(), Some(name), "{name} is a connector");
        }
        // Wayland hands over the connector with the model behind it.
        assert_eq!(connector_of("DP-1 Dell Inc. U2720Q").as_deref(), Some("DP-1"));
        // A make and model alone names no socket: two of them answer
        // to the same word, so it may not become a key.
        for name in ["", "?", "Dell", "Dell Inc. U2720Q", "-1", "DP-"] {
            assert!(connector_of(name).is_none(), "'{name}' must not name a screen");
        }
    }

    /// The arithmetic the chassis rule hangs on: a 217x136 mm panel is
    /// the classic 10.1", a 0x0 EDID is no measurement at all.
    #[test]
    fn a_diagonal_is_inches_or_nothing() {
        let d = diagonal(217, 136).unwrap();
        assert!((d - 10.08).abs() < 0.05, "217x136 mm is a 10.1\" panel, got {d}");
        let d = diagonal(598, 336).unwrap();
        assert!((d - 27.0).abs() < 0.2, "598x336 mm is a 27\" monitor, got {d}");
        assert!(diagonal(0, 336).is_none(), "a silent EDID measures nothing");
    }

    /// A screen's identity is what the MONITOR said, and the socket is
    /// only what is left when it said nothing.
    #[test]
    fn a_monitor_names_itself_and_the_socket_is_the_fallback() {
        let block =
            edid::parse(&edid::tests::block("DEL", 0x41B2, 0x0123ABCD, (60, 34), None, None))
                .unwrap();
        let plugged = ScreenId::of(Some("DP-1"), Some(&block));
        assert_eq!(
            plugged.keys(),
            vec!["edid:DEL-41B2-0123ABCD".to_string(), "DP-1".to_string()],
            "the monitor's own name is looked up before the socket's"
        );
        assert_eq!(plugged.key().as_deref(), Some("edid:DEL-41B2-0123ABCD"));

        // THE POINT OF THE WHOLE CHANGE: the same monitor moved to
        // another socket is the same screen.
        let moved = ScreenId::of(Some("HDMI-A-1"), Some(&block));
        assert_eq!(moved.edid, plugged.edid, "one monitor, one identity, whatever it hangs off");
        assert!(moved.answers_to("edid:DEL-41B2-0123ABCD"));
        assert!(moved.answers_to("EDID:del-41b2-0123abcd"), "a key is not case sensitive");
        assert!(!moved.answers_to("DP-1"), "and it is no longer the screen on DP-1");

        // A monitor that says nothing still hangs off a socket.
        let silent = ScreenId::of(Some("DP-2"), None);
        assert_eq!(silent.keys(), vec!["DP-2".to_string()]);
        // A screen nothing names at all cannot be given a setting.
        assert!(ScreenId::of(None, None).key().is_none());
    }

    /// The two key vocabularies, and the reason they cannot be
    /// confused: a monitor key would pass the connector's own shape
    /// test, so the prefix is what tells them apart.
    #[test]
    fn a_key_names_a_monitor_or_a_socket_and_the_prefix_says_which() {
        assert_eq!(screen_key("edid:DEL-41B2-0123ABCD").as_deref(), Some("edid:DEL-41B2-0123ABCD"));
        assert_eq!(screen_key("edid:SAM-000F").as_deref(), Some("edid:SAM-000F"));
        assert_eq!(
            screen_key("edid:del-41b2-0123abcd").as_deref(),
            Some("edid:DEL-41B2-0123ABCD"),
            "a hand-typed monitor key is the same entry as a written one"
        );
        assert_eq!(screen_key(" DP-1 ").as_deref(), Some("DP-1"), "a connector keeps its case");
        assert_eq!(screen_key("eDP-1").as_deref(), Some("eDP-1"));
        // Without the prefix a monitor key WOULD pass for a connector,
        // which is the whole reason there is one.
        assert_eq!(
            connector_of("DEL-41B2-0123ABCD").as_deref(),
            Some("DEL-41B2-0123ABCD"),
            "the shapes overlap"
        );
        for bad in [
            "",
            "edid:",
            "edid:DEL",
            "edid:DELL-41B2",
            "edid:DE1-41B2",
            "edid:DEL-41B2-0123-EXTRA",
            "edid:DEL--41B2",
            "Dell Inc. U2720Q",
            "screen 2",
        ] {
            assert!(screen_key(bad).is_none(), "'{bad}' must not become a key");
        }
    }

    /// What a person reads. The model is what they recognise and the
    /// socket is what tells two of the same model apart, so both go in
    /// when both are known — and the label is worked out here rather
    /// than stored, which is what keeps it out of the configuration.
    #[test]
    fn a_screen_is_labelled_by_its_model_and_its_socket() {
        assert_eq!(label(Some("U2720Q"), Some("DP-1")).as_deref(), Some("U2720Q (DP-1)"));
        assert_eq!(label(Some("U2720Q"), None).as_deref(), Some("U2720Q"));
        assert_eq!(label(None, Some("DP-1")).as_deref(), Some("DP-1"));
        assert!(label(None, None).is_none(), "the caller has the display server's name left");
    }

    /// The role: the configuration outranks the display server, the
    /// display server outranks the order the monitors came up in, and
    /// somebody is always the main screen.
    #[test]
    fn the_main_screen_is_the_one_the_configuration_names() {
        let dell = ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-1".into()) };
        let lg = ScreenId { edid: Some("GSM-5B0F".into()), connector: Some("HDMI-A-1".into()) };
        let ids = [dell.clone(), lg.clone()];

        assert_eq!(
            main_screen(&ids, Some("edid:GSM-5B0F"), Some(0)),
            Some(1),
            "the setting beats the display server"
        );
        assert_eq!(
            main_screen(&ids, Some("HDMI-A-1"), Some(0)),
            Some(1),
            "a socket names a screen too, for a monitor that gives no name"
        );
        assert_eq!(main_screen(&ids, None, Some(1)), Some(1), "with nothing set, the server");
        assert_eq!(
            main_screen(&ids, Some("edid:ACR-0001"), Some(1)),
            Some(1),
            "a screen that is not plugged in today keeps its setting and changes nothing"
        );
        assert_eq!(main_screen(&ids, None, None), Some(0), "somebody must be the main screen");
        assert_eq!(
            main_screen(&ids, None, Some(7)),
            Some(0),
            "and an answer pointing off the end of the list is no answer"
        );
        assert_eq!(main_screen(&[], Some("DP-1"), None), None, "a desktop with no screens");

        // The role travels with the MONITOR: the same two screens with
        // their cables swapped keep the same main screen, which is what
        // an index or a connector could not do.
        let swapped = [
            ScreenId { connector: Some("HDMI-A-1".into()), ..dell },
            ScreenId { connector: Some("DP-1".into()), ..lg },
        ];
        assert_eq!(main_screen(&swapped, Some("edid:GSM-5B0F"), Some(0)), Some(1));
    }

    /// The role's duties are written down once. A duty added to the
    /// type and not to the list is a duty no reader of the list has to
    /// think about.
    #[test]
    fn the_main_screen_role_means_four_things_and_they_are_all_written_down() {
        use MainScreenDuty::*;
        for duty in [VirtualDesktopOne, UnplacedWindow, MainPanel, Notification] {
            assert!(MAIN_SCREEN_DUTIES.contains(&duty), "{duty:?} is a duty of the role");
        }
        assert_eq!(MAIN_SCREEN_DUTIES.len(), 4);
    }
}
