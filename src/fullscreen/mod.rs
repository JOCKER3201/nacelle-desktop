//! The window-management connector: WHAT is done to a window, kept
//! apart from WHO carries it out.
//!
//! # Why this module is called `fullscreen`
//!
//! It is the connector; the name is a leftover and a one-line change.
//! `main.rs:15` says `mod fullscreen;` and that file is held by another
//! fleet, so the module keeps the old name until the rename
//! (`mod fullscreen;` → `mod wm;` plus `git mv src/fullscreen src/wm`)
//! can be made in one commit. Nothing else in the tree depends on the
//! name: [`Fullscreen`] is re-exported below with the signature
//! `main.rs` already calls.
//!
//! # The seam
//!
//! One vocabulary — [`Verb`] for what can be asked, [`Act`] for asking,
//! [`Window`] for what came back — and one trait, [`Backend`], for the
//! thing that actually speaks to a compositor. Two carriers are built:
//!
//!   * [`wayland`] — `ext-foreign-toplevel-list-v1`, reading only. The
//!     protocol a compositor of our own will speak.
//!   * [`x11`] — EWMH over XWayland, reading AND the whole control
//!     vocabulary. This is where the gamescope "make everything
//!     fullscreen" policy went: it did not become rubbish, it became a
//!     mode of a backend ([`x11::Policy`]).
//!
//! A third seat is left for the compositor of our own, which will need
//! no protocol at all — it will hold the window list itself and
//! implement [`Backend`] against its own state.
//!
//! # Why a snapshot and an epoch, not callbacks
//!
//! The desktop draws every frame from state it owns. A backend that
//! called back into the interface would need the interface to be
//! reachable from a Wayland dispatch, and the ordering between "a
//! window appeared" and "the frame is being laid out" would be nobody's
//! to state. So: [`Connector::poll`] once a frame drains the carrier,
//! [`Connector::windows`] hands back a snapshot, and
//! [`Connector::epoch`] only moves when something actually changed.
//!
//! That last part is not decoration. The same shape, gotten wrong,
//! is what pinned a CPU at 100 %: `theme::epoch()` answered "which bake
//! is published", which alternates every frame with two screens of
//! different heights, and the font system re-read every font on disk
//! sixty times a second. An epoch that ticks when nothing happened is
//! not a harmless epoch.
//!
//! # Why `ext-foreign-toplevel-list-v1` and not the Plasma protocol
//!
//! Measured on the owner's machine, 2026-08-18, KWin 6.7.4:
//!
//! ```text
//! wayland-info                       70 globals, and among them
//!   ext_foreign_toplevel_list_v1     ABSENT
//!   org_kde_plasma_window_management  ABSENT
//! strings /usr/lib64/libkwin.so.6.7.4
//!   org_kde_plasma_window_management  present  (implemented, not handed out)
//!   ext_foreign_toplevel_list_v1      absent   (not implemented at all)
//! ```
//!
//! So the Plasma protocol is implemented by this KWin and *still* not
//! reachable from an ordinary client — it is only given to the clients
//! KWin trusts. Carrying an LGPL protocol description into the tree
//! would therefore buy nothing at all: the list would be as empty as
//! the neutral one, and the licence question would be real. That is the
//! whole argument. The neutral protocol wins by not costing anything
//! and by being the one every other compositor speaks — wlroots,
//! COSMIC, niri, Hyprland, Mutter, and Smithay, which ours will stand
//! on.
//!
//! Nothing is copied either way: the XML lives inside
//! `wayland-protocols 0.32.13`, which is ALREADY a locked dependency
//! with the `staging` feature ALREADY on, and the crate generates the
//! bindings from its own vendored copy. This branch adds no dependency
//! and does not touch `Cargo.toml` or `Cargo.lock`. The XML's own
//! header is an X11-style permissive grant (Bozhinov, Freund, wb9688,
//! i509VCB), not LGPL.
//!
//! What the neutral choice costs, said plainly: `ext-foreign-toplevel-list-v1`
//! carries title, app id and a stable identifier, and nothing else — no
//! state, no icon, no board, and no way to act. Those are filled in
//! later by protocols from the same crate (`ext-workspace-v1` for
//! boards) or by the compositor of our own. Under bare KWin they are
//! filled in by the EWMH backend, for X11 clients only.

