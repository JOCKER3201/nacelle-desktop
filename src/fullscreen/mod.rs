//! The window-management carriers: WHO actually speaks to a compositor,
//! for the vocabulary (WHAT can be asked) defined in
//! [`nacelle::wm`](nacelle::wm).
//!
//! # Why this module is called `fullscreen`
//!
//! The carrier-selection function is [`connect`]; the module name is a
//! leftover from when this file also held the vocabulary it now hands
//! off to libnacelle, and the rename (`mod fullscreen;` → `mod wm;`
//! plus `git mv src/fullscreen src/wm`) is left for a commit of its
//! own rather than folded into this one. Nothing else in the tree
//! depends on the name: [`Fullscreen`] is re-exported below with the
//! signature `main.rs` already calls.
//!
//! # The seam
//!
//! [`nacelle::wm`] owns the vocabulary: [`nacelle::wm::Verb`] for what
//! can be asked, [`nacelle::wm::Act`] for asking,
//! [`nacelle::wm::Window`] for what came back, and the
//! [`nacelle::wm::Backend`] trait for the thing that actually speaks to
//! a compositor. Two carriers implement it here:
//!
//!   * [`wayland`] — `ext-foreign-toplevel-list-v1`, reading only. The
//!     protocol a compositor of our own will speak.
//!   * [`x11`] — EWMH over XWayland, reading AND the whole control
//!     vocabulary. This is where the gamescope "make everything
//!     fullscreen" policy went: it did not become rubbish, it became a
//!     mode of a backend ([`x11::Policy`]).
//!
//! Choosing between them — which to try first, and what to fall back to
//! — is [`connect`], because that choice is made of these two concrete
//! carrier types and cannot live in a crate that does not know them.
//! [`nacelle::wm::Connector::over`] is the generic half, and it lives
//! in libnacelle with the vocabulary. A third seat is left for the
//! compositor of our own, which will need no protocol at all — it will
//! hold the window list itself and implement
//! [`nacelle::wm::Backend`] against its own state.
//!
//! # Why a snapshot and an epoch, not callbacks
//!
//! The desktop draws every frame from state it owns. A backend that
//! called back into the interface would need the interface to be
//! reachable from a Wayland dispatch, and the ordering between "a
//! window appeared" and "the frame is being laid out" would be nobody's
//! to state. So: [`nacelle::wm::Connector::poll`] once a frame drains
//! the carrier, [`nacelle::wm::Connector::windows`] hands back a
//! snapshot, and [`nacelle::wm::Connector::epoch`] only moves when
//! something actually changed. See that module for why an epoch that
//! ticks on every quiet frame is not a harmless epoch.
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
//!
//! # Where the vocabulary lives now
//!
//! `CLAUDE.md`: "JEDEN MODEL OKNA … Zachowania okien mieszkają w
//! libnacelle i działają globalnie — nie w nacelle-desktop". What a
//! window can be asked for — [`nacelle::wm::Verb`],
//! [`nacelle::wm::Act`], [`nacelle::wm::Window`],
//! [`nacelle::wm::State`], [`nacelle::wm::Place`],
//! [`nacelle::wm::Icon`], [`nacelle::wm::Outcome`],
//! [`nacelle::wm::Names`], [`nacelle::wm::Backend`] — moved there on
//! 2026-08-23: it is one list whether the window is somebody else's or
//! `nacelle::object::winframe`'s own, and it named no type from this
//! crate to begin with. What is left here is exactly what a crate with
//! no `winit`, `x11rb` or `wayland-client` dependency could never hold:
//! the two carriers, [`Host`] (which compositor concept is "our
//! window"), and [`connect`] (which carrier to try first). "Which
//! compositor is this" and "which window is ours" stay the desktop's
//! questions, not the toolkit's.
//!
//! [`connect`] is wired into `main.rs`'s redraw path next to
//! [`Fullscreen`]: a [`nacelle::wm::Connector`] is opened once at
//! startup and polled once a frame, the same discipline
//! `fullscreen.poll()` already ran under the old name. Nothing in the
//! interface reads [`nacelle::wm::Connector::windows`] yet — no board
//! shows a foreign-window list, no control sends
//! [`nacelle::wm::Act::Close`] — so the verbs beyond `poll` are
//! exercised by this crate's own tests
//! (`x11::tests::a_window_this_test_opens_comes_back_with_its_name`,
//! ignored by default, needs a display) and not yet by a person's
//! click. That is the piece of "JEDEN MODEL OKNA" this branch leaves
//! undone: the vocabulary is one and it is in the right place, but the
//! desktop does not yet let a person act on a foreign window through
//! it.

pub mod host;
pub mod wayland;
pub mod x11;

pub use x11::Fullscreen;

/// Which window is nacelle's own — told in the terms each carrier
/// speaks, because "our window" is not one number.
///
/// A carrier that is not told puts the desktop in the list of windows a
/// person can switch to, and under [`x11::Policy::Enlarge`] tells it to
/// go fullscreen as well. On a Wayland session there is no X11 window
/// and nothing to tell; on an X11 session, or with
/// `WINIT_UNIX_BACKEND=x11`, there is.
///
/// The Wayland carrier has no field here and cannot have one:
/// `ext-foreign-toplevel-list-v1` mints its identifiers in the
/// compositor and gives a client no way to ask which one is its own. So
/// on a compositor that advertises the list, our own toplevel is in it
/// — said out loud rather than papered over with a guess at our title
/// or our app id. The compositor of our own, the third seat, holds the
/// list itself and needs nobody to tell it which window is the desktop.
///
/// [`Host::of`] reads both out of a winit window; it is the one thing
/// in this module that knows what a window library is, and it lives in
/// [`host`] rather than in libnacelle with the rest, because "our
/// window" is a fact about which carrier this crate opened — not
/// something the vocabulary needs to name a verb.
#[derive(Clone, Copy, Debug)]
pub struct Host {
    /// winit's own Wayland display pointer — the same one `wl_color` is
    /// handed. None on an X11 session.
    pub wayland_display: Option<*mut std::ffi::c_void>,
    /// nacelle's own X11 window, under XWayland or on an X11 session.
    /// None where the window is not an X11 one.
    pub x11_window: Option<u32>,
}

/// Picks a carrier for [`nacelle::wm::Connector`]. Wayland first, and
/// only if the compositor actually advertises the list — on a
/// compositor that does not (KWin 6.7.4, measured) falling through to
/// EWMH is the difference between seeing the X11 clients and seeing
/// nothing.
///
/// The call site is `main.rs`: `connect(Host::of(&screens[0].window))`
/// — the window, not the display pointer `main.rs` already has to
/// hand. Passing [`Host::nobody`] there compiles and puts the desktop
/// in its own window list.
///
/// [`Host`] is nacelle's own window, and it is an argument rather than
/// something a carrier could work out: the EWMH carrier reads
/// `_NET_CLIENT_LIST`, our own window is in it on an X11 session like
/// anybody else's, and a desktop that lists itself is a row that
/// switches to the thing you are already looking at.
pub fn connect(host: Host) -> Option<nacelle::wm::Connector> {
    if let Some(d) = host.wayland_display {
        if let Some(b) = wayland::Toplevels::start(d) {
            return Some(nacelle::wm::Connector::over(Box::new(b)));
        }
    }
    x11::Ewmh::start(x11::Policy::Observe, host.x11_window)
        .map(|b| nacelle::wm::Connector::over(Box::new(b)))
}

