//! Modal settings window (centered). Main view: CLOSE + THEMES. The THEMES
//! view is a submenu with LOOK (the theme engine's themes), LAYAUTS
//! (layouts) and SOUNDS (sound themes). A theme comes from the toolkit — the
//! eight compiled in plus anything installed on the search path — and is
//! written as Theme=; layouts and sound sets are read from the data
//! directories and written as Layaut= / Sounds=. Everything applies live.

use super::{Ctx, PanelSpec, Rect};
use crate::config::{self, GRID_MAX, GRID_MIN};
use crate::font::FONT_UI;
use nacelle::focus::{Caps, FocusCtl, FocusId, Key as FKey, KeyEv, Nav};
use nacelle::theme::bake::StateStyle;
use nacelle::theme::parse::State;
use nacelle::theme::{self, TokenId};
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Menu,
    Themes,
    Look,
    Layauts,
    Sounds,
    Font,
    Grid,
    Sound,
    Boards,
    Color,
    Blur,
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Close,
    Back,
    OpenThemes,
    OpenLook,
    OpenLayauts,
    OpenSounds,
    Look(usize),
    Layaut(usize),
    Sounds(usize),
    OpenFont,
    OpenGrid,
    OpenSound,
    OpenBoards,
    OpenColor,
    OpenBlur,
    /// LAYAUTS view: clear the pinned per-screen section of the
    /// selected layout for the screen the window is on (u1 §5.3's way
    /// out of a stale pinned arrangement).
    ResetScreen,
    BlurRadiusTrack,
    BlurOpacityTrack,
    ColorDepth(u32),
    ColorSpaceNext,
    ColorLutNext,
    ColorIccNext,
    BoardGo((i32, i32)),
    BoardAdd(i8),
    BoardDel((i32, i32)),
    VolumeTrack,
    ToggleTyping,
    ToggleAmbient,
    ToggleSnap,
    ColsTrack,
    RowsTrack,
    PadTrack,
    EditGrid,
    SizeTrack(Sect),
    FamilyBtn(Sect),
    WeightBtn(Sect),
    FamilyPick(Sect, usize),
    WeightPick(Sect, usize),
}

/// Font section: terminal or the rest of the interface.
#[derive(Clone, Copy, PartialEq)]
enum Sect {
    Term,
    Ui,
}

#[derive(Clone, Copy, PartialEq)]
enum Dropdown {
    Family(Sect),
    Weight(Sect),
}

/// What a key did to the window — the settings layer's answer to the
/// application router (F1 §1.5).
pub enum KeyOut {
    /// Not this window's key: the application decides (its Escape
    /// stays the layer's close, exactly as before focus existed).
    Ignored,
    /// Consumed: navigation moved, a control acted, a dropdown closed.
    Consumed,
    /// Consumed AND the configuration changed — the caller re-resolves
    /// and applies it, exactly like a click that returned true.
    Changed,
}

/// The focus-chain identity of every control this window registers —
/// STABLE paths (F1 §1.1), never positions in a frame's layout, so
/// focus survives a redraw. List rows derive `.item(i)` from their
/// list's id: a row's order is its content's order, which is the one
/// place an index is legal.
fn focus_id(act: Act) -> FocusId {
    use Act::*;
    fn sect_path(s: Sect, term: &'static str, ui: &'static str) -> FocusId {
        FocusId::of(match s {
            Sect::Term => term,
            Sect::Ui => ui,
        })
    }
    // A board key's two signed coordinates, chained as derived ids —
    // deterministic and collision-free without encoding geometry.
    fn board_id(path: &str, k: (i32, i32)) -> FocusId {
        FocusId::of(path).item(k.0 as usize).item(k.1 as usize)
    }
    match act {
        Close => FocusId::of("settings.close"),
        Back => FocusId::of("settings.back"),
        OpenThemes => FocusId::of("settings.menu.themes"),
        OpenFont => FocusId::of("settings.menu.font"),
        OpenSound => FocusId::of("settings.menu.sound"),
        OpenGrid => FocusId::of("settings.menu.grid"),
        OpenBoards => FocusId::of("settings.menu.boards"),
        OpenColor => FocusId::of("settings.menu.color"),
        OpenBlur => FocusId::of("settings.menu.blur"),
        OpenLook => FocusId::of("settings.themes.look"),
        OpenLayauts => FocusId::of("settings.themes.layauts"),
        OpenSounds => FocusId::of("settings.themes.sounds"),
        Look(i) => FocusId::of("settings.look.item").item(i),
        Layaut(i) => FocusId::of("settings.layauts.item").item(i),
        Sounds(i) => FocusId::of("settings.sounds.item").item(i),
        ResetScreen => FocusId::of("settings.layauts.reset_screen"),
        BlurRadiusTrack => FocusId::of("settings.blur.radius"),
        BlurOpacityTrack => FocusId::of("settings.blur.opacity"),
        ColorDepth(bits) => FocusId::of("settings.color.depth").item(bits as usize),
        ColorSpaceNext => FocusId::of("settings.color.space"),
        ColorLutNext => FocusId::of("settings.color.lut"),
        ColorIccNext => FocusId::of("settings.color.icc"),
        BoardGo(k) => board_id("settings.boards.go", k),
        BoardAdd(side) => FocusId::of(if side < 0 {
            "settings.boards.add_left"
        } else {
            "settings.boards.add_right"
        }),
        BoardDel(k) => board_id("settings.boards.del", k),
        VolumeTrack => FocusId::of("settings.sound.volume"),
        ToggleTyping => FocusId::of("settings.sound.typing"),
        ToggleAmbient => FocusId::of("settings.sound.ambient"),
        ToggleSnap => FocusId::of("settings.grid.snap"),
        ColsTrack => FocusId::of("settings.grid.cols"),
        RowsTrack => FocusId::of("settings.grid.rows"),
        PadTrack => FocusId::of("settings.grid.pad"),
        EditGrid => FocusId::of("settings.grid.edit"),
        SizeTrack(s) => sect_path(s, "settings.font.term.size", "settings.font.ui.size"),
        FamilyBtn(s) => {
            sect_path(s, "settings.font.term.family", "settings.font.ui.family")
        }
        WeightBtn(s) => {
            sect_path(s, "settings.font.term.weight", "settings.font.ui.weight")
        }
        // Dropdown rows carry the ids the accordion object derives
        // itself (`base.item(i)`, dropdown.rs) — one derivation on both
        // sides, so Enter and click can never disagree about a row.
        FamilyPick(s, i) => dropdown_base(Dropdown::Family(s)).item(i),
        WeightPick(s, i) => dropdown_base(Dropdown::Weight(s)).item(i),
    }
}

/// The id an OPEN dropdown's rows derive from — distinct from the
/// anchor button's own id (the button stays in the chain while its
/// list is open).
fn dropdown_base(d: Dropdown) -> FocusId {
    FocusId::of(match d {
        Dropdown::Family(Sect::Term) => "settings.font.term.family.list",
        Dropdown::Family(Sect::Ui) => "settings.font.ui.family.list",
        Dropdown::Weight(Sect::Term) => "settings.font.term.weight.list",
        Dropdown::Weight(Sect::Ui) => "settings.font.ui.weight.list",
    })
}

/// [`Settings::hit`]'s working part, over the hit map alone so a loop
/// that borrows another field of the window (the boards walk) can
/// still register its rects.
fn hit_into(hits: &mut Vec<(Rect, Act)>, ctx: &mut Ctx, r: Rect, act: Act) {
    let ring = ctx
        .focus
        .as_deref_mut()
        .map_or(false, |fc| fc.register(focus_id(act), r, Caps::NONE).ring);
    if ring {
        nacelle::object::focus_ring::draw(ctx, r);
    }
    hits.push((r, act));
}

/// The slider tracks — the acts whose keyboard is Left/Right, never
/// Enter: a synthetic press at a track's centre would SET the value
/// to the centre, which no keyboard user asked for.
fn is_track(act: Act) -> bool {
    matches!(
        act,
        Act::BlurRadiusTrack
            | Act::BlurOpacityTrack
            | Act::VolumeTrack
            | Act::ColsTrack
            | Act::RowsTrack
            | Act::PadTrack
            | Act::SizeTrack(_)
    )
}

/// One board, as the settings window sees it: enough to draw a
/// miniature and to know whether clicking it means going somewhere.
pub struct BoardThumb {
    pub id: (i32, i32),
    pub current: bool,
    /// Visible panels in percent of the window.
    pub panels: Vec<PanelSpec>,
}

/// What the boards view asked the application to do. The window cannot
/// switch boards or edit their arrangement itself — boards are the
/// application's, so it asks, exactly the way widgets ask.
pub enum BoardAction {
    Go((i32, i32)),
    /// Grow the horizontal row on this side: negative left, positive
    /// right. The top and bottom boards are fixtures — nothing to add.
    Add(i8),
    Del((i32, i32)),
}

/// Weight options offered in the WEIGHT dropdown.
const WEIGHTS: [&str; 5] = ["Light", "Regular", "Medium", "SemiBold", "Bold"];

// ------------------------------------------------------------- theme access

/// Token ids resolve once and live for the process. A missing name degrades
/// through the engine's per-kind fallback, never through a constant kept here.
fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

/// The legacy draw-list colour from an engine colour.
fn col(c: theme::ThemeColor) -> nacelle::theme::Color {
    nacelle::theme::Color { r: c.r, g: c.g, b: c.b, a: c.a }
}

/// The baked state ladder of one interaction class; RAW when no theme
/// declares the class.
fn ladder(
    th: &theme::ResolvedTheme,
    cell: &'static OnceLock<Option<u16>>,
    name: &'static str,
    state: State,
) -> StateStyle {
    match *cell.get_or_init(|| theme::class_id(name)) {
        Some(c) => th.class_state(c, state),
        None => StateStyle::RAW,
    }
}

// The three ladders this window draws from.
static BTN_CLASS: OnceLock<Option<u16>> = OnceLock::new();
static CHIP_CLASS: OnceLock<Option<u16>> = OnceLock::new();
static TILE_CLASS: OnceLock<Option<u16>> = OnceLock::new();

// Cells for the row grammar every view repeats: label/value columns,
// per-component row heights, the section break under the BACK row.
static LABEL_FG: OnceLock<TokenId> = OnceLock::new();
static VALUE_FG: OnceLock<TokenId> = OnceLock::new();
static MUTED_FG: OnceLock<TokenId> = OnceLock::new();
static SECTION_GAP: OnceLock<TokenId> = OnceLock::new();
static CHECK_ROW_H: OnceLock<TokenId> = OnceLock::new();
static SLIDER_ROW_H: OnceLock<TokenId> = OnceLock::new();
static LABEL_PAD: OnceLock<TokenId> = OnceLock::new();
static VALUE_GUTTER: OnceLock<TokenId> = OnceLock::new();
static LIST_W_FRAC: OnceLock<TokenId> = OnceLock::new();

