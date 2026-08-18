//! One SCREEN: a monitor, and the whole desktop standing on it.
//!
//! This replaces the old `sala`, which drew a second monitor as a plan
//! of the first — the board's rectangles, solved for its own size, with
//! nothing inside them. That was not a desktop but a diagram of one,
//! and it could not become one by degrees: it was built on the
//! assumption that a widget is a live singular thing living on the main
//! screen, so a second copy of it could not exist. Every screen here
//! runs its OWN widgets, off its OWN layaut, on its OWN board.
//!
//! What lives here is everything that used to be a local of the one
//! event loop: which board is being stood on, the ride between boards,
//! the widgets that board runs, the layaut the screen took, the editor
//! over it, the decoration plates baked for its size. The main screen
//! stopped being a special case the moment those stopped being locals —
//! it is simply the first element of the list.
//!
//! What does NOT live here is the application's own interface: one
//! settings window, one popup, one context menu, one shell. Those are
//! the program's, drawn on whichever screen is hosting them, and a
//! second copy of any of them would be a second program.

use crate::config;
use crate::widgets;
use nacelle::base::{Panel, Rect, SizeTable};
use nacelle::draw::ImageId;
use nacelle::layout::{BoardId, InstanceId, LayoutDef};
use nacelle::stage::BoardWorld;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowBuilder};

/// A board move finishing (or undoing) itself after the hand let go.
pub struct Cube {
    pub horizontal: bool,
    pub a0: f32,
    pub a1: f32,
    pub t0: Instant,
    /// Board the move lands on when it completes.
    pub to: BoardId,
    /// Board shown coming in while it moves.
    pub face_b: BoardId,
}

/// One running widget: which placement it answers for, what it runs,
/// and the two things its last frame left behind.
struct Live {
    id: InstanceId,
    widget: Panel,
    /// None only while the factory refused the file: the placement
    /// takes part in the layout and draws nothing.
    inst: Option<Box<dyn widgets::Widget>>,
    /// The content box the host's container left it (u2 §4.1) — the
    /// ONE rect its clicks and wheel turns are answered in, stored
    /// rather than recomputed, because input arrives with no frame in
    /// flight.
    content: Option<Rect>,
    /// The font scale its last measurement asked for.
    scale: f32,
}

/// The widgets ONE screen is running, one entry per PLACED INSTANCE.
///
/// THE INVARIANT THIS TYPE EXISTS FOR (u3 §5, and asked for by name by
/// the owner of the project): A WIDGET THAT IS ON NO BOARD DOES NOT
/// RUN. Nothing is built for a placement the layaut does not make, and
/// a placement that leaves the layaut takes its widget's work with it —
/// its threads, its shell, its directory scans — because dropping the
/// box is what ends them (a plugin's Drop crosses the ABI and frees the
/// instance behind it).
///
/// The distinction that is easy to lose, and expensive when it is lost:
/// a widget on a board OTHER THAN THE ONE BEING LOOKED AT still runs.
/// A terminal must survive its board being turned away from, and the
/// presence scan is therefore over EVERY board of the layaut, never
/// over the visible one. What stops is what is in no layaut at all.
///
/// Keyed by instance and not by widget kind, because two terminals are
/// two shells: closing one of them may not take the other's process
/// with it.
#[derive(Default)]
pub struct WidgetSet {
    items: Vec<Live>,
}

impl WidgetSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Brings the running set into line with what the boards place:
    /// placed but not built — build; built but no longer placed — drop.
    /// Returns (built, dropped) for the caller that wants to say so.
    ///
    /// `present` is the whole layaut's placements, from every board.
    /// Handing in the visible board's alone is exactly the mistake this
    /// doc comment is here to prevent.
    pub fn sync(
        &mut self,
        present: &[(InstanceId, Panel)],
        mut make: impl FnMut(Panel) -> Option<Box<dyn widgets::Widget>>,
    ) -> (usize, usize) {
        let before = self.items.len();
        // Dropped first, so a screen swapping one layaut for another
        // never holds both sets of widgets at once.
        //
        // The WIDGET is compared as well as the identity, because two
        // layauts are two id spaces: swapping this screen's layaut can
        // hand placement 3 to a file browser where it used to be a
        // terminal, and keeping the old box would leave a shell running
        // under a name that no longer means it.
        self.items
            .retain(|l| present.iter().any(|(id, w)| *id == l.id && *w == l.widget));
        let dropped = before - self.items.len();
        let mut built = 0;
        for (id, widget) in present {
            if self.items.iter().any(|l| l.id == *id) {
                continue;
            }
            built += 1;
            self.items.push(Live {
                id: *id,
                widget: *widget,
                inst: make(*widget),
                content: None,
                scale: 1.0,
            });
        }
        (built, dropped)
    }

    // The `+ 'static` is not decoration: in return position `&mut dyn
    // Trait` defaults the object's own lifetime to the reference's, and
    // a `&mut` is invariant over what it points at — so the boxed
    // widget, which IS 'static, would not coerce into it.
    pub fn get_mut(
        &mut self,
        id: InstanceId,
    ) -> Option<&mut (dyn widgets::Widget + 'static)> {
        self.items
            .iter_mut()
            .find(|l| l.id == id)
            .and_then(|l| l.inst.as_deref_mut())
    }

    /// Every widget that came up, to be told something they all hear.
    pub fn each_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn widgets::Widget>> {
        self.items.iter_mut().filter_map(|l| l.inst.as_mut())
    }

    /// Which widget a placement is currently running — the other half
    /// of its identity, for state that outlives a frame by remembering
    /// a placement NUMBER (who owns the keyboard, who was told the
    /// button went down). An id alone cannot say whether that state is
    /// still about the same thing; the pair can.
    pub fn widget_of(&self, id: InstanceId) -> Option<Panel> {
        self.items.iter().find(|l| l.id == id).map(|l| l.widget)
    }

    /// The content box one placement's last draw used; None before it
    /// has drawn once.
    pub fn content(&self, id: InstanceId) -> Option<Rect> {
        self.items.iter().find(|l| l.id == id).and_then(|l| l.content)
    }

    pub fn set_content(&mut self, id: InstanceId, r: Rect) {
        if let Some(l) = self.items.iter_mut().find(|l| l.id == id) {
            l.content = Some(r);
        }
    }

    pub fn scale(&self, id: InstanceId) -> f32 {
        self.items.iter().find(|l| l.id == id).map(|l| l.scale).unwrap_or(1.0)
    }

    pub fn set_scale(&mut self, id: InstanceId, s: f32) {
        if let Some(l) = self.items.iter_mut().find(|l| l.id == id) {
            l.scale = s;
        }
    }

    /// How many placements actually came up — what the start-up line
    /// counts.
    pub fn running(&self) -> usize {
        self.items.iter().filter(|l| l.inst.is_some()).count()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The placement reporting a CHARACTER GRID — the terminal view,
    /// found by the capability it declares and by no name at all.
    pub fn grid_holder(&self) -> Option<(InstanceId, (usize, usize))> {
        self.items
            .iter()
            .find_map(|l| l.inst.as_ref().and_then(|w| w.grid()).map(|g| (l.id, g)))
    }
}

