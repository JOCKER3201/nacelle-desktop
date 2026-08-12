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
//! What the pages hold: LOOK AND FEEL is one page carrying the three
//! choices that say which installed set is in use — THEMES (the theme
//! engine's themes, written as Theme=), LAYAUTS (Layaut=) and SOUNDS
//! (Sounds=) — plus the doors to the pages behind it. Each of the three
//! is a DROP-DOWN and not a page of its own, so the page at rest is six
//! rows and only ever one list is unfolded. A theme comes from the
//! toolkit (the eight compiled in plus anything on the search path);
//! layouts and sound sets are read from the data directories.
//! Everything applies live.
//!
//! TWO WORDS THAT ARE NOT THE SAME THING. `SOUNDS` is the drop-down
//! that picks WHICH SET of clips is installed and in force (Sounds=),
//! and `SOUND LEVELS` is the button that opens how loudly and how often
//! that set is played — volume, typing, ambient. They stand one under
//! the other on this page, so the two labels have to tell a reader
//! apart what a single word "SOUND" would have run together: a set is a
//! choice, a level is a dial. The button's own act
//! ([`Act::OpenSoundLevels`]) and view ([`View::SoundLevels`]) carry the
//! longer name for the same reason.
//!
//! An anchor wears its LIST'S NAME and not the choice standing in it
//! (decision §2b) — THEMES, LAYAUTS, SOUNDS — against the convention
//! of the font page, whose anchors read "FAMILY: TERMINUS". Which
//! member of a set is in force is said INSIDE the open list instead, by
//! the row that wears the `menu.item` ladder's `selected` rung
//! ([`nacelle::object::dropdown::AccordionStyle::current`]).
//!
//! The THEMES list carries the door to the theme editor at the top of
//! its unfolded body (decision §3), and that door is a BUTTON and not a
//! row of the list. It has to be: a list whose whole role is "choose
//! one of these" may not offer a member that is not a choice, and a row
//! set apart by a hairline is a hair among hairs — every row of an
//! accordion already has a seam above it. A button is a different
//! object, a different height, a different ladder and a different type
//! role, which is what "not a choice" has to look like. It is also what
//! the page's other two doors already are, so the three read alike.
//!
//! LOOK AND FEEL pins one more row: LOOK AND FEEL RESET, which clears
//! everything the page and its doors write (decision §2a) and is
//! therefore the one destructive control in this window. It spends six
//! settings and a pinned arrangement with nothing to undo them, so it
//! opens a confirmation ([`View::LookFeelReset`]) that names what is
//! about to go, and the press that does the work stands on that page
//! and nowhere else.

use super::{Ctx, PanelSpec, Rect};
use std::borrow::Cow;
use crate::config::{self, GRID_MAX, GRID_MIN};
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
    LookFeel,
    /// The confirmation LOOK AND FEEL RESET stands behind (decision
    /// §2a): what the reset clears, named, and the one control that
    /// does it.
    LookFeelReset,
    ThemeEditor,
    Font,
    Grid,
    /// How loudly the installed sound set is played — NOT which set it
    /// is, which is LOOK AND FEEL's `SOUNDS` list.
    SoundLevels,
    Boards,
    Color,
    Blur,
}

