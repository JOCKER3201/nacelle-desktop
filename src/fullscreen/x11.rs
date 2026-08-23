//! The X11 carrier: EWMH over XWayland, and the gamescope policy that
//! used to be this whole file.
//!
//! # What became of the fullscreen machinery
//!
//! Every program launched under gamescope takes the whole screen. That
//! replaced a window manager's worth of machinery — adoption,
//! reparenting, frames rasterized on the CPU — which used to live here.
//! Framing other clients' windows inside a compositor that already has
//! its own manager meant fighting it: crashes on reparent races, black
//! overlay windows, chrome repainted on every exposure. Gamescope's own
//! model is one fullscreen client at a time, so the right move was to
//! lean into it.
//!
//! That policy did not go in the bin when the connector was built. It
//! is [`Policy::Enlarge`], one of two modes of this carrier — the other
//! being [`Policy::Observe`], which watches and reports and enlarges
//! nothing. Both ride the same connection, the same atoms and the same
//! event drain, because "make everything fullscreen" and "tell me what
//! is open" are two readings of one X11 conversation, not two programs.
//!
//! The mechanism is still the polite one: an EWMH `_NET_WM_STATE`
//! message asking for FULLSCREEN, addressed to the root — exactly what
//! the window itself would send, and what gamescope's manager already
//! knows how to honour. Nothing is reparented, unmapped or redrawn, so
//! there is nothing left to race over. The frame look stays in the
//! toolkit ([`winframe`]), waiting for the compositor of our own.
//!
//! # Why EWMH is the carrier that can actually DO things
//!
//! Measured on the owner's machine, 2026-08-18, KWin 6.7.4:
//! `_NET_SUPPORTED` on the XWayland root lists `_NET_CLIENT_LIST`,
//! `_NET_ACTIVE_WINDOW`, `_NET_CLOSE_WINDOW`, `_NET_MOVERESIZE_WINDOW`,
//! `_NET_WM_DESKTOP`, `_NET_WM_STATE_HIDDEN`, `_NET_WM_ICON` and the
//! whole `_NET_WM_ALLOWED_ACTIONS` family. KWin is a complete EWMH
//! window manager for its X11 side, so every verb in the vocabulary has
//! a real request behind it here — which is more than the neutral
//! Wayland protocol gives, and more than the Plasma one gives an
//! unprivileged client.
//!
//! The catch, and it is the whole shape of the alpha: **this sees X11
//! clients only.** A Wayland-native window is not in `_NET_CLIENT_LIST`
//! and never will be. That is what [`Backend::blind_spot`] says out
//! loud, so an empty list is not mistaken for an empty desktop.
//!
//! [`winframe`]: nacelle::object::winframe

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, EventMask,
    MapState, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use super::Host;
use nacelle::wm::{
    reads_differently, Act, Backend, Icon, Names, Outcome, Place, State, Verb, Window, WindowId,
};

x11rb::atom_manager! {
    Atoms:
    AtomsCookie {
        _NET_SUPPORTED,
        _NET_CLIENT_LIST,
        _NET_ACTIVE_WINDOW,
        _NET_CLOSE_WINDOW,
        _NET_MOVERESIZE_WINDOW,
        _NET_FRAME_EXTENTS,
        _NET_CURRENT_DESKTOP,
        _NET_WM_DESKTOP,
        _NET_WM_NAME,
        _NET_WM_ICON,
        _NET_WM_STATE,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_FOCUSED,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_TOOLBAR,
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_SPLASH,
        WM_CHANGE_STATE,
        GAMESCOPE_NO_FOCUS,
        GAMESCOPE_EXTERNAL_OVERLAY,
        STEAM_OVERLAY,
        STEAM_NOTIFICATION,
        STEAM_BIGPICTURE,
        STEAM_GAME,
    }
}

/// From the EWMH `_NET_WM_STATE` message.
const STATE_REMOVE: u32 = 0;
const STATE_ADD: u32 = 1;

/// EWMH's "who is asking". 2 is a pager or taskbar — which is what this
/// program is when it moves somebody else's window.
const SOURCE_PAGER: u32 = 2;

/// ICCCM `WM_CHANGE_STATE`: the only way to ask for a window to be
/// minimized. `_NET_WM_STATE_HIDDEN` is the manager's to set and not
/// the client's to request, which is a trap worth writing down.
const ICONIC_STATE: u32 = 3;

/// `_NET_MOVERESIZE_WINDOW`: all four of x, y, width, height are being
/// given (bits 8..11), the gravity stays the window's own (0), and the
/// source is a pager (bits 12..13).
const MOVERESIZE_ALL: u32 = (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (SOURCE_PAGER << 12);

/// What this carrier does with the windows it sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Watch and report. Touch nothing that was not asked for.
    Observe,
    /// Whatever maps, fills the screen. Gamescope's own model.
    Enlarge,
}

/// One X11 request, described before it is sent.
///
/// Pulling the routing out of the sending is what makes this carrier
/// testable without a display: the shape of every message — which atom,
/// which window, which five words — can be checked on a bench. Getting
/// one of those words wrong produces no error anywhere; the window
/// manager simply ignores a malformed request, and the control does
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// No request exists for this verb in EWMH.
    ///
    /// [`Ewmh::act`] already answers `Outcome::Unsupported` for this
    /// arm, but nothing constructs it today —
    /// `every_verb_ewmh_offers_has_a_request_behind_it` below proves
    /// [`route`] is total over the whole vocabulary as it stands. The
    /// arm stays for the next verb that is not one EWMH can do, so
    /// that day's diff is one match arm shorter and not a new variant.
    #[allow(dead_code)]
    Nothing,
    /// A client message addressed to `window`, delivered through the
    /// root — which is how EWMH says everything.
    Root { atom: Atom, window: u32, data: [u32; 5] },
}

/// How thick the manager's frame is around a window: left, right, top,
/// bottom, in the order `_NET_FRAME_EXTENTS` gives them.
///
/// Zero for a window with no frame, which is every window under a
/// compositor that draws no decorations — and every Wayland toplevel,
/// which is why [`Place`] means the client rectangle and this
/// conversion lives here rather than in the vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Frame {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl Frame {
    fn of(prop: &[u32]) -> Frame {
        if prop.len() < 4 {
            return Frame::default();
        }
        Frame {
            left: prop[0] as i32,
            right: prop[1] as i32,
            top: prop[2] as i32,
            bottom: prop[3] as i32,
        }
    }
}