/// One monitor's whole desktop.
pub struct Screen {
    /// The socket this screen hangs off — a LABEL, and no longer an
    /// identity: [`Screen::id`] is what the configuration keys by and
    /// how this screen is told which layaut to take. Kept as a field of
    /// its own because it is what every caller wants to PRINT, and it
    /// is the same string `id.connector` holds.
    pub connector: Option<String>,
    /// What the configuration calls this screen: the monitor's own name
    /// when its firmware gives one, the socket when it does not.
    pub id: crate::screens::ScreenId,
    pub window: Window,
    gfx: nacelle_renderer::Gfx,
    /// Resolution + diagonal in inches: what picks a layaut's
    /// per-screen override section. A second monitor is not the first,
    /// so it answers for itself.
    pub key: (u32, u32, u32),
    /// The name of the layaut this screen took, for the log and for the
    /// settings screen that will let it be changed.
    pub layaut: String,
    /// The board world of that layaut: home, the row it sits on, the
    /// two fixtures above and below.
    pub world: BoardWorld,
    /// Where on that world this screen stands. Per screen, so a drag on
    /// the second monitor turns the second monitor.
    pub board: BoardId,
    /// A held click, not delivered yet: it becomes a board drag if the
    /// pointer travels, and the widget's click on release if it does
    /// not. Delivering on release is what lets one gesture be both.
    pub press_at: Option<(f32, f32)>,
    /// A drag in progress, locked to the axis it started on. Sideways
    /// the world turns like a cube and the number is degrees; up and
    /// down it slides flat and the number is the fraction of the window
    /// already travelled. Positive goes right, or down.
    pub pan: Option<(bool, f32)>,
    pub cube: Option<Cube>,
    /// Steps still to walk after the current move lands, last first.
    pub go_queue: Vec<BoardId>,
    /// The widgets this screen runs — its own, off its own layaut.
    pub widgets: WidgetSet,
    /// A left press a widget's `drag(Begin)` accepted owns the pointer
    /// until it is released (F1 §5.1).
    pub drag_capture: Option<InstanceId>,
    /// Which placement was told the button went DOWN, so exactly one
    /// release closes it. Separate from `drag_capture` on purpose: the
    /// press is delivered whether or not the widget then accepted the
    /// gesture, and what it turned into afterwards — a capture, a board
    /// ride, a plain click — must not decide whether its release ever
    /// arrives. A widget left holding a press that never came up is a
    /// control stuck dark.
    pub press_inst: Option<InstanceId>,
    /// Which placement owns the KEYBOARD here.
    ///
    /// Per screen, unlike the settings window and the menu, because the
    /// keys have already been routed before the program sees them: they
    /// arrive at whichever window the display server gave focus to, so
    /// the screen the event came in on is the only one that could
    /// answer for them. None means nobody has claimed it and every key
    /// keeps the route it had at boot — the shell's.
    ///
    /// This is CONTAINER focus, one per container, and not a second
    /// control chain: `FocusCtl` remains the one of those, and this
    /// only decides which widget the next key is OFFERED to.
    pub kbd: Option<InstanceId>,
    /// The layout editor over THIS screen: a screen is arranged where
    /// it is looked at, against its own pixels.
    pub editor: widgets::editor::Editor,
    /// The ADD WIDGET miniatures, by panel index. Kept apart from the
    /// running set: a preview is a picture of a widget the layaut does
    /// not place, and it may not be mistaken for one it does.
    preview: Vec<Option<Box<dyn widgets::Widget>>>,
    /// Whether this screen is still showing the boot log.
    pub booting: bool,
    /// When this screen last actually drew. The pace cannot be kept by
    /// only asking politely: the display server asks for a redraw of
    /// its own whenever it exposes the window, and dragging a framed
    /// window over the desktop exposes it hundreds of times a second.
    /// Per screen, or two monitors would share one screen's worth of
    /// frames between them.
    pub last_render: Instant,
    /// Where the pointer last was ON THIS SCREEN, as the device reported
    /// it. What the event loop routes presses and menus by.
    pub mouse: (f32, f32),
    /// The same pointer as the DRAWING sees it: the toolkit's routing
    /// ([`nacelle::pointer::Pointer`]), which answers a control that has a
    /// window drawn over it that the pointer is nowhere near it.
    ///
    /// Per screen, and held between frames, because what covered the
    /// pointer is the one fact about a frame the next frame needs. It is
    /// lent to each frame's `Ctx` and taken back at the end of it, the
    /// same way [`Screen::dl`] is.
    pub pointer: nacelle::pointer::Pointer,
    /// Which placement reported a character grid on the last frame.
    pub term_inst: Option<InstanceId>,
    /// This screen's own draw list, kept between frames so a steady
    /// frame allocates nothing, together with the lane it is armed on
    /// (f3 §6 K3a).
    ///
    /// The two arrive as one object because they cannot be got right
    /// separately: a frame that empties the list without asking
    /// `render.vector` again draws this theme on the last theme's lane.
    /// `crate::vector::FrameList::begin` is the only way to the list and
    /// does both, so the arming cannot be dropped without dropping the
    /// frame with it.
    ///
    /// Beside the list rather than beside the theme, because the mode
    /// belongs to a LIST: two monitors are two lists, and each is armed
    /// where it is cleared.
    pub frame: crate::vector::FrameList,
    /// Widget padding: the content inset from the outer panel edge. The
    /// user's one setting, mirrored here because every solve this
    /// screen makes needs it — including the ones outside a frame, when
    /// the widget set is brought into line with the boards.
    pub pad: f32,
    // ---- the theme's baked decoration, for THIS surface size --------
    backdrop: Option<(ImageId, u32, u32)>,
    overlay: Option<(ImageId, u32, u32)>,
    /// (theme epoch, w, h) the bake was last kicked for.
    plate_key: Option<(u32, u32, u32)>,
    plate_rx: Option<Receiver<PlatePair>>,
    /// Glyph-atlas rows this screen's GPU copy is missing, as a row
    /// span (lo, hi exclusive). The font system is one for the whole
    /// program; each screen's renderer holds a copy of its atlas.
    atlas_behind: Option<(u32, u32)>,
    /// False until this screen has uploaded the WHOLE atlas once —
    /// glyphs rasterised before it existed were never dirty while it
    /// listened.
    atlas_synced: bool,
}