#![allow(dead_code)] // The call site is `main.rs`, held by another fleet.

use std::collections::HashMap;

pub mod wayland;
pub mod x11;

pub use x11::Fullscreen;

/// A window's identity for as long as it is mapped.
///
/// Minted here, never the server's own number. Both carriers have a
/// native key — an X11 window id, a Wayland object id — and both reuse
/// them: X11 hands the same id to a new client once the old one is
/// gone, and the Wayland object id is a slot in a table. A number the
/// interface is holding on to must not quietly start meaning a
/// different window, so the native key is kept private and this is what
/// travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u64);

/// Where a window is, in the carrier's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// The four states a window can be told to be in and asked about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub active: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// What a window looks like in a list, as far as a carrier can say.
///
/// Two shapes because the two carriers answer differently and neither
/// answer is a paraphrase of the other: EWMH puts the pixels on the
/// window itself (`_NET_WM_ICON`), the Wayland protocol gives an app id
/// and expects the icon theme to be searched. Flattening one into the
/// other would mean inventing pixels or inventing a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A name to look up in the icon theme.
    Named(String),
    /// Pre-multiplied ARGB, row-major, `w * h` long.
    Pixels { w: u32, h: u32, argb: Vec<u32> },
}

/// One thing the desktop knows how to want from a window.
///
/// A closed list, and every carrier answers [`Backend::can`] for each
/// one. That is the same discipline the COLOR page runs on: a control
/// the carrier cannot honour is not drawn enabled, because a button
/// that does nothing is worse than a button that is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verb {
    /// There is a list of windows at all.
    List,
    Title,
    App,
    Icon,
    Focus,
    Close,
    Minimize,
    Maximize,
    Fullscreen,
    /// Move and resize; one verb, because every carrier that can do
    /// either can do both.
    Place,
    /// Which board (virtual desktop) the window sits on.
    Board,
}

impl Verb {
    /// Every verb, once. Tests walk this so a verb added to the
    /// vocabulary cannot be forgotten by a carrier in silence.
    pub const ALL: [Verb; 11] = [
        Verb::List,
        Verb::Title,
        Verb::App,
        Verb::Icon,
        Verb::Focus,
        Verb::Close,
        Verb::Minimize,
        Verb::Maximize,
        Verb::Fullscreen,
        Verb::Place,
        Verb::Board,
    ];

    /// The name to write in a log line or under a greyed-out control.
    pub fn label(self) -> &'static str {
        match self {
            Verb::List => "list",
            Verb::Title => "title",
            Verb::App => "app id",
            Verb::Icon => "icon",
            Verb::Focus => "focus",
            Verb::Close => "close",
            Verb::Minimize => "minimize",
            Verb::Maximize => "maximize",
            Verb::Fullscreen => "fullscreen",
            Verb::Place => "move and resize",
            Verb::Board => "board",
        }
    }
}

/// An order, addressed to one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Focus(WindowId),
    Close(WindowId),
    Minimize(WindowId, bool),
    Maximize(WindowId, bool),
    Fullscreen(WindowId, bool),
    Place(WindowId, Place),
    SendToBoard(WindowId, u32),
}

impl Act {
    pub fn verb(self) -> Verb {
        match self {
            Act::Focus(..) => Verb::Focus,
            Act::Close(..) => Verb::Close,
            Act::Minimize(..) => Verb::Minimize,
            Act::Maximize(..) => Verb::Maximize,
            Act::Fullscreen(..) => Verb::Fullscreen,
            Act::Place(..) => Verb::Place,
            Act::SendToBoard(..) => Verb::Board,
        }
    }

    pub fn who(self) -> WindowId {
        match self {
            Act::Focus(id)
            | Act::Close(id)
            | Act::Minimize(id, _)
            | Act::Maximize(id, _)
            | Act::Fullscreen(id, _)
            | Act::Place(id, _)
            | Act::SendToBoard(id, _) => id,
        }
    }

