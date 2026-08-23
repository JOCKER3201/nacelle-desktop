//! Layout editor: an Android-style snap grid over the live interface.
//! Entered from SETTINGS -> GRID -> EDIT GRID. The grid becomes visible;
//! panels can be moved by dragging, resized by dragging their edges or
//! corners, removed with the X in their top-right corner and added via
//! the ADD WIDGET button (hold an entry for the themed hold time — the
//! list hides and the widget follows the cursor until you drop it on the
//! grid). With SNAP TO GRID enabled every panel edge is aligned to the
//! grid cells — including an automatic fit of all panels when the editor
//! opens. The editor works on the OUTER panel rectangles; the widget
//! padding (SETTINGS -> GRID) insets the content inside them.
//! Bottom-right buttons: ADD WIDGET, SAVE (overwrites the currently
//! selected layout), SAVE AS (asks for a name) and CANCEL (exits
//! without saving).
//!
//! Everything here works on INSTANCES, not on widgets. A board holds a
//! list of placements ([`nacelle::layout::InstanceList`]), each with an
//! identity of its own, so the same widget may stand on the same board
//! as many times as it is dragged out: two terminals are two shells,
//! two file browsers keep two current directories. That is why ADD
//! WIDGET offers every widget the board takes at all times — there is
//! no longer such a thing as a widget that is "already used up" — and
//! why dragging, resizing and removing name an identity: removing one
//! terminal has to leave the other exactly where it stands.

use super::{Ctx, Layout, Panel, PanelSpec, Rect, WidgetCategory, OFF_SPEC};
use crate::screen::Gutter;
use nacelle::layout::{BoardId, Instance, InstanceId, InstanceList};
use nacelle::theme::{self, bake::StateStyle, parse::State, Color, TokenId};
use std::sync::OnceLock;
use std::time::Instant;

fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

/// The engine's colour in the draw list's clothes.
fn col(c: theme::ThemeColor) -> Color {
    Color { r: c.r, g: c.g, b: c.b, a: c.a }
}

// The role each of this file's runs is set in, named by the TOKEN that
// binds it and never by the role itself.
//
// A local `Role` stood here, holding four role NAMES — and the comment
// over it listed the six master bindings it was standing in for, which is
// the whole confession: `editor.proxy.label_role`, `editor.hint.role`,
// `editor.list.title_role`, `modal.title.role`, `settings.hint.role` and
// `emptystate.role` were all declared and read by nothing, so a theme
// re-roling this window moved the settings window beside it and left this
// one where it was. It also read half a ladder — the role's own `min_px`
// but not the global `type.min_px` under it, and no `max_px` ceiling at
// all — so a theme capping a role capped every line in the program except
// these five.
//
// [`nacelle::ui::bound_role`] is the toolkit's own reader, which is what
// makes "how big is this role" one question with one answer.
static ROLE_LABEL: OnceLock<TokenId> = OnceLock::new();
static ROLE_HINT: OnceLock<TokenId> = OnceLock::new();
static ROLE_MODAL_HINT: OnceLock<TokenId> = OnceLock::new();
static ROLE_LIST_TITLE: OnceLock<TokenId> = OnceLock::new();
static ROLE_MODAL_TITLE: OnceLock<TokenId> = OnceLock::new();
static ROLE_EMPTY: OnceLock<TokenId> = OnceLock::new();

/// A proxy's name tag — the little plate over a placed widget, and the
/// same plate on an ADD WIDGET miniature.
fn role_label() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_LABEL, "editor.proxy.label_role")
}

/// The one line pinned to the board's bottom-left corner.
fn role_hint() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_HINT, "editor.hint.role")
}

/// The line under the SAVE AS field. A MODAL's hint, so it takes the
/// binding the settings window's hints take and not the board's: the two
/// used to be one role by accident of both landing on `caption`.
fn role_modal_hint() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_MODAL_HINT, "settings.hint.role")
}

/// The ADD WIDGET window's own title.
fn role_list_title() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_LIST_TITLE, "editor.list.title_role")
}

/// The SAVE AS modal's title band.
fn role_modal_title() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_MODAL_TITLE, "modal.title.role")
}

/// NO WIDGETS INSTALLED FOR THIS BOARD — the line a panel with nothing
/// to show is set in, everywhere in the program.
fn role_empty() -> nacelle::ui::Role {
    nacelle::ui::bound_role(&ROLE_EMPTY, "emptystate.role")
}

/// A role's px for this frame.
///
/// `ui_font_scale` is NOT a factor: the user's interface scale is
/// `metric.ui_scale`, the viewport multiplies u by it, and every
/// `type.<role>.size` is written in u — so the baked px already grew.
/// The shrink argument is 1.0 because there is no widget stack here to
/// shrink for; `ctx.panel_scale` rides inside [`nacelle::ui::Role::px`],
/// which is exactly what this file's own reader multiplied by.
fn px_of(role: nacelle::ui::Role, ctx: &Ctx) -> f32 {
    role.px(ctx, 1.0)
}

/// Edge-grab margin (resize handles), with its device-px floor.
fn grab_edge() -> f32 {
    static E: OnceLock<TokenId> = OnceLock::new();
    static EM: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    t.px(tok(&E, "editor.edge")).max(t.px(tok(&EM, "editor.edge_min_px")))
}

/// The little name tag over a proxy's top-left corner: the plate, and
/// the label sitting on it in `ink`.
///
/// One function for both wearers — the proxies on the board and the
/// miniatures in the ADD WIDGET window — because they are the same tag,
/// and two copies of its arithmetic meant fixing its centring once
/// fixed only half the program.
fn nameplate(ctx: &mut Ctx, x: f32, y: f32, label: &str, ink: Color) {
    static FILL: OnceLock<TokenId> = OnceLock::new();
    static PAD: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    let role = role_label();
    let px = px_of(role, ctx);
    let track = role.tracking_px(px);
    // The role's own face, not this call site's guess at one.
    let face = role.font();
    let tw = ctx.fonts.measure(face, px, label, track);
    // ONE key for one plate, spent the way the master's other `pad_x`
    // keys are: ON EACH SIDE. That is what `button.pad_x` means to every
    // reader it has — `settings.rs`'s bar plates and disclosure inset
    // here, all three AI panels in nacelle-addons — and a word cannot
    // mean one thing per tag. So the plate is the text plus twice the
    // padding and the label starts one padding in — which is the same
    // statement twice, and is why a theme cannot slide the label off
    // its own plate.
    //
    // The inset used to come from `panel.title.inset_x`, a length
    // belonging to the title band of a PANEL: two families met on one
    // tag, and a theme that widened the plate slid its label off centre
    // instead of moving it with the padding. (At the master's own
    // numbers the label sat 8.6 px from the left edge and 2.9 px from
    // the right.) The master halves the key in the same batch — 1.2x of
    // the caption size became 0.6x, because the number was calibrated
    // against a reader spending it on both sides at once — so the tag is
    // drawn exactly as it is today the moment the toolkit pin carries
    // that master, and until then it wears the padding twice over.
    let pad = t.px(tok(&PAD, "editor.proxy.label_pad_x"));
    ctx.dl.rect(
        x,
        y,
        tw + 2.0 * pad,
        nameplate_h(),
        col(t.color(tok(&FILL, "component.nameplate.fill"))),
    );
    ctx.dl.text(
        ctx.fonts,
        face,
        px,
        x + pad,
        nameplate_text_y(y, px),
        label,
        ink,
        track,
    );
}

/// Height of that plate.
fn nameplate_h() -> f32 {
    static H: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().px(tok(&H, "editor.proxy.label_h"))
}

/// Where the label sits inside a plate whose top edge is at `top`.
fn nameplate_text_y(top: f32, px: f32) -> f32 {
    super::center_line_y(top, nameplate_h(), px, role_label().leading())
}

/// Where a modal's title sits in a title band starting at `top`. Upper
/// case in a band is the very case the master turned optical centring
/// on for.
fn modal_title_y(top: f32, px: f32) -> f32 {
    static BAND_H: OnceLock<TokenId> = OnceLock::new();
    let band_h = theme::resolved().px(tok(&BAND_H, "modal.title.band_h"));
    super::center_line_y(top, band_h, px, role_modal_title().leading())
}

/// How far the tear-off growth has run `elapsed` seconds in: the raw
/// 0..1 progress, which says when the animation is over, and the same
/// progress under `motion.widget_grow.easing`, which says how big the
/// widget is drawn.
///
/// Reduced motion (`motion.scale = 0`), a disabled effect and a zero
/// duration all answer (1, 1) on the first frame: a one-shot freezes at
/// the state it was travelling to, so the widget is simply placed at
/// full size. Only the ARRIVAL was ever the point of this animation.
fn grow_progress(elapsed: f32) -> (f32, f32) {
    // The effect, not a hand-copied table. What stood here compared the
    // theme's easing word against THREE cached enum indices — so
    // `sine`, `step` and `custom` all fell through to linear without a
    // word of complaint, and the cache itself was the theme-swap defect
    // `motion.rs` was written to end (an enum index only names a word
    // against the schema it was interned in).
    let e = nacelle::motion::Effect::of("widget_grow");
    // `one_shot_secs` already carries `motion.scale` and the effect's
    // own `enabled` flag, and answers 0 when either says "no animation".
    let dur = e.one_shot_secs();
    if dur <= 0.0 {
        return (1.0, 1.0);
    }
    let x = (elapsed / dur).clamp(0.0, 1.0);
    (x, e.ease(x))
}

/// How long a button stays lit after a click, in milliseconds.
///
/// Zero when the theme switches the effect off or asks for reduced
/// motion — the decay is a one-shot, and its end state is a button that
/// is not lit, so it never lights at all.
fn press_ms() -> f32 {
    nacelle::motion::Effect::of("press").one_shot_secs() * 1000.0
}

/// Hold time on an ADD WIDGET entry before placement starts.
///
/// Its own token, not `motion.hold.duration_ms`: that one exists so a
/// DESTRUCTIVE control can be held long enough to change your mind,
/// and adding a widget is not that — it is placed, moved, or removed
/// again in a second. Sharing the token meant one of the two was
/// always wrong, and the destructive one is the one you cannot undo.
fn hold_secs() -> f32 {
    static D: OnceLock<TokenId> = OnceLock::new();
    (theme::resolved().px(tok(&D, "editor.add_hold_ms")) / 1000.0).max(0.0)
}

/// How far the pointer may wander off a held ADD WIDGET entry before
/// the hold is abandoned, with its device-px floor. Holding a control
/// for a second is a still gesture, not a motionless one.
fn hold_slack() -> f32 {
    static S: OnceLock<TokenId> = OnceLock::new();
    static SM: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    t.px(tok(&S, "editor.list.hold_slack"))
        .max(t.px(tok(&SM, "editor.list.hold_slack_min_px")))
}

/// Padding inside the ADD WIDGET window, with its device-px floor.
fn list_pad() -> f32 {
    static P: OnceLock<TokenId> = OnceLock::new();
    static PM: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    t.px(tok(&P, "editor.list.pad")).max(t.px(tok(&PM, "editor.list.pad_min_px")))
}

/// A proxy's (and a list entry's) ring width, resting or under the pointer.
fn proxy_border(hot: bool) -> f32 {
    static B: OnceLock<TokenId> = OnceLock::new();
    static BH: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    if hot {
        t.px(tok(&BH, "editor.proxy.border_hot"))
    } else {
        t.px(tok(&B, "editor.proxy.border"))
    }
}