type PlatePair = (Option<nacelle::theme::Plate>, Option<nacelle::theme::Plate>);

/// The gutter of ONE screen, in the only form the layout editor takes.
///
/// A bare `f32` in that position is how the same defect came back twice.
/// There are two answers to "how wide is the gutter": the theme's length
/// at THIS screen's height — fractions and all, which is what the boards
/// are solved with ([`Screen::pad`]) — and `config::grid_prefs().3`,
/// which is whole pixels because it is the number the settings spinner
/// edits. The second is one `as f32` away from every call site, and an
/// editor given it draws its grid up to half a pixel per `u` off the
/// panels it is editing. At the shipped 9u and 1080 lines that is 49
/// against 48.6, and the screen says nothing about it: a WYSIWYG editor
/// quietly lying about where the panels are.
///
/// A test would only catch the call sites a test can reach, and the two
/// that matter are on [`Screen`], which needs a window and an event loop
/// to exist. So the wrong number is made UNSPEAKABLE instead: the field
/// is private to this module and [`Screen::gutter`] is the only thing
/// that fills it, so `grid_prefs().3 as f32` no longer type-checks
/// anywhere on the way to the editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gutter(f32);

impl Gutter {
    /// The length itself, for the editor, which has to do arithmetic
    /// with it. Reading is safe; it is MINTING one that is restricted.
    pub fn px(self) -> f32 {
        self.0
    }

    /// A stand-in for a screen, in tests that cannot have one.
    ///
    /// `Screen::new` wants a window and an event loop, so the editor's
    /// own tests reach its doors directly — and they are testing exactly
    /// that a fractional gutter survives them. Test-only by
    /// construction: nothing shipped can reach this.
    #[cfg(test)]
    pub fn of_test(px: f32) -> Gutter {
        Gutter(px)
    }
}

