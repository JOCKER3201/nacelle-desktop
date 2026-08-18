//! The Wayland carrier: `ext-foreign-toplevel-list-v1`.
//!
//! Reading only, and that is the protocol's whole design — it says so
//! in its own preamble: "intentionally minimalistic and expects
//! additional functionality … to be implemented in extension
//! protocols". What it gives is a handle per mapped toplevel, a title,
//! an app id and a stable identifier. What it does not give is state,
//! icon, board, or any way to act on a window at all. So
//! [`Backend::can`] here answers yes to exactly three verbs, and the
//! interface greys out the rest rather than drawing dead buttons.
//!
//! # The second connection
//!
//! The same arrangement `wl_color` uses and for the same reason:
//! libwayland is built for one socket with many queues, so this opens a
//! second [`Connection`] onto winit's own display and gives it a queue
//! of its own. Only toplevel news travels here, so the two event loops
//! never trip over each other.
//!
//! # Why nothing shows before `done`
//!
//! The protocol requires it — "The configured state must not be applied
//! immediately. See ext_foreign_toplevel_handle_v1.done for details" —
//! and the requirement has teeth. Title and app id arrive as separate
//! events, so a list built as they land would show a window with no
//! title for a frame and then a window with no app id, and a rename
//! would be visible half-applied. So everything goes into a draft and
//! the draft is committed on `done`.
//!
//! # Why `done` is not automatically news
//!
//! `done` is the atomic-commit point for EVERY extension hung off this
//! protocol, not just for the two strings read here. A compositor that
//! also speaks `ext-workspace-v1`, or a toplevel-management extension,
//! fires `done` whenever any of that changes — which for a focused
//! window is many times a second. Committing a draft that came out
//! identical must therefore not count as a change, or the epoch ticks
//! forever and every reader memoising on it rebuilds forever.

use std::collections::BTreeMap;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;

use wayland_client::backend::Backend as WlBackend;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};

use super::{reads_differently, Act, Backend, Icon, Names, Outcome, Verb, Window, WindowId};

/// Everything one toplevel has said since its last `done`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Draft {
    title: Option<String>,
    app: Option<String>,
    ident: Option<String>,
}

/// Everything one toplevel has said, as of its last `done`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Shown {
    title: String,
    app: String,
    /// The protocol's own cross-process name for this toplevel. Nothing
    /// here needs it — the identity handed to the interface is minted
    /// by [`Names`] — but it is the only thing that could carry "this
    /// window belongs on board 3" across a restart, so it is kept
    /// rather than dropped on the floor.
    ident: String,
}

/// The bookkeeping, with no Wayland types in it beyond the event enum.
///
/// Keyed on the handle's protocol id, which is what
/// [`Proxy::id`]`.protocol_id()` answers and what the compositor reuses
/// once a handle is destroyed — hence [`Names`] on top of it.
#[derive(Default)]
struct Feed {
    /// Registry name and version, if the compositor advertises the list.
    global: Option<(u32, u32)>,
    draft: BTreeMap<u32, Draft>,
    shown: BTreeMap<u32, Shown>,
    changed: bool,
    finished: bool,
}

impl Feed {
    /// A handle was created for a toplevel we have not heard of.
    fn opened(&mut self, key: u32) {
        self.draft.entry(key).or_default();
    }

    /// One event about one toplevel.
    ///
    /// Separate from the [`Dispatch`] impl so the bookkeeping can be
    /// driven with as many toplevels as a test likes — a queue with no
    /// compositor behind it can only make one proxy, and one proxy
    /// cannot be two windows.
    fn hear(&mut self, key: u32, ev: ext_foreign_toplevel_handle_v1::Event) {
        use ext_foreign_toplevel_handle_v1::Event as E;
        match ev {
            E::Title { title } => {
                self.draft.entry(key).or_default().title = Some(title);
            }
            E::AppId { app_id } => {
                self.draft.entry(key).or_default().app = Some(app_id);
            }
            E::Identifier { identifier } => {
                self.draft.entry(key).or_default().ident = Some(identifier);
            }
            E::Done => self.commit(key),
            E::Closed => {
                self.draft.remove(&key);
                if self.shown.remove(&key).is_some() {
                    self.changed = true;
                }
            }
            _ => {}
        }
    }