/// One fixed type role, resolved: size, letter spacing and line height in px.
/// The role *indirection* tokens (`settings.row_label_role`,
/// `modal.title.role`, …) name these roles, but no text primitive takes a
/// role yet, so the mapped role is read directly.
#[derive(Clone, Copy)]
struct TypeRole {
    px: f32,
    track: f32,
    line: f32,
}

fn type_role(
    th: &theme::ResolvedTheme,
    cells: &'static [OnceLock<TokenId>; 3],
    size: &'static str,
    track: &'static str,
    lead: &'static str,
) -> TypeRole {
    let px = th.px(tok(&cells[0], size));
    TypeRole {
        px,
        track: px * th.px(tok(&cells[1], track)),
        line: px * th.px(tok(&cells[2], lead)),
    }
}

macro_rules! role_fn {
    ($fn_name:ident, $role:literal) => {
        fn $fn_name(th: &theme::ResolvedTheme) -> TypeRole {
            static C: [OnceLock<TokenId>; 3] =
                [OnceLock::new(), OnceLock::new(), OnceLock::new()];
            type_role(
                th,
                &C,
                concat!("type.", $role, ".size"),
                concat!("type.", $role, ".tracking"),
                concat!("type.", $role, ".leading"),
            )
        }
    };
}

role_fn!(role_body, "body"); // settings.row_label_role
role_fn!(role_value, "value"); // component.columns values
role_fn!(role_caption, "caption"); // settings.hint.role / settings.note.role / boards.tile.caption_role
role_fn!(role_button, "button"); // button labels
role_fn!(role_title, "title.window"); // modal.title.role
role_fn!(role_section, "label.section"); // the FONT view's section headers

pub struct Settings {
    pub open: bool,
    view: View,
    /// The engine's theme names, for the THEMES list.
    themes: Vec<String>,
    layauts: Vec<String>,
    sounds: Vec<String>,
    /// Current selections from nacelle-desktop.conf (highlighted in the lists).
    current_look: Option<String>,
    current_layaut: Option<String>,
    current_sounds: Option<String>,
    /// Font view state, indexed by section (0 = Term, 1 = Ui).
    families: [Vec<String>; 2],
    cur_family: [Option<String>; 2],
    cur_weight: [Option<String>; 2],
    /// Font sizes in percent (50-200).
    cur_size: [u32; 2],
    dragging_size: Option<Sect>,
    slider_rect: [Rect; 2],
    dropdown: Option<Dropdown>,
    /// When the dropdown was opened — drives the accordion animation.
    dropdown_since: Option<Instant>,
    /// Grid editor preferences (GRID view).
    grid_snap: bool,
    grid_cols: u32,
    grid_rows: u32,
    /// Widget padding in px (0-40) + its slider state.
    grid_pad: u32,
    dragging_pad: bool,
    pad_rect: Rect,
    /// The two grid sliders.
    dragging_cols: bool,
    dragging_rows: bool,
    cols_rect: Rect,
    rows_rect: Rect,
    /// SOUND view: master volume 0-100 and the two mute switches.
    sound_volume: u32,
    sound_typing: bool,
    sound_ambient: bool,
    dragging_volume: bool,
    volume_rect: Rect,
    /// BLUR view: radius and opacity in percent, with their tracks.
    blur_radius: u32,
    blur_opacity: u32,
    dragging_blur: [bool; 2],
    blur_rect: [Rect; 2],
    /// Set to true whenever a sound preference changed, so main can
    /// push it straight to the audio output.
    pub sound_dirty: bool,
    /// Set by EDIT GRID — main enters the layout editor and clears it.
    pub edit_requested: bool,
    /// Set by RESET THIS SCREEN (the LAYAUTS view) — main clears the
    /// pinned [WxH@D] section of the selected layout for the screen it
    /// is on, then re-applies the configuration. The window itself
    /// cannot: only the application knows which screen this is.
    pub reset_screen: bool,
    /// The boards, fed by the application every frame the window is
    /// open; drawn by the BOARDS view.
    pub boards: Vec<BoardThumb>,
    /// Whether the COLOR view may be entered at all: true only in a
    /// native Wayland session where the compositor speaks the Color
    /// Management protocol. Everywhere else the button is a grey
    /// inscription and the stored preferences are ignored.
    pub color_enabled: bool,
    /// The COLOR view changed something; the application applies it.
    pub color_dirty: bool,
    /// The BLUR sliders moved; main re-reads blur_settings().
    pub blur_dirty: bool,
    color_depth: u32,
    color_space: String,
    color_lut: Option<String>,
    color_icc: Option<String>,
    color_luts: Vec<String>,
    color_iccs: Vec<String>,
    /// What the BOARDS view asked for; the application consumes it.
    pub board_action: Option<BoardAction>,
    hits: Vec<(Rect, Act)>,
    flash: Option<(Act, Instant)>,
}

