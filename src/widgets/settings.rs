//! Modal settings window (centred), DESCRIBED rather than drawn.
//!
//! Every view is a [`Page`]: a title, the corner button it wears and a
//! list of [`Row`]s. One walker places and draws them all, so a view is
//! data and not a function — the hit map, the focus chain and the
//! keyboard all fall out of the same description instead of agreeing by
//! hand, and the vertical rhythm is written once instead of six times.
//! The one thing no row can express is the BOARDS cross, which is why
//! [`Ctrl::Custom`] exists and why it has exactly one user.
//!
//! A page is allowed to be longer than its window. The chrome and
//! anything the page pins stand still; everything between them flows
//! inside [`Settings::body_box`], clipped to it and scrolled through it
//! by the toolkit's own offset ([`nacelle::view::scroll`]). So a row
//! that does not fit is a scroll away rather than a row drawn on the
//! desktop behind the window, and a list is as long as it likes.
//!
//! What the pages hold: THEMES is a submenu with LOOK (the theme
//! engine's themes), LAYAUTS (layouts) and SOUNDS (sound themes). A
//! theme comes from the toolkit — the eight compiled in plus anything
//! installed on the search path — and is written as Theme=; layouts and
//! sound sets are read from the data directories and written as
//! Layaut= / Sounds=. Everything applies live.

use super::{Ctx, PanelSpec, Rect};
use std::borrow::Cow;
use crate::config::{self, GRID_MAX, GRID_MIN};
use crate::font::FONT_UI;
use nacelle::focus::{Caps, FocusCtl, FocusId, Key as FKey, KeyEv, Mods, Nav};
use nacelle::theme::bake::StateStyle;
use nacelle::theme::parse::State;
use nacelle::theme::{self, TokenId};
use nacelle::view::scroll::{self, ScrollPhysics, ScrollView, ScrollbarLook};
use nacelle::view::{CtxSurface, Snap};
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

/// The part of `r` the clip leaves on screen, or nothing at all.
fn visible(r: Rect, clip: Option<Rect>) -> Option<Rect> {
    let Some(c) = clip else { return Some(r) };
    let (x0, y0) = (r.x.max(c.x), r.y.max(c.y));
    let (x1, y1) = (r.right().min(c.right()), r.bottom().min(c.bottom()));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// [`Settings::hit`]'s working part, over the hit map alone so a loop
/// that borrows another field of the window (the boards walk) can
/// still register its rects.
fn hit_into(hits: &mut Vec<(Rect, Act)>, clip: Option<Rect>, ctx: &mut Ctx, r: Rect, act: Act) {
    // A rect the clip drew away is no target: the scrolled-off half of
    // a row would otherwise still answer the pointer, under a chrome
    // button that is painted over it.
    let Some(seen) = visible(r, clip) else { return };
    let ring = ctx
        .focus
        .as_deref_mut()
        .map_or(false, |fc| fc.register(focus_id(act), r, Caps::NONE).ring);
    if ring {
        nacelle::object::focus_ring::draw(ctx, r);
    }
    hits.push((seen, act));
}

/// The slider tracks — the acts whose keyboard is Left/Right, never
/// Enter: a synthetic press at a track's centre would SET the value
/// to the centre, which no keyboard user asked for.
///
/// Answered by ASKING THE DESCRIPTION, not by a list kept beside it: a
/// slider is a slider because some page says so, so the keyboard cannot
/// disagree with what is drawn.
fn is_track(act: Act) -> bool {
    slider_of(act).is_some()
}

/// The row that describes the slider an act drives, if it drives one.
fn slider_of(act: Act) -> Option<&'static Ctrl> {
    PAGES
        .iter()
        .flat_map(|p| p.rows.iter())
        .map(|r| &r.ctrl)
        .find(|c| matches!(c, Ctrl::Slider { act: a, .. } if *a == act))
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
static LABEL_PAD: OnceLock<TokenId> = OnceLock::new();
static VALUE_GUTTER: OnceLock<TokenId> = OnceLock::new();
static LIST_W_FRAC: OnceLock<TokenId> = OnceLock::new();
static LABEL_COL: OnceLock<TokenId> = OnceLock::new();
static MODAL_PAD: OnceLock<TokenId> = OnceLock::new();

/// One type role, resolved for the frame being drawn: the size its runs
/// are set at, the letter spacing that belongs to it, and how tall one
/// line of it stands.
#[derive(Clone, Copy)]
struct Type {
    px: f32,
    track: f32,
    /// Line height as a MULTIPLE of `px` — the form the centring
    /// arithmetic takes it in.
    leading: f32,
}

impl Type {
    /// One line's height: what a single-line row reserves.
    fn line(self) -> f32 {
        self.px * self.leading
    }
}

/// The role a `*_role` binding token names, resolved.
///
/// [`nacelle::ui::bound_role`] is the toolkit's own reader — the menu,
/// the toaster, the tooltip and the text field all go through it — so a
/// theme that writes `settings.row_label_role = caption` finally moves
/// this window with them. The bindings the master declares for this
/// window had no reader at all while the roles were spelled out here
/// instead, which made six declared tokens into dead letters.
///
/// The px carries `ui_font_scale` — the UIFontSize= this very window
/// writes — and `panel_scale`, and never falls under `type.min_px`.
/// Without those three factors, the half of this window's text that
/// `object::button` does not draw stayed at 100 % while the other half
/// followed the preference: the interface size setting looked broken
/// from the one screen that sets it.
fn bound(ctx: &Ctx, cell: &'static OnceLock<TokenId>, binding: &'static str) -> Type {
    let role = nacelle::ui::bound_role(cell, binding);
    let px = role.px(ctx, ctx.ui_font_scale);
    Type { px, track: role.tracking_px(px), leading: role.leading() }
}

// One binding per place this file writes text. Every name below is a
// TOKEN name and never a role name: which role a binding lands on is
// the theme's decision, and reading it is this window's whole job here.
static ROLE_LABEL: OnceLock<TokenId> = OnceLock::new();
static ROLE_VALUE: OnceLock<TokenId> = OnceLock::new();
static ROLE_NOTE: OnceLock<TokenId> = OnceLock::new();
static ROLE_HINT: OnceLock<TokenId> = OnceLock::new();
static ROLE_CAPTION: OnceLock<TokenId> = OnceLock::new();
static ROLE_EMPTY: OnceLock<TokenId> = OnceLock::new();
static ROLE_BUTTON: OnceLock<TokenId> = OnceLock::new();
static ROLE_TITLE: OnceLock<TokenId> = OnceLock::new();
static ROLE_SECTION: OnceLock<TokenId> = OnceLock::new();

/// Every row label: COLUMNS, PADDING, VOLUME.
fn role_label(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_LABEL, "settings.row_label_role")
}

/// The value half of a row — the number written beside a track.
fn role_value(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_VALUE, "columns.value_role")
}

/// An aside in the flow: SET:, and where the LUT and ICC files live.
fn role_note(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_NOTE, "settings.note.role")
}

/// The one line pinned under the boards.
fn role_hint(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_HINT, "settings.hint.role")
}

/// A board thumbnail's caption.
fn role_caption(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_CAPTION, "boards.tile.caption_role")
}

/// NO … FOUND.
fn role_empty(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_EMPTY, "emptystate.role")
}

/// A button's label, for the two this file letters itself — BACK, and a
/// row its page turned off. `object::button` reads the same binding for
/// every other button on screen.
fn role_button(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_BUTTON, "button.role")
}

/// The modal's title band.
fn role_title(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_TITLE, "modal.title.role")
}

/// A section header inside a page: the FONT view's TERMINAL and
/// INTERFACE.
fn role_section(ctx: &Ctx) -> Type {
    bound(ctx, &ROLE_SECTION, "settings.section_role")
}

/// Top of one centred line in a band.
///
/// `rhythm.center_mode = optical` nudges the line by
/// `rhythm.cap_center_bias`, which is what an all-caps run needs and
/// what a hand-written `y + (h - line) / 2.0` cannot say — this file had
/// fifteen of those and every one of them parked its label visibly low.
/// The arithmetic is the toolkit's
/// ([`nacelle::view::paint::center_line_y`]), so the window now centres
/// the way the views beyond the plugin boundary do, and the way the
/// neighbouring popup spells out by hand.
fn center_y(ctx: &mut Ctx, band: Rect, t: Type) -> f32 {
    nacelle::view::paint::center_line_y(
        &mut nacelle::view::surface::CtxSurface::new(ctx),
        band.y,
        band.h,
        t.px,
        t.leading,
    )
}

// ------------------------------------------------------------- description

/// A piece of text a row shows: fixed, or read from the window (the
/// FONT view's buttons carry their current value in their label).
///
/// The read form returns an owned string, so resolving it does not
/// borrow the window while the window is drawing itself.
#[derive(Clone, Copy)]
enum Text {
    Fixed(&'static str),
    Of(fn(&Settings) -> String),
}

/// The space a row leaves under itself. Named rather than counted,
/// because four views hand-space their groups today and a walker cannot
/// guess which gap was meant.
#[derive(Clone, Copy, PartialEq)]
enum Gap {
    /// `modal.row_gap` — the ordinary space between two rows.
    Row,
    /// Two row gaps: the break in front of GRID's EDIT GRID and SOUND's
    /// closing note.
    Double,
    /// `settings.section_gap` — the break in front of COLOR's note.
    Section,
    /// None: the FONT view's section header, whose slider sits directly
    /// under it.
    None,
}

/// How a page splits its rows into a label column and a value column.
/// Two rules where there should be one — the fraction is COLOR's alone
/// and `rhythm.label_col = auto` is what retires it.
#[derive(Clone, Copy)]
enum Cols {
    /// Measured against the page's widest label and widest value, so
    /// every track on the page starts and ends on the same pixel.
    Measured { label: &'static str, value: &'static str },
    /// `rhythm.label_col_frac` of the content width, no value column.
    Frac,
    /// The page has no label/value rows.
    None,
}

/// How wide a button is and where it sits.
#[derive(Clone, Copy, PartialEq)]
enum BtnKind {
    /// Centred at `settings.list_w_frac`: menu entries, EDIT GRID.
    Listed,
    /// The full content width: the FONT view's dropdown anchors.
    Wide,
    /// Listed, but pinned to the bottom of the content box instead of
    /// flowing — RESET THIS SCREEN. The rows above it used to be able
    /// to reach it, and the later of two targets on one pixel won the
    /// click (P12); the flow is now held to [`Settings::body_box`],
    /// which stops short of whatever a page pins.
    Footer,
}

/// How a slider writes its value out. The two percent spellings differ
/// by a space and are kept apart because changing one would move a
/// column.
#[derive(Clone, Copy, PartialEq)]
enum Unit {
    /// A bare count: the grid's columns and rows.
    None,
    /// `N PX` — the widget padding.
    Px,
    /// `N %` — volume, blur radius and blur opacity.
    Percent,
    /// `N%`, no space — the font sizes, whose column is measured
    /// against "200%".
    Tight,
}

impl Unit {
    fn text(self, v: u32) -> String {
        match self {
            Unit::None => v.to_string(),
            Unit::Px => format!("{v} PX"),
            Unit::Percent => format!("{v} %"),
            Unit::Tight => format!("{v}%"),
        }
    }
}

/// Which list of names a picker offers. The list, the act its rows
/// carry, the selection it highlights and the words for "there are
/// none" are all one decision, so they are one value.
#[derive(Clone, Copy, PartialEq)]
enum ListId {
    Looks,
    Layauts,
    Sounds,
}

impl ListId {
    fn act(self) -> fn(usize) -> Act {
        match self {
            ListId::Looks => Act::Look,
            ListId::Layauts => Act::Layaut,
            ListId::Sounds => Act::Sounds,
        }
    }