    /// The draft becomes what is shown — and only then is it news, and
    /// only if it came out different.
    fn commit(&mut self, key: u32) {
        let Some(draft) = self.draft.get(&key).cloned() else { return };
        let was = self.shown.get(&key);
        let next = Shown {
            title: draft.title.clone().or_else(|| was.map(|s| s.title.clone())).unwrap_or_default(),
            app: draft.app.clone().or_else(|| was.map(|s| s.app.clone())).unwrap_or_default(),
            ident: draft.ident.clone().or_else(|| was.map(|s| s.ident.clone())).unwrap_or_default(),
        };
        if was != Some(&next) {
            self.shown.insert(key, next);
            self.changed = true;
        }
        self.draft.insert(key, Draft::default());
    }
}

/// The carrier the desktop holds.
pub struct Toplevels {
    conn: Connection,
    queue: EventQueue<Feed>,
    feed: Feed,
    list: Option<ExtForeignToplevelListV1>,
    names: Names,
    snapshot: Vec<Window>,
}

impl Toplevels {
    /// The three verbs this protocol can honour. One table, read by
    /// [`Backend::can`] and by the test that walks the vocabulary.
    pub(super) const KNOWS: &'static [Verb] = &[Verb::List, Verb::Title, Verb::App];

    /// Attaches to winit's display. None when the compositor does not
    /// advertise the list — which on KWin 6.7.4 it does not, so the
    /// connector falls through to EWMH rather than showing an empty
    /// list forever.
    pub fn start(display: *mut std::ffi::c_void) -> Option<Toplevels> {
        if display.is_null() {
            return None;
        }
        let backend = unsafe { WlBackend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);
        let mut queue: EventQueue<Feed> = conn.new_event_queue();
        let qh = queue.handle();
        let mut feed = Feed::default();
        let _registry = conn.display().get_registry(&qh, ());
        queue.roundtrip(&mut feed).ok()?;
        let (name, version) = feed.global?;
        let registry = conn.display().get_registry(&qh, ());
        let list: ExtForeignToplevelListV1 = registry.bind(
            name,
            version.min(ExtForeignToplevelListV1::interface().version),
            &qh,
            (),
        );
        // The compositor sends one `toplevel` per mapped window right
        // after the bind, then a handle's worth of properties each.
        // Two round trips: the first brings the handles, the second the
        // `done` that makes them showable.
        queue.roundtrip(&mut feed).ok()?;
        queue.roundtrip(&mut feed).ok()?;
        let mut me = Toplevels {
            conn,
            queue,
            feed,
            list: Some(list),
            names: Names::new(),
            snapshot: Vec::new(),
        };
        me.rebuild();
        Some(me)
    }

    /// Turns the bookkeeping into the snapshot the interface reads.
    fn rebuild(&mut self) {
        let keys: Vec<u64> = self.feed.shown.keys().map(|&k| k as u64).collect();
        self.names.retain(&keys);
        self.snapshot = self
            .feed
            .shown
            .iter()
            .map(|(&key, s)| {
                let mut w = Window::new(self.names.of(key as u64));
                w.title = s.title.clone();
                w.app = s.app.clone();
                w
            })
            .collect();
    }
}

impl Drop for Toplevels {
    fn drop(&mut self) {
        // The protocol asks for stop-then-destroy. Nothing waits for
        // the `finished` that answers it, because the process is on its
        // way out and the compositor drops the objects with the client.
        if let Some(list) = self.list.take() {
            list.stop();
            let _ = self.conn.flush();
        }
    }
}