    /// A specimen of every verb, for tests that must walk the whole
    /// vocabulary against a carrier.
    pub fn specimen(verb: Verb, id: WindowId) -> Option<Act> {
        Some(match verb {
            Verb::List | Verb::Title | Verb::App | Verb::Icon => return None,
            Verb::Focus => Act::Focus(id),
            Verb::Close => Act::Close(id),
            Verb::Minimize => Act::Minimize(id, true),
            Verb::Maximize => Act::Maximize(id, true),
            Verb::Fullscreen => Act::Fullscreen(id, true),
            Verb::Place => Act::Place(id, Place { x: 0, y: 0, w: 640, h: 480 }),
            Verb::Board => Act::SendToBoard(id, 0),
        })
    }
}

/// What came of an order.
///
/// Four answers and not two. "The carrier does not do this at all" and
/// "the carrier tried and it did not work" are different sentences to
/// write under a control, and "I have never heard of that window" is a
/// third — it is what a stale identity earns, and it must not read as a
/// failure of the compositor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Sent. Wayland and X11 are both asynchronous and neither answers
    /// an order, so this says the request left, not that it was obeyed.
    Sent,
    Unsupported,
    Unknown(WindowId),
    Failed(String),
}

/// One window, as the interface sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub app: String,
    /// None where the carrier cannot say — not "board zero".
    pub board: Option<u32>,
    pub state: State,
    pub place: Option<Place>,
}

impl Window {
    pub fn new(id: WindowId) -> Window {
        Window {
            id,
            title: String::new(),
            app: String::new(),
            board: None,
            state: State::default(),
            place: None,
        }
    }
}

/// Whoever actually talks to a compositor.
pub trait Backend {
    /// For a log line and for the settings page.
    fn carrier(&self) -> &'static str;

    /// Whether this carrier does this verb at all. Asked BEFORE a
    /// control is drawn.
    fn can(&self, verb: Verb) -> bool;

    /// What this carrier cannot see, in words fit to print under an
    /// empty list. An empty list with no explanation reads as "no
    /// windows are open", which is a lie the EWMH carrier tells on a
    /// Wayland session every time.
    fn blind_spot(&self) -> Option<&'static str>;

    /// Drain whatever the compositor has said. True when the list came
    /// out different.
    fn poll(&mut self) -> bool;

    fn windows(&self) -> &[Window];

    /// Fetched on demand, not on every poll: EWMH icons are megabytes
    /// of pixels sitting on a window property.
    fn icon(&mut self, id: WindowId) -> Option<Icon>;

    fn act(&mut self, act: Act) -> Outcome;
}

/// The identity mint, shared by both carriers so there is one rule.
#[derive(Default)]
pub struct Names {
    next: u64,
    by_native: HashMap<u64, WindowId>,
}

impl Names {
    pub fn new() -> Names {
        Names { next: 1, by_native: HashMap::new() }
    }

    /// The id for a native key, minting one the first time.
    pub fn of(&mut self, native: u64) -> WindowId {
        if let Some(&id) = self.by_native.get(&native) {
            return id;
        }
        if self.next == 0 {
            self.next = 1;
        }
        let id = WindowId(self.next);
        self.next += 1;
        self.by_native.insert(native, id);
        id
    }

    /// The key is dead. The next window to be handed the same native
    /// key gets a NEW identity.
    pub fn forget(&mut self, native: u64) {
        self.by_native.remove(&native);
    }

    /// Everything not in `alive` is dead. The X11 carrier learns of
    /// departures by a list arriving without them, never by an event.
    pub fn retain(&mut self, alive: &[u64]) {
        self.by_native.retain(|k, _| alive.contains(k));
    }

    pub fn native(&self, id: WindowId) -> Option<u64> {
        self.by_native.iter().find(|(_, &v)| v == id).map(|(&k, _)| k)
    }
}

/// The connector the desktop holds: one carrier, and the frame-by-frame
/// discipline around it.
pub struct Connector {
    back: Box<dyn Backend>,
    epoch: u64,
}