/// Which EWMH request carries which order.
///
/// Pure, so the message can be read in a test. `window` is the native
/// X11 id, already resolved from the [`WindowId`] the interface holds;
/// `frame` is what the manager has drawn around it.
fn route(act: Act, window: u32, atoms: &Atoms, frame: Frame) -> Step {
    let root = |atom: Atom, data: [u32; 5]| Step::Root { atom, window, data };
    match act {
        // data[2] is "the window that was active when this was asked",
        // which the manager uses to decide whether this is a legitimate
        // focus change. Zero says "none", which is the honest answer
        // from a taskbar that is not itself the focused window.
        Act::Focus(_) => root(atoms._NET_ACTIVE_WINDOW, [SOURCE_PAGER, x11rb::CURRENT_TIME, 0, 0, 0]),
        Act::Close(_) => root(atoms._NET_CLOSE_WINDOW, [x11rb::CURRENT_TIME, SOURCE_PAGER, 0, 0, 0]),
        Act::Minimize(_, true) => root(atoms.WM_CHANGE_STATE, [ICONIC_STATE, 0, 0, 0, 0]),
        // Un-minimizing is not a WM_CHANGE_STATE with another number;
        // ICCCM has no such request. Asking for the window to be
        // activated is what a taskbar does and what every manager
        // reads as "restore".
        Act::Minimize(_, false) => {
            root(atoms._NET_ACTIVE_WINDOW, [SOURCE_PAGER, x11rb::CURRENT_TIME, 0, 0, 0])
        }
        Act::Maximize(_, on) => root(
            atoms._NET_WM_STATE,
            [
                if on { STATE_ADD } else { STATE_REMOVE },
                atoms._NET_WM_STATE_MAXIMIZED_VERT,
                atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                SOURCE_PAGER,
                0,
            ],
        ),
        Act::Fullscreen(_, on) => root(
            atoms._NET_WM_STATE,
            [
                if on { STATE_ADD } else { STATE_REMOVE },
                atoms._NET_WM_STATE_FULLSCREEN,
                0,
                SOURCE_PAGER,
                0,
            ],
        ),
        // `_NET_MOVERESIZE_WINDOW` mixes two coordinate systems in one
        // message, and nothing warns you: the width and height are the
        // CLIENT's, but x and y place the FRAME's corner. Handing a
        // rectangle straight back to the manager therefore walks the
        // window down and right by the thickness of its own decoration,
        // every single time. Measured against KWin 6.7.4 by the probe
        // below: asked for (300, 200), the client landed at (304, 228).
        //
        // [`Place`] means the client rectangle on both sides of the
        // seam — it has to, because a Wayland toplevel has no frame to
        // mean anything else by — so the frame is taken off here.
        Act::Place(_, p) => root(
            atoms._NET_MOVERESIZE_WINDOW,
            [
                MOVERESIZE_ALL,
                (p.x - frame.left) as u32,
                (p.y - frame.top) as u32,
                p.w,
                p.h,
            ],
        ),
        Act::SendToBoard(_, board) => {
            root(atoms._NET_WM_DESKTOP, [board, SOURCE_PAGER, 0, 0, 0])
        }
    }
}

/// Whether a window belongs in a list of windows a person can switch
/// between.
///
/// Three reasons to say no, and none of them is tidiness.
///
/// The first is that nacelle's own window is in `_NET_CLIENT_LIST` like
/// any other client's — on an X11 session, or under
/// `WINIT_UNIX_BACKEND=x11` — so a desktop that is not told which
/// window is its own offers a row that switches to the thing you are
/// already looking at. `host` is [`Host::x11_window`], and None means
/// "no window of ours on this display", not "list everything".
///
/// The others were measured. On the machine this branch was written on,
/// `_NET_CLIENT_LIST` held exactly one window — `xwaylandvideobridge`,
/// carrying `_NET_WM_STATE_SKIP_TASKBAR` — so without the filter the
/// whole delivered feature would show one entry, and it would be an
/// entry nobody asked for and nobody can use.
fn worth_listing(
    host: Option<u32>,
    w: u32,
    types: &[Atom],
    states: &[Atom],
    atoms: &Atoms,
) -> bool {
    if Some(w) == host {
        return false;
    }
    if states.contains(&atoms._NET_WM_STATE_SKIP_TASKBAR) {
        return false;
    }
    let furniture = [
        atoms._NET_WM_WINDOW_TYPE_DESKTOP,
        atoms._NET_WM_WINDOW_TYPE_DOCK,
        atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
        atoms._NET_WM_WINDOW_TYPE_MENU,
        atoms._NET_WM_WINDOW_TYPE_SPLASH,
    ];
    // An absent `_NET_WM_WINDOW_TYPE` means NORMAL, which is a window
    // like any other — so the empty list is a yes, not a no.
    !types.iter().any(|t| furniture.contains(t))
}

/// Picks one icon out of `_NET_WM_ICON`.
///
/// The property is a run of `[width, height, width * height pixels]`
/// blocks, one per size the application shipped, in no particular
/// order. Nothing in X guarantees the property is well formed — it is
/// written by the client, and a client can write anything — so a block
/// that runs off the end of the buffer, or claims a size that would
/// overflow, has to end the walk rather than take the desktop down with
/// it.
fn best_icon(prop: &[u32], want: u32) -> Option<Icon> {
    let mut best: Option<(u32, u32, usize)> = None;
    let mut i = 0usize;
    while i + 2 <= prop.len() {
        let (w, h) = (prop[i], prop[i + 1]);
        // A zero side would make the walk stand still; an enormous one
        // is a client lying about a buffer it did not write.
        if w == 0 || h == 0 || w > 1 << 14 || h > 1 << 14 {
            break;
        }
        let Some(n) = (w as usize).checked_mul(h as usize) else { break };
        let Some(end) = i.checked_add(2).and_then(|s| s.checked_add(n)) else { break };
        if end > prop.len() {
            break;
        }
        let better = match best {
            None => true,
            // The smallest that is still big enough; failing that, the
            // biggest there is. Scaling down loses less than scaling up
            // invents.
            Some((bw, _, _)) => {
                if bw >= want {
                    w >= want && w < bw
                } else {
                    w > bw
                }
            }
        };
        if better {
            best = Some((w, h, i + 2));
        }
        i = end;
    }
    let (w, h, at) = best?;
    Some(Icon::Pixels { w, h, argb: prop[at..at + (w as usize) * (h as usize)].to_vec() })
}

/// `WM_CLASS` is two NUL-terminated strings: the instance name, then
/// the class. The class is the one that names the application, and it
/// is the nearest thing X11 has to a Wayland app id.
fn wm_class(raw: &[u8]) -> String {
    let mut parts = raw.split(|&b| b == 0).filter(|p| !p.is_empty());
    let instance = parts.next();
    let class = parts.next().or(instance);
    class.map(|c| String::from_utf8_lossy(c).into_owned()).unwrap_or_default()
}

/// The carrier.
pub struct Ewmh {
    conn: RustConnection,
    root: u32,
    /// nacelle's own window, which must never be enlarged or listed.
    host: Option<u32>,
    atoms: Atoms,
    policy: Policy,
    names: Names,
    snapshot: Vec<Window>,
    /// Something happened that only reading the list again can answer.
    relist: bool,
    /// Something moved. Answered by two requests, not by reading every
    /// property of every window again — see [`Ewmh::restake`].
    moved: bool,
}