impl Screen {
    /// A borderless fullscreen window on one monitor, with its own
    /// renderer, its own layaut and its own widgets. A screen that
    /// cannot come up is reported and skipped — the desktop keeps its
    /// other screens.
    ///
    /// `monitor` None means "wherever the window manager puts it",
    /// which is what a guest on somebody else's desktop gets.
    pub fn new(
        el: &winit::event_loop::EventLoop<()>,
        monitor: Option<MonitorHandle>,
        connector: Option<String>,
        primary: bool,
    ) -> Option<Self> {
        let title = match (&connector, primary) {
            (_, true) => "nacelle-desktop".to_string(),
            (Some(c), false) => format!("nacelle-desktop \u{2014} {c}"),
            (None, false) => "nacelle-desktop".to_string(),
        };
        let window = WindowBuilder::new()
            .with_title(title)
            .with_decorations(false)
            .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0))
            // Fullscreen right away, like eDEX-UI.
            .with_fullscreen(Some(Fullscreen::Borderless(monitor)))
            .build(el)
            .map_err(|e| {
                eprintln!(
                    "nacelle-desktop: screen '{}' failed: {e}",
                    connector.as_deref().unwrap_or("?")
                )
            })
            .ok()?;
        // Minimum window size in landscape orientation.
        window.set_min_inner_size(Some(winit::dpi::PhysicalSize::new(1280u32, 720u32)));
        let size = window.inner_size();
        let gfx = nacelle_renderer::Gfx::new(&window, size.width, size.height);
        let key = screen_key(&window);
        // The band around this screen's panels, asked once and handed to
        // both the boards and the editor drawn over them. The editor may
        // not ask for itself: a second reading is a second answer the
        // moment the two are taken under different window heights, and
        // then the editor's grid no longer sits on the panels it edits.
        let pad = config::panel_gutter(config::grid_padding_override());
        // Asked here rather than handed in, so that a screen built by
        // any door — the desktop's survey, a guest window, a chassis
        // panel — is keyed the same way. The reading is remembered per
        // socket, so this costs one kernel file per monitor per run.
        let id = crate::screens::identify(connector.as_deref());
        let mut sc = Screen {
            connector,
            id,
            window,
            gfx,
            key,
            layaut: String::new(),
            world: BoardWorld::new(LayoutDef::default()),
            board: (0, 0),
            press_at: None,
            pan: None,
            cube: None,
            go_queue: Vec::new(),
            widgets: WidgetSet::new(),
            drag_capture: None,
            press_inst: None,
            kbd: None,
            editor: widgets::editor::Editor::new(Gutter(pad)),
            preview: (0..widgets::panel_count()).map(|_| None).collect(),
            booting: true,
            last_render: Instant::now() - std::time::Duration::from_secs(1),
            mouse: (0.0, 0.0),
            pointer: nacelle::pointer::Pointer::default(),
            term_inst: None,
            frame: crate::vector::FrameList::new(),
            pad,
            backdrop: None,
            overlay: None,
            plate_key: None,
            plate_rx: None,
            atlas_behind: None,
            atlas_synced: false,
        };
        sc.reload_layaut();
        Some(sc)
    }

    /// Re-reads the layaut this screen is assigned, rebuilds its world
    /// of boards and brings its widgets into line with it.
    ///
    /// This is the whole of "one monitor, one room": the configuration
    /// answers by the screen's IDENTITY — what the monitor says it is,
    /// falling back to the socket — and a screen nothing was written
    /// for takes the selected layaut, exactly as the one screen always
    /// did.
    pub fn reload_layaut(&mut self) {
        let (name, def) = config::screen_layaut(&self.id);
        self.layaut = name;
        self.rebuild(def);
    }

    /// Rebuilds the world from a layaut already in hand — the path a
    /// save takes, which must not re-read the file it has just written
    /// through a second door.
    pub fn rebuild(&mut self, def: LayoutDef) {
        self.go_queue.clear();
        self.world.rebuild(def);
        // A layaut with fewer boards than where the user stood: home is
        // the one place that always exists.
        if !self.world.has_board(self.board) {
            self.board = (0, 0);
        }
        self.world.set_current(self.board);
        self.sync_widgets();
    }

    /// The layaut this screen shows, whole — what a save hands back to
    /// the store.
    pub fn spec(&self) -> &LayoutDef {
        self.world.layout()
    }

    pub fn cur_def(&self) -> &LayoutDef {
        self.world.def(self.board)
    }

    pub fn has_board(&self, k: BoardId) -> bool {
        self.world.has_board(k)
    }

    /// The OUTER rectangles one board shows on THIS screen — what the
    /// layout files and the grid editor speak in.
    pub fn solve(&self, k: BoardId) -> nacelle::base::Layout {
        let (w, h) = self.size();
        self.solve_at(k, w, h)
    }

    /// The same for a size the caller names — the board thumbnails ask
    /// at sizes that are not this window's.
    pub fn solve_at(&self, k: BoardId, w: f32, h: f32) -> nacelle::base::Layout {
        self.world.solve(k, w, h, self.pad, self.key, &nacelle::base::size_table())
    }

    /// The CONTENT boxes of the board being stood on — the rects
    /// widgets draw in and clicks are answered in. The editor's
    /// rectangles while it is open, so what is edited is what is hit.
    pub fn content_layout(&self) -> nacelle::base::Layout {
        let (w, h) = self.size();
        if self.editor.active {
            self.editor.layout(w, h)
        } else {
            self.solve(self.board)
        }
        .padded(self.pad)
    }

    pub fn size(&self) -> (f32, f32) {
        let s = self.window.inner_size();
        (s.width as f32, s.height as f32)
    }

    /// Which placements the layaut makes ANYWHERE, with the widget each
    /// one runs — the presence scan widget lifetime hangs on.
    fn present(&self) -> Vec<(InstanceId, Panel)> {
        let (w, h) = self.size();
        let table: SizeTable = nacelle::base::size_table();
        let insts = &self.world.layout().instances;
        self.world
            .present(w, h, self.pad, self.key, &table)
            .into_iter()
            .filter_map(|id| insts.get(id).map(|i| (id, i.widget)))
            .collect()
    }

    /// Builds what this screen's boards place and drops what they no
    /// longer do — see [`WidgetSet`] for the invariant.
    pub fn sync_widgets(&mut self) {
        let present = self.present();
        // Everything this screen remembers by placement NUMBER — who
        // owns the keyboard, who was told the button went down — is
        // asked to survive the coming sync, BEFORE it happens, because
        // afterwards there is nothing left to compare against.
        //
        // The number alone is not the test. Two layauts are two id
        // spaces, so a sync can drop the box at a number and build
        // another widget's under the same one; a check by identity
        // would leave the newcomer holding somebody else's keyboard and
        // the release of a press it never got.
        let survives = |held: Option<InstanceId>| -> Option<InstanceId> {
            let id = held?;
            let was = self.widgets.widget_of(id)?;
            present.iter().any(|(i, w)| *i == id && *w == was).then_some(id)
        };
        let (kbd, pressed) = (survives(self.kbd), survives(self.press_inst));
        self.widgets.sync(&present, make_widget);
        self.kbd = kbd;
        self.press_inst = pressed;
    }

    /// Starts the one-neighbour move to `t` — the cube sideways, the
    /// flat slide up and down.
    pub fn step_to(&mut self, t: BoardId) {
        let horizontal = t.0 != self.board.0;
        let sign: i32 = if horizontal {
            if t.0 > self.board.0 {
                1
            } else {
                -1
            }
        } else if t.1 > self.board.1 {
            1
        } else {
            -1
        };
        let full: f32 = if horizontal { 90.0 } else { 1.0 };
        nacelle::sound::emit(nacelle::sound::Event::Snap);
        self.cube = Some(Cube {
            horizontal,
            a0: 0.0,
            a1: full * sign as f32,
            t0: Instant::now(),
            to: t,
            face_b: t,
        });
    }

    /// Every board of this screen as it would look at the given size,
    /// the one being stood on marked — what the BOARDS view draws.
    pub fn board_thumbs(&self, w: f32, h: f32) -> Vec<widgets::settings::BoardThumb> {
        let mut thumbs = Vec::new();
        for k in self.world.ids() {
            let lay = self.solve_at(k, w, h);
            let panels = lay
                .all()
                .iter()
                .filter(|p| p.rect.x < w)
                .map(|p| widgets::PanelSpec {
                    x: p.rect.x / w * 100.0,
                    y: p.rect.y / h * 100.0,
                    w: p.rect.w / w * 100.0,
                    h: p.rect.h / h * 100.0,
                })
                .collect();
            thumbs.push(widgets::settings::BoardThumb {
                id: k,
                current: k == nacelle::layout::board_key(self.board),
                panels,
            });
        }
        thumbs
    }

    /// Leaves the editor and lets its miniatures go: they are live
    /// widgets — a shell among them — and keeping them past the window
    /// that showed them would be exactly the leak `WidgetSet` exists to
    /// make impossible.
    pub fn stop_editor(&mut self) {
        self.editor.stop();
        for slot in self.preview.iter_mut() {
            *slot = None;
        }
    }

    /// The gutter this screen's boards are actually solved with, in the
    /// form the editor takes.
    ///
    /// The ONE place a [`Gutter`] comes into being: `pad` is the theme's
    /// length at this screen's height, unrounded, and the editor is
    /// drawn over boards laid out with exactly it. Everything that could
    /// answer the same question differently — the settings file's whole
    /// pixels above all — has no way to reach the editor now, because it
    /// cannot make one of these.
    pub fn gutter(&self) -> Gutter {
        Gutter(self.pad)
    }

    /// Enters the layout editor over THIS screen, with THIS screen's
    /// rectangles and this screen's pixels.
    ///
    /// Which widgets the ADD WIDGET window offers is a question about
    /// this screen's world alone: another monitor's boards are another
    /// room's furniture, and a widget standing there says nothing about
    /// what may stand here.
    pub fn enter_editor(&mut self) {
        let (w, h) = self.size();
        let (snap, cols, rows, _) = config::grid_prefs();
        // The editor edits the OUTER panel rects.
        let outer = self.solve(self.board);
        let k = nacelle::layout::board_key(self.board);
        // Which KIND of widget this board takes: an ordinary board, the
        // APPGRID below it or SEARCH AND AI above.
        let takes = match k {
            (0, y) if y < 0 => nacelle::base::WidgetCategory::SearchAi,
            (0, y) if y > 0 => nacelle::base::WidgetCategory::Appgrid,
            _ => nacelle::base::WidgetCategory::Board,
        };
        // The identity counter of the WHOLE layaut, not of this board:
        // an id this editor hands out must not collide with one another
        // board — or another screen showing the same layaut — holds.
        let next_id = self.world.layout().instances.next_free();
        self.editor
            // The gutter this screen is actually drawing with, not a
            // second reading of the file: a slider still under the hand
            // has changed the one and not yet the other.
            .start(&outer, w, h, snap, cols, rows, self.gutter(), k, takes, next_id);
    }

    /// Brings a running editor into line with grid preferences the
    /// settings window has just changed.
    ///
    /// The gutter is taken here and nowhere else, and it is
    /// [`Screen::gutter`] — the very length this screen's boards were
    /// solved with. The event loop is given no gutter to pass, on
    /// purpose: the one it had in hand was `grid_prefs`', which is whole
    /// pixels because that is what the settings spinner edits, and
    /// handing it over slid the editor's grid off the panels it is drawn
    /// over by up to half a pixel per u. The screen that draws the
    /// boards is the only owner of that number, and since the editor's
    /// doors take a [`Gutter`] rather than an `f32`, it is now the only
    /// thing that CAN own it — a comment is not a guard, and this one
    /// was ignored twice.
    pub fn sync_editor(&mut self) {
        if !self.editor.active {
            return;
        }
        let (w, h) = self.size();
        self.editor.sync_from_screen(self.gutter(), w, h);
    }

    /// This screen's layaut with the edited board folded back into it —
    /// what the store is handed to write.
    ///
    /// The editor works on ONE board and knows nothing of the rest, so
    /// the three things it changed are applied by identity: placements
    /// the user dropped are removed, placements it holds are moved onto
    /// this board with the rectangles it left them, and the identity
    /// counter is carried over so an id it handed out is never handed
    /// out twice.
    pub fn edited_spec(&self) -> LayoutDef {
        let mut def = self.world.layout().clone();
        let k = self.editor.board();
        for id in self.editor.removed_since_start() {
            def.instances.remove(id);
        }
        for inst in self.editor.instances() {
            if def.instances.get(inst.id).is_some() {
                def.instances.set_rect(inst.id, inst.rect);
                def.instances.set_board(inst.id, k);
            } else {
                // Dragged out of ADD WIDGET while the editor was open:
                // a brand-new placement with an identity of its own.
                def.instances.restore(*inst);
            }
        }
        def.instances.reserve_up_to(self.editor.next_free());
        def
    }

    /// The editor's overlay, with live miniatures in the ADD WIDGET
    /// window. The two fields are borrowed apart here on purpose: the
    /// closure the editor calls back into needs the preview cache while
    /// the editor itself is being driven.
    pub fn draw_editor(&mut self, ctx: &mut nacelle::Ctx, host: &widgets::Host) {
        let (editor, preview) = (&mut self.editor, &mut self.preview);
        editor.draw(ctx, |ctx, p, r| {
            let Some(slot) = preview.get_mut(p.idx()) else { return };
            if slot.is_none() {
                *slot = make_widget(p);
            }
            ctx.panel_scale = ctx.panel_font_scale(&r, p);
            if let Some(wg) = slot.as_mut() {
                // A live miniature with its container, exactly as it
                // will look placed; no input table is touched — these
                // rects are the ADD WIDGET window's.
                draw_panel(ctx, wg.as_mut(), r, host, p);
            }
            ctx.panel_scale = 1.0;
        });
    }

    /// Re-reads the screen key — done when it can actually change (a
    /// resize, a scale change), never per frame: it costs a round trip
    /// to the display server and a sysfs scan.
    pub fn resized(&mut self) {
        self.gfx.resize();
        self.key = screen_key(&self.window);
    }

    /// Notes glyph rows another screen's frame drained, so this one can
    /// catch its own atlas copy up before it draws text.
    pub fn note_atlas_rows(&mut self, y0: u32, rows: u32) {
        let (lo, hi) = (y0, y0 + rows);
        self.atlas_behind = Some(match self.atlas_behind {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }

    /// The backdrop plate's image, for the board ground under a turn.
    pub fn backdrop_id(&self) -> Option<ImageId> {
        self.backdrop.map(|(id, _, _)| id)
    }

    /// Kicks a rebake when the theme or the surface changed, and
    /// installs one whenever it lands. The bake is a WORKER thread's:
    /// it is milliseconds of CPU at a screen-sized image, and a frame
    /// may not wait for it.
    pub fn poll_plates(&mut self) {
        let size = self.window.inner_size();
        // `content_epoch`, not `epoch`, and this is the SECOND place that
        // distinction has cost us. `epoch` names WHICH BAKE IS PUBLISHED,
        // and a desktop whose screens differ in height alternates it every
        // frame — so this key would differ every frame, and a screen-sized
        // plate would be re-baked on a worker sixty times a second for a
        // theme that never changed. That is exactly the shape of the
        // 100 % CPU fault the font system had (`theme::content_epoch`'s
        // own doc records it). The plate depends on the theme's CONTENT
        // and on this surface's size — neither of which the publication
        // counter describes.
        let want = (nacelle::theme::content_epoch(), size.width, size.height);
        if self.plate_key != Some(want) {
            self.plate_key = Some(want);
            let (pw, ph) = (size.width, size.height);
            let (tx, rx) = std::sync::mpsc::channel();
            self.plate_rx = Some(rx);
            // The bake reads the resolved theme once at entry; a swap
            // mid-bake re-kicks on the next frame's epoch check. A stale
            // worker's send fails into a dropped receiver, silently.
            let baker = crate::threads::spawn(crate::threads::PLATE, move || {
                let _ = tx.send((
                    nacelle::theme::plate::bake_backdrop(pw, ph),
                    nacelle::theme::plate::bake_overlay(pw, ph),
                ));
            });
            if baker.is_err() {
                // No worker means no plate will ever arrive on that
                // receiver, so drop it and draw without one — the screen
                // keeps its previous plate, or none at all.
                //
                // The KEY stays claimed on purpose. Clearing it too would
                // make the next frame ask again, and the next, sixty times
                // a second: the only reasons `spawn` fails are EAGAIN and
                // ENOMEM, neither of which passes because a frame went by,
                // and each retry costs a 2 MiB stack mapping and a clone
                // before it fails. So a failed bake is spent, not retried
                // — the next theme swap or resize moves the key, and that
                // is when a machine which has since found room tries
                // again.
                self.plate_rx = None;
            }
        }
        let Some((back, over)) = self.plate_rx.as_ref().and_then(|rx| rx.try_recv().ok())
        else {
            return;
        };
        self.plate_rx = None;
        install_plate(&mut self.gfx, &mut self.backdrop, back, "backdrop");
        install_plate(&mut self.gfx, &mut self.overlay, over, "overlay");
    }

    /// The overlay plate — the LAST themed thing in a frame's list.
    pub fn overlay_id(&self) -> Option<ImageId> {
        self.overlay.map(|(id, _, _)| id)
    }

    // ---- what the renderer is told, screen by screen ----------------
    // The colour pipeline, the frost radius and the lens are the user's
    // choices about the picture, so every screen showing the picture is
    // told them; they used to reach the one renderer there was.

    pub fn set_color_depth(&mut self, bits: u32) {
        self.gfx.set_color_depth(bits);
    }

    /// The bit depth this screen's swapchain really carries — which is
    /// not always the one just asked for, because a surface answers with
    /// the formats it has. The COLOR page shows both numbers; a page
    /// that showed only the wish would say "16" over a picture nothing
    /// in the machine can render.
    pub fn color_depth(&self) -> u32 {
        self.gfx.color_depth()
    }

    pub fn set_lut(&mut self, lut: Option<(u32, Vec<f32>)>) {
        self.gfx.set_lut(lut);
    }

    pub fn set_blur_radius(&mut self, percent: u32) {
        self.gfx.set_blur_radius(percent);
    }

    pub fn set_text_gamma(&mut self, g: f32) {
        self.gfx.set_text_gamma(g);
    }

    /// Sends this screen's list to its own renderer, with whatever
    /// glyph rows its atlas copy still owes. `drained` is what the font
    /// system handed over for THIS frame; the first frame of a screen
    /// uploads the whole atlas, because glyphs rasterised before it
    /// existed were never dirty while it listened.
    pub fn present_frame(
        &mut self,
        fonts: &nacelle::font::FontSystem,
        drained: Option<(u32, u32)>,
    ) {
        if let Some((y0, rows)) = drained {
            self.note_atlas_rows(y0, rows);
        }
        let atlas = if !self.atlas_synced {
            self.atlas_synced = true;
            self.atlas_behind = None;
            Some((fonts.atlas.as_slice(), 0u32, nacelle::font::ATLAS_H as u32))
        } else {
            self.atlas_behind
                .take()
                .filter(|(lo, hi)| hi > lo)
                .map(|(lo, hi)| (fonts.atlas.as_slice(), lo, hi - lo))
        };
        let size = self.window.inner_size();
        // The swapchain clear is the absolute bed — one rung below the
        // board's own fill; the master forces its alpha to 1.0.
        let clear = nacelle::deco::clear_color();
        // The list the frame just gave back. Empty if a frame is still
        // holding it, which the one caller cannot be doing: it hands the
        // list back on the line above this call.
        let dl = self.frame.list();
        self.gfx.render(
            size.width,
            size.height,
            &dl.verts,
            &dl.runs,
            dl.shapes(),
            atlas,
            [clear.r, clear.g, clear.b, 1.0],
        );
    }
}

fn install_plate(
    gfx: &mut nacelle_renderer::Gfx,
    tex: &mut Option<(ImageId, u32, u32)>,
    baked: Option<nacelle::theme::Plate>,
    which: &str,
) {
    match baked {
        Some(p) => {
            let stale = match *tex {
                Some((_, tw, th)) => (tw, th) != (p.w, p.h),
                None => true,
            };
            if stale {
                // destroy_texture waits for the device — theme swap and
                // resize only, never a steady frame.
                if let Some((old, _, _)) = tex.take() {
                    gfx.destroy_texture(old);
                }
                *tex = Some((gfx.create_texture(p.w, p.h), p.w, p.h));
            }
            if let Some((id, _, _)) = *tex {
                gfx.update_texture(id, &p.rgba);
            }
            eprintln!(
                "nacelle-desktop: {which} plate {}x{} baked in {:.1} ms",
                p.w, p.h, p.bake_ms
            );
        }
        // Every layer off: no plate, no quad.
        None => {
            if let Some((old, _, _)) = tex.take() {
                gfx.destroy_texture(old);
            }
        }
    }
}

/// Draws one panel through the one contract (u2 §4.1): asks the widget
/// for its chrome, has the host's container drawn — material, ring,
/// title band — and hands the widget the CONTENT BOX the container
/// left. Answers that box, because it is also the rect the placement's
/// `click` and `wheel` must later receive: hit-testing against any
/// other rectangle would land clicks on chrome the widget cannot see.
pub fn draw_panel(
    ctx: &mut nacelle::Ctx,
    wg: &mut dyn widgets::Widget,
    r: Rect,
    host: &widgets::Host,
    panel: Panel,
) -> Rect {
    let chrome = wg.chrome(ctx, host);
    let content = nacelle::object::panel::draw(ctx, r, &chrome, panel.idx());
    wg.draw(ctx, content, host);
    content
}

/// Builds one widget by name. A widget is its file: the compiled
/// library wins where there is one — it exists precisely because a
/// script could not do that job — and the script otherwise. None when
/// the file fails to load: the placement takes part in the layout and
/// draws nothing.
pub fn make_widget(p: Panel) -> Option<Box<dyn widgets::Widget>> {
    config::widget_factory().make(p.name())
}

/// A window's screen key: monitor resolution + diagonal in inches.
///
/// Asked once per screen change, never per frame: it queries the
/// display server for the monitor list and then the monitor's physical
/// size, and the layout code wants it in a dozen places.
pub fn screen_key(window: &Window) -> (u32, u32, u32) {
    match window.current_monitor().or_else(|| window.primary_monitor()) {
        Some(m) => {
            let s = m.size();
            let diag = m.name().map(|n| config::monitor_diag_inches(&n)).unwrap_or(0);
            (s.width, s.height, diag)
        }
        None => (0, 0, 0),
    }
}

// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nacelle::layout::InstanceId;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A widget that counts how many of it are alive — a stand-in for
    /// the shells, threads and directory scans a real one starts.
    ///
    /// The counter is handed in rather than global, so each test owns
    /// its own and the suite may run them all at once.
    struct Counted(Arc<AtomicUsize>);

    fn counter() -> Arc<AtomicUsize> {
        Arc::new(AtomicUsize::new(0))
    }

    fn spawn(c: &Arc<AtomicUsize>) -> Option<Box<dyn widgets::Widget>> {
        c.fetch_add(1, Ordering::SeqCst);
        Some(Box::new(Counted(c.clone())))
    }

    fn alive(c: &Arc<AtomicUsize>) -> usize {
        c.load(Ordering::SeqCst)
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl widgets::Widget for Counted {
        fn draw(&mut self, _ctx: &mut nacelle::Ctx, _r: Rect, _h: &widgets::Host) {}
    }

    fn id(n: u32) -> InstanceId {
        InstanceId::new(n)
    }

    /// The invariant, stated as a test because losing it is SILENT: a
    /// widget nobody placed would simply grind away in the background,
    /// and nothing on screen would say so.
    ///
    /// Nothing exists before a board places it; a placement leaving the
    /// layaut ends its widget's work then and there.
    #[test]
    fn a_widget_on_no_board_does_not_run() {
        let c = counter();
        let mut set = WidgetSet::new();
        // An empty layaut runs nothing at all — not one instance per
        // registered widget, none.
        let (built, dropped) = set.sync(&[], |_| spawn(&c));
        assert_eq!((built, dropped), (0, 0));
        assert_eq!(alive(&c), 0, "nothing placed, nothing running");

        // A board takes the widget: NOW it comes up, and only then.
        let placed = [(id(1), Panel(0))];
        let (built, dropped) = set.sync(&placed, |_| spawn(&c));
        assert_eq!((built, dropped), (1, 0));
        assert_eq!(alive(&c), 1);

        // Syncing again with the same placement changes nothing: a
        // rebuild of the world may not restart a running shell.
        let (built, dropped) = set.sync(&placed, |_| spawn(&c));
        assert_eq!((built, dropped), (0, 0));
        assert_eq!(alive(&c), 1, "a resync is not a restart");

        // It comes off the board: its work ends with it.
        let (built, dropped) = set.sync(&[], |_| spawn(&c));
        assert_eq!((built, dropped), (0, 1));
        assert_eq!(alive(&c), 0, "off every board is off");
    }

    /// The other half, and the one that is easy to break while moving
    /// this state onto a screen: the presence scan is over EVERY board
    /// of the layaut, not the visible one. A terminal on the board next
    /// door keeps its shell while the user stands somewhere else.
    #[test]
    fn a_widget_on_another_board_keeps_running() {
        let c = counter();
        let mut set = WidgetSet::new();
        // Two placements of two different boards — which board is which
        // is the world's business; what reaches the set is the whole
        // layaut's placements at once.
        let both = [(id(1), Panel(0)), (id(2), Panel(1))];
        set.sync(&both, |_| spawn(&c));
        assert_eq!(alive(&c), 2);
        // Standing on the other board changes nothing here: nothing in
        // this type knows or cares which board is being looked at.
        set.sync(&both, |_| spawn(&c));
        assert_eq!(alive(&c), 2, "turning away is not closing");
    }

    /// Two instances of one widget are two widgets — the feature the
    /// whole instance model exists for. Removing one leaves the other
    /// running, with its own content box and its own scale.
    #[test]
    fn two_instances_of_one_widget_are_two_widgets() {
        let c = counter();
        let mut set = WidgetSet::new();
        let two = [(id(1), Panel(0)), (id(2), Panel(0))];
        set.sync(&two, |_| spawn(&c));
        assert_eq!(alive(&c), 2, "one widget, two shells");
        set.set_content(id(1), Rect::new(0.0, 0.0, 10.0, 10.0));
        set.set_content(id(2), Rect::new(50.0, 0.0, 10.0, 10.0));
        set.set_scale(id(1), 0.5);
        set.set_scale(id(2), 2.0);
        let box_of = |s: &WidgetSet, i| s.content(i).map(|r| (r.x, r.y, r.w, r.h));
        assert_ne!(box_of(&set, id(1)), box_of(&set, id(2)));
        assert_ne!(set.scale(id(1)), set.scale(id(2)));

        // The first one goes; the second keeps its identity, its box
        // and its process.
        let (_, dropped) = set.sync(&[(id(2), Panel(0))], |_| spawn(&c));
        assert_eq!(dropped, 1);
        assert_eq!(alive(&c), 1);
        assert_eq!(box_of(&set, id(2)), Some((50.0, 0.0, 10.0, 10.0)));
        assert!(set.content(id(1)).is_none());
    }

    /// A widget whose file will not load leaves a placement that draws
    /// nothing — and is still a placement, so it is not rebuilt every
    /// frame. A factory called twice for one identity would be a
    /// process started sixty times a second.
    #[test]
    fn a_placement_whose_widget_will_not_load_is_still_a_placement() {
        let mut calls = 0;
        let mut set = WidgetSet::new();
        let one = [(id(1), Panel(0))];
        set.sync(&one, |_| {
            calls += 1;
            None
        });
        set.sync(&one, |_| {
            calls += 1;
            None
        });
        assert_eq!(calls, 1, "the factory is asked once per placement");
        assert_eq!(set.len(), 1);
        assert_eq!(set.running(), 0);
        assert!(set.get_mut(id(1)).is_none());
    }

    /// The pair a screen's remembered placement is checked as — the
    /// number AND the widget standing on it.
    ///
    /// This is what makes state that outlives a frame (who owns the
    /// keyboard, who was told the button went down) survivable across a
    /// sync. The dangerous case is not the placement that leaves, which
    /// would answer nothing anyway: it is the number the next layaut
    /// hands to ANOTHER widget, which a check by identity alone would
    /// let quietly inherit both.
    #[test]
    fn a_placement_is_its_number_and_its_widget_together() {
        let c = counter();
        let mut set = WidgetSet::new();
        set.sync(&[(id(1), Panel(0)), (id(2), Panel(3))], |_| spawn(&c));
        assert_eq!(set.widget_of(id(1)), Some(Panel(0)));
        assert_eq!(set.widget_of(id(2)), Some(Panel(3)));
        assert_eq!(set.widget_of(id(9)), None, "a number nobody placed");
        // A placement that has left every board: gone, and the pair
        // says so.
        set.sync(&[(id(2), Panel(3))], |_| spawn(&c));
        assert_eq!(set.widget_of(id(1)), None);
        // The same number, another widget. The number is still in the
        // set — which is exactly why the number alone cannot be the
        // question; the pair has changed, and that is the answer.
        set.sync(&[(id(2), Panel(7))], |_| spawn(&c));
        assert_eq!(set.widget_of(id(2)), Some(Panel(7)));
    }

    /// Two layauts are two id spaces. Swapping the one a screen shows
    /// may hand placement 1 to another widget entirely, and the box
    /// that was there has to go: keeping it would leave a shell running
    /// under a name that no longer means it.
    #[test]
    fn the_same_id_for_another_widget_is_another_widget() {
        let c = counter();
        let mut set = WidgetSet::new();
        set.sync(&[(id(1), Panel(0))], |_| spawn(&c));
        set.set_content(id(1), Rect::new(0.0, 0.0, 10.0, 10.0));
        let (built, dropped) = set.sync(&[(id(1), Panel(7))], |_| spawn(&c));
        assert_eq!((built, dropped), (1, 1), "same identity, another widget");
        assert_eq!(alive(&c), 1);
        assert!(set.content(id(1)).is_none(), "and a box of its own to draw in");
    }
}
