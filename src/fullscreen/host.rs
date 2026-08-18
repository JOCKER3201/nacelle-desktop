//! Reading nacelle's own window out of winit.
//!
//! The one file in this module that knows what a window library is.
//! [`Host`] itself is four words of plain data, so the vocabulary can
//! move to libnacelle whole (see the module header); this is what stays
//! behind in the desktop, because "which window is ours" is a question
//! only the program that opened it can answer.
//!
//! Both halves are read from the same window, and both are allowed to
//! come back empty: a Wayland session has no X11 window, an X11 session
//! has no Wayland display, and `WINIT_UNIX_BACKEND=x11` on a Wayland
//! desktop gives the second without the first.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

use super::Host;

impl Host {
    /// nacelle's own window, as the carriers need to hear about it.
    pub fn of(window: &winit::window::Window) -> Host {
        let wayland_display = match window.display_handle().map(|h| h.as_raw()) {
            Ok(RawDisplayHandle::Wayland(d)) => Some(d.display.as_ptr()),
            _ => None,
        };
        // Xlib and Xcb are the same window seen through two libraries;
        // winit answers with whichever one it is built on, and a
        // carrier that only knew one of them would silently fail to
        // recognise the desktop on the other.
        let x11_window = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Xlib(h)) => Some(h.window as u32),
            Ok(RawWindowHandle::Xcb(h)) => Some(h.window.get()),
            _ => None,
        };
        Host { wayland_display, x11_window }
    }

    /// A host that knows nothing — for a caller with no window of its
    /// own on the display it is watching, which today is only the hand
    /// probes in [`super::x11`].
    ///
    /// Spelled out rather than derived, so that reaching for it is a
    /// sentence somebody wrote: every OTHER caller has a window, and
    /// passing this one instead puts the desktop in its own list.
    pub fn nobody() -> Host {
        Host { wayland_display: None, x11_window: None }
    }
}
