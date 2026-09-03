//! The desktop's clipboard backends, behind the libnacelle seam.
//!
//! The toolkit calls `nacelle::clipboard::{store, load}` and never knows
//! which of these answered — that is the seam's whole point (F1 §2.3).
//! This file only PICKS and CONSTRUCTS: whichever display winit actually
//! connected to decides the backend, read from the window's own
//! `RawDisplayHandle` and never guessed from environment variables
//! (under gamescope this follows whichever socket winit picked).
//!
//! Anything else — no handle, a failed X connection, a compositor with
//! no data device — leaves libnacelle's `LocalClipboard` installed, and
//! copy/paste keeps working within the program.

use nacelle::clipboard::{Board, ClipboardBackend, Seat};
use raw_window_handle::RawDisplayHandle;

/// Picks and installs the backend for the display winit connected to.
/// Called once at startup, right after the window exists.
pub fn install(display: Option<RawDisplayHandle>) {
    match display {
        Some(RawDisplayHandle::Wayland(d)) => {
            // SAFETY: smithay-clipboard must be handed WINIT'S OWN
            // `wl_display` (never a second connection of the kind
            // `fullscreen/wayland.rs` opens for the window-management
            // protocols — on one of those its serial tracking sees no
            // keyboard and every store is refused). The pointer must
            // outlive the clipboard: it does, because the window — and
            // with it the display connection — lives for the whole
            // process.
            let cb = unsafe { smithay_clipboard::Clipboard::new(d.display.as_ptr()) };
            nacelle::clipboard::install(Box::new(WaylandClipboard(cb)));
        }
        Some(RawDisplayHandle::Xlib(_)) | Some(RawDisplayHandle::Xcb(_)) => {
            // X11 or XWayland. x11-clipboard opens its own connection —
            // correct here, because the X selection protocol needs no
            // input serials from winit's.
            match x11_clipboard::Clipboard::new() {
                Ok(cb) => nacelle::clipboard::install(Box::new(X11Clipboard(cb))),
                Err(e) => eprintln!(
                    "nacelle-desktop: no X11 clipboard ({e}) — copy/paste stays in-app"
                ),
            }
        }
        _ => {
            // No handle at all: the Local fallback is already there.
        }
    }
}

/// Wayland, on winit's own connection. smithay-clipboard runs its own
/// event queue thread and tracks keyboard-enter serials itself; a store
/// the compositor refuses (or a missing `zwp_primary_selection_v1` —
/// gamescope often has none) fails SILENTLY, which is the documented
/// contract: primary is a nicety, never an error dialog.
///
/// # What that worker costs, and why nothing here fixes it
///
/// The strace audit of 2026-08-18 measured the worker waking 281 times a
/// second across a 88-second run in which the clipboard was never used —
/// 196 048 syscalls, second only to the event loop and the telemetry
/// sweep. The cause was traced through the dependency chain and it is not
/// in this file; the whole of it is in code we do not own:
///
/// * **The wake-ups.** All 41 139 of the worker's `recvmsg` calls are on
///   fd 3 — winit's `wayland-0` socket, the same one the event loop reads.
///   That sharing is deliberate (see the SAFETY note above: it is the only
///   way the store gets an input serial), and it means every batch the
///   compositor sends — two screens' frame callbacks, sixty times a second
///   — makes the socket readable for BOTH pollers. The worker wakes, reads
///   its share of the stream, finds nothing addressed to its own queue and
///   sleeps again. Nothing short of a second connection changes that, and
///   a second connection is the thing that breaks the serial.
/// * **The idle re-arm.** 24 975 `timerfd_settime` calls, every one of
///   them disarming an already disarmed timer (`it_value = {0,0}`), plus
///   two `epoll_ctl` and one `read` per turn. That is `polling` 3.11.0,
///   `src/epoll.rs:180-203`: it sets the timerfd and re-registers interest
///   unconditionally on every `wait`, INCLUDING the `None` deadline this
///   worker always passes. calloop 0.14 sits on `polling`, sctk 0.20 sits
///   on calloop, smithay-clipboard 0.7 sits on sctk. Roughly 100 000 of
///   the worker's syscalls — half of its whole footprint — are this
///   bookkeeping, and every one of them is behind three crates of ours.
///
/// So the honest answer is that the idle work cannot be taken off without
/// either patching `polling` or changing how the clipboard is dispatched.
/// The real fix, when it is worth doing, is the second one: dispatch the
/// clipboard's queue from the event loop the program already runs instead
/// of from a thread of its own, which needs an API smithay-clipboard 0.7
/// does not offer. Making the worker LAZY — built on the first copy rather
/// than at startup — would zero the cost for a session that never copies,
/// but a queue born after the fact has no keyboard-enter serial yet, so
/// the first store could be refused. Neither is a change to make quietly.
struct WaylandClipboard(smithay_clipboard::Clipboard);

impl ClipboardBackend for WaylandClipboard {
    fn store(&mut self, _seat: Seat, board: Board, text: &str) {
        match board {
            Board::Clipboard => self.0.store(text),
            Board::Primary => self.0.store_primary(text),
        }
    }

    fn load(&mut self, _seat: Seat, board: Board) -> Option<String> {
        match board {
            Board::Clipboard => self.0.load().ok(),
            Board::Primary => self.0.load_primary().ok(),
        }
    }
}

/// X11/XWayland: CLIPBOARD and PRIMARY over x11rb, INCR handled by the
/// crate. Loads carry a timeout because an X load waits on the owner,
/// and a stuck owner must cost one paste, not hang the desktop — the
/// seam only ever loads on an explicit paste gesture.
struct X11Clipboard(x11_clipboard::Clipboard);

impl ClipboardBackend for X11Clipboard {
    fn store(&mut self, _seat: Seat, board: Board, text: &str) {
        let atoms = &self.0.setter.atoms;
        let sel = match board {
            Board::Clipboard => atoms.clipboard,
            Board::Primary => atoms.primary,
        };
        // A refused store is a no-op by contract.
        let _ = self.0.store(sel, atoms.utf8_string, text.as_bytes());
    }

    fn load(&mut self, _seat: Seat, board: Board) -> Option<String> {
        let atoms = &self.0.getter.atoms;
        let sel = match board {
            Board::Clipboard => atoms.clipboard,
            Board::Primary => atoms.primary,
        };
        self.0
            .load(
                sel,
                atoms.utf8_string,
                atoms.property,
                std::time::Duration::from_millis(250),
            )
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }
}