/// What a mouse press on the editor resolved to.
pub enum EditorHit {
    /// Handled internally (drag started, widget list, empty space).
    Handled,
    /// SETTINGS — show/hide the settings window over the editor.
    Settings,
    /// SAVE — overwrite the currently selected layout.
    Save,
    /// SAVE AS — open the name prompt.
    SaveAs,
    /// EXIT — leave the editor without saving.
    Exit,
}

/// Cursor shape the editor wants at a given position.
#[derive(Clone, Copy, PartialEq)]
pub enum CursorKind {
    Normal,
    Move,
    Ew,
    Ns,
    Nwse,
    Nesw,
}

/// Active drag: moving the instance or resizing by its edges.
#[derive(Clone, Copy)]
enum Mode {
    Move { dx: f32, dy: f32 },
    Resize { l: bool, r: bool, t: bool, b: bool },
}

pub struct Editor {
    pub active: bool,
    pub snap: bool,
    pub cols: u32,
    pub rows: u32,
    /// Widget padding: the outer rect is always this much larger than
    /// the inner content container on every side.
    padding: f32,
    /// Which board is being edited — the board every instance placed
    /// here belongs to, and the one the caller writes back.
    board: BoardId,
    /// Which kind of widget this board takes. The board decides, not
    /// the editor: an ordinary board, the APPGRID fixture and the
    /// SEARCH AND AI fixture each hold their own kind.
    takes: WidgetCategory,
    /// The instances standing on that board, with the rectangles the
    /// user is dragging around, in percent of the window. The list owns
    /// the identities: a widget pulled out of ADD WIDGET is a brand-new
    /// one, and no id is ever handed out twice.
    list: InstanceList,
    /// The list as it was when the editor opened — SAVE stores only
    /// what differs from this, and CANCEL puts it back.
    initial: Vec<Instance>,
    drag: Option<(InstanceId, Mode)>,
    /// SAVE AS name prompt; Some = the prompt is open. The field is the
    /// F1 §3 input object — caret, selection, undo and IME land here
    /// through the model, not through bespoke prompt code.
    pub naming: Option<nacelle::object::text_input::InputModel>,
    /// Where the prompt's field sat last frame — the immediate-mode
    /// anchor for click-to-caret (a click is applied on the next draw,
    /// when the fonts are at hand to hit-test with), and the box the
    /// field's context menu anchors to (right-click hit, Shift+F10).
    pub naming_field: Option<Rect>,
    /// A pending click inside the prompt, in window px.
    naming_click: Option<(f32, f32)>,
    /// The field's caret box last frame, in window px — what the
    /// application anchors the IME candidate window to.
    pub naming_caret: Option<Rect>,
    /// ADD WIDGET list window.
    add_open: bool,
    /// Held list entry: (widget, hold start). A KIND, because the list
    /// offers kinds — the instance does not exist until the hold ends.
    adding: Option<(Panel, Instant)>,
    /// Pull-out animation after a completed hold: the instance grows
    /// from its miniature size to the placement size under the cursor.
    grow: Option<(InstanceId, Instant, f32, f32)>,
    flash: Option<(usize, Instant)>,
}

fn pct(r: Rect, w: f32, h: f32) -> PanelSpec {
    PanelSpec {
        x: r.x / w * 100.0,
        y: r.y / h * 100.0,
        w: r.w / w * 100.0,
        h: r.h / h * 100.0,
    }
}

/// Whether two rectangles are the same one, to the precision a layaut
/// file writes. Only the tests weigh placements against each other now —
/// a SAVE writes them all, so nothing in the editor asks "did this one
/// move".
#[cfg(test)]
fn same_spec(a: &PanelSpec, b: &PanelSpec) -> bool {
    (a.x - b.x).abs() < 0.05
        && (a.y - b.y).abs() < 0.05
        && (a.w - b.w).abs() < 0.05
        && (a.h - b.h).abs() < 0.05
}

/// Puts one instance into a list exactly as the layout holds it.
///
/// `restore` refuses a COMPOSED identity on purpose — a file may not
/// name one — but the editor is handed whatever the board is showing,
/// and a board still arranged from the registry shows composed ones.
/// They go back in through the door that mints them, so that editing
/// such a board keeps the identities it was drawn with until the save
/// turns them into the user's own ([`nacelle::layout::LayoutDef::
/// materialize`]).
fn seed(list: &mut InstanceList, inst: Instance) {
    if inst.id.is_generated() {
        list.add_generated(inst.widget, inst.board, inst.id.get() - InstanceId::GENERATED);
        list.set_rect(inst.id, inst.rect);
    } else {
        list.restore(inst);
    }
}

impl Editor {
    /// `padding` is the gutter of the screen this editor belongs to.
    ///
    /// Handed in rather than asked for. The gutter is a `u`, so the
    /// answer depends on the height of the window it is asked under, and
    /// an editor asking for itself would be answered under whatever
    /// screen last drew — the neighbour's, at boot the engine's own
    /// 1080-line default. One screen, one length, and its owner is the
    /// screen. `start` refreshes it from the same place.
    pub fn new(padding: Gutter) -> Self {
        Editor {
            active: false,
            snap: false,
            cols: crate::config::GRID_MIN,
            rows: crate::config::GRID_MIN,
            padding: padding.px(),
            board: (0, 0),
            takes: WidgetCategory::default(),
            list: InstanceList::new(),
            initial: Vec::new(),
            drag: None,
            naming: None,
            naming_field: None,
            naming_click: None,
            naming_caret: None,
            add_open: false,
            adding: None,
            grow: None,
            flash: None,
        }
    }