impl Connector {
    /// Picks a carrier. Wayland first, and only if the compositor
    /// actually advertises the list — on a compositor that does not
    /// (KWin 6.7.4, measured) falling through to EWMH is the difference
    /// between seeing the X11 clients and seeing nothing.
    ///
    /// `wayland_display` is winit's own display pointer, the same one
    /// `wl_color` is handed.
    pub fn start(wayland_display: Option<*mut std::ffi::c_void>) -> Option<Connector> {
        if let Some(d) = wayland_display {
            if let Some(b) = wayland::Toplevels::start(d) {
                return Some(Connector::over(Box::new(b)));
            }
        }
        x11::Ewmh::start(x11::Policy::Observe, None).map(|b| Connector::over(Box::new(b)))
    }

    pub fn over(back: Box<dyn Backend>) -> Connector {
        Connector { back, epoch: 0 }
    }

    /// Once a frame. The epoch moves only when the list did.
    pub fn poll(&mut self) {
        if self.back.poll() {
            self.epoch = self.epoch.wrapping_add(1);
        }
    }

    /// What to compare against last frame's. Never a clock, never a
    /// counter of polls.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn windows(&self) -> &[Window] {
        self.back.windows()
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.back.windows().iter().find(|w| w.id == id)
    }

    pub fn can(&self, verb: Verb) -> bool {
        self.back.can(verb)
    }

    pub fn icon(&mut self, id: WindowId) -> Option<Icon> {
        self.back.icon(id)
    }

    pub fn act(&mut self, act: Act) -> Outcome {
        self.back.act(act)
    }

    pub fn carrier(&self) -> &'static str {
        self.back.carrier()
    }

    pub fn blind_spot(&self) -> Option<&'static str> {
        self.back.blind_spot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A carrier that says yes to some verbs and no to others, and
    /// counts how often it was asked to look.
    struct Toy {
        list: Vec<Window>,
        yes: Vec<Verb>,
        news: Vec<bool>,
        polls: usize,
    }

    impl Toy {
        fn new(yes: &[Verb]) -> Toy {
            Toy { list: Vec::new(), yes: yes.to_vec(), news: Vec::new(), polls: 0 }
        }
    }

    impl Backend for Toy {
        fn carrier(&self) -> &'static str {
            "toy"
        }
        fn can(&self, verb: Verb) -> bool {
            self.yes.contains(&verb)
        }
        fn blind_spot(&self) -> Option<&'static str> {
            None
        }
        fn poll(&mut self) -> bool {
            self.polls += 1;
            if self.news.is_empty() {
                false
            } else {
                self.news.remove(0)
            }
        }
        fn windows(&self) -> &[Window] {
            &self.list
        }
        fn icon(&mut self, _: WindowId) -> Option<Icon> {
            None
        }
        fn act(&mut self, act: Act) -> Outcome {
            if !self.can(act.verb()) {
                return Outcome::Unsupported;
            }
            Outcome::Sent
        }
    }

    /// **An epoch that moves when nothing happened is the bug that
    /// pinned a CPU at 100 %.**
    ///
    /// `theme::epoch()` answered "which bake is published", which
    /// alternates every frame on two screens of unequal height, and the
    /// font system took that for news and re-read every font on disk
    /// sixty times a second (`.gap-program/usterka-cpu-desktop.md`;
    /// measured 100,7 % → 10,6 %). Whatever the window list is fed
    /// into will memoise on this number exactly the same way, so the
    /// number has to be silent on a quiet frame — polling is not news.
    ///
    /// The assertion is on a hundred polls and not one, because a
    /// single quiet poll can be got right by an epoch that ticks every
    /// other time.
    #[test]
    fn a_quiet_frame_does_not_move_the_epoch() {
        let mut c = Connector::over(Box::new(Toy::new(&[])));
        let start = c.epoch();
        for _ in 0..100 {
            c.poll();
        }
        assert_eq!(
            c.epoch(),
            start,
            "the epoch moved on frames where the carrier reported no change — \
             every reader memoising on it will rebuild sixty times a second"
        );

        let mut noisy = Toy::new(&[]);
        noisy.news = vec![false, true, false];
        let mut c = Connector::over(Box::new(noisy));
        c.poll();
        assert_eq!(c.epoch(), 0, "silence counted as news");
        c.poll();
        assert_eq!(c.epoch(), 1, "news did not count");
        c.poll();
        assert_eq!(c.epoch(), 1, "silence after news counted as news");
    }

    /// **A verb the carrier says no to must also do nothing when
    /// asked.**
    ///
    /// The two are read by two different callers — `can` by whatever
    /// decides whether to draw the control, `act` by the click — and
    /// they are allowed to be written in two places, so only this holds
    /// them together. A carrier that says no and then quietly obeys is
    /// a control the interface has greyed out for no reason; one that
    /// says yes and answers `Unsupported` is the button that does
    /// nothing, which is the failure this whole seam exists to prevent.
    #[test]
    fn a_carrier_that_says_no_does_nothing_and_says_so() {
        let allowed = [Verb::Focus, Verb::Close];
        let mut toy = Toy::new(&allowed);
        let id = WindowId(1);
        for verb in Verb::ALL {
            let Some(act) = Act::specimen(verb, id) else { continue };
            let answer = toy.act(act);
            if allowed.contains(&verb) {
                assert_ne!(
                    answer,
                    Outcome::Unsupported,
                    "the carrier offers '{}' and then refuses to do it — \
                     the control would be drawn live and do nothing",
                    verb.label()
                );
            } else {
                assert_eq!(
                    answer,
                    Outcome::Unsupported,
                    "the carrier does not offer '{}' and did it anyway",
                    verb.label()
                );
            }
        }
    }

    /// **Every verb has a specimen, or is a reading verb.**
    ///
    /// [`Act::specimen`] is what the walk above rides on. A verb added
    /// to [`Verb::ALL`] with no specimen and no place among the reading
    /// four would be skipped by every carrier's agreement test in
    /// silence — the vocabulary would grow a word nothing checks.
    #[test]
    fn every_verb_is_either_read_or_has_an_order() {
        let reading = [Verb::List, Verb::Title, Verb::App, Verb::Icon];
        for verb in Verb::ALL {
            let has = Act::specimen(verb, WindowId(1)).is_some();
            assert_eq!(
                has,
                !reading.contains(&verb),
                "'{}' is neither a reading verb nor an order that can be \
                 built — no carrier's agreement test will ever look at it",
                verb.label()
            );
            if let Some(act) = Act::specimen(verb, WindowId(1)) {
                assert_eq!(act.verb(), verb, "the specimen of '{}' is a different verb", verb.label());
                assert_eq!(act.who(), WindowId(1), "the specimen lost its window");
            }
        }
    }

    /// **A native key that dies and comes back is a different window.**
    ///
    /// X11 hands window ids out again once a client is gone, and the
    /// Wayland object id is a slot in a table that is reused the moment
    /// it is freed. If the mint were the native key, an interface still
    /// holding the id of a window that closed would find itself
    /// addressing whatever took its place — closing, moving or
    /// fullscreening a stranger. That is why the native key never
    /// leaves this file.
    #[test]
    fn a_reused_native_key_is_never_the_same_window() {
        let mut names = Names::new();
        let first = names.of(0x0120_0007);
        assert_eq!(names.of(0x0120_0007), first, "the same live window changed identity");

        names.forget(0x0120_0007);
        let second = names.of(0x0120_0007);
        assert_ne!(
            first, second,
            "the window id the X server reused brought the old identity back \
             with it — an order meant for a window that closed would land on \
             whatever opened next"
        );
        assert_eq!(names.native(second), Some(0x0120_0007));
        assert_eq!(names.native(first), None, "a dead identity still resolves to a live window");
    }

    /// **A list arriving without a window is that window dying.**
    ///
    /// EWMH never says "closed"; `_NET_CLIENT_LIST` simply comes back
    /// shorter. [`Names::retain`] is the only place that turns absence
    /// into death, so a carrier that forgot to call it would keep
    /// handing out an identity for a window that is gone — and, worse,
    /// would hand the SAME one out again when the id is reused.
    #[test]
    fn a_window_missing_from_the_list_loses_its_identity() {
        let mut names = Names::new();
        let a = names.of(10);
        let b = names.of(20);
        names.retain(&[20]);
        assert_eq!(names.native(a), None, "a window that left the list kept its identity");
        assert_eq!(names.native(b), Some(20), "a window still in the list lost its identity");
        assert_ne!(names.of(10), a, "the identity came back with the id");
    }
}