/// What to select for on the root window, by policy.
///
/// Watching, never redirecting: whoever manages these windows keeps the
/// job. `SUBSTRUCTURE_NOTIFY` brings maps and unmaps, and it is
/// everything [`Policy::Enlarge`] has ever needed. `PROPERTY_CHANGE` is
/// the reading half — `_NET_CLIENT_LIST` and `_NET_ACTIVE_WINDOW`
/// moving — and it is asked for only where somebody is reading.
///
/// The split is not decoration. A window manager rewrites both of those
/// properties on every window change and every change of focus, so
/// under gamescope, where nobody is building a window list at all,
/// asking for them means every one of those events is put on the wire,
/// woken up for, drained and thrown away. This path is meant to cost
/// what it cost before the connector existed, and that is only true if
/// it asks for exactly what it asked for then.
fn root_mask(policy: Policy) -> EventMask {
    match policy {
        Policy::Enlarge => EventMask::SUBSTRUCTURE_NOTIFY,
        Policy::Observe => EventMask::SUBSTRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE,
    }
}

/// Whether a property changing means the list has to be read again.
///
/// [`Ewmh::rebuild`] asks for `PROPERTY_CHANGE` on every window it
/// lists, which means this carrier hears everything every application
/// writes about itself — and `_NET_WM_USER_TIME` is written on every
/// keystroke and every click. Taking that for news costs seven round
/// trips per window, per keypress, to produce a list that comes back
/// identical; the epoch stays put, but the requests are paid for all
/// the same.
///
/// So the answer is the properties this carrier actually reads, and
/// nothing else. `_NET_WM_ICON` and `_NET_FRAME_EXTENTS` are read on
/// demand and never appear in the snapshot, so they are not on this
/// list either — an application redrawing its icon must not rebuild
/// anything.
fn worth_rereading(atom: Atom, atoms: &Atoms) -> bool {
    [
        atoms._NET_CLIENT_LIST,
        atoms._NET_ACTIVE_WINDOW,
        atoms._NET_WM_NAME,
        atoms._NET_WM_STATE,
        atoms._NET_WM_DESKTOP,
        atoms._NET_WM_WINDOW_TYPE,
        AtomEnum::WM_NAME.into(),
        AtomEnum::WM_CLASS.into(),
    ]
    .contains(&atom)
}

impl Ewmh {
    /// Every verb EWMH carries. One table, read by [`Backend::can`] and
    /// by the test that walks the vocabulary against [`route`].
    const KNOWS: &'static [Verb] = &[
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