impl Backend for Toplevels {
    fn carrier(&self) -> &'static str {
        "wayland ext-foreign-toplevel-list-v1"
    }

    fn can(&self, verb: Verb) -> bool {
        Toplevels::KNOWS.contains(&verb)
    }

    fn blind_spot(&self) -> Option<&'static str> {
        Some(
            "this protocol lists windows and names them; it carries no state, \
             no icon, no way to act on a window, and no way for a client to \
             tell which of the windows is its own",
        )
    }

    fn poll(&mut self) -> bool {
        let _ = self.conn.flush();
        let _ = self.queue.dispatch_pending(&mut self.feed);
        // Read only when the socket already holds bytes. A blocking
        // read here would stall the frame on the compositor, and this
        // queue rides winit's socket — the one the whole program's
        // input arrives on.
        if let Some(guard) = self.queue.prepare_read() {
            let fd = self.conn.as_fd().as_raw_fd();
            let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            let ready = unsafe { libc::poll(&mut pfd, 1, 0) } > 0;
            if ready {
                let _ = guard.read();
            } else {
                drop(guard);
            }
            let _ = self.queue.dispatch_pending(&mut self.feed);
        }
        if !self.feed.changed {
            return false;
        }
        self.feed.changed = false;
        let before = std::mem::take(&mut self.snapshot);
        self.rebuild();
        // The same rule as the other carrier, from the same function:
        // news is the list reading differently. `changed` is only the
        // bookkeeping saying it committed something, and a compositor
        // is free to commit a window's own values back to it.
        reads_differently(&self.snapshot, &before)
    }

    fn windows(&self) -> &[Window] {
        &self.snapshot
    }

    fn icon(&mut self, _: WindowId, _: u32) -> Option<Icon> {
        // Not "no icon" — no way to ask. The app id is on the window
        // already; whoever wants an icon looks it up in the icon theme
        // and that is a job for the toolkit, not for this seam.
        None
    }

    fn act(&mut self, act: Act) -> Outcome {
        let _ = act;
        Outcome::Unsupported
    }
}