    fn empty_note(self) -> &'static str {
        match self {
            ListId::Looks => "NO LOOKS FOUND",
            ListId::Layauts => "NO LAYAUTS FOUND",
            ListId::Sounds => "NO SOUND THEMES FOUND",
        }
    }
}

/// One control of a page, as a description. Everything a row needs to
/// draw itself, register itself and answer a key lives here: nothing
/// else in the file may know that GRID has three sliders.
#[derive(Clone, Copy)]
enum Ctrl {
    /// A switch over the whole row width.
    Toggle { label: &'static str, get: fn(&Settings) -> bool, act: Act },
    /// A track with a label and a value. `range` and `step` are the
    /// ONLY statement of a slider's limits: the drag, the keyboard step
    /// and the knob position all read them here (R7). The font ranges
    /// mirror the clamps in `config.rs`, which owns the data's range.
    Slider {
        label: &'static str,
        act: Act,
        unit: Unit,
        range: (u32, u32),
        step: u32,
        get: fn(&Settings) -> u32,
        set: fn(&mut Settings, u32),
        /// Writes the value to nacelle-desktop.conf.
        save: fn(&Settings),
    },
    /// A row of fixed segments, one of them on: COLOR's DEPTH.
    Chips {
        label: &'static str,
        values: &'static [u32],
        get: fn(&Settings) -> u32,
        act: fn(u32) -> Act,
    },
    /// A value that steps to the next on every press: COLOR's SPACE,
    /// LUT and ICC.
    Cycle { label: &'static str, get: fn(&Settings) -> String, act: Act },
    /// A list of names filling the page.
    Picker { list: ListId },
    Button { label: Text, kind: BtnKind, act: Act },
    /// A module header inside a page: the FONT view's TERMINAL and
    /// INTERFACE separators.
    Section { title: &'static str },
    /// A left-aligned aside in the flow.
    Note { text: Text },
    /// A centred one-liner pinned to the bottom edge
    /// (`settings.hint_inset`) — the BOARDS view's gesture hint.
    Hint { text: Text },
    /// A row nothing else describes. The BOARDS cross, and nothing
    /// else: `h` is the vertical reserve it takes out of the content
    /// box, `draw` fills it.
    Custom { h: fn(Metrics, Rect) -> f32, draw: fn(&mut Settings, &mut Ctx, Rect) },
}

impl Ctrl {
    /// Whether the row sits at a fixed place instead of flowing.
    fn pinned(&self) -> bool {
        matches!(
            self,
            Ctrl::Button { kind: BtnKind::Footer, .. } | Ctrl::Hint { .. }
        )
    }
}

/// A control plus the two things the page says ABOUT it rather than
/// about its kind: the space under it and whether it is live at all.
#[derive(Clone, Copy)]
struct Row {
    ctrl: Ctrl,
    after: Gap,
    /// R6: a row that answers false is drawn as a grey inscription and
    /// registers nothing — no hit, no place in the focus chain. The one
    /// genuinely disabled control in the program takes this road.
    enabled: fn(&Settings) -> bool,
}

fn always(_: &Settings) -> bool {
    true
}

const fn row(ctrl: Ctrl) -> Row {
    Row { ctrl, after: Gap::Row, enabled: always }
}

const fn row_after(ctrl: Ctrl, after: Gap) -> Row {
    Row { ctrl, after, enabled: always }
}

const fn row_when(ctrl: Ctrl, enabled: fn(&Settings) -> bool) -> Row {
    Row { ctrl, after: Gap::Row, enabled }
}

/// The corner button a page wears, and what the body does about it.
#[derive(Clone, Copy, PartialEq)]
enum Chrome {
    /// CLOSE, with the body below it — the main view.
    Close,
    /// BACK, with the body below it.
    Back,
    /// BACK, with the body BESIDE it: a picker grid starts in the same
    /// row's second column.
    BackInline,
}

/// One view of the window.
struct Page {
    view: View,
    title: &'static str,
    chrome: Chrome,
    /// The space between the chrome row and the first flowed row.
    lead: Gap,
    cols: Cols,
    rows: &'static [Row],
}

// --------------------------------------------------------------- the pages

static MENU_ROWS: [Row; 7] = [
    row(Ctrl::Button {
        label: Text::Fixed("THEMES"),
        kind: BtnKind::Listed,
        act: Act::OpenThemes,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("FONT"),
        kind: BtnKind::Listed,
        act: Act::OpenFont,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("SOUND"),
        kind: BtnKind::Listed,
        act: Act::OpenSound,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("GRID"),
        kind: BtnKind::Listed,
        act: Act::OpenGrid,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("BOARDS"),
        kind: BtnKind::Listed,
        act: Act::OpenBoards,
    }),
    // Colour is a conversation with a Wayland compositor; where there
    // is none, the door is painted shut — visible, not clickable.
    row_when(
        Ctrl::Button {
            label: Text::Fixed("COLOR SPACE"),
            kind: BtnKind::Listed,
            act: Act::OpenColor,
        },
        |s| s.color_enabled,
    ),
    row(Ctrl::Button {
        label: Text::Fixed("BLUR"),
        kind: BtnKind::Listed,
        act: Act::OpenBlur,
    }),
];

static THEMES_ROWS: [Row; 3] = [
    row(Ctrl::Button {
        label: Text::Fixed("LOOK"),
        kind: BtnKind::Listed,
        act: Act::OpenLook,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("LAYAUTS"),
        kind: BtnKind::Listed,
        act: Act::OpenLayauts,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("SOUNDS"),
        kind: BtnKind::Listed,
        act: Act::OpenSounds,
    }),
];

static LOOK_ROWS: [Row; 1] = [row(Ctrl::Picker { list: ListId::Looks })];

static LAYAUTS_ROWS: [Row; 2] = [
    row(Ctrl::Picker { list: ListId::Layauts }),
    // Beside the picker: deletes the pinned [WxH@D] section of the
    // selected layout for the screen this window is on — the way out
    // when a saved arrangement predates a change to the base under it.
    row(Ctrl::Button {
        label: Text::Fixed("RESET THIS SCREEN"),
        kind: BtnKind::Footer,
        act: Act::ResetScreen,
    }),
];

static SOUNDS_ROWS: [Row; 1] = [row(Ctrl::Picker { list: ListId::Sounds })];

/// The FONT view's two sections. The section header takes no gap under
/// it: the size slider sits directly against the separator.
static FONT_ROWS: [Row; 8] = [
    row_after(Ctrl::Section { title: "TERMINAL" }, Gap::None),
    row(Ctrl::Slider {
        label: "SIZE",
        act: Act::SizeTrack(Sect::Term),
        unit: Unit::Tight,
        range: (50, 200),
        step: 5,
        get: |s| s.cur_size[0],
        set: |s, v| s.cur_size[0] = v,
        save: |s| config::set_term_font_size(s.cur_size[0]),
    }),
    row(Ctrl::Button {
        label: Text::Of(|s| family_label(s, Sect::Term)),
        kind: BtnKind::Wide,
        act: Act::FamilyBtn(Sect::Term),
    }),
    row(Ctrl::Button {
        label: Text::Of(|s| weight_label(s, Sect::Term)),
        kind: BtnKind::Wide,
        act: Act::WeightBtn(Sect::Term),
    }),
    row_after(Ctrl::Section { title: "INTERFACE" }, Gap::None),
    // The interface starts at 30% so a big screen can have a small
    // interface — 75% was as low as it went, which on a 4K panel was
    // still larger than anyone wanted.
    row(Ctrl::Slider {
        label: "SIZE",
        act: Act::SizeTrack(Sect::Ui),
        unit: Unit::Tight,
        range: (30, 125),
        step: 5,
        get: |s| s.cur_size[1],
        set: |s, v| s.cur_size[1] = v,
        save: |s| config::set_ui_font_size(s.cur_size[1]),
    }),
    row(Ctrl::Button {
        label: Text::Of(|s| family_label(s, Sect::Ui)),
        kind: BtnKind::Wide,
        act: Act::FamilyBtn(Sect::Ui),
    }),
    row(Ctrl::Button {
        label: Text::Of(|s| weight_label(s, Sect::Ui)),
        kind: BtnKind::Wide,
        act: Act::WeightBtn(Sect::Ui),
    }),
];

/// A hundred cells is a hundred presses of a [+] button, which is why
/// the counts are sliders and not spinners.
static GRID_ROWS: [Row; 5] = [
    row(Ctrl::Toggle {
        label: "SNAP TO GRID",
        get: |s| s.grid_snap,
        act: Act::ToggleSnap,
    }),
    row(Ctrl::Slider {
        label: "COLUMNS",
        act: Act::ColsTrack,
        unit: Unit::None,
        range: (GRID_MIN, GRID_MAX),
        step: 1,
        get: |s| s.grid_cols,
        set: |s, v| s.grid_cols = v,
        save: |s| config::set_grid_cols(s.grid_cols),
    }),
    row(Ctrl::Slider {
        label: "ROWS",
        act: Act::RowsTrack,
        unit: Unit::None,
        range: (GRID_MIN, GRID_MAX),
        step: 1,
        get: |s| s.grid_rows,
        set: |s, v| s.grid_rows = v,
        save: |s| config::set_grid_rows(s.grid_rows),
    }),
    row_after(
        Ctrl::Slider {
            label: "PADDING",
            act: Act::PadTrack,
            unit: Unit::Px,
            range: (0, 40),
            step: 1,
            get: |s| s.grid_pad,
            set: |s, v| s.grid_pad = v,
            save: |s| config::set_grid_padding(s.grid_pad),
        },
        Gap::Double,
    ),
    // Hides this window and enters the layout editor.
    row(Ctrl::Button {
        label: Text::Fixed("EDIT GRID"),
        kind: BtnKind::Listed,
        act: Act::EditGrid,
    }),
];

/// Master volume plus the two switches that matter in daily use —
/// typing, which fires constantly, and the ambient bed.
static SOUND_ROWS: [Row; 4] = [
    row(Ctrl::Slider {
        label: "VOLUME",
        act: Act::VolumeTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: 5,
        get: |s| s.sound_volume,
        set: |s, v| s.sound_volume = v,
        save: |s| config::set_sound_volume(s.sound_volume),
    }),
    row(Ctrl::Toggle {
        label: "TYPING SOUNDS",
        get: |s| s.sound_typing,
        act: Act::ToggleTyping,
    }),
    row_after(
        Ctrl::Toggle {
            label: "AMBIENT",
            get: |s| s.sound_ambient,
            act: Act::ToggleAmbient,
        },
        Gap::Double,
    ),
    row(Ctrl::Note { text: Text::Of(sound_set_note) }),
];

static BOARDS_ROWS: [Row; 2] = [
    row(Ctrl::Custom { h: boards_h, draw: Settings::draw_boards }),
    row(Ctrl::Hint {
        text: Text::Fixed("HOLD THE LEFT BUTTON AND DRAG TO SWITCH BOARDS"),
    }),
];

/// Swapchain depth, the colour space asked of the compositor, and the
/// optional grading LUT and ICC profile.
static COLOR_ROWS: [Row; 5] = [
    row(Ctrl::Chips {
        label: "DEPTH",
        values: &[8, 10, 12, 16],
        get: |s| s.color_depth,
        act: Act::ColorDepth,
    }),
    row(Ctrl::Cycle {
        label: "SPACE",
        get: |s| s.color_space.clone(),
        act: Act::ColorSpaceNext,
    }),
    row(Ctrl::Cycle {
        label: "LUT",
        get: |s| s.color_lut.clone().unwrap_or_else(|| "none".into()),
        act: Act::ColorLutNext,
    }),
    row_after(
        Ctrl::Cycle {
            label: "ICC",
            get: |s| s.color_icc.clone().unwrap_or_else(|| "none".into()),
            act: Act::ColorIccNext,
        },
        Gap::Section,
    ),
    // Where the files come from, for whoever wonders why the lists are
    // empty (settings.note.role).
    row(Ctrl::Note {
        text: Text::Fixed("LUT: lut/*.cube    ICC: icc/*.icc — in the assets directories"),
    }),
];

/// The frosted glass under APPGRID and SEARCH AND AI — its radius (how
/// deep the renderer's pyramid goes, always fully applied) and the
/// background wash painted over the blur (0 % is pure blur, 100 % the
/// old solid fixture background).
static BLUR_ROWS: [Row; 2] = [
    row(Ctrl::Slider {
        label: "RADIUS",
        act: Act::BlurRadiusTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: 5,
        get: |s| s.blur_radius,
        set: |s, v| s.blur_radius = v,
        save: |s| config::set_blur_radius(s.blur_radius),
    }),
    row(Ctrl::Slider {
        label: "OPACITY",
        act: Act::BlurOpacityTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: 5,
        get: |s| s.blur_opacity,
        set: |s, v| s.blur_opacity = v,
        save: |s| config::set_blur_opacity(s.blur_opacity),
    }),
];

/// The whole window. Indexed by [`View`], which `pages_are_in_view_order`
/// keeps true.
static PAGES: [Page; 11] = [
    Page {
        view: View::Menu,
        title: "SETTINGS",
        chrome: Chrome::Close,
        lead: Gap::Row,
        cols: Cols::None,
        rows: &MENU_ROWS,
    },
    Page {
        view: View::Themes,
        title: "SETTINGS \u{2014} THEMES",
        chrome: Chrome::Back,
        lead: Gap::Row,
        cols: Cols::None,
        rows: &THEMES_ROWS,
    },
    Page {
        view: View::Look,
        title: "SETTINGS \u{2014} LOOK",
        chrome: Chrome::BackInline,
        lead: Gap::None,
        cols: Cols::None,
        rows: &LOOK_ROWS,
    },
    Page {
        view: View::Layauts,
        title: "SETTINGS \u{2014} LAYAUTS",
        chrome: Chrome::BackInline,
        lead: Gap::None,
        cols: Cols::None,
        rows: &LAYAUTS_ROWS,
    },
    Page {
        view: View::Sounds,
        title: "SETTINGS \u{2014} SOUNDS",
        chrome: Chrome::BackInline,
        lead: Gap::None,
        cols: Cols::None,
        rows: &SOUNDS_ROWS,
    },
    Page {
        view: View::Font,
        title: "SETTINGS \u{2014} FONT",
        chrome: Chrome::Back,
        lead: Gap::Row,
        cols: Cols::Measured { label: "SIZE", value: "200%" },
        rows: &FONT_ROWS,
    },
    Page {
        view: View::Grid,
        title: "SETTINGS \u{2014} GRID",
        chrome: Chrome::Back,
        lead: Gap::Section,
        // Measured against the widest of the three labels rather than
        // each one's own, so all three tracks line up.
        cols: Cols::Measured { label: "COLUMNS", value: "100 PX" },
        rows: &GRID_ROWS,
    },
    Page {
        view: View::Sound,
        title: "SETTINGS \u{2014} SOUND",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::Measured { label: "VOLUME", value: "100 %" },
        rows: &SOUND_ROWS,
    },
    Page {
        view: View::Boards,
        title: "SETTINGS \u{2014} BOARDS",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::None,
        rows: &BOARDS_ROWS,
    },
    Page {
        view: View::Color,
        title: "SETTINGS \u{2014} COLOR",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::Frac,
        rows: &COLOR_ROWS,
    },
    Page {
        view: View::Blur,
        title: "SETTINGS \u{2014} BLUR",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::Measured { label: "OPACITY", value: "100 %" },
        rows: &BLUR_ROWS,
    },
];

fn page(view: View) -> &'static Page {
    let p = &PAGES[view as usize];
    debug_assert!(p.view == view, "PAGES must stand in View's order");
    p
}

/// The vertical reserve the BOARDS cross takes: the content box less
/// the corner button and the room the caption strip and the hint need.
fn boards_h(m: Metrics, content: Rect) -> f32 {
    content.h - m.btn_h * 2.0 - m.gap * 4.0
}

fn family_label(s: &Settings, sect: Sect) -> String {
    let i = Settings::sect_idx(sect);
    format!(
        "FAMILY: {}",
        s.cur_family[i].as_deref().unwrap_or("DEFAULT").to_uppercase()
    )
}

fn weight_label(s: &Settings, sect: Sect) -> String {
    let i = Settings::sect_idx(sect);
    format!(
        "WEIGHT: {}",
        s.cur_weight[i].as_deref().unwrap_or("REGULAR").to_uppercase()
    )
}

/// Which sound set is in use, and whether it was found at all: silence
/// with no explanation is the one thing worth spelling out here.
fn sound_set_note(_: &Settings) -> String {
    match config::active_sounds_dir() {
        Some(dir) => format!(
            "SET: {}",
            dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_uppercase()
        ),
        None => "NO SOUND SET SELECTED".to_string(),
    }
}

// ------------------------------------------------------------------ lengths

/// Every length one frame lays a page out with, resolved once. The row
/// grammar reads nothing else — which is what makes a page's height
/// something a test can compute without drawing it.
#[derive(Clone, Copy)]
struct Metrics {
    btn_h: f32,
    gap: f32,
    section_gap: f32,
    slider_h: f32,
    check_h: f32,
    seg_h: f32,
    seg_gap: f32,
    cyc_h: f32,
    /// `panel.title.block_h`: a section header's band plus the gap
    /// under it, i.e. what the body starts after.
    block_h: f32,
    /// One line of the note role — the height a note occupies. Two
    /// lengths for what a theme may bind to one role, because
    /// `settings.note.role` and `settings.hint.role` are two decisions.
    note_h: f32,
    hint_h: f32,
    hint_inset: f32,
    corner_w: f32,
    list_w: f32,
    grid_cols: usize,
}

impl Metrics {
    /// Resolved against the drawing context and not the theme alone,
    /// because two of these lengths are type: a role's px follows
    /// `ui_font_scale` and `panel_scale`, so a page's height does too.
    fn of(ctx: &Ctx, content: Rect) -> Metrics {
        let th = theme::resolved();
        static BTN_H: OnceLock<TokenId> = OnceLock::new();
        static ROW_GAP: OnceLock<TokenId> = OnceLock::new();
        static SLIDER_H: OnceLock<TokenId> = OnceLock::new();
        static SEG_H: OnceLock<TokenId> = OnceLock::new();
        static SEG_GAP: OnceLock<TokenId> = OnceLock::new();
        static CYC_H: OnceLock<TokenId> = OnceLock::new();
        static BLOCK_H: OnceLock<TokenId> = OnceLock::new();
        static HINT_INSET: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_FRAC: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_MIN: OnceLock<TokenId> = OnceLock::new();
        static BACK_W_MIN_PX: OnceLock<TokenId> = OnceLock::new();
        static GRID_COLS: OnceLock<TokenId> = OnceLock::new();
        Metrics {
            btn_h: th.px(tok(&BTN_H, "button.h")),
            gap: th.px(tok(&ROW_GAP, "modal.row_gap")),
            section_gap: th.px(tok(&SECTION_GAP, "settings.section_gap")),
            slider_h: th.px(tok(&SLIDER_H, "slider.row_h")),
            check_h: th.px(tok(&CHECK_ROW_H, "checkbox.row_h")),
            seg_h: th.px(tok(&SEG_H, "segmented.h")),
            seg_gap: th.px(tok(&SEG_GAP, "segmented.gap")),
            cyc_h: th.px(tok(&CYC_H, "cycler.h")),
            block_h: th.px(tok(&BLOCK_H, "panel.title.block_h")),
            note_h: role_note(ctx).line(),
            hint_h: role_hint(ctx).line(),
            hint_inset: th.px(tok(&HINT_INSET, "settings.hint_inset")),
            corner_w: (content.w * th.px(tok(&BACK_W_FRAC, "settings.back_w_frac")))
                .max(th.px(tok(&BACK_W_MIN, "settings.back_w_min")))
                .max(th.px(tok(&BACK_W_MIN_PX, "settings.back_w_min_min_px"))),
            list_w: content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac")),
            grid_cols: (th.px(tok(&GRID_COLS, "settings.grid_cols")) as usize).max(1),
        }
    }

    fn space(&self, g: Gap) -> f32 {
        match g {
            Gap::None => 0.0,
            Gap::Row => self.gap,
            Gap::Double => self.gap * 2.0,
            Gap::Section => self.section_gap,
        }
    }
}

/// What one row needs to place itself: the page's content box, the box
/// the flow scrolls in, its own band in that flow, and the two column
/// widths the page's rule produced.
#[derive(Clone, Copy)]
struct RowCtx {
    content: Rect,
    /// The scrolled body's box — what the clip holds the row to, and
    /// what a row that lays out its own grid must stop at.
    view: Rect,
    band: Rect,
    label_w: f32,
    value_w: f32,
    m: Metrics,
}

/// Where a page's body starts. A picker shares the chrome's row, which
/// is what `BackInline` names.
fn body_top(page: &Page, m: Metrics, content: Rect) -> f32 {
    match page.chrome {
        Chrome::BackInline => content.y,
        Chrome::Close | Chrome::Back => content.y + m.btn_h + m.space(page.lead),
    }
}

/// The body box of the window: the modal less its title band and its
/// padding.
fn content_rect(modal: Rect) -> Rect {
    static BODY_TOP: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let pad = th.px(tok(&MODAL_PAD, "modal.pad"));
    let body_top = th.px(tok(&BODY_TOP, "modal.body_top"));
    Rect::new(
        modal.x + pad,
        modal.y + body_top,
        modal.w - 2.0 * pad,
        modal.h - body_top - pad,
    )
}

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
    /// The one track a press is currently holding, if any. A track's
    /// rectangle is not kept beside it: the hit map already has it, and
    /// two copies of a geometry are two chances to disagree.
    dragging: Option<Act>,
    dropdown: Option<Dropdown>,
    /// When the dropdown was opened — drives the accordion animation.
    dropdown_since: Option<Instant>,
    /// Grid editor preferences (GRID view).
    grid_snap: bool,
    grid_cols: u32,
    grid_rows: u32,
    /// Widget padding in px (0-40).
    grid_pad: u32,
    /// SOUND view: master volume 0-100 and the two mute switches.
    sound_volume: u32,
    sound_typing: bool,
    sound_ambient: bool,
    /// BLUR view: radius and opacity in percent.
    blur_radius: u32,
    blur_opacity: u32,
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
    /// The body's scroll offset, and its physics. One per window rather
    /// than one per page: every road into a page runs through
    /// [`Settings::go`], and a page reopened halfway down is a page that
    /// opens showing its middle.
    scroll: ScrollView,
    /// The viewport and content length the last frame laid out, and the
    /// clock it ran at. A key arrives outside the drawing and has to ask
    /// somebody how far a page is before it can move it.
    span: (f32, f32),
    now: f64,
    /// The box the body is being clipped to while it draws, so a rect
    /// can be trimmed to what the eye can actually see. None outside the
    /// body: the chrome and the dropdown are not clipped.
    clip: Option<Rect>,
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
            dragging: None,
            dropdown: None,
            dropdown_since: None,
            grid_snap: false,
            grid_cols: GRID_MIN,
            grid_rows: GRID_MIN,
            grid_pad: 8,
            sound_volume: 100,
            sound_typing: true,
            sound_ambient: true,
            blur_radius: 100,
            blur_opacity: 100,
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
            scroll: ScrollView::new(),
            span: (0.0, 0.0),
            now: 0.0,
            clip: None,
            hits: Vec::new(),
            flash: None,
        }
    }

    /// Enters a page. The offset belongs to the page being left, so it
    /// stays with it: every `self.view =` in the window goes through
    /// here, which is the only reason a reopened page starts at its top.
    fn go(&mut self, view: View) {
        self.view = view;
        self.scroll.reset();
    }

    fn sect_idx(sect: Sect) -> usize {
        match sect {
            Sect::Term => 0,
            Sect::Ui => 1,
        }
    }

    /// The rectangle an act was last drawn at. A slider's track and its
    /// hit target are the same thing, so the hit map is where a drag
    /// asks how far along the press landed.
    fn rect_of_act(&self, act: Act) -> Option<Rect> {
        self.hits.iter().find(|(_, a)| *a == act).map(|&(r, _)| r)
    }

    /// Which side effect a track's new value has. Only two tracks have
    /// one: they are the two the application pushes onward every frame.
    fn mark_dirty(&mut self, act: Act) {
        match act {
            Act::VolumeTrack => self.sound_dirty = true,
            Act::BlurRadiusTrack | Act::BlurOpacityTrack => self.blur_dirty = true,
            _ => {}
        }
    }

    /// Sets a track's value from a position along it. The range comes
    /// from the description and nowhere else, so the mouse cannot reach
    /// a value the keyboard cannot.
    fn set_from_x(&mut self, act: Act, x: f32) {
        let Some(&Ctrl::Slider { range: (lo, hi), set, .. }) = slider_of(act) else {
            return;
        };
        let Some(track) = self.rect_of_act(act) else { return };
        let t = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        set(self, (lo as f32 + t * (hi - lo) as f32).round() as u32);
    }

    /// A wheel notch over the open window — the pointer's half of the
    /// scrolling. The window is modal, so nothing under it may take the
    /// turn; how far a notch goes, and whether it glides, is the
    /// theme's (`scroll.*`).
    ///
    /// It is answered here and asked in the event loop, which routes the
    /// wheel and belongs to another stage. `allow(dead_code)` says
    /// exactly that, and comes off the day it is called; until then the
    /// keyboard's PageUp/PageDown/Home/End move the same offset.
    #[allow(dead_code)]
    pub fn wheel(&mut self, notches: f32) {
        if !self.open {
            return;
        }
        self.scroll.wheel(notches, &ScrollPhysics::from_theme(), self.now);
    }

    /// Mouse move while a track is held.
    pub fn drag(&mut self, x: f32) {
        let Some(act) = self.dragging else { return };
        self.set_from_x(act, x);
        self.mark_dirty(act);
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

    /// Mouse button released; returns true when the configuration
    /// changed — the font sizes, which the caller must re-resolve and
    /// re-apply. The rest write themselves and are pushed on by their
    /// dirty flags.
    pub fn release(&mut self) -> bool {
        let Some(act) = self.dragging.take() else { return false };
        if let Some(&Ctrl::Slider { save, .. }) = slider_of(act) {
            save(self);
        }
        matches!(act, Act::SizeTrack(_))
    }

    pub fn show(&mut self) {
        self.open = true;
        self.go(View::Menu);
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
        self.go(View::Grid);
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
                self.go(match self.view {
                    View::Look | View::Layauts | View::Sounds => View::Themes,
                    _ => View::Menu,
                })
            }
            Act::OpenThemes => self.go(View::Themes),
            Act::OpenLook => {
                // Scanned when the view is opened.
                // The engine's themes, not the look/ directories: a look
                // bundled a stylesheet, and stylesheets are gone.
                self.themes = config::list_engine_themes();
                self.refresh_current();
                self.go(View::Look);
            }
            Act::OpenLayauts => {
                self.layauts = config::list_layauts();
                self.refresh_current();
                self.go(View::Layauts);
            }
            Act::OpenSounds => {
                self.sounds = config::list_sound_themes();
                self.refresh_current();
                self.go(View::Sounds);
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
                self.go(View::Sound);
            }
            Act::OpenBlur => {
                let (radius, opacity) = config::blur_prefs();
                self.blur_radius = radius;
                self.blur_opacity = opacity;
                self.go(View::Blur);
            }
            // Every track, in one arm: a press takes the value under
            // the pointer and holds the track until the button is let
            // go. The arm names its acts because exhaustiveness is what
            // makes the compiler ask about the next slider someone
            // adds; WHAT the press does is the description's answer.
            Act::BlurRadiusTrack
            | Act::BlurOpacityTrack
            | Act::VolumeTrack
            | Act::ColsTrack
            | Act::RowsTrack
            | Act::PadTrack
            | Act::SizeTrack(_) => {
                self.dragging = Some(act);
                self.set_from_x(act, x);
                self.mark_dirty(act);
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
                    self.go(View::Color);
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
                self.go(View::Grid);
            }
            Act::OpenBoards => self.go(View::Boards),
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
                self.go(View::Font);
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
    /// values where a mouse sets positions. PageUp/PageDown/Home/End
    /// move the body, which is the keyboard's half of the scrolling a
    /// long page now has.
    pub fn key(&mut self, ev: &KeyEv, fc: &mut FocusCtl) -> KeyOut {
        if !self.open {
            return KeyOut::Ignored;
        }
        // Bare only: the same rule `Nav::of` applies to the arrows —
        // a modified key is a shortcut's business, never navigation.
        if ev.mods == Mods::NONE
            && matches!(ev.key, FKey::PageUp | FKey::PageDown | FKey::Home | FKey::End)
        {
            let (viewport, length) = self.span;
            // One primitive for all four: a page is one viewport, and
            // an end is one whole content length, which the tick's
            // clamp turns into "as far as it goes".
            let (toward_end, by) = match ev.key {
                FKey::PageUp => (false, viewport),
                FKey::PageDown => (true, viewport),
                FKey::Home => (false, length),
                _ => (true, length),
            };
            self.scroll.page(toward_end, by, self.now);
            return KeyOut::Consumed;
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
        let Some(&Ctrl::Slider { range: (lo, hi), step, get, set, save, .. }) =
            slider_of(act)
        else {
            return false;
        };
        let v = get(self) as i64 + dir as i64 * step as i64;
        set(self, v.clamp(lo as i64, hi as i64) as u32);
        save(self);
        self.mark_dirty(act);
        matches!(act, Act::SizeTrack(_))
    }

    /// One interactive rect that no object helper draws (board tiles,
    /// colour chips, cyclers): the click map, the focus chain and the
    /// ring overlay in one motion, so a control cannot be clickable
    /// yet unreachable by keyboard. [`hit_into`] with this window's
    /// own map — loops that already hold a field of `self` call the
    /// free form directly.
    fn hit(&mut self, ctx: &mut Ctx, r: Rect, act: Act) {
        hit_into(&mut self.hits, self.clip, ctx, r, act);
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
        static TITLE_FG: OnceLock<TokenId> = OnceLock::new();

        // Dim the background and draw the window frame (nacelle::object).
        nacelle::object::window::backdrop(ctx, th.px(tok(&SCRIM_A, "modal.scrim_alpha")));
        let modal = modal_rect(ctx.w, ctx.h);
        nacelle::object::window::frame(ctx, modal);

        let page = page(self.view);
        let pad = th.px(tok(&MODAL_PAD, "modal.pad"));
        // The rule under the title is the primitive's own; its colour token
        // (component.panel.header_underline) waits on a header primitive that
        // takes two colours.
        let title_px = role_title(ctx).px;
        ctx.dl.module_title(
            ctx.fonts,
            modal.x + pad,
            modal.y + pad,
            modal.w - 2.0 * pad,
            title_px,
            page.title,
            "",
            col(th.color(tok(&TITLE_FG, "component.panel.title"))),
            true,
        );

        let content = content_rect(modal);
        let m = Metrics::of(ctx, content);
        let corner = Rect::new(content.x, content.y, m.corner_w, m.btn_h);
        let (chrome_act, chrome_label) = match page.chrome {
            Chrome::Close => (Act::Close, "CLOSE"),
            Chrome::Back | Chrome::BackInline => (Act::Back, "BACK"),
        };
        // The corner button takes its place at the HEAD of the focus
        // chain before the body registers anything (R5) — and is PAINTED
        // after it, because the body scrolls under the chrome and a row
        // sliding past must not paint over the way out. Registering and
        // painting are two moments here, and only here.
        let ring = ctx.focus.as_deref_mut().map_or(false, |fc| {
            fc.register(focus_id(chrome_act), corner, Caps::NONE).ring
        });
        self.draw_body(ctx, page, m, content);
        self.button_drawn(ctx, corner, chrome_label, chrome_act, Some(ring));
        // Last, so it covers what it hangs from and the reverse hit walk
        // reaches its rows first.
        self.draw_open_dropdown(ctx);
    }

    /// The box the flowed rows live in: the content box, less the
    /// chrome's own row at the top and less whatever the page pins to
    /// the bottom.
    ///
    /// Three of this window's four drawing faults answer to this one
    /// rectangle. Nothing outside it is drawn (`push_clip`), so no page
    /// reaches the desktop behind the window any more (P9, P10);
    /// nothing outside it is a target, so a scrolled row cannot be
    /// pressed through the chrome painted over it; and the flow stops
    /// short of a pinned footer instead of sharing pixels with it (P12).
    fn body_box(&self, page: &Page, m: Metrics, content: Rect) -> Rect {
        let top = body_top(page, m, content);
        let mut bottom = content.bottom();
        for row in page.rows {
            if row.ctrl.pinned() {
                bottom -= m.gap + self.row_h(page, &row.ctrl, m, content);
            }
        }
        Rect::new(content.x, top, content.w, (bottom - top).max(0.0))
    }

    /// How tall the flowed rows stand together — the scroll's content
    /// length. The last row's trailing gap is not content: a page ends
    /// at its last row, not at the space it asked for after it.
    fn flow_h(&self, page: &Page, m: Metrics, content: Rect) -> f32 {
        let mut h = 0.0;
        let mut trailing = 0.0;
        for row in page.rows {
            if row.ctrl.pinned() {
                continue;
            }
            h += self.row_h(page, &row.ctrl, m, content) + m.space(row.after);
            trailing = m.space(row.after);
        }
        (h - trailing).max(0.0)
    }

    /// The one walker: every row of the page, placed and drawn in the
    /// order the page lists them. Nothing here knows which page it is
    /// walking — that is the whole point of the description.
    ///
    /// The flow runs inside [`Settings::body_box`] and under its clip;
    /// the pinned rows are placed against the content box afterwards,
    /// outside it, which is why the flow can no longer meet them.
    fn draw_body(&mut self, ctx: &mut Ctx, page: &Page, m: Metrics, content: Rect) {
        let (label_w, value_w) = self.columns(ctx, page, content);
        let view = self.body_box(page, m, content);
        let length = self.flow_h(page, m, content);
        // The offset, its clamp, its physics and its bar are the
        // toolkit's (`view::scroll`); the wheel, the page keys and the
        // thumb all move this one number. `Snap::None` because the clip
        // is real — only a surface that cannot clip has to land on whole
        // rows to avoid painting half of one.
        self.now = ctx.t;
        self.span = (view.h, length);
        self.scroll.tick(ctx.t, view.h, length, Snap::None, &ScrollPhysics::from_theme());
        let off = self.scroll.offset();

        ctx.dl.push_clip(view.x, view.y, view.w, view.h);
        self.clip = Some(view);
        let mut y = view.y - off;
        for row in page.rows {
            if row.ctrl.pinned() {
                continue;
            }
            let h = self.row_h(page, &row.ctrl, m, content);
            let band = Rect::new(content.x, y, content.w, h);
            // A row wholly off the viewport is not drawn, and therefore
            // registers nothing: what the eye cannot see is not a
            // target and does not belong in the Tab order either.
            if band.bottom() > view.y && band.y < view.bottom() {
                let rc = RowCtx { content, view, band, label_w, value_w, m };
                if (row.enabled)(self) {
                    self.draw_row(ctx, &row.ctrl, rc);
                } else {
                    self.draw_disabled(ctx, &row.ctrl, rc);
                }
            }
            y += h + m.space(row.after);
        }
        self.clip = None;
        ctx.dl.pop_clip();

        for row in page.rows {
            if !row.ctrl.pinned() {
                continue;
            }
            let h = self.row_h(page, &row.ctrl, m, content);
            let band = Rect::new(content.x, content.bottom() - h, content.w, h);
            let rc = RowCtx { content, view, band, label_w, value_w, m };
            if (row.enabled)(self) {
                self.draw_row(ctx, &row.ctrl, rc);
            } else {
                self.draw_disabled(ctx, &row.ctrl, rc);
            }
        }
        self.draw_scrollbar(ctx, view, length);
    }

    /// Where the page is, when there is more of it than fits. Drawn
    /// after the body so it sits over it, and only ever an indicator:
    /// `scrollbar.auto_hide` is on in the master, so a page at rest
    /// shows nothing and looks exactly as it did.
    fn draw_scrollbar(&mut self, ctx: &mut Ctx, view: Rect, length: f32) {
        let look = ScrollbarLook::from_theme();
        // The band the bar could occupy at its WIDEST: a bar that grows
        // under the pointer must not shrink out from under it.
        let reach = look.w_hover.max(look.w) + look.margin;
        let band = match look.edge {
            scroll::ScrollbarEdge::Left => Rect::new(view.x, view.y, reach, view.h),
            scroll::ScrollbarEdge::Right => {
                Rect::new(view.right() - reach, view.y, reach, view.h)
            }
        };
        let hovered = band.contains(ctx.mouse.0, ctx.mouse.1);
        let Some(geom) = scroll::scrollbar(
            view,
            &look,
            self.scroll.offset(),
            view.h,
            length,
            hovered,
        ) else {
            return;
        };
        let alpha = if hovered {
            1.0
        } else {
            self.scroll.fade_alpha(ctx.t, look.auto_hide, look.fade_ms)
        };
        nacelle::view::paint::scrollbar(&mut CtxSurface::new(ctx), &geom, alpha, hovered, false);
    }

    /// The page's label and value columns, in px.
    fn columns(&self, ctx: &mut Ctx, page: &Page, content: Rect) -> (f32, f32) {
        let th = theme::resolved();
        match page.cols {
            Cols::None => (0.0, 0.0),
            // rhythm.label_col = auto needs a measuring column primitive
            // before this fraction can go.
            Cols::Frac => (
                content.w
                    * th.px(tok(&LABEL_COL, "rhythm.label_col_frac")).clamp(0.0, 1.0),
                0.0,
            ),
            Cols::Measured { label, value } => {
                let f = role_label(ctx);
                let v = role_value(ctx);
                (
                    ctx.fonts.measure(FONT_UI, f.px, label, f.track)
                        + th.px(tok(&LABEL_PAD, "rhythm.label_pad")),
                    ctx.fonts.measure(FONT_UI, v.px, value, v.track)
                        + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter")),
                )
            }
        }
    }

    /// How tall a row is. Everything is a theme length except the two
    /// rows whose height is their content's: a picker's grid and the
    /// boards' reserve.
    fn row_h(&self, page: &Page, ctrl: &Ctrl, m: Metrics, content: Rect) -> f32 {
        match ctrl {
            Ctrl::Toggle { .. } => m.check_h,
            Ctrl::Slider { .. } => m.slider_h,
            Ctrl::Chips { .. } => m.seg_h,
            Ctrl::Cycle { .. } => m.cyc_h,
            Ctrl::Button { .. } => m.btn_h,
            Ctrl::Section { .. } => m.block_h,
            Ctrl::Note { .. } => m.note_h,
            Ctrl::Hint { .. } => m.hint_h,
            Ctrl::Custom { h, .. } => h(m, content),
            Ctrl::Picker { list } => {
                let n = self.names(*list).len();
                let first = match page.chrome {
                    Chrome::BackInline => 1,
                    Chrome::Close | Chrome::Back => 0,
                };
                let rows = (first + n).max(1).div_ceil(m.grid_cols) as f32;
                let mut h = rows * m.btn_h + (rows - 1.0) * m.gap;
                if n == 0 {
                    // The empty note stands under the corner button.
                    h += m.gap + m.note_h;
                }
                h
            }
        }
    }

    /// The hit map takes the VISIBLE part of a rect. The clip already
    /// draws a scrolled row away; without this the hand could still find
    /// what the eye has lost.
    fn push_hit(&mut self, r: Rect, act: Act) {
        if let Some(seen) = visible(r, self.clip) {
            self.hits.push((seen, act));
        }
    }

    /// One row, drawn in the band the walker gave it.
    fn draw_row(&mut self, ctx: &mut Ctx, ctrl: &Ctrl, rc: RowCtx) {
        let th = theme::resolved();
        match ctrl {
            // The whole row toggles (nacelle::object).
            Ctrl::Toggle { label, get, act } => {
                let hover = rc.band.contains(ctx.mouse.0, ctx.mouse.1);
                let on = get(self);
                nacelle::object::checkbox::draw_focusable(
                    ctx,
                    rc.band,
                    label,
                    on,
                    hover,
                    focus_id(*act),
                );
                self.push_hit(rc.band, *act);
            }
            Ctrl::Slider { label, act, unit, range: (lo, hi), get, .. } => {
                self.row_label(ctx, label, rc);
                let track = Rect::new(
                    rc.content.x + rc.label_w,
                    rc.band.y,
                    rc.content.w - rc.label_w - rc.value_w,
                    rc.band.h,
                );
                let value = get(self);
                let t = ((value as f32 - *lo as f32) / (*hi - *lo) as f32).clamp(0.0, 1.0);
                nacelle::object::slider::track_focusable(ctx, track, t, focus_id(*act));
                let v = role_value(ctx);
                let vy = center_y(ctx, rc.band, v);
                ctx.dl.text_right(
                    ctx.fonts,
                    FONT_UI,
                    v.px,
                    rc.content.right(),
                    vy,
                    &unit.text(value),
                    col(th.color(tok(&VALUE_FG, "component.columns.value"))),
                    v.track,
                );
                self.push_hit(track, *act);
            }
            Ctrl::Chips { label, values, get, act } => {
                self.draw_chips(ctx, label, values, *get, *act, rc)
            }
            Ctrl::Cycle { label, get, act } => {
                let value = get(self);
                self.draw_cycle(ctx, label, &value, *act, rc)
            }
            Ctrl::Picker { list } => self.draw_picker(ctx, *list, rc),
            Ctrl::Button { label, kind, act } => {
                let r = Self::button_rect(*kind, rc);
                let text = self.text_of(*label);
                self.button(ctx, r, &text, *act);
            }
            // A separator like every other module header.
            Ctrl::Section { title } => {
                static SECTION_FG: OnceLock<TokenId> = OnceLock::new();
                let px = role_section(ctx).px;
                ctx.dl.module_title(
                    ctx.fonts,
                    rc.content.x,
                    rc.band.y,
                    rc.content.w,
                    px,
                    title,
                    "",
                    col(th.color(tok(&SECTION_FG, "component.panel.title"))),
                    true,
                );
            }
            Ctrl::Note { text } => {
                let n = role_note(ctx);
                // The band IS one line, so the centring adds nothing but
                // the cap-height bias — which is the point: every run in
                // this window sits by the same rule.
                let ny = center_y(ctx, rc.band, n);
                let s = self.text_of(*text);
                ctx.dl.text(
                    ctx.fonts,
                    FONT_UI,
                    n.px,
                    rc.content.x,
                    ny,
                    &s,
                    col(th.color(tok(&MUTED_FG, "text.muted"))),
                    n.track,
                );
            }
            // One line that explains the other way in (settings.hint.role).
            Ctrl::Hint { text } => {
                let n = role_hint(ctx);
                let s = self.text_of(*text);
                ctx.dl.text_center(
                    ctx.fonts,
                    FONT_UI,
                    n.px,
                    rc.content.cx(),
                    rc.content.bottom() - rc.m.hint_inset,
                    &s,
                    col(th.color(tok(&MUTED_FG, "text.muted"))),
                    n.track,
                );
            }
            Ctrl::Custom { draw, .. } => draw(self, ctx, rc.band),
        }
    }

    /// A row the page turned off. Only a button has a disabled form —
    /// the ladder's Disabled rung, an inscription, and nothing in the
    /// hit map or the focus chain (R6).
    fn draw_disabled(&mut self, ctx: &mut Ctx, ctrl: &Ctrl, rc: RowCtx) {
        let Ctrl::Button { label, kind, .. } = ctrl else { return };
        let th = theme::resolved();
        let r = Self::button_rect(*kind, rc);
        let st = ladder(th, &BTN_CLASS, "button", State::Disabled);
        ctx.dl.rect_outline(r.x, r.y, r.w, r.h, st.edge_width, col(st.edge));
        let f = role_button(ctx);
        let ty = center_y(ctx, r, f);
        let s = self.text_of(*label);
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            f.px,
            r.cx(),
            ty,
            &s,
            col(st.text),
            f.track,
        );
    }

    /// A row's label in the label column, written once for the four
    /// kinds of row that have one (settings.row_label_role).
    fn row_label(&self, ctx: &mut Ctx, label: &str, rc: RowCtx) {
        let th = theme::resolved();
        let f = role_label(ctx);
        let ty = center_y(ctx, rc.band, f);
        ctx.dl.text(
            ctx.fonts,
            FONT_UI,
            f.px,
            rc.content.x,
            ty,
            label,
            col(th.color(tok(&LABEL_FG, "component.columns.label"))),
            f.track,
        );
    }

    fn button_rect(kind: BtnKind, rc: RowCtx) -> Rect {
        let x = rc.content.x + (rc.content.w - rc.m.list_w) / 2.0;
        match kind {
            BtnKind::Listed => Rect::new(x, rc.band.y, rc.m.list_w, rc.m.btn_h),
            BtnKind::Wide => {
                Rect::new(rc.content.x, rc.band.y, rc.content.w, rc.m.btn_h)
            }
            BtnKind::Footer => Rect::new(
                x,
                rc.content.bottom() - rc.m.btn_h,
                rc.m.list_w,
                rc.m.btn_h,
            ),
        }
    }

    /// A row's text, resolved. Owned or static either way, so reading it
    /// does not borrow the window while the window draws itself.
    fn text_of(&self, t: Text) -> Cow<'static, str> {
        match t {
            Text::Fixed(s) => Cow::Borrowed(s),
            Text::Of(f) => Cow::Owned(f(self)),
        }
    }

    /// Fixed segments, one of them on; the segment count is data, not
    /// theme.
    fn draw_chips(
        &mut self,
        ctx: &mut Ctx,
        label: &str,
        values: &[u32],
        get: fn(&Settings) -> u32,
        act: fn(u32) -> Act,
        rc: RowCtx,
    ) {
        static SEG_BORDER: OnceLock<TokenId> = OnceLock::new();
        static SEG_BORDER_ON: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        self.row_label(ctx, label, rc);
        // A segment's own text sits in the row's role, which is where
        // this control has always set it; `segmented.role` is a size
        // change and so belongs to the stage that moves furniture.
        let f = role_label(ctx);
        let n = values.len().max(1) as f32;
        let cw = (rc.content.w - rc.label_w - rc.m.seg_gap * (n - 1.0)) / n;
        let cur = get(self);
        for (i, bits) in values.iter().enumerate() {
            let r = Rect::new(
                rc.content.x + rc.label_w + (cw + rc.m.seg_gap) * i as f32,
                rc.band.y,
                cw,
                rc.band.h,
            );
            let hover = r.contains(ctx.mouse.0, ctx.mouse.1);
            let on = cur == *bits;
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
            let ty = center_y(ctx, rc.band, f);
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                f.px,
                r.cx(),
                ty,
                &bits.to_string(),
                col(st.text),
                f.track,
            );
            self.hit(ctx, r, act(*bits));
        }
    }

    /// The current value in a button-like slot; a click steps to the
    /// next entry, wrapping through NONE where a file may be absent. No
    /// chevrons yet — cycler.chevron_* wait on the affordance being
    /// drawn at all.
    fn draw_cycle(&mut self, ctx: &mut Ctx, label: &str, value: &str, act: Act, rc: RowCtx) {
        static CYC_BORDER: OnceLock<TokenId> = OnceLock::new();
        static VALUE_TXT: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        self.row_label(ctx, label, rc);
        // Same as the segments: the value keeps the row's role until a
        // cycler role is a decision someone has made.
        let f = role_label(ctx);
        let r = Rect::new(
            rc.content.x + rc.label_w,
            rc.band.y,
            rc.content.w - rc.label_w,
            rc.band.h,
        );
        let hover = r.contains(ctx.mouse.0, ctx.mouse.1);
        let st = ladder(
            th,
            &BTN_CLASS,
            "button",
            if hover { State::Hover } else { State::Idle },
        );
        ctx.dl.rect_outline(
            r.x,
            r.y,
            r.w,
            r.h,
            th.px(tok(&CYC_BORDER, "cycler.border")),
            col(st.edge),
        );
        let ty = center_y(ctx, rc.band, f);
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            f.px,
            r.cx(),
            ty,
            &value.to_uppercase(),
            col(th.color(tok(&VALUE_TXT, "text.primary"))),
            f.track,
        );
        self.hit(ctx, r, act);
    }

    /// The names of one list, in the order they are offered.
    fn names(&self, list: ListId) -> &[String] {
        match list {
            ListId::Looks => &self.themes,
            ListId::Layauts => &self.layauts,
            ListId::Sounds => &self.sounds,
        }
    }

    /// Whether row `i` of a list is the entry the configuration names.
    fn is_selected(&self, list: ListId, i: usize) -> bool {
        let current = match list {
            ListId::Looks => self.current_look.as_ref(),
            ListId::Layauts => self.current_layaut.as_ref(),
            ListId::Sounds => self.current_sounds.as_ref(),
        };
        current.is_some() && self.names(list).get(i) == current
    }

    /// A list of names filling the page: the corner button holds the
    /// first cell and the names take the rest.
    ///
    /// The grid is laid out in full and only DRAWN where the viewport
    /// reaches, which is the whole of P11: a name past the bottom edge
    /// used to end the loop and vanish — no bar, no count, no notice —
    /// and now it is simply a scroll away, because the offset the walker
    /// applies runs to the end of the description and not to the end of
    /// the box.
    fn draw_picker(&mut self, ctx: &mut Ctx, list: ListId, rc: RowCtx) {
        let cols = rc.m.grid_cols;
        let bw = (rc.content.w - rc.m.gap * (cols as f32 - 1.0)) / cols as f32;
        let make_act = list.act();
        let n = self.names(list).len();
        let mut c = 1usize; // the first row starts next to the corner button
        let mut y = rc.band.y;
        for i in 0..n {
            if c >= cols {
                c = 0;
                y += rc.m.btn_h + rc.m.gap;
            }
            if y + rc.m.btn_h > rc.view.y && y < rc.view.bottom() {
                let label = self.names(list)[i].to_uppercase();
                let br =
                    Rect::new(rc.content.x + c as f32 * (bw + rc.m.gap), y, bw, rc.m.btn_h);
                self.button(ctx, br, &label, make_act(i));
            }
            c += 1;
        }
        if n == 0 {
            self.empty_note(ctx, rc, list.empty_note());
        }
    }

    /// `emptystate.role` says how the "nothing here" line is set;
    /// `text.muted` is the ink an aside is written in.
    fn empty_note(&mut self, ctx: &mut Ctx, rc: RowCtx, note: &str) {
        let th = theme::resolved();
        let v = role_empty(ctx);
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            v.px,
            rc.content.cx(),
            rc.band.y + rc.m.btn_h + rc.m.gap,
            note,
            col(th.color(tok(&MUTED_FG, "text.muted"))),
            v.track,
        );
    }

    /// The open dropdown, drawn over everything else. Its anchor is
    /// wherever the button that owns it was drawn this frame, which the
    /// hit map already knows — an anchor kept beside it would be a
    /// second copy of a geometry. That is the VISIBLE part of the
    /// button: a list hangs from the edge the eye sees, and an anchor
    /// scrolled off the page has nothing to hang from at all.
    fn draw_open_dropdown(&mut self, ctx: &mut Ctx) {
        static MENU_ROW_H: OnceLock<TokenId> = OnceLock::new();
        let Some(d) = self.dropdown else { return };
        let anchor_act = match d {
            Dropdown::Family(s) => Act::FamilyBtn(s),
            Dropdown::Weight(s) => Act::WeightBtn(s),
        };
        let Some(anchor) = self.rect_of_act(anchor_act) else { return };
        let item_h = theme::resolved().px(tok(&MENU_ROW_H, "menu.row_h"));
        match d {
            Dropdown::Family(sect) => {
                let si = Self::sect_idx(sect);
                // First entry: DEFAULT (auto-detected font).
                let mut names = vec!["DEFAULT".to_string()];
                names.extend(self.families[si].iter().map(|f| f.to_uppercase()));
                self.draw_dropdown(ctx, anchor, item_h, &names, dropdown_base(d), |i| {
                    Act::FamilyPick(sect, i)
                });
            }
            Dropdown::Weight(sect) => {
                let names: Vec<String> =
                    WEIGHTS.iter().map(|w| w.to_uppercase()).collect();
                self.draw_dropdown(ctx, anchor, item_h, &names, dropdown_base(d), |i| {
                    Act::WeightPick(sect, i)
                });
            }
        }
    }

    /// The boards laid out the way they sit in the world — the
    /// horizontal row centred on home, with the permanent top and
    /// bottom boards above and below. Clicking one the user is not on
    /// goes there with this window still open, which is why no board
    /// needs a control panel of its own. The small [+] tiles at the two
    /// ends of the row add a board on that side; the x removes one. The
    /// top and bottom boards, like home, have neither.
    ///
    /// The one page a row cannot describe, and the only [`Ctrl::Custom`]
    /// in the file.
    fn draw_boards(&mut self, ctx: &mut Ctx, area: Rect) {
        let th = theme::resolved();
        // Read out before the walk: the loop below holds `self.boards`,
        // which is why its rects go through the free `hit_into`.
        let clip = self.clip;
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

        let f = role_caption(ctx);
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
        let cap_strip = th.px(tok(&CAP_GAP, "boards.tile.caption_gap"))
            + th.px(tok(&CAP_H, "boards.tile.caption_h"));
        let cols = (r - l + 1) as f32;
        let rows = (d - u + 1) as f32;
        let tile_w = ((area.w - 2.0 * (plus + tgap) - (cols - 1.0) * tgap) / cols)
            .min(area.w * th.px(tok(&TILE_MAX_W, "boards.tile.max_w_frac")))
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
        let wc_close =
            col(th.color(tok(&WC_CLOSE_HOVER, "component.window_control.close_hover")));

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
                hit_into(&mut self.hits, clip, ctx, tile, Act::BoardGo(b.id));
            }
            if b.id != (0, 0) && by == 0 {
                let xs =
                    th.px(tok(&CLOSE_SIZE, "boards.tile.close_size")).min(tile.w * 0.2);
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
                hit_into(&mut self.hits, clip, ctx, xr, Act::BoardDel(b.id));
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
            // Deliberately unclipped: the list hangs past the window's
            // edge and stays pressable there, which is the one place in
            // this file where a target may leave the body's box.
            self.hits.push((r, make_act(i)));
        }
    }

    /// Button in the terminal-tab style (slant, hover, flash on click),
    /// joining the focus chain where it is drawn.
    fn button(&mut self, ctx: &mut Ctx, r: Rect, label: &str, act: Act) {
        self.button_drawn(ctx, r, label, act, None)
    }

    /// [`Settings::button`], with the chain membership spelled out.
    /// `None` registers here, which is what every button in the body
    /// wants. `Some(ring)` is the chrome: it registered earlier, to hold
    /// the head of the chain, and carries the answer down to the moment
    /// it is finally painted.
    fn button_drawn(
        &mut self,
        ctx: &mut Ctx,
        r: Rect,
        label: &str,
        act: Act,
        ring: Option<bool>,
    ) {
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
            Act::Look(i) => self.is_selected(ListId::Looks, i),
            Act::Layaut(i) => self.is_selected(ListId::Layauts, i),
            Act::Sounds(i) => self.is_selected(ListId::Sounds, i),
            _ => false,
        };
        let st = nacelle::object::button::ButtonState { hover, flash, selected: is_current };
        // The plate, and the ring the chain owes it (F1 §1.3). BACK's
        // arrow and label draw after it, which is fine — the ring sits
        // outside the quad and overlaps neither.
        let plate = if act == Act::Back { "" } else { label };
        match ring {
            None => {
                nacelle::object::button::draw_focusable(ctx, r, plate, st, focus_id(act))
            }
            Some(owed) => {
                nacelle::object::button::draw(ctx, r, plate, st);
                if owed {
                    nacelle::object::focus_ring::draw_quad(
                        ctx,
                        nacelle::object::button::quad(&r),
                    );
                }
            }
        }
        if act == Act::Back {
            // A left arrow, and the label shifted to make room for it.
            // Arrow and label share the ladder's text colour — the arrow
            // is a quad, so no glyph token applies.
            let f = role_button(ctx);
            let ty = center_y(ctx, r, f);
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
                ty,
                label,
                color,
                f.track,
            );
        }
        self.push_hit(r, act);
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

    /// [`page`] indexes [`PAGES`] by the view's discriminant, so the
    /// table has to stand in the enum's order. Out of order, every view
    /// would draw a neighbour's page.
    #[test]
    fn pages_are_in_view_order() {
        for (i, p) in PAGES.iter().enumerate() {
            assert!(p.view as usize == i, "PAGES[{i}] is out of order");
            assert!(std::ptr::eq(page(p.view), p));
        }
    }

    /// A window with enough in it to draw every page: three names in
    /// every list, colour enabled, one board.
    fn furnished() -> Settings {
        let mut s = Settings::new();
        s.open = true;
        s.color_enabled = true;
        let names: Vec<String> =
            ["one", "two", "three"].iter().map(|n| n.to_string()).collect();
        s.themes = names.clone();
        s.layauts = names.clone();
        s.sounds = names;
        s.current_look = Some("one".to_string());
        s.boards = vec![BoardThumb { id: (0, 0), current: true, panels: Vec::new() }];
        s
    }

    /// A drawing context at one window height and one interface scale:
    /// no pointer, no focus, no panel shrink — the resting state every
    /// measurement below is taken in.
    fn probe<'a>(
        dl: &'a mut nacelle::draw::DrawList,
        fonts: &'a mut nacelle::font::FontSystem,
        h: f32,
        ui_font_scale: f32,
    ) -> Ctx<'a> {
        Ctx {
            dl,
            fonts,
            w: h * 16.0 / 9.0,
            h,
            t: 0.0,
            mouse: (-1.0, -1.0),
            term_font_scale: 1.0,
            ui_font_scale,
            panel_scale: 1.0,
            focus: None,
            tips: None,
        }
    }

    /// Every window height the program is built for. A page's geometry
    /// is its description's, so a measurement at each of them needs no
    /// GPU — which is the only reason P9 and P10 can be tested at all.
    const HEIGHTS: [f32; 5] = [500.0, 720.0, 1080.0, 1440.0, 2160.0];

    /// The engine's own default, so the tests that share the theme lock
    /// find the viewport they expect.
    fn viewport_home() {
        theme::set_viewport(1080.0, 1.0);
    }

    /// §8.3/1, part one — the body's box is inside the window, at every
    /// height and on every page.
    ///
    /// This is what P9 and P10 were: the main menu overran the content
    /// box by about 7.6 px and the FONT view by about 19.4 px, at every
    /// resolution, because every length scales with the window and the
    /// sum was simply too big. Nothing clipped, so the surplus landed on
    /// the desktop behind the window.
    ///
    /// The box below is the answer to both. Everything the flow draws is
    /// held to it, so a page can be too long for its window — pages are,
    /// and always will be — without a pixel of it escaping.
    #[test]
    fn the_bodys_box_never_leaves_the_content_box() {
        let _g = crate::widgets::theme_test_lock();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            // Two of a page's lengths are type, so the measurement needs
            // the scales a frame would carry — at rest, which is 1.0.
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            for p in PAGES.iter() {
                let view = s.body_box(p, m, content);
                let where_ = format!("{} at {h}px", p.title);
                assert!(view.h > 0.0, "{where_}: no room for a body at all");
                assert!(view.y >= content.y - 0.01, "{where_}: the body starts above the box");
                assert!(
                    view.bottom() <= content.bottom() + 0.01,
                    "{where_}: the body ends {} px past the box",
                    view.bottom() - content.bottom()
                );
            }
        }
        viewport_home();
    }

    /// §8.3/1, part two — the whole of a page can be brought into that
    /// box, and the flow never reaches a row pinned under it (P12).
    ///
    /// Scrolled to its end, the last row of every page stands exactly on
    /// the bottom edge of the body's box: not short of it (which would
    /// mean the offset stops before the content does) and not past it
    /// (which would mean a row nobody can reach). RESET THIS SCREEN used
    /// to share pixels with the rows above it and win the click by
    /// standing later in the hit map; it now stands outside the box the
    /// rows are held to, so the two can no longer meet.
    #[test]
    fn every_row_can_be_brought_into_view_and_no_pinned_row_is_in_the_way() {
        let _g = crate::widgets::theme_test_lock();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            for p in PAGES.iter() {
                let view = s.body_box(p, m, content);
                let length = s.flow_h(p, m, content);
                let furthest = (length - view.h).max(0.0);
                let where_ = format!("{} at {h}px", p.title);
                // Walk the description to the last flowed row, at the
                // furthest the offset goes.
                let mut y = view.y - furthest;
                let mut last = None;
                for row in p.rows {
                    if row.ctrl.pinned() {
                        continue;
                    }
                    let rh = s.row_h(p, &row.ctrl, m, content);
                    last = Some(y + rh);
                    y += rh + m.space(row.after);
                }
                let Some(end) = last else { continue };
                assert!(
                    end <= view.bottom() + 0.01,
                    "{where_}: the last row ends {} px below the body, unreachable",
                    end - view.bottom()
                );
                if furthest > 0.0 {
                    assert!(
                        end >= view.bottom() - 0.01,
                        "{where_}: the offset stops {} px short of the end",
                        view.bottom() - end
                    );
                }
                // P12: whatever the page pins, the flow cannot touch it.
                for row in p.rows {
                    if !row.ctrl.pinned() {
                        continue;
                    }
                    let rh = s.row_h(p, &row.ctrl, m, content);
                    let pinned_top = content.bottom() - rh;
                    assert!(
                        view.bottom() <= pinned_top + 0.01,
                        "{where_}: the body overlaps a pinned row by {} px",
                        view.bottom() - pinned_top
                    );
                }
            }
        }
        viewport_home();
    }

    /// §8.3/1, part three — nothing the window draws is a target
    /// outside the window's own body.
    ///
    /// The measurements above are the description's; this is the
    /// drawing's. Every page is drawn at every height, at rest and
    /// scrolled to its end, and every rect that ends up in the hit map
    /// is held against the content box. Before the clip, the main menu's
    /// BLUR button and the FONT view's second WEIGHT button answered the
    /// pointer below the window's own edge.
    #[test]
    fn no_page_leaves_a_target_outside_the_window() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            for p in PAGES.iter() {
                for at_end in [false, true] {
                    let mut s = furnished();
                    s.view = p.view;
                    let mut dl = nacelle::draw::DrawList::new();
                    let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    if at_end {
                        s.scroll.set_offset(f32::MAX / 4.0);
                    }
                    s.draw(&mut ctx);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    for (r, _) in &s.hits {
                        assert!(
                            r.y >= content.y - 0.01
                                && r.bottom() <= content.bottom() + 0.01,
                            "{} at {h}px: a target sits {:?} outside {:?}",
                            p.title,
                            (r.y, r.bottom()),
                            (content.y, content.bottom())
                        );
                    }
                }
            }
        }
        viewport_home();
    }

    /// The body is clipped, and the stack it pushes is the one it pops.
    ///
    /// `settings.rs` never called `push_clip` at all — the draw list has
    /// had it all along — which is why an overlong page reached the
    /// desktop. An unbalanced push would be worse than none: the clip
    /// would outlive the window and cut whatever drew next.
    #[test]
    fn the_body_draws_under_a_balanced_clip() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for p in PAGES.iter() {
            let mut s = furnished();
            s.view = p.view;
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let view = s.body_box(p, m, content);
            let clips: Vec<[f32; 4]> = dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::ClipPush { r } => Some(*r),
                    _ => None,
                })
                .collect();
            assert!(
                clips.contains(&[view.x, view.y, view.w, view.h]),
                "{}: the body's box {:?} was never pushed; pushed {clips:?}",
                p.title,
                (view.x, view.y, view.w, view.h)
            );
            let pops = dl
                .cmds()
                .iter()
                .filter(|c| matches!(c, nacelle::draw::DrawCmd::ClipPop))
                .count();
            assert_eq!(
                pops,
                clips.len(),
                "{}: the clip stack is left standing",
                p.title
            );
        }
    }

    /// The corner button is painted after the body it scrolls, so that
    /// a row sliding past cannot paint over the way out. That is only
    /// safe while the chrome and the body do not share pixels when the
    /// body is at rest — and the one page where they stand in the same
    /// row is the picker, whose first name sits in the grid's second
    /// column. This is the gap between them, at every height.
    #[test]
    fn the_corner_button_never_reaches_the_first_list_cell() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let cell = (content.w - m.gap * (m.grid_cols as f32 - 1.0)) / m.grid_cols as f32;
            assert!(
                m.corner_w <= cell + m.gap,
                "at {h}px BACK is {} px wide and the second column starts at {}",
                m.corner_w,
                cell + m.gap
            );
        }
        viewport_home();
    }

    /// P11 — a list loses no name, however long it is.
    ///
    /// The grid used to `break` out of its loop at the bottom edge of
    /// the content box: past about twenty entries a name simply was not
    /// there, with no bar, no count and no notice. Forty names are laid
    /// out here and the view is walked from top to bottom; every one of
    /// them has to appear, and the ones that do have to be inside the
    /// body's box while they do.
    #[test]
    fn a_long_list_loses_no_name() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        const N: usize = 40;
        let names: Vec<String> = (0..N).map(|i| format!("theme {i}")).collect();
        let p = page(View::Look);
        let stocked = |offset: f32| {
            let mut s = furnished();
            s.view = View::Look;
            s.themes = names.clone();
            s.scroll.set_offset(offset);
            s
        };
        // The travel, and a step of one row: no name can slip between
        // two frames of the walk.
        let (furthest, step) = {
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let s = stocked(0.0);
            ((s.flow_h(p, m, content) - s.body_box(p, m, content).h).max(0.0), m.btn_h)
        };
        assert!(furthest > 0.0, "forty names ought not to fit");
        assert!(step > 0.0, "a row has a height");
        let mut seen = vec![false; N];
        let mut offset = 0.0f32;
        while offset <= furthest + step {
            let mut s = stocked(offset);
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            for (_, act) in &s.hits {
                if let Act::Look(i) = act {
                    seen[*i] = true;
                }
            }
            offset += step;
        }
        let lost: Vec<usize> = (0..N).filter(|i| !seen[*i]).collect();
        assert!(lost.is_empty(), "names {lost:?} were never drawn");
        viewport_home();
    }

    /// §5.3 — the window's text answers UIFontSize=, all of it.
    ///
    /// Every page is drawn twice, at the two ends of the range the FONT
    /// view's own INTERFACE slider offers (30 % and 125 %), and the two
    /// frames' text commands are paired in call order. A run whose size
    /// did not move between them is a run set from a px the preference
    /// never reached — which is what this window used to be made of:
    /// `object::button` scaled its labels while the row labels, values,
    /// notes, captions and the title around them stayed at 100 %, so the
    /// one screen that sets the interface size was the one screen where
    /// setting it half worked.
    #[test]
    fn every_run_in_the_window_answers_the_interface_scale() {
        /// Every run one frame wrote, as (text, px) in call order.
        fn runs(dl: &nacelle::draw::DrawList) -> Vec<(String, f32)> {
            dl.cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Text { text, px, .. } => {
                        Some((text.clone(), *px))
                    }
                    nacelle::draw::DrawCmd::ModuleTitle { left, px, .. } => {
                        Some((left.clone(), *px))
                    }
                    _ => None,
                })
                .collect()
        }
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for p in PAGES.iter() {
            let mut frames: Vec<Vec<(String, f32)>> = Vec::new();
            for scale in [0.3f32, 1.25] {
                let mut s = furnished();
                s.view = p.view;
                let mut dl = nacelle::draw::DrawList::recording();
                let mut ctx = probe(&mut dl, &mut fonts, 1080.0, scale);
                s.draw(&mut ctx);
                frames.push(runs(&dl));
            }
            let (small, big) = (&frames[0], &frames[1]);
            assert_eq!(
                small.len(),
                big.len(),
                "{}: the two frames drew different text",
                p.title
            );
            assert!(!small.is_empty(), "{}: the page wrote nothing", p.title);
            for ((ts, ps), (tb, pb)) in small.iter().zip(big) {
                assert_eq!(ts, tb, "{}: the runs fell out of step", p.title);
                assert!(
                    pb > ps,
                    "{}: \"{ts}\" is {ps} px either way — it does not \
                     read the interface scale",
                    p.title
                );
            }
        }
    }

    /// §8.3/2 — everything a page describes is reachable.
    ///
    /// Every row that carries an act must land in the hit map AND in the
    /// focus chain: a control the mouse can press but Tab cannot reach
    /// is the bug this window used to have one control at a time. The
    /// BOARDS cross is skipped — it is the one page whose contents no
    /// row describes ([`Ctrl::Custom`]).
    ///
    /// Two frames per page, at rest and scrolled to the end, and the
    /// union of the two has to hold everything: a row is off screen for
    /// part of the travel now, and off screen is off the chain — but
    /// there must be no row that is off screen for all of it.
    #[test]
    fn every_described_control_is_reachable() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for p in PAGES.iter() {
            let mut hit: Vec<Act> = Vec::new();
            let mut chained: Vec<Act> = Vec::new();
            let mut reference = furnished();
            reference.view = p.view;
            for at_end in [false, true] {
                let mut s = furnished();
                s.view = p.view;
                if at_end {
                    s.scroll.set_offset(f32::MAX / 4.0);
                }
                let mut fc = FocusCtl::new();
                let mut dl = nacelle::draw::DrawList::new();
                fc.begin_frame();
                let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
                ctx.focus = Some(&mut fc);
                s.draw(&mut ctx);
                // The chain answers about the last COMPLETED frame, so
                // the frame the drawing built has to be closed before it
                // can be read back.
                fc.begin_frame();
                hit.extend(s.hits.iter().map(|&(_, a)| a));
                chained.extend(
                    described_acts(&s, p)
                        .into_iter()
                        .filter(|a| fc.rect_of(focus_id(*a)).is_some()),
                );
            }
            for act in described_acts(&reference, p) {
                assert!(
                    hit.contains(&act),
                    "{}: a described control is missing from the hit map",
                    p.title
                );
                assert!(
                    chained.contains(&act),
                    "{}: a described control never joined the focus chain",
                    p.title
                );
            }
        }
    }

    /// Every act a page promises, chrome included.
    fn described_acts(s: &Settings, page: &Page) -> Vec<Act> {
        let mut out = vec![match page.chrome {
            Chrome::Close => Act::Close,
            Chrome::Back | Chrome::BackInline => Act::Back,
        }];
        for row in page.rows {
            if !(row.enabled)(s) {
                continue;
            }
            match &row.ctrl {
                Ctrl::Toggle { act, .. }
                | Ctrl::Slider { act, .. }
                | Ctrl::Cycle { act, .. }
                | Ctrl::Button { act, .. } => out.push(*act),
                Ctrl::Chips { values, act, .. } => {
                    out.extend(values.iter().map(|v| act(*v)))
                }
                Ctrl::Picker { list } => {
                    let make = list.act();
                    out.extend((0..s.names(*list).len()).map(make));
                }
                Ctrl::Section { .. }
                | Ctrl::Note { .. }
                | Ctrl::Hint { .. }
                | Ctrl::Custom { .. } => {}
            }
        }
        out
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