    /// Connects to the display nacelle's own window lives on.
    ///
    /// None where there is no display. Under [`Policy::Observe`] also
    /// None where nobody has claimed to be an EWMH window manager — a
    /// carrier that can see nothing is worse than no carrier, because
    /// the connector would stop looking for a better one.
    /// [`Policy::Enlarge`] asks for no such proof: gamescope's manager
    /// honours the fullscreen message whatever it advertises, and that
    /// path worked before this module was a connector.
    ///
    /// `host` is nacelle's own window ([`Host::x11_window`]), which is
    /// neither listed nor enlarged. None means there is no window of
    /// ours on this display — which is true on a Wayland session and
    /// true of the hand probes below, and is not the same sentence as
    /// "do not bother".
    pub fn start(policy: Policy, host: Option<u32>) -> Option<Ewmh> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;
        let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
        if policy == Policy::Observe {
            let supported = read32(&conn, root, atoms._NET_SUPPORTED);
            if supported.is_empty() {
                return None;
            }
        }
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(root_mask(policy)),
        )
        .ok()?;
        conn.flush().ok()?;
        let mut me = Ewmh {
            conn,
            root,
            host,
            atoms,
            policy,
            names: Names::new(),
            snapshot: Vec::new(),
            relist: false,
            moved: false,
        };
        if policy == Policy::Observe {
            me.rebuild();
        } else {
            // Clients already up before nacelle finished starting.
            if let Ok(Ok(tree)) = me.conn.query_tree(root).map(|c| c.reply()) {
                for w in tree.children {
                    if let Ok(Ok(attrs)) = me.conn.get_window_attributes(w).map(|c| c.reply()) {
                        if attrs.map_state == MapState::VIEWABLE
                            && !attrs.override_redirect
                            && attrs.class != WindowClass::INPUT_ONLY
                        {
                            me.enlarge(w);
                        }
                    }
                }
            }
        }
        Some(me)
    }

    /// Asks for the window to be made fullscreen — unless it is nacelle
    /// itself, or a window speaking gamescope's private protocol (an
    /// overlay, a notification), which has already arranged its
    /// presentation with the compositor.
    fn enlarge(&self, w: u32) {
        if Some(w) == self.host || self.overlayish(w) {
            return;
        }
        self.tell(w, self.atoms._NET_WM_STATE, [
            STATE_ADD,
            self.atoms._NET_WM_STATE_FULLSCREEN,
            0,
            1,
            0,
        ]);
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

    /// One client message, addressed to a window, delivered through the
    /// root — the shape every EWMH request takes.
    fn tell(&self, window: u32, atom: Atom, data: [u32; 5]) {
        let msg = ClientMessageEvent::new(32, window, atom, data);
        let _ = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            msg,
        );
        let _ = self.conn.flush();
    }

    /// Takes in whatever the server has said, applies the policy, and
    /// notes whether anything might have moved.
    ///
    /// Separate from the reading half on purpose. Under
    /// [`Policy::Enlarge`] this is ALL that happens: gamescope's path
    /// costs one drained event queue a frame, exactly what it cost when
    /// this file did nothing else. Building a window list nobody reads
    /// would be seven round trips per window per frame paid for
    /// nothing.
    ///
    /// Which is also to say that the two flags below belong to the
    /// reading half. Under [`Policy::Enlarge`] nothing ever reads them,
    /// because [`Fullscreen::poll`] calls this and not
    /// [`Backend::poll`] — they are two booleans that stay set, and
    /// that is the whole of it.
    fn drain(&mut self) {
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            match ev {
                Event::MapNotify(e) if e.event == self.root && !e.override_redirect => {
                    if self.policy == Policy::Enlarge {
                        self.enlarge(e.window);
                    }
                    self.relist = true;
                }
                Event::UnmapNotify(_) | Event::DestroyNotify(_) => self.relist = true,
                // A window under the pointer sends one of these per
                // frame. Reading every property of every window sixty
                // times a second to find out that a rectangle moved is
                // what this second flag exists to avoid; the rectangle
                // is answered by two requests in [`Ewmh::restake`].
                Event::ConfigureNotify(_) => self.moved = true,
                // Not every property is one this carrier reads.
                // `_NET_WM_USER_TIME` arrives on every keystroke in
                // every listed application.
                Event::PropertyNotify(e) => {
                    if worth_rereading(e.atom, &self.atoms) {
                        self.relist = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// Reads where the listed windows are, and nothing else.
    ///
    /// Both batches of requests are put on the wire before either reply
    /// is waited for, so this is two round trips for the whole list
    /// rather than two per window — which is the difference that
    /// matters, because what triggers it is a drag, once a frame.
    ///
    /// A window that died between the list and this request answers
    /// nothing, and keeps the rectangle it had. It is about to leave
    /// the list anyway.
    fn restake(&mut self) {
        let natives: Vec<Option<u32>> =
            self.snapshot.iter().map(|w| self.names.native(w.id).map(|n| n as u32)).collect();
        let sizes: Vec<_> =
            natives.iter().map(|n| n.and_then(|w| self.conn.get_geometry(w).ok())).collect();
        let spots: Vec<_> = natives
            .iter()
            .map(|n| n.and_then(|w| self.conn.translate_coordinates(w, self.root, 0, 0).ok()))
            .collect();
        let _ = self.conn.flush();
        for ((win, size), spot) in self.snapshot.iter_mut().zip(sizes).zip(spots) {
            let (Some(size), Some(spot)) = (size, spot) else { continue };
            let (Ok(g), Ok(t)) = (size.reply(), spot.reply()) else { continue };
            win.place = Some(Place {
                x: t.dst_x as i32,
                y: t.dst_y as i32,
                w: g.width as u32,
                h: g.height as u32,
            });
        }
    }

    /// Reads the whole list afresh. EWMH has no "this one window
    /// changed" — `_NET_CLIENT_LIST` is a property, and a property
    /// changing means reading it again.
    fn rebuild(&mut self) {
        let listed = read32(&self.conn, self.root, self.atoms._NET_CLIENT_LIST);
        let active = read32(&self.conn, self.root, self.atoms._NET_ACTIVE_WINDOW)
            .first()
            .copied()
            .unwrap_or(0);
        let alive: Vec<u64> = listed.iter().map(|&w| w as u64).collect();
        self.names.retain(&alive);

        let mut out = Vec::with_capacity(listed.len());
        for w in listed {
            let types = read32(&self.conn, w, self.atoms._NET_WM_WINDOW_TYPE);
            let states = read32(&self.conn, w, self.atoms._NET_WM_STATE);
            if !worth_listing(self.host, w, &types, &states, &self.atoms) {
                continue;
            }
            // Told about this window's own property changes from now
            // on, so a rename is news without polling every window
            // every frame. Errors are the window having died between
            // the list and this request, which is nothing to report.
            let _ = self.conn.change_window_attributes(
                w,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            );

            let mut win = Window::new(self.names.of(w as u64));
            win.title = read_text(&self.conn, w, self.atoms._NET_WM_NAME);
            if win.title.is_empty() {
                win.title = read_text(&self.conn, w, AtomEnum::WM_NAME.into());
            }
            win.app = wm_class(&read8(&self.conn, w, AtomEnum::WM_CLASS.into()));
            win.board = read32(&self.conn, w, self.atoms._NET_WM_DESKTOP).first().copied();
            win.state = State {
                active: w == active || states.contains(&self.atoms._NET_WM_STATE_FOCUSED),
                minimized: states.contains(&self.atoms._NET_WM_STATE_HIDDEN),
                maximized: states.contains(&self.atoms._NET_WM_STATE_MAXIMIZED_VERT)
                    && states.contains(&self.atoms._NET_WM_STATE_MAXIMIZED_HORZ),
                fullscreen: states.contains(&self.atoms._NET_WM_STATE_FULLSCREEN),
            };
            if let Ok(Ok(g)) = self.conn.get_geometry(w).map(|c| c.reply()) {
                if let Ok(Ok(t)) =
                    self.conn.translate_coordinates(w, self.root, 0, 0).map(|c| c.reply())
                {
                    win.place = Some(Place {
                        x: t.dst_x as i32,
                        y: t.dst_y as i32,
                        w: g.width as u32,
                        h: g.height as u32,
                    });
                }
            }
            out.push(win);
        }
        let _ = self.conn.flush();
        self.snapshot = out;
    }
}

/// A 32-bit property, or nothing. Never an error the caller has to
/// think about: on X11 every window can die between two requests.
fn read32(conn: &RustConnection, window: u32, atom: Atom) -> Vec<u32> {
    let Ok(cookie) =
        conn.get_property(false, window, atom, AtomEnum::ANY, 0, u32::MAX / 4)
    else {
        return Vec::new();
    };
    let Ok(reply) = cookie.reply() else { return Vec::new() };
    reply.value32().map(|v| v.collect()).unwrap_or_default()
}

fn read8(conn: &RustConnection, window: u32, atom: Atom) -> Vec<u8> {
    let Ok(cookie) = conn.get_property(false, window, atom, AtomEnum::ANY, 0, 4096) else {
        return Vec::new();
    };
    let Ok(reply) = cookie.reply() else { return Vec::new() };
    reply.value
}

fn read_text(conn: &RustConnection, window: u32, atom: Atom) -> String {
    let raw = read8(conn, window, atom);
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

impl Backend for Ewmh {
    fn carrier(&self) -> &'static str {
        match self.policy {
            Policy::Observe => "x11 ewmh",
            Policy::Enlarge => "x11 ewmh (gamescope: everything fullscreen)",
        }
    }

    fn can(&self, verb: Verb) -> bool {
        Ewmh::KNOWS.contains(&verb)
    }

    fn blind_spot(&self) -> Option<&'static str> {
        Some(
            "X11 clients only — a Wayland-native window is not in \
             _NET_CLIENT_LIST and cannot be",
        )
    }

    fn poll(&mut self) -> bool {
        self.drain();
        if self.relist {
            self.relist = false;
            // The list was read whole, geometry with it.
            self.moved = false;
            let before = std::mem::take(&mut self.snapshot);
            self.rebuild();
            // News is the list coming out different, not the server
            // having said something — and a rectangle is not part of
            // "different" ([`reads_differently`]).
            return reads_differently(&self.snapshot, &before);
        }
        if self.moved {
            self.moved = false;
            // Where the windows are, kept current for whoever reads it
            // every frame. Never news: this is what a drag looks like,
            // and it looks like it sixty times a second.
            self.restake();
        }
        false
    }

    fn windows(&self) -> &[Window] {
        &self.snapshot
    }

    fn icon(&mut self, id: WindowId, want: u32) -> Option<Icon> {
        let native = self.names.native(id)? as u32;
        best_icon(&read32(&self.conn, native, self.atoms._NET_WM_ICON), want)
    }

    fn act(&mut self, act: Act) -> Outcome {
        if !self.can(act.verb()) {
            return Outcome::Unsupported;
        }
        let Some(native) = self.names.native(act.who()) else {
            return Outcome::Unknown(act.who());
        };
        let frame =
            Frame::of(&read32(&self.conn, native as u32, self.atoms._NET_FRAME_EXTENTS));
        match route(act, native as u32, &self.atoms, frame) {
            Step::Nothing => Outcome::Unsupported,
            Step::Root { atom, window, data } => {
                self.tell(window, atom, data);
                Outcome::Sent
            }
        }
    }
}

/// The gamescope arrangement, under the name and the signature
/// `main.rs` already calls.
///
/// A carrier in [`Policy::Enlarge`] and nothing more, kept as its own
/// type rather than folded into [`connect`](super::connect): that
/// function opens [`Policy::Observe`] and is meant to run alongside
/// this one, not replace it — under gamescope both the "enlarge
/// everything" policy and a real window list are wanted at once, on
/// two separate connections, each paying only for what it asks the
/// server for (see [`root_mask`]). Merging the two into one
/// policy-parametrised call would make them share a connection and a
/// poll, which is a real simplification but a separate one from the
/// vocabulary's move to libnacelle, so it is left for its own change.
pub struct Fullscreen(Ewmh);

impl Fullscreen {
    /// Connects to the display nacelle's own window lives on and starts
    /// watching for windows to enlarge. None where there is no X11
    /// window or display.
    pub fn start(window: &winit::window::Window) -> Option<Fullscreen> {
        // One reader for "which window is ours", shared with
        // [`connect`](super::connect) — the two must not be able to
        // disagree about which window must never be enlarged.
        let host = Host::of(window).x11_window?;
        Ewmh::start(Policy::Enlarge, Some(host)).map(Fullscreen)
    }

    /// Drains the display's news. Called once a frame.
    ///
    /// [`Ewmh::drain`] and not [`Backend::poll`]: the second one would
    /// also read every listed window's properties back, and under
    /// gamescope nobody is asking for a window list. This path costs
    /// what it cost before the connector existed.
    pub fn poll(&mut self) {
        self.0.drain();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Atoms with a number per name, so a message can be read back and
    /// the wrong atom is visible instead of being another 32-bit word.
    ///
    /// Every field gets a DIFFERENT number, which is the point: a
    /// routing that reached for `_NET_WM_STATE_MAXIMIZED_VERT` where it
    /// meant `_NET_WM_STATE_FULLSCREEN` would pass against a table of
    /// zeroes.
    fn bench() -> Atoms {
        Atoms {
            _NET_SUPPORTED: 101,
            _NET_CLIENT_LIST: 102,
            _NET_ACTIVE_WINDOW: 103,
            _NET_CLOSE_WINDOW: 104,
            _NET_MOVERESIZE_WINDOW: 105,
            _NET_FRAME_EXTENTS: 130,
            _NET_CURRENT_DESKTOP: 106,
            _NET_WM_DESKTOP: 107,
            _NET_WM_NAME: 108,
            _NET_WM_ICON: 109,
            _NET_WM_STATE: 110,
            _NET_WM_STATE_FULLSCREEN: 111,
            _NET_WM_STATE_MAXIMIZED_VERT: 112,
            _NET_WM_STATE_MAXIMIZED_HORZ: 113,
            _NET_WM_STATE_HIDDEN: 114,
            _NET_WM_STATE_FOCUSED: 115,
            _NET_WM_STATE_SKIP_TASKBAR: 116,
            _NET_WM_WINDOW_TYPE: 117,
            _NET_WM_WINDOW_TYPE_DESKTOP: 118,
            _NET_WM_WINDOW_TYPE_DOCK: 119,
            _NET_WM_WINDOW_TYPE_TOOLBAR: 120,
            _NET_WM_WINDOW_TYPE_MENU: 121,
            _NET_WM_WINDOW_TYPE_SPLASH: 122,
            WM_CHANGE_STATE: 123,
            GAMESCOPE_NO_FOCUS: 124,
            GAMESCOPE_EXTERNAL_OVERLAY: 125,
            STEAM_OVERLAY: 126,
            STEAM_NOTIFICATION: 127,
            STEAM_BIGPICTURE: 128,
            STEAM_GAME: 129,
        }
    }

    /// **Every verb this carrier offers has a request behind it.**
    ///
    /// [`Ewmh::KNOWS`] is what the interface asks before it draws a
    /// control live; [`route`] is what the click reaches. They are two
    /// lists in two places and only this holds them together. A verb
    /// added to the table with no arm in the routing would be a button
    /// drawn enabled over nothing at all — which is the failure the
    /// whole seam exists to prevent, and which produces no error
    /// message anywhere.
    #[test]
    fn every_verb_ewmh_offers_has_a_request_behind_it() {
        let atoms = bench();
        for verb in Verb::ALL {
            let Some(act) = Act::specimen(verb, WindowId(1)) else { continue };
            let offered = Ewmh::KNOWS.contains(&verb);
            let routed = route(act, 0x4200, &atoms, Frame::default()) != Step::Nothing;
            assert_eq!(
                offered, routed,
                "'{}' is offered={offered} but routed={routed} — one of the \
                 two lists moved without the other",
                verb.label()
            );
        }
    }

    /// **The messages say what EWMH says they must say.**
    ///
    /// A window manager ignores a malformed client message in silence:
    /// no error comes back, no log line is written, the window simply
    /// does not move. So the five words of each request are checked
    /// here, word by word, against the specification — because there is
    /// nowhere else they could ever be checked.
    #[test]
    fn each_request_carries_the_words_the_specification_asks_for() {
        let a = bench();
        let w = 0x0120_0007u32;
        let id = WindowId(1);

        assert_eq!(
            route(Act::Focus(id), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_ACTIVE_WINDOW,
                window: w,
                data: [SOURCE_PAGER, 0, 0, 0, 0]
            },
            "focus must be _NET_ACTIVE_WINDOW with a pager's source, or KWin \
             treats it as focus stealing and refuses"
        );

        assert_eq!(
            route(Act::Close(id), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_CLOSE_WINDOW,
                window: w,
                data: [0, SOURCE_PAGER, 0, 0, 0]
            },
            "close puts the timestamp first and the source second — the \
             opposite order of _NET_ACTIVE_WINDOW, which is exactly the \
             mistake that is invisible"
        );

        assert_eq!(
            route(Act::Minimize(id, true), w, &a, Frame::default()),
            Step::Root { atom: a.WM_CHANGE_STATE, window: w, data: [ICONIC_STATE, 0, 0, 0, 0] },
            "minimizing is ICCCM's WM_CHANGE_STATE; _NET_WM_STATE_HIDDEN is \
             the manager's to set and ignores anyone who asks for it"
        );
        assert_eq!(
            route(Act::Minimize(id, false), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_ACTIVE_WINDOW,
                window: w,
                data: [SOURCE_PAGER, 0, 0, 0, 0]
            },
            "restoring is asking for the window to be activated; there is no \
             un-iconify request in ICCCM"
        );

        assert_eq!(
            route(Act::Fullscreen(id, true), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_WM_STATE,
                window: w,
                data: [STATE_ADD, a._NET_WM_STATE_FULLSCREEN, 0, SOURCE_PAGER, 0]
            }
        );
        assert_eq!(
            route(Act::Fullscreen(id, false), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_WM_STATE,
                window: w,
                data: [STATE_REMOVE, a._NET_WM_STATE_FULLSCREEN, 0, SOURCE_PAGER, 0]
            },
            "turning a state off is the same message with a different first \
             word, not the absence of a message"
        );

        assert_eq!(
            route(Act::Maximize(id, true), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_WM_STATE,
                window: w,
                data: [
                    STATE_ADD,
                    a._NET_WM_STATE_MAXIMIZED_VERT,
                    a._NET_WM_STATE_MAXIMIZED_HORZ,
                    SOURCE_PAGER,
                    0
                ]
            },
            "maximizing is TWO states in one message; sending one of them \
             leaves a window maximized in one direction"
        );

        assert_eq!(
            route(Act::Place(id, Place { x: -20, y: 5, w: 800, h: 600 }), w, &a, Frame::default()),
            Step::Root {
                atom: a._NET_MOVERESIZE_WINDOW,
                window: w,
                data: [MOVERESIZE_ALL, (-20i32) as u32, 5, 800, 600]
            },
            "a negative coordinate must travel as its two's complement — a \
             window on a screen left of the origin has one"
        );
        assert_eq!(
            MOVERESIZE_ALL & 0xf00,
            0xf00,
            "_NET_MOVERESIZE_WINDOW ignores whichever of x, y, w, h is not \
             flagged present, in silence"
        );

        assert_eq!(
            route(Act::SendToBoard(id, 3), w, &a, Frame::default()),
            Step::Root { atom: a._NET_WM_DESKTOP, window: w, data: [3, SOURCE_PAGER, 0, 0, 0] }
        );
    }

    /// **A rectangle read off a window and handed straight back must
    /// leave the window where it was.**
    ///
    /// This is the defect the live probe below caught, and it is worth
    /// the words because nothing else would have. `_NET_MOVERESIZE_WINDOW`
    /// mixes two coordinate systems in one message: width and height are
    /// the client's, but x and y place the FRAME. The reader reports the
    /// client rectangle, because that is the only rectangle a Wayland
    /// toplevel has — so a `Place` taken from [`Backend::windows`] and
    /// passed to [`Backend::act`] unaltered used to walk the window down
    /// and right by the thickness of its own title bar, every time.
    /// Measured against KWin 6.7.4: asked for (300, 200), the client
    /// landed at (304, 228).
    ///
    /// "Drag a window and drop it back where it was" is exactly that
    /// round trip, and it would have crept.
    #[test]
    fn a_place_handed_back_unaltered_moves_nothing() {
        let a = bench();
        let dressed = Frame { left: 4, right: 4, top: 28, bottom: 4 };
        let where_it_is = Place { x: 300, y: 200, w: 240, h: 160 };

        let Step::Root { data, .. } = route(Act::Place(WindowId(1), where_it_is), 9, &a, dressed)
        else {
            panic!("a move produced no request")
        };
        // What the manager will put the client at: the frame corner it
        // was given, plus the frame it draws.
        assert_eq!(
            (data[1] as i32 + dressed.left, data[2] as i32 + dressed.top),
            (where_it_is.x, where_it_is.y),
            "the window came back somewhere other than where it was asked to \
             stay — the frame was not taken off the request"
        );
        assert_eq!((data[3], data[4]), (where_it_is.w, where_it_is.h), "the size is the client's");

        // An undecorated window — every Wayland toplevel, and every X11
        // window under a compositor that draws no chrome — must not be
        // shifted by a correction for a frame that is not there.
        let Step::Root { data, .. } =
            route(Act::Place(WindowId(1), where_it_is), 9, &a, Frame::default())
        else {
            panic!("a move produced no request")
        };
        assert_eq!(
            (data[1], data[2]),
            (where_it_is.x as u32, where_it_is.y as u32),
            "a window with no frame was moved to correct for one"
        );

        assert_eq!(Frame::of(&[]), Frame::default(), "an absent property invented a frame");
        assert_eq!(Frame::of(&[4, 4, 28]), Frame::default(), "a truncated property was believed");
        assert_eq!(
            Frame::of(&[1, 2, 3, 4]),
            Frame { left: 1, right: 2, top: 3, bottom: 4 },
            "_NET_FRAME_EXTENTS is left, right, top, bottom — in that order"
        );
    }

    /// **The one window on the owner's screen today must not be
    /// listed.**
    ///
    /// Measured 2026-08-18: `_NET_CLIENT_LIST` on the KWin 6.7.4
    /// XWayland root held exactly one window, `xwaylandvideobridge`,
    /// with `_NET_WM_STATE_SKIP_TASKBAR` among its states. Without this
    /// filter the entire delivered feature is one row nobody asked for.
    #[test]
    fn the_window_that_says_skip_taskbar_is_not_offered() {
        let a = bench();
        let some = 0x0120_0007;
        assert!(
            !worth_listing(None, some, &[], &[a._NET_WM_STATE_SKIP_TASKBAR], &a),
            "a window that asked not to be listed was listed"
        );
        assert!(
            !worth_listing(None, some, &[a._NET_WM_WINDOW_TYPE_DOCK], &[], &a),
            "a panel was offered as a window to switch to"
        );
        assert!(
            !worth_listing(None, some, &[a._NET_WM_WINDOW_TYPE_DESKTOP], &[], &a),
            "the desktop background was offered as a window"
        );
        assert!(
            worth_listing(None, some, &[], &[], &a),
            "a plain window with no type at all is NORMAL and must be listed"
        );
        assert!(
            worth_listing(None, some, &[], &[a._NET_WM_STATE_MAXIMIZED_VERT], &a),
            "a maximized window stopped being a window"
        );
    }

    /// **nacelle's own window is never offered as a window to switch
    /// to, and never enlarged.**
    ///
    /// On an X11 session — or under `WINIT_UNIX_BACKEND=x11`, which is
    /// one environment variable away on any machine — the desktop's own
    /// window is in `_NET_CLIENT_LIST` exactly like every other
    /// client's. Nothing in the properties says "this one is you": the
    /// only way to know is to be told, which is why [`Host`] is an
    /// argument of [`connect`](super::connect) and not something worked
    /// out here.
    ///
    /// The second half matters as much as the first. Under
    /// [`Policy::Enlarge`] the same number is what stops the desktop
    /// being sent a fullscreen message about itself.
    #[test]
    fn the_desktop_is_never_a_window_in_its_own_list() {
        let a = bench();
        let ours = 0x0120_0007;
        let theirs = 0x0120_0008;
        assert!(
            !worth_listing(Some(ours), ours, &[], &[], &a),
            "nacelle's own window was offered as a window to switch to — the \
             row switches to the thing you are already looking at"
        );
        assert!(
            worth_listing(Some(ours), theirs, &[], &[], &a),
            "somebody else's window vanished from the list because ours was \
             known"
        );
        assert!(
            worth_listing(None, ours, &[], &[], &a),
            "with no window of our own on this display, every window is \
             somebody else's"
        );
    }

    /// **The gamescope path asks the server for exactly what it asked
    /// for before the connector existed.**
    ///
    /// `PROPERTY_CHANGE` on the root is the reading half:
    /// `_NET_CLIENT_LIST` and `_NET_ACTIVE_WINDOW`, which a manager
    /// rewrites on every window change and every change of focus. Under
    /// [`Policy::Enlarge`] nobody builds a window list at all, so every
    /// one of those events would be put on the wire, woken up for,
    /// drained and dropped — a cost that is small, and that this path
    /// promises in writing not to have.
    #[test]
    fn watching_for_windows_costs_less_than_reading_them() {
        let watching = u32::from(root_mask(Policy::Enlarge));
        let reading = u32::from(root_mask(Policy::Observe));
        let property = u32::from(EventMask::PROPERTY_CHANGE);
        let substructure = u32::from(EventMask::SUBSTRUCTURE_NOTIFY);

        assert_eq!(
            watching & property,
            0,
            "the gamescope path asked to hear about every property on the \
             root, and nothing on that path ever reads one"
        );
        assert_ne!(reading & property, 0, "the reading path cannot see the client list move");
        assert_ne!(watching & substructure, 0, "the enlarging path cannot see a window map");
        assert_ne!(reading & substructure, 0, "the reading path cannot see a window map");
    }

    /// **Only the properties this carrier reads are worth reading the
    /// list again for.**
    ///
    /// [`Ewmh::rebuild`] selects `PROPERTY_CHANGE` on every window it
    /// lists, so this carrier hears everything every application writes
    /// about itself. `_NET_WM_USER_TIME` is written on every keystroke
    /// and every click: without this filter, typing in somebody else's
    /// editor rebuilt the whole list — seven round trips per window per
    /// keypress — to produce a list that came out identical every time.
    /// The epoch stayed put and the requests were paid for anyway.
    #[test]
    fn a_keystroke_in_somebody_elses_window_does_not_reread_the_list() {
        let a = bench();
        // Not in the bench table at all: exactly what `_NET_WM_USER_TIME`,
        // `_NET_WM_SYNC_REQUEST_COUNTER` or a toolkit's private property
        // looks like from here.
        assert!(
            !worth_rereading(9001, &a),
            "a property this carrier never reads sent it to read every \
             property of every window"
        );
        assert!(
            !worth_rereading(a._NET_WM_ICON, &a),
            "an application redrawing its icon rebuilt the list — icons are \
             fetched on demand and are not in the snapshot"
        );
        assert!(
            !worth_rereading(a._NET_FRAME_EXTENTS, &a),
            "a frame thickness changing rebuilt the list — it is read when a \
             window is moved and never stored"
        );

        for (name, atom) in [
            ("_NET_CLIENT_LIST", a._NET_CLIENT_LIST),
            ("_NET_ACTIVE_WINDOW", a._NET_ACTIVE_WINDOW),
            ("_NET_WM_NAME", a._NET_WM_NAME),
            ("_NET_WM_STATE", a._NET_WM_STATE),
            ("_NET_WM_DESKTOP", a._NET_WM_DESKTOP),
            ("_NET_WM_WINDOW_TYPE", a._NET_WM_WINDOW_TYPE),
            ("WM_NAME", AtomEnum::WM_NAME.into()),
            ("WM_CLASS", AtomEnum::WM_CLASS.into()),
        ] {
            assert!(
                worth_rereading(atom, &a),
                "{name} is read into the snapshot and its changing was ignored \
                 — the list would stay wrong until something else disturbed it"
            );
        }
    }

    /// **A client's icon property is a client's word, and can be a
    /// lie.**
    ///
    /// `_NET_WM_ICON` is written by the application. A block claiming
    /// more pixels than the property holds, a zero-sided icon, a size
    /// whose product overflows — none of these is hypothetical, and all
    /// of them are read on the desktop's own thread. The walk has to
    /// stop, not panic and not spin.
    #[test]
    fn a_malformed_icon_property_stops_the_walk_instead_of_the_program() {
        assert_eq!(best_icon(&[], 64), None, "nothing at all produced an icon");
        assert_eq!(best_icon(&[16], 64), None, "half a header produced an icon");
        assert_eq!(
            best_icon(&[16, 16, 1, 2, 3], 64),
            None,
            "a block claiming 256 pixels over a buffer of three was believed"
        );
        assert_eq!(
            best_icon(&[0, 0, 7, 7, 7], 64),
            None,
            "a zero-sided icon did not end the walk"
        );
        assert_eq!(
            best_icon(&[u32::MAX, u32::MAX, 1], 64),
            None,
            "a size whose product overflows was multiplied out"
        );
        // A good block first, then rubbish: what is readable is kept.
        assert_eq!(
            best_icon(&[1, 1, 0xff00_0000, 99, 99], 64),
            Some(Icon::Pixels { w: 1, h: 1, argb: vec![0xff00_0000] }),
            "rubbish after a good icon threw the good icon away"
        );
    }

    /// **The icon chosen is the smallest one still big enough.**
    ///
    /// Applications ship a handful of sizes in one property, in no
    /// order. Taking the first is taking whatever the toolchain
    /// happened to write first; taking the biggest is scaling 512 px
    /// down to a row height every frame. And when nothing is big
    /// enough, the biggest there is loses less than the smallest, so
    /// the rule has two halves and both are checked.
    #[test]
    fn the_icon_chosen_is_the_smallest_one_still_big_enough() {
        let mut prop = Vec::new();
        for side in [16u32, 128, 48] {
            prop.push(side);
            prop.push(side);
            prop.extend(std::iter::repeat(side).take((side * side) as usize));
        }
        let Some(Icon::Pixels { w, h, argb }) = best_icon(&prop, 32) else {
            panic!("a well-formed property yielded no icon")
        };
        assert_eq!((w, h), (48, 48), "the size picked was not the smallest fit");
        assert_eq!(argb.len(), 48 * 48, "the pixels handed back are not the icon's");
        assert!(argb.iter().all(|&p| p == 48), "the pixels came from another block");

        let Some(Icon::Pixels { w, .. }) = best_icon(&prop, 4096) else {
            panic!("nothing was big enough and nothing was returned")
        };
        assert_eq!(w, 128, "with nothing big enough, the biggest must win");
    }

    /// **`WM_CLASS` names the application by its second string.**
    ///
    /// The property is instance-then-class, both NUL-terminated, and
    /// the trailing NUL means a naive split leaves an empty tail. Taking
    /// the first string gives the instance name — `dolphin` where the
    /// application is `org.kde.dolphin` — which is a different string on
    /// enough applications to matter and identical on enough to hide.
    #[test]
    fn the_application_is_named_by_the_class_and_not_the_instance() {
        assert_eq!(wm_class(b"xwaylandvideobridge\0xwaylandvideobridge\0"), "xwaylandvideobridge");
        assert_eq!(wm_class(b"dolphin\0org.kde.dolphin\0"), "org.kde.dolphin");
        assert_eq!(
            wm_class(b"only-one\0"),
            "only-one",
            "a property with one string must still name something"
        );
        assert_eq!(wm_class(b""), "", "an absent property must not invent a name");
        assert_eq!(wm_class(b"\0\0"), "", "a property of nothing but separators");
    }

    /// A hand probe, not coverage: what this carrier sees on the
    /// machine it is run on.
    ///
    /// Ignored by default and it must stay ignored — it needs a display
    /// and it answers differently on every machine, so it can neither
    /// pass nor fail in a way that means anything. It exists because
    /// everything above is a bench, and a bench cannot tell you that
    /// `_NET_CLIENT_LIST` is readable, that the titles come back as
    /// UTF-8, or that the filter leaves anything standing.
    ///
    /// ```text
    /// cargo test --offline -- --ignored --nocapture what_this_machine_shows
    /// ```
    ///
    /// It reads and selects for events on its own connection. It sends
    /// no client message and moves nothing.
    #[test]
    #[ignore = "needs a display; reports rather than asserts"]
    fn what_this_machine_shows() {
        let Some(wm) = Ewmh::start(Policy::Observe, Host::nobody().x11_window) else {
            println!("no display, or nobody claims to be an EWMH window manager");
            return;
        };
        println!("carrier: {}", wm.carrier());
        println!("blind spot: {}", wm.blind_spot().unwrap_or("none"));
        println!("{} window(s) worth listing:", wm.windows().len());
        for w in wm.windows() {
            println!(
                "  {:?}  board={:?}  {:?}  place={:?}\n      title: {}\n      app:   {}",
                w.id, w.board, w.state, w.place, w.title, w.app
            );
        }
    }

    /// A round trip through a real window manager: this test opens a
    /// window, asks the carrier what it sees, and closes it again.
    ///
    /// Ignored by default — it needs a display with a window manager on
    /// it, and it puts a window on somebody's screen for as long as it
    /// takes to read three properties. Run by hand:
    ///
    /// ```text
    /// cargo test --offline -- --ignored --nocapture a_window_this_test_opens
    /// ```
    ///
    /// Worth having anyway, because everything else in this file is a
    /// bench. A bench cannot tell you that `_NET_WM_NAME` comes back as
    /// UTF-8 rather than as a latin-1 `WM_NAME`, that `WM_CLASS` really
    /// does arrive as two NUL-separated strings, or that the manager
    /// puts a newly mapped window into `_NET_CLIENT_LIST` at all.
    #[test]
    #[ignore = "opens a window on the running display"]
    fn a_window_this_test_opens_comes_back_with_its_name() {
        use x11rb::protocol::xproto::{CreateWindowAux, PropMode};
        use x11rb::wrapper::ConnectionExt as _;
        use x11rb::COPY_DEPTH_FROM_PARENT;

        let Ok((conn, screen_num)) = x11rb::connect(None) else {
            println!("no display");
            return;
        };
        let root = conn.setup().roots[screen_num].root;
        let win = conn.generate_id().expect("an id");
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            root,
            0,
            0,
            200,
            120,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .expect("create")
        .check()
        .expect("create");
        let utf8 = conn
            .intern_atom(false, b"UTF8_STRING")
            .expect("intern")
            .reply()
            .expect("intern")
            .atom;
        let net_wm_name = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .expect("intern")
            .reply()
            .expect("intern")
            .atom;
        conn.change_property8(
            PropMode::REPLACE,
            win,
            net_wm_name,
            utf8,
            "nacelle łącznik — ątę".as_bytes(),
        )
        .expect("title");
        conn.change_property8(
            PropMode::REPLACE,
            win,
            Atom::from(AtomEnum::WM_CLASS),
            Atom::from(AtomEnum::STRING),
            b"probe\0org.nacelle.probe\0",
        )
        .expect("class");
        conn.map_window(win).expect("map").check().expect("map");
        conn.flush().expect("flush");

        let Some(mut wm) = Ewmh::start(Policy::Observe, Host::nobody().x11_window) else {
            let _ = conn.destroy_window(win);
            let _ = conn.flush();
            println!("nobody claims to be an EWMH window manager");
            return;
        };
        // The manager updates `_NET_CLIENT_LIST` when it gets round to
        // it, which is not the same instant the window mapped.
        let mut seen = None;
        for _ in 0..200 {
            Backend::poll(&mut wm);
            seen = wm.windows().iter().find(|w| w.app == "org.nacelle.probe").cloned();
            if seen.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // The control half, on this test's own window and nobody
        // else's: ask for a move, and see whether the manager did it.
        // Everything else about the orders is checked on a bench, which
        // proves the five words are right and proves nothing about
        // whether a window manager acts on them.
        let mut moved = None;
        if let Some(w) = seen.as_ref() {
            let want = Place { x: 300, y: 200, w: 240, h: 160 };
            let answer = wm.act(Act::Place(w.id, want));
            for _ in 0..200 {
                Backend::poll(&mut wm);
                let now = wm.windows().iter().find(|c| c.id == w.id).and_then(|c| c.place);
                if now == Some(want) {
                    moved = Some((answer.clone(), now));
                    break;
                }
                moved = Some((answer.clone(), now));
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let _ = conn.destroy_window(win);
        let _ = conn.flush();

        let seen = seen.expect(
            "the window this test mapped never appeared in _NET_CLIENT_LIST — \
             either the manager does not keep the list, or the reader does not \
             read it",
        );
        println!("read back: {seen:?}");
        let (answer, after) = moved.expect("the move was never attempted");
        println!("move answered {answer:?}, geometry became {after:?}");
        assert_eq!(answer, Outcome::Sent, "the order never left");
        assert_eq!(
            after,
            Some(Place { x: 300, y: 200, w: 240, h: 160 }),
            "the manager did not move the window this test asked it to move — \
             the whole control half is words on a wire that nobody acts on"
        );
        assert_eq!(
            seen.title, "nacelle łącznik — ątę",
            "the title came back mangled — _NET_WM_NAME is UTF-8 and WM_NAME \
             is not, and reading the wrong one is invisible until a title has \
             a letter outside ASCII in it"
        );
        assert_eq!(seen.app, "org.nacelle.probe", "WM_CLASS gave the instance, not the class");
        assert!(seen.place.is_some(), "a mapped window has no geometry");
    }

    /// **EWMH is the carrier that can do everything, and the Wayland
    /// one can do a part of it.**
    ///
    /// This is the shape of the whole branch stated as an assertion.
    /// EWMH answers every verb in the vocabulary; the neutral Wayland
    /// protocol answers three of them and none of the orders. A verb
    /// added to [`Verb::ALL`] therefore has to be answered here
    /// deliberately, and a Wayland carrier that grew a verb EWMH does
    /// not have would mean the vocabulary had stopped being one
    /// vocabulary — the interface would have to ask which carrier it
    /// was talking to before it knew what it could offer, which is the
    /// arrangement this seam exists to replace.
    #[test]
    fn ewmh_answers_the_whole_vocabulary_and_wayland_answers_a_part_of_it() {
        for verb in Verb::ALL {
            assert!(
                Ewmh::KNOWS.contains(&verb),
                "'{}' was added to the vocabulary and the EWMH carrier — the \
                 one carrier that can do everything today — was not told",
                verb.label()
            );
        }
        for verb in super::super::wayland::Toplevels::KNOWS {
            assert!(
                Ewmh::KNOWS.contains(verb),
                "the wayland carrier offers '{}' and the EWMH one does not — \
                 the two carriers no longer speak one vocabulary",
                verb.label()
            );
        }
        assert!(
            super::super::wayland::Toplevels::KNOWS.len() < Ewmh::KNOWS.len(),
            "the neutral protocol was measured as a strict subset of EWMH; if \
             that stopped being true the blind spots written under both \
             carriers are out of date"
        );
    }
}