/// Modal window rectangle.
fn modal_rect(w: f32, h: f32) -> Rect {
    static W_FRAC: OnceLock<TokenId> = OnceLock::new();
    static H_FRAC: OnceLock<TokenId> = OnceLock::new();
    static MIN_W: OnceLock<TokenId> = OnceLock::new();
    static MIN_W_PX: OnceLock<TokenId> = OnceLock::new();
    static MIN_H: OnceLock<TokenId> = OnceLock::new();
    static MIN_H_PX: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let mw = (w * th.px(tok(&W_FRAC, "modal.w_frac")))
        .max(th.px(tok(&MIN_W, "modal.min_w")))
        .max(th.px(tok(&MIN_W_PX, "modal.min_w_min_px")));
    let mh = (h * th.px(tok(&H_FRAC, "modal.h_frac")))
        .max(th.px(tok(&MIN_H, "modal.min_h")))
        .max(th.px(tok(&MIN_H_PX, "modal.min_h_min_px")));
    Rect::new((w - mw) / 2.0, (h - mh) / 2.0, mw, mh)
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            open: false,
            view: View::Menu,
            themes: Vec::new(),
            layauts: Vec::new(),
            sounds: Vec::new(),
            current_look: None,
            current_layaut: None,
            current_sounds: None,
            families: [Vec::new(), Vec::new()],
            cur_family: [None, None],
            cur_weight: [None, None],
            cur_size: [100, 100],
            dragging_size: None,
            slider_rect: [Rect::new(0.0, 0.0, 0.0, 0.0); 2],
            dropdown: None,
            dropdown_since: None,
            grid_snap: false,
            grid_cols: GRID_MIN,
            grid_rows: GRID_MIN,
            grid_pad: 8,
            dragging_pad: false,
            pad_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            dragging_cols: false,
            dragging_rows: false,
            cols_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            rows_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            sound_volume: 100,
            sound_typing: true,
            sound_ambient: true,
            dragging_volume: false,
            volume_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            blur_radius: 100,
            blur_opacity: 100,
            dragging_blur: [false; 2],
            blur_rect: [Rect::new(0.0, 0.0, 0.0, 0.0); 2],
            sound_dirty: false,
            edit_requested: false,
            reset_screen: false,
            boards: Vec::new(),
            color_enabled: false,
            color_dirty: false,
            blur_dirty: false,
            color_depth: 8,
            color_space: "auto".to_string(),
            color_lut: None,
            color_icc: None,
            color_luts: Vec::new(),
            color_iccs: Vec::new(),
            board_action: None,
            hits: Vec::new(),
            flash: None,
        }
    }

    fn sect_idx(sect: Sect) -> usize {
        match sect {
            Sect::Term => 0,
            Sect::Ui => 1,
        }
    }

    /// Slider range per section: terminal 50-200%, interface 30-125%.
    /// The interface starts at 30% so a big screen can have a small
    /// interface — 75% was as low as it went, which on a 4K panel was
    /// still larger than anyone wanted.
    fn size_range(sect: Sect) -> (f32, f32) {
        match sect {
            Sect::Term => (50.0, 200.0),
            Sect::Ui => (30.0, 125.0),
        }
    }

    fn set_size_from_x(&mut self, sect: Sect, x: f32) {
        let i = Self::sect_idx(sect);
        let (min, max) = Self::size_range(sect);
        let track = self.slider_rect[i];
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        self.cur_size[i] = (min + t * (max - min)).round() as u32;
    }

    fn set_volume_from_x(&mut self, x: f32) {
        let track = self.volume_rect;
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        self.sound_volume = (t * 100.0).round() as u32;
    }

    fn set_pad_from_x(&mut self, x: f32) {
        let track = self.pad_rect;
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        self.grid_pad = (t * 40.0).round() as u32;
    }

    /// Cells from a position on one of the grid tracks.
    fn cells_from_x(track: Rect, x: f32) -> u32 {
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        GRID_MIN + (t * (GRID_MAX - GRID_MIN) as f32).round() as u32
    }

    /// Mouse move while dragging a size slider.
    pub fn drag(&mut self, x: f32) {
        if let Some(sect) = self.dragging_size {
            self.set_size_from_x(sect, x);
        }
        if self.dragging_pad {
            self.set_pad_from_x(x);
        }
        if self.dragging_cols {
            self.grid_cols = Self::cells_from_x(self.cols_rect, x);
        }
        if self.dragging_rows {
            self.grid_rows = Self::cells_from_x(self.rows_rect, x);
        }
        if self.dragging_volume {
            self.set_volume_from_x(x);
            self.sound_dirty = true;
        }
        for i in 0..2 {
            if self.dragging_blur[i] {
                self.set_blur_from_x(i, x);
                self.blur_dirty = true;
            }
        }
    }

    fn set_blur_from_x(&mut self, i: usize, x: f32) {
        let track = self.blur_rect[i];
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        let v = (t * 100.0).round() as u32;
        if i == 0 {
            self.blur_radius = v;
        } else {
            self.blur_opacity = v;
        }
    }

    /// Current frosted-glass preferences, for main to apply.
    pub fn blur_settings(&self) -> (u32, u32) {
        (self.blur_radius, self.blur_opacity)
    }

    /// Live widget padding while the GRID view is open — applied every
    /// frame so dragging the PADDING slider works immediately.
    pub fn live_padding(&self) -> Option<u32> {
        if self.open && self.view == View::Grid {
            Some(self.grid_pad)
        } else {
            None
        }
    }

    /// Current sound preferences, for main to hand to the audio output.
    pub fn sound_settings(&self) -> (f32, bool, bool) {
        (
            self.sound_volume as f32 / 100.0,
            self.sound_typing,
            self.sound_ambient,
        )
    }

    /// Live font scales for the sliders in the FONT view — applied every
    /// frame so dragging changes the size smoothly, not on release.
    pub fn live_scales(&self) -> Option<(f32, f32)> {
        if self.open && self.view == View::Font {
            Some((
                self.cur_size[0] as f32 / 100.0,
                self.cur_size[1] as f32 / 100.0,
            ))
        } else {
            None
        }
    }

    /// Mouse button released; returns true when the configuration changed.
    pub fn release(&mut self) -> bool {
        if self.dragging_volume {
            self.dragging_volume = false;
            config::set_sound_volume(self.sound_volume);
        }
        if self.dragging_blur[0] {
            self.dragging_blur[0] = false;
            config::set_blur_radius(self.blur_radius);
        }
        if self.dragging_blur[1] {
            self.dragging_blur[1] = false;
            config::set_blur_opacity(self.blur_opacity);
        }
        if self.dragging_pad {
            self.dragging_pad = false;
            config::set_grid_padding(self.grid_pad);
        }
        if self.dragging_cols {
            self.dragging_cols = false;
            config::set_grid_cols(self.grid_cols);
        }
        if self.dragging_rows {
            self.dragging_rows = false;
            config::set_grid_rows(self.grid_rows);
        }
        if let Some(sect) = self.dragging_size.take() {
            let i = Self::sect_idx(sect);
            match sect {
                Sect::Term => config::set_term_font_size(self.cur_size[i]),
                Sect::Ui => config::set_ui_font_size(self.cur_size[i]),
            }
            return true;
        }
        false
    }

    pub fn show(&mut self) {
        self.open = true;
        self.view = View::Menu;
        nacelle::sound::emit(nacelle::sound::Event::PanelOpen);
    }

    /// Opens the settings window straight at the GRID view — used by the
    /// layout editor's SETTINGS button and its CANCEL return path.
    pub fn show_grid(&mut self) {
        self.open = true;
        let (snap, cols, rows, pad) = config::grid_prefs();
        self.grid_snap = snap;
        self.grid_cols = cols;
        self.grid_rows = rows;
        self.grid_pad = pad;
        self.view = View::Grid;
        nacelle::sound::emit(nacelle::sound::Event::PanelOpen);
    }

    pub fn close(&mut self) {
        self.open = false;
        nacelle::sound::emit(nacelle::sound::Event::PanelClose);
    }

    /// Whether the cursor is over an interactive element of the window.
    pub fn hover(&self, x: f32, y: f32) -> bool {
        self.hits.iter().any(|(r, _)| r.contains(x, y))
    }

    /// Click handling. Returns true when the configuration changed
    /// (the caller should re-resolve and apply it). The chain follows
    /// the pointer — typing-by-Tab continues from what was clicked —
    /// but a click never summons the ring ([`FocusCtl::focus`]).
    pub fn click(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fc: Option<&mut FocusCtl>,
    ) -> bool {
        if !self.open {
            return false;
        }
        // Topmost element wins (dropdown items are drawn last). Elements
        // are checked BEFORE the window bounds, so dropdown items that
        // extend past the window edge remain clickable.
        let act = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|&(_, a)| a);
        let Some(act) = act else {
            // No element hit: swallow the click; a click inside the
            // window closes an open dropdown.
            if modal_rect(w, h).contains(x, y) {
                self.dropdown = None;
            }
            return false;
        };
        if let Some(fc) = fc {
            fc.focus(Some(focus_id(act)));
        }
        self.perform(act, x)
    }

    /// The body every activation runs — mouse ([`Settings::click`])
    /// and keyboard ([`Settings::key`]) share it, so the two ways of
    /// pressing a control cannot drift apart (F1 §1.5). `x` is where
    /// along a slider track the press landed; buttons ignore it.
    fn perform(&mut self, act: Act, x: f32) -> bool {
        self.flash = Some((act, Instant::now()));
        // Every button clicks; the actions below that mean more than a
        // plain press replace it with their own sound.
        use nacelle::sound::{emit, Event as Sfx};
        match act {
            Act::Close | Act::Back => {}
            Act::ToggleSnap | Act::ToggleTyping | Act::ToggleAmbient => {}
            Act::VolumeTrack => {}
            Act::Look(_) | Act::Layaut(_) | Act::Sounds(_) => {}
            _ => emit(Sfx::Click),
        }
        match act {
            Act::Close => {
                self.open = false;
                emit(Sfx::PanelClose);
            }
            Act::Back => {
                emit(Sfx::Click);
                self.dropdown = None;
                self.view = match self.view {
                    View::Look | View::Layauts | View::Sounds => View::Themes,
                    _ => View::Menu,
                }
            }
            Act::OpenThemes => self.view = View::Themes,
            Act::OpenLook => {
                // Scanned when the view is opened.
                // The engine's themes, not the look/ directories: a look
                // bundled a stylesheet, and stylesheets are gone.
                self.themes = config::list_engine_themes();
                self.refresh_current();
                self.view = View::Look;
            }
            Act::OpenLayauts => {
                self.layauts = config::list_layauts();
                self.refresh_current();
                self.view = View::Layauts;
            }
            Act::OpenSounds => {
                self.sounds = config::list_sound_themes();
                self.refresh_current();
                self.view = View::Sounds;
            }
            Act::Look(i) => {
                // Selecting a theme writes Theme= and nothing else. Colour and
                // layout are two independent axes now, so this must not touch
                // Layaut= — picking crimson may not rearrange the boards.
                if let Some(name) = self.themes.get(i).cloned() {
                    config::set_engine_theme(&name);
                    self.refresh_current();
                    emit(Sfx::Theme);
                    return true;
                }
            }
            Act::Layaut(i) => {
                if let Some(name) = self.layauts.get(i).cloned() {
                    config::set_layaut_option(&name);
                    self.refresh_current();
                    emit(Sfx::Theme);
                    return true;
                }
            }
            Act::Sounds(i) => {
                if let Some(name) = self.sounds.get(i).cloned() {
                    config::set_sounds_option(&name);
                    self.refresh_current();
                    emit(Sfx::Theme);
                    return true;
                }
            }
            Act::OpenSound => {
                let (vol, typing, ambient) = config::sound_prefs();
                self.sound_volume = vol;
                self.sound_typing = typing;
                self.sound_ambient = ambient;
                self.view = View::Sound;
            }
            Act::OpenBlur => {
                let (radius, opacity) = config::blur_prefs();
                self.blur_radius = radius;
                self.blur_opacity = opacity;
                self.view = View::Blur;
            }
            Act::BlurRadiusTrack => {
                self.dragging_blur[0] = true;
                self.set_blur_from_x(0, x);
                self.blur_dirty = true;
            }
            Act::BlurOpacityTrack => {
                self.dragging_blur[1] = true;
                self.set_blur_from_x(1, x);
                self.blur_dirty = true;
            }
            Act::VolumeTrack => {
                self.dragging_volume = true;
                self.set_volume_from_x(x);
                self.sound_dirty = true;
            }
            Act::ToggleTyping => {
                self.sound_typing = !self.sound_typing;
                config::set_sound_typing(self.sound_typing);
                self.sound_dirty = true;
                emit(if self.sound_typing { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::ToggleAmbient => {
                self.sound_ambient = !self.sound_ambient;
                config::set_sound_ambient(self.sound_ambient);
                self.sound_dirty = true;
                emit(if self.sound_ambient { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::OpenColor => {
                if self.color_enabled {
                    let prefs = config::color_prefs();
                    self.color_depth = prefs.depth;
                    self.color_space = prefs.space;
                    self.color_lut = prefs.lut;
                    self.color_icc = prefs.icc;
                    self.color_luts = config::color_files("lut", &[".cube"]);
                    self.color_iccs = config::color_files("icc", &[".icc", ".icm"]);
                    self.view = View::Color;
                }
            }
            Act::ColorDepth(bits) => {
                self.color_depth = bits;
                config::set_color_depth(bits);
                self.color_dirty = true;
            }
            Act::ColorSpaceNext => {
                let list = config::COLOR_SPACES;
                let i = list
                    .iter()
                    .position(|s| *s == self.color_space)
                    .unwrap_or(0);
                self.color_space = list[(i + 1) % list.len()].to_string();
                config::set_color_space(&self.color_space);
                self.color_dirty = true;
            }
            Act::ColorLutNext => {
                // None -> first -> ... -> last -> None again.
                self.color_lut = next_of(&self.color_luts, self.color_lut.take());
                config::set_color_lut(self.color_lut.as_deref());
                self.color_dirty = true;
            }
            Act::ColorIccNext => {
                self.color_icc = next_of(&self.color_iccs, self.color_icc.take());
                config::set_color_icc(self.color_icc.as_deref());
                self.color_dirty = true;
            }
            Act::OpenGrid => {
                let (snap, cols, rows, pad) = config::grid_prefs();
                self.grid_snap = snap;
                self.grid_cols = cols;
                self.grid_rows = rows;
                self.grid_pad = pad;
                self.view = View::Grid;
            }
            Act::OpenBoards => self.view = View::Boards,
            Act::BoardGo(k) => {
                self.board_action = Some(BoardAction::Go(k));
            }
            Act::BoardAdd(side) => {
                self.board_action = Some(BoardAction::Add(side));
            }
            Act::BoardDel(k) => {
                self.board_action = Some(BoardAction::Del(k));
            }
            Act::ToggleSnap => {
                self.grid_snap = !self.grid_snap;
                config::set_grid_snap(self.grid_snap);
                emit(if self.grid_snap { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::ColsTrack => {
                self.dragging_cols = true;
                self.grid_cols = Self::cells_from_x(self.cols_rect, x);
            }
            Act::RowsTrack => {
                self.dragging_rows = true;
                self.grid_rows = Self::cells_from_x(self.rows_rect, x);
            }
            Act::PadTrack => {
                self.dragging_pad = true;
                self.set_pad_from_x(x);
            }
            Act::EditGrid => {
                self.edit_requested = true;
                self.open = false;
            }
            Act::ResetScreen => {
                // The application clears the section — it knows the
                // screen — and re-applies the layout; the window stays
                // open so the result is visible behind it.
                self.reset_screen = true;
            }
            Act::OpenFont => {
                self.families = [
                    crate::font::available_mono_families(),
                    crate::font::available_ui_families(),
                ];
                let (tscale, tfam, twgt) = config::term_font_prefs();
                let (uscale, ufam, uwgt) = config::ui_font_prefs();
                self.cur_size = [
                    (tscale * 100.0).round() as u32,
                    (uscale * 100.0).round() as u32,
                ];
                self.cur_family = [tfam, ufam];
                self.cur_weight = [twgt, uwgt];
                self.dropdown = None;
                self.view = View::Font;
            }
            Act::SizeTrack(sect) => {
                self.dragging_size = Some(sect);
                self.set_size_from_x(sect, x);
            }
            Act::FamilyBtn(sect) => {
                self.dropdown = if self.dropdown == Some(Dropdown::Family(sect)) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    Some(Dropdown::Family(sect))
                };
            }
            Act::WeightBtn(sect) => {
                self.dropdown = if self.dropdown == Some(Dropdown::Weight(sect)) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    Some(Dropdown::Weight(sect))
                };
            }
            Act::FamilyPick(sect, i) => {
                self.dropdown = None;
                let si = Self::sect_idx(sect);
                let value = if i == 0 {
                    // First entry: DEFAULT (auto-detected font).
                    None
                } else {
                    self.families[si].get(i - 1).cloned()
                };
                match sect {
                    Sect::Term => {
                        config::set_term_font_family(value.as_deref().unwrap_or(""))
                    }
                    Sect::Ui => config::set_ui_font_family(value.as_deref().unwrap_or("")),
                }
                self.cur_family[si] = value;
                return true;
            }
            Act::WeightPick(sect, i) => {
                self.dropdown = None;
                if let Some(w) = WEIGHTS.get(i) {
                    let si = Self::sect_idx(sect);
                    match sect {
                        Sect::Term => config::set_term_font_weight(w),
                        Sect::Ui => config::set_ui_font_weight(w),
                    }
                    self.cur_weight[si] = Some(w.to_string());
                    return true;
                }
            }
        }
        false
    }

    /// Keyboard entry point (F1 §1.5): Tab walks the chain in draw
    /// order, bare arrows move spatially (the boards and item grids
    /// are 2-D), Enter/Space activates through the SAME body a click
    /// runs, and Escape peels one layer — an open dropdown first; the
    /// window's own close stays the application's Escape, answered
    /// [`KeyOut::Ignored`] here. Sliders answer Left/Right themselves:
    /// they registered `GREEDY_ARROWS`, and a keyboard step walks
    /// values where a mouse sets positions.
    pub fn key(&mut self, ev: &KeyEv, fc: &mut FocusCtl) -> KeyOut {
        if !self.open {
            return KeyOut::Ignored;
        }
        match ev.key {
            FKey::Escape => {
                if self.dropdown.is_some() {
                    self.dropdown = None;
                    KeyOut::Consumed
                } else {
                    KeyOut::Ignored
                }
            }
            FKey::Enter | FKey::Space => {
                let Some(act) = self.focused_act(fc) else {
                    return KeyOut::Ignored;
                };
                if is_track(act) {
                    // A slider has no press; its keys are the arrows.
                    return KeyOut::Consumed;
                }
                let x = fc.rect_of(focus_id(act)).map_or(0.0, |r| r.cx());
                if self.perform(act, x) {
                    KeyOut::Changed
                } else {
                    KeyOut::Consumed
                }
            }
            _ => {
                let Some(n) = Nav::of(ev) else {
                    return KeyOut::Ignored;
                };
                if matches!(n, Nav::Left | Nav::Right) {
                    if let Some(act) = self.focused_act(fc) {
                        if is_track(act) {
                            let dir = if n == Nav::Right { 1 } else { -1 };
                            return if self.nudge(act, dir) {
                                KeyOut::Changed
                            } else {
                                KeyOut::Consumed
                            };
                        }
                    }
                }
                fc.nav(n);
                KeyOut::Consumed
            }
        }
    }

    /// The Act of the chain's focused control, when it is one of this
    /// window's. Walked topmost-first like the click path's hit walk.
    fn focused_act(&self, fc: &FocusCtl) -> Option<Act> {
        let id = fc.focused()?;
        self.hits
            .iter()
            .rev()
            .find(|(_, a)| focus_id(*a) == id)
            .map(|&(_, a)| a)
    }

    /// One keyboard step of a slider (Left/Right while it owns focus).
    /// A drag writes the configuration on release; the keyboard has no
    /// release moment, so every step writes immediately. Returns true
    /// when the caller must re-apply the configuration — the font size
    /// sliders, exactly [`Settings::release`]'s contract. Percent
    /// tracks step 5 per press; cell counts and pixels step 1.
    fn nudge(&mut self, act: Act, dir: i32) -> bool {
        fn stepped(v: u32, dir: i32, step: u32, lo: u32, hi: u32) -> u32 {
            (v as i64 + dir as i64 * step as i64).clamp(lo as i64, hi as i64) as u32
        }
        match act {
            Act::SizeTrack(sect) => {
                let i = Self::sect_idx(sect);
                let (lo, hi) = Self::size_range(sect);
                self.cur_size[i] =
                    stepped(self.cur_size[i], dir, 5, lo as u32, hi as u32);
                match sect {
                    Sect::Term => config::set_term_font_size(self.cur_size[i]),
                    Sect::Ui => config::set_ui_font_size(self.cur_size[i]),
                }
                true
            }
            Act::VolumeTrack => {
                self.sound_volume = stepped(self.sound_volume, dir, 5, 0, 100);
                config::set_sound_volume(self.sound_volume);
                self.sound_dirty = true;
                false
            }
            Act::BlurRadiusTrack => {
                self.blur_radius = stepped(self.blur_radius, dir, 5, 0, 100);
                config::set_blur_radius(self.blur_radius);
                self.blur_dirty = true;
                false
            }
            Act::BlurOpacityTrack => {
                self.blur_opacity = stepped(self.blur_opacity, dir, 5, 0, 100);
                config::set_blur_opacity(self.blur_opacity);
                self.blur_dirty = true;
                false
            }
            Act::ColsTrack => {
                self.grid_cols = stepped(self.grid_cols, dir, 1, GRID_MIN, GRID_MAX);
                config::set_grid_cols(self.grid_cols);
                false
            }
            Act::RowsTrack => {
                self.grid_rows = stepped(self.grid_rows, dir, 1, GRID_MIN, GRID_MAX);
                config::set_grid_rows(self.grid_rows);
                false
            }
            Act::PadTrack => {
                // 0-40 px — the range set_pad_from_x spans.
                self.grid_pad = stepped(self.grid_pad, dir, 1, 0, 40);
                config::set_grid_padding(self.grid_pad);
                false
            }
            _ => false,
        }
    }

    /// One interactive rect that no object helper draws (board tiles,
    /// colour chips, cyclers): the click map, the focus chain and the
    /// ring overlay in one motion, so a control cannot be clickable
    /// yet unreachable by keyboard. [`hit_into`] with this window's
    /// own map — loops that already hold a field of `self` call the
    /// free form directly.
    fn hit(&mut self, ctx: &mut Ctx, r: Rect, act: Act) {
        hit_into(&mut self.hits, ctx, r, act);
    }

    /// Refreshes the selection highlights from nacelle-desktop.conf:
    /// the engine's theme (Theme=), the layout (Layaut=) and the sound
    /// set (Sounds=), each falling back to "default" when unset.
    fn refresh_current(&mut self) {
        self.current_look = Some(
            config::current_engine_theme().unwrap_or_else(|| "default".to_string()),
        );
        self.current_layaut = Some(
            config::current_layaut_name().unwrap_or_else(|| "default".to_string()),
        );
        self.current_sounds = Some(
            config::current_sounds_name().unwrap_or_else(|| "default".to_string()),
        );
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        if !self.open {
            return;
        }
        self.hits.clear();
        let th = theme::resolved();
        static SCRIM_A: OnceLock<TokenId> = OnceLock::new();
        static PAD: OnceLock<TokenId> = OnceLock::new();
        static BODY_TOP: OnceLock<TokenId> = OnceLock::new();
        static ROW_GAP: OnceLock<TokenId> = OnceLock::new();
        static BTN_H: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_FRAC: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_MIN: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_MIN_PX: OnceLock<TokenId> = OnceLock::new();
        static TITLE_FG: OnceLock<TokenId> = OnceLock::new();

        // Dim the background and draw the window frame (nacelle::object).
        nacelle::object::window::backdrop(ctx, th.px(tok(&SCRIM_A, "modal.scrim_alpha")));
        let m = modal_rect(ctx.w, ctx.h);
        nacelle::object::window::frame(ctx, m);

        let pad = th.px(tok(&PAD, "modal.pad"));
        let title_px = role_title(th).px;
        let title = match self.view {
            View::Menu => "SETTINGS",
            View::Themes => "SETTINGS \u{2014} THEMES",
            View::Look => "SETTINGS \u{2014} LOOK",
            View::Layauts => "SETTINGS \u{2014} LAYAUTS",
            View::Sounds => "SETTINGS \u{2014} SOUNDS",
            View::Font => "SETTINGS \u{2014} FONT",
            View::Grid => "SETTINGS \u{2014} GRID",
            View::Sound => "SETTINGS \u{2014} SOUND",
            View::Boards => "SETTINGS \u{2014} BOARDS",
            View::Color => "SETTINGS \u{2014} COLOR",
            View::Blur => "SETTINGS \u{2014} BLUR",
        };
        // The rule under the title is the primitive's own; its colour token
        // (component.panel.header_underline) waits on a header primitive that
        // takes two colours.
        ctx.dl.module_title(
            ctx.fonts,
            m.x + pad,
            m.y + pad,
            m.w - 2.0 * pad,
            title_px,
            title,
            "",
            col(th.color(tok(&TITLE_FG, "component.panel.title"))),
            true,
        );

        let body_top = th.px(tok(&BODY_TOP, "modal.body_top"));
        let content = Rect::new(
            m.x + pad,
            m.y + body_top,
            m.w - 2.0 * pad,
            m.h - body_top - pad,
        );
        let btn_h = th.px(tok(&BTN_H, "button.h"));
        let gap = th.px(tok(&ROW_GAP, "modal.row_gap"));
        let corner_w = (content.w * th.px(tok(&BACK_W_FRAC, "settings.back_w_frac")))
            .max(th.px(tok(&BACK_W_MIN, "settings.back_w_min")))
            .max(th.px(tok(&BACK_W_MIN_PX, "settings.back_w_min_min_px")));

        match self.view {
            View::Menu => {
                // Close button in the top left of the main view.
                self.button(
                    ctx,
                    Rect::new(content.x, content.y, corner_w, btn_h),
                    "CLOSE",
                    Act::Close,
                );
                // Menu entries: THEMES, FONT and GRID.
                let bw = content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac"));
                let bx = content.x + (content.w - bw) / 2.0;
                let entries = [
                    ("THEMES", Act::OpenThemes),
                    ("FONT", Act::OpenFont),
                    ("SOUND", Act::OpenSound),
                    ("GRID", Act::OpenGrid),
                    ("BOARDS", Act::OpenBoards),
                    ("COLOR SPACE", Act::OpenColor),
                    ("BLUR", Act::OpenBlur),
                ];
                for (i, (label, act)) in entries.into_iter().enumerate() {
                    let y = content.y + (btn_h + gap) * (i as f32 + 1.0);
                    let r = Rect::new(bx, y, bw, btn_h);
                    // Colour is a conversation with a Wayland
                    // compositor; where there is none, the door is
                    // painted shut — visible, not clickable. The one
                    // genuinely disabled control in the program, so it
                    // takes the ladder's disabled rung.
                    if act == Act::OpenColor && !self.color_enabled {
                        let st = ladder(th, &BTN_CLASS, "button", State::Disabled);
                        ctx.dl.rect_outline(r.x, r.y, r.w, r.h, st.edge_width, col(st.edge));
                        let f = role_button(th);
                        ctx.dl.text_center(
                            ctx.fonts,
                            FONT_UI,
                            f.px,
                            r.cx(),
                            r.y + (r.h - f.line) / 2.0,
                            label,
                            col(st.text),
                            f.track,
                        );
                        continue;
                    }
                    self.button(ctx, r, label, act);
                }
            }
            View::Themes => {
                // Submenu: LOOK / LAYAUTS / SOUNDS.
                self.button(
                    ctx,
                    Rect::new(content.x, content.y, corner_w, btn_h),
                    "BACK",
                    Act::Back,
                );
                let bw = content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac"));
                let bx = content.x + (content.w - bw) / 2.0;
                let entries = [
                    ("LOOK", Act::OpenLook),
                    ("LAYAUTS", Act::OpenLayauts),
                    ("SOUNDS", Act::OpenSounds),
                ];
                for (i, (label, act)) in entries.into_iter().enumerate() {
                    let y = content.y + (btn_h + gap) * (i as f32 + 1.0);
                    self.button(ctx, Rect::new(bx, y, bw, btn_h), label, act);
                }
            }
            View::Look => {
                let names: Vec<String> =
                    self.themes.clone();
                self.item_grid(ctx, content, btn_h, gap, corner_w, &names, Act::Look);
                self.empty_note(ctx, content, btn_h, gap, &names, "NO LOOKS FOUND");
            }
            View::Layauts => {
                let names = self.layauts.clone();
                self.item_grid(ctx, content, btn_h, gap, corner_w, &names, Act::Layaut);
                self.empty_note(ctx, content, btn_h, gap, &names, "NO LAYAUTS FOUND");
                // RESET THIS SCREEN, beside the picker: deletes the
                // pinned [WxH@D] section of the selected layout for the
                // screen this window is on — the way out when a saved
                // arrangement predates a change to the base under it.
                let bw = content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac"));
                let bx = content.x + (content.w - bw) / 2.0;
                self.button(
                    ctx,
                    Rect::new(bx, content.bottom() - btn_h, bw, btn_h),
                    "RESET THIS SCREEN",
                    Act::ResetScreen,
                );
            }
            View::Sounds => {
                let names = self.sounds.clone();
                self.item_grid(ctx, content, btn_h, gap, corner_w, &names, Act::Sounds);
                self.empty_note(ctx, content, btn_h, gap, &names, "NO SOUND THEMES FOUND");
            }
            View::Font => self.draw_font_view(ctx, content, btn_h, gap, corner_w),
            View::Grid => self.draw_grid_view(ctx, content, btn_h, gap, corner_w),
            View::Sound => self.draw_sound_view(ctx, content, btn_h, gap, corner_w),
            View::Boards => self.draw_boards_view(ctx, content, btn_h, gap, corner_w),
            View::Color => self.draw_color_view(ctx, content, btn_h, gap, corner_w),
            View::Blur => self.draw_blur_view(ctx, content, btn_h, gap, corner_w),
        }
    }

    /// GRID view: snap checkbox, column/row counts and the EDIT GRID
    /// button that enters the layout editor.
    fn draw_grid_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        let th = theme::resolved();
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );

        let f = role_body(th);
        let v = role_value(th);
        let label_fg = col(th.color(tok(&LABEL_FG, "component.columns.label")));
        let value_fg = col(th.color(tok(&VALUE_FG, "component.columns.value")));
        let mut y = content.y + btn_h + th.px(tok(&SECTION_GAP, "settings.section_gap"));

        // SNAP TO GRID checkbox (nacelle::object; the whole row toggles).
        let check_h = th.px(tok(&CHECK_ROW_H, "checkbox.row_h"));
        let row = Rect::new(content.x, y, content.w, check_h);
        let hover = row.contains(ctx.mouse.0, ctx.mouse.1);
        nacelle::object::checkbox::draw_focusable(
            ctx,
            row,
            "SNAP TO GRID",
            self.grid_snap,
            hover,
            focus_id(Act::ToggleSnap),
        );
        self.hits.push((row, Act::ToggleSnap));
        y += check_h + gap;

        // COLUMNS / ROWS: sliders, like PADDING below and the font sizes.
        // A hundred cells is a hundred presses of a [+] button, which is
        // why these stopped being spinners.
        //
        // The label column is measured against the widest of the three
        // labels rather than each one's own, so all three tracks start
        // and end on the same pixel.
        let row_h = th.px(tok(&SLIDER_ROW_H, "slider.row_h"));
        let label_w = ctx.fonts.measure(FONT_UI, f.px, "COLUMNS", f.track)
            + th.px(tok(&LABEL_PAD, "rhythm.label_pad"));
        let value_w = ctx.fonts.measure(FONT_UI, v.px, "100 PX", v.track)
            + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter"));
        for (label, value, act) in [
            ("COLUMNS", self.grid_cols, Act::ColsTrack),
            ("ROWS", self.grid_rows, Act::RowsTrack),
        ] {
            ctx.dl.text(
                ctx.fonts,
                FONT_UI,
                f.px,
                content.x,
                y + (row_h - f.line) / 2.0,
                label,
                label_fg,
                f.track,
            );
            let track = Rect::new(content.x + label_w, y, content.w - label_w - value_w, row_h);
            match act {
                Act::ColsTrack => self.cols_rect = track,
                _ => self.rows_rect = track,
            }
            let t = (value.saturating_sub(GRID_MIN) as f32
                / (GRID_MAX - GRID_MIN) as f32)
                .clamp(0.0, 1.0);
            nacelle::object::slider::track_focusable(ctx, track, t, focus_id(act));
            ctx.dl.text_right(
                ctx.fonts,
                FONT_UI,
                v.px,
                content.right(),
                y + (row_h - v.line) / 2.0,
                &value.to_string(),
                value_fg,
                v.track,
            );
            self.hits.push((track, act));
            y += row_h + gap;
        }

        // PADDING slider — same form as the two above.
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            f.px,
            content.x,
            y + (row_h - f.line) / 2.0,
            "PADDING",
            label_fg,
            f.track,
        );
        let track = Rect::new(content.x + label_w, y, content.w - label_w - value_w, row_h);
        self.pad_rect = track;
        let t = (self.grid_pad as f32 / 40.0).clamp(0.0, 1.0);
        nacelle::object::slider::track_focusable(ctx, track, t, focus_id(Act::PadTrack));
        ctx.dl.text_right(
            ctx.fonts,
            FONT_UI,
            v.px,
            content.right(),
            y + (row_h - v.line) / 2.0,
            &format!("{} PX", self.grid_pad),
            value_fg,
            v.track,
        );
        self.hits.push((track, Act::PadTrack));
        y += row_h + gap;

        // EDIT GRID: hides this window and enters the layout editor.
        let bw = content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac"));
        let bx = content.x + (content.w - bw) / 2.0;
        self.button(
            ctx,
            Rect::new(bx, y + gap, bw, btn_h),
            "EDIT GRID",
            Act::EditGrid,
        );
    }

    /// SOUND view: master volume plus the two switches that matter in
    /// daily use — typing, which fires constantly, and the ambient bed.
    /// The BOARDS view: the boards laid out the way they sit in the
    /// world — the horizontal row centred on home, with the permanent
    /// top and bottom boards above and below. Clicking one the user is
    /// not on goes there with this window still open, which is why no
    /// board needs a control panel of its own. The small [+] tiles at
    /// the two ends of the row add a board on that side; the x removes
    /// one. The top and bottom boards, like home, have neither.
    fn draw_boards_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        let th = theme::resolved();
        static TILE_GAP: OnceLock<TokenId> = OnceLock::new();
        static TILE_MAX_W: OnceLock<TokenId> = OnceLock::new();
        static TILE_BORDER: OnceLock<TokenId> = OnceLock::new();
        static TILE_BORDER_CUR: OnceLock<TokenId> = OnceLock::new();
        static CAP_H: OnceLock<TokenId> = OnceLock::new();
        static CAP_GAP: OnceLock<TokenId> = OnceLock::new();
        static CLOSE_SIZE: OnceLock<TokenId> = OnceLock::new();
        static CLOSE_INSET: OnceLock<TokenId> = OnceLock::new();
        static CLOSE_STROKE: OnceLock<TokenId> = OnceLock::new();
        static PROXY_MIN: OnceLock<TokenId> = OnceLock::new();
        static PROXY_FILL: OnceLock<TokenId> = OnceLock::new();
        static BED_FILL: OnceLock<TokenId> = OnceLock::new();
        static PLUS_SIZE: OnceLock<TokenId> = OnceLock::new();
        static PLUS_MIN: OnceLock<TokenId> = OnceLock::new();
        static PLUS_STROKE: OnceLock<TokenId> = OnceLock::new();
        static ICON_INSET: OnceLock<TokenId> = OnceLock::new();
        static WC_IDLE: OnceLock<TokenId> = OnceLock::new();
        static WC_HOVER: OnceLock<TokenId> = OnceLock::new();
        static WC_CLOSE_HOVER: OnceLock<TokenId> = OnceLock::new();
        static HINT_INSET: OnceLock<TokenId> = OnceLock::new();
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );

        // boards.tile.caption_role; the role indirection waits on a text
        // primitive that takes a role.
        let f = role_caption(th);
        // boards.tile.aspect: `window` is the only mode the tiles can draw.
        let aspect = ctx.w / ctx.h.max(1.0);
        // Extents of the cross, from what the application handed over.
        let (mut l, mut r, mut u, mut d) = (0i32, 0i32, 0i32, 0i32);
        for b in &self.boards {
            let (x, y) = b.id;
            l = l.min(x);
            r = r.max(x);
            u = u.min(y);
            d = d.max(y);
        }
        // Tile size: fit the wider of the two arms, keep the window's
        // proportions, cap the height so the cross stays in view.
        let plus = th
            .px(tok(&PLUS_SIZE, "boards.plus_size"))
            .max(th.px(tok(&PLUS_MIN, "boards.plus_size_min_px")));
        let tgap = th.px(tok(&TILE_GAP, "boards.tile.gap"));
        let cap_strip =
            th.px(tok(&CAP_GAP, "boards.tile.caption_gap")) + th.px(tok(&CAP_H, "boards.tile.caption_h"));
        let cols = (r - l + 1) as f32;
        let rows = (d - u + 1) as f32;
        let area = Rect::new(
            content.x,
            content.y + btn_h + th.px(tok(&SECTION_GAP, "settings.section_gap")),
            content.w,
            content.h - btn_h * 2.0 - gap * 4.0,
        );
        let tile_w = ((area.w - 2.0 * (plus + tgap) - (cols - 1.0) * tgap) / cols)
            .min(content.w * th.px(tok(&TILE_MAX_W, "boards.tile.max_w_frac")))
            .min(((area.h - 2.0 * (plus + tgap)) / rows - tgap - cap_strip) * aspect);
        let tile_h = tile_w / aspect;
        let step_y = tile_h + tgap + cap_strip;
        let cross_w = cols * tile_w + (cols - 1.0) * tgap;
        let cross_h = rows * step_y - tgap - cap_strip;
        let x0 = area.x + (area.w - cross_w) / 2.0 - l as f32 * (tile_w + tgap);
        let y0 = area.y + (area.h - cross_h) / 2.0 - u as f32 * step_y;
        let tile_at = |bx: i32, by: i32| {
            Rect::new(
                x0 + bx as f32 * (tile_w + tgap),
                y0 + by as f32 * step_y,
                tile_w,
                tile_h,
            )
        };

        let bed = col(th.color(tok(&BED_FILL, "elev.inset.fill")));
        let proxy_fill = col(th.color(tok(&PROXY_FILL, "component.editor.proxy_fill")));
        let ring_w = th.px(tok(&TILE_BORDER, "boards.tile.border"));
        let ring_w_cur = th.px(tok(&TILE_BORDER_CUR, "boards.tile.border_current"));
        let idle_edge = col(ladder(th, &TILE_CLASS, "boards.tile", State::Idle).edge);
        // The ✕ and ✚ are window controls: one glyph set, one colour set.
        let wc_idle = col(th.color(tok(&WC_IDLE, "component.window_control.idle")));
        let wc_hover = col(th.color(tok(&WC_HOVER, "component.window_control.hover")));
        let wc_close = col(th.color(tok(&WC_CLOSE_HOVER, "component.window_control.close_hover")));

        for b in &self.boards {
            let (bx, by) = b.id;
            let tile = tile_at(bx, by);
            let hover = tile.contains(ctx.mouse.0, ctx.mouse.1);
            ctx.dl.rect(tile.x, tile.y, tile.w, tile.h, bed);
            let proxy_min = th.px(tok(&PROXY_MIN, "boards.tile.proxy_min_px"));
            for ps in &b.panels {
                let pr = Rect::new(
                    tile.x + ps.x / 100.0 * tile.w,
                    tile.y + ps.y / 100.0 * tile.h,
                    (ps.w / 100.0 * tile.w).max(proxy_min),
                    (ps.h / 100.0 * tile.h).max(proxy_min),
                );
                ctx.dl.rect(pr.x, pr.y, pr.w, pr.h, proxy_fill);
                ctx.dl.rect_outline(pr.x, pr.y, pr.w, pr.h, ring_w, idle_edge);
            }
            let st = ladder(
                th,
                &TILE_CLASS,
                "boards.tile",
                if b.current {
                    State::Selected
                } else if hover {
                    State::Hover
                } else {
                    State::Idle
                },
            );
            ctx.dl.rect_outline(
                tile.x,
                tile.y,
                tile.w,
                tile.h,
                if b.current { ring_w_cur } else { ring_w },
                col(st.edge),
            );
            let label = if b.id == (0, 0) {
                "HOME".to_string()
            } else if by == 0 {
                format!("{bx:+}")
            } else if by < 0 {
                "SEARCH AND AI".to_string()
            } else {
                "APPGRID".to_string()
            };
            let cap = ladder(
                th,
                &TILE_CLASS,
                "boards.tile",
                if b.current { State::Selected } else { State::Idle },
            );
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                f.px,
                tile.cx(),
                tile.bottom() + th.px(tok(&CAP_GAP, "boards.tile.caption_gap")),
                &label,
                col(cap.text),
                f.track,
            );
            if !b.current {
                hit_into(&mut self.hits, ctx, tile, Act::BoardGo(b.id));
            }
            if b.id != (0, 0) && by == 0 {
                let xs = th.px(tok(&CLOSE_SIZE, "boards.tile.close_size")).min(tile.w * 0.2);
                let inset = th.px(tok(&CLOSE_INSET, "boards.tile.close_inset"));
                let xr = Rect::new(tile.right() - xs - inset, tile.y + inset, xs, xs);
                let x_hot = xr.contains(ctx.mouse.0, ctx.mouse.1);
                // A destructive control: hovering takes the close glyph's
                // severity colour, not the plain hover.
                let c = if x_hot { wc_close } else { wc_idle };
                ctx.dl.rect(xr.x, xr.y, xr.w, xr.h, bed);
                ctx.dl.rect_outline(xr.x, xr.y, xr.w, xr.h, ring_w, c);
                let m = th.px(tok(&ICON_INSET, "winframe.icon.inset"));
                let s = th.px(tok(&CLOSE_STROKE, "boards.tile.close_stroke"));
                ctx.dl.line(xr.x + m, xr.y + m, xr.right() - m, xr.bottom() - m, s, c);
                ctx.dl.line(xr.right() - m, xr.y + m, xr.x + m, xr.bottom() - m, s, c);
                hit_into(&mut self.hits, ctx, xr, Act::BoardDel(b.id));
            }
        }

        // The [+] tiles: the same small square as the remove buttons,
        // with a plus in it, sitting just PAST the two ends of the row —
        // where the next board would appear. The row is the only thing
        // that grows.
        let left_t = tile_at(l, 0);
        let right_t = tile_at(r, 0);
        let arm_cy = left_t.y + left_t.h / 2.0;
        let ends: [((f32, f32), i8); 2] = [
            ((left_t.x - tgap - plus / 2.0, arm_cy), -1),
            ((right_t.right() + tgap + plus / 2.0, arm_cy), 1),
        ];
        for ((cx, cy), dir) in ends {
            let pr = Rect::new(cx - plus / 2.0, cy - plus / 2.0, plus, plus);
            let hot = pr.contains(ctx.mouse.0, ctx.mouse.1);
            let c = if hot { wc_hover } else { wc_idle };
            ctx.dl.rect(pr.x, pr.y, pr.w, pr.h, bed);
            ctx.dl.rect_outline(pr.x, pr.y, pr.w, pr.h, ring_w, c);
            let m = th.px(tok(&ICON_INSET, "winframe.icon.inset"));
            let s = th.px(tok(&PLUS_STROKE, "boards.plus_stroke"));
            let cyy = pr.y + pr.h / 2.0;
            let cxx = pr.x + pr.w / 2.0;
            ctx.dl.line(pr.x + m, cyy, pr.right() - m, cyy, s, c);
            ctx.dl.line(cxx, pr.y + m, cxx, pr.bottom() - m, s, c);
            self.hit(ctx, pr, Act::BoardAdd(dir));
        }

        // One line that explains the other way in (settings.hint.role).
        let hint = role_caption(th);
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            hint.px,
            content.x + content.w / 2.0,
            content.bottom() - th.px(tok(&HINT_INSET, "settings.hint_inset")),
            "HOLD THE LEFT BUTTON AND DRAG TO SWITCH BOARDS",
            col(th.color(tok(&MUTED_FG, "text.muted"))),
            hint.track,
        );
    }

    /// The COLOR view: swapchain depth, the colour space asked of the
    /// compositor, and the optional grading LUT and ICC profile. Rows
    /// of cyclers — click a value to step to the next.
    fn draw_color_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        let th = theme::resolved();
        static SEG_H: OnceLock<TokenId> = OnceLock::new();
        static SEG_GAP: OnceLock<TokenId> = OnceLock::new();
        static SEG_BORDER: OnceLock<TokenId> = OnceLock::new();
        static SEG_BORDER_ON: OnceLock<TokenId> = OnceLock::new();
        static CYC_H: OnceLock<TokenId> = OnceLock::new();
        static CYC_BORDER: OnceLock<TokenId> = OnceLock::new();
        static VALUE_TXT: OnceLock<TokenId> = OnceLock::new();
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );
        let f = role_body(th);
        let label_fg = col(th.color(tok(&LABEL_FG, "component.columns.label")));
        let mut y = content.y + btn_h + th.px(tok(&SECTION_GAP, "settings.section_gap"));
        // A fifth label-column rule; rhythm.label_col = auto needs a
        // measuring column primitive before this fraction can go.
        static LABEL_COL: OnceLock<TokenId> = OnceLock::new();
        let label_w = content.w
            * nacelle::theme::resolved()
                .px(tok(&LABEL_COL, "rhythm.label_col_frac"))
                .clamp(0.0, 1.0);

        let label = |ctx: &mut Ctx, text: &str, y: f32, row_h: f32| {
            ctx.dl.text(
                ctx.fonts,
                FONT_UI,
                f.px,
                content.x,
                y + (row_h - f.line) / 2.0,
                text,
                label_fg,
                f.track,
            );
        };

        // DEPTH: four fixed chips; the chip count is data, not theme.
        let seg_h = th.px(tok(&SEG_H, "segmented.h"));
        let seg_gap = th.px(tok(&SEG_GAP, "segmented.gap"));
        label(ctx, "DEPTH", y, seg_h);
        let chips = [8u32, 10, 12, 16];
        let cw = (content.w - label_w - seg_gap * 3.0) / 4.0;
        for (i, bits) in chips.into_iter().enumerate() {
            let r = Rect::new(content.x + label_w + (cw + seg_gap) * i as f32, y, cw, seg_h);
            let hover = r.contains(ctx.mouse.0, ctx.mouse.1);
            let on = self.color_depth == bits;
            let st = ladder(
                th,
                &CHIP_CLASS,
                "chip",
                if on {
                    State::Selected
                } else if hover {
                    State::Hover
                } else {
                    State::Idle
                },
            );
            ctx.dl.rect_outline(
                r.x,
                r.y,
                r.w,
                r.h,
                if on {
                    th.px(tok(&SEG_BORDER_ON, "segmented.border_active"))
                } else {
                    th.px(tok(&SEG_BORDER, "segmented.border"))
                },
                col(st.edge),
            );
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                f.px,
                r.cx(),
                y + (seg_h - f.line) / 2.0,
                &bits.to_string(),
                col(st.text),
                f.track,
            );
            self.hit(ctx, r, Act::ColorDepth(bits));
        }
        y += seg_h + gap;

        // Cyclers: the current value in a button-like slot; a click
        // steps to the next entry, wrapping through NONE where a file
        // may be absent. No chevrons yet — cycler.chevron_* wait on the
        // affordance being drawn at all.
        let cyc_h = th.px(tok(&CYC_H, "cycler.h"));
        let cyc_border = th.px(tok(&CYC_BORDER, "cycler.border"));
        let value_fg = col(th.color(tok(&VALUE_TXT, "text.primary")));
        let cycler = |slf: &mut Self, ctx: &mut Ctx, name: &str, value: &str, act: Act, y: f32| {
            label(ctx, name, y, cyc_h);
            let r = Rect::new(content.x + label_w, y, content.w - label_w, cyc_h);
            let hover = r.contains(ctx.mouse.0, ctx.mouse.1);
            let st = ladder(
                th,
                &BTN_CLASS,
                "button",
                if hover { State::Hover } else { State::Idle },
            );
            ctx.dl.rect_outline(r.x, r.y, r.w, r.h, cyc_border, col(st.edge));
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                f.px,
                r.cx(),
                y + (cyc_h - f.line) / 2.0,
                &value.to_uppercase(),
                value_fg,
                f.track,
            );
            slf.hit(ctx, r, act);
        };
        let space = self.color_space.clone();
        cycler(self, ctx, "SPACE", &space, Act::ColorSpaceNext, y);
        y += cyc_h + gap;
        let lut = self.color_lut.clone().unwrap_or_else(|| "none".into());
        cycler(self, ctx, "LUT", &lut, Act::ColorLutNext, y);
        y += cyc_h + gap;
        let icc = self.color_icc.clone().unwrap_or_else(|| "none".into());
        cycler(self, ctx, "ICC", &icc, Act::ColorIccNext, y);
        y += cyc_h + th.px(tok(&SECTION_GAP, "settings.section_gap"));

        // Where the files come from, for whoever wonders why the lists
        // are empty (settings.note.role).
        let n = role_caption(th);
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            n.px,
            content.x,
            y,
            "LUT: lut/*.cube    ICC: icc/*.icc — in the assets directories",
            col(th.color(tok(&MUTED_FG, "text.muted"))),
            n.track,
        );
    }

    /// BLUR view: the frosted glass under APPGRID and SEARCH AND AI —
    /// its radius (how deep the renderer's pyramid goes, always fully
    /// applied) and the background wash painted over the blur (0 % is
    /// pure blur, 100 % the old solid fixture background).
    fn draw_blur_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        let th = theme::resolved();
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );

        let f = role_body(th);
        let v = role_value(th);
        let label_fg = col(th.color(tok(&LABEL_FG, "component.columns.label")));
        let value_fg = col(th.color(tok(&VALUE_FG, "component.columns.value")));
        let row_h = th.px(tok(&SLIDER_ROW_H, "slider.row_h"));
        let mut y = content.y + btn_h + th.px(tok(&SECTION_GAP, "settings.section_gap"));
        let label_w = ctx.fonts.measure(FONT_UI, f.px, "OPACITY", f.track)
            + th.px(tok(&LABEL_PAD, "rhythm.label_pad"));
        let value_w = ctx.fonts.measure(FONT_UI, v.px, "100 %", v.track)
            + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter"));
        let rows = [
            ("RADIUS", self.blur_radius, Act::BlurRadiusTrack, 0usize),
            ("OPACITY", self.blur_opacity, Act::BlurOpacityTrack, 1usize),
        ];
        for (label, value, act, i) in rows {
            ctx.dl.text(
                ctx.fonts,
                FONT_UI,
                f.px,
                content.x,
                y + (row_h - f.line) / 2.0,
                label,
                label_fg,
                f.track,
            );
            let track =
                Rect::new(content.x + label_w, y, content.w - label_w - value_w, row_h);
            self.blur_rect[i] = track;
            nacelle::object::slider::track_focusable(
                ctx,
                track,
                (value as f32 / 100.0).clamp(0.0, 1.0),
                focus_id(act),
            );
            ctx.dl.text_right(
                ctx.fonts,
                FONT_UI,
                v.px,
                content.right(),
                y + (row_h - v.line) / 2.0,
                &format!("{value} %"),
                value_fg,
                v.track,
            );
            self.hits.push((track, act));
            y += row_h + gap;
        }
    }

    fn draw_sound_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        let th = theme::resolved();
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );

        let f = role_body(th);
        let v = role_value(th);
        let label_fg = col(th.color(tok(&LABEL_FG, "component.columns.label")));
        let value_fg = col(th.color(tok(&VALUE_FG, "component.columns.value")));
        let row_h = th.px(tok(&SLIDER_ROW_H, "slider.row_h"));
        let mut y = content.y + btn_h + th.px(tok(&SECTION_GAP, "settings.section_gap"));

        // VOLUME slider — same form as the font SIZE sliders.
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            f.px,
            content.x,
            y + (row_h - f.line) / 2.0,
            "VOLUME",
            label_fg,
            f.track,
        );
        let label_w = ctx.fonts.measure(FONT_UI, f.px, "VOLUME", f.track)
            + th.px(tok(&LABEL_PAD, "rhythm.label_pad"));
        let value_w = ctx.fonts.measure(FONT_UI, v.px, "100 %", v.track)
            + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter"));
        let track = Rect::new(content.x + label_w, y, content.w - label_w - value_w, row_h);
        self.volume_rect = track;
        nacelle::object::slider::track_focusable(
            ctx,
            track,
            (self.sound_volume as f32 / 100.0).clamp(0.0, 1.0),
            focus_id(Act::VolumeTrack),
        );
        ctx.dl.text_right(
            ctx.fonts,
            FONT_UI,
            v.px,
            content.right(),
            y + (row_h - v.line) / 2.0,
            &format!("{} %", self.sound_volume),
            value_fg,
            v.track,
        );
        self.hits.push((track, Act::VolumeTrack));
        y += row_h + gap;

        let check_h = th.px(tok(&CHECK_ROW_H, "checkbox.row_h"));
        for (label, on, act) in [
            ("TYPING SOUNDS", self.sound_typing, Act::ToggleTyping),
            ("AMBIENT", self.sound_ambient, Act::ToggleAmbient),
        ] {
            let row = Rect::new(content.x, y, content.w, check_h);
            let hover = row.contains(ctx.mouse.0, ctx.mouse.1);
            nacelle::object::checkbox::draw_focusable(
                ctx,
                row,
                label,
                on,
                hover,
                focus_id(act),
            );
            self.hits.push((row, act));
            y += check_h + gap;
        }

        // Which set is in use, and whether it was found at all: silence
        // with no explanation is the one thing worth spelling out here.
        let note = match config::active_sounds_dir() {
            Some(dir) => format!(
                "SET: {}",
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_uppercase()
            ),
            None => "NO SOUND SET SELECTED".to_string(),
        };
        let n = role_caption(th); // settings.note.role
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            n.px,
            content.x,
            y + gap,
            &note,
            col(th.color(tok(&MUTED_FG, "text.muted"))),
            n.track,
        );
    }

    /// FONT view: TERMINAL and INTERFACE sections, each with a size
    /// slider and family/weight dropdowns, separated by module headers.
    fn draw_font_view(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
    ) {
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );

        let mut y = content.y + btn_h + gap;
        let mut anchors: Vec<(Sect, Rect, Rect)> = Vec::new();
        for (sect, header) in [(Sect::Term, "TERMINAL"), (Sect::Ui, "INTERFACE")] {
            let (fam_rect, wgt_rect, next_y) =
                self.draw_font_section(ctx, content, y, btn_h, gap, sect, header);
            anchors.push((sect, fam_rect, wgt_rect));
            y = next_y;
        }

        // Open dropdown list (drawn last = on top, reverse hit-testing).
        static MENU_ROW_H: OnceLock<TokenId> = OnceLock::new();
        let item_h = theme::resolved().px(tok(&MENU_ROW_H, "menu.row_h"));
        for (sect, fam_rect, wgt_rect) in anchors {
            match self.dropdown {
                Some(Dropdown::Family(d)) if d == sect => {
                    let si = Self::sect_idx(sect);
                    let mut names = vec!["DEFAULT".to_string()];
                    names.extend(self.families[si].iter().map(|f| f.to_uppercase()));
                    self.draw_dropdown(
                        ctx,
                        fam_rect,
                        item_h,
                        &names,
                        dropdown_base(Dropdown::Family(sect)),
                        |i| Act::FamilyPick(sect, i),
                    );
                }
                Some(Dropdown::Weight(d)) if d == sect => {
                    let names: Vec<String> =
                        WEIGHTS.iter().map(|w| w.to_uppercase()).collect();
                    self.draw_dropdown(
                        ctx,
                        wgt_rect,
                        item_h,
                        &names,
                        dropdown_base(Dropdown::Weight(sect)),
                        |i| Act::WeightPick(sect, i),
                    );
                }
                _ => {}
            }
        }
    }

    /// One font section: header separator + SIZE slider + FAMILY/WEIGHT
    /// buttons. Returns the two dropdown anchors and the next free y.
    #[allow(clippy::too_many_arguments)]
    fn draw_font_section(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        top: f32,
        btn_h: f32,
        gap: f32,
        sect: Sect,
        header: &str,
    ) -> (Rect, Rect, f32) {
        let th = theme::resolved();
        static TITLE_FG: OnceLock<TokenId> = OnceLock::new();
        static BLOCK_H: OnceLock<TokenId> = OnceLock::new();
        let si = Self::sect_idx(sect);
        let title_px = role_section(th).px;
        // Section separator like every other module header.
        ctx.dl.module_title(
            ctx.fonts,
            content.x,
            top,
            content.w,
            title_px,
            header,
            "",
            col(th.color(tok(&TITLE_FG, "component.panel.title"))),
            true,
        );

        let f = role_body(th);
        let v = role_value(th);
        let label_fg = col(th.color(tok(&LABEL_FG, "component.columns.label")));
        let value_fg = col(th.color(tok(&VALUE_FG, "component.columns.value")));
        let row_h = th.px(tok(&SLIDER_ROW_H, "slider.row_h"));
        let row_x = content.x;
        let row_w = content.w;

        // SIZE: label, slider track with a knob, percent value.
        let size_y = top + th.px(tok(&BLOCK_H, "panel.title.block_h"));
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            f.px,
            row_x,
            size_y + (row_h - f.line) / 2.0,
            "SIZE",
            label_fg,
            f.track,
        );
        let label_w = ctx.fonts.measure(FONT_UI, f.px, "SIZE", f.track)
            + th.px(tok(&LABEL_PAD, "rhythm.label_pad"));
        let value_w = ctx.fonts.measure(FONT_UI, v.px, "200%", v.track)
            + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter"));
        let track = Rect::new(row_x + label_w, size_y, row_w - label_w - value_w, row_h);
        self.slider_rect[si] = track;
        let (rmin, rmax) = Self::size_range(sect);
        let t = ((self.cur_size[si] as f32 - rmin) / (rmax - rmin)).clamp(0.0, 1.0);
        nacelle::object::slider::track_focusable(
            ctx,
            track,
            t,
            focus_id(Act::SizeTrack(sect)),
        );
        ctx.dl.text_right(
            ctx.fonts,
            FONT_UI,
            v.px,
            content.right(),
            size_y + (row_h - v.line) / 2.0,
            &format!("{}%", self.cur_size[si]),
            value_fg,
            v.track,
        );
        self.hits.push((track, Act::SizeTrack(sect)));

        // FAMILY and WEIGHT dropdown buttons.
        let fam_y = size_y + row_h + gap;
        let fam_label = format!(
            "FAMILY: {}",
            self.cur_family[si].as_deref().unwrap_or("DEFAULT").to_uppercase()
        );
        let fam_rect = Rect::new(row_x, fam_y, row_w, btn_h);
        self.button(ctx, fam_rect, &fam_label, Act::FamilyBtn(sect));

        let wgt_y = fam_y + btn_h + gap;
        let wgt_label = format!(
            "WEIGHT: {}",
            self.cur_weight[si].as_deref().unwrap_or("REGULAR").to_uppercase()
        );
        let wgt_rect = Rect::new(row_x, wgt_y, row_w, btn_h);
        self.button(ctx, wgt_rect, &wgt_label, Act::WeightBtn(sect));

        (fam_rect, wgt_rect, wgt_y + btn_h + gap)
    }

    /// Dropdown list under an anchor button. `base` is the list's
    /// focus id — the accordion object registers every fully unfolded
    /// row as `base.item(i)` itself, the same derivation
    /// [`focus_id`] uses for the pick acts.
    fn draw_dropdown<F: Fn(usize) -> Act>(
        &mut self,
        ctx: &mut Ctx,
        anchor: Rect,
        item_h: f32,
        names: &[String],
        base: FocusId,
        make_act: F,
    ) {
        // Accordion animation: the list unfolds from the anchor's edge.
        static UNFOLD_MS: OnceLock<TokenId> = OnceLock::new();
        let dur = theme::resolved().px(tok(&UNFOLD_MS, "motion.menu_unfold.duration_ms")) / 1000.0;
        let t = self
            .dropdown_since
            .map(|s| {
                if dur <= 0.0 {
                    1.0
                } else {
                    (s.elapsed().as_secs_f32() / dur).clamp(0.0, 1.0)
                }
            })
            .unwrap_or(1.0);
        // motion.menu_unfold.easing = ease_out, awaiting a motion resolver.
        let p = 1.0 - (1.0 - t) * (1.0 - t);
        for (i, (r, _full)) in
            nacelle::object::dropdown::accordion_focusable(ctx, anchor, item_h, names, p, base)
                .into_iter()
                .enumerate()
        {
            self.hits.push((r, make_act(i)));
        }
    }

    /// BACK button + items next to it and below, in rows.
    #[allow(clippy::too_many_arguments)]
    fn item_grid(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        corner_w: f32,
        names: &[String],
        make_act: fn(usize) -> Act,
    ) {
        self.button(
            ctx,
            Rect::new(content.x, content.y, corner_w, btn_h),
            "BACK",
            Act::Back,
        );
        static GRID_COLS: OnceLock<TokenId> = OnceLock::new();
        let cols = (theme::resolved().px(tok(&GRID_COLS, "settings.grid_cols")) as usize).max(1);
        let bw = (content.w - gap * (cols as f32 - 1.0)) / cols as f32;
        let mut col = 1usize; // the first row starts next to BACK
        let mut y = content.y;
        for (i, name) in names.iter().enumerate() {
            if col >= cols {
                col = 0;
                y += btn_h + gap;
            }
            if y + btn_h > content.bottom() {
                break;
            }
            let br = Rect::new(content.x + col as f32 * (bw + gap), y, bw, btn_h);
            let label = name.to_uppercase();
            self.button(ctx, br, &label, make_act(i));
            col += 1;
        }
    }

    fn empty_note(
        &mut self,
        ctx: &mut Ctx,
        content: Rect,
        btn_h: f32,
        gap: f32,
        names: &[String],
        note: &str,
    ) {
        if !names.is_empty() {
            return;
        }
        // No emptystate.* tokens in the master yet; the value role and
        // text.muted stand in for emptystate.role.
        let th = theme::resolved();
        let v = role_value(th);
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            v.px,
            content.cx(),
            content.y + btn_h + gap,
            note,
            col(th.color(tok(&MUTED_FG, "text.muted"))),
            v.track,
        );
    }

    /// Button in the terminal-tab style (slant, hover, flash on click).
    fn button(&mut self, ctx: &mut Ctx, r: Rect, label: &str, act: Act) {
        let th = theme::resolved();
        static PRESS_MS: OnceLock<TokenId> = OnceLock::new();
        static SKEW: OnceLock<TokenId> = OnceLock::new();
        static ICON_SIZE: OnceLock<TokenId> = OnceLock::new();
        static ICON_MIN: OnceLock<TokenId> = OnceLock::new();
        // With an open dropdown only its items react to the mouse.
        let hover = self.dropdown.is_none() && r.contains(ctx.mouse.0, ctx.mouse.1);
        let press_s = th.px(tok(&PRESS_MS, "motion.press.duration_ms")) / 1000.0;
        let flash = self
            .flash
            .map(|(a, t)| a == act && t.elapsed().as_secs_f32() < press_s)
            .unwrap_or(false);
        // The currently selected item is highlighted like an active tab.
        let is_current = match act {
            Act::Look(i) => {
                self.themes.get(i).map(|n| Some(n) == self.current_look.as_ref())
                    == Some(true)
            }
            Act::Layaut(i) => {
                self.layauts.get(i).map(|s| Some(s) == self.current_layaut.as_ref())
                    == Some(true)
            }
            Act::Sounds(i) => {
                self.sounds.get(i).map(|s| Some(s) == self.current_sounds.as_ref())
                    == Some(true)
            }
            _ => false,
        };
        let st = nacelle::object::button::ButtonState { hover, flash, selected: is_current };
        // The focusable form: the button joins the world's chain under
        // its stable id and wears the ring itself (F1 §1.3); the arrow
        // and label of BACK draw after it, which is fine — the ring
        // sits outside the quad and overlaps neither.
        if act == Act::Back {
            // The base button (nacelle::object) plus a left arrow and a label
            // shifted to make room for it. Arrow and label share the ladder's
            // text colour — the arrow is a quad, so no glyph token applies.
            nacelle::object::button::draw_focusable(ctx, r, "", st, focus_id(act));
            let f = role_button(th);
            let state = if flash {
                State::Press
            } else if is_current {
                State::Selected
            } else if hover {
                State::Hover
            } else {
                State::Idle
            };
            let color = col(ladder(th, &BTN_CLASS, "button", state).text);
            let skew = th.px(tok(&SKEW, "button.skew"));
            let s = th
                .px(tok(&ICON_SIZE, "button.icon_size"))
                .max(th.px(tok(&ICON_MIN, "button.icon_size_min_px")));
            static ICON_SKEW: OnceLock<TokenId> = OnceLock::new();
        let ax = r.x
            + skew
                * nacelle::theme::resolved()
                    .px(tok(&ICON_SKEW, "button.icon_skew_frac"))
                    .clamp(0.0, 1.0)
            + s;
            let cy = r.y + r.h / 2.0;
            ctx.dl
                .quad([[ax - s, cy], [ax + s, cy - s], [ax + s, cy + s], [ax + s, cy + s]], color);
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                f.px,
                r.cx() + s,
                r.y + (r.h - f.line) / 2.0,
                label,
                color,
                f.track,
            );
        } else {
            nacelle::object::button::draw_focusable(ctx, r, label, st, focus_id(act));
        }
        self.hits.push((r, act));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One id per control: a collision would make Enter act on the
    /// wrong control, silently. A representative act of every view.
    #[test]
    fn focus_ids_are_pairwise_distinct() {
        let acts = [
            Act::Close,
            Act::Back,
            Act::OpenThemes,
            Act::OpenFont,
            Act::OpenSound,
            Act::OpenGrid,
            Act::OpenBoards,
            Act::OpenColor,
            Act::OpenBlur,
            Act::OpenLook,
            Act::OpenLayauts,
            Act::OpenSounds,
            Act::Look(0),
            Act::Look(1),
            Act::Layaut(0),
            Act::Sounds(0),
            Act::ResetScreen,
            Act::BlurRadiusTrack,
            Act::BlurOpacityTrack,
            Act::ColorDepth(8),
            Act::ColorDepth(10),
            Act::ColorSpaceNext,
            Act::ColorLutNext,
            Act::ColorIccNext,
            Act::BoardGo((1, 0)),
            Act::BoardGo((-1, 0)),
            Act::BoardGo((0, 1)),
            Act::BoardDel((1, 0)),
            Act::BoardAdd(-1),
            Act::BoardAdd(1),
            Act::VolumeTrack,
            Act::ToggleTyping,
            Act::ToggleAmbient,
            Act::ToggleSnap,
            Act::ColsTrack,
            Act::RowsTrack,
            Act::PadTrack,
            Act::EditGrid,
            Act::SizeTrack(Sect::Term),
            Act::SizeTrack(Sect::Ui),
            Act::FamilyBtn(Sect::Term),
            Act::FamilyBtn(Sect::Ui),
            Act::WeightBtn(Sect::Term),
            Act::WeightBtn(Sect::Ui),
            Act::FamilyPick(Sect::Term, 0),
            Act::FamilyPick(Sect::Ui, 0),
            Act::WeightPick(Sect::Term, 0),
            Act::WeightPick(Sect::Term, 1),
        ];
        for (i, a) in acts.iter().enumerate() {
            for b in acts.iter().skip(i + 1) {
                assert_ne!(
                    focus_id(*a),
                    focus_id(*b),
                    "two controls share a focus id"
                );
            }
        }
    }

    /// Dropdown rows must carry the id the accordion object derives
    /// itself (`base.item(i)`), and never the anchor button's own.
    #[test]
    fn dropdown_rows_share_the_accordion_derivation() {
        assert_eq!(
            focus_id(Act::FamilyPick(Sect::Term, 3)),
            dropdown_base(Dropdown::Family(Sect::Term)).item(3)
        );
        assert_eq!(
            focus_id(Act::WeightPick(Sect::Ui, 0)),
            dropdown_base(Dropdown::Weight(Sect::Ui)).item(0)
        );
        assert_ne!(
            focus_id(Act::FamilyPick(Sect::Term, 0)),
            focus_id(Act::FamilyBtn(Sect::Term))
        );
    }

    /// The keyboard split: sliders answer arrows, everything else
    /// answers Enter — an act in the wrong set either becomes
    /// unreachable or gets its value SET by a synthetic press.
    #[test]
    fn tracks_are_exactly_the_slider_acts() {
        assert!(is_track(Act::VolumeTrack));
        assert!(is_track(Act::BlurRadiusTrack));
        assert!(is_track(Act::BlurOpacityTrack));
        assert!(is_track(Act::ColsTrack));
        assert!(is_track(Act::RowsTrack));
        assert!(is_track(Act::PadTrack));
        assert!(is_track(Act::SizeTrack(Sect::Term)));
        assert!(!is_track(Act::EditGrid));
        assert!(!is_track(Act::ToggleSnap));
        assert!(!is_track(Act::FamilyBtn(Sect::Ui)));
        assert!(!is_track(Act::BoardGo((1, 0))));
    }
}

/// Steps a picker through None -> first -> ... -> last -> None.
fn next_of(list: &[String], current: Option<String>) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    match current {
        None => Some(list[0].clone()),
        Some(cur) => match list.iter().position(|v| *v == cur) {
            Some(i) if i + 1 < list.len() => Some(list[i + 1].clone()),
            _ => None,
        },
    }
}