    /// Enters edit mode with the CURRENT instance rectangles (WYSIWYG).
    /// With snapping enabled all of them are fitted to the grid at once.
    ///
    /// `board` and `takes` say which board is on the table and which
    /// widgets it accepts; `next_id` is the identity counter of the
    /// layout the board belongs to ([`InstanceList::next_free`]), so
    /// that a widget dragged out here cannot be given an id some other
    /// board is already using.
    ///
    /// `padding` is the calling screen's gutter, stored as given —
    /// see [`Editor::sync_from_screen`] for why it is neither rounded
    /// nor floored on the way in.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        layout: &Layout,
        w: f32,
        h: f32,
        snap: bool,
        cols: u32,
        rows: u32,
        padding: Gutter,
        board: BoardId,
        takes: WidgetCategory,
        next_id: u32,
    ) {
        self.active = true;
        self.board = board;
        self.takes = takes;
        self.snap = snap;
        self.cols = cols.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.rows = rows.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.padding = padding.px();
        self.close_naming();
        self.drag = None;
        self.add_open = false;
        self.adding = None;
        self.grow = None;
        self.list = InstanceList::new();
        self.list.reserve_up_to(next_id);
        // A panel parked OUTSIDE the window (OFF_SPEC, x >= w) used to be
        // SKIPPED here — which hid it from the editor while `edited_spec`
        // (screen.rs) kept re-writing it off-screen on every SAVE. The panel
        // was then lost for good: invisible, unreachable, re-parked forever
        // (audyt layoutu 2026-08-19, klasa ① — network/sysinfo na x=200%).
        // Bring each orphan back ONTO the screen in a small cascade instead,
        // so it is seen and can be placed, and SAVE writes it a real
        // rectangle. No panel leaves the editor at OFF_SPEC.
        let mut recovered = 0.0f32;
        for p in layout.iter() {
            let spec = if p.rect.x >= w {
                let s = PanelSpec {
                    x: 2.0 + recovered * 1.5,
                    y: (2.0 + recovered * 5.0).min(74.0),
                    w: 18.0,
                    h: 24.0,
                };
                recovered += 1.0;
                s
            } else {
                pct(p.rect, w, h)
            };
            seed(
                &mut self.list,
                Instance { id: p.id, widget: p.widget, board, rect: Some(spec) },
            );
        }
        if self.snap {
            self.snap_all(w, h);
        }
        self.initial = self.list.all().to_vec();
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.close_naming();
        self.drag = None;
        self.add_open = false;
        self.adding = None;
        self.grow = None;
    }

    /// The characters a layout name may hold — what `.layaut` filenames
    /// are made of. Uppercase input is lowercased BEFORE the model sees
    /// it (main.rs), so the validator only meets the canonical form.
    pub fn layaut_name_char(c: char) -> bool {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
    }

    /// The SAVE AS field's stable focus path — one string, two users:
    /// [`Editor::begin_naming`] focuses it, the draw pass registers it.
    fn naming_focus_id() -> nacelle::focus::FocusId {
        nacelle::focus::FocusId::of("editor.field.save_as")
    }

    /// Opens the SAVE AS prompt with an empty, validated field. The
    /// prompt is a modal grab, so opening it IS the focus change: the
    /// field takes focus here, and the chain drops it by itself once
    /// the closed prompt stops registering (the vanish rule). Pointer
    /// flavour — the buttons that open it are pointer-only in F1, and
    /// a fresh text field shows a caret, not a ring.
    pub fn begin_naming(&mut self, focus: &mut nacelle::focus::FocusCtl) {
        use nacelle::object::text_input::{InputModel, Validator};
        self.naming = Some(
            InputModel::new()
                .with_validator(Validator::Charset(Self::layaut_name_char))
                .with_max_len(40),
        );
        self.naming_field = None;
        self.naming_click = None;
        self.naming_caret = None;
        focus.focus(Some(Self::naming_focus_id()));
    }

    /// Closes it, dropping the frame-to-frame view bookkeeping with it.
    pub fn close_naming(&mut self) {
        self.naming = None;
        self.naming_field = None;
        self.naming_click = None;
        self.naming_caret = None;
    }

    // ---- the edited board ------------------------------------------------

    /// Every instance on the board, in placement order — the last one
    /// is the topmost. The identity of a freshly dragged-out widget is
    /// in here too, which is what the caller adds to its layout before
    /// asking the store to write the board.
    pub fn instances(&self) -> &[Instance] {
        self.list.all()
    }

    /// The board's placements as the store writes them.
    pub fn rects(&self) -> Vec<(InstanceId, PanelSpec)> {
        self.list
            .all()
            .iter()
            .map(|i| (i.id, i.rect.unwrap_or(OFF_SPEC)))
            .collect()
    }

    /// The identity counter to carry back into the layout, so that an
    /// id this editor handed out is never handed out again.
    pub fn next_free(&self) -> u32 {
        self.list.next_free()
    }

    /// Which board this editor is holding.
    pub fn board(&self) -> BoardId {
        self.board
    }

    /// The rectangle of one instance in window pixels.
    fn px_of(inst: &Instance, w: f32, h: f32) -> Rect {
        let p = inst.rect.unwrap_or(OFF_SPEC);
        Rect::new(p.x / 100.0 * w, p.y / 100.0 * h, p.w / 100.0 * w, p.h / 100.0 * h)
    }

    fn px_rect(&self, id: InstanceId, w: f32, h: f32) -> Option<Rect> {
        self.list.get(id).map(|i| Self::px_of(i, w, h))
    }

    /// The edited board in window pixels (drawn instead of the normal
    /// one).
    pub fn layout(&self, w: f32, h: f32) -> Layout {
        let mut l = Layout::empty(w, h);
        for i in self.list.all() {
            l.place(i.id, i.widget, Self::px_of(i, w, h));
        }
        l
    }

    /// Instances the user took off the board since the editor opened.
    ///
    /// A removal is not a rectangle, so it cannot travel in
    /// [`Editor::changes_since_start`]: the caller drops these from the
    /// layout's own list, and the board simply stops holding them.
    pub fn removed_since_start(&self) -> Vec<InstanceId> {
        self.initial
            .iter()
            .map(|i| i.id)
            .filter(|id| self.list.get(*id).is_none())
            .collect()
    }

    /// Puts the board back exactly as it was found. The identities of
    /// the abandoned widgets stay retired — an id is handed out once
    /// and never again, whatever becomes of the placement that had it.
    fn restore_initial(&mut self) {
        let counter = self.list.next_free();
        self.list = InstanceList::new();
        for inst in self.initial.clone() {
            seed(&mut self.list, inst);
        }
        self.list.reserve_up_to(counter);
    }

    /// Fits every instance to the grid: each edge lands on the nearest
    /// cell boundary.
    fn snap_all(&mut self, w: f32, h: f32) {
        let cw = w / self.cols as f32;
        let ch = h / self.rows as f32;
        let ids: Vec<InstanceId> = self.list.all().iter().map(|i| i.id).collect();
        for id in ids {
            let Some(r) = self.px_rect(id, w, h) else { continue };
            let c0 = (r.x / cw).round().clamp(0.0, self.cols as f32 - 1.0);
            let c1 = ((r.right()) / cw).round().clamp(c0 + 1.0, self.cols as f32);
            let r0 = (r.y / ch).round().clamp(0.0, self.rows as f32 - 1.0);
            let r1 = ((r.bottom()) / ch).round().clamp(r0 + 1.0, self.rows as f32);
            let snapped = Rect::new(c0 * cw, r0 * ch, (c1 - c0) * cw, (r1 - r0) * ch);
            self.list.set_rect(id, Some(pct(snapped, w, h)));
        }
    }

    fn save_buttons(w: f32, h: f32) -> [Rect; 6] {
        static BW_FRAC: OnceLock<TokenId> = OnceLock::new();
        static BW_MIN: OnceLock<TokenId> = OnceLock::new();
        static BH: OnceLock<TokenId> = OnceLock::new();
        static GAP: OnceLock<TokenId> = OnceLock::new();
        static IN_X: OnceLock<TokenId> = OnceLock::new();
        static IN_Y: OnceLock<TokenId> = OnceLock::new();
        let t = theme::resolved();
        let bw = (w * t.px(tok(&BW_FRAC, "editor.button.w_frac")))
            .max(t.px(tok(&BW_MIN, "editor.button.w_min_px")));
        let bh = t.px(tok(&BH, "button.h"));
        let gap = t.px(tok(&GAP, "space.5"));
        let x = w - bw - t.px(tok(&IN_X, "editor.button.inset_x"));
        let y1 = h - 6.0 * bh - 5.0 * gap - t.px(tok(&IN_Y, "editor.button.inset_y"));
        std::array::from_fn(|i| Rect::new(x, y1 + i as f32 * (bh + gap), bw, bh))
    }

    /// Applies grid preferences changed in the settings window while the
    /// editor is running, under the gutter of the SCREEN the editor is
    /// drawn over — [`crate::screen::Screen::sync_editor`] is the only
    /// caller, and `padding` is that screen's `pad`.
    ///
    /// The preferences file is read HERE and its fourth field — the
    /// gutter — is dropped on the floor, so that no caller is left
    /// holding a second, different answer to the same question. That
    /// field is whole pixels, because it is the number the settings
    /// spinner edits; the boards under the editor are solved with the
    /// unrounded length the theme gives at this screen's height. At the
    /// shipped 9u and 1080 lines the two are 48.6 and 49, and an editor
    /// told 49 draws its grid half a pixel per u off the panels it is
    /// editing — a WYSIWYG editor quietly lying about where the panels
    /// are.
    pub fn sync_from_screen(&mut self, padding: Gutter, w: f32, h: f32) {
        let (snap, cols, rows, _) = crate::config::grid_prefs();
        self.sync_prefs(snap, cols, rows, padding, w, h);
    }

    /// The preferences, applied. Private: the gutter must come from the
    /// screen, so [`Editor::sync_from_screen`] is the door.
    ///
    /// `padding` is stored as it arrives — not rounded, and with no
    /// floor of its own. The boards are solved with that same number
    /// unfloored (`Screen::pad`), and an editor that clamped what the
    /// board does not would disagree with the picture underneath it
    /// exactly when the theme wandered off the beaten path, which is
    /// the one time nobody would think to look here.
    fn sync_prefs(&mut self, snap: bool, cols: u32, rows: u32, padding: Gutter, w: f32, h: f32) {
        let was = self.snap;
        self.cols = cols.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.rows = rows.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.padding = padding.px();
        self.snap = snap;
        if snap && !was {
            self.snap_all(w, h);
        }
    }

    /// What the ADD WIDGET window offers: every installed widget the
    /// board takes, in registry order, always.
    ///
    /// Never "the ones not placed yet". A widget may stand on this
    /// board twice, or here and on the board next door, so how many
    /// instances of it exist says nothing about whether the user may
    /// want one more — and the list that answered otherwise was the
    /// thing that made a second terminal impossible to ask for.
    fn offer(&self) -> Vec<Panel> {
        Panel::all()
            .into_iter()
            .filter(|p| p.category() == self.takes)
            .collect()
    }

    /// ADD WIDGET window rect and its item rects (widget miniatures).
    fn add_list_rects(&self, w: f32, h: f32) -> (Rect, Vec<Rect>) {
        static W_FRAC: OnceLock<TokenId> = OnceLock::new();
        static W_MIN: OnceLock<TokenId> = OnceLock::new();
        static HEAD_H: OnceLock<TokenId> = OnceLock::new();
        static ITEM_H: OnceLock<TokenId> = OnceLock::new();
        static ITEM_MIN: OnceLock<TokenId> = OnceLock::new();
        static MAX_H: OnceLock<TokenId> = OnceLock::new();
        let t = theme::resolved();
        let items = self.offer().len().max(1);
        let bw = (w * t.px(tok(&W_FRAC, "editor.list.w_frac")))
            .max(t.px(tok(&W_MIN, "editor.list.w_min_px")));
        let pad = list_pad();
        let title_h = t.px(tok(&HEAD_H, "editor.list.head_h"));
        // Miniature height: 16:9-ish, shrunk so everything fits on screen.
        let ih = (t.px(tok(&ITEM_H, "editor.list.item_h")))
            .max(t.px(tok(&ITEM_MIN, "editor.list.item_h_min_px")))
            .min((h * t.px(tok(&MAX_H, "editor.list.max_h_frac")) - title_h - pad) / items as f32 - pad);
        let bh = title_h + items as f32 * (ih + pad) + pad * 2.0;
        let bx = (w - bw) / 2.0;
        let by = (h - bh) / 2.0;
        let list = (0..items)
            .map(|i| {
                Rect::new(
                    bx + pad,
                    by + title_h + pad + i as f32 * (ih + pad),
                    bw - 2.0 * pad,
                    ih,
                )
            })
            .collect();
        (Rect::new(bx, by, bw, bh), list)
    }

    /// The X (remove) button rect of a panel. The proportional caps only
    /// guard a proxy dragged small enough to be swallowed by its own X.
    fn x_rect(r: &Rect) -> Rect {
        static S: OnceLock<TokenId> = OnceLock::new();
        static SM: OnceLock<TokenId> = OnceLock::new();
        static INSET: OnceLock<TokenId> = OnceLock::new();
        let t = theme::resolved();
        static CLW: OnceLock<TokenId> = OnceLock::new();
        static CLH: OnceLock<TokenId> = OnceLock::new();
        let s = t
            .px(tok(&S, "editor.proxy.close_size"))
            .max(t.px(tok(&SM, "editor.proxy.close_size_min_px")))
            .min(r.w * t.px(tok(&CLW, "editor.proxy.close_max_w_frac")).clamp(0.0, 1.0))
            .min(r.h * t.px(tok(&CLH, "editor.proxy.close_max_h_frac")).clamp(0.0, 1.0));
        let inset = t.px(tok(&INSET, "editor.proxy.close_inset"));
        Rect::new(r.right() - s - inset, r.y + inset, s, s)
    }

    /// Whether this instance can be taken off the board.
    ///
    /// A widget that declared itself ESSENTIAL is somebody's only way
    /// back — the way into the settings is one of its buttons — so the
    /// board keeps one of it: its LAST instance never gets an X. A
    /// second copy is an ordinary placement, and removing it leaves the
    /// way back exactly where it was. Which widget declares this is the
    /// widget's own business, so an installation whose way back lives
    /// somewhere else is protected just the same.
    fn removable(&self, id: InstanceId) -> bool {
        match self.list.get(id) {
            Some(i) => !i.widget.essential() || self.list.count_of(i.widget) > 1,
            None => false,
        }
    }

    /// Topmost instance whose body or edge area contains the point,
    /// with the edge flags: (identity, left, right, top, bottom).
    fn instance_at(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> Option<(InstanceId, bool, bool, bool, bool)> {
        let edge = grab_edge();
        for inst in self.list.all().iter().rev() {
            let r = Self::px_of(inst, w, h);
            let outer = Rect::new(r.x - edge, r.y - edge, r.w + 2.0 * edge, r.h + 2.0 * edge);
            if !outer.contains(x, y) {
                continue;
            }
            let l = (x - r.x).abs() <= edge;
            let rr = (x - r.right()).abs() <= edge;
            let t = (y - r.y).abs() <= edge;
            let b = (y - r.bottom()).abs() <= edge;
            if l || rr || t || b || r.contains(x, y) {
                return Some((inst.id, l, rr, t, b));
            }
        }
        None
    }

    /// True when the point is over the editor's own controls (buttons,
    /// the ADD WIDGET window or the name prompt) — nothing underneath
    /// may react or highlight then.
    fn over_ui(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if self.naming.is_some() || self.add_open {
            return true;
        }
        Self::save_buttons(w, h).iter().any(|b| b.contains(x, y))
    }

    /// Cursor shape for the given position (resize arrows on the edges).
    pub fn cursor_at(&self, x: f32, y: f32, w: f32, h: f32) -> CursorKind {
        if let Some((id, mode)) = &self.drag {
            if self.list.get(*id).is_none() {
                return CursorKind::Normal;
            }
            return match mode {
                Mode::Move { .. } => CursorKind::Move,
                Mode::Resize { l, r, t, b } => edge_cursor(*l, *r, *t, *b),
            };
        }
        if self.over_ui(x, y, w, h) {
            return CursorKind::Normal;
        }
        match self.instance_at(x, y, w, h) {
            Some((id, l, r, t, b)) => {
                let Some(pr) = self.px_rect(id, w, h) else { return CursorKind::Normal };
                if self.removable(id) && Self::x_rect(&pr).contains(x, y) {
                    CursorKind::Normal
                } else if l || r || t || b {
                    edge_cursor(l, r, t, b)
                } else {
                    CursorKind::Move
                }
            }
            None => CursorKind::Normal,
        }
    }

    /// Hit-test of the editor buttons only — also used while the
    /// settings window is open over the editor (the buttons share its
    /// plane and stay clickable).
    pub fn buttons_hit(&mut self, x: f32, y: f32, w: f32, h: f32) -> Option<EditorHit> {
        let btns = Self::save_buttons(w, h);
        if btns[0].contains(x, y) {
            // SETTINGS — show/hide the window over the editor.
            self.flash = Some((0, Instant::now()));
            return Some(EditorHit::Settings);
        }
        if btns[1].contains(x, y) {
            // ADD WIDGET — toggle the list window (handled internally).
            self.flash = Some((1, Instant::now()));
            self.add_open = true;
            return Some(EditorHit::Handled);
        }
        if btns[2].contains(x, y) {
            self.flash = Some((2, Instant::now()));
            return Some(EditorHit::Save);
        }
        if btns[3].contains(x, y) {
            self.flash = Some((3, Instant::now()));
            return Some(EditorHit::SaveAs);
        }
        if btns[4].contains(x, y) {
            // CANCEL — revert the unsaved changes, stay in the editor.
            self.flash = Some((4, Instant::now()));
            self.restore_initial();
            self.drag = None;
            self.grow = None;
            return Some(EditorHit::Handled);
        }
        if btns[5].contains(x, y) {
            self.flash = Some((5, Instant::now()));
            return Some(EditorHit::Exit);
        }
        None
    }

    /// Mouse press. Only meaningful while active.
    pub fn mouse_down(&mut self, x: f32, y: f32, w: f32, h: f32) -> EditorHit {
        if self.naming.is_some() {
            // The prompt is a grab. A click inside the field is queued
            // and applied on the next draw — the immediate-mode idiom:
            // only the draw pass has the fonts to hit-test text with.
            if self.naming_field.map_or(false, |r| r.contains(x, y)) {
                self.naming_click = Some((x, y));
            }
            return EditorHit::Handled;
        }
        if self.add_open {
            // Hold an entry to start placing it; any other click closes.
            let (_, items) = self.add_list_rects(w, h);
            let offer = self.offer();
            for (slot, ir) in items.iter().enumerate() {
                if ir.contains(x, y) {
                    if let Some(&widget) = offer.get(slot) {
                        self.adding = Some((widget, Instant::now()));
                    }
                    return EditorHit::Handled;
                }
            }
            self.add_open = false;
            return EditorHit::Handled;
        }
        if let Some(hit) = self.buttons_hit(x, y, w, h) {
            return hit;
        }
        if let Some((id, l, rr, t, b)) = self.instance_at(x, y, w, h) {
            let Some(r) = self.px_rect(id, w, h) else { return EditorHit::Handled };
            // X in the top-right corner takes THIS instance off the
            // board; every other one keeps its identity and its place.
            if self.removable(id) && Self::x_rect(&r).contains(x, y) {
                self.list.remove(id);
                self.drag = None;
                self.grow = None;
                nacelle::sound::emit(nacelle::sound::Event::Drop);
                return EditorHit::Handled;
            }
            if l || rr || t || b {
                self.drag = Some((id, Mode::Resize { l, r: rr, t, b }));
            } else {
                self.drag = Some((id, Mode::Move { dx: x - r.x, dy: y - r.y }));
            }
            nacelle::sound::emit(nacelle::sound::Event::Grab);
        }
        EditorHit::Handled
    }

    /// Mouse move while an instance is being dragged or resized.
    pub fn mouse_move(&mut self, x: f32, y: f32, w: f32, h: f32) {
        // Wandering far away from the held ADD WIDGET entry cancels the
        // hold (a generous margin — small drift while holding is fine).
        if let Some((widget, _)) = self.adding {
            let (_, items) = self.add_list_rects(w, h);
            let still = self
                .offer()
                .iter()
                .position(|&p| p == widget)
                .and_then(|slot| items.get(slot))
                .map(|ir| {
                    let m = hold_slack();
                    Rect::new(ir.x - m, ir.y - m, ir.w + 2.0 * m, ir.h + 2.0 * m).contains(x, y)
                })
                .unwrap_or(false);
            if !still {
                self.adding = None;
            }
        }
        let Some((id, mode)) = self.drag else { return };
        let Some(r) = self.px_rect(id, w, h) else {
            // The instance went away under the drag (CANCEL, a removal);
            // there is nothing left to move.
            self.drag = None;
            return;
        };
        let cw = w / self.cols as f32;
        let ch = h / self.rows as f32;
        let moved = match mode {
            Mode::Move { dx, dy } => {
                let mut nx = (x - dx).clamp(0.0, (w - r.w).max(0.0));
                let mut ny = (y - dy).clamp(0.0, (h - r.h).max(0.0));
                if self.snap {
                    // The panel's corner sticks to the nearest cell boundary.
                    nx = (nx / cw).round() * cw;
                    ny = (ny / ch).round() * ch;
                    nx = nx.clamp(0.0, (w - r.w).max(0.0));
                    ny = ny.clamp(0.0, (h - r.h).max(0.0));
                }
                Rect::new(nx, ny, r.w, r.h)
            }
            Mode::Resize { l, r: rr, t, b } => {
                let (mut x0, mut x1) = (r.x, r.right());
                let (mut y0, mut y1) = (r.y, r.bottom());
                let m = self.min_outer();
                let min_w = if self.snap { cw.max(m) } else { m };
                let min_h = if self.snap { ch.max(m) } else { m };
                // In a tiny window (or a dense grid) the minimum size can
                // exceed the space available on the opposite side, which
                // would make the clamp bounds cross (lo > hi) and panic;
                // oclamp orders them so it never does.
                let oclamp = |v: f32, lo: f32, hi: f32| {
                    if hi < lo { lo } else { v.clamp(lo, hi) }
                };
                if l {
                    x0 = oclamp(x, 0.0, x1 - min_w);
                    if self.snap {
                        x0 = oclamp((x0 / cw).round() * cw, 0.0, x1 - min_w);
                    }
                }
                if rr {
                    x1 = oclamp(x, x0 + min_w, w);
                    if self.snap {
                        x1 = oclamp((x1 / cw).round() * cw, x0 + min_w, w);
                    }
                }
                if t {
                    y0 = oclamp(y, 0.0, y1 - min_h);
                    if self.snap {
                        y0 = oclamp((y0 / ch).round() * ch, 0.0, y1 - min_h);
                    }
                }
                if b {
                    y1 = oclamp(y, y0 + min_h, h);
                    if self.snap {
                        y1 = oclamp((y1 / ch).round() * ch, y0 + min_h, h);
                    }
                }
                Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
            }
        };
        self.list.set_rect(id, Some(pct(moved, w, h)));
    }

    pub fn mouse_up(&mut self, w: f32, h: f32) {
        if self.drag.is_some() {
            // Snapping makes the release land on the grid, so it gets
            // the sharper confirmation of the two.
            nacelle::sound::emit(if self.snap {
                nacelle::sound::Event::Snap
            } else {
                nacelle::sound::Event::Drop
            });
        }
        self.drag = None;
        self.adding = None;
        // Releasing mid-animation finishes the growth instantly.
        if let Some((id, _, _, _)) = self.grow.take() {
            if let Some(r) = self.px_rect(id, w, h) {
                let (tw, th) = self.spawn_size(w, h);
                let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                let nr = Rect::new(
                    (cx - tw / 2.0).clamp(0.0, (w - tw).max(0.0)),
                    (cy - th / 2.0).clamp(0.0, (h - th).max(0.0)),
                    tw,
                    th,
                );
                self.list.set_rect(id, Some(pct(nr, w, h)));
            }
        }
    }

    /// Opaque parallelogram button (nacelle::object).
    fn draw_button(ctx: &mut Ctx, br: &Rect, label: &str, hover: bool, flash: bool) {
        nacelle::object::button::draw(
            ctx,
            *br,
            label,
            nacelle::object::button::ButtonState { hover, flash, selected: false },
        );
    }

    /// The smallest allowed OUTER panel size: padding on both sides
    /// plus the minimum content.
    fn min_outer(&self) -> f32 {
        static M: OnceLock<TokenId> = OnceLock::new();
        static MM: OnceLock<TokenId> = OnceLock::new();
        let t = theme::resolved();
        2.0 * self.padding
            + t.px(tok(&M, "editor.min_content")).max(t.px(tok(&MM, "editor.min_content_min_px")))
    }

    /// How big a widget is when it first lands.
    ///
    /// One intent, expressed once: a fresh widget takes a share of the
    /// screen. Snapping does not get a size of its own — it only
    /// QUANTIZES that share up to whole cells, so turning the grid on
    /// no longer shrinks a new widget to a few cells (three by two,
    /// which on a fine grid arrived the size of an icon) and no longer
    /// hides two more numbers from the theme.
    fn spawn_size(&self, w: f32, h: f32) -> (f32, f32) {
        static SW: OnceLock<TokenId> = OnceLock::new();
        static SH: OnceLock<TokenId> = OnceLock::new();
        let t = nacelle::theme::resolved();
        let m = self.min_outer();
        let want_w = w * t.px(tok(&SW, "editor.spawn_w_frac")).clamp(0.0, 1.0);
        let want_h = h * t.px(tok(&SH, "editor.spawn_h_frac")).clamp(0.0, 1.0);
        if self.snap {
            let (cw, ch) = (w / self.cols as f32, h / self.rows as f32);
            let cols = (want_w / cw).ceil().max(1.0);
            let rows = (want_h / ch).ceil().max(1.0);
            ((cols * cw).max(m), (rows * ch).max(m))
        } else {
            (want_w.max(m), want_h.max(m))
        }
    }

    /// Pulls a widget out of the ADD WIDGET window: a BRAND-NEW
    /// instance at the miniature's size, centred on the cursor and
    /// already being dragged.
    ///
    /// New every time, which is the whole feature: holding the same
    /// entry twice puts two independent widgets on the board — two
    /// shells, two current directories — instead of picking the one
    /// that is already there up again.
    fn pull_out(
        &mut self,
        widget: Panel,
        mini: (f32, f32),
        at: (f32, f32),
        w: f32,
        h: f32,
    ) -> InstanceId {
        let ((mw, mh), (mx, my)) = (mini, at);
        let r = Rect::new(
            (mx - mw / 2.0).clamp(0.0, (w - mw).max(0.0)),
            (my - mh / 2.0).clamp(0.0, (h - mh).max(0.0)),
            mw,
            mh,
        );
        let id = self.list.add(widget, self.board, Some(pct(r, w, h)));
        self.drag = Some((id, Mode::Move { dx: mw / 2.0, dy: mh / 2.0 }));
        self.grow = Some((id, Instant::now(), mw, mh));
        id
    }

    /// Draws just the editor's button stack — called from draw() and
    /// again ON TOP of the settings window when it is open over the
    /// editor, so the buttons share the window's plane.
    pub fn draw_buttons(&mut self, ctx: &mut Ctx) {
        let (w, h) = (ctx.w, ctx.h);
        // The pointer AS THIS PLANE SEES IT: with the settings window
        // open over the editor these buttons are drawn onto the window's
        // own plane and keep it; drawn under something else they do not.
        let (mx, my) = ctx.mouse.at();
        let now = Instant::now();
        let btns = Self::save_buttons(w, h);
        let labels = ["SETTINGS", "ADD WIDGET", "SAVE", "SAVE AS", "CANCEL", "EXIT"];
        let lit_ms = press_ms();
        for (i, br) in btns.iter().enumerate() {
            let hover = !self.add_open && self.naming.is_none() && br.contains(mx, my);
            let flash = self
                .flash
                .map(|(fi, t)| {
                    fi == i && now.duration_since(t).as_secs_f32() * 1000.0 < lit_ms
                })
                .unwrap_or(false);
            Self::draw_button(ctx, br, labels[i], hover, flash);
        }
    }


    /// Draws the visible grid, panel outlines and the editor controls on
    /// top of the live interface. The `mini` callback draws a live
    /// miniature of the given widget into a rectangle (used by the ADD
    /// WIDGET window). Also advances the ADD WIDGET hold — after the
    /// themed hold time the widget pulls out of the window, grows and
    /// follows the cursor.
    pub fn draw<F: FnMut(&mut Ctx, Panel, Rect)>(&mut self, ctx: &mut Ctx, mut mini: F) {
        let t = theme::resolved();
        let (w, h) = (ctx.w, ctx.h);
        let (mx, my) = ctx.mouse.at();

        // ADD WIDGET hold finished -> a new instance pulls out of the
        // window (it starts at its miniature size and grows under the
        // cursor).
        if let Some((widget, t0)) = self.adding {
            if t0.elapsed().as_secs_f32() >= hold_secs() {
                let (_, items) = self.add_list_rects(w, h);
                let (mw, mh) = self
                    .offer()
                    .iter()
                    .position(|&p| p == widget)
                    .and_then(|s| items.get(s))
                    .map(|ir| (ir.w, ir.h))
                    .unwrap_or_else(|| {
                        // The degenerate path reuses a themed size instead
                        // of carrying a design of its own (governing
                        // principle).
                        let m = self.min_outer();
                        (m, m)
                    });
                self.adding = None;
                self.add_open = false;
                // PLACEMENT — where the hand is, not what it is over:
                // the new instance appears under the cursor and is
                // dragged from there.
                self.pull_out(widget, (mw, mh), ctx.mouse.raw(), w, h);
            }
        }

        // Growth animation: miniature -> placement size, centred on the
        // cursor while it is being dragged.
        if let Some((id, t0, mw, mh)) = self.grow {
            let (x, e) = grow_progress(t0.elapsed().as_secs_f32());
            match self.px_rect(id, w, h) {
                None => self.grow = None,
                Some(r) => {
                    let (tw, th) = self.spawn_size(w, h);
                    let (cw_, ch_) = (mw + (tw - mw) * e, mh + (th - mh) * e);
                    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                    let nr = Rect::new(
                        (cx - cw_ / 2.0).clamp(0.0, (w - cw_).max(0.0)),
                        (cy - ch_ / 2.0).clamp(0.0, (h - ch_).max(0.0)),
                        cw_,
                        ch_,
                    );
                    self.list.set_rect(id, Some(pct(nr, w, h)));
                    if let Some((di, mode)) = self.drag.as_mut() {
                        if *di == id {
                            if let Mode::Move { dx, dy } = mode {
                                *dx = cw_ / 2.0;
                                *dy = ch_ / 2.0;
                            }
                        }
                    }
                    if x >= 1.0 {
                        self.grow = None;
                        if self.snap && self.drag.is_none() {
                            self.snap_all(w, h);
                        }
                    }
                }
            }
        }

        // The visible grid.
        static GRID_C: OnceLock<TokenId> = OnceLock::new();
        static GRID_W: OnceLock<TokenId> = OnceLock::new();
        let grid_c = col(t.color(tok(&GRID_C, "component.editor.grid_line")));
        let grid_w = t.px(tok(&GRID_W, "editor.grid.stroke"));
        for i in 0..=self.cols {
            let x = i as f32 / self.cols as f32 * w;
            ctx.dl.line(x, 0.0, x, h, grid_w, grid_c);
        }
        for i in 0..=self.rows {
            let y = i as f32 / self.rows as f32 * h;
            ctx.dl.line(0.0, y, w, y, grid_w, grid_c);
        }

        // Nothing under the editor's own controls reacts to the cursor.
        let ui_hover = self.over_ui(mx, my, w, h);

        // Panel outlines, name tags and remove buttons.
        static PROXY_CLASS: OnceLock<Option<u16>> = OnceLock::new();
        static XBTN_CLASS: OnceLock<Option<u16>> = OnceLock::new();
        static PROXY_FILL: OnceLock<TokenId> = OnceLock::new();
        static HANDLE_C: OnceLock<TokenId> = OnceLock::new();
        static HANDLE_S: OnceLock<TokenId> = OnceLock::new();
        static HANDLE_MIN: OnceLock<TokenId> = OnceLock::new();
        static X_INSET: OnceLock<TokenId> = OnceLock::new();
        static X_STROKE: OnceLock<TokenId> = OnceLock::new();
        static X_HOT_C: OnceLock<TokenId> = OnceLock::new();
        let proxy_class = *PROXY_CLASS.get_or_init(|| theme::class_id("editor.proxy"));
        let xbtn_class = *XBTN_CLASS.get_or_init(|| theme::class_id("icon_button"));
        let style = |class: Option<u16>, hot: bool| match class {
            Some(c) => t.class_state(c, if hot { State::Hover } else { State::Idle }),
            None => StateStyle::RAW,
        };
        let edge = grab_edge();
        let dragged = self.drag.as_ref().map(|(id, _)| *id);
        // The rectangles are read from a copy: the loop below asks the
        // list questions of its own (how many of this widget stand
        // here), and the borrow checker will not have both at once.
        let placed: Vec<Instance> = self.list.all().to_vec();
        for inst in &placed {
            let r = Self::px_of(inst, w, h);
            let hot = dragged == Some(inst.id)
                || (dragged.is_none() && !ui_hover && {
                    let outer = Rect::new(
                        r.x - edge,
                        r.y - edge,
                        r.w + 2.0 * edge,
                        r.h + 2.0 * edge,
                    );
                    outer.contains(mx, my)
                });
            let st = style(proxy_class, hot);
            if hot {
                ctx.dl
                    .rect(r.x, r.y, r.w, r.h, col(t.color(tok(&PROXY_FILL, "component.editor.proxy_fill"))));
            }
            ctx.dl
                .rect_outline(r.x, r.y, r.w, r.h, proxy_border(hot), col(st.edge));
            // Corner resize handles on the hot panel.
            if hot {
                let s = t
                    .px(tok(&HANDLE_S, "editor.handle"))
                    .max(t.px(tok(&HANDLE_MIN, "editor.handle_min_px")));
                let hc = col(t.color(tok(&HANDLE_C, "editor.handle_color")));
                for (cx, cy) in [
                    (r.x, r.y),
                    (r.right(), r.y),
                    (r.x, r.bottom()),
                    (r.right(), r.bottom()),
                ] {
                    ctx.dl.rect(cx - s / 2.0, cy - s / 2.0, s, s, hc);
                }
            }
            nameplate(ctx, r.x, r.y, inst.widget.label(), col(st.text));
            // X in the top-right corner takes this instance off the
            // board — except where it is the last of something the
            // user could not switch back on.
            if self.removable(inst.id) {
                let xr = Self::x_rect(&r);
                let x_hot = !ui_hover && xr.contains(mx, my);
                let xst = style(xbtn_class, x_hot);
                ctx.dl.rect(xr.x, xr.y, xr.w, xr.h, col(xst.fill));
                ctx.dl.rect_outline(
                    xr.x,
                    xr.y,
                    xr.w,
                    xr.h,
                    proxy_border(false),
                    col(xst.edge),
                );
                let m = t.px(tok(&X_INSET, "winframe.icon.inset"));
                // A remove control heats up in the critical colour, the
                // way the window close glyph does.
                let c = if x_hot {
                    col(t.color(tok(&X_HOT_C, "component.window_control.close_hover")))
                } else {
                    col(xst.glyph)
                };
                let xw = t.px(tok(&X_STROKE, "editor.proxy.close_stroke"));
                ctx.dl
                    .line(xr.x + m, xr.y + m, xr.right() - m, xr.bottom() - m, xw, c);
                ctx.dl
                    .line(xr.right() - m, xr.y + m, xr.x + m, xr.bottom() - m, xw, c);
            }
        }

        // The editor buttons in the bottom-right corner.
        self.draw_buttons(ctx);

        // Hint line in the bottom-left corner.
        static HINT_X: OnceLock<TokenId> = OnceLock::new();
        static HINT_Y: OnceLock<TokenId> = OnceLock::new();
        static HINT_C: OnceLock<TokenId> = OnceLock::new();
        let hint = role_hint();
        let hint_px = px_of(hint, ctx);
        ctx.dl.text(
            ctx.fonts,
            hint.font(),
            hint_px,
            t.px(tok(&HINT_X, "editor.hint.inset_x")),
            h - t.px(tok(&HINT_Y, "editor.hint.inset_y")),
            "DRAG TO MOVE \u{2014} DRAG EDGES TO RESIZE \u{2014} X REMOVES \u{2014} ESC EXITS WITHOUT SAVING",
            col(t.color(tok(&HINT_C, "text.muted"))),
            hint.tracking_px(hint_px),
        );

        // ADD WIDGET list window (opaque).
        if self.add_open {
            static SCRIM: OnceLock<TokenId> = OnceLock::new();
            static TITLE_C: OnceLock<TokenId> = OnceLock::new();
            static EMPTY_C: OnceLock<TokenId> = OnceLock::new();
            static TILE_CLASS: OnceLock<Option<u16>> = OnceLock::new();
            static HOLD_FILL: OnceLock<TokenId> = OnceLock::new();
            static HEAD: OnceLock<TokenId> = OnceLock::new();
            static RING_W: OnceLock<TokenId> = OnceLock::new();
            let (win, items) = self.add_list_rects(w, h);
            nacelle::object::window::backdrop(ctx, t.px(tok(&SCRIM, "modal.scrim_alpha")));
            nacelle::object::window::frame(ctx, win);
            // Read again, on THIS plane: `mx, my` above belongs to the
            // grid under this window, and under this window there is no
            // pointer. The tiles are on the window, so they get the
            // window's answer.
            let (mx, my) = ctx.mouse.at();
            let list_title = role_list_title();
            let tpx = px_of(list_title, ctx);
            ctx.dl.text_center(
                ctx.fonts,
                list_title.font(),
                tpx,
                win.cx(),
                win.y + list_pad(),
                &format!(
                    "ADD WIDGET \u{2014} HOLD {:.0}S TO PLACE",
                    hold_secs().ceil()
                ),
                col(t.color(tok(&TITLE_C, "text.title"))),
                list_title.tracking_px(tpx),
            );
            let offer = self.offer();
            if offer.is_empty() {
                // The only way this window has nothing to show: the
                // installation holds no widget this board could take.
                // "Everything is already placed" is no longer a state
                // that exists — a placed widget is still on offer.
                let empty = role_empty();
                let px = px_of(empty, ctx);
                ctx.dl.text_center(
                    ctx.fonts,
                    empty.font(),
                    px,
                    win.cx(),
                    win.y + win.h / 2.0,
                    "NO WIDGETS INSTALLED FOR THIS BOARD",
                    col(t.color(tok(&EMPTY_C, "text.muted"))),
                    empty.tracking_px(px),
                );
            }
            let tile_class = *TILE_CLASS.get_or_init(|| theme::class_id("tile"));
            let tile = |hot: bool| match tile_class {
                Some(c) => t.class_state(c, if hot { State::Hover } else { State::Idle }),
                None => StateStyle::RAW,
            };
            for (slot, ir) in items.iter().enumerate() {
                let Some(&widget) = offer.get(slot) else { break };
                let held = self.adding.map(|(p, _)| p == widget).unwrap_or(false);
                let hover = ir.contains(mx, my);
                let st = tile(hover || held);
                ctx.dl.rect(ir.x, ir.y, ir.w, ir.h, col(tile(false).fill));
                // Live miniature of the widget (headers drawn above the
                // rect by some widgets get a little headroom).
                let head = t.px(tok(&HEAD, "editor.list.preview_head"));
                // The window's own padding, through the one function that
                // knows it: `editor.list.pad` held above
                // `editor.list.pad_min_px`. Reading the FLOOR here read a
                // device-px companion (§3.2) as though it were the length
                // — a theme that opened the list up left its miniatures
                // pressed against the entry at 6 px, and every other
                // inset in this window moved without them.
                let m = list_pad();
                mini(
                    ctx,
                    widget,
                    Rect::new(
                        ir.x + m,
                        ir.y + m + head,
                        ir.w - 2.0 * m,
                        // Only the degenerate case is caught here. The
                        // 10 px this used to stand on was a floor with
                        // nobody's name on it: a theme that squeezed the
                        // entry got a miniature this file had decided
                        // on. `editor.list.item_h_min_px` is where an
                        // entry's own floor is declared.
                        (ir.h - 2.0 * m - head).max(0.0),
                    ),
                );
                // Hold progress fills the entry from the left.
                if held {
                    if let Some((_, t0)) = self.adding {
                        let p = (t0.elapsed().as_secs_f32() / hold_secs().max(0.001))
                            .clamp(0.0, 1.0);
                        ctx.dl.rect(
                            ir.x,
                            ir.y,
                            ir.w * p,
                            ir.h,
                            col(t.color(tok(&HOLD_FILL, "component.editor.hold_fill"))),
                        );
                    }
                } else if hover {
                    ctx.dl.rect(ir.x, ir.y, ir.w, ir.h, col(st.fill));
                }
                ctx.dl.rect_outline(
                    ir.x,
                    ir.y,
                    ir.w,
                    ir.h,
                    t.px(tok(&RING_W, "editor.proxy.border")),
                    col(st.edge),
                );
                // The same name tag the panels wear.
                nameplate(ctx, ir.x, ir.y, widget.label(), col(st.text));
            }
        }

        // SAVE AS name prompt — the F1 §3 input object is the field.
        if self.naming.is_some() {
            static SCRIM: OnceLock<TokenId> = OnceLock::new();
            static W_FRAC: OnceLock<TokenId> = OnceLock::new();
            static W_MIN: OnceLock<TokenId> = OnceLock::new();
            static W_MIN_PX: OnceLock<TokenId> = OnceLock::new();
            static H_FRAC: OnceLock<TokenId> = OnceLock::new();
            static H_MIN: OnceLock<TokenId> = OnceLock::new();
            static TITLE_C: OnceLock<TokenId> = OnceLock::new();
            static BODY_TOP: OnceLock<TokenId> = OnceLock::new();
            static PAD: OnceLock<TokenId> = OnceLock::new();
            static FIELD_H: OnceLock<TokenId> = OnceLock::new();
            static HINT_INSET: OnceLock<TokenId> = OnceLock::new();
            static HINT_C: OnceLock<TokenId> = OnceLock::new();
            nacelle::object::window::backdrop(ctx, t.px(tok(&SCRIM, "modal.scrim_alpha")));
            let bw = (w * t.px(tok(&W_FRAC, "modal.w_frac")))
                .max(t.px(tok(&W_MIN, "modal.min_w")))
                .max(t.px(tok(&W_MIN_PX, "modal.min_w_min_px")));
            // The prompt's own height token — modal.h_frac is the settings
            // window, a different object.
            let bh = (h * t.px(tok(&H_FRAC, "dialog.h_frac")))
                .max(t.px(tok(&H_MIN, "dialog.h_min_px")));
            let bx = (w - bw) / 2.0;
            let by = (h - bh) / 2.0;
            nacelle::object::window::frame(ctx, Rect::new(bx, by, bw, bh));
            let modal_title = role_modal_title();
            let tpx = px_of(modal_title, ctx);
            ctx.dl.text_center(
                ctx.fonts,
                modal_title.font(),
                tpx,
                bx + bw / 2.0,
                modal_title_y(by, tpx),
                "SAVE AS \u{2014} TYPE A NAME",
                col(t.color(tok(&TITLE_C, "text.title"))),
                modal_title.tracking_px(tpx),
            );
            // The field: modal padding aside, [field] geometry, the
            // model already holds caret/selection/preedit. The desktop
            // always passes a focus chain, and `begin_naming` focused
            // this field on open — so the chain answers "focused" and
            // the caret/selection/preedit actually draw. The fallback
            // below only speaks in a chainless world (none in the
            // desktop today): a modal grab's field IS focused by
            // construction.
            let pad = t.px(tok(&PAD, "modal.pad")).max(0.0);
            let fh = t.px(tok(&FIELD_H, "field.h")).max(1.0);
            let field_y = by + t.px(tok(&BODY_TOP, "modal.body_top"));
            let field = Rect::new(bx + pad, field_y, (bw - 2.0 * pad).max(2.0), fh);
            self.naming_field = Some(field);
            let click = self.naming_click.take();
            if let Some(model) = self.naming.as_mut() {
                use nacelle::object::text_input::{self, InputMsg, InputStyle};
                // The queued click lands now, when the fonts are here
                // to hit-test with (drag/double-click selection stays
                // keyboard-shaped in F1: Shift+arrows, Ctrl+A).
                if let Some((cx, _)) = click {
                    let at = text_input::hit(ctx, field, model, cx);
                    model.apply(InputMsg::Point { at, extend: false });
                }
                let out = text_input::draw(
                    ctx,
                    field,
                    model,
                    Self::naming_focus_id(),
                    &InputStyle {
                        placeholder: "layout name",
                        hover: field.contains(mx, my),
                        disabled: false,
                        focused_fallback: true,
                    },
                );
                self.naming_caret = out.caret;
            }
            let modal_hint = role_modal_hint();
            let hpx = px_of(modal_hint, ctx);
            ctx.dl.text_center(
                ctx.fonts,
                modal_hint.font(),
                hpx,
                bx + bw / 2.0,
                field.bottom() + t.px(tok(&HINT_INSET, "settings.hint_inset")),
                "ENTER SAVES \u{2014} ESC CANCELS",
                col(t.color(tok(&HINT_C, "text.muted"))),
                modal_hint.tracking_px(hpx),
            );
        }
    }
}