impl Dispatch<WlRegistry, ()> for Feed {
    fn event(
        state: &mut Self,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == ExtForeignToplevelListV1::interface().name {
                state.global = Some((name, version));
            }
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for Feed {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                state.opened(toplevel.id().protocol_id());
            }
            ext_foreign_toplevel_list_v1::Event::Finished => {
                state.finished = true;
            }
            _ => {}
        }
    }

    event_created_child!(Feed, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for Feed {
    fn event(
        state: &mut Self,
        proxy: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.hear(proxy.id().protocol_id(), event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use wayland_client::backend::ObjectId;

    fn title(s: &str) -> ext_foreign_toplevel_handle_v1::Event {
        ext_foreign_toplevel_handle_v1::Event::Title { title: s.to_string() }
    }
    fn app(s: &str) -> ext_foreign_toplevel_handle_v1::Event {
        ext_foreign_toplevel_handle_v1::Event::AppId { app_id: s.to_string() }
    }
    fn done() -> ext_foreign_toplevel_handle_v1::Event {
        ext_foreign_toplevel_handle_v1::Event::Done
    }

    /// **Nothing is shown before the compositor says `done`.**
    ///
    /// The protocol demands it in so many words, and the reason is
    /// visible on a screen: title and app id are two events, so a list
    /// that took them as they came would draw a nameless window for one
    /// frame and a window with no app for the next. Worse for a rename,
    /// where the old app id would stand under the new title.
    #[test]
    fn a_toplevel_is_not_in_the_list_until_the_compositor_says_done() {
        let mut feed = Feed::default();
        feed.opened(7);
        feed.hear(7, title("Files"));
        assert!(
            feed.shown.is_empty(),
            "a half-described window reached the list — it would be drawn \
             with whichever of its two names happened to arrive first"
        );
        assert!(!feed.changed, "an uncommitted draft counted as news");

        feed.hear(7, app("org.kde.dolphin"));
        feed.hear(7, done());
        assert_eq!(feed.shown.len(), 1, "a described window never reached the list");
        assert_eq!(feed.shown[&7].title, "Files");
        assert_eq!(feed.shown[&7].app, "org.kde.dolphin");
        assert!(feed.changed, "a window appearing was not news");
    }

    /// **A rename lands whole or not at all.**
    ///
    /// The second `done` is what makes the new title and the new app id
    /// visible together. Between them the window keeps BOTH old values
    /// — not one old and one new, which is what a list built event by
    /// event would show.
    #[test]
    fn a_rename_lands_whole_or_not_at_all() {
        let mut feed = Feed::default();
        feed.opened(7);
        feed.hear(7, title("one.txt"));
        feed.hear(7, app("org.gnome.TextEditor"));
        feed.hear(7, done());
        feed.changed = false;

        feed.hear(7, title("two.txt"));
        assert_eq!(feed.shown[&7].title, "one.txt", "half a rename was already visible");
        assert!(!feed.changed, "half a rename counted as news");

        feed.hear(7, done());
        assert_eq!(feed.shown[&7].title, "two.txt", "the rename never landed");
        assert_eq!(
            feed.shown[&7].app, "org.gnome.TextEditor",
            "the app id was dropped by a change that never mentioned it"
        );
        assert!(feed.changed, "the rename was not news");
    }

    /// **A `done` that changes nothing is not news.**
    ///
    /// `done` is the commit point for every extension hung off this
    /// protocol, so a compositor that also speaks workspaces or
    /// toplevel management fires it whenever any of that moves — many
    /// times a second for a focused window. Counting each one as a
    /// change would move the connector's epoch on every frame, and an
    /// epoch that never rests is the bug that pinned this program's CPU
    /// at 100 % once already.
    #[test]
    fn a_done_that_says_nothing_new_is_not_news() {
        let mut feed = Feed::default();
        feed.opened(7);
        feed.hear(7, title("Files"));
        feed.hear(7, done());
        feed.changed = false;

        for _ in 0..64 {
            feed.hear(7, done());
        }
        assert!(
            !feed.changed,
            "a bare `done` was taken for a change — the epoch would move on \
             every frame a window is focused"
        );

        feed.hear(7, title("Files"));
        feed.hear(7, done());
        assert!(!feed.changed, "being renamed to the same name counted as a change");

        feed.hear(7, title("Downloads"));
        feed.hear(7, done());
        assert!(feed.changed, "a real rename was not news");
    }

    /// **A closed toplevel leaves the list, and its slot does not bring
    /// it back.**
    ///
    /// The compositor reuses the protocol id of a destroyed handle, so
    /// the next window to open can land on the same number. The list
    /// must lose the old one, and the identity minted for the new one
    /// must not be the old one's.
    #[test]
    fn a_closed_toplevel_leaves_and_its_slot_does_not_resurrect_it() {
        let mut feed = Feed::default();
        feed.opened(7);
        feed.hear(7, title("Files"));
        feed.hear(7, done());
        feed.changed = false;

        feed.hear(7, ext_foreign_toplevel_handle_v1::Event::Closed);
        assert!(feed.shown.is_empty(), "a closed window stayed in the list");
        assert!(feed.changed, "a window closing was not news");

        let mut names = Names::new();
        let first = names.of(7);
        names.retain(&[]);
        feed.opened(7);
        feed.hear(7, title("Terminal"));
        feed.hear(7, done());
        let second = names.of(7);
        assert_ne!(
            first, second,
            "the new window on the reused slot inherited the closed window's \
             identity"
        );
    }

    /// **Two toplevels are two windows.**
    ///
    /// The keying is what keeps them apart, and getting it wrong is
    /// invisible with one window open — which is the case a hand test
    /// on a fresh session hits first.
    #[test]
    fn two_toplevels_do_not_share_a_draft() {
        let mut feed = Feed::default();
        feed.opened(7);
        feed.opened(9);
        feed.hear(7, title("Files"));
        feed.hear(9, title("Terminal"));
        feed.hear(9, done());
        assert_eq!(feed.shown.len(), 1, "committing one window committed the other");
        assert_eq!(feed.shown[&9].title, "Terminal");
        feed.hear(7, done());
        assert_eq!(feed.shown[&7].title, "Files", "the two windows shared a title");
        assert_eq!(feed.shown[&9].title, "Terminal");
    }

    /// **The registry handler recognises the global by the protocol's
    /// own name.**
    ///
    /// The whole carrier hangs off this one string comparison: get it
    /// wrong and [`Toplevels::start`] answers None on every compositor
    /// alive, the connector falls through to EWMH, and nobody sees a
    /// bug — just a shorter list. So the name is taken from the
    /// generated interface rather than typed out, and this proves the
    /// handler stores what it is told.
    #[test]
    fn the_registry_handler_hears_the_global_it_is_looking_for() {
        assert_eq!(
            ExtForeignToplevelListV1::interface().name,
            "ext_foreign_toplevel_list_v1",
            "the protocol this branch was measured against is not the one \
             being bound"
        );
        let wire = Wire::new();
        let mut feed = Feed::default();
        wire.registry(
            &mut feed,
            wl_registry::Event::Global {
                name: 3,
                interface: "wl_compositor".to_string(),
                version: 6,
            },
        );
        assert_eq!(feed.global, None, "any global at all was taken for this one");
        wire.registry(
            &mut feed,
            wl_registry::Event::Global {
                name: 42,
                interface: ExtForeignToplevelListV1::interface().name.to_string(),
                version: 1,
            },
        );
        assert_eq!(feed.global, Some((42, 1)), "the global went unheard");
    }

    /// **The handle handler is wired to the bookkeeping.**
    ///
    /// Everything above drives [`Feed::hear`] directly, which proves
    /// the rules and not the wiring. This one goes through
    /// [`Dispatch::event`] exactly as the queue would, so a handler
    /// that stopped calling `hear` is a failure and not a silent list
    /// that never fills.
    ///
    /// The socket pair and the null id are the same trick `wl_color`'s
    /// tests use: `Connection::from_socket` performs no handshake, so
    /// the far end is never spoken to, and `Proxy::from_id` accepts the
    /// null id by design — which is what makes a proxy with no server
    /// possible at all.
    ///
    /// What this CANNOT prove, said plainly because a reader would
    /// otherwise assume it: the null proxy's protocol id is zero, so a
    /// handler that filed everything under a hard-coded zero would pass
    /// here. Only a real compositor hands out two distinct ids. The
    /// keying is proved instead by
    /// [`two_toplevels_do_not_share_a_draft`], which drives the
    /// bookkeeping with two keys and no proxy at all — between the two
    /// tests, both halves are covered, and neither covers both.
    struct Wire {
        conn: Connection,
        _queue: EventQueue<Feed>,
        qh: QueueHandle<Feed>,
        handle: ExtForeignToplevelHandleV1,
        registry: WlRegistry,
        _far_end: UnixStream,
    }

    impl Wire {
        fn new() -> Wire {
            let (near, far) = UnixStream::pair().expect("a socket pair");
            let conn = Connection::from_socket(near).expect("a connection to nobody");
            let queue: EventQueue<Feed> = conn.new_event_queue();
            let qh = queue.handle();
            let handle = ExtForeignToplevelHandleV1::from_id(&conn, ObjectId::null())
                .expect("a null proxy");
            let registry = WlRegistry::from_id(&conn, ObjectId::null()).expect("a null proxy");
            Wire { conn, _queue: queue, qh, handle, registry, _far_end: far }
        }

        fn deliver(&self, feed: &mut Feed, event: ext_foreign_toplevel_handle_v1::Event) {
            <Feed as Dispatch<ExtForeignToplevelHandleV1, ()>>::event(
                feed,
                &self.handle,
                event,
                &(),
                &self.conn,
                &self.qh,
            );
        }

        fn registry(&self, feed: &mut Feed, event: wl_registry::Event) {
            <Feed as Dispatch<WlRegistry, ()>>::event(
                feed,
                &self.registry,
                event,
                &(),
                &self.conn,
                &self.qh,
            );
        }
    }

    #[test]
    fn the_handle_handler_reaches_the_bookkeeping() {
        let wire = Wire::new();
        let mut feed = Feed::default();
        let key = wire.handle.id().protocol_id();

        wire.deliver(&mut feed, title("Files"));
        wire.deliver(&mut feed, app("org.kde.dolphin"));
        assert!(feed.shown.is_empty(), "the handler committed before `done`");
        wire.deliver(&mut feed, done());
        assert_eq!(
            feed.shown.get(&key).map(|s| s.title.as_str()),
            Some("Files"),
            "the dispatch handler dropped the compositor's news on the floor — \
             the list would stay empty on a compositor that speaks the protocol"
        );
    }

    /// **The carrier offers only what the protocol can do.**
    ///
    /// [`Toplevels::KNOWS`] is read by `can`; `act` answers
    /// `Unsupported` to everything. If a verb were ever added to the
    /// table without an implementation behind it, the interface would
    /// draw a live control over a carrier that does nothing — which is
    /// the exact failure the seam exists to prevent.
    #[test]
    fn the_wayland_carrier_offers_no_verb_it_cannot_do() {
        for verb in Verb::ALL {
            let offered = Toplevels::KNOWS.contains(&verb);
            let is_order = Act::specimen(verb, WindowId(1)).is_some();
            assert!(
                !(offered && is_order),
                "the wayland carrier offers '{}', which is an order, and this \
                 protocol carries no orders at all",
                verb.label()
            );
        }
        assert!(Toplevels::KNOWS.contains(&Verb::List), "a carrier that lists nothing");
        assert!(Toplevels::KNOWS.contains(&Verb::Title));
        assert!(
            !Toplevels::KNOWS.contains(&Verb::Icon),
            "the protocol carries no icon and the carrier must not claim one"
        );
    }
}