/// The view one layer out, or `None` at the outermost one.
///
/// The Escape ladder and the BACK button read the SAME answer, which is
/// what stops the two ways out of a page from disagreeing. The window's
/// own last layer — closing it — is not here: that is the application's
/// Escape ([`KeyOut::Ignored`]), and this window peels one layer per
/// press until there is none left to peel.
fn parent_view(v: View) -> Option<View> {
    match v {
        View::Menu => None,
        // All FOUR of LOOK AND FEEL's doors lead back to it: the editor
        // stands at the head of its THEMES list, the levels page and
        // the font page are what its two buttons open, and the reset
        // confirmation is what its footer opens. That the way back out
        // of the confirmation is the ordinary one is the point — a
        // destructive control the user changed their mind about must be
        // left the same way as anything else, by BACK or by Escape.
        View::ThemeEditor | View::SoundLevels | View::Font | View::LookFeelReset => {
            Some(View::LookFeel)
        }
        _ => Some(View::Menu),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Close,
    Back,
    OpenLookFeel,
    /// The anchor of one of LOOK AND FEEL's three lists: a press
    /// unfolds it, a second press folds it back.
    ListBtn(ListId),
    /// A NAME of one of those lists — an index into the names, which is
    /// also the index of the row the accordion drew it in, because the
    /// list object is given the names and nothing else.
    Pick(ListId, usize),
    /// The door to the theme editor, standing at the head of the
    /// unfolded THEMES list (decision §3). Not a member of that list:
    /// it is drawn as a button, above the rows, and picks no theme.
    ThemesEditor,
    OpenFont,
    OpenGrid,
    /// LOOK AND FEEL's second button: volume, typing, ambient. Named
    /// for the LEVELS and not for "sound", which on this page already
    /// means the SOUNDS list's set of clips.
    OpenSoundLevels,
    OpenBoards,
    OpenColor,
    OpenBlur,
    /// LOOK AND FEEL's pinned footer: opens the confirmation, and does
    /// nothing else. A press that could clear six settings is a press
    /// the pointer must not be able to make by accident.
    LookFeelReset,
    /// The one control on that confirmation: everything the LOOK AND
    /// FEEL page and its doors write goes at once (decision §2a).
    LookFeelResetConfirm,
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
    /// One of LOOK AND FEEL's three lists. The same mechanism the two
    /// font lists have always used, which is the whole reason a page of
    /// choices could become a page of drop-downs at all.
    List(ListId),
}

/// The button an open list hangs from. Asked in two places — the
/// drawing, which needs the rect the anchor was last drawn at, and the
/// anchor itself, which wears the ladder's Selected rung while its list
/// is unfolded — so it is answered once.
fn anchor_act(d: Dropdown) -> Act {
    match d {
        Dropdown::Family(s) => Act::FamilyBtn(s),
        Dropdown::Weight(s) => Act::WeightBtn(s),
        Dropdown::List(l) => Act::ListBtn(l),
    }
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
        OpenLookFeel => FocusId::of("settings.menu.lookfeel"),
        OpenFont => FocusId::of("settings.lookfeel.fonts"),
        // A LOOK AND FEEL door now, not a menu entry — the path says
        // where the control stands, which is the whole point of paths.
        OpenSoundLevels => FocusId::of("settings.lookfeel.sound_levels"),
        OpenGrid => FocusId::of("settings.menu.grid"),
        OpenBoards => FocusId::of("settings.menu.boards"),
        OpenColor => FocusId::of("settings.menu.color"),
        OpenBlur => FocusId::of("settings.menu.blur"),
        ListBtn(l) => FocusId::of(match l {
            ListId::Looks => "settings.lookfeel.themes",
            ListId::Layauts => "settings.lookfeel.layauts",
            ListId::Sounds => "settings.lookfeel.sounds",
        }),
        // A name's row is its index, with nothing added: the list
        // object is handed the names alone, so `base.item(i)` is what
        // it registers and what a click must agree with.
        Pick(l, i) => dropdown_base(Dropdown::List(l)).item(i),
        // A path of its own, and not `themes.list.item(0)`, because the
        // door is no longer a row of that list — an id derived from the
        // list would now collide with its first theme.
        ThemesEditor => FocusId::of("settings.lookfeel.themes.editor"),
        LookFeelReset => FocusId::of("settings.lookfeel.reset"),
        LookFeelResetConfirm => FocusId::of("settings.lookfeel.reset.confirm"),
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
        Dropdown::List(ListId::Looks) => "settings.lookfeel.themes.list",
        Dropdown::List(ListId::Layauts) => "settings.lookfeel.layauts.list",
        Dropdown::List(ListId::Sounds) => "settings.lookfeel.sounds.list",
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

/// The percentage that means "no scaling": the value
/// [`config::term_font_prefs`] reports when nothing is written
/// (`unwrap_or(1.0)`), in the percent its writers take.
///
/// Not an appearance number — it is the identity of the scale, and the
/// same one [`Settings::new`] starts at. LOOK AND FEEL RESET writes it
/// because `config.rs` has no way to say "unset" for a numeric key:
/// every other key the reset touches takes an empty value, which its
/// reader treats as absent, and this one cannot. That is the one place
/// the reset pins a value instead of standing out of the cascade's
/// way, and it is in the fleet's report as such.
const FONT_SIZE_UNSET: u32 = 100;

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

/// Which rung of the button ladder a button state stands on — the
/// object's own rule ([`nacelle::object::button::ButtonState`] keeps it
/// private), needed here only by the two glyphs this file draws on top
/// of a plate the object drew.
fn rung(st: nacelle::object::button::ButtonState) -> State {
    if st.flash {
        State::Press
    } else if st.hover && st.selected {
        State::SelectedHover
    } else if st.hover {
        State::Hover
    } else if st.selected {
        State::Selected
    } else {
        State::Idle
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
    /// The font slot the role's `face` names. Read from the role, not
    /// chosen here: every text call in this file wrote `FONT_UI`, which
    /// is the family decided at the call site while `type.<role>.face`
    /// sat in the master with no reader on this side of the boundary.
    face: u8,
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
/// The px carries `panel_scale` and never falls under `type.min_px`.
/// UIFontSize= is not applied here and must not be: this window writes it
/// into `metric.ui_scale`, the frame hands it to the viewport, and every
/// role's size is a multiple of the u that scale multiplies. The shrink
/// argument used to be `ctx.ui_font_scale` — the shortest route to the
/// setting back when the viewport was told a literal 1.0, and a straight
/// doubling now that it is told the truth: 125 % would draw at 156 %.
fn bound(ctx: &Ctx, cell: &'static OnceLock<TokenId>, binding: &'static str) -> Type {
    let role = nacelle::ui::bound_role(cell, binding);
    let px = role.px(ctx, 1.0);
    Type { px, track: role.tracking_px(px), leading: role.leading(), face: role.font() }
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
    /// flowing — LOOK AND FEEL RESET. The rows above it used to be able
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

/// Which of LOOK AND FEEL's three lists is meant. The names it offers,
/// the word its anchor wears, what it carries in front of those names
/// and the words for "there are none" are all one decision, so they are
/// one value.
#[derive(Clone, Copy, PartialEq)]
enum ListId {
    Looks,
    Layauts,
    Sounds,
}

/// The label of the button that opens the theme editor. It stands
/// INSIDE the unfolded THEMES list, at the top (decision §3) — a door
/// among the choices, and drawn as one ([`Settings::door`]).
const EDITOR_ROW: &str = "THEMES EDITOR";

impl ListId {
    /// The word its anchor wears — the whole of it (decision §2b).
    fn label(self) -> &'static str {
        match self {
            ListId::Looks => "THEMES",
            ListId::Layauts => "LAYAUTS",
            ListId::Sounds => "SOUNDS",
        }
    }

    fn empty_note(self) -> &'static str {
        match self {
            ListId::Looks => "NO LOOKS FOUND",
            ListId::Layauts => "NO LAYAUTS FOUND",
            ListId::Sounds => "NO SOUND THEMES FOUND",
        }
    }

    /// Whether the unfolded list stands under a door — a control that
    /// belongs to the list's subject but is not a member of its set.
    /// Only THEMES has one, the editor (decision §3), and it is asked
    /// for rather than assumed because the door is drawn by this file
    /// while the members are drawn by the list object.
    fn carries_door(self) -> bool {
        matches!(self, ListId::Looks)
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
    /// One of LOOK AND FEEL's three lists: an anchor wearing the LIST'S
    /// OWN NAME (decision §2b), and the list itself unfolding from its
    /// bottom edge when it is the open one
    /// ([`Settings::draw_open_dropdown`]). Which member is in force is
    /// said by the open list, on the row that wears the ladder's
    /// `selected` rung.
    Drop { list: ListId },
    Button { label: Text, kind: BtnKind, act: Act },
    /// A module header inside a page: the FONT view's TERMINAL and
    /// INTERFACE separators.
    Section { title: &'static str },
    /// A left-aligned aside in the flow.
    Note { text: Text },
    /// The centred line a surface with nothing in it yet says about
    /// itself (`emptystate.role`).
    Empty { text: Text },
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

/// The main menu. THEMES, LOOK, LAYAUTS, SOUNDS, FONT and SOUND are no
/// longer here: all six were one question — what the desktop looks and
/// sounds like — and they are now one door (decision §2). SOUND went
/// the way THEMES and FONT went, and for the same reason: how loud the
/// clips are is part of the same sitting as which clips they are, and
/// the two used to stand on opposite sides of the window.
static MENU_ROWS: [Row; 5] = [
    row(Ctrl::Button {
        label: Text::Fixed("LOOK AND FEEL"),
        kind: BtnKind::Listed,
        act: Act::OpenLookFeel,
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

/// LOOK AND FEEL (decision §2): three choices and the doors beside
/// them. Three drop-downs rather than three columns, so the page at
/// rest fits whole and at most one list is ever unfolded — three open
/// lists one under the other would be a page you scroll through to
/// reach the sounds.
///
/// The two flowed buttons are `BtnKind::Wide`, the same width as the
/// three anchors above them, because they are the same KIND of thing:
/// another way into the same subject. A centred `Listed` button among
/// full-width rows reads as a different class of control — which is
/// what FONTS looked like, sitting at 60 % of the width under three
/// rows that ran edge to edge.
///
/// SOUND LEVELS stands directly under the SOUNDS list on purpose: the
/// set and the loudness of the set are one sitting, and a reader who
/// has just chosen a set is the reader who wants the dial.
///
/// The footer is the page's own undo, and it is pinned rather than
/// flowed so that the five rows above it are the page the decision
/// describes. It opens a confirmation and nothing else: what stands
/// behind it (decision §2a) is every setting this page and its doors
/// write, and one press may not be able to spend all of them.
static LOOKFEEL_ROWS: [Row; 6] = [
    row(Ctrl::Drop { list: ListId::Looks }),
    row(Ctrl::Drop { list: ListId::Layauts }),
    row(Ctrl::Drop { list: ListId::Sounds }),
    row(Ctrl::Button {
        label: Text::Fixed("SOUND LEVELS"),
        kind: BtnKind::Wide,
        act: Act::OpenSoundLevels,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("FONTS"),
        kind: BtnKind::Wide,
        act: Act::OpenFont,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("LOOK AND FEEL RESET"),
        kind: BtnKind::Footer,
        act: Act::LookFeelReset,
    }),
];

/// LOOK AND FEEL RESET's confirmation (decision §2a).
///
/// The decision asks for a hold or a confirmation, and this is the
/// confirmation. It is a PAGE and not a second press on the footer for
/// three reasons, each of which a same-place second press fails: the
/// control that does the work stands where the pointer is not, so a
/// double click cannot reach it; the keyboard reaches it exactly as
/// the mouse does, which a hold cannot (this window has an Enter and
/// no key release, so a held key is a key pressed once); and a page
/// has room to NAME what is about to go, which is the difference
/// between a confirmation and a speed bump.
///
/// The first three lines read the configuration in force, so what the
/// user is told they are losing is what they are actually losing.
static LOOKFEEL_RESET_ROWS: [Row; 8] = [
    row_after(Ctrl::Section { title: "WHAT THIS CLEARS" }, Gap::None),
    row(Ctrl::Note { text: Text::Of(clears_theme) }),
    row(Ctrl::Note { text: Text::Of(clears_layaut) }),
    row(Ctrl::Note { text: Text::Of(clears_sounds) }),
    row(Ctrl::Note {
        text: Text::Fixed("FONTS: SIZE, FAMILY AND WEIGHT, TERMINAL AND INTERFACE"),
    }),
    row_after(
        Ctrl::Note { text: Text::Fixed("THE PINNED ARRANGEMENT OF THIS SCREEN") },
        Gap::Section,
    ),
    row_after(
        Ctrl::Note {
            text: Text::Fixed("WHAT IS LEFT IS WHAT THE SYSTEM SETS. THERE IS NO WAY BACK."),
        },
        Gap::Double,
    ),
    row(Ctrl::Button {
        label: Text::Fixed("RESET LOOK AND FEEL"),
        kind: BtnKind::Listed,
        act: Act::LookFeelResetConfirm,
    }),
];

/// The theme editor's doorway, and nothing behind it yet.
///
/// It is a page like every other one so that the way in, the way back
/// and the Escape ladder are real before the editor is: the door that
/// opens it takes the window's content area over, exactly as it will
/// when there is an editor to put there. Building that editor is
/// another stage's work (decision §3, requirement 3).
static EDITOR_ROWS: [Row; 2] = [
    row_after(
        Ctrl::Empty { text: Text::Fixed("THE THEME EDITOR IS NOT BUILT YET") },
        Gap::Section,
    ),
    row(Ctrl::Note {
        text: Text::Fixed("IT WILL EDIT THE TOKENS OF THE THEME IN FORCE, HERE."),
    }),
];

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

/// SOUND LEVELS: master volume plus the two switches that matter in
/// daily use — typing, which fires constantly, and the ambient bed.
///
/// Levels only. WHICH set of clips is playing is LOOK AND FEEL's
/// `SOUNDS` list, and this page merely says which one that turned out
/// to be ([`sound_set_note`]) so a silent desktop has somewhere to
/// explain itself.
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
static PAGES: [Page; 10] = [
    Page {
        view: View::Menu,
        title: "SETTINGS",
        chrome: Chrome::Close,
        lead: Gap::Row,
        cols: Cols::None,
        rows: &MENU_ROWS,
    },
    Page {
        view: View::LookFeel,
        title: "SETTINGS \u{2014} LOOK AND FEEL",
        chrome: Chrome::Back,
        lead: Gap::Row,
        cols: Cols::None,
        rows: &LOOKFEEL_ROWS,
    },
    Page {
        view: View::LookFeelReset,
        title: "SETTINGS \u{2014} LOOK AND FEEL RESET",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::None,
        rows: &LOOKFEEL_RESET_ROWS,
    },
    Page {
        view: View::ThemeEditor,
        title: "SETTINGS \u{2014} THEMES EDITOR",
        chrome: Chrome::Back,
        lead: Gap::Section,
        cols: Cols::None,
        rows: &EDITOR_ROWS,
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
        view: View::SoundLevels,
        title: "SETTINGS \u{2014} SOUND LEVELS",
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

// The three lines of the reset's confirmation that name a VALUE and
// not just a key. They read the window's own copy of the
// configuration — the one [`Settings::refresh_current`] takes on the
// way into the page — so a line the page draws every frame costs no
// file read.

fn clears_theme(s: &Settings) -> String {
    format!("THEME AND VARIANT: {}", s.drop_value(ListId::Looks))
}

/// Both halves of the layout setting: the desktop's own and the
/// assignments made per connector, which are what a second monitor
/// would otherwise keep after a reset.
fn clears_layaut(s: &Settings) -> String {
    format!("LAYAUT: {}, AND EVERY SCREEN'S OWN", s.drop_value(ListId::Layauts))
}

fn clears_sounds(s: &Settings) -> String {
    format!("SOUNDS: {}", s.drop_value(ListId::Sounds))
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
    /// One line of the empty-state role: what a page with nothing in it
    /// yet reserves for saying so.
    empty_h: f32,
    hint_inset: f32,
    corner_w: f32,
    list_w: f32,
}

impl Metrics {
    /// Resolved against the drawing context and not the theme alone,
    /// because two of these lengths are type: a role's px follows
    /// `panel_scale`, so a page's height does too. (The interface scale
    /// reaches both through the bake, and needs no context.)
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
            empty_h: role_empty(ctx).line(),
            hint_inset: th.px(tok(&HINT_INSET, "settings.hint_inset")),
            corner_w: (content.w * th.px(tok(&BACK_W_FRAC, "settings.back_w_frac")))
                .max(th.px(tok(&BACK_W_MIN, "settings.back_w_min")))
                .max(th.px(tok(&BACK_W_MIN_PX, "settings.back_w_min_min_px"))),
            list_w: content.w * th.px(tok(&LIST_W_FRAC, "settings.list_w_frac")),
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

/// What one row needs to place itself: the page's content box, its own
/// band in the flow, and the two column widths the page's rule
/// produced. The box the flow scrolls in is the walker's business —
/// every row is drawn under its clip and none of them lays out a grid
/// of its own any more.
#[derive(Clone, Copy)]
struct RowCtx {
    content: Rect,
    band: Rect,
    label_w: f32,
    value_w: f32,
    m: Metrics,
}

/// Where a page's body starts: under the chrome's own row.
fn body_top(page: &Page, m: Metrics, content: Rect) -> f32 {
    content.y + m.btn_h + m.space(page.lead)
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
    /// SOUND LEVELS view: master volume 0-100 and the two mute
    /// switches. Not the sound SET, which is `current_sounds`.
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
    /// The last step of LOOK AND FEEL RESET — main clears the pinned
    /// [WxH@D] section of the selected layout for the screen it is on,
    /// then re-applies the configuration. The window itself cannot:
    /// only the application knows which screen this is.
    ///
    /// Still spelled for the control's old name, which cleared this
    /// section and nothing else. Renaming it is a `main.rs` change and
    /// `main.rs` belongs to another fleet today; it is in the report.
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
            Act::Pick(..) => {}
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
                // The same answer Escape peels a layer by, so the two
                // ways out of a page cannot lead to different places.
                self.go(parent_view(self.view).unwrap_or(View::Menu))
            }
            Act::OpenLookFeel => {
                // All three lists are scanned on the way in, once: they
                // are directories the user installs into behind the
                // program's back, and the page offers all three at the
                // same time now. The engine's themes, not the look/
                // directories: a look bundled a stylesheet, and
                // stylesheets are gone.
                self.themes = config::list_engine_themes();
                self.layauts = config::list_layauts();
                self.sounds = config::list_sound_themes();
                self.refresh_current();
                self.dropdown = None;
                self.go(View::LookFeel);
            }
            Act::ListBtn(list) => {
                let d = Dropdown::List(list);
                self.dropdown = if self.dropdown == Some(d) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    Some(d)
                };
            }
            Act::Pick(list, i) => {
                self.dropdown = None;
                // Choosing a theme writes Theme= and nothing else.
                // Colour and layout are two independent axes now, so
                // this must not touch Layaut= — picking crimson may not
                // rearrange the boards.
                if let Some(name) = self.names(list).get(i).cloned() {
                    match list {
                        ListId::Looks => config::set_engine_theme(&name),
                        ListId::Layauts => config::set_layaut_option(&name),
                        ListId::Sounds => config::set_sounds_option(&name),
                    }
                    self.refresh_current();
                    emit(Sfx::Theme);
                    return true;
                }
            }
            Act::ThemesEditor => {
                // Decision §3, requirement 2: the control at the head of
                // the THEMES list is a door and not a choice. It writes
                // no Theme=, moves no selection, and takes the window's
                // content area over instead — which is why it answers
                // false: nothing about the configuration changed.
                self.dropdown = None;
                self.go(View::ThemeEditor);
            }
            Act::OpenSoundLevels => {
                let (vol, typing, ambient) = config::sound_prefs();
                self.sound_volume = vol;
                self.sound_typing = typing;
                self.sound_ambient = ambient;
                // Like every other door off this page: an anchor the
                // next page does not draw has nothing to hang a list
                // from, and a dropdown left standing eats the first
                // Escape.
                self.dropdown = None;
                self.go(View::SoundLevels);
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
            Act::LookFeelReset => {
                // The footer opens the confirmation and writes nothing.
                // Answering false is what it means: no configuration
                // changed, so the caller re-applies nothing.
                //
                // The list is folded on the way out, like every other
                // door out of this page: an anchor the next page does
                // not draw has nothing to hang a list from, and a
                // dropdown left standing would eat the first Escape.
                self.dropdown = None;
                self.go(View::LookFeelReset);
            }
            Act::LookFeelResetConfirm => {
                self.reset_look_and_feel();
                // Back to the page the footer stands on, which now
                // shows what the system end of the cascade gives.
                //
                // And it answers FALSE although everything changed.
                // The caller re-applies the configuration itself, but
                // only AFTER the request left in `reset_screen` — and
                // that request clears the pinned section of the layout
                // IN FORCE. Answering true would re-apply first, the
                // screen would move to the layout the reset just fell
                // back to, and the section would be cleared out of a
                // file that never pinned anything.
                self.go(View::LookFeel);
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
                // ONE layer per press (decision §3): the open list
                // first, then the view it was opened on, and so on out
                // to the main menu. The window's own last layer —
                // closing it — is the application's, which is what
                // Ignored asks for; from the editor, three levels in,
                // Escape used to land straight on the desktop.
                if self.dropdown.take().is_some() {
                    KeyOut::Consumed
                } else if let Some(back) = parent_view(self.view) {
                    self.go(back);
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

    /// Decision §2a: everything to do with look and feel, cleared in
    /// one go — everything the LOOK AND FEEL page controls and
    /// everything its doors lead to. Afterwards the program looks the
    /// way it looks with no user configuration at all: it takes what it
    /// finds at the system end of the cascade.
    ///
    /// Every key goes out EMPTY rather than being deleted, because
    /// `config.rs` writes values and has no way to remove a line
    /// ([`config`]'s `set_conf_kv` replaces or appends). An empty value
    /// reads as absent to `Theme=`, `Layaut=`, `Sounds=`, the two font
    /// families, the two weights and the per-connector assignments —
    /// their readers all filter it — so those eight keys really do fall
    /// back. `Variant=` and the two sizes are the exceptions and are in
    /// the fleet's report: an empty `Variant=` is documented as an
    /// explicit off, and a size cannot be written empty at all
    /// ([`FONT_SIZE_UNSET`]). Both need a "clear this key" writer in
    /// `config.rs`, which belongs to another fleet today.
    ///
    /// The pinned `[WxH@D]` section is the application's to clear —
    /// only it knows which screen this window is on — so it is asked
    /// for, exactly as it was when this control cleared nothing else.
    fn reset_look_and_feel(&mut self) {
        // The theme engine's two keys. Colour and contrast are one
        // choice made in two lines, and a reset that left the variant
        // standing would hand the default theme somebody else's
        // contrast.
        config::set_engine_theme("");
        config::set_engine_variant(None);
        // The layout, and then every screen that was given one of its
        // own: clearing `Layaut=` alone would leave a second monitor
        // pinned to whatever `Layaut[DP-2]=` says, which is precisely
        // the setting the user cannot see from this page.
        config::set_layaut_option("");
        for connector in config::screen_layauts().into_keys() {
            config::set_layaut_for_connector(&connector, "");
        }
        config::set_sounds_option("");
        // Both font sections, all three properties each — the page
        // behind the FONTS door, whole.
        config::set_term_font_size(FONT_SIZE_UNSET);
        config::set_term_font_family("");
        config::set_term_font_weight("");
        config::set_ui_font_size(FONT_SIZE_UNSET);
        config::set_ui_font_family("");
        config::set_ui_font_weight("");
        // The window's own copy of what it just cleared, so the page it
        // returns to and the font page behind it do not go on showing
        // settings that no longer exist.
        self.cur_size = [FONT_SIZE_UNSET, FONT_SIZE_UNSET];
        self.cur_family = [None, None];
        self.cur_weight = [None, None];
        self.refresh_current();
        self.reset_screen = true;
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
            Chrome::Back => (Act::Back, "BACK"),
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
        self.draw_open_dropdown(ctx, m);
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
                bottom -= m.gap + self.row_h(&row.ctrl, m, content);
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
            h += self.row_h(&row.ctrl, m, content) + m.space(row.after);
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
            let h = self.row_h(&row.ctrl, m, content);
            let band = Rect::new(content.x, y, content.w, h);
            // A row wholly off the viewport is not drawn, and therefore
            // registers nothing: what the eye cannot see is not a
            // target and does not belong in the Tab order either.
            if band.bottom() > view.y && band.y < view.bottom() {
                let rc = RowCtx { content, band, label_w, value_w, m };
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
            let h = self.row_h(&row.ctrl, m, content);
            let band = Rect::new(content.x, content.bottom() - h, content.w, h);
            let rc = RowCtx { content, band, label_w, value_w, m };
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
                    ctx.fonts.measure(f.face, f.px, label, f.track)
                        + th.px(tok(&LABEL_PAD, "rhythm.label_pad")),
                    ctx.fonts.measure(v.face, v.px, value, v.track)
                        + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter")),
                )
            }
        }
    }

    /// How tall a row is. Everything is a theme length except the one
    /// row whose height is its content's: the boards' reserve.
    ///
    /// A drop-down is one button tall however many names it carries:
    /// the list is drawn over the page and takes no room in the flow,
    /// which is what lets LOOK AND FEEL stay five rows.
    fn row_h(&self, ctrl: &Ctrl, m: Metrics, content: Rect) -> f32 {
        match ctrl {
            Ctrl::Toggle { .. } => m.check_h,
            Ctrl::Slider { .. } => m.slider_h,
            Ctrl::Chips { .. } => m.seg_h,
            Ctrl::Cycle { .. } => m.cyc_h,
            Ctrl::Button { .. } | Ctrl::Drop { .. } => m.btn_h,
            Ctrl::Section { .. } => m.block_h,
            Ctrl::Note { .. } => m.note_h,
            Ctrl::Hint { .. } => m.hint_h,
            Ctrl::Empty { .. } => m.empty_h,
            Ctrl::Custom { h, .. } => h(m, content),
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
                    v.face,
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
            Ctrl::Drop { list } => self.draw_drop(ctx, *list, rc),
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
                    n.face,
                    n.px,
                    rc.content.x,
                    ny,
                    &s,
                    col(th.color(tok(&MUTED_FG, "text.muted"))),
                    n.track,
                );
            }
            // What a surface with nothing in it yet says about itself,
            // set the way every other empty box in the program sets it
            // (`emptystate.role`) and inked like every other aside.
            Ctrl::Empty { text } => {
                let v = role_empty(ctx);
                let ty = center_y(ctx, rc.band, v);
                let s = self.text_of(*text);
                ctx.dl.text_center(
                    ctx.fonts,
                    v.face,
                    v.px,
                    rc.content.cx(),
                    ty,
                    &s,
                    col(th.color(tok(&MUTED_FG, "text.muted"))),
                    v.track,
                );
            }
            // One line that explains the other way in (settings.hint.role).
            Ctrl::Hint { text } => {
                let n = role_hint(ctx);
                let s = self.text_of(*text);
                ctx.dl.text_center(
                    ctx.fonts,
                    n.face,
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
            f.face,
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
            f.face,
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
                f.face,
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
            f.face,
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

    /// The name the configuration carries for one list.
    fn current_of(&self, list: ListId) -> Option<&String> {
        match list {
            ListId::Looks => self.current_look.as_ref(),
            ListId::Layauts => self.current_layaut.as_ref(),
            ListId::Sounds => self.current_sounds.as_ref(),
        }
    }

    /// Which ROW of the open list is the member already in force — the
    /// theme now applied, the layout now loaded — for the object to
    /// draw on its ladder's `selected` rung
    /// ([`nacelle::object::dropdown::AccordionStyle::current`]).
    ///
    /// The three of these answer one question and are written as three
    /// functions because the three lists number their rows differently,
    /// and each one's answer must be the SAME index its `Act` carries:
    /// a mark that disagreed with the pick would show one row and apply
    /// another, which is worse than no mark at all.
    ///
    /// `None` is a real answer here: the configuration can name a theme
    /// that is not installed on this machine, and then no member of the
    /// set is standing — which is not "the first one".
    fn current_row(&self, list: ListId) -> Option<usize> {
        let cur = self.current_of(list)?;
        self.names(list).iter().position(|n| n == cur)
    }

    /// [`Settings::current_row`] for a font family. Row zero is the
    /// `DEFAULT` entry this file puts in front of the families, so an
    /// unset family IS row zero — that is what the desktop resolves it
    /// to — and every other family stands one past its place in the
    /// scan, the same shift [`Act::FamilyPick`] undoes.
    fn family_row(&self, sect: Sect) -> Option<usize> {
        let si = Self::sect_idx(sect);
        match self.cur_family[si].as_deref() {
            None => Some(0),
            Some(f) => self.families[si].iter().position(|x| x == f).map(|i| i + 1),
        }
    }

    /// [`Settings::current_row`] for a font weight. [`WEIGHTS`] is a
    /// fixed table with nothing in front of it, so the row is the place
    /// in it — matched without case, because the table is spelled
    /// `SemiBold` and the configuration keeps whatever was written.
    fn weight_row(&self, sect: Sect) -> Option<usize> {
        let si = Self::sect_idx(sect);
        let w = self.cur_weight[si].as_deref()?;
        WEIGHTS.iter().position(|k| k.eq_ignore_ascii_case(w))
    }

    /// The name one list is set to, for the page that has to say what
    /// it is about to destroy — an empty list says why there is nothing
    /// to choose from rather than standing blank
    /// (`ListId::empty_note`).
    ///
    /// It was the anchor's second half until decision §2b took the
    /// choice off the anchor. The reading survives the anchor because
    /// the confirmation needs it: "THEME AND VARIANT: MIDNIGHT" is a
    /// warning, "THEME AND VARIANT" alone is a category.
    fn drop_value(&self, list: ListId) -> String {
        match self.current_of(list) {
            Some(name) if !self.names(list).is_empty() => name.to_uppercase(),
            _ => list.empty_note().to_string(),
        }
    }

    /// One of LOOK AND FEEL's three lists at rest: the toolkit's button
    /// wearing the LIST'S OWN NAME, with the toolkit's disclosure
    /// triangle at its tail. The list itself is not drawn here — it
    /// hangs over the whole window and is drawn last
    /// ([`Settings::draw_open_dropdown`]).
    ///
    /// The name and not the choice (decision §2b). The owner ruled it
    /// against the convention of the font page, whose anchors read
    /// "FAMILY: TERMINUS" — so this is a deliberate difference and not
    /// an oversight, and the value it used to wear now stands on the
    /// LOOK AND FEEL RESET page, where naming what is about to be
    /// cleared is the whole point ([`Settings::drop_value`]).
    fn draw_drop(&mut self, ctx: &mut Ctx, list: ListId, rc: RowCtx) {
        let act = Act::ListBtn(list);
        let r = Self::button_rect(BtnKind::Wide, rc);
        self.button(ctx, r, list.label(), act);
        self.caret(ctx, r, act);
    }

    /// The triangle at the tail of an anchor: the toolkit's own
    /// disclosure glyph ([`nacelle::view::paint::disclosure`]) in its
    /// DROP grammar — closed it points down, at the direction the list
    /// will unfold, and open it points back up at the edge the list
    /// folds into. The tree's grammar (closed points along the row) is
    /// the other sentence the same primitive speaks, and it belongs to
    /// file trees; a `▷` on a drop-down reads as "go into this row".
    /// The state turns the GLYPH and not its colour, which is the
    /// primitive's rule and not this window's.
    ///
    /// Sized and inked like the BACK arrow at the other end of a button
    /// — `button.icon_size` glyph, `button.pad_x` from the edge, the
    /// ladder's own text colour — because a glyph on a button is a
    /// glyph on a button.
    fn caret(&mut self, ctx: &mut Ctx, r: Rect, act: Act) {
        static ICON_SIZE: OnceLock<TokenId> = OnceLock::new();
        static ICON_MIN: OnceLock<TokenId> = OnceLock::new();
        static PAD_X: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        let s = th
            .px(tok(&ICON_SIZE, "button.icon_size"))
            .max(th.px(tok(&ICON_MIN, "button.icon_size_min_px")));
        let pad = th.px(tok(&PAD_X, "button.pad_x"));
        let open = self.dropdown.map_or(false, |d| anchor_act(d) == act);
        let ink = col(ladder(th, &BTN_CLASS, "button", self.button_rung(ctx, r, act)).text);
        // `line_px` is the box the glyph is centred in vertically; the
        // glyph's own size is that box, so the triangle sits on the
        // row's middle without a second centring rule.
        nacelle::view::paint::disclosure(
            &mut nacelle::view::surface::CtxSurface::new(ctx),
            r.right() - pad - s,
            r.y + (r.h - s) / 2.0,
            s,
            s,
            nacelle::view::paint::Disclosure::Drop,
            open,
            ink,
        );
    }

    /// The open dropdown, drawn over everything else. Its anchor is
    /// wherever the button that owns it was drawn this frame, which the
    /// hit map already knows — an anchor kept beside it would be a
    /// second copy of a geometry. That is the VISIBLE part of the
    /// button: a list hangs from the edge the eye sees, and an anchor
    /// scrolled off the page has nothing to hang from at all.
    fn draw_open_dropdown(&mut self, ctx: &mut Ctx, m: Metrics) {
        static MENU_ROW_H: OnceLock<TokenId> = OnceLock::new();
        let Some(d) = self.dropdown else { return };
        let Some(anchor) = self.rect_of_act(anchor_act(d)) else { return };
        let item_h = theme::resolved().px(tok(&MENU_ROW_H, "menu.row_h"));
        match d {
            Dropdown::Family(sect) => {
                let si = Self::sect_idx(sect);
                // First entry: DEFAULT (auto-detected font).
                let mut names = vec!["DEFAULT".to_string()];
                names.extend(self.families[si].iter().map(|f| f.to_uppercase()));
                let current = self.family_row(sect);
                self.draw_dropdown(
                    ctx,
                    anchor,
                    item_h,
                    &names,
                    dropdown_base(d),
                    current,
                    |i| Act::FamilyPick(sect, i),
                );
            }
            Dropdown::Weight(sect) => {
                let names: Vec<String> =
                    WEIGHTS.iter().map(|w| w.to_uppercase()).collect();
                let current = self.weight_row(sect);
                self.draw_dropdown(
                    ctx,
                    anchor,
                    item_h,
                    &names,
                    dropdown_base(d),
                    current,
                    |i| Act::WeightPick(sect, i),
                );
            }
            Dropdown::List(list) => {
                // Decision §3, requirement 1: what is NOT a choice may
                // not read as one. So the door is not handed to the
                // list object at all — it is a BUTTON, drawn above the
                // rows, and the object is given the names alone. That
                // is the whole difference and it is a real one: a
                // different object, `button.h` tall against
                // `menu.row_h`, on the `button` class's ladder against
                // `menu.item`'s, with its label in the `button` type
                // role. A hairline between two identical rows was not,
                // and could not be — every row of an accordion already
                // has a seam above it.
                let below = if list.carries_door() {
                    self.door(ctx, anchor, m)
                } else {
                    anchor
                };
                let names: Vec<String> =
                    self.names(list).iter().map(|n| n.to_uppercase()).collect();
                let current = self.current_row(list);
                self.draw_dropdown(
                    ctx,
                    below,
                    item_h,
                    &names,
                    dropdown_base(d),
                    current,
                    |i| Act::Pick(list, i),
                );
            }
        }
    }

    /// The control at the head of an unfolded list that is not a member
    /// of it — today only the THEMES editor (decision §3). Drawn as the
    /// toolkit's button, flush under the anchor and exactly as wide,
    /// and answering with the rect the NAMES then hang from, so the
    /// caller has no second geometry to keep in step.
    ///
    /// It does not unfold with the list: a plate whose height is still
    /// moving cannot hold a label, and this one is not a member of the
    /// set being revealed — it is the list's own furniture, there from
    /// the moment the list is.
    fn door(&mut self, ctx: &mut Ctx, anchor: Rect, m: Metrics) -> Rect {
        let act = Act::ThemesEditor;
        let r = Rect::new(anchor.x, anchor.bottom(), anchor.w, m.btn_h);
        // Not [`Settings::button_state`]: that one refuses hover while
        // a dropdown is open, which is right for the page underneath
        // and exactly wrong here — the door stands INSIDE the open
        // list and is one of the two things the pointer can reach.
        let st = nacelle::object::button::ButtonState {
            hover: r.contains(ctx.mouse.0, ctx.mouse.1),
            flash: self.flashing(act),
            selected: false,
        };
        nacelle::object::button::draw_focusable(ctx, r, EDITOR_ROW, st, focus_id(act));
        // Unclipped, like the rows below it: the list hangs past the
        // window's edge and stays pressable there.
        self.hits.push((r, act));
        r
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
                f.face,
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
    ///
    /// `current` is the place IN `names` of the member already in force
    /// — the theme now applied, the family now loaded — which the list
    /// object draws on the `menu.item` ladder's `selected` rung. It has
    /// to be the very index `make_act` turns into a pick, or the marked
    /// row and the row a click applies would be two different rows.
    /// `None` says the set has no standing member, which is not the
    /// same as "the first one".
    ///
    /// Returns the rows the object drew, in order, each with whether it
    /// is done unfolding.
    fn draw_dropdown<F: Fn(usize) -> Act>(
        &mut self,
        ctx: &mut Ctx,
        anchor: Rect,
        item_h: f32,
        names: &[String],
        base: FocusId,
        current: Option<usize>,
        make_act: F,
    ) -> Vec<(Rect, bool)> {
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
        // Nothing about the standing row's dress is stated here: the
        // wash, the ring's colour, its width and the label's brightness
        // all come off the `menu.item` class's ladder inside the object.
        let rows = nacelle::object::dropdown::accordion(
            ctx,
            anchor,
            item_h,
            names,
            p,
            &nacelle::object::dropdown::AccordionStyle {
                focus: Some(base),
                current,
                ..Default::default()
            },
        );
        for (i, (r, _full)) in rows.iter().copied().enumerate() {
            // Deliberately unclipped: the list hangs past the window's
            // edge and stays pressable there, which is the one place in
            // this file where a target may leave the body's box.
            self.hits.push((r, make_act(i)));
        }
        rows
    }

    /// Button in the terminal-tab style (slant, hover, flash on click),
    /// joining the focus chain where it is drawn.
    fn button(&mut self, ctx: &mut Ctx, r: Rect, label: &str, act: Act) {
        self.button_drawn(ctx, r, label, act, None)
    }

    /// How a button stands this frame. Asked in two places — the plate
    /// itself, and the glyphs this file draws ON a plate (BACK's arrow,
    /// a list anchor's caret) — so that a glyph can never end up a
    /// different colour from the word beside it.
    fn button_state(
        &self,
        ctx: &Ctx,
        r: Rect,
        act: Act,
    ) -> nacelle::object::button::ButtonState {
        // With an open dropdown only its items react to the mouse.
        let hover = self.dropdown.is_none() && r.contains(ctx.mouse.0, ctx.mouse.1);
        let flash = self.flashing(act);
        // An anchor whose list is unfolded is the one button on the
        // page that is switched ON, and the ladder already has the rung
        // for it: a list left open is a state the eye should see.
        let selected = self.dropdown.map_or(false, |d| anchor_act(d) == act);
        nacelle::object::button::ButtonState { hover, flash, selected }
    }

    /// Whether an act's click flash is still decaying
    /// (`motion.press.duration_ms`). Split out of
    /// [`Settings::button_state`] because the door inside an open list
    /// wants the flash and NOT that method's hover rule.
    fn flashing(&self, act: Act) -> bool {
        static PRESS_MS: OnceLock<TokenId> = OnceLock::new();
        let press_s =
            theme::resolved().px(tok(&PRESS_MS, "motion.press.duration_ms")) / 1000.0;
        self.flash
            .map(|(a, t)| a == act && t.elapsed().as_secs_f32() < press_s)
            .unwrap_or(false)
    }

    /// [`Settings::button_state`]'s ladder rung, for a caller that
    /// needs the colour rather than the plate.
    fn button_rung(&self, ctx: &Ctx, r: Rect, act: Act) -> State {
        rung(self.button_state(ctx, r, act))
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
        static SKEW: OnceLock<TokenId> = OnceLock::new();
        static ICON_SIZE: OnceLock<TokenId> = OnceLock::new();
        static ICON_MIN: OnceLock<TokenId> = OnceLock::new();
        let st = self.button_state(ctx, r, act);
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
            let color = col(ladder(th, &BTN_CLASS, "button", rung(st)).text);
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
                f.face,
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
            Act::OpenLookFeel,
            Act::OpenFont,
            Act::OpenSoundLevels,
            Act::OpenGrid,
            Act::OpenBoards,
            Act::OpenColor,
            Act::OpenBlur,
            Act::ListBtn(ListId::Looks),
            Act::ListBtn(ListId::Layauts),
            Act::ListBtn(ListId::Sounds),
            // The door and the first theme stand on two rows of one
            // list: the pair a shared id would silently merge.
            Act::ThemesEditor,
            Act::Pick(ListId::Looks, 0),
            Act::Pick(ListId::Looks, 1),
            Act::Pick(ListId::Layauts, 0),
            Act::Pick(ListId::Layauts, 1),
            Act::Pick(ListId::Sounds, 0),
            // The footer and the control it leads to: one id between
            // them would let the press that opens the confirmation be
            // the press that answers it.
            Act::LookFeelReset,
            Act::LookFeelResetConfirm,
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
        // The three lists of LOOK AND FEEL derive theirs the same way,
        // and all three count from ZERO: the object is handed the names
        // and nothing else, so the first name is the first row on every
        // one of them — the THEMES door is no longer a row of the list
        // and takes no place out of its numbering.
        let themes = dropdown_base(Dropdown::List(ListId::Looks));
        assert_eq!(focus_id(Act::Pick(ListId::Looks, 0)), themes.item(0));
        assert_eq!(
            focus_id(Act::Pick(ListId::Sounds, 0)),
            dropdown_base(Dropdown::List(ListId::Sounds)).item(0)
        );
        // And the door stands outside that numbering entirely — an id
        // derived from the list would now BE its first theme's.
        assert_ne!(focus_id(Act::ThemesEditor), themes.item(0));
    }

    /// Decision §3, requirements 1 and 2 — the control at the head of
    /// the THEMES list is a door, not a theme.
    ///
    /// It stands at the top of a list whose entire role is "choose one
    /// of these", so what keeps it from writing `Theme=` has to be
    /// stated and kept: only THEMES has one, and pressing it changes no
    /// configuration. [`Settings::perform`] answers true exactly when
    /// the configuration changed, and this one may not.
    #[test]
    fn the_themes_editor_row_is_a_door_and_writes_no_theme() {
        assert!(ListId::Looks.carries_door(), "the THEMES list lost its door");
        for list in [ListId::Layauts, ListId::Sounds] {
            assert!(
                !list.carries_door(),
                "{} carries a door it has no editor for",
                list.label()
            );
        }

        let mut s = furnished();
        s.view = View::LookFeel;
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        let before = s.current_look.clone();
        assert!(
            !s.perform(Act::ThemesEditor, 0.0),
            "the door reported a configuration change"
        );
        assert_eq!(s.current_look, before, "the door moved the selection");
        assert!(s.view == View::ThemeEditor, "the door opened nothing");
        assert!(s.dropdown.is_none(), "the list stayed open behind the editor");
        // And the way back out is the page it was opened from.
        assert!(parent_view(View::ThemeEditor) == Some(View::LookFeel));
    }

    /// Decision §3 — Escape peels ONE layer per press.
    ///
    /// Open list, then editor, then page, and only then the window
    /// itself: the last step is the application's ([`KeyOut::Ignored`]
    /// is what `main.rs` turns into a close), so the window has to
    /// answer for the three before it. From the editor, three levels
    /// in, one press used to land on the desktop.
    #[test]
    fn escape_peels_one_layer_at_a_time() {
        fn escape(s: &mut Settings) -> KeyOut {
            let mut fc = FocusCtl::new();
            s.key(
                &KeyEv { key: FKey::Escape, mods: Mods::NONE, repeat: false, text: None },
                &mut fc,
            )
        }
        let mut s = furnished();
        s.view = View::LookFeel;
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        assert!(matches!(escape(&mut s), KeyOut::Consumed));
        assert!(s.dropdown.is_none(), "the list stayed open");
        assert!(s.view == View::LookFeel, "the list took the page with it");

        s.view = View::ThemeEditor;
        assert!(matches!(escape(&mut s), KeyOut::Consumed));
        assert!(s.view == View::LookFeel, "the editor did not fall back to its page");
        assert!(matches!(escape(&mut s), KeyOut::Consumed));
        assert!(s.view == View::Menu, "the page did not fall back to the menu");
        // The last layer is the window itself, and that one is not
        // this window's to close.
        assert!(matches!(escape(&mut s), KeyOut::Ignored));
        assert!(s.open, "the window closed itself");
    }

    /// Decision §3, requirement 1 — the door is set apart from the
    /// choices under it, and set apart by MORE than a line.
    ///
    /// A list whose whole role is "choose one of these" opens with a
    /// control that is not a choice, and it may not read as one. It
    /// used to be an accordion row with a hairline drawn along its
    /// bottom edge, which the owner rejected on sight: every row of an
    /// accordion already has a seam above it, so one more hair among
    /// hairs is invisible, and the door had the theme rows' height,
    /// their ink, their type and their centring.
    ///
    /// It is now a different OBJECT — the toolkit's button — so the
    /// four things the owner listed all move at once. Two of them are
    /// readable headless and are what this test reads: the door's box
    /// is a different height from a name's, and its label is set at a
    /// different size, because the two come off two different type
    /// roles (`button.role` against `menu.item.role`). Both differences
    /// are the THEME's to state; the test only insists that the theme
    /// has in fact stated them.
    #[test]
    fn the_door_is_more_than_a_hairline_apart_from_the_themes() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeel;
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        s.dropdown_since = None;
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let rect_of = |act: Act, what: &str| {
            s.hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .unwrap_or_else(|| panic!("the open THEMES list drew no {what}"))
        };
        let door = rect_of(Act::ThemesEditor, "door");
        let first = rect_of(Act::Pick(ListId::Looks, 0), "first theme");
        // Flush under the anchor, and the names start under the DOOR —
        // one column, in the order the decision names.
        assert!(door.y < first.y, "the door does not stand above the themes");
        assert!(
            (first.y - door.bottom()).abs() < 0.51,
            "the first theme does not hang from the door's edge"
        );
        assert!(
            (door.h - first.h).abs() > 1.0,
            "the door is a theme's height ({} px against {} px): it is still \
             a row of the list",
            door.h,
            first.h
        );
        // The two labels, at the size each was actually set in.
        let px_of = |label: &str| {
            dl.cmds()
                .iter()
                .find_map(|c| match c {
                    nacelle::draw::DrawCmd::Text { text, px, .. } if text == label => {
                        Some(*px)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{label} was never written"))
        };
        assert!(
            (px_of(EDITOR_ROW) - px_of(&s.themes[0].to_uppercase())).abs() > 0.5,
            "the door is set in the theme names' own type"
        );
    }

    /// The three anchors and the two doors are one column.
    ///
    /// FONTS used to be a `Listed` button — `settings.list_w_frac` of
    /// the content, centred — under three anchors that ran the full
    /// width, and it read as a different class of control although it
    /// is the same kind of thing: another way into the same subject.
    /// The footer is deliberately not in this set: it is pinned, it is
    /// destructive, and looking unlike the page is its job.
    #[test]
    fn the_pages_choices_and_doors_stand_in_one_column() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeel;
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let boxes: Vec<(Act, Rect)> = [
            Act::ListBtn(ListId::Looks),
            Act::ListBtn(ListId::Layauts),
            Act::ListBtn(ListId::Sounds),
            Act::OpenSoundLevels,
            Act::OpenFont,
        ]
        .into_iter()
        .map(|act| {
            let r = s
                .hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .expect("a row of LOOK AND FEEL was not drawn");
            (act, r)
        })
        .collect();
        let (_, first) = boxes[0];
        for (i, (_, r)) in boxes.iter().enumerate() {
            assert!(
                (r.x - first.x).abs() < 0.01 && (r.w - first.w).abs() < 0.01,
                "row {i} is {} px wide at x {}, the lists are {} px at x {}",
                r.w,
                r.x,
                first.w,
                first.x
            );
        }
    }

    /// SOUND LEVELS is the page's other door, and it is not the SOUNDS
    /// list wearing a second name.
    ///
    /// The two stand one under the other and a single word "SOUND"
    /// would run them together, so the guard is three-sided: the labels
    /// differ, the button stands directly above FONTS where the owner
    /// put it and no longer in the main menu, and pressing it opens the
    /// LEVELS — writing no `Sounds=` and moving no set, which a door
    /// mistakenly wired to the list would do.
    #[test]
    fn the_sound_button_opens_the_levels_and_never_a_set() {
        fn button_at(rows: &[Row], act: Act) -> Option<usize> {
            rows.iter()
                .position(|r| matches!(r.ctrl, Ctrl::Button { act: a, .. } if a == act))
        }
        let levels = button_at(&LOOKFEEL_ROWS, Act::OpenSoundLevels)
            .expect("LOOK AND FEEL has no SOUND LEVELS door");
        let fonts_at =
            button_at(&LOOKFEEL_ROWS, Act::OpenFont).expect("LOOK AND FEEL lost FONTS");
        assert_eq!(levels + 1, fonts_at, "SOUND LEVELS does not stand above FONTS");
        assert!(
            button_at(&MENU_ROWS, Act::OpenSoundLevels).is_none(),
            "SOUND is still an entry of the main menu"
        );
        let Ctrl::Button { label: Text::Fixed(word), .. } = LOOKFEEL_ROWS[levels].ctrl
        else {
            panic!("the door lost its fixed label")
        };
        assert_ne!(
            word,
            ListId::Sounds.label(),
            "the door and the list wear one word: a reader cannot tell the \
             set from the levels"
        );

        let mut s = furnished();
        s.view = View::LookFeel;
        s.dropdown = Some(Dropdown::List(ListId::Sounds));
        let before = s.current_sounds.clone();
        assert!(
            !s.perform(Act::OpenSoundLevels, 0.0),
            "the door reported a configuration change"
        );
        assert!(s.view == View::SoundLevels, "the door opened the wrong page");
        assert_eq!(s.current_sounds, before, "the door changed the sound SET");
        assert!(s.dropdown.is_none(), "the list stayed open behind the page");
        // What it opened really is the levels: every act that page
        // describes is one of the three, and none of them is a pick.
        for act in described_acts(&s, page(View::SoundLevels)) {
            assert!(
                matches!(
                    act,
                    Act::Back
                        | Act::VolumeTrack
                        | Act::ToggleTyping
                        | Act::ToggleAmbient
                ),
                "the levels page describes a control that is not a level"
            );
        }
        // And the way back out is the page the door stands on.
        assert!(parent_view(View::SoundLevels) == Some(View::LookFeel));
    }

    /// The row drawn as the one IN FORCE is the row a click applies.
    ///
    /// The mark and the pick are two derivations of one index, made in
    /// two places ([`Settings::current_row`] against `Act::Pick`), and a
    /// list that marked row 2 while its click wrote row 3 would be
    /// worse than a list that marked nothing: it would lie about what
    /// is standing. So the index is walked over every name of every
    /// list, and over the one shift there is — the `DEFAULT` entry the
    /// font families are drawn behind.
    #[test]
    fn the_marked_row_is_the_row_a_click_applies() {
        let mut s = furnished();
        for list in [ListId::Looks, ListId::Layauts, ListId::Sounds] {
            for i in 0..s.names(list).len() {
                let name = s.names(list)[i].clone();
                match list {
                    ListId::Looks => s.current_look = Some(name),
                    ListId::Layauts => s.current_layaut = Some(name),
                    ListId::Sounds => s.current_sounds = Some(name),
                }
                assert_eq!(
                    s.current_row(list),
                    Some(i),
                    "{}: the mark is not on the name in force",
                    list.label()
                );
            }
            // A set whose standing member is not installed here has no
            // standing member — and no mark, rather than a mark on the
            // first name.
            match list {
                ListId::Looks => s.current_look = Some("not installed".into()),
                ListId::Layauts => s.current_layaut = Some("not installed".into()),
                ListId::Sounds => s.current_sounds = Some("not installed".into()),
            }
            assert_eq!(
                s.current_row(list),
                None,
                "{}: a name nobody has is marked as standing",
                list.label()
            );
        }

        // The families are drawn behind a DEFAULT entry this file adds,
        // so their rows are shifted by exactly one — and an unset
        // family is that entry, because that is what the desktop
        // resolves it to.
        let si = Settings::sect_idx(Sect::Ui);
        s.families[si] = ["alpha", "beta"].iter().map(|f| f.to_string()).collect();
        s.cur_family[si] = None;
        assert_eq!(s.family_row(Sect::Ui), Some(0), "DEFAULT is not the standing family");
        s.cur_family[si] = Some("beta".to_string());
        assert_eq!(s.family_row(Sect::Ui), Some(2));
        s.cur_family[si] = Some("gamma".to_string());
        assert_eq!(s.family_row(Sect::Ui), None, "a family nobody has is marked");
        // The weights are a fixed table with nothing in front of it,
        // and the configuration keeps whatever case was written.
        s.cur_weight[si] = Some("semibold".to_string());
        assert_eq!(s.weight_row(Sect::Ui), WEIGHTS.iter().position(|w| *w == "SemiBold"));
        s.cur_weight[si] = None;
        assert_eq!(s.weight_row(Sect::Ui), None);
    }

    /// Every position of the page is one Tab away — with a list open
    /// as well as at rest.
    ///
    /// [`every_described_control_is_reachable`] asks this of the page's
    /// DESCRIPTION, which cannot speak for what an unfolded list puts
    /// on the screen: the names, and the door standing over them. Those
    /// are the rows a keyboard user would lose, and the door is the one
    /// most easily lost, because it is the only control of this window
    /// drawn outside the row walker. So this walks the chain the way
    /// the user does — press Tab until it comes back round — and
    /// insists that everything the pointer could press was landed on.
    #[test]
    fn every_position_of_look_and_feel_is_a_tab_away() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let open: [Option<ListId>; 4] =
            [None, Some(ListId::Looks), Some(ListId::Layauts), Some(ListId::Sounds)];
        for list in open {
            let mut s = furnished();
            s.view = View::LookFeel;
            s.dropdown = list.map(Dropdown::List);
            // Fully unfolded: a row still in flight registers nothing,
            // by the list object's own rule.
            s.dropdown_since = None;
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::new();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            ctx.focus = Some(&mut fc);
            s.draw(&mut ctx);
            // Navigation walks the last COMPLETED frame, so the frame
            // the drawing built has to be closed before Tab can see it.
            fc.begin_frame();
            let mut landed: Vec<FocusId> = Vec::new();
            for _ in 0..s.hits.len() * 2 + 8 {
                fc.nav(Nav::Next);
                if let Some(id) = fc.focused() {
                    landed.push(id);
                }
            }
            for &(_, act) in &s.hits {
                assert!(
                    landed.contains(&focus_id(act)),
                    "a control the pointer can press is not on the Tab round: \
                     {} names {} controls",
                    list.map_or("the page at rest", |l| l.label()),
                    s.hits.len()
                );
            }
        }
    }

    /// Every anchor wears the toolkit's disclosure triangle, and the
    /// triangle turns when its list unfolds.
    ///
    /// The affordance is the only thing on the page that says a row is
    /// a list and not a button, and it is the toolkit's glyph
    /// ([`nacelle::view::paint::disclosure`]) in its DROP grammar:
    /// closed it points DOWN, at the direction the list will unfold,
    /// and open it points back up at the edge the list folds into. A
    /// caret announces where the list goes, not the fact that it is
    /// currently shut — `▷` is the tree's sentence, and it reads here
    /// as "go into this row". The state turns the GLYPH, so the shape
    /// is what this test reads.
    #[test]
    fn every_list_anchor_wears_a_caret_that_turns() {
        /// The closed three-point outlines one frame drew — the shape
        /// `paint::disclosure` makes, and nothing else on this page.
        fn carets(dl: &nacelle::draw::DrawList) -> Vec<Vec<[f32; 2]>> {
            dl.cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Polyline { pts, closed: true, .. }
                        if pts.len() == 3 =>
                    {
                        Some(pts.clone())
                    }
                    _ => None,
                })
                .collect()
        }
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let drawn = |fonts: &mut nacelle::font::FontSystem, open: Option<ListId>| {
            let mut s = furnished();
            s.view = View::LookFeel;
            s.dropdown = open.map(Dropdown::List);
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            carets(&dl)
        };
        let rest = drawn(&mut fonts, None);
        assert_eq!(rest.len(), 3, "one caret per list, and no more");
        for pts in &rest {
            // ▼ — a flat top edge (its two ends on one y) with the apex
            // below them, pointing at where the list will unfold.
            assert!(
                (pts[0][1] - pts[1][1]).abs() < 0.01 && pts[2][1] > pts[0][1],
                "a closed list's caret is not pointing at where its list goes"
            );
        }
        let open = drawn(&mut fonts, Some(ListId::Looks));
        assert_eq!(open.len(), 3, "an open list took a caret with it");
        // ▲ — the same triangle upside down: a flat BOTTOM edge with the
        // apex above it, back at the edge the list folds into. Exactly
        // one caret turns, because only one list is ever unfolded.
        assert_eq!(
            open.iter()
                .filter(|p| (p[1][1] - p[2][1]).abs() < 0.01 && p[0][1] < p[1][1])
                .count(),
            1,
            "the open list's caret never turned"
        );
    }

    /// Every text run one frame wrote, in call order.
    fn text_runs(dl: &nacelle::draw::DrawList) -> Vec<String> {
        dl.cmds()
            .iter()
            .filter_map(|c| match c {
                nacelle::draw::DrawCmd::Text { text, .. } => Some(text.clone()),
                nacelle::draw::DrawCmd::ModuleTitle { left, .. } => Some(left.clone()),
                _ => None,
            })
            .collect()
    }

    /// One page, drawn at rest, as the text it wrote.
    fn page_runs(fonts: &mut nacelle::font::FontSystem, s: &mut Settings) -> Vec<String> {
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        text_runs(&dl)
    }

    /// Decision §2b — an anchor wears its list's NAME, and nothing else.
    ///
    /// The anchors read "THEMES: MIDNIGHT" until the owner ruled
    /// otherwise, deliberately against the font page's own convention.
    /// So the test is two-sided: the name has to be there, and the
    /// value the anchor used to carry — the choice in force, or the
    /// note an empty list stands behind — may not be anywhere on the
    /// page. Half a rename would leave the old spelling drawn under a
    /// new one.
    #[test]
    fn a_list_anchor_wears_its_name_and_not_its_choice() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeel;
        let drawn = page_runs(&mut fonts, &mut s);
        for list in [ListId::Looks, ListId::Layauts, ListId::Sounds] {
            assert!(
                drawn.iter().any(|t| t == list.label()),
                "{} does not wear its own name",
                list.label()
            );
            let value = s.drop_value(list);
            assert!(
                !drawn.iter().any(|t| t.contains(&value)),
                "{} still wears its choice: {value}",
                list.label()
            );
        }
    }

    /// Decision §2a — the reset cannot be done by one press.
    ///
    /// Six settings and a pinned arrangement go at once and nothing
    /// puts them back, so the footer may only OPEN the confirmation:
    /// it writes nothing, asks the application for nothing, and the
    /// control that does the work is described by one page alone. That
    /// last assertion is the guard itself — a confirm act described by
    /// the LOOK AND FEEL page too would be a second door standing open
    /// beside the locked one.
    ///
    /// The reset itself is not performed here, and cannot be: it
    /// writes the configuration of whoever runs the tests.
    #[test]
    fn the_look_and_feel_reset_takes_a_second_press_on_another_page() {
        let mut s = furnished();
        s.view = View::LookFeel;
        assert!(
            !s.perform(Act::LookFeelReset, 0.0),
            "the footer reported a configuration change"
        );
        assert!(s.view == View::LookFeelReset, "the footer opened nothing");
        assert!(!s.reset_screen, "the footer asked for the screen to be cleared");

        for p in PAGES.iter() {
            assert_eq!(
                described_acts(&s, p).contains(&Act::LookFeelResetConfirm),
                p.view == View::LookFeelReset,
                "{}: the reset's own control is on the wrong page",
                p.title
            );
        }

        // And changing your mind is the ordinary way out, leaving
        // everything exactly as it was.
        let mut fc = FocusCtl::new();
        let esc =
            KeyEv { key: FKey::Escape, mods: Mods::NONE, repeat: false, text: None };
        assert!(matches!(s.key(&esc, &mut fc), KeyOut::Consumed));
        assert!(s.view == View::LookFeel, "Escape did not come back to the page");
        assert!(!s.reset_screen, "cancelling still asked for a reset");
    }

    /// Decision §2a — the confirmation says what it is about to spend.
    ///
    /// A confirmation that only asks "are you sure" is a speed bump.
    /// This one names the theme, the layout and the sound set standing
    /// in the configuration right now — the reading that used to sit on
    /// the anchors (§2b) — and the two settings that have no name to
    /// give: the fonts and the pinned arrangement.
    #[test]
    fn the_confirmation_names_what_the_reset_clears() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeelReset;
        let drawn = page_runs(&mut fonts, &mut s);
        let said = |needle: &str| drawn.iter().any(|t| t.contains(needle));
        for list in [ListId::Looks, ListId::Layauts, ListId::Sounds] {
            let value = s.drop_value(list);
            assert!(
                said(&value),
                "the confirmation does not say what {} is set to",
                list.label()
            );
        }
        assert!(said("FONTS"), "the confirmation does not mention the fonts");
        assert!(
            said("PINNED"),
            "the confirmation does not mention the pinned arrangement"
        );
    }

    /// Every view can be left, and no view is its own way out. A cycle
    /// here would be a window Escape cannot get out of.
    #[test]
    fn every_view_but_the_menu_has_a_way_out() {
        for p in PAGES.iter() {
            let mut v = p.view;
            let mut steps = 0;
            while let Some(up) = parent_view(v) {
                v = up;
                steps += 1;
                assert!(steps <= PAGES.len(), "{}: the way out is a circle", p.title);
            }
            assert!(v == View::Menu, "{}: the way out is not the menu", p.title);
        }
    }

    /// An open list offers one pressable row per name, plus whatever it
    /// carries in front of them — and every row picks the name it is
    /// drawn against.
    ///
    /// This is the page's whole worth: the three choices moved out of
    /// pages of their own and into lists that hang over one page, and a
    /// list that loses a name loses a theme the user installed.
    #[test]
    fn an_open_list_offers_every_name_it_has() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for list in [ListId::Looks, ListId::Layauts, ListId::Sounds] {
            let mut s = furnished();
            s.view = View::LookFeel;
            s.dropdown = Some(Dropdown::List(list));
            // Fully unfolded: a list caught mid-animation is drawn but
            // not yet answerable, by the object's own rule.
            s.dropdown_since = None;
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let acts: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
            for i in 0..s.names(list).len() {
                assert!(
                    acts.contains(&Act::Pick(list, i)),
                    "{}: name {i} is in the list and not on the screen",
                    list.label()
                );
            }
            assert_eq!(
                acts.contains(&Act::ThemesEditor),
                list.carries_door(),
                "{}: the editor door is on the wrong list",
                list.label()
            );
        }
    }

    /// The reach P11 used to guard, pinned the WRONG way round on
    /// purpose, because the toolkit cannot give it back yet.
    ///
    /// P11 (`a_long_list_loses_no_name`) stocked forty names on the LOOK
    /// page and walked the page's scroll from top to bottom, asserting
    /// every name was drawn INSIDE the body's box. That page is gone,
    /// and with it its scroll: `object::dropdown::accordion` lays its
    /// rows out as one column of `item_h * names.len()` with no scroll,
    /// no `max_rows` and no flip, and this file may not grow a second
    /// list of its own to fix that (the toolkit rule). The half of P11
    /// that still holds — no name is dropped — is
    /// [`an_open_list_offers_every_name_it_has`]; this is the half that
    /// does not.
    ///
    /// So it asserts the FAULT: with forty themes the tail of the list
    /// is pressable below the window's bottom edge. The day the
    /// accordion learns to scroll (the fleet's report asks for scroll +
    /// max_rows + flip) this test fails, and whoever made it pass has to
    /// come here and turn it back into P11. A regression nobody can
    /// delete quietly is worth more than a guard nobody replaced.
    #[test]
    fn a_long_list_hangs_past_the_window_until_the_accordion_can_scroll() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        const N: usize = 40;
        let mut s = furnished();
        s.view = View::LookFeel;
        s.themes = (0..N).map(|i| format!("theme {i}")).collect();
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        s.dropdown_since = None;
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let h = ctx.h;
        s.draw(&mut ctx);
        // Offered, all forty of them — that much the list does keep.
        let acts: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
        for i in 0..N {
            assert!(
                acts.contains(&Act::Pick(ListId::Looks, i)),
                "name {i} is in the list and not in the hit map"
            );
        }
        // And offered where the pointer cannot follow.
        let lost = s
            .hits
            .iter()
            .filter(|(r, a)| matches!(a, Act::Pick(ListId::Looks, _)) && r.y > h)
            .count();
        assert!(
            lost > 0,
            "forty names now fit the window: the accordion has learnt to \
             scroll, so turn this test back into P11 — every name inside \
             the body's box, walking the list's own scroll"
        );
        viewport_home();
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
    ///
    /// `ui_font_scale` is carried the way a frame carries it and is NOT a
    /// lever on the drawing: the interface scale reaches the window
    /// through `theme::set_viewport`, so a test that wants to move it
    /// moves the viewport (`crate::widgets::assert_scales_once`).
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
    /// (which would mean a row nobody can reach). That footer used
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
                    let rh = s.row_h(&row.ctrl, m, content);
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
                    let rh = s.row_h(&row.ctrl, m, content);
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

    /// A page whose flow is longer than its box can be scrolled to its
    /// end, and every row it describes is reachable somewhere along the
    /// way.
    ///
    /// P11 was a list that `break`-ed out of its own loop at the bottom
    /// edge: past about twenty entries a name simply was not there,
    /// with no bar, no count and no notice. The lists have since moved
    /// into drop-downs, which is a DIFFERENT surface with the same
    /// question still open (see the fleet's report: the accordion does
    /// not scroll). What is tested here is the half that stayed — the
    /// page's own flow, walked from top to bottom with a page of extra
    /// rows in it.
    #[test]
    fn a_page_longer_than_its_box_reaches_its_last_row() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        // The tallest page the description has, drawn short: FONT's two
        // sections do not fit a 500 px window, which is the case the
        // scrolling exists for.
        let p = page(View::Font);
        let stocked = |offset: f32| {
            let mut s = furnished();
            s.view = View::Font;
            s.scroll.set_offset(offset);
            s
        };
        let (furthest, step, last_act) = {
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, 500.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let s = stocked(0.0);
            (
                (s.flow_h(p, m, content) - s.body_box(p, m, content).h).max(0.0),
                m.btn_h,
                Act::WeightBtn(Sect::Ui),
            )
        };
        assert!(furthest > 0.0, "the FONT page ought not to fit at 500 px");
        let mut seen = false;
        let mut offset = 0.0f32;
        while offset <= furthest + step {
            let mut s = stocked(offset);
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 500.0, 1.0);
            s.draw(&mut ctx);
            seen |= s.hits.iter().any(|&(_, a)| a == last_act);
            offset += step;
        }
        assert!(seen, "the last row of the page was never drawn");
        viewport_home();
    }

    /// §5.3 — the window's text answers UIFontSize=, all of it, once.
    ///
    /// Every page is drawn twice, at 100 % and at 125 %, and every string
    /// both frames wrote has to be exactly 25 % bigger in the second. A
    /// run that did not move is a run the preference never reached —
    /// which is what this window used to be made of: `object::button`
    /// scaled its labels while the row labels, values, notes, captions
    /// and the title around them stayed at 100 %, so the one screen that
    /// sets the interface size was the one screen where setting it half
    /// worked.
    ///
    /// A run that moved by MORE than 25 % is the other half of the same
    /// bug, and the one this window was likeliest to have: the preference
    /// is `metric.ui_scale`, the frame hands it to the viewport, and a
    /// drawer that multiplies a baked px by it as well squares it. The
    /// scale is therefore driven through the viewport here — the way the
    /// program drives it — and not through `Ctx::ui_font_scale`, which no
    /// longer moves a token and must not.
    #[test]
    fn every_run_in_the_window_answers_the_interface_scale_exactly_once() {
        let _g = crate::widgets::theme_test_lock();
        for p in PAGES.iter() {
            crate::widgets::assert_scales_once(p.title, 1080.0, 0.0, |ctx| {
                // A window of its own per frame: `draw` settles hover and
                // animation state, and the two frames must be one picture.
                let mut s = furnished();
                s.view = p.view;
                s.draw(ctx);
            });
        }
        // And at every height the program is built for, because u is
        // clamped at both ends and `metric.ui_scale` multiplies after the
        // clamp: a viewport already sitting on `unit_max_px` must still
        // answer the preference.
        for h in HEIGHTS {
            crate::widgets::assert_scales_once("the settings window", h, 0.0, |ctx| {
                let mut s = furnished();
                s.draw(ctx);
            });
        }
        viewport_home();
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
            Chrome::Back => Act::Back,
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
                // The anchor alone: what the list holds is only on
                // screen while it is open, which is another test's
                // question (`an_open_list_offers_every_name_it_has`).
                Ctrl::Drop { list } => out.push(Act::ListBtn(*list)),
                Ctrl::Section { .. }
                | Ctrl::Note { .. }
                | Ctrl::Empty { .. }
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

