//! Layout editor: an Android-style snap grid over the live interface.
//! Entered from SETTINGS -> GRID -> EDIT GRID. The grid becomes visible;
//! panels can be moved by dragging, resized by dragging their edges or
//! corners, removed with the X in their top-right corner and added back
//! via the ADD WIDGET button (hold an entry for the themed hold time —
//! the list hides and the widget follows the cursor until you drop it
//! on the grid). With SNAP TO GRID enabled every panel edge is aligned to the
//! grid cells — including an automatic fit of all panels when the editor
//! opens. The editor works on the OUTER panel rectangles; the widget
//! padding (SETTINGS -> GRID) insets the content inside them.
//! Bottom-right buttons: ADD WIDGET, SAVE (overwrites the currently
//! selected layout), SAVE AS (asks for a name) and CANCEL (exits
//! without saving).

use super::{panel_count, Ctx, Layout, LayoutSpec, Panel, PanelSpec, Rect, OFF_SPEC};
use crate::font::FONT_UI;
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

/// One type role's hot ids, resolved once. Size is the theme's px times
/// the user's font preference, floored by the role's `min_px`; tracking
/// is the role's em value multiplied out per run — the same arithmetic
/// the object layer uses.
struct Role {
    name: &'static str,
    size: OnceLock<TokenId>,
    min: OnceLock<TokenId>,
    track: OnceLock<TokenId>,
}

impl Role {
    const fn new(name: &'static str) -> Self {
        Role { name, size: OnceLock::new(), min: OnceLock::new(), track: OnceLock::new() }
    }
    fn px(&self, ctx: &Ctx) -> f32 {
        let t = theme::resolved();
        let s = *self.size.get_or_init(|| {
            theme::id(&format!("type.{}.size", self.name)).unwrap_or(TokenId::MISSING)
        });
        let m = *self.min.get_or_init(|| {
            theme::id(&format!("type.{}.min_px", self.name)).unwrap_or(TokenId::MISSING)
        });
        (t.px(s) * ctx.ui_font_scale * ctx.panel_scale).max(t.px(m))
    }
    fn tracking(&self, px: f32) -> f32 {
        let t = theme::resolved();
        let k = *self.track.get_or_init(|| {
            theme::id(&format!("type.{}.tracking", self.name)).unwrap_or(TokenId::MISSING)
        });
        px * t.px(k)
    }
}

// The roles the editor's own chrome binds its text to in the master:
// `editor.proxy.label_role`, `editor.hint.role`, `editor.list.title_role`,
// `modal.title.role`, `settings.hint.role` and the empty-state's `value`.
static ROLE_LABEL: Role = Role::new("caption");
static ROLE_HINT: Role = Role::new("tooltip");
static ROLE_TITLE: Role = Role::new("title.window");
static ROLE_VALUE: Role = Role::new("value");

/// Edge-grab margin (resize handles), with its device-px floor.
fn grab_edge() -> f32 {
    static E: OnceLock<TokenId> = OnceLock::new();
    static EM: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    t.px(tok(&E, "editor.edge")).max(t.px(tok(&EM, "editor.edge_min_px")))
}

