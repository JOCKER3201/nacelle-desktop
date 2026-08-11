//! Every program launched under gamescope takes the whole screen.
//!
//! This replaces a window manager's worth of machinery — adoption,
//! reparenting, frames rasterized on the CPU — that used to live here.
//! Framing other clients' windows inside a compositor that already has
//! its own manager meant fighting it: crashes on reparent races, black
//! overlay windows, chrome repainted on every exposure. Gamescope's
//! own model is one fullscreen client at a time, so the right move is
//! to lean into it: whatever maps, fills the screen, and gamescope
//! does the switching. The frame look stays in the toolkit
//! ([`winframe`]), waiting for the compositor of our own where it can
//! be drawn and animated properly.
//!
//! The mechanism is the polite one: an EWMH _NET_WM_STATE message
//! asking for FULLSCREEN, addressed to the root — exactly what the
//! window itself would send, and what gamescope's manager already
//! knows how to honour. Nothing is reparented, unmapped or redrawn, so
//! there is nothing left to race over.
//!
//! [`winframe`]: nacelle::object::winframe

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, EventMask,
    WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

x11rb::atom_manager! {
    Atoms:
    AtomsCookie {
        _NET_WM_STATE,
        _NET_WM_STATE_FULLSCREEN,
        GAMESCOPE_NO_FOCUS,
        GAMESCOPE_EXTERNAL_OVERLAY,
        STEAM_OVERLAY,
        STEAM_NOTIFICATION,
        STEAM_BIGPICTURE,
        STEAM_GAME,
    }
}

/// From the EWMH _NET_WM_STATE message: add the state.
const STATE_ADD: u32 = 1;

pub struct Fullscreen {
    conn: RustConnection,
    root: u32,
    host: u32,
    atoms: Atoms,
}

impl Fullscreen {
    /// Connects to the display nacelle's own window lives on and
    /// starts watching for windows to enlarge. None where there is no
    /// X11 window or display.
    pub fn start(window: &winit::window::Window) -> Option<Fullscreen> {
        let host = match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            _ => return None,
        };
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;
        let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
        // Watching, never redirecting: gamescope's manager keeps its
        // job, this only learns what mapped.
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
        )
        .ok()?;
        conn.flush().ok()?;
        let fs = Fullscreen { conn, root, host, atoms };
        // Clients already up before nacelle finished starting.
        if let Ok(Ok(tree)) = fs.conn.query_tree(root).map(|c| c.reply()) {
            for w in tree.children {
                if let Ok(Ok(attrs)) = fs.conn.get_window_attributes(w).map(|c| c.reply())
                {
                    if attrs.map_state == x11rb::protocol::xproto::MapState::VIEWABLE
                        && !attrs.override_redirect
                        && attrs.class != WindowClass::INPUT_ONLY
                    {
                        fs.enlarge(w);
                    }
                }
            }
        }
        Some(fs)
    }

    /// Drains the display's news. Called once a frame.
    pub fn poll(&mut self) {
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            if let Event::MapNotify(e) = ev {
                if e.event == self.root && !e.override_redirect {
                    self.enlarge(e.window);
                }
            }
        }
    }

    /// Asks for the window to be made fullscreen — unless it is
    /// nacelle itself, or a window speaking gamescope's private
    /// protocol (an overlay, a notification), which has already
    /// arranged its presentation with the compositor.
    fn enlarge(&self, w: u32) {
        if w == self.host || self.overlayish(w) {
            return;
        }
        let msg = ClientMessageEvent::new(
            32,
            w,
            self.atoms._NET_WM_STATE,
            [STATE_ADD, self.atoms._NET_WM_STATE_FULLSCREEN, 0, 1, 0],
        );
        let _ = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            msg,
        );
        let _ = self.conn.flush();
    }

    fn overlayish(&self, w: u32) -> bool {
        let Ok(Ok(props)) = self.conn.list_properties(w).map(|c| c.reply()) else {
            return false;
        };
        let spoken_for = [
            self.atoms.GAMESCOPE_NO_FOCUS,
            self.atoms.GAMESCOPE_EXTERNAL_OVERLAY,
            self.atoms.STEAM_OVERLAY,
            self.atoms.STEAM_NOTIFICATION,
            self.atoms.STEAM_BIGPICTURE,
            self.atoms.STEAM_GAME,
        ];
        props.atoms.iter().any(|a| spoken_for.contains(a))
    }
}