/// Resize cursor for the given edge combination.
fn edge_cursor(l: bool, r: bool, t: bool, b: bool) -> CursorKind {
    if (l && t) || (r && b) {
        CursorKind::Nwse
    } else if (r && t) || (l && b) {
        CursorKind::Nesw
    } else if l || r {
        CursorKind::Ew
    } else {
        CursorKind::Ns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window to place things in — the editor works in percentages,
    /// so the numbers only have to be a plausible screen.
    const W: f32 = 1920.0;
    const H: f32 = 1080.0;

    /// A drawing context over a plausible screen, and everything the
    /// closure put on it.
    ///
    /// The editor draws through the same `Ctx` the program hands it, so
    /// these tests need no window: a recording [`DrawList`] is as good a
    /// destination as a GPU, and the miniature callback is a closure the
    /// caller already owns.
    fn recorded(draw: impl FnOnce(&mut Ctx)) -> Vec<nacelle::draw::DrawCmd> {
        let mut fonts = crate::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::recording();
        {
            let mut ctx = Ctx {
                dl: &mut dl,
                fonts: &mut fonts,
                w: W,
                h: H,
                t: 0.0,
                // Off the screen: a hovered control may swap the look it
                // draws in, and nothing here is a question about hover.
                mouse: nacelle::pointer::Pointer::new(-1.0, -1.0),
                term_font_scale: 1.0,
                ui_font_scale: 1.0,
                panel_scale: 1.0,
                focus: None,
                tips: None,
            };
            draw(&mut ctx);
        }
        dl.cmds().to_vec()
    }

    /// The name tag is ONE plate: its label stands the same distance
    /// from its left edge as from its right, and that distance is the
    /// padding the master declares for it.
    ///
    /// The plate's width comes from `editor.proxy.label_pad_x` and the
    /// label's inset used to come from `panel.title.inset_x` — a length
    /// belonging to the title band of a PANEL. Two families met on one
    /// tag: at the master's own numbers the label sat 8.6 px in from the
    /// left of a plate that allowed 11.5 px for both sides together, so
    /// it stood 2.9 px from the right, and a theme that widened the plate
    /// slid its label further off centre instead of moving it.
    ///
    /// Both halves are asked here, because centring alone is the weaker
    /// half of the claim: a plate `tw + pad` wide with its label `pad / 2`
    /// in is centred too, and hands a theme author half the room the
    /// master's word `pad_x` promises — the word every other reader of
    /// `button.pad_x` in this program spends on EACH side.
    #[test]
    fn a_nameplate_centres_its_label_in_its_own_padding() {
        // Selects nothing, but READS a process-wide engine other tests
        // select in (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        nacelle::theme::load();
        nacelle::theme::set_viewport(H, 1.0);

        const LABEL: &str = "SHELL";
        const X: f32 = 100.0;
        const Y: f32 = 200.0;
        let mut text_w = 0.0;
        let cmds = recorded(|ctx| {
            nameplate(ctx, X, Y, LABEL, Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
            // Measured through the plate's OWN role and face, which is
            // what the plate measured itself with.
            let role = role_label();
            let px = px_of(role, ctx);
            text_w = ctx.fonts.measure(role.font(), px, LABEL, role.tracking_px(px));
        });

        let plate = cmds
            .iter()
            .find_map(|c| match c {
                nacelle::draw::DrawCmd::Rect { r, .. } => Some(*r),
                _ => None,
            })
            .expect("the tag draws its plate");
        let (tx, anchor) = cmds
            .iter()
            .find_map(|c| match c {
                nacelle::draw::DrawCmd::Text { at, anchor, .. } => Some((at[0], *anchor)),
                _ => None,
            })
            .expect("the tag draws its label");
        assert_eq!(
            anchor,
            nacelle::draw::TextAnchor::Left,
            "the label is set from its left edge, so `at[0]` is where it starts"
        );

        let left = tx - plate[0];
        let right = (plate[0] + plate[2]) - (tx + text_w);
        assert!(
            left > 0.0,
            "the label starts outside its own plate: left {left}, plate {plate:?}"
        );
        assert!(
            (left - right).abs() < 0.01,
            "the label is off centre in its plate — {left} px from the left edge and \
             {right} px from the right: the width and the inset are two different \
             tokens' answers"
        );
        let pad = crate::widgets::token_px("editor.proxy.label_pad_x");
        assert!(
            (left - pad).abs() < 0.01,
            "the label stands {left} px in from its plate's edge, and the master \
             declares `editor.proxy.label_pad_x` = {pad} px — a padding a reader \
             spends on both sides together is half the padding the file offered"
        );
    }

    /// The ADD WIDGET miniatures are inset by the LIST's padding.
    ///
    /// `editor.list.pad_min_px` is a §3.2 companion — the device-px floor
    /// under `editor.list.pad`, never a length in its own right — and the
    /// miniature read it directly. A theme that opened the list up moved
    /// every other inset in that window and left the previews pressed
    /// against their entries at six pixels.
    #[test]
    fn a_miniature_is_inset_by_the_lists_own_padding() {
        // Selects a theme in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        // Far above the floor, so the two answers cannot be mistaken for
        // each other: 6u is 32.4 px against a 6 px floor.
        let _open = Themed::new("editor-list-pad", "[editor]\nlist.pad = 6u\n");
        nacelle::theme::set_viewport(H, 1.0);
        let pad = list_pad();
        assert!(
            pad > 20.0,
            "the fixture must part the padding from its floor, or this test guards \
             nothing (it read {pad})"
        );

        let mut ed = Editor::new(screen_gutter(0.0));
        ed.start(
            &Layout::empty(W, H),
            W,
            H,
            false,
            20,
            20,
            screen_gutter(0.0),
            (0, 0),
            WidgetCategory::Board,
            1,
        );
        // ADD WIDGET is the second button of the stack; pressing it is
        // how the window opens in the program.
        let btns = Editor::save_buttons(W, H);
        let b = btns[1];
        ed.buttons_hit(b.x + b.w / 2.0, b.y + b.h / 2.0, W, H);

        let (_, items) = ed.add_list_rects(W, H);
        assert!(!items.is_empty(), "the fixture registry offers no widget to list");

        let mut shown: Vec<Rect> = Vec::new();
        recorded(|ctx| ed.draw(ctx, |_, _, r| shown.push(r)));
        assert!(!shown.is_empty(), "the open ADD WIDGET window drew no miniature");
        for (m, entry) in shown.iter().zip(items.iter()) {
            assert!(
                (m.x - (entry.x + pad)).abs() < 0.01,
                "a miniature is {} px in from its entry, not the list's own {pad} px",
                m.x - entry.x
            );
            assert!(
                (m.w - (entry.w - 2.0 * pad)).abs() < 0.01,
                "a miniature is {} px wide inside a {} px entry, which is not that \
                 padding on both sides",
                m.w,
                entry.w
            );
        }
    }

    /// A gutter as a SCREEN would hand it over.
    ///
    /// `Screen::new` wants a window and an event loop, so these tests
    /// reach the editor's doors directly — and what they are testing is
    /// that a length passed through them comes out unchanged. The type
    /// exists so that nothing SHIPPED can put the settings file's
    /// rounded reading in this position; standing in for a screen is
    /// what its test-only constructor is for.
    fn screen_gutter(px: f32) -> Gutter {
        Gutter::of_test(px)
    }

    /// The widget registry these tests place from: the crates linked
    /// into this program plus the addons shipped beside it, which is
    /// the set the program itself comes up with.
    ///
    /// It must hold the SAME widgets as the fixture in `config`'s
    /// tests. The registry is process-wide and fixed by the first call
    /// — or by the first READ, which freezes it empty — and every unit
    /// test of this program runs in one binary, so whichever fixture
    /// wins the race has to leave the other one's tests true. Its
    /// staging directory is its own, though: two fixtures sharing one
    /// would be free to wipe it while the other is scanning it.
    fn fixture_registry() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let stage = std::env::temp_dir()
                .join(format!("nacelle-editor-registry-fixture-{}", std::process::id()));
            let scripts = stage.join("addons").join("scripts");
            let _ = std::fs::remove_dir_all(&stage);
            std::fs::create_dir_all(&scripts).expect("the fixture tree must be writable");
            let shipped = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../nacelle-addons/scripts"
            ));
            let rd = std::fs::read_dir(shipped)
                .expect("the nacelle-addons repository must sit next to this one");
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                    let name = path.file_name().expect("a file has a name");
                    std::fs::copy(&path, scripts.join(name)).expect("stage the addon");
                }
            }
            let roots = nacelle::assets::AssetRoots::new(vec![stage.clone()], stage);
            let factory = [
                nacelle_widget_aichat::WIDGET,
                nacelle_widget_ailoop::WIDGET,
                nacelle_widget_aiphoto::WIDGET,
                nacelle_widget_aisort::WIDGET,
                nacelle_widget_appcats::WIDGET,
                nacelle_widget_appgrid::WIDGET,
                nacelle_widget_control::WIDGET,
                nacelle_widget_filesystem::WIDGET,
                nacelle_widget_keyboard::WIDGET,
                nacelle_widget_search::WIDGET,
                nacelle_widget_shell::WIDGET,
            ]
            .into_iter()
            .fold(
                nacelle::widget::factory::WidgetFactory::new(roots),
                |f, w| f.with_builtin(w),
            );
            nacelle::base::set_registry(factory.registry());
        });
    }

    /// Every widget an ordinary board takes.
    fn board_widgets() -> Vec<Panel> {
        fixture_registry();
        Panel::all()
            .into_iter()
            .filter(|p| p.category() == WidgetCategory::Board)
            .collect()
    }

    /// One of them to experiment on, chosen for having declared nothing
    /// that would keep it on the board.
    fn a_board_widget() -> Panel {
        board_widgets()
            .into_iter()
            .find(|p| !p.essential())
            .expect("the shipped set holds an ordinary board widget")
    }

    /// An editor over an empty board of a layout whose counter stands
    /// at `next_id`.
    fn editor_over(board: &Layout, next_id: u32) -> Editor {
        fixture_registry();
        let mut ed = Editor::new(screen_gutter(8.0));
        ed.start(
            board,
            W,
            H,
            false,
            20,
            20,
            screen_gutter(8.0),
            (0, 0),
            WidgetCategory::Board,
            next_id,
        );
        ed
    }

    /// Drags one widget out of the ADD WIDGET window and drops it.
    fn place(ed: &mut Editor, widget: Panel, x: f32, y: f32) -> InstanceId {
        let id = ed.pull_out(widget, (160.0, 90.0), (x, y), W, H);
        ed.mouse_up(W, H);
        id
    }

    fn rect_of(ed: &Editor, id: InstanceId) -> PanelSpec {
        ed.list.get(id).and_then(|i| i.rect).expect("a placed instance has a rectangle")
    }

    /// The request this whole model exists for: the same widget, twice,
    /// on one board — two placements with two identities, not one
    /// placement moved.
    #[test]
    fn the_same_widget_stands_on_one_board_twice() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let w = a_board_widget();
        let mut ed = editor_over(&Layout::empty(W, H), 1);
        let a = place(&mut ed, w, 400.0, 300.0);
        let b = place(&mut ed, w, 1400.0, 800.0);
        assert_ne!(a, b, "the second pull-out is a second widget");
        assert_eq!(ed.instances().len(), 2);
        assert!(ed.instances().iter().all(|i| i.widget == w));
        assert!(
            !same_spec(&rect_of(&ed, a), &rect_of(&ed, b)),
            "two instances, two rectangles"
        );
    }

    /// Removing one of them leaves the other exactly where it stands —
    /// the property a table indexed by widget could not have.
    #[test]
    fn removing_one_instance_leaves_the_other_untouched() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let w = a_board_widget();
        let mut ed = editor_over(&Layout::empty(W, H), 1);
        let a = place(&mut ed, w, 400.0, 300.0);
        let b = place(&mut ed, w, 1400.0, 800.0);
        let before = rect_of(&ed, b);

        // Through the control the user actually clicks: the X in the
        // top-right corner of the first one.
        let xr = Editor::x_rect(&Editor::px_of(ed.list.get(a).expect("just placed"), W, H));
        ed.mouse_down(xr.cx(), xr.y + xr.h / 2.0, W, H);

        assert!(ed.list.get(a).is_none(), "the X removes the instance it sits on");
        assert_eq!(ed.instances().len(), 1);
        assert!(
            same_spec(&rect_of(&ed, b), &before),
            "the other one did not move"
        );
    }

    /// Dragging is by identity too: the second terminal stays put while
    /// the first one is pulled across the board.
    #[test]
    fn dragging_one_instance_moves_only_that_one() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let w = a_board_widget();
        let mut ed = editor_over(&Layout::empty(W, H), 1);
        let a = place(&mut ed, w, 400.0, 300.0);
        let b = place(&mut ed, w, 1400.0, 800.0);
        let before = rect_of(&ed, b);

        let r = Editor::px_of(ed.list.get(a).expect("just placed"), W, H);
        ed.mouse_down(r.cx(), r.y + r.h / 2.0, W, H);
        ed.mouse_move(r.cx() + 200.0, r.y + r.h / 2.0 + 100.0, W, H);
        ed.mouse_up(W, H);

        assert!(rect_of(&ed, a).x > 0.0);
        assert!(!same_spec(&rect_of(&ed, a), &before));
        assert!(same_spec(&rect_of(&ed, b), &before), "the other one did not move");
    }

    /// ADD WIDGET stopped hiding things. Every widget the board takes
    /// is on offer whatever is already standing there — and nothing of
    /// another board's kind ever is.
    #[test]
    fn the_add_widget_list_hides_nothing() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let all = board_widgets();
        let w = a_board_widget();
        let mut ed = editor_over(&Layout::empty(W, H), 1);
        assert_eq!(ed.offer(), all, "an empty board offers everything it takes");

        place(&mut ed, w, 400.0, 300.0);
        place(&mut ed, w, 1400.0, 800.0);
        assert_eq!(
            ed.offer(),
            all,
            "a widget standing here twice is still on offer a third time"
        );
        assert!(
            Panel::all().len() > all.len(),
            "the fixture holds widgets of other kinds, so the filter is doing something"
        );
        assert!(ed.offer().iter().all(|p| p.category() == WidgetCategory::Board));
    }

    /// A widget dragged out here may not be given an identity some
    /// other board of the same layout is already using, and a removal
    /// does not put one back into circulation.
    #[test]
    fn a_new_instance_never_takes_an_id_already_in_the_layout() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let w = a_board_widget();
        // The board arrives with three placements; the layout's other
        // boards have taken the counter as far as 9.
        let mut board = Layout::empty(W, H);
        for (n, x) in [(1u32, 0.0f32), (2, 300.0), (3, 600.0)] {
            board.place(InstanceId::new(n), w, Rect::new(x, 0.0, 200.0, 150.0));
        }
        let mut ed = editor_over(&board, 9);
        assert_eq!(ed.instances().len(), 3, "the board came in as it was drawn");

        let a = place(&mut ed, w, 1000.0, 500.0);
        assert!(a.get() >= 9, "the layout's counter is where the next id comes from");

        // Take it off again: the id it held is retired, not recycled.
        ed.list.remove(a);
        let b = place(&mut ed, w, 1200.0, 600.0);
        assert_ne!(a, b);

        let mut ids: Vec<u32> = ed.instances().iter().map(|i| i.id.get()).collect();
        let placed = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), placed, "no two placements share an identity");
        assert!(ed.next_free() > b.get(), "the counter is carried back past what was used");
    }

    /// CANCEL puts the board back as it was found, and the identities
    /// the abandoned widgets held stay retired.
    #[test]
    fn cancel_puts_the_board_back_without_reusing_identities() {
        // The editor's geometry comes out of the theme, and the
        // theme is a process-wide global (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let w = a_board_widget();
        let mut board = Layout::empty(W, H);
        board.place(InstanceId::new(4), w, Rect::new(0.0, 0.0, 200.0, 150.0));
        let mut ed = editor_over(&board, 5);
        let kept = rect_of(&ed, InstanceId::new(4));

        let a = place(&mut ed, w, 1000.0, 500.0);
        let btns = Editor::save_buttons(W, H);
        ed.buttons_hit(btns[4].cx(), btns[4].y + btns[4].h / 2.0, W, H);

        assert_eq!(ed.instances().len(), 1, "the added widget is gone again");
        assert!(same_spec(&rect_of(&ed, InstanceId::new(4)), &kept));
        let b = place(&mut ed, w, 1000.0, 500.0);
        assert_ne!(a, b, "an abandoned id is never handed out again");
    }

    // ------------------------------------------------- the theme's say

    use crate::widgets::{token_px as number, Themed};

    /// Where the name tag's label sits is the rhythm block's business,
    /// and this file used to keep it to itself: it shared the plate out
    /// over the GLYPH (`px`) instead of over the line the glyph stands
    /// on, and never asked whether the theme centres optically. The
    /// master asks for both, so the tag was drawn in a place its own
    /// theme does not name.
    #[test]
    fn the_name_tag_sits_where_the_rhythm_block_puts_it() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let (top, px) = (100.0, 20.0);
        let plate_h = nameplate_h();
        let leading = role_label().leading();
        let bias = number("rhythm.cap_center_bias");
        assert!(leading > 1.0, "the master gives type.caption a line taller than its glyphs");

        let optical = nameplate_text_y(top, px);
        let glyph_centred = top + (plate_h - px) / 2.0;
        assert!(
            (optical - glyph_centred).abs() > 0.5,
            "the tag is still centred the old way: {optical} vs {glyph_centred}"
        );

        // The one token that decides whether the optical nudge happens.
        let geometric = {
            let _t = Themed::new("geometric", "[rhythm]\ncenter_mode = geometric\n");
            nameplate_text_y(top, px)
        };
        assert!(
            ((optical - geometric) - px * bias).abs() < 1e-3,
            "switching center_mode moved the label by {} rather than the declared {}",
            optical - geometric,
            px * bias
        );

        // And the role's line height, which is the other half of it.
        let tall = {
            let _t = Themed::new("tall-caption", "[type]\ncaption.leading = 2.0\n");
            nameplate_text_y(top, px)
        };
        assert!(
            (tall - (optical - px * (2.0 - leading) / 2.0)).abs() < 1e-3,
            "a taller line did not move the label: {tall} vs {optical}"
        );
    }

    /// The SAVE AS title had the same two omissions, in a band whose
    /// height is a token of its own.
    #[test]
    fn the_modal_title_sits_where_the_rhythm_block_puts_it() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let (top, px) = (60.0, 24.0);
        let master = modal_title_y(top, px);
        let glyph_centred = top + (number("modal.title.band_h") - px) / 2.0;
        assert!(
            (master - glyph_centred).abs() > 0.5,
            "the title is still centred the old way: {master} vs {glyph_centred}"
        );

        let geometric = {
            let _t = Themed::new("geometric-title", "[rhythm]\ncenter_mode = geometric\n");
            modal_title_y(top, px)
        };
        let bias = number("rhythm.cap_center_bias");
        assert!(
            ((master - geometric) - px * bias).abs() < 1e-3,
            "switching center_mode left the modal title where it was"
        );

        let deep = {
            let _t = Themed::new("deep-band", "[modal]\ntitle.band_h = 20u\n");
            modal_title_y(top, px)
        };
        assert!(deep > master + 1.0, "a taller title band did not move the title");
    }

    /// `motion.scale = 0` is how a theme says "no animation", and the
    /// tear-off growth ran straight through it. A one-shot freezes at
    /// the state it was going to, so the widget arrives at full size on
    /// the first frame instead of never arriving.
    #[test]
    fn reduced_motion_places_the_widget_instead_of_growing_it() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let dur = number("motion.widget_grow.duration_ms") / 1000.0;
        let (x0, e0) = grow_progress(0.0);
        assert_eq!((x0, e0), (0.0, 0.0), "the master's widget does start small");
        let (half, eased) = grow_progress(dur / 2.0);
        assert!((half - 0.5).abs() < 1e-3, "halfway through is {half}");
        assert!(eased > half, "the master's ease_out runs ahead of linear");

        for (tag, body) in [
            ("still", "[motion]\nscale = 0.0\n"),
            ("nogrow", "[motion.widget_grow]\nenabled = false\n"),
        ] {
            let _t = Themed::new(tag, body);
            assert_eq!(
                grow_progress(0.0),
                (1.0, 1.0),
                "{tag}: the widget still grew on screen"
            );
        }

        // And the scale is a MULTIPLIER, not a switch: doubling it
        // doubles the time the same instant is a fraction of.
        let _t = Themed::new("slow", "[motion]\nscale = 2.0\n");
        let (slow, _) = grow_progress(dur / 2.0);
        assert!((slow - 0.25).abs() < 1e-3, "half the time into a doubled duration is {slow}");
    }

    /// `custom` is a word in `motion.*.easing`'s closed set like any
    /// other, and it is the one word that carries numbers with it —
    /// `easing_p`'s four bezier points. The growth's private resolver
    /// knew THREE words and let every other one fall through to linear,
    /// so a theme that spent its bezier on the tear-off got a straight
    /// line and no complaint.
    ///
    /// The curve below stands at ~0.98 halfway through its time. Linear
    /// says 0.5 there and the master's `ease_out` says 0.75, so the
    /// assertion separates the fix from both things the old code could
    /// have answered.
    #[test]
    fn a_custom_curve_moves_the_growing_widget() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let dur = number("motion.widget_grow.duration_ms") / 1000.0;
        let _t = Themed::new(
            "grow-custom",
            "[motion.widget_grow]\neasing = custom\neasing_p = [0.00, 0.90, 0.10, 1.00]\n",
        );
        let (x, eased) = grow_progress(dur / 2.0);
        assert!((x - 0.5).abs() < 1e-3, "the raw progress is not halfway: {x}");
        assert!(
            eased > 0.9,
            "the custom curve ran as something else — halfway it stands at \
             {eased} (linear says 0.5, the master's ease_out 0.75)"
        );
    }

    /// The same global, on the button flash — whose end state is a
    /// button that is not lit, so stillness means it never lights.
    #[test]
    fn reduced_motion_takes_the_flash_off_the_editor_buttons() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let master = press_ms();
        assert!(master > 0.0, "the master's buttons do flash");

        for (tag, body) in [
            ("still-press", "[motion]\nscale = 0.0\n"),
            ("nopress", "[motion.press]\nenabled = false\n"),
        ] {
            let _t = Themed::new(tag, body);
            assert_eq!(press_ms(), 0.0, "{tag}: the button still flashed");
        }

        let _t = Themed::new("slow-press", "[motion]\nscale = 2.0\n");
        assert!(
            (press_ms() - 2.0 * master).abs() < 1e-3,
            "the flash does not follow motion.scale"
        );
    }

    // ------------------------------------------ the gutter, and whose it is

    /// A window height at which the theme's gutter is not a whole number
    /// of device pixels, with that gutter. A `u` is a fraction of the
    /// window, so most heights are such a height — but which ones depends
    /// on the theme, so it is searched for rather than assumed.
    ///
    /// The viewport is left standing at the height that answered: the
    /// caller is about to compare numbers taken under it.
    fn a_height_with_a_fractional_gutter() -> (f32, f32) {
        (600..=2400)
            .step_by(3)
            .find_map(|h| {
                nacelle::theme::set_viewport(h as f32, 1.0);
                let g = crate::config::panel_gutter(None);
                (g.fract() > 0.01 && g.fract() < 0.99).then_some((h as f32, g))
            })
            .expect(
                "no height in 600..2400 gave a fractional gutter — if the gutter is \
                 now whole by construction, the editor no longer needs the screen's \
                 own reading and this test should be retired deliberately",
            )
    }

    /// THE defect this seam exists to prevent, stated as a test.
    ///
    /// The editor is drawn OVER the boards, and the boards are solved
    /// with `Screen::pad` — the theme's gutter at that screen's height,
    /// fractions and all. The settings file can only answer the same
    /// question in whole pixels, because that is what the spinner edits,
    /// and for a long time the event loop handed the editor that rounded
    /// answer four lines after the screen had handed it the exact one.
    /// The picture said nothing: the grid simply sat up to half a pixel
    /// per u away from the panels it was editing.
    ///
    /// So: whatever the screen hands over is what the editor keeps —
    /// through the front door (`start`) and through the one the settings
    /// window uses while the editor runs (`sync_from_screen`), which is
    /// the door that reads the file and must drop the gutter it finds
    /// there. A rounded reading cannot be equal to a fractional length,
    /// so a fixture with a fraction in it is the whole assertion.
    #[test]
    fn the_editor_draws_with_the_screens_gutter_and_never_the_files() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let _wide = Themed::new("editor-gutter", "[layout]\npanel_gutter = 9u\n");
        let (h, exact) = a_height_with_a_fractional_gutter();
        assert_ne!(
            exact,
            exact.round(),
            "the fixture must be a length no whole-pixel reading can equal"
        );

        // Built while the engine's last bake was somebody ELSE's height —
        // the neighbour drawing, or at boot the engine's own default. The
        // number belongs to the screen that hands it over; an editor that
        // goes and asks for itself gets the neighbour's answer and nothing
        // says so.
        nacelle::theme::set_viewport(h * 2.0, 1.0);
        let theirs = crate::config::panel_gutter(None);
        assert_ne!(theirs, exact, "two heights must give two gutters, or this proves nothing");
        assert_eq!(
            Editor::new(screen_gutter(exact)).padding,
            exact,
            "the editor asked the theme under the neighbour's height ({theirs}) instead \
             of taking its own screen's {exact}"
        );
        nacelle::theme::set_viewport(h, 1.0);

        // Entering the editor: the screen's number, unrounded.
        let mut ed = Editor::new(screen_gutter(exact));
        assert_eq!(ed.padding, exact, "an editor is built with its screen's gutter");
        ed.start(
            &Layout::empty(W, h),
            W,
            h,
            false,
            20,
            20,
            screen_gutter(exact),
            (0, 0),
            WidgetCategory::Board,
            1,
        );
        assert_eq!(
            ed.padding, exact,
            "at {h} lines the board is drawn with {exact}; the editor took \
             something else"
        );

        // And the path the settings window takes with the editor already
        // running — the one that re-reads the preferences file. It must
        // come back with the SCREEN's gutter still in place.
        ed.sync_from_screen(screen_gutter(exact), W, h);
        assert_eq!(
            ed.padding, exact,
            "the file's gutter ({}) beat the screen's ({exact}) on the way \
             through sync_from_screen",
            crate::config::grid_prefs().3
        );

        nacelle::theme::set_viewport(1080.0, 1.0);
    }

    /// And no floor of the editor's own on top of it.
    ///
    /// `.max(0.0)` on the way in reads as a guard against a nonsense
    /// theme, but the boards are solved with the same length UNGUARDED
    /// (`Screen::pad`), so all it can ever do is make the editor
    /// disagree with the picture underneath it — and say nothing. The
    /// gutter here is a §5.0 sentinel, which is how a length token in
    /// this engine becomes negative at all: the master's `@corner.pill`
    /// bakes to -2, a theme may name it in any length slot, and the
    /// board takes it as it stands.
    #[test]
    fn the_editor_puts_no_floor_of_its_own_under_the_screens_gutter() {
        // Selects themes in a process-wide engine (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        let _odd = Themed::new("editor-gutter-sentinel", "[layout]\npanel_gutter = @corner.pill\n");
        nacelle::theme::set_viewport(1080.0, 1.0);
        let pad = crate::config::panel_gutter(None);
        assert!(
            pad < 0.0,
            "the fixture must make the gutter non-positive, or this test guards \
             nothing (it read {pad})"
        );

        let mut ed = Editor::new(screen_gutter(pad));
        assert_eq!(ed.padding, pad, "the constructor floored the screen's gutter");
        ed.start(
            &Layout::empty(W, H),
            W,
            H,
            false,
            20,
            20,
            screen_gutter(pad),
            (0, 0),
            WidgetCategory::Board,
            1,
        );
        assert_eq!(ed.padding, pad, "`start` floored the screen's gutter");
        ed.sync_from_screen(screen_gutter(pad), W, H);
        assert_eq!(ed.padding, pad, "`sync_from_screen` floored the screen's gutter");
    }
}