/// Hold time on an ADD WIDGET entry before placement starts.
fn hold_secs() -> f32 {
    static D: OnceLock<TokenId> = OnceLock::new();
    (theme::resolved().px(tok(&D, "motion.hold.duration_ms")) / 1000.0).max(0.0)
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

/// Active drag: moving the panel or resizing by its edges.
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
    /// Edited panel rects in percent of the window, Panel order.
    rects: Vec<PanelSpec>,
    /// The rects as they were when the editor opened — SAVE stores only
    /// the panels that differ from this.
    initial: Vec<PanelSpec>,
    drag: Option<(usize, Mode)>,
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
    /// Panels that live on ANOTHER board. A widget exists once, so a
    /// panel placed elsewhere is not offered by ADD WIDGET here —
    /// moving it means removing it there first.
    blocked: Vec<bool>,
    /// ADD WIDGET list window.
    add_open: bool,
    /// Held list entry: (panel index, hold start).
    adding: Option<(usize, Instant)>,
    /// Pull-out animation after a completed hold: the widget grows from
    /// its miniature size to the placement size under the cursor.
    grow: Option<(usize, Instant, f32, f32)>,
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

fn on_screen(p: &PanelSpec) -> bool {
    p.x < 100.0
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            active: false,
            snap: false,
            cols: crate::config::GRID_MIN,
            rows: crate::config::GRID_MIN,
            padding: 8.0,
            rects: vec![OFF_SPEC; panel_count()],
            initial: vec![OFF_SPEC; panel_count()],
            drag: None,
            naming: None,
            naming_field: None,
            naming_click: None,
            naming_caret: None,
            blocked: Vec::new(),
            add_open: false,
            adding: None,
            grow: None,
            flash: None,
        }
    }

    /// Enters edit mode with the CURRENT panel rectangles (WYSIWYG).
    /// With snapping enabled all panels are fitted to the grid at once.
    pub fn start(
        &mut self,
        layout: &Layout,
        w: f32,
        h: f32,
        snap: bool,
        cols: u32,
        rows: u32,
        padding: f32,
        blocked: Vec<bool>,
    ) {
        self.active = true;
        self.blocked = blocked;
        self.snap = snap;
        self.cols = cols.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.rows = rows.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.padding = padding.max(0.0);
        self.close_naming();
        self.drag = None;
        self.add_open = false;
        self.adding = None;
        self.grow = None;
        self.rects = (0..panel_count())
            .map(|i| pct(layout.panels[i], w, h))
            .collect();
        if self.snap {
            self.snap_all(w, h);
        }
        self.initial = self.rects.clone();
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

    /// Fits every visible panel to the grid: each edge lands on the
    /// nearest cell boundary.
    fn snap_all(&mut self, w: f32, h: f32) {
        let cw = w / self.cols as f32;
        let ch = h / self.rows as f32;
        for i in 0..self.rects.len() {
            if !on_screen(&self.rects[i]) {
                continue;
            }
            let r = self.px_rect(i, w, h);
            let c0 = (r.x / cw).round().clamp(0.0, self.cols as f32 - 1.0);
            let c1 = ((r.right()) / cw).round().clamp(c0 + 1.0, self.cols as f32);
            let r0 = (r.y / ch).round().clamp(0.0, self.rows as f32 - 1.0);
            let r1 = ((r.bottom()) / ch).round().clamp(r0 + 1.0, self.rows as f32);
            let snapped =
                Rect::new(c0 * cw, r0 * ch, (c1 - c0) * cw, (r1 - r0) * ch);
            self.rects[i] = pct(snapped, w, h);
        }
    }

    fn px_rect(&self, i: usize, w: f32, h: f32) -> Rect {
        let p = &self.rects[i];
        Rect::new(p.x / 100.0 * w, p.y / 100.0 * h, p.w / 100.0 * w, p.h / 100.0 * h)
    }

    /// The edited layout in window pixels (drawn instead of the normal one).
    pub fn layout(&self, w: f32, h: f32) -> Layout {
        Layout { panels: (0..panel_count()).map(|i| self.px_rect(i, w, h)).collect() }
    }

    /// The edited layout as a percent spec for saving.
    pub fn spec(&self) -> LayoutSpec {
        LayoutSpec { panels: self.rects.clone() }
    }

    /// Panels whose rectangles differ from the given reference spec
    /// (with a small tolerance) — the "only the changes" save payload.
    pub fn changes_vs(&self, reference: &LayoutSpec) -> Vec<(Panel, PanelSpec)> {
        let mut out = Vec::new();
        for panel in Panel::all() {
            let a = &self.rects[panel.idx()];
            let b = reference.p(panel);
            let both_hidden = a.x >= 100.0 && b.x >= 100.0;
            let same = (a.x - b.x).abs() < 0.05
                && (a.y - b.y).abs() < 0.05
                && (a.w - b.w).abs() < 0.05
                && (a.h - b.h).abs() < 0.05;
            if !both_hidden && !same {
                out.push((panel, *a));
            }
        }
        out
    }

    /// Panels changed since the editor was opened.
    pub fn changes_since_start(&self) -> Vec<(Panel, PanelSpec)> {
        self.changes_vs(&LayoutSpec { panels: self.initial.clone() })
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
    /// editor is running; enabling snap auto-fits all panels.
    pub fn sync_prefs(&mut self, snap: bool, cols: u32, rows: u32, padding: f32, w: f32, h: f32) {
        let was = self.snap;
        self.cols = cols.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.rows = rows.clamp(crate::config::GRID_MIN, crate::config::GRID_MAX);
        self.padding = padding.max(0.0);
        self.snap = snap;
        if snap && !was {
            self.snap_all(w, h);
        }
    }

    /// Hidden panels offered by the ADD WIDGET window — the ones not
    /// placed here and not placed on any other board either.
    fn hidden_panels(&self) -> Vec<usize> {
        (0..self.rects.len())
            .filter(|&i| {
                !on_screen(&self.rects[i]) && !self.blocked.get(i).copied().unwrap_or(false)
            })
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
        let items = self.hidden_panels().len().max(1);
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

    /// Whether the panel can be removed from the grid. The control
    /// panel is the one widget that cannot be switched off — it is the
    /// only way back into the settings — so it never gets an X.
    fn removable(i: usize) -> bool {
        Panel(i as u16).name() != "control"
    }

    /// Topmost panel whose body or edge area contains the point,
    /// with the edge flags: (index, left, right, top, bottom).
    fn panel_at(&self, x: f32, y: f32, w: f32, h: f32) -> Option<(usize, bool, bool, bool, bool)> {
        let edge = grab_edge();
        for i in (0..self.rects.len()).rev() {
            if !on_screen(&self.rects[i]) {
                continue;
            }
            let r = self.px_rect(i, w, h);
            let outer = Rect::new(
                r.x - edge,
                r.y - edge,
                r.w + 2.0 * edge,
                r.h + 2.0 * edge,
            );
            if !outer.contains(x, y) {
                continue;
            }
            let l = (x - r.x).abs() <= edge;
            let rr = (x - r.right()).abs() <= edge;
            let t = (y - r.y).abs() <= edge;
            let b = (y - r.bottom()).abs() <= edge;
            if l || rr || t || b || r.contains(x, y) {
                return Some((i, l, rr, t, b));
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
        if let Some((i, mode)) = &self.drag {
            if self.rects[*i].x >= 100.0 {
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
        match self.panel_at(x, y, w, h) {
            Some((i, l, r, t, b)) => {
                let pr = self.px_rect(i, w, h);
                if Self::removable(i) && Self::x_rect(&pr).contains(x, y) {
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
            self.rects = self.initial.clone();
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
            let hidden = self.hidden_panels();
            for (slot, ir) in items.iter().enumerate() {
                if ir.contains(x, y) {
                    if let Some(&panel) = hidden.get(slot) {
                        self.adding = Some((panel, Instant::now()));
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
        if let Some((i, l, rr, t, b)) = self.panel_at(x, y, w, h) {
            let r = self.px_rect(i, w, h);
            // X in the top-right corner removes the widget from the grid.
            if Self::removable(i) && Self::x_rect(&r).contains(x, y) {
                self.rects[i] = OFF_SPEC;
                self.drag = None;
                nacelle::sound::emit(nacelle::sound::Event::Drop);
                return EditorHit::Handled;
            }
            if l || rr || t || b {
                self.drag = Some((i, Mode::Resize { l, r: rr, t, b }));
            } else {
                self.drag = Some((i, Mode::Move { dx: x - r.x, dy: y - r.y }));
            }
            nacelle::sound::emit(nacelle::sound::Event::Grab);
        }
        EditorHit::Handled
    }

    /// Mouse move while a panel is being dragged or resized.
    pub fn mouse_move(&mut self, x: f32, y: f32, w: f32, h: f32) {
        // Wandering far away from the held ADD WIDGET entry cancels the
        // hold (a generous margin — small drift while holding is fine).
        if let Some((panel, _)) = self.adding {
            let (_, items) = self.add_list_rects(w, h);
            let hidden = self.hidden_panels();
            let still = hidden
                .iter()
                .position(|&p| p == panel)
                .and_then(|slot| items.get(slot))
                .map(|ir| {
                    let m = 30.0;
                    Rect::new(ir.x - m, ir.y - m, ir.w + 2.0 * m, ir.h + 2.0 * m)
                        .contains(x, y)
                })
                .unwrap_or(false);
            if !still {
                self.adding = None;
            }
        }
        let Some((i, mode)) = &self.drag else { return };
        let i = *i;
        let cw = w / self.cols as f32;
        let ch = h / self.rows as f32;
        let r = self.px_rect(i, w, h);
        match mode {
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
                self.rects[i].x = nx / w * 100.0;
                self.rects[i].y = ny / h * 100.0;
            }
            Mode::Resize { l, r: rr, t, b } => {
                let (l, rr, t, b) = (*l, *rr, *t, *b);
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
                self.rects[i] = pct(Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)), w, h);
            }
        }
    }

    pub fn mouse_up(&mut self) {
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
        self.grow = None;
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

    /// Placement size of a freshly added widget.
    fn spawn_size(&self, w: f32, h: f32) -> (f32, f32) {
        let m = self.min_outer();
        if self.snap {
            ((w / self.cols as f32 * 3.0).max(m), (h / self.rows as f32 * 2.0).max(m))
        } else {
            static SW: OnceLock<TokenId> = OnceLock::new();
            static SH: OnceLock<TokenId> = OnceLock::new();
            let t = nacelle::theme::resolved();
            (
                (w * t.px(tok(&SW, "editor.spawn_w_frac")).clamp(0.0, 1.0)).max(m),
                (h * t.px(tok(&SH, "editor.spawn_h_frac")).clamp(0.0, 1.0)).max(m),
            )
        }
    }

    /// Draws just the editor's button stack — called from draw() and
    /// again ON TOP of the settings window when it is open over the
    /// editor, so the buttons share the window's plane.
    pub fn draw_buttons(&mut self, ctx: &mut Ctx) {
        let (w, h) = (ctx.w, ctx.h);
        let (mx, my) = ctx.mouse;
        let now = Instant::now();
        let btns = Self::save_buttons(w, h);
        let labels = ["SETTINGS", "ADD WIDGET", "SAVE", "SAVE AS", "CANCEL", "EXIT"];
        static PRESS_MS: OnceLock<TokenId> = OnceLock::new();
        let press_ms = theme::resolved().px(tok(&PRESS_MS, "motion.press.duration_ms"));
        for (i, br) in btns.iter().enumerate() {
            let hover = !self.add_open && self.naming.is_none() && br.contains(mx, my);
            let flash = self
                .flash
                .map(|(fi, t)| {
                    fi == i && now.duration_since(t).as_secs_f32() * 1000.0 < press_ms
                })
                .unwrap_or(false);
            Self::draw_button(ctx, br, labels[i], hover, flash);
        }
    }

    /// Draws the visible grid, panel outlines and the editor controls on
    /// top of the live interface. The `mini` callback draws a live
    /// miniature of the given panel into a rectangle (used by the ADD
    /// WIDGET window). Also advances the ADD WIDGET hold — after the
    /// themed hold time the widget pulls out of the window, grows and
    /// follows the cursor.
    pub fn draw<F: FnMut(&mut Ctx, usize, Rect)>(&mut self, ctx: &mut Ctx, mut mini: F) {
        let t = theme::resolved();
        let (w, h) = (ctx.w, ctx.h);
        let (mx, my) = ctx.mouse;

        // ADD WIDGET hold finished -> the widget pulls out of the window
        // (it starts at its miniature size and grows under the cursor).
        if let Some((panel, t0)) = self.adding {
            if t0.elapsed().as_secs_f32() >= hold_secs() {
                let (_, items) = self.add_list_rects(w, h);
                let slot = self.hidden_panels().iter().position(|&p| p == panel);
                let (mw, mh) = slot
                    .and_then(|s| items.get(s))
                    .map(|ir| (ir.w, ir.h))
                    .unwrap_or_else(|| {
                // The degenerate path reuses a themed size instead of
                // carrying a design of its own (governing principle).
                let m = self.min_outer();
                (m, m)
            });
                self.adding = None;
                self.add_open = false;
                let r = Rect::new(
                    (mx - mw / 2.0).clamp(0.0, (w - mw).max(0.0)),
                    (my - mh / 2.0).clamp(0.0, (h - mh).max(0.0)),
                    mw,
                    mh,
                );
                self.rects[panel] = pct(r, w, h);
                self.drag = Some((panel, Mode::Move { dx: mw / 2.0, dy: mh / 2.0 }));
                self.grow = Some((panel, Instant::now(), mw, mh));
            }
        }

        // Growth animation: miniature -> placement size, centred on the
        // cursor while it is being dragged.
        if let Some((panel, t0, mw, mh)) = self.grow {
            static GROW_MS: OnceLock<TokenId> = OnceLock::new();
            static GROW_EASE: OnceLock<TokenId> = OnceLock::new();
            static EASE_WORDS: OnceLock<[Option<u16>; 3]> = OnceLock::new();
            let dur = (t.px(tok(&GROW_MS, "motion.widget_grow.duration_ms")) / 1000.0).max(0.001);
            let x = (t0.elapsed().as_secs_f32() / dur).min(1.0);
            // The declared curve; an unknown word runs linear (the raw look).
            let ease = tok(&GROW_EASE, "motion.widget_grow.easing");
            let words = EASE_WORDS.get_or_init(|| {
                [
                    theme::enum_index(ease, "ease_out"),
                    theme::enum_index(ease, "ease_in"),
                    theme::enum_index(ease, "ease_in_out"),
                ]
            });
            let word = Some(t.enum_of(ease));
            let e = if word == words[0] {
                1.0 - (1.0 - x) * (1.0 - x)
            } else if word == words[1] {
                x * x
            } else if word == words[2] {
                x * x * (3.0 - 2.0 * x)
            } else {
                x
            };
            let t01 = x;
            let (tw, th) = self.spawn_size(w, h);
            let (cw_, ch_) = (mw + (tw - mw) * e, mh + (th - mh) * e);
            let r = self.px_rect(panel, w, h);
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            let nr = Rect::new(
                (cx - cw_ / 2.0).clamp(0.0, (w - cw_).max(0.0)),
                (cy - ch_ / 2.0).clamp(0.0, (h - ch_).max(0.0)),
                cw_,
                ch_,
            );
            self.rects[panel] = pct(nr, w, h);
            if let Some((di, mode)) = self.drag.as_mut() {
                if *di == panel {
                    if let Mode::Move { dx, dy } = mode {
                        *dx = cw_ / 2.0;
                        *dy = ch_ / 2.0;
                    }
                }
            }
            if t01 >= 1.0 {
                self.grow = None;
                if self.snap && self.drag.is_none() {
                    self.snap_all(w, h);
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
        static PLATE: OnceLock<TokenId> = OnceLock::new();
        static PLATE_PAD: OnceLock<TokenId> = OnceLock::new();
        static PLATE_H: OnceLock<TokenId> = OnceLock::new();
        static TAG_X: OnceLock<TokenId> = OnceLock::new();
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
        let dragged = self.drag.as_ref().map(|(i, _)| *i);
        for i in 0..self.rects.len() {
            if !on_screen(&self.rects[i]) {
                continue;
            }
            let r = self.px_rect(i, w, h);
            let hot = dragged == Some(i)
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
            let px = ROLE_LABEL.px(ctx);
            let track = ROLE_LABEL.tracking(px);
            let label = Panel(i as u16).label();
            let tw = ctx.fonts.measure(FONT_UI, px, label, track);
            let plate_h = t.px(tok(&PLATE_H, "editor.proxy.label_h"));
            ctx.dl.rect(
                r.x,
                r.y,
                tw + t.px(tok(&PLATE_PAD, "editor.proxy.label_pad_x")),
                plate_h,
                col(t.color(tok(&PLATE, "component.nameplate.fill"))),
            );
            ctx.dl.text(
                ctx.fonts,
                FONT_UI,
                px,
                r.x + t.px(tok(&TAG_X, "panel.title.inset_x")),
                r.y + (plate_h - px) / 2.0,
                label,
                col(st.text),
                track,
            );
            // X in the top-right corner removes the widget from the grid
            // — except on the panels that cannot be switched off.
            if Self::removable(i) {
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
        let hint_px = ROLE_HINT.px(ctx);
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            hint_px,
            t.px(tok(&HINT_X, "editor.hint.inset_x")),
            h - t.px(tok(&HINT_Y, "editor.hint.inset_y")),
            "DRAG TO MOVE \u{2014} DRAG EDGES TO RESIZE \u{2014} X REMOVES \u{2014} ESC EXITS WITHOUT SAVING",
            col(t.color(tok(&HINT_C, "text.muted"))),
            ROLE_HINT.tracking(hint_px),
        );

        // ADD WIDGET list window (opaque).
        if self.add_open {
            static SCRIM: OnceLock<TokenId> = OnceLock::new();
            static TITLE_C: OnceLock<TokenId> = OnceLock::new();
            static EMPTY_C: OnceLock<TokenId> = OnceLock::new();
            static TILE_CLASS: OnceLock<Option<u16>> = OnceLock::new();
            static HOLD_FILL: OnceLock<TokenId> = OnceLock::new();
            static HEAD: OnceLock<TokenId> = OnceLock::new();
            static MINI_PAD: OnceLock<TokenId> = OnceLock::new();
            static PLATE2: OnceLock<TokenId> = OnceLock::new();
            static PLATE2_PAD: OnceLock<TokenId> = OnceLock::new();
            static PLATE2_H: OnceLock<TokenId> = OnceLock::new();
            static TAG2_X: OnceLock<TokenId> = OnceLock::new();
            static RING_W: OnceLock<TokenId> = OnceLock::new();
            let (win, items) = self.add_list_rects(w, h);
            nacelle::object::window::backdrop(ctx, t.px(tok(&SCRIM, "modal.scrim_alpha")));
            nacelle::object::window::frame(ctx, win);
            let tpx = ROLE_TITLE.px(ctx);
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                tpx,
                win.cx(),
                win.y + list_pad(),
                &format!(
                    "ADD WIDGET \u{2014} HOLD {:.0}S TO PLACE",
                    hold_secs().ceil()
                ),
                col(t.color(tok(&TITLE_C, "text.title"))),
                ROLE_TITLE.tracking(tpx),
            );
            let hidden = self.hidden_panels();
            if hidden.is_empty() {
                let px = ROLE_VALUE.px(ctx);
                ctx.dl.text_center(
                    ctx.fonts,
                    FONT_UI,
                    px,
                    win.cx(),
                    win.y + win.h / 2.0,
                    "ALL WIDGETS ARE PLACED",
                    col(t.color(tok(&EMPTY_C, "text.muted"))),
                    ROLE_VALUE.tracking(px),
                );
            }
            let tile_class = *TILE_CLASS.get_or_init(|| theme::class_id("tile"));
            let tile = |hot: bool| match tile_class {
                Some(c) => t.class_state(c, if hot { State::Hover } else { State::Idle }),
                None => StateStyle::RAW,
            };
            for (slot, ir) in items.iter().enumerate() {
                let Some(&panel) = hidden.get(slot) else { break };
                let held = self.adding.map(|(p, _)| p == panel).unwrap_or(false);
                let hover = ir.contains(mx, my);
                let st = tile(hover || held);
                ctx.dl.rect(ir.x, ir.y, ir.w, ir.h, col(tile(false).fill));
                // Live miniature of the widget (headers drawn above the
                // rect by some widgets get a little headroom).
                let head = t.px(tok(&HEAD, "editor.list.preview_head"));
                let m = t.px(tok(&MINI_PAD, "editor.list.pad_min_px"));
                mini(
                    ctx,
                    panel,
                    Rect::new(
                        ir.x + m,
                        ir.y + m + head,
                        ir.w - 2.0 * m,
                        (ir.h - 2.0 * m - head).max(10.0),
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
                // Small name tag like on the panels.
                let px = ROLE_LABEL.px(ctx);
                let track = ROLE_LABEL.tracking(px);
                let tw =
                    ctx.fonts.measure(FONT_UI, px, Panel(panel as u16).label(), track);
                let plate_h = t.px(tok(&PLATE2_H, "editor.proxy.label_h"));
                ctx.dl.rect(
                    ir.x,
                    ir.y,
                    tw + t.px(tok(&PLATE2_PAD, "editor.proxy.label_pad_x")),
                    plate_h,
                    col(t.color(tok(&PLATE2, "component.nameplate.fill"))),
                );
                ctx.dl.text(
                    ctx.fonts,
                    FONT_UI,
                    px,
                    ir.x + t.px(tok(&TAG2_X, "panel.title.inset_x")),
                    ir.y + (plate_h - px) / 2.0,
                    Panel(panel as u16).label(),
                    col(st.text),
                    track,
                );
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
            static BAND_H: OnceLock<TokenId> = OnceLock::new();
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
            let tpx = ROLE_TITLE.px(ctx);
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                tpx,
                bx + bw / 2.0,
                by + (t.px(tok(&BAND_H, "modal.title.band_h")) - tpx) / 2.0,
                "SAVE AS \u{2014} TYPE A NAME",
                col(t.color(tok(&TITLE_C, "text.title"))),
                ROLE_TITLE.tracking(tpx),
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
            let hpx = ROLE_LABEL.px(ctx);
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                hpx,
                bx + bw / 2.0,
                field.bottom() + t.px(tok(&HINT_INSET, "settings.hint_inset")),
                "ENTER SAVES \u{2014} ESC CANCELS",
                col(t.color(tok(&HINT_C, "text.muted"))),
                ROLE_LABEL.tracking(hpx),
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
