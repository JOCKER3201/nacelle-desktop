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
//! THREE PANELS, NOT A STACK OF PAGES (owner, 2026-08-16, the
//! specification's annex). The window carries a permanent navigation
//! RAIL down its left edge — every section of the window under the
//! headings its group stands for — and, for a section that has pages of
//! its own, a second column of those pages beside it. What is left is
//! the page. There is no MENU page any more: the window opens on LOOK
//! AND FEEL, the rail is how a section is reached, and Escape from a
//! section is the window's own last layer rather than a step back to a
//! menu that no longer exists. Both navigation columns are the same
//! width by the theme's own word (`settings.subrail_w_frac =
//! @settings.rail_w_frac`) and the page takes the whole of the rest.
//!
//! The layout is FLEX: where the three panels cannot all have their
//! width — `settings.col_min_w` for the page, with the usual device-px
//! floor — the whole window folds into ONE vertical list, the rail's
//! entries first, then the section's pages, then the page itself, all
//! inside the one scroll. The Tab order is the same in both shapes,
//! because registration follows the DESCRIPTION and never the geometry.
//! It follows it off the frame as well: a row the scroll has carried
//! out of sight is not drawn and is not a target, but it keeps its
//! place in the route out of the rect the layout gave it, and the
//! scroll goes and fetches back whatever the keyboard lands on
//! ([`Settings::register_offscreen`], [`Settings::chase_focus`]).
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
// The two ladders' walls and ceilings, TAKEN from the stage that applies
// them (`theme/bake.rs`) instead of written out beside every slider that
// runs to them. Six call sites here used to carry the same four numbers.
use nacelle::theme::bake::{
    SURFACE_CHROMA_CEILING, SURFACE_LIFT_WALL, TEXT_CHROMA_CEILING, TEXT_LIFT_WALL,
};
use nacelle::theme::parse::State;
use nacelle::theme::{self, TokenId};
use nacelle::view::scroll::{self, ScrollPhysics, ScrollView, ScrollbarLook};
use nacelle::view::{CtxSurface, Snap};
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
enum View {
    /// Where the window opens. It is a section of the rail like any
    /// other; it is first because the rail lists it first, and not
    /// because anything about it is special.
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
    /// What the addons on this machine were told to do — the settings
    /// files the user writes by hand, and every one the program could
    /// not use.
    Addons,
}

/// The view one layer out, or `None` at the outermost one.
///
/// The Escape ladder and the BACK button read the SAME answer, which is
/// what stops the two ways out of a page from disagreeing. The window's
/// own last layer — closing it — is not here: that is the application's
/// Escape ([`KeyOut::Ignored`]), and this window peels one layer per
/// press until there is none left to peel.
///
/// The ladder is TWO rungs shorter than it was (owner, 2026-08-16): a
/// page the navigation reaches in one press has nothing to go back TO,
/// so every section of the rail and every page of the second column
/// answers `None` here and wears CLOSE. What is left is what the
/// navigation does not list — the theme editor, which stands at the head
/// of the THEMES list, and the reset confirmation, which is what the
/// pinned footer opens. Those two keep their way back, and Escape from
/// them lands on the page that opened them rather than on the desktop.
fn parent_view(v: View) -> Option<View> {
    match v {
        // A destructive control the user changed their mind about must
        // be left the same way as anything else, by BACK or by Escape.
        View::ThemeEditor | View::LookFeelReset => Some(View::LookFeel),
        _ => None,
    }
}

/// The corner button a view wears, DERIVED from the ladder above: a
/// page with somewhere to go back to says BACK, and a page the
/// navigation reaches says CLOSE. The table states it as well, because
/// the walker reads it there — and a test holds the two together.
fn chrome_of(v: View) -> Chrome {
    match parent_view(v) {
        Some(_) => Chrome::Back,
        None => Chrome::Close,
    }
}

/// Which number of the editor's colour a track moves.
///
/// The theme writes colours as `oklch(L, C, H)`, so a colour is three
/// numbers and a slider moves one of them. Named rather than indexed
/// because a swapped pair would be a colour that is merely wrong instead
/// of a compile error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Knob {
    EdgeL,
    EdgeC,
    EdgeH,
    /// The glass TINT — the multiply quad, the one that can only darken.
    TintB,
    TintS,
    TintH,
    /// The glass WASH — the alpha-over quad, the only one that brightens.
    WashB,
    WashS,
    WashH,
    /// The whole effect's opacity, every kind.
    BgOpacity,
    /// The blur pyramid depth, BLUR and FROSTED.
    BgDepth,
    /// The wash's coverage, FROSTED only.
    BgCoverage,
    // ---- the whole-theme groups (2026-08-16): one knob per number the
    // ---- model in theme/edit.rs takes, nothing that has no set to join.
    /// The one seed the interface re-derives itself from
    /// (`palette.accent`, written opaque by the model).
    AccentB,
    AccentS,
    AccentH,
    /// The surface ladder's own hue, degrees — only while OWN HUE is on;
    /// off, the set writes the reference `@hue.accent` back instead.
    SurfHue,
    /// `surface.lift`, the bake's own wall either way on a 0..100 track
    /// ([`nacelle::theme::bake::SURFACE_LIFT_WALL`]).
    SurfLift,
    /// `surface.chroma`, the bake's own ceiling on a 0..100 track
    /// ([`nacelle::theme::bake::SURFACE_CHROMA_CEILING`]).
    SurfChroma,
    /// `text.lift`, the text ladder's wall likewise.
    TextLift,
    /// `text.chroma`, the text ladder's ceiling likewise.
    TextChroma,
    /// The chosen severity role's author colour (`severity.<role>.text`).
    SevB,
    SevS,
    SevH,
    /// The three preset radii and the two counts of the shape set —
    /// radii on 0..100 tracks over the model's 4u wall, the kerf over
    /// its 1u wall, segments a bare 3..16.
    CornerSm,
    CornerMd,
    CornerLg,
    CornerSeg,
    Hairline,
    /// The focus ring's stroke and rhythm, 0..100 over the declared
    /// walls (width/offset 2u, dash/gap open-ended — 4u of track).
    RingW,
    RingOffset,
    RingB,
    RingS,
    RingH,
    RingDash,
    RingGap,
    /// `glow.focus_ring.alpha`, 0..1.
    HaloAlpha,
    /// `focus.unfocused_dim`, the declared 0.3..1.0 floor kept by the
    /// track's own range (30..100).
    UnfocusedDim,
    /// The context menu's four tokens: bed, ring, ring width, hint ink.
    MenuFillB,
    MenuFillS,
    MenuFillH,
    MenuEdgeB,
    MenuEdgeS,
    MenuEdgeH,
    MenuEdgeW,
    MenuHintB,
    MenuHintS,
    MenuHintH,
    /// The tooltip's four, the menu's sibling float.
    TipFillB,
    TipFillS,
    TipFillH,
    TipEdgeB,
    TipEdgeS,
    TipEdgeH,
    TipEdgeW,
    TipTextB,
    TipTextS,
    TipTextH,
    /// The scrollbar's widths (0..100 over 0.5u..4u), its fade
    /// (0..100 over 0..2000ms) and the groove's colour.
    BarW,
    BarWHover,
    BarFade,
    BarTrackB,
    BarTrackS,
    BarTrackH,
    // ---- the BASIC page (2026-08-17). Three knobs for the WHOLE theme,
    // ---- and every one of them a RELATIVE move over whatever the theme
    // ---- already says — a rotation, a multiplier and an offset. They
    // ---- are not a fourth way of writing a colour: the model turns them
    // ---- into edits to the AUTHORS the rest of the theme derives from
    // ---- (`theme::edit::tone_edits`), and the cascade does the rest.
    /// Degrees the whole theme is turned by. One hue for the interface,
    /// and the severity roles carried round together so `ok` stays as
    /// far from `critical` as its author put it.
    ToneHue,
    /// The multiplier over every author's chroma; 100 is the theme
    /// unchanged.
    ToneSat,
    /// The offset over every author's lightness and over the surface and
    /// text ladders' two lifts; 50 is the theme unchanged.
    ToneLight,
}

/// Which of the editor's switches a toggle row flips. Named like [`Knob`]
/// and for the same reason: a swapped pair would be a switch that is
/// merely wrong instead of a compile error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flip {
    /// OFF restores `surface.hue = @hue.accent` as a REFERENCE (the
    /// model's `SurfaceHue::FollowAccent`); ON cuts the surfaces loose
    /// with plain degrees from the HUE track under it.
    SurfaceOwnHue,
    /// `focus.ring.enabled`. OFF is the flag alone — the model leaves
    /// the ring's whole dress standing, LINE's lesson.
    Ring,
    /// `glow.focus_ring.enabled`, dressing itself like NEON on a theme
    /// whose halo has no radius yet.
    Halo,
    /// `scrollbar.auto_hide`; the FADE track appears with it, because
    /// the declaration reads the fade only while this is on.
    BarAutoHide,
    /// `scrollbar.track`; OFF is the switch alone and the theme's own
    /// groove colour survives the trip.
    BarTrack,
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Close,
    Back,
    /// The rail's LOOK AND FEEL section.
    OpenLookFeel,
    /// The same page, reached from the SECOND column instead of the
    /// rail. It does exactly what [`Act::OpenLookFeel`] does and exists
    /// only so the two entries are two places in the focus chain: one
    /// act drawn twice in a frame would register one id at two rects,
    /// and Tab would land on whichever the drawing wrote last.
    OpenSets,
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
    /// Reads the toolkit's report on the addon settings files, then
    /// shows it. Read on the way in rather than every frame: the list
    /// grows as widgets are built, and a page that re-asked while it
    /// was open would change under the eye for no reason the user
    /// could connect to anything they did.
    OpenAddons,
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
    /// The editor's BASIC/ADVANCED switch, at the head of the page.
    /// Steps to the other mode and shows which one is in force
    /// ([`Ctrl::Cycle`] with two members is a toggle that says its own
    /// state, which a bare switch could not).
    EditorMode,
    /// One of the theme editor's colour tracks.
    EditorTrack(Knob),
    /// One of the theme editor's switches ([`Flip`]).
    EditorFlip(Flip),
    /// Write the edit set into the theme in force — or, for `default`,
    /// fall through to SAVE AS: the master is not a file.
    EditorSave,
    /// Write the edit set under a new name, asked for in a prompt.
    EditorSaveAs,
    /// Drop the preview and reseed the controls from the theme.
    EditorCancel,
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
        // The rail's own paths: a section is where it is reached from,
        // and it is reached from the rail on every page of the window.
        OpenLookFeel => FocusId::of("settings.rail.lookfeel"),
        OpenGrid => FocusId::of("settings.rail.grid"),
        OpenBoards => FocusId::of("settings.rail.boards"),
        OpenColor => FocusId::of("settings.rail.color"),
        OpenBlur => FocusId::of("settings.rail.blur"),
        OpenAddons => FocusId::of("settings.rail.addons"),
        // The second column: the pages of the section the rail is
        // standing on.
        OpenSets => FocusId::of("settings.lookfeel.sets"),
        OpenFont => FocusId::of("settings.lookfeel.fonts"),
        OpenSoundLevels => FocusId::of("settings.lookfeel.sound_levels"),
        EditorSave => FocusId::of("settings.editor.save"),
        EditorSaveAs => FocusId::of("settings.editor.saveas"),
        EditorCancel => FocusId::of("settings.editor.cancel"),
        EditorMode => FocusId::of("settings.editor.mode"),
        EditorTrack(k) => FocusId::of(match k {
            Knob::EdgeL => "settings.editor.edge.l",
            Knob::EdgeC => "settings.editor.edge.c",
            Knob::EdgeH => "settings.editor.edge.h",
            Knob::TintB => "settings.editor.tint.b",
            Knob::TintS => "settings.editor.tint.s",
            Knob::TintH => "settings.editor.tint.h",
            Knob::WashB => "settings.editor.wash.b",
            Knob::WashS => "settings.editor.wash.s",
            Knob::WashH => "settings.editor.wash.h",
            Knob::BgOpacity => "settings.editor.bg.opacity",
            Knob::BgDepth => "settings.editor.bg.depth",
            Knob::BgCoverage => "settings.editor.bg.coverage",
            Knob::AccentB => "settings.editor.accent.b",
            Knob::AccentS => "settings.editor.accent.s",
            Knob::AccentH => "settings.editor.accent.h",
            Knob::SurfHue => "settings.editor.surface.hue",
            Knob::SurfLift => "settings.editor.surface.lift",
            Knob::SurfChroma => "settings.editor.surface.chroma",
            Knob::TextLift => "settings.editor.text.lift",
            Knob::TextChroma => "settings.editor.text.chroma",
            Knob::SevB => "settings.editor.severity.b",
            Knob::SevS => "settings.editor.severity.s",
            Knob::SevH => "settings.editor.severity.h",
            Knob::CornerSm => "settings.editor.corner.sm",
            Knob::CornerMd => "settings.editor.corner.md",
            Knob::CornerLg => "settings.editor.corner.lg",
            Knob::CornerSeg => "settings.editor.corner.segments",
            Knob::Hairline => "settings.editor.stroke.hair",
            Knob::RingW => "settings.editor.ring.w",
            Knob::RingOffset => "settings.editor.ring.offset",
            Knob::RingB => "settings.editor.ring.b",
            Knob::RingS => "settings.editor.ring.s",
            Knob::RingH => "settings.editor.ring.h",
            Knob::RingDash => "settings.editor.ring.dash",
            Knob::RingGap => "settings.editor.ring.gap",
            Knob::HaloAlpha => "settings.editor.ring.halo_alpha",
            Knob::UnfocusedDim => "settings.editor.focus.dim",
            Knob::MenuFillB => "settings.editor.menu.fill.b",
            Knob::MenuFillS => "settings.editor.menu.fill.s",
            Knob::MenuFillH => "settings.editor.menu.fill.h",
            Knob::MenuEdgeB => "settings.editor.menu.edge.b",
            Knob::MenuEdgeS => "settings.editor.menu.edge.s",
            Knob::MenuEdgeH => "settings.editor.menu.edge.h",
            Knob::MenuEdgeW => "settings.editor.menu.edge.w",
            Knob::MenuHintB => "settings.editor.menu.hint.b",
            Knob::MenuHintS => "settings.editor.menu.hint.s",
            Knob::MenuHintH => "settings.editor.menu.hint.h",
            Knob::TipFillB => "settings.editor.tooltip.fill.b",
            Knob::TipFillS => "settings.editor.tooltip.fill.s",
            Knob::TipFillH => "settings.editor.tooltip.fill.h",
            Knob::TipEdgeB => "settings.editor.tooltip.edge.b",
            Knob::TipEdgeS => "settings.editor.tooltip.edge.s",
            Knob::TipEdgeH => "settings.editor.tooltip.edge.h",
            Knob::TipEdgeW => "settings.editor.tooltip.edge.w",
            Knob::TipTextB => "settings.editor.tooltip.text.b",
            Knob::TipTextS => "settings.editor.tooltip.text.s",
            Knob::TipTextH => "settings.editor.tooltip.text.h",
            Knob::BarW => "settings.editor.scrollbar.w",
            Knob::BarWHover => "settings.editor.scrollbar.w_hover",
            Knob::BarFade => "settings.editor.scrollbar.fade",
            Knob::BarTrackB => "settings.editor.scrollbar.track.b",
            Knob::BarTrackS => "settings.editor.scrollbar.track.s",
            Knob::BarTrackH => "settings.editor.scrollbar.track.h",
            Knob::ToneHue => "settings.editor.tone.hue",
            Knob::ToneSat => "settings.editor.tone.sat",
            Knob::ToneLight => "settings.editor.tone.light",
        }),
        EditorFlip(f) => FocusId::of(match f {
            Flip::SurfaceOwnHue => "settings.editor.surface.own_hue",
            Flip::Ring => "settings.editor.ring.on",
            Flip::Halo => "settings.editor.ring.halo",
            Flip::BarAutoHide => "settings.editor.scrollbar.auto_hide",
            Flip::BarTrack => "settings.editor.scrollbar.track_on",
        }),
        ListBtn(l) => FocusId::of(match l {
            ListId::Looks => "settings.lookfeel.themes",
            ListId::Layauts => "settings.lookfeel.layauts",
            ListId::Sounds => "settings.lookfeel.sounds",
            ListId::Borders => "settings.editor.border",
            ListId::Backgrounds => "settings.editor.background",
            ListId::Severities => "settings.editor.severity",
            ListId::Corners => "settings.editor.corner",
            ListId::RingStyles => "settings.editor.ring.style",
            ListId::ScrollModes => "settings.editor.scrollbar.mode",
            ListId::ScrollEdges => "settings.editor.scrollbar.edge",
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
        Dropdown::List(ListId::Borders) => "settings.editor.border.list",
        Dropdown::List(ListId::Backgrounds) => "settings.editor.background.list",
        Dropdown::List(ListId::Severities) => "settings.editor.severity.list",
        Dropdown::List(ListId::Corners) => "settings.editor.corner.list",
        Dropdown::List(ListId::RingStyles) => "settings.editor.ring.style.list",
        Dropdown::List(ListId::ScrollModes) => "settings.editor.scrollbar.mode.list",
        Dropdown::List(ListId::ScrollEdges) => "settings.editor.scrollbar.edge.list",
    })
}

/// The part of `r` the clip leaves on screen, or nothing at all.
fn visible(r: Rect, clip: Option<Rect>) -> Option<Rect> {
    let Some(c) = clip else { return Some(r) };
    let (x0, y0) = (r.x.max(c.x), r.y.max(c.y));
    let (x1, y1) = (r.right().min(c.right()), r.bottom().min(c.bottom()));
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Where a slider's track stands in its row: after the label column,
/// stopping short of the value the row writes at its right edge.
fn track_rect(rc: RowCtx) -> Rect {
    Rect::new(
        rc.content.x + rc.label_w,
        rc.band.y,
        rc.content.w - rc.label_w - rc.value_w,
        rc.band.h,
    )
}

/// Where a cycler's plate stands: everything after the label column —
/// it writes its value INSIDE the plate, so it keeps the value gutter.
fn cycle_rect(rc: RowCtx) -> Rect {
    Rect::new(rc.content.x + rc.label_w, rc.band.y, rc.content.w - rc.label_w, rc.band.h)
}

/// Where each of `n` segments stands: the row after its label, split
/// equally, `segmented.gap` between.
///
/// These three and [`Settings::button_rect`]/[`Settings::bar_plates`] are
/// the only statements of where a control's targets are. The drawing
/// places its ink through them and [`Settings::targets`] reads them back,
/// so a rect the eye sees and a rect the chain registers cannot be two
/// different rects.
fn chip_rects(n: usize, rc: RowCtx) -> Vec<Rect> {
    let count = n.max(1) as f32;
    let cw = (rc.content.w - rc.label_w - rc.m.seg_gap * (count - 1.0)) / count;
    (0..n)
        .map(|i| {
            Rect::new(
                rc.content.x + rc.label_w + (cw + rc.m.seg_gap) * i as f32,
                rc.band.y,
                cw,
                rc.band.h,
            )
        })
        .collect()
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
/// HSV -> RGB, all components 0..1 (hue in degrees). The editor's colour
/// model is HSV because that is how the owner thinks about it: brightness
/// at 100 % is the FULL brightness of the chosen hue — red lands on
/// #FF0000 — never white. OKLCh's lightness at 1.0 is white by definition,
/// which read as a broken slider. The theme file still receives `oklch`:
/// the conversion runs at the seam, so the editor speaks HSV and the file
/// keeps its native space.
///
/// **The RGB on this side of the pair is sRGB-ENCODED**, which is what
/// makes "#FF0000 at brightness 100" the sentence above and not a claim
/// about photons — and it is the space the bake hands colours back in
/// (`theme/bake.rs`, `Color::to_srgb` on the Unorm path). Every crossing
/// to OKLCh therefore decodes first ([`Color::to_linear`]) and every
/// crossing back encodes ([`Color::to_srgb`]): OKLCh is defined over
/// LINEAR light, and feeding it encoded values reads a colour lighter
/// than it is. Measured before this note existed: the master's accent
/// answered L 0.8904 instead of 0.8200, and six trips through the
/// editor's BASIC page walked it 0.8200 -> 0.8904 -> 0.9413 -> 0.9715
/// with every slider at rest, because the page seeded itself from what
/// it had just written.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (r + m, g + m, b + m)
}

/// RGB -> HSV, the way back for seeding the sliders off the live theme.
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

// The editor's slider unit maps, one pair per SHAPE of range. Both
// directions live here so a value survives the round trip seed ->
// slider -> file: a map written twice is a slider that reopens a notch
// away from where it was left. The walls the spans run to are the
// MODEL's clamps (theme/edit.rs), never numbers of this file's own.

/// A 0..100 track over a symmetric span: 50 is zero, the ends are ±span.
fn span_of(v: u32, span: f32) -> f32 {
    (v.min(100) as f32 - 50.0) / 50.0 * span
}
fn span_back(x: f32, span: f32) -> u32 {
    (50.0 + x / span * 50.0).round().clamp(0.0, 100.0) as u32
}

/// A 0..100 track over 0..hi — the radii (4u), the kerf and the ring
/// widths (1u, 2u), the halo's 0..1, the fade's 0..2000ms.
fn scale_of(v: u32, hi: f32) -> f32 {
    v.min(100) as f32 / 100.0 * hi
}
fn scale_back(x: f32, hi: f32) -> u32 {
    (x / hi * 100.0).round().clamp(0.0, 100.0) as u32
}

/// A 0..100 track over lo..hi — the scrollbar widths (0.5u..4u).
fn band_of(v: u32, lo: f32, hi: f32) -> f32 {
    lo + v.min(100) as f32 / 100.0 * (hi - lo)
}
fn band_back(x: f32, lo: f32, hi: f32) -> u32 {
    ((x - lo) / (hi - lo) * 100.0).round().clamp(0.0, 100.0) as u32
}

// ---- BASIC's three tracks. Their ends are the CONTROL's own span and
// ---- not a gamut wall: the owner ruled gamut limits out ("BEZ
// ---- OGRANICZEŃ zakresu"), and the model clamps nothing but what the
// ---- numbers MEAN — lightness stays inside 0..1, chroma cannot go
// ---- negative, hue wraps. What a track's end says is only how far one
// ---- drag can carry you, which is the same statement the other
// ---- eighty-two tracks in this file make with their own `range`.

/// A whole turn of the circle, in degrees.
const TONE_HUE_MAX: u32 = 359;
/// Up to twice the theme's own chroma; 100 leaves it alone, 0 greys it.
const TONE_SAT_MAX: u32 = 200;
/// The lightness offset's half-span: the WIDER of the bake's two ladder
/// walls, TAKEN from the toolkit and not written out again here. The
/// bake holds `surface.lift` to one and `text.lift` to the other, so a
/// track that went past the wider of the two would have ends that do
/// nothing. Which of them is wider is pinned by
/// `the_editor_slider_maps_survive_the_round_trip`.
const TONE_LIGHT_SPAN: f32 = nacelle::theme::bake::TEXT_LIFT_WALL;
/// One whole unit of the LIGHTNESS track, in OKLab L: the 0..100 track
/// covers -span..+span, so a unit is a fiftieth of the span.
const TONE_LIGHT_UNIT: f32 = TONE_LIGHT_SPAN / 50.0;

/// The three tracks at REST — the theme exactly as it stands. A
/// rotation of nothing, a multiplier of one and an offset of zero, which
/// is [`nacelle::theme::edit::Tone::NEUTRAL`] in track units.
const TONE_REST: [u32; 3] = [0, 100, 50];

/// A colour on the editor's three HSV tracks — brightness, saturation,
/// hue, in the whole units a slider moves.
///
/// The ONE map that way. Both readers use it: the seeding, which brings
/// the theme's own colours onto the tracks, and BASIC's fold, which
/// brings a moved author onto the same tracks. Two copies of it would
/// be two chances for the two to round a colour differently.
///
/// `c` is **sRGB-ENCODED** — the space [`hsv_to_rgb`] is the inverse of
/// and the space the bake answers in. A caller holding a colour that
/// came out of OKLCh holds LINEAR light and encodes it first
/// ([`Color::to_srgb`]); the fold did not, and handed the page a colour
/// four track units too bright.
fn hsv_track_of(c: nacelle::theme::Color) -> [u32; 3] {
    let (h, sat, v) = rgb_to_hsv(c.r, c.g, c.b);
    [
        (v * 100.0).round().clamp(0.0, 100.0) as u32,
        (sat * 100.0).round().clamp(0.0, 100.0) as u32,
        h.rem_euclid(360.0).round().clamp(0.0, 359.0) as u32,
    ]
}

/// A slider triple back to a colour: the inverse of [`hsv_track_of`], and
/// the ONE map this way for the reason there is one map the other.
///
/// The three readers are the write-out (`editor_edits`, where a track
/// becomes a value in the file), BASIC's fold, which carries a track the
/// three sliders moved but do not author, and the tests that measure
/// either.
///
/// THE DECODE IS PART OF THE MAP. The tracks hold an sRGB-ENCODED colour
/// and OKLCh is defined over LINEAR light, so the trip is HSV -> sRGB ->
/// LINEAR -> OKLCh and the decode is not optional. Without it the editor
/// wrote a lighter colour than the slider showed and — since the next
/// visit seeds off what was written — every visit wrote a lighter one
/// still: the accent's L climbed 0.8200 -> 0.8904 -> 0.9413 -> 0.9715 with
/// nobody dragging anything.
///
/// `a` is the CALLER's: 1.0 where the model forces opacity anyway, the
/// seed's own where the model passes a colour's channel through (menu,
/// tooltip, track).
fn oklch_of_track(hsv: &[u32; 3], a: f32) -> nacelle::theme::color::Oklch {
    let (r, g, b) =
        hsv_to_rgb(hsv[2] as f32, hsv[1] as f32 / 100.0, hsv[0] as f32 / 100.0);
    nacelle::theme::Color { r, g, b, a }.to_linear().to_oklch()
}

fn is_track(act: Act) -> bool {
    slider_of(act).is_some()
}

/// The row that describes the slider an act drives, if it drives one.
fn slider_of(act: Act) -> Option<&'static Ctrl> {
    PAGES
        .iter()
        .flat_map(all_page_rows)
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

/// NO … FOUND — the addon report's all-clear line.
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
    /// The theme editor's border kind. Unlike the three above it names no
    /// file: its members are the two shapes a border can take, and choosing
    /// one lays a value over the theme instead of writing a config line.
    Borders,
    /// The editor's background kind — the same arrangement as `Borders`,
    /// over the three shapes a surface's back can take.
    Backgrounds,
    /// The severity role the three sliders under it pin — §5.10's closed
    /// set, offered whole because each role is its own author token.
    Severities,
    /// The one corner cut the whole interface wears (`corner.mode`).
    Corners,
    /// How the focus ring is stroked (`focus.ring.style`).
    RingStyles,
    /// Whether the scrollbar takes layout space (`scrollbar.mode`).
    ScrollModes,
    /// Which side of the content the bar sits on (`scrollbar.edge`).
    ScrollEdges,
}

/// §5.10's severity roles in declaration order: the name the list offers,
/// the author token the pin writes, and the model's own name for the role
/// — ONE table, so the three spellings cannot drift apart.
const SEVERITY_ROLES: [(&str, &str, nacelle::theme::edit::SeverityRole); 7] = {
    use nacelle::theme::edit::SeverityRole as R;
    [
        ("OK", "severity.ok.text", R::Ok),
        ("INFO", "severity.info.text", R::Info),
        ("WARNING", "severity.warning.text", R::Warning),
        ("CRITICAL", "severity.critical.text", R::Critical),
        ("CONTAINED", "severity.contained.text", R::Contained),
        ("OFFLINE", "severity.offline.text", R::Offline),
        ("UNKNOWN", "severity.unknown.text", R::Unknown),
    ]
};

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
            ListId::Borders => "BORDER",
            ListId::Backgrounds => "BACKGROUND",
            ListId::Severities => "SEVERITY ROLE",
            ListId::Corners => "CORNER CUT",
            ListId::RingStyles => "RING STYLE",
            ListId::ScrollModes => "SCROLLBAR MODE",
            ListId::ScrollEdges => "SCROLLBAR EDGE",
        }
    }

    fn empty_note(self) -> &'static str {
        match self {
            ListId::Looks => "NO LOOKS FOUND",
            ListId::Layauts => "NO LAYAUTS FOUND",
            ListId::Sounds => "NO SOUND THEMES FOUND",
            // Unreachable while the kinds are built in, and stated
            // anyway: an empty list is a state this type has to have a
            // word for, not a case to leave to whatever draws it.
            ListId::Borders => "NO BORDER KINDS",
            ListId::Backgrounds => "NO BACKGROUND KINDS",
            ListId::Severities => "NO SEVERITY ROLES",
            ListId::Corners => "NO CORNER CUTS",
            ListId::RingStyles => "NO RING STYLES",
            ListId::ScrollModes => "NO SCROLLBAR MODES",
            ListId::ScrollEdges => "NO SCROLLBAR EDGES",
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
    ///
    /// `step` is a QUESTION and not a number, because one family of
    /// sliders cannot answer it in the description: the theme editor's
    /// BASIC tracks step by what the swapchain can actually show, and
    /// the bit depth that says so is a setting the window carries and
    /// the user can change while the editor is open ([`Settings::tone_step`]).
    /// Every other slider answers with a constant ([`step_1`] and its
    /// two siblings), which is the same statement it made before and in
    /// the same place — R7 is intact.
    Slider {
        label: &'static str,
        act: Act,
        unit: Unit,
        range: (u32, u32),
        step: fn(&Settings) -> u32,
        get: fn(&Settings) -> u32,
        set: fn(&mut Settings, u32),
        /// Writes the value to nacelle-desktop.ron.
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
    /// Several buttons in ONE row, centred together, `settings.bar_gap`
    /// apart: the editor's SAVE · SAVE AS · CANCEL. Each plate is as
    /// wide as its own label plus the theme's `button.pad_x`, never
    /// narrower than `button.min_w` — no width of its own is written
    /// here.
    ///
    /// Drawn by `nacelle::object::button` like every other button in
    /// the window; this row only says where each plate goes.
    /// Registration runs left to right, which is the order the acts are
    /// written in.
    Bar { items: &'static [(Text, Act)] },
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
    /// A row that answers false here IS NOT THERE: no height in the flow,
    /// no draw, no grey ghost. Distinct from `enabled` on purpose — a
    /// disabled control says "not now", a hidden one says "not for this
    /// choice". The editor's per-kind sliders are the first users: a
    /// COVERAGE slider next to a SOLID background would be a question
    /// about a thing that does not exist.
    when: fn(&Settings) -> bool,
}

fn always(_: &Settings) -> bool {
    true
}

// The constant answers to `Ctrl::Slider`'s step. Percent tracks step 5
// per press; cell counts, pixels and every 0..100 track step 1; the
// blur's pyramid steps 2. Exactly the numbers the descriptions carried
// as literals before the field became a question — one function each,
// the way [`always`] is the constant answer to `Row::when`.
fn step_1(_: &Settings) -> u32 {
    1
}
fn step_2(_: &Settings) -> u32 {
    2
}
fn step_5(_: &Settings) -> u32 {
    5
}

const fn row(ctrl: Ctrl) -> Row {
    Row { ctrl, after: Gap::Row, enabled: always, when: always }
}

const fn row_after(ctrl: Ctrl, after: Gap) -> Row {
    Row { ctrl, after, enabled: always, when: always }
}

const fn row_when(ctrl: Ctrl, enabled: fn(&Settings) -> bool) -> Row {
    Row { ctrl, after: Gap::Row, enabled, when: always }
}

fn bg_chosen(s: &Settings) -> bool {
    s.current_background.is_some()
}
fn bg_blurs(s: &Settings) -> bool {
    matches!(s.current_background.as_deref(), Some("BLUR") | Some("FROSTED GLASS"))
}
fn bg_frosted(s: &Settings) -> bool {
    s.current_background.as_deref() == Some("FROSTED GLASS")
}
fn bg_solid_or_frosted(s: &Settings) -> bool {
    matches!(s.current_background.as_deref(), Some("SOLID") | Some("FROSTED GLASS"))
}

// The whole-theme sections' conditions, one small function per question so
// the rows and `editor_edits` ask the SAME one: a slider is on screen
// exactly when the set it feeds is in the edit — the iron rule's UI half,
// a control over a set that would not be written is a control that looks
// like it works.

/// OWN HUE on: the surfaces' HUE track exists, and the set writes degrees.
fn surface_own(s: &Settings) -> bool {
    s.surface_own_hue
}
/// A severity role stands in the list: the three sliders pin ITS author.
fn severity_chosen(s: &Settings) -> bool {
    s.current_severity.is_some()
}
/// A corner cut stands: the shape set is in the edit at all.
fn corner_chosen(s: &Settings) -> bool {
    s.current_corner.is_some()
}
/// The ring is on AND its style is known — the model's enabled branch
/// cannot be written without a style word.
fn ring_dressed(s: &Settings) -> bool {
    s.ring_on && s.current_ring_style.is_some()
}
fn ring_dashed(s: &Settings) -> bool {
    ring_dressed(s) && s.current_ring_style.as_deref() == Some("DASHED")
}
fn ring_haloed(s: &Settings) -> bool {
    ring_dressed(s) && s.ring_halo
}
/// Both scrollbar words stand: the bar set is in the edit at all.
fn bar_chosen(s: &Settings) -> bool {
    s.current_scroll_mode.is_some() && s.current_scroll_edge.is_some()
}
/// The declaration reads the fade only while auto_hide is on.
fn bar_fades(s: &Settings) -> bool {
    bar_chosen(s) && s.bar_auto_hide
}
/// Track OFF is the switch alone; the groove's colour is only written on.
fn bar_tracked(s: &Settings) -> bool {
    bar_chosen(s) && s.bar_track
}

/// A row that exists only while `when` holds — see `Row::when`.
const fn row_shown(ctrl: Ctrl, when: fn(&Settings) -> bool) -> Row {
    Row { ctrl, after: Gap::Row, enabled: always, when }
}

/// The corner button a page wears, and what the body does about it.
#[derive(Clone, Copy, PartialEq)]
enum Chrome {
    /// CLOSE, with the body below it — the main view.
    Close,
    /// BACK, with the body below it.
    Back,
}

/// One band of a page. A page is a sequence of bands; a band flows
/// downwards the way rows always have, and inside itself may set its
/// rows in columns beside one another instead of one under the other.
///
/// `Row` and `Ctrl` are untouched by this. What changed is only what
/// the walker walks — which is the whole reason the grammar is worth
/// having: the pages keep describing themselves in the same words.
///
/// The tiled band the first draft carried is deliberately NOT here. The
/// owner replaced the tiled MENU page with a permanent navigation rail
/// (the specification's annex, 2026-08-16), so a `Grid` variant would
/// be a shape nothing can ever build.
#[derive(Clone, Copy)]
enum Zone {
    /// A full-width band: what every page was before there were bands.
    ///
    /// `when` is [`Row::when`]'s own word one level up: a band that
    /// answers false IS NOT THERE — no height in the flow, no drawing,
    /// no place in the chain and nothing in the hit map. It exists
    /// because a MODE is a property of a whole run of rows and not of
    /// any one of them: the theme editor's ADVANCED page is eighty-six
    /// rows that are all there together or not at all, and writing the
    /// same condition on every one of them would be one decision spelt
    /// eighty-six times, with eighty-six chances to spell it wrong.
    Flow { when: fn(&Settings) -> bool, cols: Cols, rows: &'static [Row] },
    /// Columns of EQUAL width beside one another, `settings.col_gap`
    /// between them. Registration and drawing run COLUMN BY COLUMN —
    /// the whole of the first, then the whole of the second — so the
    /// Tab order is the description's and never the geometry's.
    ///
    /// Under `settings.col_min_w` a column is too narrow to hold a
    /// label, a track and a value, and the band FOLDS: the columns run
    /// one after the other down the full width instead, which is the
    /// list the page was before it had columns. Because registration
    /// already ran column by column, the chain does not move a step
    /// when it folds (M4).
    Cols { columns: &'static [ZCol] },
    /// A band pinned to the bottom of the content box. This is where
    /// `Ctrl::pinned()` went: standing still is a property of the BAND
    /// and not of the control, so the same button can flow on one page
    /// and stand under another without a kind of its own.
    Pinned { cols: Cols, rows: &'static [Row] },
}

/// One column of a columned band, with its OWN label/value measurement:
/// the sliders on the left do not inherit the width of the labels on
/// the right ("the widest label IN THE BLOCK", `rhythm.label_col`).
#[derive(Clone, Copy)]
struct ZCol {
    cols: Cols,
    rows: &'static [Row],
}

/// The rows of one band, in the order it registers them: column by
/// column where there are columns.
fn zone_rows(zone: &'static Zone) -> Box<dyn Iterator<Item = &'static Row>> {
    match zone {
        Zone::Flow { rows, .. } | Zone::Pinned { rows, .. } => Box::new(rows.iter()),
        Zone::Cols { columns } => Box::new(columns.iter().flat_map(|c| c.rows.iter())),
    }
}

/// Whether a band stands in this frame at all ([`Zone::Flow`]'s `when`).
/// Asked wherever a page's bands are walked, so the description and the
/// drawing cannot disagree about which page the editor is showing.
fn zone_shown(zone: &'static Zone, s: &Settings) -> bool {
    match zone {
        Zone::Flow { when, .. } => when(s),
        _ => true,
    }
}

/// Every row a page DESCRIBES, standing or not, in registration order.
///
/// The mode-blind reading, and it has exactly one caller: [`slider_of`],
/// which answers "what does this act drive" and must answer it about a
/// declaration rather than about the frame on screen.
fn all_page_rows(page: &'static Page) -> impl Iterator<Item = &'static Row> {
    page.zones.iter().flat_map(zone_rows)
}

/// Every row a page is SHOWING, in registration order — the bands whose
/// `when` says no left out, exactly as the walker leaves them out.
///
/// The window itself never asks: its walkers go through
/// [`Settings::frame_zones`], which has already dropped the bands that
/// are not standing. This is the TESTS' independent reading of the same
/// table — the thing that lets them check the drawing instead of
/// echoing it — so it is built only for them.
#[cfg(test)]
fn page_rows<'a>(
    page: &'static Page,
    s: &'a Settings,
) -> impl Iterator<Item = &'static Row> + 'a {
    page.zones.iter().filter(move |z| zone_shown(z, s)).flat_map(zone_rows)
}

/// The gutter between two columns of a band.
fn col_gap() -> f32 {
    static COL_GAP: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().px(tok(&COL_GAP, "settings.col_gap"))
}

/// The break between two bands. Said once, because the flow's height
/// and the flow's drawing have to place the bands on the same pixel.
fn zone_gap() -> f32 {
    static ZONE_GAP: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().px(tok(&ZONE_GAP, "settings.zone_gap"))
}

/// The narrowest a column may be before its band folds into one list —
/// `settings.col_min_w` with its device-px floor, the pair every other
/// minimum in this theme is written as.
///
/// One reader for the bands and one for the window's own three panels:
/// the page and the columns inside it fold on the same word, which is
/// why "there is no room" means one thing in this window and not two.
///
/// Measured 2026-08-17, and the THEME's to answer, not this file's: at
/// the master's `72u` the threshold scales with the screen, so whether
/// a band stands in columns is very nearly the question of how much of
/// the content box its page HAS. A page standing beside BOTH navigation
/// columns keeps a little over half of it, and its own columns fall
/// short at every height the program is built for — FONT is one list
/// even at 2160 px, by four pixels. A page beside the rail alone
/// (COLOR) stands in columns from 1080 px up. Moving the number is the
/// owner's call and a `libnacelle` commit; nothing here may hard-code
/// around it.
fn col_min_w() -> f32 {
    static MIN_W: OnceLock<TokenId> = OnceLock::new();
    static MIN_W_PX: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    th.px(tok(&MIN_W, "settings.col_min_w"))
        .max(th.px(tok(&MIN_W_PX, "settings.col_min_w_min_px")))
}

/// Whether a band has run out of width and stands as one list (M4).
/// A band with one column never folds: it is already the list.
fn zone_folded(zone: &'static Zone, box_: Rect) -> bool {
    match zone {
        Zone::Cols { columns } if columns.len() > 1 => {
            let n = columns.len() as f32;
            (box_.w - col_gap() * (n - 1.0)) / n < col_min_w()
        }
        _ => false,
    }
}

/// The boxes a band lays its rows in, in registration order: one for a
/// flow, one per column for a columned band. Only x and width differ
/// between them — the y and the height stay the page's, so a row that
/// reserves part of the CONTENT box (`Ctrl::Custom`) measures the same
/// box whichever band it stands in.
///
/// A FOLDED band gives every column the whole box: the columns no
/// longer stand beside one another, so they no longer share the width.
/// Where each one starts down the band is [`Settings::zone_offsets`],
/// which is the only thing that has to know the heights.
///
/// The walker, the height and the tests all ask this one function, so
/// the split is stated once and cannot drift.
fn zone_regions(zone: &'static Zone, box_: Rect) -> Vec<(Rect, Cols, &'static [Row])> {
    match zone {
        Zone::Flow { cols, rows, .. } | Zone::Pinned { cols, rows } => {
            vec![(box_, *cols, *rows)]
        }
        Zone::Cols { columns } => {
            if zone_folded(zone, box_) {
                return columns.iter().map(|c| (box_, c.cols, c.rows)).collect();
            }
            let gap = col_gap();
            let n = columns.len().max(1) as f32;
            let w = ((box_.w - gap * (n - 1.0)) / n).max(0.0);
            columns
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let x = box_.x + i as f32 * (w + gap);
                    (Rect::new(x, box_.y, w, box_.h), c.cols, c.rows)
                })
                .collect()
        }
    }
}

/// One view of the window.
///
/// The corner button is NOT a field: it follows [`chrome_of`], which
/// follows [`parent_view`]. A page that said one thing here and another
/// there is how a BACK button once led to the page it stood on.
struct Page {
    view: View,
    title: &'static str,
    /// The space between the chrome row and the first flowed row.
    lead: Gap,
    zones: &'static [Zone],
}

// ---------------------------------------------------------- the navigation

/// The RAIL: every section of the window, standing under the heading of
/// the group it belongs to, on every page and at all times.
///
/// It is written in the same words a page is — `Row`, `Ctrl`, `Gap` —
/// and drawn by the same walker, so a section is disabled, spaced or
/// hidden by the same three predicates a setting is, and nothing about
/// navigating is a second grammar. What used to be the MENU page is
/// this table: the entries are the same entries, in the same order, and
/// the six of them no longer cost a page of their own.
///
/// The headings are `Ctrl::Section`, which takes its own gap under it
/// (`panel.title.block_h`) — hence `Gap::None` after every one of them,
/// exactly as the FONT page writes its two.
static RAIL_ROWS: [Row; 9] = [
    row_after(Ctrl::Section { title: "APPEARANCE" }, Gap::None),
    row(Ctrl::Button {
        label: Text::Fixed("LOOK AND FEEL"),
        kind: BtnKind::Wide,
        act: Act::OpenLookFeel,
    }),
    // Colour is a conversation with a Wayland compositor; where there
    // is none, the entry is painted shut — visible, not clickable.
    row_when(
        Ctrl::Button {
            label: Text::Fixed("COLOR SPACE"),
            kind: BtnKind::Wide,
            act: Act::OpenColor,
        },
        |s| s.color_enabled,
    ),
    row_after(
        Ctrl::Button {
            label: Text::Fixed("BLUR"),
            kind: BtnKind::Wide,
            act: Act::OpenBlur,
        },
        Gap::Section,
    ),
    row_after(Ctrl::Section { title: "DESKTOP" }, Gap::None),
    row(Ctrl::Button {
        label: Text::Fixed("GRID"),
        kind: BtnKind::Wide,
        act: Act::OpenGrid,
    }),
    row_after(
        Ctrl::Button {
            label: Text::Fixed("BOARDS"),
            kind: BtnKind::Wide,
            act: Act::OpenBoards,
        },
        Gap::Section,
    ),
    row_after(Ctrl::Section { title: "SYSTEM" }, Gap::None),
    // The one section that changes nothing. It is here because the
    // files behind it are edited in a text editor and the program's
    // only other word about them goes to a stderr a desktop session has
    // nowhere to show — so without it a user who mistyped a bracket had
    // a widget on factory values and no way to find out from inside the
    // program.
    row(Ctrl::Button {
        label: Text::Fixed("ADDONS"),
        kind: BtnKind::Wide,
        act: Act::OpenAddons,
    }),
];

/// The second column of LOOK AND FEEL: its pages, in reading order.
///
/// SETS is the section's own page — the three lists that say which
/// installed theme, layout and sound set are in force — and it stands
/// first because it is what the section opens on. FONTS and SOUND
/// LEVELS used to be two buttons ON that page; they are entries here
/// now, which is the whole of what the annex bought: one navigation
/// layer fewer on every path into them.
///
/// The theme editor and the reset confirmation are deliberately NOT
/// here. The editor stands at the head of the THEMES list, where the
/// theme it edits is chosen, and the confirmation is what the pinned
/// footer opens — a destructive control one press from every page of
/// the window is exactly the friction decision §2a exists to keep.
static LOOKFEEL_SUBRAIL_ROWS: [Row; 3] = [
    row(Ctrl::Button {
        label: Text::Fixed("SETS"),
        kind: BtnKind::Wide,
        act: Act::OpenSets,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("FONTS"),
        kind: BtnKind::Wide,
        act: Act::OpenFont,
    }),
    row(Ctrl::Button {
        label: Text::Fixed("SOUND LEVELS"),
        kind: BtnKind::Wide,
        act: Act::OpenSoundLevels,
    }),
];

/// The navigation as BANDS, for the folded window: the same two tables,
/// laid down the one list instead of beside it. Statics because a band
/// is `&'static` everywhere else in this file.
static RAIL_ZONE: Zone = Zone::Flow { when: always, cols: Cols::None, rows: &RAIL_ROWS };
static LOOKFEEL_SUBRAIL_ZONE: Zone =
    Zone::Flow { when: always, cols: Cols::None, rows: &LOOKFEEL_SUBRAIL_ROWS };

/// The second column of a section: its entries, and the BAND those same
/// entries stand in once the window has folded. One table for both,
/// because the two are one column drawn two ways.
///
/// A section with no answer here has no second column at all and its
/// page starts straight after the rail (owner: "sekcje-formularze idą
/// wprost do treści").
fn subrail(view: View) -> Option<(&'static [Row], &'static Zone)> {
    match rail_act(view) {
        Act::OpenLookFeel => Some((&LOOKFEEL_SUBRAIL_ROWS, &LOOKFEEL_SUBRAIL_ZONE)),
        _ => None,
    }
}

/// The pages of a section, or `None` where the section IS its page.
fn subrail_rows(view: View) -> Option<&'static [Row]> {
    subrail(view).map(|(rows, _)| rows)
}

/// The band a section's second column stands in when the window has
/// folded — the same rows, laid down the one list instead of beside it.
fn subrail_zone(view: View) -> Option<&'static Zone> {
    subrail(view).map(|(_, zone)| zone)
}

/// The rail entry a view stands under — its SECTION. Every page of LOOK
/// AND FEEL, however deep, marks the one entry; the other sections are
/// their own page.
fn rail_act(view: View) -> Act {
    match view {
        View::LookFeel
        | View::Font
        | View::SoundLevels
        | View::ThemeEditor
        | View::LookFeelReset => Act::OpenLookFeel,
        View::Grid => Act::OpenGrid,
        View::Boards => Act::OpenBoards,
        View::Color => Act::OpenColor,
        View::Blur => Act::OpenBlur,
        View::Addons => Act::OpenAddons,
    }
}

/// The second column's entry for a view, where the column lists it.
/// The two pages the column does not list ([`LOOKFEEL_SUBRAIL_ROWS`])
/// answer `None`, and nothing in that column is marked while they
/// stand — which is true: neither of them is one of its entries.
fn sub_act(view: View) -> Option<Act> {
    match view {
        View::LookFeel => Some(Act::OpenSets),
        View::Font => Some(Act::OpenFont),
        View::SoundLevels => Some(Act::OpenSoundLevels),
        _ => None,
    }
}

// --------------------------------------------------------------- the pages

/// LOOK AND FEEL (decision §2): the three choices that say which
/// installed set is in force. Three drop-downs rather than three
/// columns, so the page at rest fits whole and at most one list is ever
/// unfolded — three open lists one under the other would be a page you
/// scroll through to reach the sounds.
///
/// The two doors that used to stand under them — SOUND LEVELS and
/// FONTS — are entries of the section's own column now
/// ([`LOOKFEEL_SUBRAIL_ROWS`]). They are the same two pages, reached in
/// one press from anywhere in the section instead of two from the menu,
/// and leaving them here as well would be one subject with two doors
/// standing open beside each other.
///
/// The footer is the page's own undo, and it stands in a pinned BAND
/// rather than flowing, so that the rows above it are the page the
/// decision describes. It opens a confirmation and nothing else: what
/// stands behind it (decision §2a) is every setting this section
/// writes, and one press may not be able to spend all of them.
static LOOKFEEL_ROWS: [Row; 3] = [
    row(Ctrl::Drop { list: ListId::Looks }),
    row(Ctrl::Drop { list: ListId::Layauts }),
    row(Ctrl::Drop { list: ListId::Sounds }),
];

/// The page's undo, in a band of its own. `BtnKind::Footer` used to say
/// BOTH "centred at `settings.list_w_frac`" and "against the bottom
/// edge"; the second half is the band's word now, so the button is an
/// ordinary `Listed` one standing where the band stands.
static LOOKFEEL_FOOTER: [Row; 1] = [row(Ctrl::Button {
    label: Text::Fixed("LOOK AND FEEL RESET"),
    kind: BtnKind::Listed,
    act: Act::LookFeelReset,
})];

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
static LOOKFEEL_RESET_ROWS: [Row; 9] = [
    row_after(Ctrl::Section { title: "WHAT THIS CLEARS" }, Gap::None),
    row(Ctrl::Note { text: Text::Of(clears_theme) }),
    row(Ctrl::Note { text: Text::Of(clears_layaut) }),
    row(Ctrl::Note { text: Text::Of(clears_sounds) }),
    row(Ctrl::Note {
        text: Text::Fixed("FONTS: SIZE, FAMILY AND WEIGHT, TERMINAL AND INTERFACE"),
    }),
    // Typed on the GRID page, cleared here: it overrides the theme's
    // `layout.panel_gutter` and nothing else, so it is part of the look
    // whatever page it is reached from. Named for the same reason as
    // the rest — a reset that took away a spacing the page had not
    // mentioned would be a reset that surprises.
    row(Ctrl::Note { text: Text::Fixed("THE PANEL GUTTER TYPED ON THE GRID PAGE") }),
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
/// Whether the editor is on its BASIC page. The two bands' conditions
/// and nothing else — one question, asked twice, so the two can never
/// both stand or both be missing.
fn editor_basic(s: &Settings) -> bool {
    s.editor_basic
}
fn editor_advanced(s: &Settings) -> bool {
    !s.editor_basic
}

/// The switch at the HEAD of the editor, before every section: which of
/// the two pages is showing.
///
/// A [`Ctrl::Cycle`] and not a [`Ctrl::Toggle`], because with two
/// members "step to the next" IS a toggle — and this one says which
/// mode is in force in the mode's own word, which a switch showing an
/// on/off state could not. The same control COLOR's SPACE, LUT and ICC
/// are, and no new kind for a two-valued button.
static EDITOR_MODE_ROWS: [Row; 1] = [row(Ctrl::Cycle {
    label: "MODE",
    get: |s| if s.editor_basic { "BASIC" } else { "ADVANCED" }.to_string(),
    act: Act::EditorMode,
})];

/// THE WHOLE THEME ON THREE SLIDERS — the editor's BASIC page.
///
/// Three questions, and the reason three of them can move a hundred
/// colours is that they do not move colours at all: the model turns
/// them into edits to the AUTHORS everything else is derived from
/// (`theme::edit::tone_edits` — `palette.accent`, the seven
/// `severity.<role>.text`, `surface.lift` and `text.lift`), and the
/// cascade does what it already does.
///
/// EVERY ONE IS RELATIVE, by the owner's decision. HUE is a rotation,
/// SATURATION a multiplier and LIGHTNESS an offset, so each of them
/// leaves every difference the theme's author wrote exactly where it
/// was. That is what keeps a theme from flattening into one colour, and
/// it is what makes ONE HUE FOR THE INTERFACE and A ROTATION FOR
/// SEVERITY the same mechanism instead of two: the chrome family has
/// one author, so turning it lands surfaces, containers, controls and
/// text on a single shared hue, while the severity family has seven, so
/// the same turn carries all of them and green `ok` stays as far from
/// red `critical` as it was. What tells the families apart afterwards is
/// SHADE, and the shades are the master's own ladders, which this page
/// never touches — the owner's ŻYCZENIE 2b, and it needed no second
/// mechanism.
///
/// The three tracks step by what the pipeline can SHOW
/// ([`Settings::tone_step`]), which is why `step` is a question and not
/// a literal here.
static EDITOR_BASIC_ROWS: [Row; 4] = [
    row_after(Ctrl::Section { title: "THE WHOLE THEME" }, Gap::None),
    row(Ctrl::Slider {
        label: "HUE",
        act: Act::EditorTrack(Knob::ToneHue),
        unit: Unit::None,
        range: (0, TONE_HUE_MAX),
        step: |s| s.tone_step()[0],
        get: |s| s.tone[0],
        set: |s, v| s.tone[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "SATURATION",
        act: Act::EditorTrack(Knob::ToneSat),
        unit: Unit::None,
        range: (0, TONE_SAT_MAX),
        step: |s| s.tone_step()[1],
        get: |s| s.tone[1],
        set: |s, v| s.tone[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "LIGHTNESS",
        act: Act::EditorTrack(Knob::ToneLight),
        unit: Unit::None,
        range: (0, 100),
        step: |s| s.tone_step()[2],
        get: |s| s.tone[2],
        set: |s, v| s.tone[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
];

/// The editor's first section. The border is one kind and one colour, and
/// the colour is three numbers because the theme writes colours as
/// `oklch(L, C, H)` — three sliders is the shape of the value, not a
/// choice about how many controls to offer.
///
/// The halo of NEON has no colour of its own; it wears the line's. So there
/// is one colour here and not two, and the list above it switches a glow
/// on rather than introducing a second thing to tint.
///
/// Since 2026-08-16 the page carries the WHOLE THEME: after the border
/// and the background come one section per set of `theme/edit.rs` —
/// accent, surfaces, text, severity, shape, focus ring, menu, tooltip,
/// scrollbar — in the model's own order, every control live through the
/// same preview pulse and every value landing in the same builder
/// (`editor_edits`) that SAVE writes to a file. A control exists here
/// ONLY for a number the model takes, and the model takes only tokens
/// with a named reader in Rust — the iron rule, held on both sides of
/// the seam.
///
/// Since 2026-08-17 this is the ADVANCED page of two, and it is
/// UNCHANGED by the other one: the switch above it and BASIC's three
/// sliders are bands of their own, and the only thing that happened to
/// these eighty-six rows is that their band now has a condition.
static EDITOR_ROWS: [Row; 86] = [
    row_after(Ctrl::Section { title: "BORDER" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Borders }),
    row(Ctrl::Slider {
        label: "BRIGHTNESS",
        act: Act::EditorTrack(Knob::EdgeL),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.edge[0],
        set: |s, v| s.edge[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "SATURATION",
        act: Act::EditorTrack(Knob::EdgeC),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.edge[1],
        set: |s, v| s.edge[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "HUE",
        act: Act::EditorTrack(Knob::EdgeH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.edge[2],
        set: |s, v| s.edge[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    // BACKGROUND: the kind, then the glass pair — TINT multiplies the
    // blurred scene (it can only darken), WASH lays over with alpha (the
    // only knob that brightens). SOLID reads its colour from the WASH
    // group, the one that behaves like an ordinary colour.
    row_after(Ctrl::Section { title: "BACKGROUND" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Backgrounds }),
    // The kind's own knobs appear WITH the kind: a slider for a thing the
    // choice does not have would be a question about nothing (Row::when).
    row_shown(
        Ctrl::Slider {
            label: "OPACITY",
            act: Act::EditorTrack(Knob::BgOpacity),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bg_opacity,
            set: |s, v| s.bg_opacity = v,
            save: |s| s.apply_editor_preview(),
        },
        bg_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "BLUR DEPTH",
            act: Act::EditorTrack(Knob::BgDepth),
            unit: Unit::None,
            // 0..100 mapped onto the pyramid's 1.0..3.0: the emitter mixes
            // two rungs by the fraction, so every stop is a real depth.
            range: (0, 100),
            step: step_2,
            get: |s| s.bg_depth,
            set: |s, v| s.bg_depth = v,
            save: |s| s.apply_editor_preview(),
        },
        bg_blurs,
    ),
    row_shown(
        Ctrl::Slider {
            label: "WASH COVERAGE",
            act: Act::EditorTrack(Knob::BgCoverage),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bg_coverage,
            set: |s, v| s.bg_coverage = v,
            save: |s| s.apply_editor_preview(),
        },
        bg_frosted,
    ),
    row_shown(Ctrl::Slider {
        label: "TINT BRIGHTNESS",
        act: Act::EditorTrack(Knob::TintB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tint[0],
        set: |s, v| s.tint[0] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_blurs),
    row_shown(Ctrl::Slider {
        label: "TINT SATURATION",
        act: Act::EditorTrack(Knob::TintS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tint[1],
        set: |s, v| s.tint[1] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_blurs),
    row_shown(Ctrl::Slider {
        label: "TINT HUE",
        act: Act::EditorTrack(Knob::TintH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.tint[2],
        set: |s, v| s.tint[2] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_blurs),
    row_shown(Ctrl::Slider {
        label: "WASH BRIGHTNESS",
        act: Act::EditorTrack(Knob::WashB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.wash[0],
        set: |s, v| s.wash[0] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_solid_or_frosted),
    row_shown(Ctrl::Slider {
        label: "WASH SATURATION",
        act: Act::EditorTrack(Knob::WashS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.wash[1],
        set: |s, v| s.wash[1] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_solid_or_frosted),
    row_shown(Ctrl::Slider {
        label: "WASH HUE",
        act: Act::EditorTrack(Knob::WashH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.wash[2],
        set: |s, v| s.wash[2] = v,
        save: |s| s.apply_editor_preview(),
    }, bg_solid_or_frosted),
    // ------------------------------------------------------------------
    // The whole-theme sections (2026-08-16): one section per set of
    // theme/edit.rs, in the model's own order. Every slider's save is
    // the preview pulse, like every editor track above — the SAVE
    // buttons at the bottom write the same builder's answer to a file.
    // ------------------------------------------------------------------
    // ACCENT: the one seed the master re-derives the interface from.
    // Three sliders because a colour is three numbers; no alpha knob,
    // because the model writes the seed opaque by force.
    row_after(Ctrl::Section { title: "ACCENT" }, Gap::None),
    row(Ctrl::Slider {
        label: "BRIGHTNESS",
        act: Act::EditorTrack(Knob::AccentB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.accent[0],
        set: |s, v| s.accent[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "SATURATION",
        act: Act::EditorTrack(Knob::AccentS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.accent[1],
        set: |s, v| s.accent[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "HUE",
        act: Act::EditorTrack(Knob::AccentH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.accent[2],
        set: |s, v| s.accent[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    // SURFACES: the three meta-knobs over the six-level ladder — never
    // eighteen sliders, the rungs are §5.5's. The HUE track appears
    // with OWN HUE, because off it the set restores `@hue.accent` as a
    // reference and a hue slider would be a question about nothing.
    row_after(Ctrl::Section { title: "SURFACES" }, Gap::None),
    row(Ctrl::Toggle {
        label: "OWN HUE",
        get: |s| s.surface_own_hue,
        act: Act::EditorFlip(Flip::SurfaceOwnHue),
    }),
    row_shown(
        Ctrl::Slider {
            label: "HUE",
            act: Act::EditorTrack(Knob::SurfHue),
            unit: Unit::None,
            range: (0, 359),
            step: step_5,
            get: |s| s.surface_hue,
            set: |s, v| s.surface_hue = v,
            save: |s| s.apply_editor_preview(),
        },
        surface_own,
    ),
    row(Ctrl::Slider {
        // 0..100 over the bake's -0.09..0.09, 50 the ladder as derived.
        label: "LIFT",
        act: Act::EditorTrack(Knob::SurfLift),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.surface_lift,
        set: |s, v| s.surface_lift = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        // 0..100 over the bake's 0..4, 25 the derived scale of 1.
        label: "CHROMA",
        act: Act::EditorTrack(Knob::SurfChroma),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.surface_chroma,
        set: |s, v| s.surface_chroma = v,
        save: |s| s.apply_editor_preview(),
    }),
    // TEXT: the two meta-knobs over the seven roles. No per-role colour
    // on purpose — the roles ride the accent's hue and chroma by §5.6.
    row_after(Ctrl::Section { title: "TEXT" }, Gap::None),
    row(Ctrl::Slider {
        label: "LIFT",
        act: Act::EditorTrack(Knob::TextLift),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.text_lift,
        set: |s, v| s.text_lift = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "CHROMA",
        act: Act::EditorTrack(Knob::TextChroma),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.text_chroma,
        set: |s, v| s.text_chroma = v,
        save: |s| s.apply_editor_preview(),
    }),
    // SEVERITY: pick a role, pin its author colour. The sliders write
    // the CHOSEN role and only a touched role joins the edit — six
    // untouched roles keep the theme's own words.
    row_after(Ctrl::Section { title: "SEVERITY" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Severities }),
    row_shown(
        Ctrl::Slider {
            label: "BRIGHTNESS",
            act: Act::EditorTrack(Knob::SevB),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.severity_idx().map_or(0, |i| s.severity[i][0]),
            set: |s, v| s.set_severity(0, v),
            save: |s| s.apply_editor_preview(),
        },
        severity_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "SATURATION",
            act: Act::EditorTrack(Knob::SevS),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.severity_idx().map_or(0, |i| s.severity[i][1]),
            set: |s, v| s.set_severity(1, v),
            save: |s| s.apply_editor_preview(),
        },
        severity_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "HUE",
            act: Act::EditorTrack(Knob::SevH),
            unit: Unit::None,
            range: (0, 359),
            step: step_5,
            get: |s| s.severity_idx().map_or(0, |i| s.severity[i][2]),
            set: |s, v| s.set_severity(2, v),
            save: |s| s.apply_editor_preview(),
        },
        severity_chosen,
    ),
    // SHAPE: the corner language, its three radii, the tessellation and
    // the hairline. The sliders appear with the cut, because the model
    // writes the six as ONE set and a radius without its cut is half a
    // decision — the theme file's own words.
    row_after(Ctrl::Section { title: "SHAPE" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Corners }),
    row_shown(
        Ctrl::Slider {
            // 0..100 over the model's 4u wall, here and for MD and LG.
            label: "CORNER SM",
            act: Act::EditorTrack(Knob::CornerSm),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.corner_sm,
            set: |s, v| s.corner_sm = v,
            save: |s| s.apply_editor_preview(),
        },
        corner_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "CORNER MD",
            act: Act::EditorTrack(Knob::CornerMd),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.corner_md,
            set: |s, v| s.corner_md = v,
            save: |s| s.apply_editor_preview(),
        },
        corner_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "CORNER LG",
            act: Act::EditorTrack(Knob::CornerLg),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.corner_lg,
            set: |s, v| s.corner_lg = v,
            save: |s| s.apply_editor_preview(),
        },
        corner_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            // The declared range itself (3..16): a fraction of a
            // tessellation quad does not exist, so the track is bare.
            label: "SEGMENTS",
            act: Act::EditorTrack(Knob::CornerSeg),
            unit: Unit::None,
            range: (3, 16),
            step: step_1,
            get: |s| s.corner_segments,
            set: |s, v| s.corner_segments = v,
            save: |s| s.apply_editor_preview(),
        },
        corner_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            // 0..100 over the kerf's 1u wall.
            label: "HAIRLINE",
            act: Act::EditorTrack(Knob::Hairline),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.stroke_hair,
            set: |s, v| s.stroke_hair = v,
            save: |s| s.apply_editor_preview(),
        },
        corner_chosen,
    ),
    // FOCUS RING: the switch first — OFF is one flag and the dress
    // stands, so everything under it folds away. DASH and GAP appear
    // with DASHED (SOLID leaves the rhythm alone), the halo's alpha
    // with the halo. The dim is the section's one always-on track: it
    // is read on the window, not on the ring, and works with it off.
    row_after(Ctrl::Section { title: "FOCUS RING" }, Gap::None),
    row(Ctrl::Toggle {
        label: "FOCUS RING",
        get: |s| s.ring_on,
        act: Act::EditorFlip(Flip::Ring),
    }),
    row_shown(Ctrl::Drop { list: ListId::RingStyles }, ring_dressed),
    row_shown(
        Ctrl::Slider {
            // 0..100 over the declared 2u, WIDTH and OFFSET alike.
            label: "WIDTH",
            act: Act::EditorTrack(Knob::RingW),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_width,
            set: |s, v| s.ring_width = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "OFFSET",
            act: Act::EditorTrack(Knob::RingOffset),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_offset,
            set: |s, v| s.ring_offset = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "BRIGHTNESS",
            act: Act::EditorTrack(Knob::RingB),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_colour[0],
            set: |s, v| s.ring_colour[0] = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "SATURATION",
            act: Act::EditorTrack(Knob::RingS),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_colour[1],
            set: |s, v| s.ring_colour[1] = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "HUE",
            act: Act::EditorTrack(Knob::RingH),
            unit: Unit::None,
            range: (0, 359),
            step: step_5,
            get: |s| s.ring_colour[2],
            set: |s, v| s.ring_colour[2] = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            // 0..100 over 4u of rhythm, DASH and GAP alike — the model
            // only floors these at zero.
            label: "DASH",
            act: Act::EditorTrack(Knob::RingDash),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_dash,
            set: |s, v| s.ring_dash = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dashed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "GAP",
            act: Act::EditorTrack(Knob::RingGap),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_gap,
            set: |s, v| s.ring_gap = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_dashed,
    ),
    row_shown(
        Ctrl::Toggle {
            label: "HALO",
            get: |s| s.ring_halo,
            act: Act::EditorFlip(Flip::Halo),
        },
        ring_dressed,
    ),
    row_shown(
        Ctrl::Slider {
            label: "HALO ALPHA",
            act: Act::EditorTrack(Knob::HaloAlpha),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.ring_halo_alpha,
            set: |s, v| s.ring_halo_alpha = v,
            save: |s| s.apply_editor_preview(),
        },
        ring_haloed,
    ),
    row(Ctrl::Slider {
        // The declared floor is the track's own start: dimming an
        // unfocused window must not hide it (30 = the model's 0.3).
        label: "UNFOCUSED DIM",
        act: Act::EditorTrack(Knob::UnfocusedDim),
        unit: Unit::None,
        range: (30, 100),
        step: step_1,
        get: |s| s.unfocused_dim,
        set: |s, v| s.unfocused_dim = v,
        save: |s| s.apply_editor_preview(),
    }),
    // MENU: the four tokens menu.rs and winframe.rs read — bed, ring,
    // ring width, hint ink. The colours' alphas are the SEED's and stay
    // with it: there is no opacity knob here to own the channel.
    row_after(Ctrl::Section { title: "MENU" }, Gap::None),
    row(Ctrl::Slider {
        label: "FILL BRIGHTNESS",
        act: Act::EditorTrack(Knob::MenuFillB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_fill[0],
        set: |s, v| s.menu_fill[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "FILL SATURATION",
        act: Act::EditorTrack(Knob::MenuFillS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_fill[1],
        set: |s, v| s.menu_fill[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "FILL HUE",
        act: Act::EditorTrack(Knob::MenuFillH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.menu_fill[2],
        set: |s, v| s.menu_fill[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "BORDER BRIGHTNESS",
        act: Act::EditorTrack(Knob::MenuEdgeB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_edge[0],
        set: |s, v| s.menu_edge[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "BORDER SATURATION",
        act: Act::EditorTrack(Knob::MenuEdgeS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_edge[1],
        set: |s, v| s.menu_edge[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "BORDER HUE",
        act: Act::EditorTrack(Knob::MenuEdgeH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.menu_edge[2],
        set: |s, v| s.menu_edge[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        // 0..100 over 1u; zero is a legal answer — no ring at all,
        // menu.rs's own floor.
        label: "BORDER WIDTH",
        act: Act::EditorTrack(Knob::MenuEdgeW),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_edge_w,
        set: |s, v| s.menu_edge_w = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "HINT BRIGHTNESS",
        act: Act::EditorTrack(Knob::MenuHintB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_hint[0],
        set: |s, v| s.menu_hint[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "HINT SATURATION",
        act: Act::EditorTrack(Knob::MenuHintS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.menu_hint[1],
        set: |s, v| s.menu_hint[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "HINT HUE",
        act: Act::EditorTrack(Knob::MenuHintH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.menu_hint[2],
        set: |s, v| s.menu_hint[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    // TOOLTIP: the menu's sibling float, the four tokens tooltip.rs
    // reads, the same arrangement row for row.
    row_after(Ctrl::Section { title: "TOOLTIP" }, Gap::None),
    row(Ctrl::Slider {
        label: "FILL BRIGHTNESS",
        act: Act::EditorTrack(Knob::TipFillB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_fill[0],
        set: |s, v| s.tip_fill[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "FILL SATURATION",
        act: Act::EditorTrack(Knob::TipFillS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_fill[1],
        set: |s, v| s.tip_fill[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "FILL HUE",
        act: Act::EditorTrack(Knob::TipFillH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.tip_fill[2],
        set: |s, v| s.tip_fill[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "EDGE BRIGHTNESS",
        act: Act::EditorTrack(Knob::TipEdgeB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_edge[0],
        set: |s, v| s.tip_edge[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "EDGE SATURATION",
        act: Act::EditorTrack(Knob::TipEdgeS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_edge[1],
        set: |s, v| s.tip_edge[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "EDGE HUE",
        act: Act::EditorTrack(Knob::TipEdgeH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.tip_edge[2],
        set: |s, v| s.tip_edge[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "EDGE WIDTH",
        act: Act::EditorTrack(Knob::TipEdgeW),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_edge_w,
        set: |s, v| s.tip_edge_w = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "TEXT BRIGHTNESS",
        act: Act::EditorTrack(Knob::TipTextB),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_text[0],
        set: |s, v| s.tip_text[0] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "TEXT SATURATION",
        act: Act::EditorTrack(Knob::TipTextS),
        unit: Unit::None,
        range: (0, 100),
        step: step_1,
        get: |s| s.tip_text[1],
        set: |s, v| s.tip_text[1] = v,
        save: |s| s.apply_editor_preview(),
    }),
    row(Ctrl::Slider {
        label: "TEXT HUE",
        act: Act::EditorTrack(Knob::TipTextH),
        unit: Unit::None,
        range: (0, 359),
        step: step_5,
        get: |s| s.tip_text[2],
        set: |s, v| s.tip_text[2] = v,
        save: |s| s.apply_editor_preview(),
    }),
    // SCROLLBAR: the two words, the two widths, and the two switches
    // with the rows each one alone makes real — the fade is read only
    // while the bar auto-hides, the groove's colour only while the
    // groove is drawn.
    row_after(Ctrl::Section { title: "SCROLLBAR" }, Gap::None),
    row(Ctrl::Drop { list: ListId::ScrollModes }),
    row(Ctrl::Drop { list: ListId::ScrollEdges }),
    row_shown(
        Ctrl::Slider {
            // 0..100 over the model's 0.5u..4u, WIDTH and HOVER alike.
            label: "WIDTH",
            act: Act::EditorTrack(Knob::BarW),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bar_w,
            set: |s, v| s.bar_w = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "HOVER WIDTH",
            act: Act::EditorTrack(Knob::BarWHover),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bar_w_hover,
            set: |s, v| s.bar_w_hover = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_chosen,
    ),
    row_shown(
        Ctrl::Toggle {
            label: "AUTO HIDE",
            get: |s| s.bar_auto_hide,
            act: Act::EditorFlip(Flip::BarAutoHide),
        },
        bar_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            // 0..100 over the declared 0..2000ms.
            label: "FADE",
            act: Act::EditorTrack(Knob::BarFade),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bar_fade,
            set: |s, v| s.bar_fade = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_fades,
    ),
    row_shown(
        Ctrl::Toggle {
            label: "TRACK",
            get: |s| s.bar_track,
            act: Act::EditorFlip(Flip::BarTrack),
        },
        bar_chosen,
    ),
    row_shown(
        Ctrl::Slider {
            label: "TRACK BRIGHTNESS",
            act: Act::EditorTrack(Knob::BarTrackB),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bar_track_colour[0],
            set: |s, v| s.bar_track_colour[0] = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_tracked,
    ),
    row_shown(
        Ctrl::Slider {
            label: "TRACK SATURATION",
            act: Act::EditorTrack(Knob::BarTrackS),
            unit: Unit::None,
            range: (0, 100),
            step: step_1,
            get: |s| s.bar_track_colour[1],
            set: |s, v| s.bar_track_colour[1] = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_tracked,
    ),
    row_shown(
        Ctrl::Slider {
            label: "TRACK HUE",
            act: Act::EditorTrack(Knob::BarTrackH),
            unit: Unit::None,
            range: (0, 359),
            step: step_5,
            get: |s| s.bar_track_colour[2],
            set: |s, v| s.bar_track_colour[2] = v,
            save: |s| s.apply_editor_preview(),
        },
        bar_tracked,
    ),
];

/// The editor's three verbs. They were three centred rows at the far
/// end of a page that is many viewports long, so reaching CANCEL meant
/// scrolling past every slider in the theme; they are one pinned row
/// now, which is what the pinned band and [`Ctrl::Bar`] exist for.
///
/// The order is the order they are written in, and it is also the Tab
/// order: SAVE first because it is what the page is for, CANCEL last
/// because it is the way out.
static EDITOR_BAR_ITEMS: [(Text, Act); 3] = [
    (Text::Fixed("SAVE"), Act::EditorSave),
    (Text::Fixed("SAVE AS"), Act::EditorSaveAs),
    (Text::Fixed("CANCEL"), Act::EditorCancel),
];

static EDITOR_BAR: [Row; 1] = [row(Ctrl::Bar { items: &EDITOR_BAR_ITEMS })];

/// The FONT view's two sections, one per column (§3). They are the same
/// three questions asked twice, so they are the case columns were made
/// for: side by side the answer to "how big is the terminal" stands
/// beside "how big is everything else", which is the comparison the
/// page exists to let a reader make.
///
/// The section header takes no gap under it: the size slider sits
/// directly against the separator.
static FONT_TERM_ROWS: [Row; 4] = [
    row_after(Ctrl::Section { title: "TERMINAL" }, Gap::None),
    row(Ctrl::Slider {
        label: "SIZE",
        act: Act::SizeTrack(Sect::Term),
        unit: Unit::Tight,
        range: (50, 200),
        step: step_5,
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
];

static FONT_UI_ROWS: [Row; 4] = [
    row_after(Ctrl::Section { title: "INTERFACE" }, Gap::None),
    // The interface starts at 30% so a big screen can have a small
    // interface — 75% was as low as it went, which on a 4K panel was
    // still larger than anyone wanted.
    row(Ctrl::Slider {
        label: "SIZE",
        act: Act::SizeTrack(Sect::Ui),
        unit: Unit::Tight,
        range: (30, 125),
        step: step_5,
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
        step: step_1,
        get: |s| s.grid_cols,
        set: |s, v| s.grid_cols = v,
        save: |s| config::set_grid_cols(s.grid_cols),
    }),
    row(Ctrl::Slider {
        label: "ROWS",
        act: Act::RowsTrack,
        unit: Unit::None,
        range: (GRID_MIN, GRID_MAX),
        step: step_1,
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
            step: step_1,
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
        step: step_5,
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

static BOARDS_ROWS: [Row; 1] =
    [row(Ctrl::Custom { h: boards_h, draw: Settings::draw_boards })];

/// The gesture the cross does not say by itself, under it. BOARDS was
/// already a mixed layout — a reserve plus a line held to the bottom
/// edge — and it is the page the rest of the window is now shaped like,
/// not the other way round.
static BOARDS_HINT: [Row; 1] = [row(Ctrl::Hint {
    text: Text::Fixed("HOLD THE LEFT BUTTON AND DRAG TO SWITCH BOARDS"),
})];

/// What the swapchain is asked for: its depth and the space it is asked
/// in. The two are one question and stand together (§2 of the screen
/// decision — DEPTH and SPACE are not to be separated), which is why
/// they are one column and not two rows of a wider one.
static COLOR_SWAPCHAIN_ROWS: [Row; 2] = [
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
];

/// The files that grade what the swapchain produced, and where they are
/// read from. A different subject from the column beside it: those are
/// numbers asked of the compositor, these are files on disk.
static COLOR_FILE_ROWS: [Row; 3] = [
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
        step: step_5,
        get: |s| s.blur_radius,
        set: |s, v| s.blur_radius = v,
        save: |s| config::set_blur_radius(s.blur_radius),
    }),
    row(Ctrl::Slider {
        label: "OPACITY",
        act: Act::BlurOpacityTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: step_5,
        get: |s| s.blur_opacity,
        set: |s, v| s.blur_opacity = v,
        save: |s| config::set_blur_opacity(s.blur_opacity),
    }),
];

/// ADDONS: where an addon's own settings go, and every file the program
/// could not use.
///
/// A page that reads and never writes, which is the whole of the
/// promise `nacelle-addons/README.md` makes about these files — that a
/// file which does not load is reported on stderr AND here, because a
/// desktop session has nowhere to show a stderr. It edits nothing: the
/// files are the user's, written by hand, and a window that offered to
/// save over one would need the toolkit's `store` and a reason.
///
/// Two rows and no more. The first says where the files live, which is
/// the question somebody who has never written one has; the second is
/// the report, and it takes the rest of the page because how many lines
/// it has is not this file's to guess.
static ADDONS_ROWS: [Row; 2] = [
    row_after(Ctrl::Note { text: Text::Of(addon_settings_where) }, Gap::Section),
    row(Ctrl::Custom { h: addon_report_h, draw: Settings::draw_addon_report }),
];

/// The one line that answers "where do I put it?" — the directory the
/// program would itself write to, so what is named is what is read
/// first.
fn addon_settings_where(_: &Settings) -> String {
    format!("{}/<addon>.ron", config::addon_settings_dir().display())
}

/// The report takes what the row above it left of the content box.
/// Stated here rather than measured, because [`Ctrl::Custom`] is asked
/// for its height before anything is drawn — and it is the page's own
/// arithmetic: the chrome row, the lead, the note and the break under
/// it.
fn addon_report_h(m: Metrics, content: Rect) -> f32 {
    (content.h - m.btn_h - m.gap - m.note_h - m.section_gap).max(0.0)
}

/// What the page says when the toolkit has nothing to complain about.
/// It is a whole answer rather than an empty page: "no news" and "this
/// window is not looking" read identically on a blank surface, and the
/// second is the state this page exists because of.
const ADDONS_ALL_CLEAR: &str = "EVERY ADDON SETTINGS FILE ON THIS MACHINE LOADS";

/// The report the page draws, from the two questions the toolkit
/// answers with different things.
///
/// A pure function of those two answers so that what the page SAYS can
/// be tested without a settings directory anywhere near the machine
/// running the tests — the roots are process-wide, and a test that
/// installed its own would be a test that decides what another one
/// reads.
///
/// The path leads and the message follows it, indented, because the
/// path is the part the user acts on: it is what they open in an
/// editor, and it must be readable straight off the screen. Neither is
/// shouted into the window's capitals — a path is case-sensitive, and a
/// position in a file is not a heading.
fn addon_report(installed: bool, problems: &[nacelle::settings::Problem]) -> Vec<String> {
    if !installed {
        // The larger failure, and the one nothing else can see: with no
        // directories installed EVERY read is refused, so there are no
        // bad files to list and every file on the machine is ignored.
        return vec![
            "NO SETTINGS DIRECTORIES ARE INSTALLED \u{2014} EVERY ADDON IS RUNNING ON \
             THE VALUES BUILT INTO IT, AND EVERY SETTINGS FILE ON THIS MACHINE IS \
             BEING IGNORED"
                .to_string(),
        ];
    }
    let mut out = Vec::new();
    for p in problems {
        out.push(p.path.display().to_string());
        out.push(format!("    {}", p.message));
    }
    out
}

// The bands of each page. A page of a single `Flow` band lays its rows
// out with the arithmetic it always had; the pages that hold something
// against the bottom edge carry a second, pinned band; and the two that
// have two subjects of their own — the fonts of two sections, the
// colour of the swapchain against the files that grade it — set them
// side by side and fold back to the one list where the width runs out.

static LOOKFEEL_ZONES: [Zone; 2] = [
    Zone::Flow { when: always, cols: Cols::None, rows: &LOOKFEEL_ROWS },
    Zone::Pinned { cols: Cols::None, rows: &LOOKFEEL_FOOTER },
];

static LOOKFEEL_RESET_ZONES: [Zone; 1] =
    [Zone::Flow { when: always, cols: Cols::None, rows: &LOOKFEEL_RESET_ROWS }];

/// The editor is a switch, ONE of two pages, and a pinned bar.
///
/// §3 drew it as BORDER beside BACKGROUND, and that was true of the page
/// it described: two lists and a handful of sliders. The page has nine
/// whole-theme sections now, and where its columns should be cut is not
/// something the description can answer — the sections are `Ctrl::Section`
/// rows in a run, not a declared grouping a band could take apart. The
/// specification's own answer for the grown editor is a rail of
/// categories (its step 3, `settings.rail_w_frac`), which is a different
/// mechanism from a columned band and a different sitting.
///
/// FOUR BANDS AND THE MODE IS THE MIDDLE TWO. The switch stands first,
/// on every frame, because the owner asked for it "na samej górze
/// strony"; then exactly one of the two pages, by its band's own `when`;
/// then the bar, which belongs to BOTH — SAVE, SAVE AS and CANCEL write
/// and drop the same edit set whichever page made it.
///
/// The two page bands are mutually exclusive by construction
/// ([`editor_basic`] and [`editor_advanced`] are one question and its
/// negation), so the flow can never carry both and can never carry
/// neither.
static EDITOR_ZONES: [Zone; 4] = [
    Zone::Flow { when: always, cols: Cols::None, rows: &EDITOR_MODE_ROWS },
    Zone::Flow { when: editor_basic, cols: Cols::None, rows: &EDITOR_BASIC_ROWS },
    Zone::Flow { when: editor_advanced, cols: Cols::None, rows: &EDITOR_ROWS },
    Zone::Pinned { cols: Cols::None, rows: &EDITOR_BAR },
];

/// Two symmetrical columns, each measuring its OWN "SIZE" against its
/// own "200%": the two tracks are the same length because the two
/// columns are, not because one inherited the other's label width.
static FONT_COLUMNS: [ZCol; 2] = [
    ZCol {
        cols: Cols::Measured { label: "SIZE", value: "200%" },
        rows: &FONT_TERM_ROWS,
    },
    ZCol {
        cols: Cols::Measured { label: "SIZE", value: "200%" },
        rows: &FONT_UI_ROWS,
    },
];

static FONT_ZONES: [Zone; 1] = [Zone::Cols { columns: &FONT_COLUMNS }];

static GRID_ZONES: [Zone; 1] = [Zone::Flow {
    when: always,
    // Measured against the widest of the three labels rather than each
    // one's own, so all three tracks line up.
    cols: Cols::Measured { label: "COLUMNS", value: "100 PX" },
    rows: &GRID_ROWS,
}];

static SOUND_ZONES: [Zone; 1] = [Zone::Flow {
    when: always,
    cols: Cols::Measured { label: "VOLUME", value: "100 %" },
    rows: &SOUND_ROWS,
}];

static BOARDS_ZONES: [Zone; 2] = [
    Zone::Flow { when: always, cols: Cols::None, rows: &BOARDS_ROWS },
    Zone::Pinned { cols: Cols::None, rows: &BOARDS_HINT },
];

static COLOR_COLUMNS: [ZCol; 2] = [
    ZCol { cols: Cols::Frac, rows: &COLOR_SWAPCHAIN_ROWS },
    ZCol { cols: Cols::Frac, rows: &COLOR_FILE_ROWS },
];

static COLOR_ZONES: [Zone; 1] = [Zone::Cols { columns: &COLOR_COLUMNS }];

static BLUR_ZONES: [Zone; 1] = [Zone::Flow {
    when: always,
    cols: Cols::Measured { label: "OPACITY", value: "100 %" },
    rows: &BLUR_ROWS,
}];

static ADDONS_ZONES: [Zone; 1] = [Zone::Flow { when: always, cols: Cols::None, rows: &ADDONS_ROWS }];

/// The whole window. Indexed by [`View`], which `pages_are_in_view_order`
/// keeps true.
///
/// The corner button is not written here at all: it follows
/// [`parent_view`] through [`chrome_of`]. A page the navigation reaches
/// wears CLOSE, because there is nowhere to go back to that the rail is
/// not already showing, and the two pages the navigation does not list
/// wear BACK.
static PAGES: [Page; 10] = [
    Page {
        view: View::LookFeel,
        title: "SETTINGS \u{2014} LOOK AND FEEL",
        lead: Gap::Row,
        zones: &LOOKFEEL_ZONES,
    },
    Page {
        view: View::LookFeelReset,
        title: "SETTINGS \u{2014} LOOK AND FEEL RESET",
        lead: Gap::Section,
        zones: &LOOKFEEL_RESET_ZONES,
    },
    Page {
        view: View::ThemeEditor,
        title: "SETTINGS \u{2014} THEMES EDITOR",
        lead: Gap::Section,
        zones: &EDITOR_ZONES,
    },
    Page {
        view: View::Font,
        title: "SETTINGS \u{2014} FONT",
        lead: Gap::Row,
        zones: &FONT_ZONES,
    },
    Page {
        view: View::Grid,
        title: "SETTINGS \u{2014} GRID",
        lead: Gap::Section,
        zones: &GRID_ZONES,
    },
    Page {
        view: View::SoundLevels,
        title: "SETTINGS \u{2014} SOUND LEVELS",
        lead: Gap::Section,
        zones: &SOUND_ZONES,
    },
    Page {
        view: View::Boards,
        title: "SETTINGS \u{2014} BOARDS",
        lead: Gap::Section,
        zones: &BOARDS_ZONES,
    },
    Page {
        view: View::Color,
        title: "SETTINGS \u{2014} COLOR",
        lead: Gap::Section,
        zones: &COLOR_ZONES,
    },
    Page {
        view: View::Blur,
        title: "SETTINGS \u{2014} BLUR",
        lead: Gap::Section,
        zones: &BLUR_ZONES,
    },
    // `Gap::Row` and not `Gap::Section`: `addon_report_h` counts this
    // lead, and the two have to be the same decision or the report
    // stands one break below where the page reserved room for it.
    Page {
        view: View::Addons,
        title: "SETTINGS \u{2014} ADDONS",
        lead: Gap::Row,
        zones: &ADDONS_ZONES,
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
    hint_inset: f32,
    corner_w: f32,
    /// `settings.list_w_frac` itself, NOT a width. A listed button is a
    /// fraction of the REGION it stands in — and since the window grew
    /// its rail and sub-rail, the region is no longer the whole content
    /// box. Resolving it here against `content.w` made every listed
    /// button 60% of the WINDOW inside a panel roughly half that wide,
    /// so the plate hung out of its column and off the window.
    list_frac: f32,
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
            hint_inset: th.px(tok(&HINT_INSET, "settings.hint_inset")),
            corner_w: (content.w * th.px(tok(&BACK_W_FRAC, "settings.back_w_frac")))
                .max(th.px(tok(&BACK_W_MIN, "settings.back_w_min")))
                .max(th.px(tok(&BACK_W_MIN_PX, "settings.back_w_min_min_px"))),
            list_frac: th.px(tok(&LIST_W_FRAC, "settings.list_w_frac")),
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

/// One navigation column's width: a fraction of the content box, never
/// under the theme's own minimum and never under its device-px floor —
/// the three-part rule every width in this window is written with.
///
/// The rail and the second column ask two tokens, and the master gives
/// the second the first's own value (`settings.subrail_w_frac =
/// @settings.rail_w_frac`), which is the owner's decision that the two
/// are equal said WHERE such a thing is said. Nothing here knows they
/// are equal; if a theme parts them, they part.
fn nav_w(content: Rect, cell: &'static OnceLock<TokenId>, frac: &'static str) -> f32 {
    static MIN: OnceLock<TokenId> = OnceLock::new();
    static MIN_PX: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    (content.w * th.px(tok(cell, frac)))
        .max(th.px(tok(&MIN, "settings.rail_w_min")))
        .max(th.px(tok(&MIN_PX, "settings.rail_w_min_min_px")))
}

/// Where the window's three panels stand this frame.
///
/// The rail hangs one ordinary row gap under the corner button — a
/// FIXED lead, not the page's, so the sections do not step up and down
/// as the pages behind them change what they lead with. The second
/// column stands beside it when the section has pages of its own, and
/// what is left over is the page.
///
/// FOLDED is the whole window's word, not one section's: the room is
/// measured against BOTH columns whether the section shows the second
/// one or not, so moving between sections cannot re-shape the window
/// under the reader's hand. Below the threshold there are no columns at
/// all — the navigation goes into the flow as bands ahead of the page,
/// and the window is the one vertical list it has always been able to
/// fall back to.
#[derive(Clone, Copy)]
struct Panes {
    rail: Option<Rect>,
    sub: Option<Rect>,
    /// What the page has: the whole content box when folded.
    page: Rect,
    folded: bool,
}

impl Panes {
    fn of(view: View, m: Metrics, content: Rect) -> Panes {
        static RAIL_FRAC: OnceLock<TokenId> = OnceLock::new();
        static SUB_FRAC: OnceLock<TokenId> = OnceLock::new();
        let gap = col_gap();
        let rail_w = nav_w(content, &RAIL_FRAC, "settings.rail_w_frac");
        let sub_w = nav_w(content, &SUB_FRAC, "settings.subrail_w_frac");
        // Both columns, always: the fold is the WINDOW's shape.
        let folded = content.w - rail_w - sub_w - 2.0 * gap < col_min_w();
        if folded {
            return Panes { rail: None, sub: None, page: content, folded: true };
        }
        let top = content.y + m.btn_h + m.gap;
        let h = (content.bottom() - top).max(0.0);
        let rail = Rect::new(content.x, top, rail_w, h);
        let mut x = rail.right() + gap;
        let sub = subrail_rows(view).map(|_| {
            let r = Rect::new(x, top, sub_w, h);
            x = r.right() + gap;
            r
        });
        Panes {
            rail: Some(rail),
            sub,
            page: Rect::new(x, content.y, (content.right() - x).max(0.0), content.h),
            folded: false,
        }
    }
}

/// The box a page's ROWS really stand in: the page's own box less the
/// lane the scrollbar keeps beside them.
///
/// An inset bar takes its lane OUT of the rows' box, so it stands
/// BESIDE the controls instead of over them — the owner's ask, and the
/// master's `scrollbar.mode` decision. The lane is reserved at the
/// bar's WIDEST (hover included): a lane that appeared only while
/// scrolling would reflow every row under the pointer.
///
/// Asked by the MEASUREMENT as well as by the drawing. A band folds on
/// its WIDTH (M4), so a page measured at one width and drawn at another
/// would not even be the same height — the scroll would stop short of a
/// list it had itself made longer. The clip, the scroll's span and the
/// bar go on measuring the FULL box: only the rows are narrowed, and
/// the bar has to be drawn against the edge the lane was cut from.
fn rows_box(page_box: Rect) -> Rect {
    let look = ScrollbarLook::from_theme();
    let lane = scroll::inset_w(&look).max(match look.mode {
        scroll::ScrollbarMode::Inset => look.w_hover + 2.0 * look.margin,
        _ => 0.0,
    });
    let w = (page_box.w - lane).max(0.0);
    match look.edge {
        scroll::ScrollbarEdge::Right => Rect::new(page_box.x, page_box.y, w, page_box.h),
        scroll::ScrollbarEdge::Left => {
            Rect::new(page_box.x + lane, page_box.y, w, page_box.h)
        }
    }
}

/// What the last frame made of the flow: the box the flowed bands stood
/// in, how long they were together, and the offset they were drawn at.
///
/// The keyboard happens BETWEEN frames. A page key has to know how far
/// the page goes before it can move it, and the chase
/// ([`Settings::chase_focus`]) has to know both where the frame stood
/// and which offset the rects it is reading were laid out at — the chain
/// answers about the last COMPLETED frame, so a chase measured against
/// an offset that has moved since would chase twice.
#[derive(Clone, Copy)]
struct Flow {
    view: Rect,
    length: f32,
    offset: f32,
}

pub struct Settings {
    pub open: bool,
    view: View,
    /// The engine's theme names, for the THEMES list.
    themes: Vec<String>,
    layauts: Vec<String>,
    sounds: Vec<String>,
    /// Current selections from nacelle-desktop.ron (highlighted in the lists).
    current_look: Option<String>,
    current_layaut: Option<String>,
    current_sounds: Option<String>,
    /// Font view state, indexed by section (0 = Term, 1 = Ui).
    families: [Vec<String>; 2],
    cur_family: [Option<String>; 2],
    cur_weight: [Option<String>; 2],
    /// Font sizes in percent (50-200).
    cur_size: [u32; 2],
    /// The two shapes a border can take, as the list offers them. Built in
    /// rather than read from anywhere: they are not files, they are the
    /// only two things the renderer can draw for a border.
    border_kinds: Vec<String>,
    current_border: Option<String>,
    background_kinds: Vec<String>,
    current_background: Option<String>,
    /// Glass tint colour, HSV in whole slider units, like `edge`.
    tint: [u32; 3],
    /// Glass wash colour, HSV in whole slider units, like `edge`.
    wash: [u32; 3],
    /// Effect opacity in percent, every background kind.
    bg_opacity: u32,
    /// Blur pyramid depth, 1..=3.
    bg_depth: u32,
    /// Wash coverage in percent, FROSTED only.
    bg_coverage: u32,
    /// The SAVE AS prompt, when it is open — the same `InputModel` the
    /// layout editor names its files with, driven here purely by the
    /// keyboard: the field is focused on open, Enter saves, Esc closes.
    naming: Option<nacelle::object::text_input::InputModel>,
    /// When the editor last re-baked the desktop during a drag; the pulse
    /// that keeps a live slider from leaking a bake per frame.
    editor_pulse: Option<Instant>,
    /// Which of the editor's two pages is showing: BASIC's three
    /// sliders, or the ADVANCED page that has always been here. The
    /// window keeps BOTH pages' state at all times — that is the whole
    /// of "switching modes loses no work", and the reason this is one
    /// bool beside the rest of the editor rather than two editors.
    editor_basic: bool,
    /// BASIC's three sliders in TRACK units, indexed HUE, SATURATION,
    /// LIGHTNESS. [`TONE_REST`] is the theme untouched.
    tone: [u32; 3],
    /// What BASIC's relative move is relative TO: the theme's own
    /// authors, read off the live bake when the page was seeded. `None`
    /// until it has been — an unseeded BASIC writes nothing, the same
    /// neutrality `current_border`'s `None` earned.
    tone_seeds: Option<nacelle::theme::edit::ToneSeeds>,
    /// The border colour the editor is showing, as OKLCh in whole units:
    /// lightness 0..100, chroma 0..40, hue 0..359. A slider moves whole
    /// numbers, and the theme's own colours are written to four decimal
    /// places, so these are scaled back on the way out.
    edge: [u32; 3],
    // ---- the whole-theme sections' state (2026-08-16). Every numeric
    // ---- field is in SLIDER units (whole numbers on the track's own
    // ---- range); the maps to the model's units live in editor_edits
    // ---- and seed_editor_from_theme, one each way.
    /// The accent seed, HSV like `edge`.
    accent: [u32; 3],
    /// Whether the surfaces' hue is their own number (ON) or the
    /// restored reference `@hue.accent` (OFF).
    surface_own_hue: bool,
    /// The own hue in degrees, only meaningful while the switch is on.
    surface_hue: u32,
    /// 0..100 over the bake's -0.09..0.09 and 0..4 respectively.
    surface_lift: u32,
    surface_chroma: u32,
    /// 0..100 over the bake's -0.10..0.10 and 0..3.
    text_lift: u32,
    text_chroma: u32,
    /// The severity list's names ([`SEVERITY_ROLES`]' first column).
    severity_kinds: Vec<String>,
    current_severity: Option<String>,
    /// One HSV per role, all seeded from the theme — switching roles
    /// must not lose an edit already made.
    severity: [[u32; 3]; 7],
    /// Which roles a slider actually moved: only these are written.
    /// A seeded-but-untouched role keeps the theme's own words, spelling
    /// included — a reference author survives the editor untouched.
    severity_touched: [bool; 7],
    corner_kinds: Vec<String>,
    current_corner: Option<String>,
    /// 0..100 over 4u (radii), 3..16 bare (segments), 0..100 over 1u.
    corner_sm: u32,
    corner_md: u32,
    corner_lg: u32,
    corner_segments: u32,
    stroke_hair: u32,
    ring_style_kinds: Vec<String>,
    current_ring_style: Option<String>,
    ring_on: bool,
    /// 0..100 over 2u each.
    ring_width: u32,
    ring_offset: u32,
    ring_colour: [u32; 3],
    /// 0..100 over 4u each.
    ring_dash: u32,
    ring_gap: u32,
    ring_halo: bool,
    /// 0..100 over 0..1.
    ring_halo_alpha: u32,
    /// 30..100 over the declared 0.3..1.0.
    unfocused_dim: u32,
    /// The menu's and tooltip's colours, HSV plus the SEED's alpha kept
    /// beside it: the model passes a colour's alpha through, the three
    /// sliders have no say over the channel, and flattening a
    /// translucent bed to opaque just by saving would be an edit nobody
    /// made.
    menu_fill: [u32; 3],
    menu_fill_a: f32,
    menu_edge: [u32; 3],
    menu_edge_a: f32,
    /// 0..100 over 1u.
    menu_edge_w: u32,
    menu_hint: [u32; 3],
    menu_hint_a: f32,
    tip_fill: [u32; 3],
    tip_fill_a: f32,
    tip_edge: [u32; 3],
    tip_edge_a: f32,
    tip_edge_w: u32,
    tip_text: [u32; 3],
    tip_text_a: f32,
    scroll_mode_kinds: Vec<String>,
    current_scroll_mode: Option<String>,
    scroll_edge_kinds: Vec<String>,
    current_scroll_edge: Option<String>,
    /// 0..100 over the model's 0.5u..4u walls.
    bar_w: u32,
    bar_w_hover: u32,
    bar_auto_hide: bool,
    /// 0..100 over the declared 0..2000ms.
    bar_fade: u32,
    bar_track: bool,
    bar_track_colour: [u32; 3],
    bar_track_a: f32,
    /// The one track a press is currently holding, if any. A track's
    /// rectangle is not kept beside it: the hit map already has it, and
    /// two copies of a geometry are two chances to disagree.
    dragging: Option<Act>,
    dropdown: Option<Dropdown>,
    /// When the dropdown was opened — drives the accordion animation.
    dropdown_since: Option<Instant>,
    /// The open list's own scroll — the offset
    /// `object::dropdown::accordion` frames its body at. ONE view for
    /// whichever list is open, reset when a list opens: an offset is a
    /// property of the unfolding, not of the button it hangs from.
    list_scroll: ScrollView,
    /// The open list's bar, as the accordion framed it this frame: the
    /// box the bar stands in, the frame's height and the body's length.
    /// `None` while no list scrolls. The object draws the bar; the
    /// pointer is this window's, so the press aims with this.
    list_bar: Option<(Rect, f32, f32)>,
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
    /// The ADDONS view's report, taken from the toolkit on the way into
    /// the page and not re-asked while it is open. Lines rather than
    /// problems: what a page shows is text, and keeping the shaping
    /// where the two answers are read is what lets it be tested without
    /// installing settings directories in the test process.
    addon_report: Vec<String>,
    /// What the BOARDS view asked for; the application consumes it.
    pub board_action: Option<BoardAction>,
    /// The body's scroll offset, and its physics. One per window rather
    /// than one per page: every road into a page runs through
    /// [`Settings::go`], and a page reopened halfway down is a page that
    /// opens showing its middle.
    scroll: ScrollView,
    /// How the last frame laid the flow out, and the clock it ran at. A
    /// key arrives outside the drawing and has to ask somebody where the
    /// page stood before it can move it.
    flow: Flow,
    now: f64,
    /// The box the body is being clipped to while it draws, so a rect
    /// can be trimmed to what the eye can actually see. None outside the
    /// body: the chrome and the dropdown are not clipped.
    clip: Option<Rect>,
    /// What the FLOWED bands registered this frame — everything the one
    /// scroll is answerable for, and nothing else.
    ///
    /// The chrome stands still, a pinned band stands outside the box the
    /// flow is read in, an open list has a scroll of its own, and the
    /// navigation stands in its own columns until the window folds — at
    /// which point its entries ARE bands of the flow and are in here
    /// with the rest. Only what is in here is chased
    /// ([`Settings::chase_focus`]): spending the page's scroll on
    /// anything else would carry the page off under something that had
    /// not moved.
    flowed: Vec<FocusId>,
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
            view: View::LookFeel,
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
            editor_pulse: None,
            border_kinds: vec!["LINE".to_string(), "NEON".to_string()],
            current_border: None,
            background_kinds: vec![
                "SOLID".to_string(),
                "BLUR".to_string(),
                "FROSTED GLASS".to_string(),
            ],
            current_background: None,
            tint: [60, 20, 210],
            wash: [20, 15, 210],
            bg_opacity: 100,
            bg_depth: 50,
            bg_coverage: 42,
            naming: None,
            // The editor opens on ADVANCED — the page that has always
            // been there — with BASIC's three sliders at rest and no
            // seeds yet, so a BASIC page that was somehow reached before
            // `seed_editor_from_theme` ran would write nothing at all.
            editor_basic: false,
            tone: TONE_REST,
            tone_seeds: None,
            edge: [70, 12, 200],
            // The whole-theme sections. OPENING VALUES ONLY, and never a
            // frame's: the one road onto the editor page
            // (`Act::ThemesEditor`) and its CANCEL both run
            // `seed_editor_from_theme`, which overwrites every one of
            // these from the live bake before anything is drawn. The
            // documented exception the no-baked-look rule allows.
            accent: [70, 60, 200],
            surface_own_hue: false,
            surface_hue: 200,
            surface_lift: 50,
            surface_chroma: 25,
            text_lift: 50,
            text_chroma: 33,
            severity_kinds: SEVERITY_ROLES.iter().map(|r| r.0.to_string()).collect(),
            current_severity: None,
            severity: [[70, 60, 200]; 7],
            severity_touched: [false; 7],
            corner_kinds: ["SQUARE", "ROUND", "CHAMFER"]
                .iter()
                .map(|k| k.to_string())
                .collect(),
            current_corner: None,
            corner_sm: 20,
            corner_md: 30,
            corner_lg: 55,
            corner_segments: 6,
            stroke_hair: 25,
            ring_style_kinds: ["SOLID", "DASHED"].iter().map(|k| k.to_string()).collect(),
            current_ring_style: None,
            ring_on: false,
            ring_width: 25,
            ring_offset: 20,
            ring_colour: [70, 60, 200],
            ring_dash: 40,
            ring_gap: 20,
            ring_halo: false,
            ring_halo_alpha: 30,
            unfocused_dim: 62,
            menu_fill: [33, 10, 210],
            menu_fill_a: 1.0,
            menu_edge: [60, 40, 210],
            menu_edge_a: 1.0,
            menu_edge_w: 25,
            menu_hint: [60, 10, 210],
            menu_hint_a: 1.0,
            tip_fill: [33, 10, 210],
            tip_fill_a: 1.0,
            tip_edge: [60, 40, 210],
            tip_edge_a: 1.0,
            tip_edge_w: 25,
            tip_text: [90, 10, 210],
            tip_text_a: 1.0,
            scroll_mode_kinds: ["OVERLAY", "INSET", "NONE"]
                .iter()
                .map(|k| k.to_string())
                .collect(),
            current_scroll_mode: None,
            scroll_edge_kinds: ["RIGHT", "LEFT"].iter().map(|k| k.to_string()).collect(),
            current_scroll_edge: None,
            bar_w: 20,
            bar_w_hover: 43,
            bar_auto_hide: true,
            bar_fade: 13,
            bar_track: false,
            bar_track_colour: [30, 10, 210],
            bar_track_a: 0.5,
            dragging: None,
            dropdown: None,
            dropdown_since: None,
            list_scroll: ScrollView::new(),
            list_bar: None,
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
            addon_report: Vec::new(),
            board_action: None,
            scroll: ScrollView::new(),
            flow: Flow {
                view: Rect::new(0.0, 0.0, 0.0, 0.0),
                length: 0.0,
                offset: 0.0,
            },
            now: 0.0,
            clip: None,
            flowed: Vec::new(),
            hits: Vec::new(),
            flash: None,
        }
    }

    /// Enters a page. The offset belongs to the page being left, so it
    /// stays with it: every `self.view =` in the window goes through
    /// here, which is the only reason a reopened page starts at its top.
    /// Leaving the editor IS the cancel, for as long as SAVE is unbuilt:
    /// the preview never touched a file, so dropping it puts the desktop
    /// back the way the theme has it. Without this — a verified finding —
    /// the first touched slider left its overlay standing for the rest of
    /// the session, with no way back short of restarting.
    fn leave_editor_preview(&mut self) {
        if self.view == View::ThemeEditor {
            nacelle::theme::clear_preview();
            self.editor_pulse = None;
        }
    }

    fn go(&mut self, view: View) {
        // Any road out of the editor page drops its preview — Back, Escape
        // and every door share this one gate, so none can forget.
        if view != View::ThemeEditor {
            self.leave_editor_preview();
        }
        // And any road at all drops an open list. An anchor the next
        // page does not draw has nothing to hang a list from, so it
        // would hang over whatever the next page put there and eat the
        // first Escape. Three doors used to say this each for
        // themselves, which was enough while every way to another page
        // stood ON that page — the rail is a way to another page from
        // ANY page, so the rule belongs to the road and not to the door.
        self.dropdown = None;
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
    /// wheel and belongs to another stage. That day has come: `main.rs`
    /// calls this from its `MouseWheel` arm, ahead of the hit test on the
    /// board behind the window, so the `allow(dead_code)` this carried is
    /// gone. The keyboard's PageUp/PageDown/Home/End move the same offset.
    pub fn wheel(&mut self, notches: f32) {
        if !self.open {
            return;
        }
        // The SAVE AS prompt already swallows clicks and keys; the wheel
        // joins them, or the page scrolls under the scrim.
        if self.naming.is_some() {
            return;
        }
        // An open list is the topmost scrolled thing on the page, so the
        // wheel is its before it is the page's — a notch over an unfolded
        // list that moved the rows UNDER the list was the settled page
        // grabbing a gesture aimed at the float above it. The accordion
        // owns the clamp (its tick runs every frame it is drawn); this
        // only feeds the offset.
        if self.dropdown.is_some() {
            self.list_scroll.wheel(-notches, &ScrollPhysics::from_theme(), self.now);
            return;
        }
        // Negated, as at every other caller (search's list, the file
        // browser): winit reports scrolling UP as positive, and a page
        // scrolled up shows EARLIER content — a smaller offset. Passed
        // through raw, the page ran away from the hand.
        self.scroll.wheel(-notches, &ScrollPhysics::from_theme(), self.now);
    }

    /// The place of the CHOSEN severity role in [`SEVERITY_ROLES`] — the
    /// index the three severity sliders read and write through. `None`
    /// while no role stands in the list, in which case the sliders are
    /// not on screen at all (`severity_chosen`).
    fn severity_idx(&self) -> Option<usize> {
        let cur = self.current_severity.as_deref()?;
        self.severity_kinds.iter().position(|k| k == cur)
    }

    /// One severity slider's write: the chosen role's component, and the
    /// role marked TOUCHED — the mark `editor_edits` gates the write on,
    /// so six untouched roles keep the theme's own words.
    fn set_severity(&mut self, component: usize, v: u32) {
        if let Some(i) = self.severity_idx() {
            self.severity[i][component] = v;
            self.severity_touched[i] = true;
        }
    }

    /// BASIC's three sliders as the model's own relative move.
    ///
    /// [`TONE_REST`] maps to `Tone::NEUTRAL` exactly, which is what
    /// makes "open BASIC and touch nothing" a no-op all the way to the
    /// file.
    fn tone_of(&self) -> nacelle::theme::edit::Tone {
        nacelle::theme::edit::Tone {
            hue_deg: self.tone[0] as f32,
            sat: self.tone[1] as f32 / 100.0,
            light: span_of(self.tone[2], TONE_LIGHT_SPAN),
        }
    }

    /// How far one press of each BASIC slider moves it, in that track's
    /// own whole units.
    ///
    /// THE NOTCH IS THE PIPELINE'S, not a look and not a taste. The
    /// model works it out from the swapchain's bit depth — one output
    /// code is `q = 1/(2^bits - 1)`, and a notch is the smallest move
    /// that can change one code — and the depth is a SETTING, chosen on
    /// SETTINGS -> COLOR (the DEPTH chips) and kept in the desktop's
    /// config beside the space, the LUT and the ICC profile. libnacelle
    /// has no config and could not read it; a theme token would let a
    /// file lie about the hardware. So it crosses the seam as an
    /// argument, from the copy this window took on the way into the
    /// editor.
    ///
    /// EIGHT BITS WHEN NOBODY HAS SAID, and the config is where that is
    /// answered: `ColorConf::depth()` falls back to the toolkit's own
    /// `DEFAULT_DEPTH_BITS`, which is the floor every swapchain
    /// supports, so what arrives here has already been decided. A notch
    /// coarser than the pipeline is honest; a notch finer is a slider
    /// that does nothing for several presses and reads as broken.
    ///
    /// NEVER BELOW ONE, AND PAST TEN BITS THE TRACK IS THE LIMIT — not
    /// the depth. These tracks carry whole numbers: one degree of HUE,
    /// one percent of SATURATION, a fiftieth of the lightness span. On
    /// the master's mint seed (C 0.1531) the pipeline's own notch is
    /// 1.47 deg / 0.0256 / 0.0039 at eight bits, which is 1, 3 and 2
    /// track units; at ten it is 0.37 deg / 0.0064 / 0.00098, already
    /// finer than a unit, and at twelve and sixteen finer still — so all
    /// three answers are 1 from ten bits up. That is the FLOOR doing its
    /// job and not a slider that stopped listening: a whole-number track
    /// has nothing between 1 and 0, and 0 would be a press that moves
    /// nothing. Reaching every code a twelve-bit swapchain can show
    /// would take a finer TRACK, which is a change to the control and
    /// the owner's to make — and coarse is the side he chose.
    fn tone_step(&self) -> [u32; 3] {
        // The seed's own chroma, which is what a rotation and a scaling
        // move along: a grey theme gets coarse notches, and that is
        // right — turning a grey moves nothing however far it goes.
        let seed_chroma = self.tone_seeds.map_or(0.0, |s| s.accent.c);
        let st = nacelle::theme::edit::tone_step(self.color_depth, seed_chroma);
        let unit = |x: f32, per_unit: f32| {
            let n = (x / per_unit).round();
            if n.is_finite() { (n as u32).max(1) } else { 1 }
        };
        [
            unit(st.hue_deg, 1.0),
            unit(st.sat, 0.01),
            unit(st.light, TONE_LIGHT_UNIT),
        ]
    }

    /// BASIC's move, folded into the ADVANCED page's own controls.
    ///
    /// This is what makes leaving BASIC keep its work. BASIC is
    /// RELATIVE, so its edits exist only while its band is standing;
    /// the moment the page turns, `editor_edits` stops writing them and
    /// the ADVANCED sliders answer for the same ten authors alone. So
    /// the move has to become THEIR value, and the model's own
    /// `ToneSeeds::shifted` is the arithmetic — the same clamps the
    /// writes use, so the fold and the preview cannot drift.
    ///
    /// WHY THIS AND NOT A RE-SEED FROM THE BAKE. Re-reading the whole
    /// page off the live theme looks like the simpler answer and is a
    /// trap: a preview carries COLOURS and LENGTHS into the bake but not
    /// enum WORDS (`enum_word_of` answers off the schema), so a re-seed
    /// would quietly put a chosen corner cut, ring style, background
    /// kind and scrollbar mode back to whatever the FILE says — and
    /// exactly the unsaved ADVANCED work the switch is supposed to
    /// protect would be the work it destroyed. Measured, 2026-08-17.
    ///
    /// WHAT THE FOLD CANNOT CARRY, measured 2026-08-17. The ADVANCED
    /// page edits colours on three HSV tracks over sRGB — the owner's
    /// decision of 2026-08-16, and how every colour on that page has
    /// always worked — while BASIC writes `oklch(...)` and, by the
    /// owner's "BEZ OGRANICZEŃ zakresu", is not held to any gamut. So a
    /// BASIC move that lands OUTSIDE sRGB (a light violet, say: the
    /// LIGHTNESS slider up and the HUE slider round to 295 deg) arrives
    /// at a page that has no way to write it, and `from_oklch` maps it
    /// in: the hue and the lightness come through exactly, the chroma
    /// comes through as far as sRGB reaches. Nothing else can happen
    /// while the destination page is an sRGB editor — and losing a
    /// little chroma is the small loss; refusing to fold at all would
    /// lose the whole move.
    ///
    /// Nothing outside the ten authors is touched.
    fn fold_tone_into_advanced(&mut self) {
        let Some(seeds) = self.tone_seeds else { return };
        let tone = self.tone_of();
        if tone == nacelle::theme::edit::Tone::NEUTRAL {
            return;
        }
        let moved = seeds.shifted(tone);
        // `from_oklch` answers in LINEAR light; the tracks are sRGB-encoded
        // ([`hsv_track_of`]), so the colour is encoded on the way over.
        // Without the encode the fold handed ADVANCED a lighter colour than
        // BASIC was showing, and the page then wrote that lighter colour
        // back to the theme.
        let onto_tracks =
            |p| hsv_track_of(nacelle::theme::Color::from_oklch(p).to_srgb());
        self.accent = onto_tracks(moved.accent);
        for i in 0..SEVERITY_ROLES.len() {
            self.severity[i] = onto_tracks(moved.severity[i]);
            // ADVANCED writes only the roles a slider TOUCHED, and the
            // rotation moved all seven — so all seven are now the page's
            // own words, or the next preview would put the theme's back.
            self.severity_touched[i] = true;
        }
        self.surface_lift = span_back(moved.surface_lift, SURFACE_LIFT_WALL);
        self.text_lift = span_back(moved.text_lift, TEXT_LIFT_WALL);
        // THE BODY'S BED COMES TOO. The BACKGROUND section holds it as an
        // absolute colour and not as one of the ten authors, so the move
        // does not re-derive it — but the screen has been SHOWING it
        // turned (`editor_edits` carries it there), and ADVANCED has to
        // carry on from what the screen showed rather than from what the
        // track still said. Both quads, not just the live one: which of
        // them is written is the kind list's business, and a kind chosen
        // after the drag would otherwise arrive on the old hue.
        for t in [&mut self.tint, &mut self.wash] {
            let moved = tone.shift(oklch_of_track(t, 1.0));
            *t = onto_tracks(moved);
        }
        // A hue move re-welds the beds to the accent — BASIC's promise of
        // one hue for the whole interface — so ADVANCED carries on
        // writing the reference rather than a number of its own.
        if tone.hue_deg != 0.0 {
            self.surface_own_hue = false;
        }
        // The seeds are now what the sliders left behind, and the
        // sliders are back at rest: the move has become part of what a
        // NEXT visit to BASIC would be relative to.
        self.tone_seeds = Some(moved);
        self.tone = TONE_REST;
    }

    /// The theme's AUTHORS as they stand, and the three sliders back at
    /// rest — what BASIC's relative move is measured from.
    ///
    /// Read off the LIVE bake, preview included, so arriving on BASIC
    /// from a page of ADVANCED edits measures from what is on the screen
    /// and not from what is in the file. A token this build cannot read
    /// leaves the seeds unset altogether and BASIC writes nothing, which
    /// is the same neutrality every other unseeded set in this editor
    /// keeps: better a page that does nothing than one that guesses.
    ///
    /// THE BAKE ANSWERS IN sRGB, so every reading here is decoded before
    /// it is asked for its OKLCh — the space OKLCh is defined over is
    /// linear light (`theme/color.rs`), and libnacelle's own tests take
    /// the same step (`to_linear().to_oklch()`). Reading encoded values
    /// as if they were linear does not merely mis-report: BASIC WRITES
    /// WHAT IT READS, so the page seeded itself from the colour it had
    /// just written and the theme climbed on its own. Measured over the
    /// master with all three sliders AT REST: the accent's L went
    /// 0.8200 -> 0.8904 -> 0.9413 -> 0.9715 on successive visits, and
    /// the gap between `ok` and `critical` opened from 121.0 deg to
    /// 126.8 deg on the first visit alone.
    fn seed_tone_from_theme(&mut self) {
        let t = nacelle::theme::resolved();
        let col_of = |n: &str| nacelle::theme::id(n).map(|i| t.color(i).to_linear());
        let px = |n: &str| nacelle::theme::id(n).map(|i| t.px(i)).unwrap_or(0.0);
        self.tone = TONE_REST;
        let Some(accent) = col_of("palette.accent") else {
            self.tone_seeds = None;
            return;
        };
        let accent = accent.to_oklch();
        // A role whose author this build cannot read follows the accent,
        // which is where the rotation would carry it in any case.
        let mut severity = [accent; 7];
        for (i, (_, token, _)) in SEVERITY_ROLES.iter().enumerate() {
            if let Some(c) = col_of(token) {
                severity[i] = c.to_oklch();
            }
        }
        self.tone_seeds = Some(nacelle::theme::edit::ToneSeeds {
            accent,
            severity,
            surface_lift: px("surface.lift"),
            text_lift: px("text.lift"),
        });
    }

    /// The whole of what the editor is set to, as the token edits both the
    /// PREVIEW and a SAVE are made of — one builder, or the file and the
    /// screen would drift.
    ///
    /// BASIC does not REPLACE this set, it lands ON it. The advanced
    /// controls were all seeded off the theme, so carrying them is
    /// carrying the theme's own state; BASIC then overrides the ten
    /// authors it moves and leaves everything else — a corner cut, a
    /// scrollbar's width, a focus ring — exactly where ADVANCED left it.
    /// That is what makes the switch lossless in the BASIC direction:
    /// the page the user cannot see is still in the edit.
    fn editor_edits(&self) -> Vec<nacelle::theme::edit::Edit> {
        use nacelle::theme::edit::{
            accent_edit, border_colour_edit, border_edits, focus_ring_edits, glass_edits,
            menu_edits, scrollbar_edits, severity_role_edit, shape_edits, surface_edits,
            text_edits, tooltip_edits, unfocused_dim_edit, Border, CornerCut, FocusRing,
            Glass, RingStyle, Scope, ScrollbarEdge, ScrollbarMode, SurfaceHue,
        };
        // The sliders are HSV — brightness, saturation, hue — and the file
        // wants OKLCh, so every value below crosses HSV -> sRGB -> LINEAR
        // -> OKLCh on the way out. That map is [`oklch_of_track`], written
        // once and paired with [`hsv_track_of`] going the other way; the
        // decode in the middle of it is why they must stay a pair, and
        // what happens when they do not is recorded there. See
        // [`hsv_to_rgb`] for why HSV at all: brightness 100 % must be the
        // hue's own full brightness, never white.
        let of = oklch_of_track;
        // What the LIVE theme already dresses — the two `halo_dressed`
        // answers the model asks its caller for, read off the bake here so
        // the model itself stays pure.
        let t = nacelle::theme::resolved();
        let live = |n: &str| nacelle::theme::id(n).map(|i| t.px(i)).unwrap_or(0.0);
        let colour = of(&self.edge, 1.0);
        let mut edits = match self.current_border.as_deref() {
            // No kind chosen: the colour moves ALONE. Mapping "no choice"
            // to LINE was a verified bug — a colour drag before the list
            // was touched switched the halo off as a side effect.
            None => vec![border_colour_edit(Scope::Theme, colour)],
            other => {
                let kind = if other == Some("NEON") { Border::Neon } else { Border::Line };
                // Whether the THEME already dresses a visible halo — if it
                // does, NEON keeps the theme's radius and alpha instead of
                // flattening five themes' dress to one theme's numbers.
                let dressed = live("glow.panel_edge.radius") > 0.0
                    && live("glow.panel_edge.alpha") > 0.0;
                border_edits(Scope::Theme, kind, colour, dressed)
            }
        };
        // The background joins the same set. `None` means the list was
        // never touched and the theme's own background stands — the same
        // neutrality the border's `None` earned after verification.
        if let Some(kind_name) = self.current_background.as_deref() {
            let kind = match kind_name {
                "BLUR" => Glass::Blur,
                "FROSTED GLASS" => Glass::Frosted,
                _ => Glass::Solid,
            };
            // BASIC'S PROMISE, KEPT FOR THE BODY TOO. This section writes
            // the window body's bed as an ABSOLUTE colour — `panel.fill`
            // on SOLID, the glass tint and wash otherwise — and BASIC's
            // ten authors do not include it, so a hue drag turned the
            // rail, the sub-page column and every other bed and left the
            // BODY on the theme's old hue. Measured on the master at the
            // first slider position the gate takes: rail 203.46 deg, sub
            // 203.46 deg, the body still 166.22. It is the same case
            // `tone_edits` answers by re-pointing `surface.hue` at the
            // accent, except a literal cannot be re-pointed — so it is
            // carried, by the model's own arithmetic ([`Tone::shift`]).
            //
            // Gated exactly as the tone edits are (BASIC, and seeds to
            // move from), so the preview and the ten-token move can never
            // be shifted by different amounts; and undone the moment the
            // fold banks the move onto the tracks and puts the sliders
            // back at rest, so nothing is applied twice.
            let carry = match (self.editor_basic, self.tone_seeds) {
                (true, Some(_)) => self.tone_of(),
                _ => nacelle::theme::edit::Tone::NEUTRAL,
            };
            edits.extend(glass_edits(
                Scope::Theme,
                kind,
                carry.shift(of(&self.tint, 1.0)),
                carry.shift(of(&self.wash, 1.0)),
                self.bg_opacity as f32 / 100.0,
                1.0 + self.bg_depth.min(100) as f32 / 50.0,
                self.bg_coverage as f32 / 100.0,
            ));
        }
        // ---- the whole-theme sets (2026-08-16), in the model's order.
        // Everything below is SEEDED off the live bake on the way into the
        // page, so carrying it whole is carrying the theme's own state
        // back — the two exceptions with a real side to choose are gated:
        // severity on its TOUCHED marks, and any set whose word could not
        // be read at all is left out rather than guessed.
        edits.push(accent_edit(Scope::Theme, of(&self.accent, 1.0)));
        let hue = if self.surface_own_hue {
            SurfaceHue::Own(self.surface_hue.min(359) as f32)
        } else {
            // OFF restores the derivation AS A REFERENCE, so a later
            // accent drag keeps moving the surfaces — the model's test
            // pins this exact string.
            SurfaceHue::FollowAccent
        };
        edits.extend(surface_edits(
            Scope::Theme,
            hue,
            span_of(self.surface_lift, SURFACE_LIFT_WALL),
            scale_of(self.surface_chroma, SURFACE_CHROMA_CEILING),
        ));
        edits.extend(text_edits(
            Scope::Theme,
            span_of(self.text_lift, TEXT_LIFT_WALL),
            scale_of(self.text_chroma, TEXT_CHROMA_CEILING),
        ));
        // Only the roles a slider actually moved: six untouched authors
        // keep the theme's own words, references included.
        for (i, (_, _, role)) in SEVERITY_ROLES.iter().enumerate() {
            if self.severity_touched[i] {
                edits.push(severity_role_edit(Scope::Theme, *role, of(&self.severity[i], 1.0)));
            }
        }
        if let Some(cut) = self.current_corner.as_deref() {
            let cut = match cut {
                "SQUARE" => CornerCut::Square,
                "CHAMFER" => CornerCut::Chamfer,
                _ => CornerCut::Round,
            };
            edits.extend(shape_edits(
                Scope::Theme,
                cut,
                scale_of(self.corner_sm, 4.0),
                scale_of(self.corner_md, 4.0),
                scale_of(self.corner_lg, 4.0),
                self.corner_segments.clamp(3, 16) as u8,
                scale_of(self.stroke_hair, 1.0),
            ));
        }
        if let Some(style) = self.current_ring_style.as_deref() {
            let ring = FocusRing {
                style: if style == "DASHED" { RingStyle::Dashed } else { RingStyle::Solid },
                width_u: scale_of(self.ring_width, 2.0),
                offset_u: scale_of(self.ring_offset, 2.0),
                colour: of(&self.ring_colour, 1.0),
                dash_u: scale_of(self.ring_dash, 4.0),
                gap_u: scale_of(self.ring_gap, 4.0),
                halo: self.ring_halo,
                halo_alpha: self.ring_halo_alpha.min(100) as f32 / 100.0,
                // The same live-dress contract as the border's NEON: a
                // theme that has dressed its halo keeps its radius.
                halo_dressed: live("glow.focus_ring.radius") > 0.0
                    && live("glow.focus_ring.alpha") > 0.0,
            };
            edits.extend(focus_ring_edits(Scope::Theme, self.ring_on, &ring));
        }
        edits.push(unfocused_dim_edit(
            Scope::Theme,
            self.unfocused_dim.min(100) as f32 / 100.0,
        ));
        // The floats' colours carry the SEED's alphas — the model passes
        // a colour's channel through, and the sliders have no say in it.
        edits.extend(menu_edits(
            Scope::Theme,
            of(&self.menu_fill, self.menu_fill_a),
            of(&self.menu_edge, self.menu_edge_a),
            scale_of(self.menu_edge_w, 1.0),
            of(&self.menu_hint, self.menu_hint_a),
        ));
        edits.extend(tooltip_edits(
            Scope::Theme,
            of(&self.tip_fill, self.tip_fill_a),
            of(&self.tip_edge, self.tip_edge_a),
            scale_of(self.tip_edge_w, 1.0),
            of(&self.tip_text, self.tip_text_a),
        ));
        if let (Some(mode), Some(edge)) =
            (self.current_scroll_mode.as_deref(), self.current_scroll_edge.as_deref())
        {
            let mode = match mode {
                "OVERLAY" => ScrollbarMode::Overlay,
                "NONE" => ScrollbarMode::None,
                _ => ScrollbarMode::Inset,
            };
            let edge = if edge == "LEFT" { ScrollbarEdge::Left } else { ScrollbarEdge::Right };
            edits.extend(scrollbar_edits(
                Scope::Theme,
                mode,
                edge,
                band_of(self.bar_w, 0.5, 4.0),
                band_of(self.bar_w_hover, 0.5, 4.0),
                self.bar_auto_hide,
                scale_of(self.bar_fade, 2000.0),
                // Track OFF is the switch alone; the groove's colour — with
                // the seed's alpha — is only written while the groove is
                // drawn at all.
                self.bar_track.then(|| of(&self.bar_track_colour, self.bar_track_a)),
            ));
        }
        // ---- BASIC (2026-08-17), last and over the top of the rest.
        // The three sliders move ten AUTHORS; everything above either
        // agrees with them or is about something else entirely, and the
        // ten that overlap are the ten BASIC is FOR.
        if self.editor_basic {
            if let Some(seeds) = self.tone_seeds {
                let tone = self.tone_of();
                for e in nacelle::theme::edit::tone_edits(Scope::Theme, &seeds, tone) {
                    // ONE assignment per token. A list carrying a token
                    // twice would save a file with the key written twice
                    // in one section, and then the file and the screen
                    // would be answering to two different rules about
                    // which of the two wins.
                    match edits.iter_mut().find(|b| b.token == e.token) {
                        Some(slot) => *slot = e,
                        None => edits.push(e),
                    }
                }
            }
        }
        edits
    }

    /// Shows what the editor is set to, without writing anything.
    ///
    /// Called when a value SETTLES — a slider released, an arrow pressed, a
    /// kind chosen — and never while a slider is being dragged. Each call
    /// re-bakes the theme, and a bake is 76 031 bytes that is never freed,
    /// so one per gesture is affordable and one per frame is not. The
    /// slider itself moves at whatever rate the hand does; only the picture
    /// behind it waits for the hand to stop.
    ///
    /// Nothing here touches the file. `theme::clear_preview` puts the
    /// screen back, which is what CANCEL will be made of.
    fn apply_editor_preview(&self) {
        let edits = self.editor_edits();
        let pairs: Vec<(&str, &str)> =
            edits.iter().map(|e| (e.token, e.value.as_str())).collect();
        let refused = nacelle::theme::set_preview(&pairs);
        for r in refused {
            eprintln!("nacelle-desktop: the theme editor could not show {r}");
        }
    }

    /// The editor OPENS ON THE THEME'S OWN STATE — and CANCEL returns to
    /// it: every colour, kind, word and length off the live bake, never a
    /// built-in. The maps back onto the tracks are the exact inverses of
    /// `editor_edits`' maps out, so a theme saved and reopened lands the
    /// sliders where the hand left them.
    fn seed_editor_from_theme(&mut self) {
        let t = nacelle::theme::resolved();
        // One u for the lengths the model writes in u: the bake keeps them
        // in device px, and the file wants the unit back.
        let unit = t.unit_px.max(f32::MIN_POSITIVE);
        let px = |n: &str| nacelle::theme::id(n).map(|i| t.px(i)).unwrap_or(0.0);
        let flag = |n: &str| nacelle::theme::id(n).map(|i| t.flag(i)).unwrap_or(false);
        // An enum token's WORD, spelled the way the lists spell their
        // members. `None` — a token this build has no vocabulary for —
        // leaves the list unseeded, and `editor_edits` then leaves the
        // whole set out rather than guessing a word.
        let word =
            |n: &str| nacelle::theme::id(n).and_then(nacelle::theme::enum_word_of);
        let col_of = |n: &str| nacelle::theme::id(n).map(|i| t.color(i));
        let seed = |slot: &mut [u32; 3], c: nacelle::theme::Color| *slot = hsv_track_of(c);

        // The border: colour and kind.
        if let Some(c) = col_of("elev.panel.edge.color") {
            seed(&mut self.edge, c);
        }
        self.current_border =
            Some(if flag("glow.panel_edge.enabled") { "NEON" } else { "LINE" }.to_string());

        // The background: kind from the rank and the wash, colours from
        // whichever quads are live. A solid seeds the WASH group from the
        // shared fill, because that is the group SOLID writes back through.
        let rank = px("elev.panel.glass.rank").round() as u32;
        let wash_a = col_of("elev.panel.glass.wash").map_or(0.0, |c| c.a);
        self.current_background = Some(
            match (rank, wash_a > 0.0) {
                (0, _) => "SOLID",
                (_, false) => "BLUR",
                (_, true) => "FROSTED GLASS",
            }
            .to_string(),
        );
        if rank == 0 {
            if let Some(c) = col_of("component.panel.fill") {
                seed(&mut self.wash, c);
            }
        } else {
            if let Some(c) = col_of("elev.panel.glass.tint") {
                seed(&mut self.tint, c);
            }
            if let Some(c) = col_of("elev.panel.glass.wash") {
                if c.a > 0.0 {
                    seed(&mut self.wash, c);
                }
            }
        }

        // ---- the whole-theme sections (2026-08-16) ----
        // ACCENT: the seed itself.
        if let Some(c) = col_of("palette.accent") {
            seed(&mut self.accent, c);
        }
        // SURFACES. The bake cannot say whether `surface.hue` is still a
        // reference, only what it resolves to — so "the accent's own
        // number" reads as FOLLOW, and an own hue that happens to land
        // exactly on the accent's is indistinguishable and harmlessly
        // reads as follow too.
        let s_hue = px("surface.hue").rem_euclid(360.0);
        let a_hue = px("hue.accent").rem_euclid(360.0);
        self.surface_own_hue = (s_hue - a_hue).abs() > 0.5;
        self.surface_hue = s_hue.round().clamp(0.0, 359.0) as u32;
        self.surface_lift = span_back(px("surface.lift"), SURFACE_LIFT_WALL);
        self.surface_chroma = scale_back(px("surface.chroma"), SURFACE_CHROMA_CEILING);
        // TEXT.
        self.text_lift = span_back(px("text.lift"), TEXT_LIFT_WALL);
        self.text_chroma = scale_back(px("text.chroma"), TEXT_CHROMA_CEILING);
        // SEVERITY: all seven authors seeded, NONE touched — the marks
        // are what keeps an untouched role out of the file.
        for (i, (_, token, _)) in SEVERITY_ROLES.iter().enumerate() {
            if let Some(c) = col_of(token) {
                seed(&mut self.severity[i], c);
            }
            self.severity_touched[i] = false;
        }
        self.current_severity = Some(self.severity_kinds[0].clone());
        // SHAPE.
        self.current_corner = word("corner.mode").map(|w| w.to_uppercase());
        self.corner_sm = scale_back(px("corner.sm") / unit, 4.0);
        self.corner_md = scale_back(px("corner.md") / unit, 4.0);
        self.corner_lg = scale_back(px("corner.lg") / unit, 4.0);
        self.corner_segments = (px("corner.segments").round() as u32).clamp(3, 16);
        self.stroke_hair = scale_back(px("stroke.hair") / unit, 1.0);
        // FOCUS RING.
        self.ring_on = flag("focus.ring.enabled");
        self.current_ring_style = word("focus.ring.style").map(|w| w.to_uppercase());
        self.ring_width = scale_back(px("focus.ring.width") / unit, 2.0);
        self.ring_offset = scale_back(px("focus.ring.offset") / unit, 2.0);
        if let Some(c) = col_of("focus.ring.color") {
            seed(&mut self.ring_colour, c);
        }
        self.ring_dash = scale_back(px("focus.ring.dash") / unit, 4.0);
        self.ring_gap = scale_back(px("focus.ring.gap") / unit, 4.0);
        self.ring_halo = flag("glow.focus_ring.enabled");
        self.ring_halo_alpha = scale_back(px("glow.focus_ring.alpha"), 1.0);
        self.unfocused_dim =
            (px("focus.unfocused_dim") * 100.0).round().clamp(30.0, 100.0) as u32;
        // MENU and TOOLTIP: the colours keep their own alphas beside the
        // sliders, because the model passes the channel through.
        if let Some(c) = col_of("component.menu.fill") {
            seed(&mut self.menu_fill, c);
            self.menu_fill_a = c.a;
        }
        if let Some(c) = col_of("component.menu.border") {
            seed(&mut self.menu_edge, c);
            self.menu_edge_a = c.a;
        }
        self.menu_edge_w = scale_back(px("menu.border") / unit, 1.0);
        if let Some(c) = col_of("component.menu.hint") {
            seed(&mut self.menu_hint, c);
            self.menu_hint_a = c.a;
        }
        if let Some(c) = col_of("component.tooltip.fill") {
            seed(&mut self.tip_fill, c);
            self.tip_fill_a = c.a;
        }
        if let Some(c) = col_of("component.tooltip.edge") {
            seed(&mut self.tip_edge, c);
            self.tip_edge_a = c.a;
        }
        self.tip_edge_w = scale_back(px("tooltip.border") / unit, 1.0);
        if let Some(c) = col_of("component.tooltip.text") {
            seed(&mut self.tip_text, c);
            self.tip_text_a = c.a;
        }
        // SCROLLBAR, through the same reader the bar itself draws from —
        // one interpretation of the words, not a second.
        let look = ScrollbarLook::from_theme();
        self.current_scroll_mode = Some(
            match look.mode {
                scroll::ScrollbarMode::Overlay => "OVERLAY",
                scroll::ScrollbarMode::Inset => "INSET",
                scroll::ScrollbarMode::None => "NONE",
            }
            .to_string(),
        );
        self.current_scroll_edge = Some(
            match look.edge {
                scroll::ScrollbarEdge::Left => "LEFT",
                scroll::ScrollbarEdge::Right => "RIGHT",
            }
            .to_string(),
        );
        self.bar_w = band_back(look.w / unit, 0.5, 4.0);
        self.bar_w_hover = band_back(look.w_hover / unit, 0.5, 4.0);
        self.bar_auto_hide = look.auto_hide;
        self.bar_fade = scale_back(look.fade_ms, 2000.0);
        self.bar_track = word("scrollbar.track").as_deref() == Some("on");
        if let Some(c) = col_of("component.scrollbar.track") {
            seed(&mut self.bar_track_colour, c);
            self.bar_track_a = c.a;
        }
        // BASIC's own seeds, off the same bake and in the same breath:
        // the two pages open on ONE theme, so whatever re-seeds one
        // re-seeds the other. The three sliders come back to rest here,
        // which is what makes CANCEL and the door leave BASIC showing
        // "the theme as it stands" rather than a move already made.
        self.seed_tone_from_theme();
    }

    /// A theme's name may be its file's name, nothing more.
    fn theme_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    }

    /// Writes the edit set under `name` and, when the write lands, makes
    /// the saved theme the one in force. Answers whether the
    /// configuration changed — the caller's cue to re-resolve, which is
    /// what reloads the theme off the file just written.
    fn editor_save_named(&mut self, name: &str) -> bool {
        match nacelle::theme::save_theme(name, &self.editor_edits()) {
            Ok(path) => {
                eprintln!("nacelle-desktop: theme saved to {}", path.display());
                config::set_engine_theme(name);
                nacelle::theme::clear_preview();
                self.editor_pulse = None;
                self.naming = None;
                // The list learns about the file it just gained NOW — the
                // walk that fills it otherwise runs when the window opens,
                // and a theme saved mid-session stayed invisible until a
                // reopen.
                self.themes = nacelle::theme::available_themes();
                self.refresh_current();
                true
            }
            Err(e) => {
                eprintln!("nacelle-desktop: the theme was NOT saved: {e}");
                false
            }
        }
    }

    /// Mouse move while a track is held — a slider's, or the open
    /// list's thumb.
    pub fn drag(&mut self, x: f32, y: f32) {
        // The held thumb first: it owns the pointer absolutely (the
        // thumb goes where the hand is), and while it is held no slider
        // is — `press_thumb` and the hit walk are the same press, so the
        // two grabs cannot coexist.
        if self.list_scroll.dragging() {
            if let Some((area, viewport, content)) = self.list_bar {
                let look = ScrollbarLook::from_theme();
                if let Some(geom) = scroll::scrollbar(
                    area,
                    &look,
                    self.list_scroll.offset(),
                    viewport,
                    content,
                    true,
                ) {
                    self.list_scroll.drag(y, viewport, content, geom.track);
                }
            }
            return;
        }
        let Some(act) = self.dragging else { return };
        self.set_from_x(act, x);
        self.mark_dirty(act);
        // The editor's tracks show themselves WHILE dragged — the owner asked
        // for the picture to follow the hand, not the release. Throttled,
        // because every distinct value is a fresh 76 KB bake that is never
        // freed: ten a second is ~0.8 MB for a second of active dragging,
        // sixty a second would be 4.5. The slider itself still moves every
        // frame; only the desktop behind it updates on the pulse.
        if let Act::EditorTrack(_) = act {
            let due = self
                .editor_pulse
                .map_or(true, |t| t.elapsed().as_millis() >= 100);
            if due {
                self.editor_pulse = Some(Instant::now());
                self.apply_editor_preview();
            }
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

    /// Mouse button released; returns true when the configuration
    /// changed — the font sizes, which the caller must re-resolve and
    /// re-apply. The rest write themselves and are pushed on by their
    /// dirty flags.
    pub fn release(&mut self) -> bool {
        // The list's thumb, symmetric with its grab: nothing about the
        // configuration changes when a scrollbar is let go.
        self.list_scroll.release();
        let Some(act) = self.dragging.take() else { return false };
        if let Some(&Ctrl::Slider { save, .. }) = slider_of(act) {
            save(self);
        }
        matches!(act, Act::SizeTrack(_))
    }

    /// Opens the window where the rail opens it: on LOOK AND FEEL, the
    /// first section. There is no menu to land on any more, and landing
    /// on a section means landing on a section's PAGE — so this is the
    /// same road [`Act::OpenLookFeel`] takes, scan of the three
    /// directories included, and not a bare `go`.
    pub fn show(&mut self) {
        self.open = true;
        self.enter_look_feel();
        nacelle::sound::emit(nacelle::sound::Event::PanelOpen);
    }

    /// LOOK AND FEEL, with its three lists read fresh.
    ///
    /// They are directories the user installs into behind the program's
    /// back, and the page offers all three at the same time, so they are
    /// scanned on the way in — once. The engine's themes, not the look
    /// directories: a look bundled a stylesheet, and stylesheets are gone.
    fn enter_look_feel(&mut self) {
        self.themes = config::list_engine_themes();
        self.layauts = config::list_layauts();
        self.sounds = config::list_sound_themes();
        self.refresh_current();
        self.go(View::LookFeel);
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
        // While the SAVE AS prompt stands, the pointer reaches nothing
        // under it — the prompt is keyboard-shaped, and a click that fell
        // through to a slider would drag the theme mid-naming.
        if self.naming.is_some() {
            return false;
        }
        // The open list's bar, before the rows: it stands in the lane
        // BESIDE them, so nothing else answers there, and a bar that was
        // only drawn was the UX finding this closes — a thumb the eye
        // reads as draggable must actually take the hand. The band is
        // the bar's widest (hover included), the same rule the page's
        // bar draws by; a press on the track pages one frame-height
        // toward it, the toolkit's own word on track clicks.
        if self.dropdown.is_some() {
            if let Some((area, viewport, content)) = self.list_bar {
                let look = ScrollbarLook::from_theme();
                let reach = look.w_hover.max(look.w) + look.margin;
                let band = match look.edge {
                    scroll::ScrollbarEdge::Left => Rect::new(area.x, area.y, reach, area.h),
                    scroll::ScrollbarEdge::Right => {
                        Rect::new(area.right() - reach, area.y, reach, area.h)
                    }
                };
                if band.contains(x, y) {
                    if let Some(geom) = scroll::scrollbar(
                        area,
                        &look,
                        self.list_scroll.offset(),
                        viewport,
                        content,
                        true,
                    ) {
                        if !self.list_scroll.press_thumb(y, geom.thumb) {
                            self.list_scroll.page(y > geom.thumb.y, viewport, self.now);
                        }
                    }
                    return false;
                }
            }
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
            // The editor's switches speak toggle, like every other switch.
            Act::EditorFlip(_) => {}
            Act::VolumeTrack => {}
            Act::Pick(..) => {}
            _ => emit(Sfx::Click),
        }
        match act {
            Act::Close => {
                // Closing the window does not pass through `go`, so the
                // editor's preview is dropped here as well.
                self.leave_editor_preview();
                self.open = false;
                emit(Sfx::PanelClose);
            }
            Act::EditorCancel => {
                nacelle::theme::clear_preview();
                self.editor_pulse = None;
                self.seed_editor_from_theme();
            }
            // NEITHER PAGE EATS THE OTHER'S WORK, and the two directions
            // are not symmetrical, because the two pages are not.
            //
            // BASIC -> ADVANCED FOLDS. BASIC's edits are relative and
            // stop being written the moment its band stops standing, so
            // the move is handed to the ten ADVANCED controls that
            // answer for the same authors and the sliders come back to
            // rest ([`Settings::fold_tone_into_advanced`]). The look
            // does not move; only who is writing it does.
            //
            // ADVANCED -> BASIC TAKES SEEDS AND NOTHING ELSE. The
            // advanced controls hold work that is in no file yet, and
            // BASIC lands ON their edit rather than replacing it, so
            // there is nothing to hand over — only the question "what is
            // the move relative to", answered off the live theme.
            Act::EditorMode => {
                let folding = self.editor_basic;
                self.editor_basic = !folding;
                if folding {
                    self.fold_tone_into_advanced();
                } else {
                    self.seed_tone_from_theme();
                }
                self.apply_editor_preview();
            }
            Act::EditorSaveAs => {
                use nacelle::object::text_input::{InputModel, Validator};
                self.naming = Some(
                    InputModel::new()
                        .with_validator(Validator::Charset(Self::theme_name_char))
                        .with_max_len(40),
                );
            }
            Act::EditorSave => {
                let name = config::current_engine_theme()
                    .unwrap_or_else(|| "default".to_string());
                // The master is not a file: SAVE on `default` IS SAVE AS,
                // by the owner's rule. And the rule follows the theme IN
                // FORCE, not the config line: a `Theme=` naming a file that
                // no longer exists fell back to the master at load, so a
                // SAVE here would resurrect the missing file under the
                // person's feet instead of asking — measured on a config
                // still saying `cockpit` the day the shipped themes left.
                let known = nacelle::theme::available_themes();
                if name.eq_ignore_ascii_case("default")
                    || !known.iter().any(|n| n.eq_ignore_ascii_case(&name))
                {
                    use nacelle::object::text_input::{InputModel, Validator};
                    self.naming = Some(
                        InputModel::new()
                            .with_validator(Validator::Charset(Self::theme_name_char))
                            .with_max_len(40),
                    );
                } else {
                    return self.editor_save_named(&name);
                }
            }
            Act::Back => {
                emit(Sfx::Click);
                // The same answer Escape peels a layer by, so the two
                // ways out of a page cannot lead to different places. A
                // page with no layer above it wears CLOSE and never has
                // this act at all ([`chrome_of`]), so the fallback is
                // only ever the section the window opens on.
                self.go(parent_view(self.view).unwrap_or(View::LookFeel))
            }
            // The rail's section and the second column's first entry:
            // one page, reached from two places, so one road in.
            Act::OpenLookFeel | Act::OpenSets => self.enter_look_feel(),
            Act::ListBtn(list) => {
                let d = Dropdown::List(list);
                self.dropdown = if self.dropdown == Some(d) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    // A list opens at its head — the offset belongs to
                    // the unfolding, not to whatever list scrolled last.
                    self.list_scroll.reset();
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
                        // No config line: a border kind is a value laid
                        // over the theme until SAVE, so choosing one shows
                        // it and nothing else — and it RETURNS FALSE, which
                        // is most of the fix. The common tail below answers
                        // true, and main takes true as "the configuration
                        // changed, re-resolve it": a theme reload, which
                        // builds a fresh engine with an EMPTY preview. So a
                        // click on LINE erased its own preview in the same
                        // breath, and looked dead until the first slider
                        // pulse re-sent it; NEON only appeared to work
                        // because the post-reload state (Cockpit's glow on)
                        // matched what NEON asks for. Measured in the trace
                        // of 2026-08-16, clicks 2313/2528.
                        ListId::Borders => {
                            self.current_border = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
                        ListId::Backgrounds => {
                            self.current_background = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
                        // The whole-theme lists follow the two above: a
                        // pick lays a value over the theme until SAVE,
                        // writes no config line, and must answer FALSE —
                        // true would reload the theme and erase the very
                        // preview the pick just sent (the border pick's
                        // verified bug, not to be re-made five more times).
                        ListId::Severities => {
                            // Choosing a role EDITS nothing: the sliders
                            // re-aim at the role's stored colour, and only
                            // a slider marks it touched.
                            self.current_severity = Some(name.clone());
                            emit(Sfx::Theme);
                            return false;
                        }
                        ListId::Corners => {
                            self.current_corner = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
                        ListId::RingStyles => {
                            self.current_ring_style = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
                        ListId::ScrollModes => {
                            self.current_scroll_mode = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
                        ListId::ScrollEdges => {
                            self.current_scroll_edge = Some(name.clone());
                            self.apply_editor_preview();
                            emit(Sfx::Theme);
                            return false;
                        }
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
                //
                // The colour depth is read on the way in, the way the
                // COLOR page reads its own: BASIC's sliders notch by what
                // the swapchain can show ([`Settings::tone_step`]), and a
                // depth chosen in an earlier session would otherwise not
                // be known here until somebody had opened COLOR.
                self.color_depth = config::color_prefs().depth;
                self.seed_editor_from_theme();
                self.go(View::ThemeEditor);
            }
            Act::OpenSoundLevels => {
                let (vol, typing, ambient) = config::sound_prefs();
                self.sound_volume = vol;
                self.sound_typing = typing;
                self.sound_ambient = ambient;
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
            | Act::SizeTrack(_)
            | Act::EditorTrack(_) => {
                self.dragging = Some(act);
                self.set_from_x(act, x);
                self.mark_dirty(act);
            }
            // The editor's switches: flip the field, show the answer at
            // once — the same live contract as the tracks, with a toggle's
            // own sound. A flip writes no config line and answers false
            // like every editor control, for the border pick's reason.
            Act::EditorFlip(f) => {
                let on = match f {
                    Flip::SurfaceOwnHue => {
                        self.surface_own_hue = !self.surface_own_hue;
                        self.surface_own_hue
                    }
                    Flip::Ring => {
                        self.ring_on = !self.ring_on;
                        self.ring_on
                    }
                    Flip::Halo => {
                        self.ring_halo = !self.ring_halo;
                        self.ring_halo
                    }
                    Flip::BarAutoHide => {
                        self.bar_auto_hide = !self.bar_auto_hide;
                        self.bar_auto_hide
                    }
                    Flip::BarTrack => {
                        self.bar_track = !self.bar_track;
                        self.bar_track
                    }
                };
                self.apply_editor_preview();
                emit(if on { Sfx::ToggleOn } else { Sfx::ToggleOff });
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
            Act::OpenAddons => {
                self.addon_report = addon_report(
                    nacelle::settings::installed(),
                    &nacelle::settings::problems(),
                );
                self.go(View::Addons);
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
                self.go(View::Font);
            }
            Act::FamilyBtn(sect) => {
                self.dropdown = if self.dropdown == Some(Dropdown::Family(sect)) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    self.list_scroll.reset();
                    Some(Dropdown::Family(sect))
                };
            }
            Act::WeightBtn(sect) => {
                self.dropdown = if self.dropdown == Some(Dropdown::Weight(sect)) {
                    None
                } else {
                    self.dropdown_since = Some(Instant::now());
                    self.list_scroll.reset();
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
        // The SAVE AS prompt owns the keyboard while it stands: Enter
        // saves, Esc closes, everything else is the field's. Purely
        // keyboard-driven — the field needs no focus bookkeeping because
        // nothing else can hear a key while it is open.
        if self.naming.is_some() {
            use nacelle::object::text_input::{self, InputEdited, InputMsg};
            if ev.mods == Mods::NONE && ev.key == FKey::Escape {
                self.naming = None;
                return KeyOut::Consumed;
            }
            if ev.mods == Mods::NONE && ev.key == FKey::Enter {
                let name = self
                    .naming
                    .as_ref()
                    .map(|m| m.value().trim().to_string())
                    .unwrap_or_default();
                if name.is_empty() {
                    return KeyOut::Consumed;
                }
                return if self.editor_save_named(&name) {
                    KeyOut::Changed
                } else {
                    KeyOut::Consumed
                };
            }
            if let Some(msg) = text_input::key_msg(ev) {
                let out = self.naming.as_mut().map(|m| m.apply(msg));
                match out {
                    Some(InputEdited::CopyRequest { text, .. }) => {
                        nacelle::clipboard::store(nacelle::clipboard::Board::Clipboard, &text);
                    }
                    Some(InputEdited::PasteRequest) => {
                        if let Some(text) =
                            nacelle::clipboard::load(nacelle::clipboard::Board::Clipboard)
                        {
                            let text: String = text
                                .chars()
                                .filter(|&c| Self::theme_name_char(c))
                                .collect();
                            if let Some(m) = self.naming.as_mut() {
                                m.apply(InputMsg::Insert(text));
                            }
                        }
                    }
                    _ => {}
                }
            }
            return KeyOut::Consumed;
        }
        // Bare only: the same rule `Nav::of` applies to the arrows —
        // a modified key is a shortcut's business, never navigation.
        if ev.mods == Mods::NONE
            && matches!(ev.key, FKey::PageUp | FKey::PageDown | FKey::Home | FKey::End)
        {
            let (viewport, length) = (self.flow.view.h, self.flow.length);
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
                self.chase_focus(fc, self.flow);
                KeyOut::Consumed
            }
        }
    }

    /// After a keyboard move: bring what it landed on back into the
    /// frame (M5).
    ///
    /// The other half of registering off screen. The chain is whole in
    /// every frame now, so Tab can land on a row the page is not
    /// showing; without this the ring would simply be gone, and the row
    /// would be neither seen nor pressable — [`Settings::focused_act`]
    /// reads the hit map, and an unseen row is not in it.
    ///
    /// The rect is the LAST COMPLETED frame's, so the travel is measured
    /// from the offset that frame was drawn at and not from wherever the
    /// offset has since got to; the clamp is the next tick's, as it is
    /// for every other way this window moves.
    ///
    /// What the scroll does not carry is not chased, and the frame said
    /// which that is rather than the geometry being asked to guess
    /// ([`Settings::flowed`]): the corner button, a pinned bar and the
    /// rows of an open list all stand over the flow's own lane and none
    /// of them moves with it — chasing them would carry the page off
    /// under something that had not moved. The navigation is in the
    /// ledger exactly when the window has folded, which is exactly when
    /// its entries scroll with everything else.
    fn chase_focus(&mut self, fc: &FocusCtl, flow: Flow) {
        let Some(id) = fc.focused() else { return };
        if !self.flowed.contains(&id) {
            return;
        }
        let Some(r) = fc.rect_of(id) else { return };
        let view = flow.view;
        // A rect taller than the frame lands on its TOP edge: the first
        // branch wins, and reading starts at the top of a thing.
        let travel = if r.y < view.y {
            r.y - view.y
        } else if r.bottom() > view.bottom() {
            r.bottom() - view.bottom()
        } else {
            return;
        };
        self.scroll.set_offset(flow.offset + travel);
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
        let v = get(self) as i64 + dir as i64 * step(self) as i64;
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

    /// Refreshes the selection highlights from nacelle-desktop.ron:
    /// the engine's theme (`theme:`), the layout (`layaut:`) and the
    /// sound set (`sounds:`), each falling back to "default" when unset.
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
    /// Every field is REMOVED from the file rather than written empty,
    /// and that is the whole correctness of this control: an empty
    /// value wins the cascade and pins a setting off, so a reset made
    /// of empties would block the system defaults instead of letting
    /// them back in. It worked only because no system file existed to
    /// be blocked. [`config::clear_look_and_feel`] removes them — the
    /// theme, the variant, the layaut and every per-screen assignment,
    /// the sound set, both font sections and the panel gutter — in ONE
    /// write, so no half reset is ever on disk.
    ///
    /// The pinned `[WxH@D]` section is the application's to clear —
    /// only it knows which screen this window is on — so it is asked
    /// for, exactly as it was when this control cleared nothing else.
    fn reset_look_and_feel(&mut self) {
        config::clear_look_and_feel();
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
        let (chrome_act, chrome_label) = match chrome_of(page.view) {
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
        // The navigation comes next, and the page after it: the chain is
        // corner button, rail, the section's pages, the page. Where the
        // window has folded the columns are empty and the same entries
        // are the first bands of the flow instead — same order, one
        // shape fewer.
        let nav = Panes::of(page.view, m, content);
        // The beds first, under everything: three shades of one colour
        // where the window has its columns, one bed where it has folded.
        self.draw_bands(ctx, &nav);
        self.draw_nav(ctx, page, m, &nav);
        self.draw_body(ctx, page, m, content);
        self.button_drawn(ctx, corner, chrome_label, chrome_act, Some(ring));
        // Last, so it covers what it hangs from and the reverse hit walk
        // reaches its rows first.
        self.draw_open_dropdown(ctx, m);
        self.draw_naming(ctx);
    }

    /// The SAVE AS prompt, over everything: a scrim, a box, one field and
    /// the two-key hint. The same tokens the layout editor's prompt reads,
    /// so the two ask the theme one set of questions.
    fn draw_naming(&mut self, ctx: &mut Ctx) {
        static PAD: OnceLock<TokenId> = OnceLock::new();
        static FIELD_H: OnceLock<TokenId> = OnceLock::new();
        static BODY_TOP: OnceLock<TokenId> = OnceLock::new();
        static HINT_INSET: OnceLock<TokenId> = OnceLock::new();
        static HINT_C: OnceLock<TokenId> = OnceLock::new();
        static SCRIM: OnceLock<TokenId> = OnceLock::new();
        static SCRIM_A: OnceLock<TokenId> = OnceLock::new();
        if self.naming.is_none() {
            return;
        }
        let t = theme::resolved();
        let win = modal_rect(ctx.w, ctx.h);
        // The prompt claims the whole window: hover under it dies with the
        // clicks `click()` already swallows, through the one pointer model.
        ctx.mouse.cover(win);
        let mut scrim = col(t.color(tok(&SCRIM, "component.modal.scrim")));
        scrim.a *= t.px(tok(&SCRIM_A, "modal.scrim_alpha")).clamp(0.0, 1.0);
        ctx.dl.rect(win.x, win.y, win.w, win.h, scrim);
        let pad = t.px(tok(&PAD, "modal.pad")).max(0.0);
        let fh = t.px(tok(&FIELD_H, "field.h")).max(1.0);
        let top = t.px(tok(&BODY_TOP, "modal.body_top")).max(0.0);
        // The same species as the layout editor's SAVE AS prompt: widths
        // from modal.*, height from dialog.* (editor.rs reads the same
        // pair), boxed into the window that opened it.
        static W_FRAC: OnceLock<TokenId> = OnceLock::new();
        static W_MIN: OnceLock<TokenId> = OnceLock::new();
        static W_MIN_PX: OnceLock<TokenId> = OnceLock::new();
        static H_FRAC: OnceLock<TokenId> = OnceLock::new();
        static H_MIN: OnceLock<TokenId> = OnceLock::new();
        let bw = (ctx.w * t.px(tok(&W_FRAC, "modal.w_frac")))
            .max(t.px(tok(&W_MIN, "modal.min_w")))
            .max(t.px(tok(&W_MIN_PX, "modal.min_w_min_px")))
            .min(win.w - 2.0 * pad);
        let bh = (ctx.h * t.px(tok(&H_FRAC, "dialog.h_frac")))
            .max(t.px(tok(&H_MIN, "dialog.h_min_px")))
            .min(win.h - 2.0 * pad);
        let bx = win.x + (win.w - bw) / 2.0;
        let by = win.y + (win.h - bh) / 2.0;
        let box_ = Rect::new(bx, by, bw, bh);
        nacelle::object::window::frame(ctx, box_);
        static TITLE_FG: OnceLock<TokenId> = OnceLock::new();
        let title_px = role_title(ctx).px;
        ctx.dl.module_title(
            ctx.fonts,
            bx + pad,
            by + pad,
            bw - 2.0 * pad,
            title_px,
            "SAVE THEME AS",
            "",
            col(t.color(tok(&TITLE_FG, "component.panel.title"))),
            true,
        );
        let field = Rect::new(bx + pad, by + top, (bw - 2.0 * pad).max(2.0), fh);
        if let Some(model) = self.naming.as_mut() {
            use nacelle::object::text_input::{self, InputStyle};
            let (mx, my) = ctx.mouse.at();
            text_input::draw(
                ctx,
                field,
                model,
                FocusId::of("settings.editor.naming"),
                &InputStyle {
                    placeholder: "theme name",
                    hover: field.contains(mx, my),
                    disabled: false,
                    focused_fallback: true,
                },
            );
        }
        let hint = role_hint(ctx);
        ctx.dl.text_center(
            ctx.fonts,
            hint.face,
            hint.px,
            bx + bw / 2.0,
            field.bottom() + t.px(tok(&HINT_INSET, "settings.hint_inset")),
            "ENTER SAVES \u{2014} ESC CANCELS",
            col(t.color(tok(&HINT_C, "text.muted"))),
            hint.track * hint.px,
        );
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
    /// `content` is the WHOLE content box; the navigation is taken out
    /// of it here, so every caller — the drawing, the scroll, the tests
    /// — asks one question and gets the box the flow really has.
    fn body_box(&self, page: &'static Page, m: Metrics, content: Rect) -> Rect {
        let box_ = Panes::of(page.view, m, content).page;
        // The box is the FULL one — the clip and the bar are drawn on it
        // — but what a pinned band COSTS is measured where its rows
        // stand, which is beside the bar's lane ([`rows_box`]).
        let rows = rows_box(box_);
        let top = body_top(page, m, box_);
        let mut bottom = box_.bottom();
        for zone in page.zones {
            if matches!(zone, Zone::Pinned { .. }) {
                bottom -= m.gap + self.zone_h(zone, m, rows);
            }
        }
        Rect::new(box_.x, top, box_.w, (bottom - top).max(0.0))
    }

    /// The bands that flow this frame, in registration order.
    ///
    /// Unfolded that is the page's own bands and nothing else — the
    /// navigation stands in its columns, outside the scroll. Folded, the
    /// rail and the section's pages come FIRST, as ordinary bands ahead
    /// of the page: one list, one scroll, and the same order the two
    /// columns would have registered them in.
    fn frame_zones(&self, page: &'static Page, nav: &Panes) -> Vec<&'static Zone> {
        let mut out: Vec<&'static Zone> = Vec::with_capacity(page.zones.len() + 2);
        if nav.folded {
            out.push(&RAIL_ZONE);
            out.extend(subrail_zone(page.view));
        }
        // A band whose `when` says no is not in this frame at all — the
        // theme editor's other page. This is the ONE place the flow, the
        // length, the chain and the hit map all read, so the mode cannot
        // be showing on one of them and hidden on another.
        out.extend(page.zones.iter().filter(|z| zone_shown(z, self)));
        out
    }

    /// Where inside its band each of a band's regions starts, measured
    /// from the band's own top edge.
    ///
    /// Zero for every column standing beside its neighbours; the running
    /// sum of the ones before it once the band has folded, `modal.row_gap`
    /// apart — which is the space every one of those rows already leaves
    /// under itself, so a folded band is EXACTLY the list the page was.
    fn zone_offsets(&self, zone: &'static Zone, m: Metrics, box_: Rect) -> Vec<f32> {
        let regions = zone_regions(zone, box_);
        if !zone_folded(zone, box_) {
            return vec![0.0; regions.len()];
        }
        let mut out = Vec::with_capacity(regions.len());
        let mut y = 0.0;
        for (region, _, rows) in &regions {
            out.push(y);
            y += self.rows_h(rows, m, *region) + m.gap;
        }
        out
    }

    /// How tall one run of rows stands: every page's own arithmetic
    /// since there were pages, asked of a band's rows instead of a
    /// page's. The last row's trailing gap is not content.
    fn rows_h(&self, rows: &'static [Row], m: Metrics, region: Rect) -> f32 {
        let mut h = 0.0;
        let mut trailing = 0.0;
        for row in rows {
            if !(row.when)(self) {
                continue;
            }
            h += self.row_h(&row.ctrl, m, region) + m.space(row.after);
            trailing = m.space(row.after);
        }
        (h - trailing).max(0.0)
    }

    /// How tall a band stands: its rows, or — where it has columns — the
    /// deepest its columns reach. A columned band is as deep as the
    /// deepest thing in it, which is why a band whose right column grows
    /// with `Row::when` grows with it.
    ///
    /// One expression for both shapes: a column that stands beside its
    /// neighbours starts at zero and the deepest wins, and a folded one
    /// starts under the one before it, so the last is the deepest by
    /// construction. Two arithmetics here would be two chances for the
    /// height and the drawing to disagree about where a band ends.
    fn zone_h(&self, zone: &'static Zone, m: Metrics, box_: Rect) -> f32 {
        zone_regions(zone, box_)
            .into_iter()
            .zip(self.zone_offsets(zone, m, box_))
            .map(|((region, _, rows), dy)| dy + self.rows_h(rows, m, region))
            .fold(0.0, f32::max)
    }

    /// How tall the flowed bands stand together — the scroll's content
    /// length. `settings.zone_gap` between two bands, and nothing after
    /// the last one: a page ends at its last row, not at the space it
    /// asked for after it.
    ///
    /// A band with nothing in it takes no break either. `content` is the
    /// whole content box, exactly as [`Settings::body_box`] takes it —
    /// the navigation is taken out of it here too, and the folded
    /// window's navigation bands are part of this length.
    ///
    /// Measured in [`rows_box`], which is where the drawing puts the
    /// rows: a band folds on its width, so a length taken at the full
    /// box would be a length of a page nobody is looking at.
    fn flow_h(&self, page: &'static Page, m: Metrics, content: Rect) -> f32 {
        let nav = Panes::of(page.view, m, content);
        let box_ = rows_box(nav.page);
        let mut h = 0.0;
        for zone in self.frame_zones(page, &nav) {
            if matches!(zone, Zone::Pinned { .. }) {
                continue;
            }
            let zh = self.zone_h(zone, m, box_);
            if zh <= 0.0 {
                continue;
            }
            if h > 0.0 {
                h += zone_gap();
            }
            h += zh;
        }
        h
    }

    /// The one walker: every band of the page, and inside a band every
    /// row, placed and drawn in the order the page lists them. Nothing
    /// here knows which page it is walking — that is the whole point of
    /// the description.
    ///
    /// The flowed bands run inside [`Settings::body_box`] and under its
    /// clip; the pinned bands are placed against the content box
    /// afterwards, outside it, which is why the flow can no longer meet
    /// them.
    fn draw_body(&mut self, ctx: &mut Ctx, page: &'static Page, m: Metrics, content: Rect) {
        // The navigation is taken out of the content box FIRST: the
        // scrollbar's lane belongs to the PAGE's box, not to the
        // window's, or the rail would be pushed over by a bar that is
        // nowhere near it. The FULL box survives for the clip, the
        // scroll's span and the bar; only the rows stand beside the lane
        // ([`rows_box`]), and the bar is drawn against the original edge
        // — otherwise it would hug the narrowed edge and stand over the
        // rows again, just from the other side of its own lane.
        let nav = Panes::of(page.view, m, content);
        let rows_box = rows_box(nav.page);
        // Both of these take the WHOLE content box and split it again —
        // one split, stated in [`Panes::of`], so a test that measures a
        // page and the frame that draws it cannot answer differently.
        let view = self.body_box(page, m, content);
        let length = self.flow_h(page, m, content);
        let zones = self.frame_zones(page, &nav);
        // The offset, its clamp, its physics and its bar are the
        // toolkit's (`view::scroll`); the wheel, the page keys and the
        // thumb all move this one number. `Snap::None` because the clip
        // is real — only a surface that cannot clip has to land on whole
        // rows to avoid painting half of one.
        self.now = ctx.t;
        self.scroll.tick(ctx.t, view.h, length, Snap::None, &ScrollPhysics::from_theme());
        let off = self.scroll.offset();
        // What the keyboard reads back between frames ([`Flow`]).
        self.flow = Flow { view, length, offset: off };

        ctx.dl.push_clip(view.x, view.y, view.w, view.h);
        self.clip = Some(view);
        // This frame's ledger of what the scroll answers for.
        self.flowed.clear();
        let mut y = view.y - off;
        let mut started = false;
        for zone in &zones {
            if matches!(zone, Zone::Pinned { .. }) {
                continue;
            }
            let zh = self.zone_h(zone, m, rows_box);
            if zh <= 0.0 {
                continue;
            }
            if started {
                y += zone_gap();
            }
            started = true;
            self.draw_zone(ctx, zone, m, rows_box, y, Some(view), true);
            y += zh;
        }
        self.clip = None;
        ctx.dl.pop_clip();

        // The pinned bands stack up from the bottom edge, the last
        // declared one lowest, with the same break between them that
        // [`Settings::body_box`] reserved.
        let mut anchor = rows_box.bottom();
        for zone in zones.iter().rev() {
            if !matches!(zone, Zone::Pinned { .. }) {
                continue;
            }
            let zh = self.zone_h(zone, m, rows_box);
            self.draw_zone(ctx, zone, m, rows_box, anchor - zh, None, false);
            anchor -= zh + m.gap;
        }
        self.draw_scrollbar(ctx, view, length);
    }

    /// The three columns' beds: ONE COLOUR OF THE THEME'S AT THREE OF
    /// ITS SHADES, which is the owner's ask in his own words — "hue ten
    /// sam, odcień koloru inny".
    ///
    /// Nothing here decides what those shades are. The three tokens are
    /// [component]'s, they point at three rungs of the surface ladder,
    /// and the ladder is one hue at six lightnesses — so the difference
    /// between the bands is a SHADE by construction and a theme that
    /// re-points one band re-points only that one. Naming a rung here
    /// instead would weld the settings columns to the desktop field and
    /// no theme could ever part them again.
    ///
    /// Painted BEFORE the navigation and the body, and over each
    /// column's whole rectangle exactly as [`Panes`] cut it: the beds
    /// are the ground everything else in the window stands on.
    ///
    /// FOLDED, this is one band and not three. There are no columns
    /// below `settings.col_min_w` — the rail's rows become the first
    /// bands of one vertical flow — so `page` is the whole interior and
    /// `rail`/`sub` are nothing at all.
    ///
    /// AND THE PAGE HAS NO BED OF ITS OWN: the WINDOW BODY is it. The
    /// two columns are the DEVIATIONS from the rung the window already
    /// stands on, and a deviation is the only thing there is to paint.
    ///
    /// WHY, MEASURED. The rung `component.panel.fill` names — what
    /// `window::frame` lays — is TRANSLUCENT (`@surface.panel`, alpha
    /// 0.82). A rectangle of it over the body composes the alpha a
    /// second time and the page stops matching the window it is in:
    /// over the field the window stands on, the body's own pixel is
    /// #131E19 and the doubled one #15201B, an OKLab dE of 0.0078 —
    /// small, plainly visible as a lighter panel, and larger on a theme
    /// whose backdrop is further from the rung.
    ///
    /// AND THE TWO WORSE CASES, which is why this is not a comparison
    /// of the two colours with a paint when they differ:
    ///
    /// * GLASS. Where `elev.panel.glass.rank` lifts the body off its
    ///   fill altogether the body is a blur, and ANY bed over it — the
    ///   same rung or another — is the end of the blur the BACKGROUND
    ///   section just put there.
    /// * A MOVED BODY. The editor's SOLID writes `component.panel.fill`
    ///   (`edit::glass_edits`), and a settings token copying that rung
    ///   would not follow it — so a rule that paints when the two
    ///   differ would put the OLD rung back over the colour just
    ///   chosen.
    ///
    /// The body is the page's bed under every one of those, because it
    /// is the same surface and not a copy of it. The master says so in
    /// its own voice: `[component] settings` names the rail's band and
    /// the sub-page column's, and NOT the page's, because to re-shade
    /// the page an author moves `panel.fill` — the body the page is
    /// part of. Two names, three bands, one surface.
    fn draw_bands(&self, ctx: &mut Ctx, nav: &Panes) {
        static RAIL_FILL: OnceLock<TokenId> = OnceLock::new();
        static SUB_FILL: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        let bands = [
            (nav.rail, &RAIL_FILL, "component.settings.rail_fill"),
            (nav.sub, &SUB_FILL, "component.settings.sub_fill"),
        ];
        for (box_, cell, name) in bands {
            let Some(r) = box_ else { continue };
            ctx.dl.rect(r.x, r.y, r.w, r.h, col(th.color(tok(cell, name))));
        }
    }

    /// The two navigation columns, where the window has not folded.
    ///
    /// Each is clipped to its own box — a rail longer than the window is
    /// cut off, not painted over the page — and each is walked by the
    /// SAME row walker the pages are, so an entry is a button, a heading
    /// is a heading and a disabled section is grey by exactly the rules
    /// a setting is.
    ///
    /// Drawn before the body, so the chain runs corner button, rail,
    /// section pages, page — reading order, and the same order the
    /// folded window registers them in.
    fn draw_nav(&mut self, ctx: &mut Ctx, page: &'static Page, m: Metrics, nav: &Panes) {
        let columns = [
            (nav.rail, Some(&RAIL_ROWS[..])),
            (nav.sub, subrail_rows(page.view)),
        ];
        for (box_, rows) in columns {
            let (Some(box_), Some(rows)) = (box_, rows) else { continue };
            ctx.dl.push_clip(box_.x, box_.y, box_.w, box_.h);
            self.clip = Some(box_);
            // Not the flow's: a column stands where it stands, and the
            // page's scroll is no use to it ([`Settings::chase_focus`]).
            self.draw_rows(ctx, Cols::None, rows, m, box_, box_.y, Some(box_), false);
            self.clip = None;
            ctx.dl.pop_clip();
        }
    }

    /// One band, at the top edge it was given. A flow lays its rows in
    /// the whole box; a columned band lays each column's rows in that
    /// column's box — beside one another from the one top edge, or, once
    /// the band has folded, one under the other down the whole width
    /// ([`Settings::zone_offsets`]).
    ///
    /// `cull` is the viewport a flowed band is held to; a pinned band
    /// passes `None`, because it stands outside the clip and is always
    /// on screen. `flowed` says whether the band is one the SCROLL
    /// carries — the ledger the chase reads ([`Settings::flowed`]).
    fn draw_zone(
        &mut self,
        ctx: &mut Ctx,
        zone: &'static Zone,
        m: Metrics,
        box_: Rect,
        top: f32,
        cull: Option<Rect>,
        flowed: bool,
    ) {
        let offsets = self.zone_offsets(zone, m, box_);
        for ((region, cols, rows), dy) in zone_regions(zone, box_).into_iter().zip(offsets)
        {
            self.draw_rows(ctx, cols, rows, m, region, top + dy, cull, flowed);
        }
    }

    /// One run of rows, from `top` downwards inside `region`.
    ///
    /// `region` is what the rows measure and align against — its x and
    /// width are the column's, its height the page's — and it is what
    /// each row is handed as its content box, so a slider in the left
    /// column ends at the left column's right edge and not at the page's.
    fn draw_rows(
        &mut self,
        ctx: &mut Ctx,
        cols: Cols,
        rows: &'static [Row],
        m: Metrics,
        region: Rect,
        top: f32,
        cull: Option<Rect>,
        flowed: bool,
    ) {
        // Measured for THIS region: the sliders of one column do not
        // inherit the label width of the next (M3).
        let (label_w, value_w) = self.columns(ctx, cols, region.w);
        let mut y = top;
        for row in rows {
            if !(row.when)(self) {
                continue;
            }
            let h = self.row_h(&row.ctrl, m, region);
            let band = Rect::new(region.x, y, region.w, h);
            // A row wholly off the viewport is not DRAWN and is not a
            // TARGET — what the eye cannot see the hand cannot press —
            // but it keeps its place in the Tab order all the same (M5).
            let on_screen =
                cull.map_or(true, |v| band.bottom() > v.y && band.y < v.bottom());
            let rc = RowCtx { content: region, band, label_w, value_w, m };
            // R6: a row the page turned off registers nothing at all,
            // on screen or off it.
            if !(row.enabled)(self) {
                if on_screen {
                    self.draw_disabled(ctx, &row.ctrl, rc);
                }
            } else {
                // What the row offers, asked ONCE: the off-frame
                // registration places it, and a band the scroll carries
                // writes it into the ledger the chase reads.
                let targets = (flowed || !on_screen)
                    .then(|| self.targets(ctx, &row.ctrl, rc))
                    .unwrap_or_default();
                if on_screen {
                    self.draw_row(ctx, &row.ctrl, rc);
                } else {
                    self.register_offscreen(ctx, &row.ctrl, &targets);
                }
                if flowed {
                    self.flowed.extend(targets.iter().map(|&(_, a)| focus_id(a)));
                }
            }
            y += h + m.space(row.after);
        }
    }

    /// Everything one control offers, and where it stands: the ONE
    /// enumeration of a control's targets, in the order it registers
    /// them.
    ///
    /// The rects are the drawing's own — [`track_rect`], [`chip_rects`],
    /// [`cycle_rect`], [`Settings::button_rect`], [`Settings::bar_plates`]
    /// — so what this answers and what the frame paints cannot come
    /// apart. The three rows that carry no act (a heading, an aside, a
    /// hint) offer nothing, and neither does the BOARDS cross: what is
    /// inside it no row describes, and it registers its own tiles where
    /// it draws them.
    fn targets(
        &self,
        ctx: &mut Ctx,
        ctrl: &'static Ctrl,
        rc: RowCtx,
    ) -> Vec<(Rect, Act)> {
        match ctrl {
            Ctrl::Toggle { act, .. } => vec![(rc.band, *act)],
            Ctrl::Slider { act, .. } => vec![(track_rect(rc), *act)],
            Ctrl::Chips { values, act, .. } => chip_rects(values.len(), rc)
                .into_iter()
                .zip(values.iter())
                .map(|(r, bits)| (r, act(*bits)))
                .collect(),
            Ctrl::Cycle { act, .. } => vec![(cycle_rect(rc), *act)],
            Ctrl::Drop { list } => {
                vec![(Self::button_rect(BtnKind::Wide, rc), Act::ListBtn(*list))]
            }
            Ctrl::Button { kind, act, .. } => {
                vec![(Self::button_rect(*kind, rc), *act)]
            }
            Ctrl::Bar { items } => self
                .bar_plates(ctx, items, rc)
                .into_iter()
                .map(|(r, _, act)| (r, act))
                .collect(),
            Ctrl::Section { .. }
            | Ctrl::Note { .. }
            | Ctrl::Hint { .. }
            | Ctrl::Custom { .. } => Vec::new(),
        }
    }

    /// A row the frame did not draw, taking its place in the chain from
    /// the rect the LAYOUT gave it (M5).
    ///
    /// Off screen used to mean off the Tab order as well, and in one
    /// vertical list that was defensible: the route lost its tail and
    /// found it again as the page scrolled. With bands of columns it
    /// cuts the route in the middle — a short column's rows come back
    /// while a tall one's are still gone — so Tab would skip about
    /// according to how far the page happened to be scrolled. The route
    /// is the DESCRIPTION's, not the geometry's, so it is whole in every
    /// frame now, and the scroll goes and gets whatever the keyboard
    /// lands on ([`Settings::chase_focus`]).
    ///
    /// No ink and no target: nothing is drawn here, and the hit map is
    /// not touched — [`hit_into`] goes on trimming every rect to the
    /// clip, so what the eye cannot see the hand still cannot press.
    fn register_offscreen(
        &mut self,
        ctx: &mut Ctx,
        ctrl: &'static Ctrl,
        targets: &[(Rect, Act)],
    ) {
        // A slider owns its arrows wherever it stands
        // (`object::slider::track_focusable`); every other control of
        // this window leaves them to the chain.
        let caps = match ctrl {
            Ctrl::Slider { .. } => Caps::GREEDY_ARROWS,
            _ => Caps::NONE,
        };
        for &(r, act) in targets {
            if let Some(fc) = ctx.focus.as_deref_mut() {
                fc.register(focus_id(act), r, caps);
            }
        }
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
        let hovered = ctx.mouse.over(band);
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

    /// A REGION's label and value columns, in px.
    ///
    /// Asked once per band, and once per column of a columned band: the
    /// widest label in the BLOCK, not on the page (`rhythm.label_col`).
    /// `w` is the region's width, so a fraction is a fraction of the
    /// column and not of the whole content box.
    fn columns(&self, ctx: &mut Ctx, cols: Cols, w: f32) -> (f32, f32) {
        let th = theme::resolved();
        match cols {
            Cols::None => (0.0, 0.0),
            // rhythm.label_col = auto needs a measuring column primitive
            // before this fraction can go.
            Cols::Frac => (
                w * th.px(tok(&LABEL_COL, "rhythm.label_col_frac")).clamp(0.0, 1.0),
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
            // A bar is ONE row however many verbs it carries.
            Ctrl::Button { .. } | Ctrl::Drop { .. } | Ctrl::Bar { .. } => m.btn_h,
            Ctrl::Section { .. } => m.block_h,
            Ctrl::Note { .. } => m.note_h,
            Ctrl::Hint { .. } => m.hint_h,
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
                let hover = ctx.mouse.over(rc.band);
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
                let track = track_rect(rc);
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
            Ctrl::Bar { items } => self.draw_bar(ctx, items, rc),
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

    /// A row the page turned off. Only the two kinds of row made of
    /// buttons have a disabled form — the ladder's Disabled rung, an
    /// inscription, and nothing in the hit map or the focus chain (R6).
    fn draw_disabled(&mut self, ctx: &mut Ctx, ctrl: &Ctrl, rc: RowCtx) {
        let plates: Vec<(Rect, Cow<'static, str>)> = match ctrl {
            Ctrl::Button { label, kind, .. } => {
                vec![(Self::button_rect(*kind, rc), self.text_of(*label))]
            }
            Ctrl::Bar { items } => self
                .bar_plates(ctx, items, rc)
                .into_iter()
                .map(|(r, label, _)| (r, label))
                .collect(),
            _ => return,
        };
        let th = theme::resolved();
        let st = ladder(th, &BTN_CLASS, "button", State::Disabled);
        let f = role_button(ctx);
        for (r, s) in plates {
            ctx.dl.rect_outline(r.x, r.y, r.w, r.h, st.edge_width, col(st.edge));
            let ty = center_y(ctx, r, f);
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

    /// Where a bar's plates go, and what each says. Asked by the drawing
    /// and by the disabled form, so a bar cannot be laid out twice.
    ///
    /// Each plate is its own label wide — `button.pad_x` on both sides,
    /// never under `button.min_w` — because three verbs of three
    /// lengths in three equal boxes would put most of the ink in the
    /// widest one. The row is centred as a whole, `settings.bar_gap`
    /// between plates; no length here is this file's.
    fn bar_plates(
        &self,
        ctx: &mut Ctx,
        items: &'static [(Text, Act)],
        rc: RowCtx,
    ) -> Vec<(Rect, Cow<'static, str>, Act)> {
        static BAR_GAP: OnceLock<TokenId> = OnceLock::new();
        static PAD_X: OnceLock<TokenId> = OnceLock::new();
        static MIN_W: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        let gap = th.px(tok(&BAR_GAP, "settings.bar_gap"));
        let pad = th.px(tok(&PAD_X, "button.pad_x"));
        let min_w = th.px(tok(&MIN_W, "button.min_w"));
        let f = role_button(ctx);
        // Resolved before anything is drawn: a label may be read from
        // the window, and the window cannot be borrowed while it draws.
        let labels: Vec<Cow<'static, str>> =
            items.iter().map(|(t, _)| self.text_of(*t)).collect();
        let widths: Vec<f32> = labels
            .iter()
            .map(|s| (ctx.fonts.measure(f.face, f.px, s, f.track) + 2.0 * pad).max(min_w))
            .collect();
        let total: f32 =
            widths.iter().sum::<f32>() + gap * items.len().saturating_sub(1) as f32;
        let mut x = rc.content.x + (rc.content.w - total) / 2.0;
        let mut out = Vec::with_capacity(items.len());
        for (i, (_, act)) in items.iter().enumerate() {
            out.push((
                Rect::new(x, rc.band.y, widths[i], rc.m.btn_h),
                labels[i].clone(),
                *act,
            ));
            x += widths[i] + gap;
        }
        out
    }

    /// A row of buttons, registered left to right.
    fn draw_bar(&mut self, ctx: &mut Ctx, items: &'static [(Text, Act)], rc: RowCtx) {
        for (r, label, act) in self.bar_plates(ctx, items, rc) {
            self.button(ctx, r, &label, act);
        }
    }

    fn button_rect(kind: BtnKind, rc: RowCtx) -> Rect {
        let listed_w = (rc.content.w * rc.m.list_frac).min(rc.content.w);
        let x = rc.content.x + (rc.content.w - listed_w) / 2.0;
        match kind {
            BtnKind::Listed => Rect::new(x, rc.band.y, listed_w, rc.m.btn_h),
            BtnKind::Wide => {
                Rect::new(rc.content.x, rc.band.y, rc.content.w, rc.m.btn_h)
            }
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
        let cur = get(self);
        for (bits, r) in values.iter().zip(chip_rects(values.len(), rc)) {
            let hover = ctx.mouse.over(r);
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
        let r = cycle_rect(rc);
        let hover = ctx.mouse.over(r);
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
            ListId::Borders => &self.border_kinds,
            ListId::Backgrounds => &self.background_kinds,
            ListId::Severities => &self.severity_kinds,
            ListId::Corners => &self.corner_kinds,
            ListId::RingStyles => &self.ring_style_kinds,
            ListId::ScrollModes => &self.scroll_mode_kinds,
            ListId::ScrollEdges => &self.scroll_edge_kinds,
        }
    }

    /// The name the configuration carries for one list.
    fn current_of(&self, list: ListId) -> Option<&String> {
        match list {
            ListId::Looks => self.current_look.as_ref(),
            ListId::Layauts => self.current_layaut.as_ref(),
            ListId::Sounds => self.current_sounds.as_ref(),
            ListId::Borders => self.current_border.as_ref(),
            ListId::Backgrounds => self.current_background.as_ref(),
            ListId::Severities => self.current_severity.as_ref(),
            ListId::Corners => self.current_corner.as_ref(),
            ListId::RingStyles => self.current_ring_style.as_ref(),
            ListId::ScrollModes => self.current_scroll_mode.as_ref(),
            ListId::ScrollEdges => self.current_scroll_edge.as_ref(),
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
        // No list drawn, no bar to press: the record below is one
        // frame's, refreshed by `draw_dropdown` when a scrolling list
        // stands.
        self.list_bar = None;
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
            hover: ctx.mouse.over(r),
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
    /// The addon settings report: one line per file the program could
    /// not use, or the one line that says so when there is nothing to
    /// report.
    ///
    /// A row cannot describe it because how many lines it has is a fact
    /// about the machine — nought on almost every one, two per broken
    /// file on the one that matters. It registers nothing: there is no
    /// control here, only text, and the files behind it are edited in an
    /// editor.
    ///
    /// What does not fit is COUNTED rather than dropped. A report that
    /// silently stopped at the bottom of the box would be this whole
    /// hole again one page further in — the user would read four broken
    /// files, fix four, and still have a widget on its defaults.
    fn draw_addon_report(&mut self, ctx: &mut Ctx, area: Rect) {
        let th = theme::resolved();
        let ink = col(th.color(tok(&MUTED_FG, "text.muted")));
        if self.addon_report.is_empty() {
            let v = role_empty(ctx);
            let y = center_y(ctx, area, v);
            ctx.dl.text_center(
                ctx.fonts,
                v.face,
                v.px,
                area.cx(),
                y,
                ADDONS_ALL_CLEAR,
                ink,
                v.track,
            );
            return;
        }
        let n = role_note(ctx);
        let step = n.line();
        // At least one line however short the box is: a report that
        // rounded down to nothing would be the silence again.
        let fits = ((area.h / step).floor() as usize).max(1);
        let total = self.addon_report.len();
        for (i, line) in self.addon_report.iter().enumerate() {
            if i >= fits {
                break;
            }
            let band = Rect::new(area.x, area.y + step * i as f32, area.w, step);
            let last = i + 1 == fits && total > fits;
            let text: Cow<'_, str> = if last {
                Cow::Owned(format!(
                    "\u{2026} AND {} MORE \u{2014} ALL OF THEM ON STDERR",
                    total - i
                ))
            } else {
                Cow::Borrowed(line.as_str())
            };
            let y = center_y(ctx, band, n);
            ctx.dl.text(ctx.fonts, n.face, n.px, area.x, y, &text, ink, n.track);
        }
    }

    /// One of the two pages a row cannot describe (the other is
    /// [`Settings::draw_addon_report`]), and one of the two
    /// [`Ctrl::Custom`]s in the file.
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
            let hover = ctx.mouse.over(tile);
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
                let x_hot = ctx.mouse.over(xr);
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
            let hot = ctx.mouse.over(pr);
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
            &mut self.list_scroll,
        );
        for (i, (r, _full)) in rows.iter().copied().enumerate() {
            // AS DRAWN: the frame clips the body now and the accordion
            // reports what survived it, so an element scrolled out of
            // the frame comes back with no area and nothing to press —
            // the invisible-but-clickable tail this loop used to build
            // is gone with the toolkit's own contract.
            self.hits.push((r, make_act(i)));
        }
        // The frame the accordion just framed, restated for the BAR's
        // press: the object draws the bar and ticks the offset, but the
        // pointer is this window's, so the thumb needs the same area,
        // viewport and content the object used. The reads mirror
        // `object::dropdown::accordion` token for token — the seam gap,
        // the height cap, the skew and the anchor-width floor — because
        // the object does not hand its geometry back (the same
        // restatement `draw_scrollbar` makes for the page's own bar).
        {
            static GAP: OnceLock<TokenId> = OnceLock::new();
            static MAX_H_FRAC: OnceLock<TokenId> = OnceLock::new();
            static MAX_H_MIN_PX: OnceLock<TokenId> = OnceLock::new();
            static SKEW: OnceLock<TokenId> = OnceLock::new();
            static ANCHOR_W: OnceLock<TokenId> = OnceLock::new();
            static MIN_W: OnceLock<TokenId> = OnceLock::new();
            let t = theme::resolved();
            let gap = t.px(tok(&GAP, "menu.anchor_gap")).max(0.0);
            let content = (item_h + gap) * names.len() as f32;
            let cap = (ctx.h * t.px(tok(&MAX_H_FRAC, "menu.max_h_frac")))
                .max(t.px(tok(&MAX_H_MIN_PX, "menu.max_h_min_px")))
                .max(0.0);
            let body_h = content.min(cap);
            let mut bar_w = anchor.w - t.px(tok(&SKEW, "button.skew"));
            let aw = tok(&ANCHOR_W, "menu.anchor_width");
            if nacelle::theme::enum_word_of(aw).as_deref() == Some("min_w") {
                bar_w = bar_w.max(t.px(tok(&MIN_W, "menu.min_w")));
            }
            self.list_bar = (content > body_h + 0.5).then(|| {
                (Rect::new(anchor.x, anchor.bottom(), bar_w, body_h), body_h, content)
            });
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
        let hover = self.dropdown.is_none() && ctx.mouse.over(r);
        let flash = self.flashing(act);
        // An anchor whose list is unfolded is the one button on the
        // page that is switched ON, and the ladder already has the rung
        // for it: a list left open is a state the eye should see. The
        // navigation's own entry for the page in force stands on the
        // same rung, for the same reason and out of the same theme —
        // "where am I" is a STATE, and this window states no state in
        // Rust.
        let selected =
            self.dropdown.map_or(false, |d| anchor_act(d) == act) || self.nav_marks(act);
        nacelle::object::button::ButtonState { hover, flash, selected }
    }

    /// Whether an act is the navigation entry standing for the view in
    /// force: its section in the rail, and its own entry in the second
    /// column. Both can be true of one frame and of two different
    /// buttons — that is the point of two columns.
    fn nav_marks(&self, act: Act) -> bool {
        act == rail_act(self.view) || sub_act(self.view) == Some(act)
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
            // The section on the rail and the section's own page in the
            // column beside it: two entries that open ONE view, and the
            // pair a shared id would fold into a single chain position.
            Act::OpenLookFeel,
            Act::OpenSets,
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
            // The editor's controls, the pairs most likely to collide by
            // path: a knob against its section's list or switch, a track
            // colour against the track switch, the two width knobs of
            // the two floats.
            Act::EditorSave,
            Act::EditorSaveAs,
            Act::EditorCancel,
            Act::EditorTrack(Knob::EdgeL),
            Act::EditorTrack(Knob::AccentB),
            Act::EditorTrack(Knob::SurfHue),
            Act::EditorTrack(Knob::SevB),
            Act::EditorTrack(Knob::CornerSm),
            Act::EditorTrack(Knob::Hairline),
            Act::EditorTrack(Knob::RingW),
            Act::EditorTrack(Knob::RingH),
            Act::EditorTrack(Knob::HaloAlpha),
            Act::EditorTrack(Knob::UnfocusedDim),
            Act::EditorTrack(Knob::MenuEdgeW),
            Act::EditorTrack(Knob::TipEdgeW),
            Act::EditorTrack(Knob::BarW),
            Act::EditorTrack(Knob::BarTrackB),
            Act::EditorFlip(Flip::SurfaceOwnHue),
            Act::EditorFlip(Flip::Ring),
            Act::EditorFlip(Flip::Halo),
            Act::EditorFlip(Flip::BarAutoHide),
            Act::EditorFlip(Flip::BarTrack),
            Act::ListBtn(ListId::Borders),
            Act::ListBtn(ListId::Backgrounds),
            Act::ListBtn(ListId::Severities),
            Act::ListBtn(ListId::Corners),
            Act::ListBtn(ListId::RingStyles),
            Act::ListBtn(ListId::ScrollModes),
            Act::ListBtn(ListId::ScrollEdges),
            Act::Pick(ListId::Severities, 0),
            Act::Pick(ListId::Corners, 0),
            Act::Pick(ListId::RingStyles, 0),
            Act::Pick(ListId::ScrollModes, 0),
            Act::Pick(ListId::ScrollEdges, 0),
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
    /// Open list, then editor, and only then the window itself: the
    /// last step is the application's ([`KeyOut::Ignored`] is what
    /// `main.rs` turns into a close), so the window has to answer for
    /// the ones before it. From the editor, three levels in, one press
    /// used to land on the desktop.
    ///
    /// The ladder is one rung shorter than it was, and deliberately: a
    /// section the rail shows at all times has nothing to step back to,
    /// so Escape from it is the window's own last layer (owner,
    /// 2026-08-16, "Escape z sekcji zamyka okno"). What is still peeled
    /// is what the navigation does not list.
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
        // The last layer is the window itself, and that one is not
        // this window's to close.
        assert!(matches!(escape(&mut s), KeyOut::Ignored));
        assert!(s.open, "the window closed itself");
        assert!(s.view == View::LookFeel, "Escape moved off the section as well");
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
        // ONE term, and it is the toolkit's. There is no box any more
        // (libnacelle a449763: a drop-down is a column of anchor-dressed
        // elements on a blind, not a container with rows inside), so there
        // is no inner pad to add — the first element simply hangs
        // `menu.anchor_gap` under the door, the same air that stands
        // between every pair of elements below it.
        //
        // Written as the token and not as the 2.7 px it bakes to: a theme
        // that widens the air widens this gap, and a number here would turn
        // that into a failure instead of a following.
        let t = nacelle::theme::resolved();
        let px = |n: &str| t.px(nacelle::theme::id(n).unwrap_or_else(|| panic!("{n} must exist")));
        let want = px("menu.anchor_gap");
        assert!(
            (first.y - door.bottom() - want).abs() < 0.51,
            "the first theme hangs {} px under the door, not the {want} px of air",
            first.y - door.bottom()
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

    /// The page's three anchors are one column, and so is the section's
    /// own column of pages.
    ///
    /// FONTS used to be a `Listed` button — `settings.list_w_frac` of
    /// the content, centred — under three anchors that ran the full
    /// width, and it read as a different class of control although it
    /// is the same kind of thing: another way into the same subject.
    /// It is a navigation entry now, so the rule it has to keep is its
    /// COLUMN's, not the page's: everything in the second column is one
    /// edge, everything on the page is another, and no control of
    /// either straddles the two. The footer is deliberately in neither
    /// set: it is pinned, it is destructive, and looking unlike the
    /// page is its job.
    #[test]
    fn the_pages_choices_and_doors_stand_in_one_column() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeel;
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let box_of = |act: Act| {
            s.hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .expect("a row of LOOK AND FEEL was not drawn")
        };
        let one_column = |what: &str, acts: &[Act]| {
            let first = box_of(acts[0]);
            for (i, act) in acts.iter().enumerate() {
                let r = box_of(*act);
                assert!(
                    (r.x - first.x).abs() < 0.01 && (r.w - first.w).abs() < 0.01,
                    "{what} {i} is {} px wide at x {}, the first is {} px at x {}",
                    r.w,
                    r.x,
                    first.w,
                    first.x
                );
            }
            first
        };
        let page_x = one_column(
            "row",
            &[
                Act::ListBtn(ListId::Looks),
                Act::ListBtn(ListId::Layauts),
                Act::ListBtn(ListId::Sounds),
            ],
        );
        let nav_x = one_column(
            "entry",
            &[Act::OpenSets, Act::OpenFont, Act::OpenSoundLevels],
        );
        assert!(
            nav_x.right() <= page_x.x + 0.01,
            "the section's pages stand over the page itself: the column ends \
             at {} and the page starts at {}",
            nav_x.right(),
            page_x.x
        );
    }

    /// SOUND LEVELS is one of the section's pages, and it is not the
    /// SOUNDS list wearing a second name.
    ///
    /// The two used to stand one under the other and a single word
    /// "SOUND" would run them together; the entry is in the section's
    /// own column now and the list is on the page, which is a wider gap
    /// than the one the guard was written for and not a smaller one. The
    /// guard is unchanged all the same: the labels differ, the entry
    /// stands directly above nothing but FONTS is above it where the
    /// owner put it, and pressing it opens the LEVELS — writing no
    /// `Sounds=` and moving no set, which an entry mistakenly wired to
    /// the list would do.
    #[test]
    fn the_sound_button_opens_the_levels_and_never_a_set() {
        fn button_at(rows: &[Row], act: Act) -> Option<usize> {
            rows.iter()
                .position(|r| matches!(r.ctrl, Ctrl::Button { act: a, .. } if a == act))
        }
        let levels = button_at(&LOOKFEEL_SUBRAIL_ROWS, Act::OpenSoundLevels)
            .expect("LOOK AND FEEL has no SOUND LEVELS page");
        let fonts_at = button_at(&LOOKFEEL_SUBRAIL_ROWS, Act::OpenFont)
            .expect("LOOK AND FEEL lost FONTS");
        assert_eq!(fonts_at + 1, levels, "FONTS does not stand above SOUND LEVELS");
        assert!(
            button_at(&RAIL_ROWS, Act::OpenSoundLevels).is_none(),
            "SOUND is a section of the rail and not a page of LOOK AND FEEL"
        );
        assert!(
            button_at(&LOOKFEEL_ROWS, Act::OpenSoundLevels).is_none(),
            "the page still carries the door the column replaced"
        );
        let Ctrl::Button { label: Text::Fixed(word), .. } =
            LOOKFEEL_SUBRAIL_ROWS[levels].ctrl
        else {
            panic!("the entry lost its fixed label")
        };
        assert_ne!(
            word,
            ListId::Sounds.label(),
            "the entry and the list wear one word: a reader cannot tell the \
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
                    Act::Close
                        | Act::VolumeTrack
                        | Act::ToggleTyping
                        | Act::ToggleAmbient
                ),
                "the levels page describes a control that is not a level"
            );
        }
        // And the way out is the window's own: the page stands in the
        // section's column, which never leaves the screen, so there is
        // no layer between it and the desktop to peel (owner: BACK goes
        // from the first-level pages).
        assert!(parent_view(View::SoundLevels).is_none());
        assert!(chrome_of(View::SoundLevels) == Chrome::Close);
        assert!(
            sub_act(View::SoundLevels) == Some(Act::OpenSoundLevels),
            "the page the entry opens is not the page the entry marks"
        );
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
        // BORDER is in the loop with the three file-backed lists on
        // purpose: its members are built in rather than found on disk, and
        // that is exactly the kind of difference that makes a list behave
        // subtly unlike its neighbours unless something checks.
        for list in [
            ListId::Looks,
            ListId::Layauts,
            ListId::Sounds,
            ListId::Borders,
            ListId::Backgrounds,
            ListId::Severities,
            ListId::Corners,
            ListId::RingStyles,
            ListId::ScrollModes,
            ListId::ScrollEdges,
        ] {
            for i in 0..s.names(list).len() {
                let name = s.names(list)[i].clone();
                match list {
                    ListId::Looks => s.current_look = Some(name),
                    ListId::Layauts => s.current_layaut = Some(name),
                    ListId::Sounds => s.current_sounds = Some(name),
                    ListId::Borders => s.current_border = Some(name),
                    ListId::Backgrounds => s.current_background = Some(name),
                    ListId::Severities => s.current_severity = Some(name),
                    ListId::Corners => s.current_corner = Some(name),
                    ListId::RingStyles => s.current_ring_style = Some(name),
                    ListId::ScrollModes => s.current_scroll_mode = Some(name),
                    ListId::ScrollEdges => s.current_scroll_edge = Some(name),
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
                ListId::Borders => s.current_border = Some("not installed".into()),
                ListId::Backgrounds => s.current_background = Some("not installed".into()),
                ListId::Severities => s.current_severity = Some("not installed".into()),
                ListId::Corners => s.current_corner = Some("not installed".into()),
                ListId::RingStyles => s.current_ring_style = Some("not installed".into()),
                ListId::ScrollModes => s.current_scroll_mode = Some("not installed".into()),
                ListId::ScrollEdges => s.current_scroll_edge = Some("not installed".into()),
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

    fn a_problem(addon: &str, path: &str, message: &str) -> nacelle::settings::Problem {
        nacelle::settings::Problem {
            addon: addon.to_string(),
            file: String::new(),
            path: std::path::PathBuf::from(path),
            message: message.to_string(),
        }
    }

    /// The two questions the toolkit answers with different things, and
    /// what the page makes of each.
    ///
    /// `installed()` is asked separately from `problems()` on purpose:
    /// a host that never told the toolkit where the files are has no
    /// bad FILES to list and is ignoring all of them, so an empty
    /// `problems()` means "nothing is wrong" in one case and "nothing
    /// is being read" in the other. A page that showed only the second
    /// answer would call the worse of the two states all clear.
    #[test]
    fn the_addons_report_tells_no_files_apart_from_no_directories() {
        assert!(
            addon_report(true, &[]).is_empty(),
            "a machine with nothing wrong has nothing to report"
        );

        let dead = addon_report(false, &[]);
        assert_eq!(dead.len(), 1, "the larger failure was reported as nothing");
        assert!(
            dead[0].contains("NO SETTINGS DIRECTORIES ARE INSTALLED"),
            "the one state that ignores every file on the machine is not named: {dead:?}"
        );

        let lines = addon_report(
            true,
            &[a_problem(
                "search",
                "/home/who/.config/nacelle/addons/search.ron",
                "is not valid RON \u{2014} 3:1-3:2: Expected comma",
            )],
        );
        assert!(
            lines.iter().any(|l| l == "/home/who/.config/nacelle/addons/search.ron"),
            "the path is the part the user opens, and it is not there: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("3:1-3:2")),
            "the position in the file was dropped: {lines:?}"
        );
    }

    /// The window has somewhere to SHOW it, which is the half a report
    /// nobody draws does not have.
    ///
    /// `nacelle-addons/README.md` promises a file that does not load is
    /// reported on stderr and in the settings window. A desktop session
    /// has nowhere to show a stderr — the program is started by a
    /// display manager and its output goes to a journal — so until this
    /// page existed the promise rested entirely on a channel the user
    /// cannot see.
    #[test]
    fn a_settings_file_the_program_could_not_use_is_named_on_the_addons_page() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::Addons;
        s.addon_report = addon_report(
            true,
            &[a_problem(
                "filesystem",
                "/home/who/.config/nacelle/addons/filesystem.ron",
                "is not valid RON \u{2014} 2:15-2:16: Expected comma",
            )],
        );
        let drawn = page_runs(&mut fonts, &mut s);
        let said = |needle: &str| drawn.iter().any(|t| t.contains(needle));
        assert!(
            said("/home/who/.config/nacelle/addons/filesystem.ron"),
            "the page does not name the file: {drawn:?}"
        );
        assert!(said("2:15-2:16"), "the page does not say where in it: {drawn:?}");
        assert!(
            !said(ADDONS_ALL_CLEAR),
            "the page reported all clear over a file it had just named"
        );
        // And the way in is on the rail, in front of the reader on every
        // page of the window rather than somewhere only a keyboard could
        // reach.
        assert!(
            rail_acts(&s).contains(&Act::OpenAddons),
            "there is no door to the page"
        );
    }

    /// Nothing wrong is an ANSWER, drawn as one. A blank page and a
    /// page that is not looking read the same, and the second is the
    /// state this whole page exists because of.
    #[test]
    fn the_addons_page_says_so_when_every_file_loads() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::Addons;
        s.addon_report = addon_report(true, &[]);
        let drawn = page_runs(&mut fonts, &mut s);
        assert!(
            drawn.iter().any(|t| t.contains(ADDONS_ALL_CLEAR)),
            "a page with nothing to report said nothing at all: {drawn:?}"
        );
    }

    /// What does not fit is counted, never dropped. A report that
    /// stopped at the bottom of the box would be this window's own
    /// version of the silence it exists to end: four files named, four
    /// fixed, and a widget still on its defaults.
    #[test]
    fn a_report_longer_than_the_page_says_how_much_it_did_not_show() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::Addons;
        let many: Vec<_> = (0..40)
            .map(|i| a_problem(&format!("addon{i}"), &format!("/tmp/a{i}.ron"), "is not valid RON"))
            .collect();
        s.addon_report = addon_report(true, &many);
        let mut dl = nacelle::draw::DrawList::recording();
        // The shortest window the program is built for, so the box is
        // certainly shorter than eighty lines.
        let mut ctx = probe(&mut dl, &mut fonts, 720.0, 1.0);
        s.draw(&mut ctx);
        let drawn = text_runs(&dl);
        assert!(
            drawn.iter().any(|t| t.contains("MORE") && t.contains("STDERR")),
            "the page dropped what it could not fit without saying so: {drawn:?}"
        );
    }

    /// The page shows the TOOLKIT's answer and not one of its own.
    ///
    /// Pinned against both answers as they stand in this process rather
    /// than against a fixture: the settings roots are process-wide, so
    /// a test that installed its own would decide what another test
    /// reads. What is under test is that the door reads them at all —
    /// a page that kept its own list would drift from the toolkit the
    /// moment a widget was built.
    #[test]
    fn the_addons_page_reads_the_toolkit_on_the_way_in() {
        let mut s = furnished();
        assert!(!s.perform(Act::OpenAddons, 0.0), "a report changed the configuration");
        assert!(s.view == View::Addons, "the door opened nothing");
        assert_eq!(
            s.addon_report,
            addon_report(nacelle::settings::installed(), &nacelle::settings::problems()),
            "the page is showing something other than what the toolkit answers"
        );
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
            said("GUTTER"),
            "the confirmation does not mention the panel gutter, which the \
             reset also clears"
        );
        assert!(
            said("PINNED"),
            "the confirmation does not mention the pinned arrangement"
        );
    }

    /// Every view can be left, no view is its own way out, and the
    /// corner button says exactly what the ladder does.
    ///
    /// A cycle here would be a window Escape cannot get out of. The
    /// second half is what used to be two statements: a page said BACK
    /// in the table while the ladder had nowhere to send it, so the
    /// button led to the page it was already standing on. There is one
    /// statement now ([`chrome_of`]) and this is what holds every view
    /// to it.
    #[test]
    fn no_view_is_its_own_way_out() {
        for p in PAGES.iter() {
            let mut v = p.view;
            let mut steps = 0;
            while let Some(up) = parent_view(v) {
                v = up;
                steps += 1;
                assert!(steps <= PAGES.len(), "{}: the way out is a circle", p.title);
            }
            // What the ladder ends on is a page the navigation reaches,
            // and a page the navigation reaches wears CLOSE.
            assert!(
                chrome_of(v) == Chrome::Close,
                "{}: the ladder ends on a page that still says BACK",
                p.title
            );
            assert!(
                chrome_of(p.view) == Chrome::Back || parent_view(p.view).is_none(),
                "{}: the corner button and the Escape ladder disagree",
                p.title
            );
            // And the rail is always showing the section it stands in,
            // so no page of this window is unreachable from any other.
            assert!(
                rail_acts(&furnished()).contains(&rail_act(p.view)),
                "{}: the page's own section is not on the rail",
                p.title
            );
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

    /// P11's guard, given back: the accordion learnt to scroll, and the
    /// test its predecessor asked for is this one.
    ///
    /// The predecessor (`a_long_list_hangs_past_the_window_until_the_
    /// accordion_can_scroll`) pinned the FAULT on purpose: forty themes'
    /// tail was pressable below the window's bottom edge, because the
    /// object laid its rows out as one unclipped column. Its doc ordered
    /// whoever made it fail to turn it back into P11 — the frame now
    /// cuts, the body scrolls under it (`ScrollView`, the 7th argument),
    /// and a clipped-out element is reported with no area. So, walking
    /// the LIST's own scroll from head to tail:
    ///
    /// - nothing pressable ever stands below the window's bottom edge —
    ///   the invisible-but-clickable tail stays dead;
    /// - every one of the forty names is pressable at SOME offset — the
    ///   scroll reaches the whole body, and no name is lost to the cut.
    #[test]
    fn a_long_list_scrolls_and_no_name_is_lost_to_the_frame() {
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
        let mut reached = [false; N];
        // The stops derive from the frame the accordion itself framed —
        // `list_bar` after the first draw — half a frame apart, so
        // consecutive stops overlap (a row is far shorter than half a
        // frame), the same lesson the editor page's reachability sweep
        // learnt: a stride guessed from the window went stale the moment
        // the frame was capped shorter than it.
        let mut stops = vec![0.0f32];
        let mut at = 0;
        while at < stops.len() {
            s.list_scroll.set_offset(stops[at]);
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            let h = ctx.h;
            s.draw(&mut ctx);
            if at == 0 {
                let (_, viewport, content) =
                    s.list_bar.expect("forty names do not fit one frame");
                let stride = (viewport * 0.5).max(1.0);
                let mut next = stride;
                while next < content {
                    stops.push(next);
                    next += stride;
                }
            }
            for (r, a) in &s.hits {
                let Act::Pick(ListId::Looks, i) = *a else { continue };
                let pressable = r.w > 0.0 && r.h > 0.0;
                assert!(
                    !(pressable && r.y > h),
                    "name {i} is pressable below the window's bottom edge \
                     at offset {}",
                    s.list_scroll.offset()
                );
                if pressable {
                    reached[i] = true;
                }
            }
            at += 1;
        }
        for (i, seen) in reached.iter().enumerate() {
            assert!(
                seen,
                "name {i} was never pressable at any offset: the list's \
                 scroll does not reach it"
            );
        }
        viewport_home();
    }

    /// The wheel over an OPEN list turns the list, not the page under
    /// it: the branch in [`Settings::wheel`] — the settled page taking
    /// a notch aimed at the float above it was the wheel falling to the
    /// wrong scrolled thing, the same species of fault as the window
    /// passing the notch to the board behind it.
    #[test]
    fn the_wheel_over_an_open_list_turns_the_list_not_the_page() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        let mut s = furnished();
        s.view = View::LookFeel;
        s.now = 1.0;
        // Ticked once so the views have a frame clock behind them.
        s.scroll.tick(1.0, 100.0, 10_000.0, Snap::None, &ScrollPhysics::from_theme());
        s.list_scroll.tick(1.0, 100.0, 10_000.0, Snap::None, &ScrollPhysics::from_theme());
        let page_before = s.scroll.offset();
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        s.wheel(-1.0);
        assert_eq!(
            s.scroll.offset(),
            page_before,
            "the page moved under an open list"
        );
        assert!(
            s.list_scroll.offset() > 0.0,
            "the notch reached no scrolled thing at all"
        );
        // List closed, the same notch is the page's again.
        s.dropdown = None;
        s.wheel(-1.0);
        assert!(
            s.scroll.offset() > page_before,
            "with the list closed the page must take the wheel back"
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
        // The whole-theme sections' knobs are sliders like any other —
        // one witness per section shape, arrows not Enter.
        assert!(is_track(Act::EditorTrack(Knob::AccentB)));
        assert!(is_track(Act::EditorTrack(Knob::SurfHue)));
        assert!(is_track(Act::EditorTrack(Knob::SevH)));
        assert!(is_track(Act::EditorTrack(Knob::CornerSeg)));
        assert!(is_track(Act::EditorTrack(Knob::UnfocusedDim)));
        assert!(is_track(Act::EditorTrack(Knob::MenuEdgeW)));
        assert!(is_track(Act::EditorTrack(Knob::TipTextH)));
        assert!(is_track(Act::EditorTrack(Knob::BarTrackH)));
        assert!(!is_track(Act::EditGrid));
        assert!(!is_track(Act::ToggleSnap));
        assert!(!is_track(Act::FamilyBtn(Sect::Ui)));
        assert!(!is_track(Act::BoardGo((1, 0))));
        // The switches answer Enter, never arrows.
        assert!(!is_track(Act::EditorFlip(Flip::Ring)));
        assert!(!is_track(Act::ListBtn(ListId::Severities)));
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

    /// The wheel reaches this window, and stops at it.
    ///
    /// Both halves were broken together and are worth stating together.
    /// `wheel` existed but nothing called it — it carried an
    /// `allow(dead_code)` saying so — while the event loop's `MouseWheel`
    /// arm guarded an open menu and an open editor and NOT an open
    /// settings window. So the notch fell past the window to
    /// `content_layout()` and turned a widget on the board BEHIND it,
    /// and the pages themselves could not be scrolled at all.
    ///
    /// This measures the half that lives in this file: an open window
    /// takes the notch, a closed one refuses it, so the event loop can
    /// hand the wheel over and trust the guard. That the loop now hands
    /// it over is `main.rs`'s line, and no unit test here can see it.
    #[test]
    fn an_open_window_takes_the_wheel_and_a_closed_one_does_not() {
        let mut s = furnished();
        assert!(s.open, "the furnished window is the open one");
        let before = s.scroll.offset();
        s.wheel(-3.0);
        let after = s.scroll.offset();
        assert_ne!(
            before, after,
            "an open settings window ignored the wheel — the pages cannot \
             be scrolled and the notch has nowhere to go but the board behind"
        );

        // Closed, the same notch must not move anything: the event loop
        // asks this method before it knows whether to keep the event, so
        // a window that scrolled while shut would swallow every turn of
        // the wheel meant for the desktop.
        let mut shut = furnished();
        shut.open = false;
        let before = shut.scroll.offset();
        shut.wheel(-3.0);
        assert_eq!(
            before,
            shut.scroll.offset(),
            "a closed settings window moved on the wheel"
        );
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

    /// The editor page with every `Row::when` condition set at once —
    /// FROSTED for the background's knobs, a role for the severity
    /// sliders, a cut for the shape's, the ring on, dashed and haloed,
    /// the bar fading over a drawn groove. What the reachability sweep
    /// and the group test both mean by "everything on screen".
    fn editor_ajar(s: &mut Settings) {
        s.current_background = Some("FROSTED GLASS".to_string());
        s.current_severity = Some("OK".to_string());
        s.surface_own_hue = true;
        s.current_corner = Some("ROUND".to_string());
        s.ring_on = true;
        s.current_ring_style = Some("DASHED".to_string());
        s.ring_halo = true;
        s.current_scroll_mode = Some("INSET".to_string());
        s.current_scroll_edge = Some("RIGHT".to_string());
        s.bar_auto_hide = true;
        s.bar_track = true;
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
            mouse: nacelle::pointer::Pointer::new(-1.0, -1.0),
            term_font_scale: 1.0,
            ui_font_scale,
            panel_scale: 1.0,
            focus: None,
            tips: None,
        }
    }

    /// The editor's colour maths, pinned on both directions and the edges.
    ///
    /// The vector set was drafted by the owner's local model and CHECKED BY
    /// HAND before use — two of its eight rows were wrong (it answered grey
    /// for dark red and a violet for a steel blue), which is the working
    /// demonstration of the rule this project runs delegation under: a
    /// draft is welcome, an unverified draft is not.
    #[test]
    fn hsv_conversions_agree_with_the_checked_vectors() {
        // (h, s, v) -> (r, g, b) in 0..255.
        const VECTORS: [(f32, f32, f32, u8, u8, u8); 8] = [
            (0.0, 1.0, 1.0, 255, 0, 0),
            (0.0, 0.0, 1.0, 255, 255, 255),
            (0.0, 0.0, 0.0, 0, 0, 0),
            (0.0, 1.0, 0.5, 128, 0, 0),
            (360.0, 1.0, 1.0, 255, 0, 0),
            (120.0, 1.0, 1.0, 0, 255, 0),
            (240.0, 1.0, 1.0, 0, 0, 255),
            (200.0, 0.5, 0.75, 96, 159, 191),
        ];
        for (h, sat, v, r, g, b) in VECTORS {
            let (fr, fg, fb) = hsv_to_rgb(h, sat, v);
            let got = (
                (fr * 255.0).round() as u8,
                (fg * 255.0).round() as u8,
                (fb * 255.0).round() as u8,
            );
            assert_eq!(
                got,
                (r, g, b),
                "hsv({h}, {sat}, {v}) landed on {got:?}, the checked vector says {:?}",
                (r, g, b)
            );
            // And back: the seed path reads the theme's colour into the
            // sliders, so a round trip must land on the same numbers.
            let (h2, s2, v2) = rgb_to_hsv(fr, fg, fb);
            let (fr2, fg2, fb2) = hsv_to_rgb(h2, s2, v2);
            assert!(
                (fr - fr2).abs() < 0.005 && (fg - fg2).abs() < 0.005 && (fb - fb2).abs() < 0.005,
                "hsv({h}, {sat}, {v}) did not survive the round trip"
            );
        }
    }

    /// The owner's own gesture, end to end: open the editor page, unfold
    /// the BORDER list, click NEON — and the click must NOT report a
    /// configuration change. `true` from `perform` tells main to re-resolve
    /// the configuration, which reloads the theme, which builds a fresh
    /// engine with an EMPTY preview: a border pick would erase its own
    /// preview in the same breath. That was a real bug, and its shape was
    /// deceptive — NEON looked fine because the post-reload state (a theme
    /// with its glow on) matched what NEON asks for, while LINE looked dead
    /// until the first slider pulse re-sent the set.
    #[test]
    fn choosing_a_border_kind_does_not_reload_the_theme() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::ThemeEditor;
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let anchor = s.hits.iter().find(|&&(_, a)| a == Act::ListBtn(ListId::Borders))
            .map(|&(r, _)| r).expect("the editor page drew no BORDER anchor");
        let (w, h) = (1080.0 * 16.0 / 9.0, 1080.0);
        s.click(anchor.x + anchor.w / 2.0, anchor.y + anchor.h / 2.0, w, h, None);
        assert!(matches!(s.dropdown, Some(Dropdown::List(ListId::Borders))),
            "the anchor did not open the list");
        // A second frame: the list is drawn and its rows registered.
        let mut dl2 = nacelle::draw::DrawList::recording();
        let mut ctx2 = probe(&mut dl2, &mut fonts, 1080.0, 1.0);
        s.dropdown_since = None; // fully unfolded, no animation
        s.draw(&mut ctx2);
        let neon = s.hits.iter().find(|&&(_, a)| a == Act::Pick(ListId::Borders, 1))
            .map(|&(r, _)| r).expect("the open BORDER list registered no NEON row");
        assert!(
            !s.click(neon.x + neon.w / 2.0, neon.y + neon.h / 2.0, w, h, None),
            "a border pick reported a configuration change — main will \
             reload the theme and erase the preview the pick just set"
        );
        assert_eq!(
            s.current_border.as_deref(),
            Some("NEON"),
            "the pick did not set the border kind"
        );
    }

    /// The whole-theme sections feed the ONE builder — with the page in
    /// its everything-on state, every group of the model answers in the
    /// edit set with at least one of its tokens, so the preview and SAVE
    /// get all of them for free. Severity is the deliberate exception
    /// and the second half pins it: an untouched role is NOT written,
    /// a touched one is, and only it.
    #[test]
    fn the_editor_edits_carry_every_new_group_and_severity_only_touched() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = furnished();
        editor_ajar(&mut s);
        let edits = s.editor_edits();
        let has = |t: &str| edits.iter().any(|e| e.token == t);
        for token in [
            // one witness per group, each a token with a named reader on
            // the model's ALIVE list
            "palette.accent",
            "surface.hue",
            "surface.lift",
            "text.lift",
            "corner.mode",
            "corner.segments",
            "stroke.hair",
            "focus.ring.enabled",
            "focus.ring.dash",
            "glow.focus_ring.enabled",
            "focus.unfocused_dim",
            "component.menu.fill",
            "menu.border",
            "component.tooltip.fill",
            "tooltip.border",
            "scrollbar.mode",
            "scrollbar.w",
            "scrollbar.fade_ms",
            "scrollbar.track",
            "component.scrollbar.track",
        ] {
            assert!(has(token), "the edit set carries no {token}");
        }
        // OWN HUE writes degrees; the switch off restores the reference,
        // or a later accent drag stops moving the surfaces.
        let hue_of = |v: &Vec<nacelle::theme::edit::Edit>| {
            v.iter().find(|e| e.token == "surface.hue").unwrap().value.clone()
        };
        assert!(!hue_of(&edits).starts_with('@'), "OWN HUE wrote a reference");
        s.surface_own_hue = false;
        assert_eq!(hue_of(&s.editor_edits()), "@hue.accent");
        // No slider moved, so no severity author is in the set at all.
        assert!(
            !edits.iter().any(|e| e.token.starts_with("severity.")),
            "an untouched severity role was written"
        );
        // One slider on one role: exactly that author joins.
        s.current_severity = Some("CRITICAL".to_string());
        s.set_severity(0, 80);
        let edits = s.editor_edits();
        assert!(
            edits.iter().any(|e| e.token == "severity.critical.text"),
            "the touched role's author is missing"
        );
        assert_eq!(
            edits.iter().filter(|e| e.token.starts_with("severity.")).count(),
            1,
            "a role nobody touched rode along with the touched one"
        );
    }

    // --------------------------------------------- the editor's BASIC page

    /// An editor sitting on a real theme: the page seeded off the live
    /// bake, exactly as [`Act::ThemesEditor`] leaves it.
    fn editor_open() -> Settings {
        let mut s = furnished();
        s.view = View::ThemeEditor;
        s.seed_editor_from_theme();
        s
    }

    /// ŻYCZENIE 2, the switch. It stands at the HEAD of the page, before
    /// every section, and it is the ONE control both modes share with the
    /// footer — press it and the page under it is the other page.
    #[test]
    fn the_mode_switch_heads_the_editor_and_swaps_the_page_under_it() {
        let _g = crate::widgets::theme_test_lock();
        let page = &PAGES[View::ThemeEditor as usize];
        let mut s = editor_open();

        // AT THE TOP. `described_acts` walks the page band by band in
        // registration order and puts the chrome at its head, so the
        // mode is the first thing the page itself offers.
        let described = described_acts(&s, page);
        assert!(
            described.get(1) == Some(&Act::EditorMode),
            "the mode switch is not the first control on the editor page"
        );

        // ADVANCED is what the door opens on, and it is the page that
        // was always here: its sections are all offering their controls.
        assert!(!s.editor_basic, "the editor opened on BASIC");
        let advanced = described_acts(&s, page);
        assert!(
            advanced.contains(&Act::EditorTrack(Knob::EdgeL)),
            "ADVANCED is not showing the border section"
        );
        assert!(
            !advanced.contains(&Act::EditorTrack(Knob::ToneHue)),
            "ADVANCED is showing a BASIC slider"
        );

        // The switch flips it, and the two pages trade places whole.
        s.perform(Act::EditorMode, 0.0);
        assert!(s.editor_basic, "the switch did not reach BASIC");
        let basic = described_acts(&s, page);
        for k in [Knob::ToneHue, Knob::ToneSat, Knob::ToneLight] {
            assert!(
                basic.contains(&Act::EditorTrack(k)),
                "BASIC is missing one of its three sliders"
            );
        }
        assert!(
            !basic.iter().any(|a| matches!(
                a,
                Act::EditorTrack(Knob::EdgeL) | Act::EditorTrack(Knob::CornerSm)
            )),
            "BASIC is still showing ADVANCED's controls"
        );
        // The verbs belong to BOTH pages: the bar is pinned, not banded.
        for verb in [Act::EditorSave, Act::EditorSaveAs, Act::EditorCancel] {
            assert!(basic.contains(&verb), "BASIC lost one of the editor's verbs");
            assert!(advanced.contains(&verb), "ADVANCED lost one of the editor's verbs");
        }
        // And back, on the same one control.
        s.perform(Act::EditorMode, 0.0);
        assert!(!s.editor_basic, "the switch is one-way");
        // The preview these presses pushed is this test's, and it does
        // not leave the room in it: another test reading the theme
        // would be reading this one's editor session.
        nacelle::theme::clear_preview();
    }

    /// BASIC's three sliders write the theme's AUTHORS, and nothing that
    /// the cascade derives.
    ///
    /// The set is the model's ([`nacelle::theme::edit::tone_edits`]) and
    /// libnacelle holds it to its ten tokens; what this measures is the
    /// WINDOW's half — that the sliders are wired to it at all, that a
    /// drag changes what would be written, and that the rest of the
    /// editor's set is still in the edit underneath.
    #[test]
    fn the_basic_sliders_move_the_authors_and_leave_the_rest_standing() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::EditorMode, 0.0);
        assert!(s.tone_seeds.is_some(), "BASIC opened without seeds to move from");

        // AT REST the page is a no-op: `TONE_REST` is `Tone::NEUTRAL`,
        // so opening BASIC and touching nothing must not move a colour.
        assert_eq!(s.tone, TONE_REST);
        assert_eq!(s.tone_of(), nacelle::theme::edit::Tone::NEUTRAL);
        let rest = s.editor_edits();
        let value = |v: &Vec<nacelle::theme::edit::Edit>, t: &str| {
            v.iter().find(|e| e.token == t).map(|e| e.value.clone())
        };

        // A TURN. Every author moves, and the ones that are not authors
        // are not touched.
        s.tone[0] = 90;
        let turned = s.editor_edits();
        for token in [
            "palette.accent",
            "severity.ok.text",
            "severity.critical.text",
            "surface.lift",
            "text.lift",
        ] {
            assert!(value(&turned, token).is_some(), "BASIC wrote no {token}");
        }
        assert_ne!(
            value(&turned, "palette.accent"),
            value(&rest, "palette.accent"),
            "the HUE slider moved and the seed did not"
        );
        // The severity family is CARRIED, not flattened: all seven
        // authors ride the same turn, which is what keeps `ok` and
        // `critical` different colours (measured over the master in
        // libnacelle).
        assert_eq!(
            turned.iter().filter(|e| e.token.starts_with("severity.")).count(),
            7,
            "the turn did not carry every severity role"
        );
        // Pinning a DERIVED token would cut the cascade at the joint and
        // the next drag would find it deaf.
        for derived in ["hue.accent", "chroma.accent", "surface.chroma", "text.chroma"] {
            let before = value(&rest, derived);
            assert_eq!(
                value(&turned, derived),
                before,
                "BASIC's HUE slider wrote `{derived}`, which the cascade derives"
            );
        }
        // ONE assignment per token, or the saved file would carry a key
        // twice in one section.
        let mut names: Vec<&str> = turned.iter().map(|e| e.token).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "the edit set carries a token twice");
        // The rest of the theme is still underneath: BASIC lands ON the
        // editor's set, it does not replace it.
        for token in ["elev.panel.edge.color", "corner.mode", "scrollbar.mode"] {
            assert!(
                value(&turned, token).is_some(),
                "BASIC dropped `{token}` out of the edit"
            );
        }

        // The other two sliders move it too, each in its own way.
        s.tone = TONE_REST;
        s.tone[1] = 150;
        assert_ne!(
            value(&s.editor_edits(), "palette.accent"),
            value(&rest, "palette.accent"),
            "the SATURATION slider moved and nothing followed"
        );
        s.tone = TONE_REST;
        s.tone[2] = 80;
        let lifted = s.editor_edits();
        assert_ne!(
            value(&lifted, "surface.lift"),
            value(&rest, "surface.lift"),
            "the LIGHTNESS slider left the surface ladder where it was"
        );
        assert_ne!(
            value(&lifted, "text.lift"),
            value(&rest, "text.lift"),
            "the LIGHTNESS slider left the text ladder where it was"
        );
        // The preview these presses pushed is this test's, and it does
        // not leave the room in it: another test reading the theme
        // would be reading this one's editor session.
        nacelle::theme::clear_preview();
    }

    /// NEITHER MODE EATS THE OTHER'S WORK — the owner's condition on the
    /// switch, in both directions.
    #[test]
    fn switching_editor_modes_loses_no_work() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        // A choice only ADVANCED can make, and one only BASIC can.
        s.current_corner = Some("CHAMFER".to_string());
        s.ring_on = true;
        s.current_ring_style = Some("DASHED".to_string());

        s.perform(Act::EditorMode, 0.0);
        // ADVANCED -> BASIC keeps the advanced page untouched: those
        // controls hold work that is in no file yet.
        assert_eq!(s.current_corner.as_deref(), Some("CHAMFER"), "the cut was lost");
        assert!(s.ring_on && s.current_ring_style.as_deref() == Some("DASHED"));
        // And the edit still carries it, so the SCREEN keeps it too.
        assert!(
            s.editor_edits().iter().any(|e| e.token == "corner.mode"),
            "the cut fell out of the edit on the way into BASIC"
        );
        // BASIC opens at rest over whatever is on the screen.
        assert_eq!(s.tone, TONE_REST, "BASIC opened with a move already made");

        // A turn, then back to ADVANCED.
        s.tone[0] = 120;
        s.tone[2] = 70;
        let basic_edit = s.editor_edits();
        let accent_after_turn = basic_edit
            .iter()
            .find(|e| e.token == "palette.accent")
            .map(|e| e.value.clone());
        s.perform(Act::EditorMode, 0.0);
        assert!(!s.editor_basic);
        // THE FOLD. The sliders are back at rest — the move has become
        // part of what they are now relative to — and the advanced page
        // still has the cut.
        assert_eq!(s.tone, TONE_REST, "the fold left BASIC's sliders off rest");
        assert_eq!(
            s.current_corner.as_deref(),
            Some("CHAMFER"),
            "the trip through BASIC threw the cut away"
        );
        // The severity roles the turn moved are handed over as TOUCHED,
        // or ADVANCED — which writes only touched roles — would put the
        // theme's own words back and the rotation would vanish.
        assert_eq!(
            s.severity_touched,
            [true; 7],
            "the fold did not hand the rotated severity authors over"
        );
        let folded = s.editor_edits();
        assert_eq!(
            folded.iter().filter(|e| e.token.starts_with("severity.")).count(),
            7,
            "ADVANCED dropped the roles BASIC had turned"
        );
        // THE LOOK SURVIVED. What ADVANCED writes for the accent after
        // the fold is the colour BASIC was writing before it — compared
        // as a COLOUR and not as a string, because the fold lands on the
        // editor's whole-number HSV tracks and takes the same rounding
        // every seeded colour on that page takes.
        //
        // Hue and lightness are held exactly. CHROMA is held only as far
        // as sRGB reaches, and that is not slack in the test: BASIC is
        // held to no gamut (the owner's "BEZ OGRANICZEŃ zakresu") while
        // the ADVANCED page has edited colours on sRGB HSV tracks since
        // it was built, so a move that lands outside sRGB — this one
        // does, a light violet — arrives at a page with no way to write
        // it. It may be mapped IN; it may never be pushed out.
        let oklch_of = |v: &Vec<nacelle::theme::edit::Edit>| {
            let text = v.iter().find(|e| e.token == "palette.accent")?.value.clone();
            // `oklch(L, C, H)` as the model writes it.
            let inside = text.trim().strip_prefix("oklch(")?.strip_suffix(')')?;
            let n: Vec<f32> =
                inside.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (n.len() >= 3).then(|| (n[0], n[1], n[2]))
        };
        let before = oklch_of(&basic_edit).expect("BASIC wrote no accent to fold");
        let after = oklch_of(&folded).expect("ADVANCED wrote no accent after the fold");
        let hue_gap = |a: f32, b: f32| {
            let d = (a - b).rem_euclid(360.0);
            d.min(360.0 - d)
        };
        assert!(
            (before.0 - after.0).abs() < 0.02,
            "the fold moved the accent's LIGHTNESS: {before:?} became {after:?}"
        );
        assert!(
            hue_gap(before.2, after.2) < 2.0,
            "the fold turned the accent: {before:?} became {after:?}"
        );
        assert!(
            after.1 <= before.1 + 0.02,
            "the fold INVENTED chroma: {before:?} became {after:?}"
        );
        assert!(
            accent_after_turn.is_some(),
            "BASIC never wrote an accent to fold in the first place"
        );
        // And where the move stays INSIDE sRGB the fold is exact on all
        // three — the loss above is the destination page's gamut and
        // nothing else. A small turn at the theme's own lightness.
        let mut inside = editor_open();
        inside.perform(Act::EditorMode, 0.0);
        inside.tone[0] = 20;
        let basic_small = oklch_of(&inside.editor_edits()).expect("no accent");
        inside.perform(Act::EditorMode, 0.0);
        let folded_small = oklch_of(&inside.editor_edits()).expect("no accent");
        assert!(
            (basic_small.0 - folded_small.0).abs() < 0.02
                && (basic_small.1 - folded_small.1).abs() < 0.02
                && hue_gap(basic_small.2, folded_small.2) < 2.0,
            "a fold well inside sRGB still changed the colour: \
             {basic_small:?} became {folded_small:?}"
        );
        // A hue move re-welds the beds to the accent, so ADVANCED goes on
        // writing the reference and a later accent drag still moves them.
        assert!(!s.surface_own_hue, "the fold cut the surfaces loose from the accent");

        // A fold with NO move made leaves the severity marks alone: an
        // untouched role must keep the theme's own words, references
        // included.
        let mut quiet = editor_open();
        quiet.perform(Act::EditorMode, 0.0);
        quiet.perform(Act::EditorMode, 0.0);
        assert_eq!(
            quiet.severity_touched,
            [false; 7],
            "a trip through BASIC that moved nothing still claimed every role"
        );
        // The preview these presses pushed is this test's, and it does
        // not leave the room in it: another test reading the theme
        // would be reading this one's editor session.
        nacelle::theme::clear_preview();
    }

    /// THE CASCADE. Opening BASIC and touching nothing must leave the
    /// theme exactly where it was found — however many times it is done.
    ///
    /// This is one bug measured from two ends, and both ends are here
    /// because either one alone would have let it back in. BASIC SEEDS
    /// ITSELF FROM THE LIVE BAKE and WRITES A PREVIEW BACK INTO IT, so
    /// any error in the crossing between the bake's sRGB and the model's
    /// OKLCh does not merely mis-read once: the next visit reads what
    /// the last one wrote, and the theme walks. It did. With all three
    /// sliders at rest the accent's OKLab L went
    ///
    /// ```text
    ///   0.8200 -> 0.8904 -> 0.9413 -> 0.9715 -> 0.9869 -> ...
    /// ```
    ///
    /// one step per visit, and the gap between `ok` and `critical` — the
    /// severity pair the whole palette is told apart by — opened from
    /// 121.0 deg to 126.8 deg on the FIRST visit alone. Nobody had
    /// dragged anything.
    ///
    /// Every reading below is taken in LINEAR light, which is the space
    /// OKLCh is defined over and the space libnacelle's own tests take
    /// (`to_linear().to_oklch()`); the bake answers sRGB-encoded.
    #[test]
    fn a_quiet_trip_through_basic_leaves_the_theme_where_it_found_it() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        nacelle::theme::clear_preview();
        fn live(name: &str) -> nacelle::theme::Color {
            let t = theme::resolved();
            col(t.color(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}"))))
        }
        let lch = |c: nacelle::theme::Color| c.to_linear().to_oklch();
        let hue_gap = |a: f32, b: f32| {
            let d: f32 = (a - b).rem_euclid(360.0);
            d.min(360.0 - d)
        };
        let sev_gap = || {
            hue_gap(lch(live("severity.ok.text")).h, lch(live("severity.critical.text")).h)
        };

        let mut s = editor_open();
        let start = lch(live("palette.accent"));
        let start_gap = sev_gap();
        // The theme the master ships, so a failure names both numbers.
        assert!(
            (start.l - 0.8200).abs() < 0.001 && (start_gap - 121.0).abs() < 0.5,
            "the master moved under this test: accent L {} and ok/critical {start_gap}",
            start.l
        );

        for trip in 1..=6 {
            // In: the seeds come off the live bake and a preview goes
            // back. Out: the fold hands the move to ADVANCED, which
            // previews the same ten authors through its own tracks. Both
            // directions are on this path, and both used to move it.
            s.perform(Act::EditorMode, 0.0);
            let inside = lch(live("palette.accent"));
            assert!(
                (inside.l - start.l).abs() < 0.004,
                "visit {trip} lifted the accent on the way IN: {} -> {}",
                start.l,
                inside.l
            );
            assert!(
                hue_gap(inside.h, start.h) < 1.0,
                "visit {trip} turned the accent on the way in: {} -> {}",
                start.h,
                inside.h
            );
            assert!(
                (sev_gap() - start_gap).abs() < 1.0,
                "visit {trip} moved `ok` and `critical` apart: {start_gap} -> {}",
                sev_gap()
            );
            s.perform(Act::EditorMode, 0.0);
            let outside = lch(live("palette.accent"));
            // Four thousandths of L is what the ADVANCED page's
            // whole-number HSV tracks cost a colour on the way through
            // (one percent of brightness); the defect this pins was
            // seventy times that on the first visit alone.
            assert!(
                (outside.l - start.l).abs() < 0.004,
                "visit {trip} lifted the accent on the way OUT: {} -> {}",
                start.l,
                outside.l
            );
            assert!(
                (sev_gap() - start_gap).abs() < 1.0,
                "visit {trip} left the severity pair moved: {start_gap} -> {}",
                sev_gap()
            );
        }
        // And the SLIDERS never moved: this is what "touched nothing"
        // means, so a fold that quietly wrote a move would not pass here
        // by moving the sliders to match it.
        assert_eq!(s.tone, TONE_REST, "a quiet trip left BASIC's sliders off rest");

        // THE SAME CLAIM WITH THE FOLD DOING WORK. A quiet trip never
        // reaches `fold_tone_into_advanced` — a NEUTRAL move returns
        // early — and the fold is the OTHER crossing between the tracks
        // and OKLCh, so it gets its own six visits with a real move in
        // each. A rotation moves hue and nothing else, so the lightness
        // is what must stand still while the hue walks.
        let mut turned = editor_open();
        let base = lch(live("palette.accent"));
        for step in 1..=6u32 {
            turned.perform(Act::EditorMode, 0.0);
            turned.tone[0] = 20;
            turned.perform(Act::EditorMode, 0.0);
            let now = lch(live("palette.accent"));
            // Two hundredths, and the number is the ADVANCED page's own
            // rounding: the fold lands on whole-number HSV tracks, so a
            // hundredth of BRIGHTNESS is the finest thing it can hold and
            // six landings in a row wander inside that. The defect this
            // guards is an order larger — the crossing it pins moves a
            // colour by the whole sRGB transfer curve.
            assert!(
                (now.l - base.l).abs() < 0.02,
                "fold {step} moved the accent's LIGHTNESS: {} -> {}",
                base.l,
                now.l
            );
            assert!(
                hue_gap(now.h, base.h + 20.0 * step as f32) < 4.0,
                "fold {step} did not carry the rotation: {} is not {} + {} deg",
                now.h,
                base.h,
                20 * step
            );
        }
        nacelle::theme::clear_preview();
        viewport_home();
    }

    /// The two crossings between the editor's sRGB tracks and the file's
    /// OKLCh are INVERSES, measured on the master's own accent.
    ///
    /// The pair is `seed_editor_from_theme` -> `hsv_track_of` on the way
    /// in and `editor_edits`' `of` on the way out. A theme opened and
    /// saved with nothing touched must write the colour it opened on;
    /// what it wrote instead was a colour lighter by the sRGB transfer
    /// curve, which is the whole of the cascade above in one hop.
    #[test]
    fn the_editors_colour_tracks_and_the_file_are_inverses() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let t = theme::resolved();
        let accent =
            col(t.color(nacelle::theme::id("palette.accent").expect("no accent")));
        let s = editor_open();
        // What the page would write for the accent, with no slider moved.
        let written = s
            .editor_edits()
            .iter()
            .find(|e| e.token == "palette.accent")
            .map(|e| e.value.clone())
            .expect("the editor wrote no accent");
        let inside = written
            .trim()
            .strip_prefix("oklch(")
            .and_then(|v| v.strip_suffix(')'))
            .expect("the accent is not an oklch literal");
        let n: Vec<f32> =
            inside.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        let want = accent.to_linear().to_oklch();
        // A track carries whole numbers, so the trip costs a percent of
        // brightness and half a degree of hue and nothing else.
        assert!(
            (n[0] - want.l).abs() < 0.004,
            "the page would save the accent at L {} instead of {}",
            n[0],
            want.l
        );
        assert!(
            (n[2] - want.h).abs().min(360.0 - (n[2] - want.h).abs()) < 1.0,
            "the page would save the accent at hue {} instead of {}",
            n[2],
            want.h
        );
    }

    /// ŻYCZENIE 2b, END TO END AND ON THE REAL PIPELINE: a drag of
    /// BASIC's HUE slider turns what this window actually paints, and
    /// leaves the interface ONE HUE in DIFFERENT SHADES.
    ///
    /// libnacelle measures the same claim over the master's cascade;
    /// this measures the WINDOW's chain — slider, `editor_edits`,
    /// `set_preview`, the bake, and the colour that comes back out of a
    /// token the settings columns are painted with. Between them there
    /// is no step where a hue could be lost and nobody notice.
    ///
    /// The owner's own pair: a column's CONTAINER and a control's PLATE.
    ///
    /// EVERY READING IS DECODED FIRST. "One hue at three lightnesses" is
    /// a claim about OKLCh, OKLCh is defined over LINEAR light, and the
    /// bake answers sRGB-encoded — so a reading taken without
    /// `to_linear` measures a different quantity and the sentence stops
    /// meaning what it says. On the master's own bands it is not a
    /// rounding either: the three lightnesses read 0.2698 / 0.4036 /
    /// 0.4840 encoded against 0.1150 / 0.1780 / 0.2320 decoded, and the
    /// hue the three are supposed to SHARE spread 166.46 / 169.28 /
    /// 169.04 instead of standing on one number.
    #[test]
    fn a_basic_hue_drag_turns_the_window_and_keeps_one_hue_in_three_shades() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut s = editor_open();
        s.perform(Act::EditorMode, 0.0);

        let lch = |c: nacelle::theme::Color| c.to_linear().to_oklch();
        let hue_gap = |a: f32, b: f32| {
            let d = (a - b).rem_euclid(360.0);
            d.min(360.0 - d)
        };
        /// The colour a token resolves to in the theme as it stands
        /// RIGHT NOW — preview included, which is the whole point.
        fn live(name: &str) -> nacelle::theme::Color {
            let t = theme::resolved();
            col(t.color(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}"))))
        }

        let before = lch(live("component.settings.sub_fill"));
        // FOUR positions of the slider, not one: a claim that only holds
        // where the numbers happen to land is not the claim.
        for turn in [37u32, 90, 180, 251] {
            s.tone = TONE_REST;
            s.tone[0] = turn;
            s.apply_editor_preview();

            let rail = lch(live("component.settings.rail_fill"));
            let sub = lch(live("component.settings.sub_fill"));
            // The page's band is the window BODY's own token: the master
            // names the two deviations and leaves the third to the
            // surface the page is already a part of.
            let page = lch(live("component.panel.fill"));
            // The window really turned, by the slider's own degrees.
            assert!(
                hue_gap(sub.h, before.h + turn as f32) < 6.0,
                "at {turn} deg the columns did not follow the slider: {} -> {}",
                before.h,
                sub.h
            );
            // ONE HUE for the whole interface — the columns and the
            // plate a button stands on, which is the owner's own pair.
            let plate = lch(col(theme::resolved()
                .class_state(theme::class_id("button").expect("no button class"), State::Idle)
                .fill));
            assert!(
                hue_gap(sub.h, plate.h) < 6.0,
                "at {turn} deg a column and a button plate are two COLOURS: {} vs {}",
                sub.h,
                plate.h
            );
            // DIFFERENT SHADES — the container is a bed and the plate is
            // a control, and no reader may have to guess which is which.
            // Measured on the master, decoded: 0.1780 against 0.8200.
            assert!(
                (plate.l - sub.l).abs() > 0.40,
                "at {turn} deg a column and a button plate share a lightness: {} vs {}",
                sub.l,
                plate.l
            );
            // And the three columns are still three shades of it.
            assert!(
                rail.l < sub.l && sub.l < page.l,
                "at {turn} deg the three columns stopped being a ladder: {} {} {}",
                rail.l,
                sub.l,
                page.l
            );
            // ONE hue between themselves, and this is the assertion the
            // SPACE is load-bearing for. The two COLUMNS take their h
            // from one token (`@surface.hue`) and stand on one number:
            // 203.46 against 203.46 at the first position. The BODY
            // lands a quarter-degree off it (203.22) and not on it,
            // because its colour is not a reference — the BACKGROUND
            // section holds it on integer HSV sliders, and BASIC's hue
            // is carried onto it by `Tone::shift`. One notch of that
            // slider is the finest the body's bed can be stated at, and
            // 0.24 deg is well inside one notch.
            //
            // Read ENCODED instead of decoded the three spread nearly
            // three degrees — six times the quantisation and the thing
            // this tolerance is really here to catch, because a reader
            // could not tell that apart from a real drift.
            let spread = hue_gap(rail.h, sub.h).max(hue_gap(sub.h, page.h));
            assert!(
                spread < 0.5,
                "at {turn} deg the three bands are on three hues: {} {} {}",
                rail.h,
                sub.h,
                page.h
            );
            // THE EXCEPTION the owner carved out: severity carries
            // MEANING, so the roles stay different COLOURS.
            let ok = lch(live("severity.ok.text"));
            let crit = lch(live("severity.critical.text"));
            assert!(
                hue_gap(ok.h, crit.h) > 15.0,
                "at {turn} deg `ok` and `critical` collapsed onto one hue: {} vs {}",
                ok.h,
                crit.h
            );
        }
        // The preview is this test's, and it does not leave the room in
        // it: every other test that reads the theme would be reading
        // this one's drag.
        nacelle::theme::clear_preview();
        viewport_home();
    }

    /// The BASIC sliders notch by what the PIPELINE can show, and the
    /// depth that says so comes off SETTINGS -> COLOR.
    ///
    /// AND PAST TEN BITS THE TRACK IS THE LIMIT, WHICH IS NOT A DEFECT.
    /// On the master's seed (chroma 0.1531) the pipeline's own notch is
    ///
    /// ```text
    ///    8 bits   1.467 deg   0.02561   0.003922   ->  1, 3, 2 units
    ///   10 bits   0.366 deg   0.00638   0.000978   ->  1, 1, 1
    ///   12 bits   0.091 deg   0.00159   0.000244   ->  1, 1, 1
    ///   16 bits   0.006 deg   0.00010   0.000015   ->  1, 1, 1
    /// ```
    ///
    /// — a degree of hue, a percent of saturation and a fiftieth of the
    /// lightness span are what these tracks HAVE, so from ten bits up
    /// the answer is the track's own resolution and the floor is what
    /// keeps a press from moving nothing at all. The alternative to a
    /// coarse notch here is not a finer one, it is a finer TRACK — a
    /// change to the control, and the owner picked the coarse side of
    /// this exact trade ("a notch coarser than the pipeline is honest").
    /// So the numbers above are pinned rather than left to drift.
    #[test]
    fn the_basic_notch_comes_from_the_colour_depth() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::EditorMode, 0.0);
        let seeds = s.tone_seeds.expect("no seeds");
        assert!(seeds.accent.c > 0.0, "the master's accent is grey — the test is blind");
        // The seed's chroma is what the two derived notches divide by, so
        // it is named here: the master's mint, read in linear light.
        assert!(
            (seeds.accent.c - 0.1531).abs() < 0.001,
            "the seed's chroma moved to {} and the notches below with it",
            seeds.accent.c
        );

        let mut last = [0u32; 3];
        for (i, (bits, want)) in
            [(8u32, [1u32, 3, 2]), (10, [1, 1, 1]), (12, [1, 1, 1]), (16, [1, 1, 1])]
                .iter()
                .enumerate()
        {
            s.color_depth = *bits;
            let step = s.tone_step();
            assert_eq!(step, *want, "the notch at {bits} bits moved");
            for k in 0..3 {
                assert!(step[k] >= 1, "a slider at {bits} bits steps by nothing");
                if i > 0 {
                    assert!(
                        step[k] <= last[k],
                        "{bits} bits gave a COARSER notch than {} did on track {k}",
                        [8, 10, 12, 16][i - 1]
                    );
                }
            }
            last = step;
        }
        // The two ends really are different: eight bits is coarser than
        // sixteen somewhere, or the depth is not reaching the sliders.
        s.color_depth = 8;
        let coarse = s.tone_step();
        s.color_depth = 16;
        let fine = s.tone_step();
        assert!(
            (0..3).any(|k| coarse[k] > fine[k]),
            "the notch is the same at 8 bits as at 16 — the depth is not wired in"
        );
        // Nobody has said: the config answers with the toolkit's own
        // number, which it TAKES rather than repeats — so this is a
        // statement about where the answer lives, not two constants
        // being compared and hoped about.
        assert_eq!(
            crate::config::model::ColorConf::DEPTH,
            nacelle::theme::edit::DEFAULT_DEPTH_BITS
        );
        s.color_depth = crate::config::model::ColorConf::DEPTH;
        assert_eq!(s.tone_step(), coarse, "the depth nobody set is not the coarse one");
        // The preview these presses pushed is this test's, and it does
        // not leave the room in it: another test reading the theme
        // would be reading this one's editor session.
        nacelle::theme::clear_preview();
    }

    /// The slider unit maps, both directions — a value must survive
    /// seed -> slider -> file, or a theme saved and reopened creeps a
    /// notch per sitting. The walls are the BAKE's own, taken from it.
    #[test]
    fn the_editor_slider_maps_survive_the_round_trip() {
        // The LIGHTNESS track runs to the wider of the two ladder walls,
        // and `TONE_LIGHT_SPAN` names which of them that is.
        assert!(
            TEXT_LIFT_WALL >= SURFACE_LIFT_WALL
                && (TONE_LIGHT_SPAN - TEXT_LIFT_WALL).abs() < f32::EPSILON,
            "the wider ladder wall changed sides: surface {SURFACE_LIFT_WALL}, \
             text {TEXT_LIFT_WALL}"
        );
        // The symmetric spans: 50 is exactly zero, the ends the walls.
        for (span, wall) in [
            (SURFACE_LIFT_WALL, SURFACE_LIFT_WALL),
            (TEXT_LIFT_WALL, TEXT_LIFT_WALL),
        ] {
            assert_eq!(span_back(0.0, span), 50);
            assert!((span_of(50, span)).abs() < 1e-6);
            assert!((span_of(0, span) + wall).abs() < 1e-6);
            assert!((span_of(100, span) - wall).abs() < 1e-6);
            for v in [0u32, 13, 50, 77, 100] {
                assert_eq!(span_back(span_of(v, span), span), v);
            }
        }
        // The 0..hi scales — radii, kerf, widths, alpha, fade.
        for hi in [4.0f32, 2.0, 1.0, 2000.0] {
            for v in [0u32, 25, 50, 99, 100] {
                assert_eq!(scale_back(scale_of(v, hi), hi), v);
            }
        }
        // The lo..hi band — the scrollbar's 0.5u..4u.
        assert!((band_of(0, 0.5, 4.0) - 0.5).abs() < 1e-6);
        assert!((band_of(100, 0.5, 4.0) - 4.0).abs() < 1e-6);
        for v in [0u32, 20, 43, 100] {
            assert_eq!(band_back(band_of(v, 0.5, 4.0), 0.5, 4.0), v);
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
                // The bands that FLOW this frame, in the box they flow
                // in — which at a narrow window is the navigation ahead
                // of the page and not the page alone, and which is the
                // page's box less the scrollbar's lane. Asked of the same
                // three functions the drawing asks, or the walk would be
                // measuring a window nobody is looking at.
                let nav = Panes::of(p.view, m, content);
                let box_ = rows_box(nav.page);
                // Walk the description band by band, at the furthest the
                // offset goes, and inside a banded region column by
                // column: the last row of EVERY column has to be
                // reachable, not just the last row of the deepest one.
                let mut y = view.y - furthest;
                let mut last: Option<f32> = None;
                let mut started = false;
                for zone in s.frame_zones(p, &nav) {
                    if matches!(zone, Zone::Pinned { .. }) {
                        continue;
                    }
                    let zh = s.zone_h(zone, m, box_);
                    if zh <= 0.0 {
                        continue;
                    }
                    if started {
                        y += zone_gap();
                    }
                    started = true;
                    let band = Rect::new(box_.x, y, box_.w, box_.h);
                    for ((region, _, rows), dy) in zone_regions(zone, band)
                        .into_iter()
                        .zip(s.zone_offsets(zone, m, band))
                    {
                        let mut ry = y + dy;
                        for row in rows {
                            // Hidden rows are NOT THERE (Row::when) — the
                            // walk has to skip exactly what the flow skips
                            // or it measures a page that is not on screen.
                            if !(row.when)(&s) {
                                continue;
                            }
                            let rh = s.row_h(&row.ctrl, m, region);
                            last = Some(last.map_or(ry + rh, |v: f32| v.max(ry + rh)));
                            ry += rh + m.space(row.after);
                        }
                    }
                    y += zh;
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
                for zone in p.zones {
                    if !matches!(zone, Zone::Pinned { .. }) {
                        continue;
                    }
                    let zh = s.zone_h(zone, m, box_);
                    let pinned_top = box_.bottom() - zh;
                    assert!(
                        view.bottom() <= pinned_top + 0.01,
                        "{where_}: the body overlaps a pinned band by {} px",
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
                        // And the same question sideways, which nothing
                        // asked until the window grew a rail: a listed
                        // button was a fraction of the WHOLE content box
                        // while standing in a panel roughly half that
                        // wide, so its plate hung out of the window.
                        assert!(
                            r.x >= content.x - 0.01
                                && r.right() <= content.right() + 0.01,
                            "{} at {h}px: a target sits {:?} outside {:?} sideways",
                            p.title,
                            (r.x, r.right()),
                            (content.x, content.right())
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
    /// into drop-downs, which answered the same question their own way
    /// ([`a_long_list_scrolls_and_no_name_is_lost_to_the_frame`]: the
    /// accordion scrolls now). What is tested here is the half that
    /// stayed — the page's own flow, walked from top to bottom with a
    /// page of extra rows in it.
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

    // ------------------------------------------------------- the bands

    /// A two-column band, made only here: the pages are all one flow in
    /// this step, and a mechanism nothing builds is a mechanism nobody
    /// has measured.
    static PROBE_LEFT: [Row; 1] = [row(Ctrl::Slider {
        label: "RADIUS",
        act: Act::BlurRadiusTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: step_5,
        get: |s| s.blur_radius,
        set: |s, v| s.blur_radius = v,
        save: |_| {},
    })];

    static PROBE_RIGHT: [Row; 1] = [row(Ctrl::Slider {
        label: "OPACITY",
        act: Act::BlurOpacityTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: step_5,
        get: |s| s.blur_opacity,
        set: |s, v| s.blur_opacity = v,
        save: |_| {},
    })];

    static PROBE_COLUMNS: [ZCol; 2] = [
        // Two DIFFERENT measuring words on purpose: if the label column
        // were still the page's, both tracks would start on one pixel.
        ZCol {
            cols: Cols::Measured { label: "A", value: "100 %" },
            rows: &PROBE_LEFT,
        },
        ZCol {
            cols: Cols::Measured { label: "A MUCH LONGER LABEL", value: "100 %" },
            rows: &PROBE_RIGHT,
        },
    ];

    static PROBE_ZONES: [Zone; 1] = [Zone::Cols { columns: &PROBE_COLUMNS }];

    static PROBE_PAGE: Page = Page {
        view: View::Blur,
        title: "PROBE",
        lead: Gap::Section,
        zones: &PROBE_ZONES,
    };

    /// M1/M3, the geometry — a columned band is equal columns with the
    /// theme's gutter, and nothing of the box is left over.
    #[test]
    fn a_columned_band_splits_into_equal_columns() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let box_ = Rect::new(100.0, 50.0, 1000.0, 700.0);
        let regions = zone_regions(&PROBE_ZONES[0], box_);
        assert_eq!(regions.len(), 2, "a two-column band gave a different count");
        let (l, r) = (regions[0].0, regions[1].0);
        assert!((l.w - r.w).abs() < 0.01, "the two columns are not equal");
        assert!((l.x - box_.x).abs() < 0.01, "the band does not start at its box");
        assert!(
            (r.right() - box_.right()).abs() < 0.01,
            "the band does not end at its box"
        );
        assert!(
            (r.x - l.right() - col_gap()).abs() < 0.01,
            "the gutter is not settings.col_gap"
        );
        // The y and the height stay the page's, so a row that reserves
        // part of the CONTENT box measures the same box in a column.
        for (region, _, _) in &regions {
            assert!((region.y - box_.y).abs() < 0.01 && (region.h - box_.h).abs() < 0.01);
        }
        viewport_home();
    }

    /// M4 in the small — a band with no room for its columns is the
    /// list it was before it had them.
    ///
    /// Above the threshold the columns stand beside one another and none
    /// of them starts below the band's own top edge. Below it every
    /// column has the WHOLE width and starts under the one before it,
    /// `modal.row_gap` down — which is the space every one of those rows
    /// already leaves under itself, so a folded band is exactly the run
    /// of rows the page wrote before it had columns.
    ///
    /// The boxes are synthetic on purpose, a pixel either side of
    /// `settings.col_min_w`: what is under test is the RULE, not the
    /// width the tokens happen to leave on one screen.
    #[test]
    fn a_band_with_no_room_for_its_columns_is_the_list_it_was() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let s = furnished();
        let band = &PROBE_ZONES[0];
        let (min, gap) = (col_min_w(), col_gap());
        let wide = Rect::new(10.0, 20.0, 2.0 * (min + 1.0) + gap, 700.0);
        let narrow = Rect::new(10.0, 20.0, 2.0 * (min - 1.0) + gap, 700.0);
        assert!(!zone_folded(band, wide), "a band with the width for its columns folded");
        assert!(
            zone_folded(band, narrow),
            "a band a pixel short of the width stood in columns"
        );

        // Standing: two boxes side by side, both at the band's top.
        let (regions, offsets) =
            (zone_regions(band, wide), s.zone_offsets(band, m, wide));
        assert!(
            offsets.iter().all(|&d| d == 0.0),
            "a column standing beside its neighbour started below the band"
        );
        assert!(
            regions[1].0.x >= regions[0].0.right() + gap - 0.01,
            "the standing columns are not the gutter apart"
        );

        // Folded: the whole width each, one under the other.
        let (regions, offsets) =
            (zone_regions(band, narrow), s.zone_offsets(band, m, narrow));
        for (region, _, _) in &regions {
            assert!(
                (region.x - narrow.x).abs() < 0.01 && (region.w - narrow.w).abs() < 0.01,
                "a folded column did not take the whole width"
            );
        }
        let first = s.rows_h(PROBE_COLUMNS[0].rows, m, narrow);
        assert!(offsets[0] == 0.0, "the first column of a folded band moved");
        assert!(
            (offsets[1] - (first + m.gap)).abs() < 0.01,
            "the second column starts {} px down and the first ends {} px down",
            offsets[1],
            first
        );
        // And the band is as deep as the two together, so whatever
        // follows it starts below BOTH — the height and the drawing read
        // this one arithmetic.
        let stacked = offsets[1] + s.rows_h(PROBE_COLUMNS[1].rows, m, narrow);
        assert!(
            (s.zone_h(band, m, narrow) - stacked).abs() < 0.01,
            "a folded band is not as deep as the columns it stacked"
        );
        viewport_home();
    }

    /// M3, the measurement — a label column is measured against the
    /// REGION's own words and sized from the REGION's width.
    ///
    /// This is the whole reason `columns` stopped being the page's: two
    /// columns of one band ask it separately, so the sliders on the left
    /// do not inherit the width of the labels on the right.
    #[test]
    fn a_column_measures_its_own_labels() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let s = furnished();
        let (short, _) = s.columns(&mut ctx, PROBE_COLUMNS[0].cols, 600.0);
        let (long, _) = s.columns(&mut ctx, PROBE_COLUMNS[1].cols, 600.0);
        assert!(
            long > short + 0.01,
            "two different measuring words gave one label column ({short} px and {long} px)"
        );
        // A fraction is a fraction of the REGION, not of the page.
        let (half, _) = s.columns(&mut ctx, Cols::Frac, 600.0);
        let (whole, _) = s.columns(&mut ctx, Cols::Frac, 1200.0);
        assert!(
            half > 0.0 && (whole - half * 2.0).abs() < 0.01,
            "Cols::Frac did not follow the region's width"
        );
        viewport_home();
    }

    /// M2/M3, the walk — a columned band registers COLUMN BY COLUMN,
    /// and each column's rows are laid inside that column.
    ///
    /// The Tab order is the description's, never the geometry's: the
    /// whole of the first column, then the whole of the second.
    #[test]
    fn a_columned_band_registers_column_by_column() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let mut s = furnished();
        s.draw_body(&mut ctx, &PROBE_PAGE, m, content);
        assert!(
            s.hits.iter().map(|&(_, a)| a).collect::<Vec<_>>()
                == vec![Act::BlurRadiusTrack, Act::BlurOpacityTrack],
            "a columned band did not register column by column"
        );
        let track = |act: Act| {
            s.hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .expect("a column's row was not drawn")
        };
        let (left, right) = (track(Act::BlurRadiusTrack), track(Act::BlurOpacityTrack));
        assert!(
            (left.y - right.y).abs() < 0.01,
            "the two columns do not start on one line"
        );
        assert!(
            right.x >= left.right() + col_gap() - 0.01,
            "the two columns overlap: the left ends at {} and the right starts at {}",
            left.right(),
            right.x
        );
        // The right column's track is the SHORTER of the two, because
        // its own label column is the wider — which is only true if the
        // measurement was taken per column.
        assert!(
            right.w < left.w - 0.01,
            "both columns measured one label width ({} px and {} px)",
            left.w,
            right.w
        );
        viewport_home();
    }

    /// M6 — an action bar centres its plates, each as wide as its own
    /// word, `settings.bar_gap` apart. No length in it is the file's.
    #[test]
    fn an_action_bar_takes_its_widths_and_its_gap_from_the_theme() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let s = furnished();
        let band = Rect::new(content.x, content.y, content.w, m.btn_h);
        let rc = RowCtx { content, band, label_w: 0.0, value_w: 0.0, m };
        let plates = s.bar_plates(&mut ctx, &EDITOR_BAR_ITEMS, rc);
        assert_eq!(plates.len(), EDITOR_BAR_ITEMS.len());

        let th = theme::resolved();
        let gap = th.px(theme::id("settings.bar_gap").expect("the master declares it"));
        let min_w = th.px(theme::id("button.min_w").expect("the master declares it"));
        let pad = th.px(theme::id("button.pad_x").expect("the master declares it"));
        let f = role_button(&ctx);
        for (i, (r, label, act)) in plates.iter().enumerate() {
            assert!(*act == EDITOR_BAR_ITEMS[i].1, "the bar reordered its verbs");
            assert!((r.y - band.y).abs() < 0.01, "a plate left the row");
            let wanted =
                (ctx.fonts.measure(f.face, f.px, label, f.track) + 2.0 * pad).max(min_w);
            assert!(
                (r.w - wanted).abs() < 0.01,
                "plate {i} is {} px wide, its own word wants {wanted} px",
                r.w
            );
        }
        for pair in plates.windows(2) {
            assert!(
                (pair[1].0.x - pair[0].0.right() - gap).abs() < 0.01,
                "the plates are not settings.bar_gap apart"
            );
        }
        let (first, last) = (plates[0].0, plates[plates.len() - 1].0);
        assert!(
            ((first.x - content.x) - (content.right() - last.right())).abs() < 0.01,
            "the bar is not centred: {} px on the left, {} px on the right",
            first.x - content.x,
            content.right() - last.right()
        );
        viewport_home();
    }

    /// M6 in place — the editor's three verbs are one row held against
    /// the bottom edge, and every one of them is a target AT REST.
    ///
    /// They were three centred rows at the far end of a page many
    /// viewports long: reaching CANCEL meant scrolling past every slider
    /// in the theme, and with the page unscrolled none of the three was
    /// on screen at all.
    #[test]
    fn the_editors_verbs_stand_together_under_the_page() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let mut s = furnished();
        s.view = View::ThemeEditor;
        // At rest, with the page not scrolled a pixel.
        s.draw(&mut ctx);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let plate = |act: Act| {
            s.hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .expect("the editor's bar lost one of its verbs")
        };
        let rects: Vec<Rect> = [Act::EditorSave, Act::EditorSaveAs, Act::EditorCancel]
            .iter()
            .map(|a| plate(*a))
            .collect();
        for (i, r) in rects.iter().enumerate() {
            assert!(
                (r.y - (content.bottom() - m.btn_h)).abs() < 0.01,
                "verb {i} does not stand on the bottom edge of the content box"
            );
            assert!((r.h - m.btn_h).abs() < 0.01, "verb {i} is not one row tall");
        }
        for pair in rects.windows(2) {
            assert!(
                pair[1].x >= pair[0].right() - 0.01,
                "two verbs of the bar share pixels"
            );
        }
        // And the flow above them stops short of the bar (P12).
        let view = s.body_box(page(View::ThemeEditor), m, content);
        assert!(
            view.bottom() <= rects[0].y + 0.01,
            "the flow overlaps the action bar by {} px",
            view.bottom() - rects[0].y
        );
        viewport_home();
    }

    // ------------------------------------------------ the three panels

    /// The rail stands on every page of the window, and it says which
    /// page that is out of the theme's own ladder.
    ///
    /// The marker is the point. "Where am I" is a STATE, so it is the
    /// button ladder's `selected` rung — the same rung an unfolded
    /// list's anchor stands on — and not a colour, a bar or an inset
    /// this file made up. What the test reads is therefore the rung and
    /// not a pixel: a theme is free to draw `selected` however it likes,
    /// and free to draw it the same as `idle`, and this still holds.
    #[test]
    fn the_rail_stands_on_every_page_and_says_which_page_it_is() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        for p in PAGES.iter() {
            let mut s = furnished();
            s.view = p.view;
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let at = |act: Act| {
                s.hits.iter().find(|&&(_, a)| a == act).map(|&(r, _)| r)
            };
            for act in rail_acts(&s) {
                let r = at(act).unwrap_or_else(|| {
                    panic!("{}: the rail lost a section", p.title)
                });
                let want = act == rail_act(p.view);
                assert_eq!(
                    rung(s.button_state(&ctx, r, act)) == State::Selected,
                    want,
                    "{}: the rail marks the wrong section",
                    p.title
                );
            }
            // The section's own column marks the page inside it, and
            // marks nothing at all where the page is not one of its
            // entries (the editor, the reset confirmation).
            for act in sub_acts(&s, p.view) {
                let r = at(act).expect("a page of the section was not drawn");
                assert_eq!(
                    rung(s.button_state(&ctx, r, act)) == State::Selected,
                    sub_act(p.view) == Some(act),
                    "{}: the section's column marks the wrong page",
                    p.title
                );
            }
        }
        // A section the machine cannot offer registers nothing at all
        // (R6): grey, and not a target.
        let mut s = furnished();
        s.color_enabled = false;
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        assert!(
            !s.hits.iter().any(|&(_, a)| a == Act::OpenColor),
            "COLOR SPACE answers the pointer with no colour compositor"
        );
        viewport_home();
    }

    /// Every road to another page folds an open list, the rail's roads
    /// included.
    ///
    /// The doors used to say this each for themselves, and that was
    /// enough while every way off a page stood ON that page. The rail is
    /// a way off EVERY page: with an unfolded THEMES list standing, one
    /// press on BLUR used to carry the list onto the blur page, where it
    /// hung over two sliders with no anchor under it and ate the first
    /// Escape. The rule is the road's now ([`Settings::go`]), so a door
    /// added later cannot forget it.
    #[test]
    fn moving_to_another_page_folds_whatever_list_was_open() {
        for (act, opens) in [
            (Act::OpenBlur, View::Blur),
            (Act::OpenGrid, View::Grid),
            (Act::OpenBoards, View::Boards),
            (Act::OpenColor, View::Color),
            (Act::OpenAddons, View::Addons),
            (Act::OpenFont, View::Font),
            (Act::OpenSoundLevels, View::SoundLevels),
            (Act::LookFeelReset, View::LookFeelReset),
            // The section's own page, pressed while standing on it: the
            // road is the same road and folds the list all the same.
            (Act::OpenSets, View::LookFeel),
        ] {
            let mut s = furnished();
            s.view = View::LookFeel;
            s.dropdown = Some(Dropdown::List(ListId::Looks));
            s.perform(act, 0.0);
            assert!(
                s.dropdown.is_none(),
                "a list stayed open behind the page a navigation entry opened"
            );
            assert!(s.view == opens, "the entry opened the wrong page");
        }
    }

    /// The rail's groups are drawn, and they are the rail's own — a
    /// heading is a heading by the same `Ctrl::Section` a page's is.
    #[test]
    fn the_rail_writes_the_headings_its_groups_stand_under() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        let drawn = page_runs(&mut fonts, &mut s);
        for heading in ["APPEARANCE", "DESKTOP", "SYSTEM"] {
            assert!(
                drawn.iter().any(|t| t == heading),
                "the rail did not write {heading}: {drawn:?}"
            );
        }
    }

    /// A second column only where the section has pages, and the two
    /// columns are the same width when both stand (owner, 2026-08-16:
    /// "OBIE kolumny nawigacji RÓWNEJ szerokości").
    #[test]
    fn only_a_section_with_pages_gets_a_second_column() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let with = Panes::of(View::LookFeel, m, content);
        let (rail, sub) = (
            with.rail.expect("no rail"),
            with.sub.expect("LOOK AND FEEL has pages and no column for them"),
        );
        assert!(!with.folded, "the window folded at a width it fits in");
        assert!(
            (rail.w - sub.w).abs() < 0.01,
            "the two navigation columns are {} px and {} px",
            rail.w,
            sub.w
        );
        assert!(
            (sub.x - rail.right() - col_gap()).abs() < 0.01,
            "the gutter between the columns is not settings.col_gap"
        );
        assert!(
            (with.page.x - sub.right() - col_gap()).abs() < 0.01,
            "the page does not start after the second column"
        );
        assert!(
            (with.page.right() - content.right()).abs() < 0.01,
            "the page does not take the whole of the rest"
        );
        // The column and the band it becomes when the window folds are
        // the SAME entries — one table read two ways.
        let (rows, zone) = subrail(View::LookFeel).expect("no second column");
        match zone {
            Zone::Flow { rows: banded, .. } => assert!(
                std::ptr::eq(*banded, rows),
                "the folded window's column is not the column beside the page"
            ),
            _ => panic!("a navigation column is a flow and nothing else"),
        }

        // A section that IS its page: the content starts straight after
        // the rail, and the width the second column would have taken is
        // the page's.
        let without = Panes::of(View::Grid, m, content);
        assert!(without.sub.is_none(), "GRID grew a column of pages");
        assert!(
            (without.page.x - rail.right() - col_gap()).abs() < 0.01,
            "a section without pages still leaves room for a column"
        );
        assert!(
            without.page.w > with.page.w + 0.01,
            "the section without pages did not take the room back"
        );
        viewport_home();
    }

    /// ŻYCZENIE 1, MEASURED HERE. The three columns are three DIFFERENT
    /// colours of ONE hue, and every one of the three comes out of the
    /// theme.
    ///
    /// The window's half of the claim is what this can check: that each
    /// bed is the box [`Panes`] cut and carries the colour ITS OWN TOKEN
    /// resolves to — no fourth colour mixed in Rust, and no two beds
    /// sharing a token. That the three tokens are three rungs of one
    /// ladder is the MASTER's claim and is measured over the master, in
    /// libnacelle (`the_three_settings_bands_are_three_shades_of_one_hue`).
    ///
    /// TWO OF THE THREE ARE PAINTED HERE, and the master names exactly
    /// those two. The page's bed is the WINDOW BODY, `component.panel
    /// .fill`, and that rung is translucent — so laying a bed of it over
    /// the body composes its alpha twice. Measured on the master, over
    /// the field the window stands on: the body's own pixel is #131E19
    /// and the doubled one #15201B, an OKLab dE of 0.0078. It is the
    /// same for the FOLDED window, where the page is the whole interior.
    ///
    /// Every lightness and hue below is read in LINEAR light: OKLCh is
    /// defined over it, the bake answers sRGB-encoded, and the two are
    /// far apart — the rungs read 0.2698 / 0.4036 / 0.4840 encoded
    /// against 0.1150 / 0.1780 / 0.2320 decoded.
    #[test]
    fn the_three_columns_are_three_shades_the_theme_chose() {
        let _g = crate::widgets::theme_test_lock();
        // The MASTER's own bands, so a theme-editor preview left standing
        // by another test is not what this measures — a preview moves
        // `component.panel.fill` (`edit::glass_edits`) and would answer
        // for the body with a colour nobody in this test chose.
        nacelle::theme::clear_preview();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        /// Every rect one call to [`Settings::draw_bands`] laid down.
        fn bands(
            s: &Settings,
            fonts: &mut nacelle::font::FontSystem,
            h: f32,
        ) -> (Vec<([f32; 4], nacelle::theme::Color)>, Panes, Rect) {
            // Recording, so the bands can be read back as commands: the
            // shipping list keeps no register at all.
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(View::LookFeel, m, content);
            s.draw_bands(&mut ctx, &nav);
            let out = ctx
                .dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Rect { r, color } => Some((*r, *color)),
                    _ => None,
                })
                .collect();
            (out, nav, content)
        }
        let lch = |c: nacelle::theme::Color| c.to_linear().to_oklch();
        let hue_gap = |a: f32, b: f32| {
            let d = (a - b).rem_euclid(360.0);
            d.min(360.0 - d)
        };

        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
        assert!(!nav.folded, "the window folded at a width it fits in");
        assert_eq!(drawn.len(), 2, "two columns to bed; the page's bed is the body");
        // Each band is its column's own rectangle — the same cut the
        // rows are laid in, or the bed and what stands on it would
        // disagree about where the column is.
        let boxes: Vec<[f32; 4]> = drawn.iter().map(|(r, _)| *r).collect();
        let same = |b: &[f32; 4], r: Rect| {
            let want = [r.x, r.y, r.w, r.h];
            b.iter().zip(want.iter()).all(|(a, w)| (a - w).abs() < 0.01)
        };
        for (name, r) in [
            ("rail", nav.rail.expect("no rail")),
            ("sub", nav.sub.expect("no column of pages")),
        ] {
            assert!(
                boxes.iter().any(|b| same(b, r)),
                "the {name} column has no bed of its own"
            );
        }
        // Every colour is the one ITS token resolves to: the window
        // carries a name to the theme and paints back what it is given.
        let th = theme::resolved();
        let of = |n: &str| {
            col(th.color(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}"))))
        };
        let want =
            [of("component.settings.rail_fill"), of("component.settings.sub_fill")];
        // And the master names those two and no third: a `page_fill` back
        // in it would be a token this window could only honour by bedding
        // the body a second time.
        assert!(
            nacelle::theme::id("component.settings.page_fill").is_none(),
            "the master named the page's bed again; the body's `panel.fill` is it"
        );
        for (i, (_, got)) in drawn.iter().enumerate() {
            let w = want[i];
            assert!(
                (got.r - w.r).abs() < 1e-6
                    && (got.g - w.g).abs() < 1e-6
                    && (got.b - w.b).abs() < 1e-6,
                "band #{i} was not painted in the colour its token names"
            );
        }
        // AND THE PAGE KEEPS THE BODY'S OWN PIXEL. Nothing is laid over
        // it, so what stands there is what `window::frame` laid.
        let body = col(th.bed(nacelle::theme::id("component.panel.fill").expect("no body")));
        assert!(
            !boxes.iter().any(|b| same(b, nav.page)),
            "the page was bedded a second time over the window body"
        );
        // The measurement that says the defect was real and that this
        // test can still see it: the rung is translucent, and laying it
        // twice does NOT come out where laying it once does. Over the
        // field the window stands on, #131E19 against #15201B.
        assert!(body.a < 1.0, "the body's rung went opaque — this test is measuring nothing");
        let field = of("surface.base");
        let once = nacelle::theme::Color::composite_as_rendered(body, field);
        let twice = nacelle::theme::Color::composite_as_rendered(body, once);
        let doubled = nacelle::theme::Color::delta_e_ok(once.to_linear(), twice.to_linear());
        assert!(
            doubled > 0.005,
            "one coat of the body's rung and two no longer compose to two \
             different pixels ({} vs {}) — the alpha went, and this test can no \
             longer see the defect it guards",
            once.to_hex(),
            twice.to_hex()
        );
        // THREE COLOURS, ONE HUE — the owner's "hue ten sam, odcień
        // koloru inny", read off what the window really shows: the two
        // beds it laid, and the BODY standing where the page is.
        let (page, rail, sub) = (lch(body), lch(want[0]), lch(want[1]));
        for (a, b, n) in [(page, rail, "page/rail"), (page, sub, "page/sub"), (rail, sub, "rail/sub")]
        {
            // Two degrees, which is what libnacelle holds each rung to
            // against the SEED over the master — read in linear light
            // the three sit on ONE number (166.46 deg on the master's
            // mint) because they take their hue from ONE token, and the
            // tolerance is float noise and the sRGB rounding. Read
            // encoded they spread nearly three degrees and this
            // assertion would fail, which is the point of the space.
            assert!(
                hue_gap(a.h, b.h) < 2.0,
                "{n}: two settings beds are two COLOURS ({} vs {} deg), not two shades",
                a.h,
                b.h
            );
            // The master's rungs, decoded: 0.1150 / 0.1780 / 0.2320, so
            // the smaller of the two steps is 0.054.
            assert!(
                (a.l - b.l).abs() > 0.03,
                "{n}: two settings beds are the same shade ({} vs {})",
                a.l,
                b.l
            );
        }

        // FOLDED: NO bed at all. There are no columns to shade
        // differently — the page is the whole interior and the body is
        // already standing on the page's own rung — so the folded window
        // looks exactly as it did before any of this existed.
        let mut folded_seen = false;
        for h in HEIGHTS {
            theme::set_viewport(h, 1.0);
            let (drawn, nav, _) = bands(&s, &mut fonts, h);
            if !nav.folded {
                continue;
            }
            folded_seen = true;
            assert!(drawn.is_empty(), "a folded window bedded its interior twice");
        }
        assert!(
            folded_seen,
            "no window height in HEIGHTS folds — the folded band was never measured"
        );

        // AND A THEME STILL MOVES THE PAGE'S BED — through the token
        // that IS the page's bed. The body's own fill is what stands
        // under the page, so re-pointing it moves the page and nothing
        // in this window has to be told: the two columns keep their own
        // tokens and stay where the theme put them.
        {
            let _t = crate::widgets::Themed::new(
                "page-bed",
                "[component]\npanel.fill = @surface.void\n",
            );
            theme::set_viewport(1080.0, 1.0);
            let th = theme::resolved();
            let moved = col(th.bed(
                nacelle::theme::id("component.panel.fill").expect("no body"),
            ));
            let void =
                col(th.color(nacelle::theme::id("surface.void").expect("no void")));
            assert!(
                (moved.r - void.r).abs() < 1e-6 && (moved.a - void.a).abs() < 1e-6,
                "the fixture did not move the body's bed"
            );
            let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
            assert!(!nav.folded);
            assert_eq!(drawn.len(), 2, "a moved body grew the window a third bed");
        }
        viewport_home();
    }


    /// The rail shows every section it has, at every window height the
    /// program is built for.
    ///
    /// Fail-closed: a rail taller than its box is cut off by its own
    /// clip, and a section cut off is a section no pointer can reach —
    /// the navigation would be the one part of this window with no way
    /// to scroll to what it hides.
    #[test]
    fn the_navigation_fits_the_window_it_stands_in() {
        let _g = crate::widgets::theme_test_lock();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut measured = 0;
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            for view in [View::LookFeel, View::Grid] {
                let nav = Panes::of(view, m, content);
                // Folded, the entries are in the flow and the scroll
                // answers for them — that is the other test's ground.
                let Some(rail) = nav.rail else { continue };
                measured += 1;
                let want = s.rows_h(&RAIL_ROWS, m, rail);
                assert!(
                    want <= rail.h + 0.01,
                    "at {h}px the rail wants {want} px and has {} px",
                    rail.h
                );
                if let (Some(box_), Some(rows)) = (nav.sub, subrail_rows(view)) {
                    let want = s.rows_h(rows, m, box_);
                    assert!(
                        want <= box_.h + 0.01,
                        "at {h}px the section's column wants {want} px and has {} px",
                        box_.h
                    );
                }
            }
        }
        assert!(measured > 0, "no height in the ladder drew a rail at all");
        viewport_home();
    }

    /// M4 in the large — the whole window folds, and the FOCUS CHAIN
    /// does not move a step when it does.
    ///
    /// At the smallest window the three panels cannot all have their
    /// width, so there are no panels: the rail's sections, the section's
    /// pages and the page itself become one vertical list inside the one
    /// scroll, and a band of columns runs its columns one after the
    /// other down that list instead of beside one another. At the
    /// largest, all of it stands side by side. Three things have to
    /// survive that, on EVERY page and at EVERY window the program is
    /// built for:
    ///
    /// * every frame registers in the order the DESCRIPTION writes, and
    ///   never in the order the geometry happens to stand in;
    /// * a sweep from end to end reaches everything the window promises
    ///   — one shape may not hide what another offers;
    /// * so the sequence of `FocusId`s is the SAME sequence at every
    ///   height, which is what M4 is for: Tab walks one route whatever
    ///   shape the window is in, and a reader who learned the route on
    ///   one screen keeps it on another.
    ///
    /// The chain is read by WALKING it — `Nav::Next` from the head until
    /// it comes back round — and not by asking whether an id registered
    /// somewhere in the frame. Tab order is the question, so Tab is what
    /// answers it. The hit map is walked beside it, because a route the
    /// pointer cannot follow is half a window.
    ///
    /// The order is checked FRAME BY FRAME and not across the sweep: a
    /// pinned band is on screen at every stop while the rows above it
    /// come and go, so "the order they were first seen in" is the
    /// scroll's order and not the chain's. Inside one frame the question
    /// is exact — because no control appears twice, a frame that is a
    /// subsequence of the description IS the description with some of it
    /// missing, in the description's own order.
    #[test]
    fn the_window_folds_to_one_list_and_the_chain_keeps_its_order() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();

        /// One frame of one page: the Tab route it built and the targets
        /// it left, each in the order the frame made them and mapped
        /// back to the acts the description names its controls by.
        ///
        /// An id or a target the description does not name — a board's
        /// thumbnail, a row of an open list — is not one of the page's
        /// described controls and has no place in the answer.
        fn frame(
            fonts: &mut nacelle::font::FontSystem,
            view: View,
            h: f32,
            stop: f32,
            named: &[(FocusId, Act)],
        ) -> (Vec<Act>, Vec<Act>) {
            let mut s = furnished();
            s.view = view;
            // Every `Row::when` condition set at once, so the sweep
            // walks the conditional rows as well.
            editor_ajar(&mut s);
            s.scroll.set_offset(stop);
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::new();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, fonts, h, 1.0);
            ctx.focus = Some(&mut fc);
            s.draw(&mut ctx);
            // The chain answers about the last COMPLETED frame, so the
            // frame the drawing built has to be closed before Tab can
            // walk it.
            fc.begin_frame();
            let hits: Vec<Act> = s
                .hits
                .iter()
                .map(|&(_, a)| a)
                .filter(|a| named.iter().any(|(_, n)| n == a))
                .collect();
            let mut chain: Vec<Act> = Vec::new();
            fc.focus(None);
            if !fc.nav(Nav::Next) {
                return (chain, hits);
            }
            let head = fc.focused();
            let mut at = head;
            // Bounded, because a chain that never came back round would
            // hang the suite rather than fail it. The bound is every
            // control the page describes and a board's worth of
            // thumbnails over.
            for _ in 0..named.len() + 64 {
                if let Some((_, act)) = named.iter().find(|(id, _)| Some(*id) == at) {
                    chain.push(*act);
                }
                fc.nav(Nav::Next);
                at = fc.focused();
                if at == head {
                    return (chain, hits);
                }
            }
            panic!("the focus chain never came back to its head");
        }

        /// A frame is the description with some of it missing, never
        /// re-ordered: every act it made stands after the one before it
        /// in the description too.
        fn in_order(frame: &[Act], described: &[Act], page: &str, h: f32, what: &str) {
            let mut want = described.iter();
            for act in frame {
                assert!(
                    want.any(|d| d == act),
                    "{page} at {h}px: {what} is out of the order the description \
                     writes — it registered the places {:?}",
                    frame
                        .iter()
                        .map(|a| described.iter().position(|d| d == a))
                        .collect::<Vec<_>>()
                );
            }
        }

        /// Everything the description promises was reached somewhere in
        /// the sweep. What is missing is named by its PLACE in the
        /// description, which is where it is read.
        fn all_of_it(seen: &[Act], described: &[Act], page: &str, h: f32, what: &str) {
            if let Some(i) = described.iter().position(|a| !seen.contains(a)) {
                panic!(
                    "{page} at {h}px: #{i} of the {} controls it describes {what}",
                    described.len()
                );
            }
        }

        for p in PAGES.iter() {
            let described: Vec<Act> = {
                let mut s = furnished();
                s.view = p.view;
                editor_ajar(&mut s);
                window_acts(&s, p)
            };
            let named: Vec<(FocusId, Act)> =
                described.iter().map(|&a| (focus_id(a), a)).collect();
            // The pointer is owed the same list WITHOUT the corner
            // button, which is dropped from both sides below: it
            // registers at the head of the chain and is PAINTED last, so
            // the hit map is the one place its position is not the
            // chain's ([`Settings::draw`]).
            let pressed: Vec<Act> = described
                .iter()
                .copied()
                .filter(|a| !matches!(a, Act::Close | Act::Back))
                .collect();
            for h in HEIGHTS {
                theme::resolved();
                theme::set_viewport(h, 1.0);
                // Half a viewport per stop, so consecutive stops overlap
                // — every row is far shorter than half a viewport — and
                // the far end is the clamp's own, exactly as the
                // reachability sweep walks a page.
                let stops: Vec<f32> = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    let m = Metrics::of(&ctx, content);
                    let mut s = furnished();
                    s.view = p.view;
                    editor_ajar(&mut s);
                    let stride = (s.body_box(p, m, content).h * 0.5).max(1.0);
                    let length = s.flow_h(p, m, content);
                    let mut out = vec![0.0];
                    let mut at = stride;
                    while at < length {
                        out.push(at);
                        at += stride;
                    }
                    out.push(f32::MAX / 4.0);
                    out
                };
                let mut walked: Vec<Act> = Vec::new();
                let mut pointed: Vec<Act> = Vec::new();
                for stop in stops {
                    let (chain, hits) = frame(&mut fonts, p.view, h, stop, &named);
                    let hits: Vec<Act> = hits
                        .into_iter()
                        .filter(|a| !matches!(a, Act::Close | Act::Back))
                        .collect();
                    in_order(&chain, &described, p.title, h, "the focus chain");
                    in_order(&hits, &pressed, p.title, h, "the hit map");
                    for act in chain {
                        if !walked.contains(&act) {
                            walked.push(act);
                        }
                    }
                    for act in hits {
                        if !pointed.contains(&act) {
                            pointed.push(act);
                        }
                    }
                }
                all_of_it(&walked, &described, p.title, h, "never joined the focus chain");
                all_of_it(&pointed, &pressed, p.title, h, "never became a target");
            }
        }

        // The shapes really are different shapes, or all of the above is
        // one window measured five times. The WINDOW folds at the
        // smallest height and stands in three panels at the largest; a
        // BAND of columns folds with it and stands again once its own
        // columns have the width. Both sides of M4 are therefore walked
        // above, because both are true somewhere in the ladder.
        for (h, window_folded, band_folded) in
            [(HEIGHTS[0], true, true), (HEIGHTS[4], false, false)]
        {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(View::Color, m, content);
            assert_eq!(
                nav.folded, window_folded,
                "the window at {h}px is not the shape this test is about"
            );
            assert_eq!(
                zone_folded(&COLOR_ZONES[0], rows_box(nav.page)),
                band_folded,
                "the COLOR page's band at {h}px is not the shape this test is about"
            );
        }
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

    /// §8.3/2 — everything the window describes is reachable, band by
    /// band and in BOTH shapes of the window.
    ///
    /// Every row that carries an act must land in the hit map AND in the
    /// focus chain: a control the mouse can press but Tab cannot reach
    /// is the bug this window used to have one control at a time. The
    /// BOARDS cross is skipped — it is the one page whose contents no
    /// row describes ([`Ctrl::Custom`]).
    ///
    /// The description is walked band by band, and inside a banded
    /// region column by column ([`described_acts`]), with the window's
    /// own navigation ahead of it ([`window_acts`]) — so a page that
    /// grows a second column is swept in it and not around it.
    ///
    /// TWO SHAPES, because the fold moves the navigation out of its
    /// columns and into the flow: at the smallest window the whole thing
    /// is one list inside the one scroll, at the largest it stands in
    /// three panels, and neither shape may hide what the other offers.
    ///
    /// The hit map is asked of the SWEEP — a target is owed to the
    /// pointer somewhere along the travel, not at every stop of it — and
    /// the chain is asked of EVERY FRAME. That is M5: a row off the
    /// frame keeps its place in the Tab order out of the rect the layout
    /// gave it, so a route with a hole in it is a fault at whatever the
    /// page happens to be scrolled to. Before M5 this could only be the
    /// union too, and a chain that came and went with the scroll passed
    /// it.
    #[test]
    fn every_described_control_is_reachable() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        for h in [HEIGHTS[0], HEIGHTS[4]] {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            // BOTH of the editor's modes. The BASIC page and the
            // ADVANCED page are two bands with two conditions, and a
            // sweep that only ever saw the default one would leave the
            // other's controls — the switch's own neighbours — never
            // asked about. Every other page ignores the flag.
            for (p, basic) in PAGES.iter().flat_map(|p| [(p, false), (p, true)]) {
                let mut hit: Vec<Act> = Vec::new();
                let mut reference = furnished();
                reference.view = p.view;
                editor_ajar(&mut reference);
                reference.editor_basic = basic;
                let described = window_acts(&reference, p);
                // The five hand-written stops died with the whole-theme
                // sections: the editor page is many viewports long now, and a
                // fixed list would go quietly stale on the NEXT section too —
                // reporting a mid-page control unreachable when only the sweep
                // was. So the stops are walked from the page's own length, a
                // half-viewport apart (rows are far shorter than half a
                // viewport, so consecutive stops overlap), and the far end is
                // still the clamp's own MAX/4.
                let stops: Vec<f32> = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    let m = Metrics::of(&ctx, content);
                    let view = reference.body_box(p, m, content);
                    let length = reference.flow_h(p, m, content);
                    let stride = (view.h * 0.5).max(1.0);
                    let mut out = vec![0.0];
                    let mut at = stride;
                    while at < length {
                        out.push(at);
                        at += stride;
                    }
                    out.push(f32::MAX / 4.0);
                    out
                };
                for stop in stops {
                    let mut s = furnished();
                    s.view = p.view;
                    // Every condition set at once, so the reachability sweep
                    // covers the conditional rows as well.
                    editor_ajar(&mut s);
                    s.editor_basic = basic;
                    if stop > 0.0 {
                        s.scroll.set_offset(stop);
                    }
                    let mut fc = FocusCtl::new();
                    let mut dl = nacelle::draw::DrawList::new();
                    fc.begin_frame();
                    let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    ctx.focus = Some(&mut fc);
                    s.draw(&mut ctx);
                    // The chain answers about the last COMPLETED frame, so
                    // the frame the drawing built has to be closed before it
                    // can be read back.
                    fc.begin_frame();
                    hit.extend(s.hits.iter().map(|&(_, a)| a));
                    if let Some(i) =
                        described.iter().position(|a| fc.rect_of(focus_id(*a)).is_none())
                    {
                        panic!(
                            "{} at {h}px: #{i} of the {} controls the window describes \
                             is not in the chain of the frame at {stop} px",
                            p.title,
                            described.len()
                        );
                    }
                }
                for act in &described {
                    assert!(
                        hit.contains(act),
                        "{} at {h}px: a described control is missing from the hit map",
                        p.title
                    );
                }
            }
        }
        viewport_home();
    }

    /// M5, the other half — the keyboard brings what it lands on into
    /// the frame.
    ///
    /// The chain is whole in every frame now, so Tab can land on a row
    /// the page is not showing. Left alone that is worse than the hole
    /// it replaced: the ring would be nowhere, and the row would be
    /// neither seen nor pressable, because Enter reads the hit map and
    /// an unseen row is not in it. So every page is walked with Tab from
    /// end to end, redrawing between presses exactly as the program
    /// does, and after every press whatever the chain landed on that
    /// belongs to the FLOW has to stand inside the box the flow is read
    /// in. What stands in the navigation's own columns is not the
    /// scroll's to move and is not asked (nor is what the page PINS,
    /// which is outside that box by construction and always on screen).
    ///
    /// Both shapes again: folded, the navigation is part of the flow and
    /// is chased with it.
    #[test]
    fn the_keyboard_scrolls_to_whatever_it_lands_on() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let tab = KeyEv { key: FKey::Tab, mods: Mods::NONE, repeat: false, text: None };
        /// One frame, closed so the chain can be read and walked. The
        /// boundary is crossed ONCE per frame — a second crossing would
        /// swap the chain back out and the walk would be reading the
        /// frame before last.
        fn frame(
            fonts: &mut nacelle::font::FontSystem,
            s: &mut Settings,
            fc: &mut FocusCtl,
            h: f32,
        ) {
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, fonts, h, 1.0);
            ctx.focus = Some(fc);
            s.draw(&mut ctx);
            fc.begin_frame();
        }
        let mut walked = 0;
        for h in [HEIGHTS[0], HEIGHTS[4]] {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            for p in PAGES.iter() {
                let mut s = furnished();
                s.view = p.view;
                editor_ajar(&mut s);
                let folded = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    Panes::of(p.view, Metrics::of(&ctx, content), content).folded
                };
                let flowed = flowed_acts(&s, p, folded);
                let mut fc = FocusCtl::new();
                frame(&mut fonts, &mut s, &mut fc, h);
                // Once round the whole chain, and a few presses over.
                for _ in 0..window_acts(&s, p).len() + 8 {
                    s.key(&tab, &mut fc);
                    frame(&mut fonts, &mut s, &mut fc, h);
                    let Some(id) = fc.focused() else { continue };
                    let Some(i) = flowed.iter().position(|a| focus_id(*a) == id) else {
                        continue;
                    };
                    let r = fc.rect_of(id).expect("the chain lost what it just landed on");
                    let view = s.flow.view;
                    walked += 1;
                    assert!(
                        r.y >= view.y - 0.01 && r.bottom() <= view.bottom() + 0.01,
                        "{} at {h}px: the ring on #{i} of the {} rows that flow stands \
                         {:?} outside the frame {:?} the page is read in",
                        p.title,
                        flowed.len(),
                        (r.y, r.bottom()),
                        (view.y, view.bottom())
                    );
                }
            }
        }
        assert!(walked > 0, "the walk never landed on a row of any page's flow");
        viewport_home();
    }

    /// The live acts of a run of navigation rows, in the order the
    /// column registers them. A disabled entry (COLOR SPACE with no
    /// colour compositor) is deliberately not one: R6 says it registers
    /// nothing at all.
    fn nav_row_acts(s: &Settings, rows: &'static [Row]) -> Vec<Act> {
        rows.iter()
            .filter(|r| (r.enabled)(s) && (r.when)(s))
            .filter_map(|r| match r.ctrl {
                Ctrl::Button { act, .. } => Some(act),
                _ => None,
            })
            .collect()
    }

    /// The sections the rail offers this window.
    fn rail_acts(s: &Settings) -> Vec<Act> {
        nav_row_acts(s, &RAIL_ROWS)
    }

    /// The pages the section in force offers beside the rail, if any.
    fn sub_acts(s: &Settings, view: View) -> Vec<Act> {
        subrail_rows(view).map_or_else(Vec::new, |rows| nav_row_acts(s, rows))
    }

    /// Everything the WINDOW promises on one page: the navigation, then
    /// the page's own acts. The order is the order the frame registers
    /// them in, which is what the fold has to keep.
    fn window_acts(s: &Settings, page: &'static Page) -> Vec<Act> {
        let mut out = described_acts(s, page);
        // The chrome first, then the navigation, then the rest of the
        // page: `described_acts` puts the corner button at its head.
        let rest = out.split_off(1);
        out.extend(rail_acts(s));
        out.extend(sub_acts(s, page.view));
        out.extend(rest);
        out
    }

    /// Every act a page promises, chrome included — walked band by band
    /// and, inside a banded region, column by column, which is the order
    /// the window registers them in. The NAVIGATION is not here: it is
    /// the window's and not the page's, and a page that claimed it would
    /// make every "is this control on the right page" test say yes.
    fn described_acts(s: &Settings, page: &'static Page) -> Vec<Act> {
        let mut out = vec![match chrome_of(page.view) {
            Chrome::Close => Act::Close,
            Chrome::Back => Act::Back,
        }];
        out.extend(page_rows(page, s).flat_map(|row| row_acts(s, row)));
        out
    }

    /// The acts of the bands that FLOW, in registration order: what the
    /// window's one scroll is answerable for.
    ///
    /// A pinned band stands outside the box the flow is read in and is
    /// always on screen, so it is not the scroll's; the navigation is
    /// the scroll's only where the window has FOLDED, which is when its
    /// entries are bands of the flow like any other
    /// ([`Settings::frame_zones`]).
    ///
    /// This reads the description the way [`described_acts`] does and
    /// the way the window registers it — an independent reading of the
    /// same table, which is what lets it check the drawing rather than
    /// echo it.
    fn flowed_acts(s: &Settings, page: &'static Page, folded: bool) -> Vec<Act> {
        let mut out: Vec<Act> = Vec::new();
        if folded {
            out.extend(rail_acts(s));
            out.extend(sub_acts(s, page.view));
        }
        for zone in page.zones {
            if matches!(zone, Zone::Pinned { .. }) || !zone_shown(zone, s) {
                continue;
            }
            out.extend(zone_rows(zone).flat_map(|row| row_acts(s, row)));
        }
        out
    }

    /// Every act ONE row offers, in the order it registers them — the
    /// description's own answer to what [`Settings::targets`] places.
    ///
    /// A hidden row IS NOT THERE (`Row::when`) and a disabled one
    /// registers nothing (R6), so neither offers anything; the sweeps
    /// draw with every condition set, so the conditional rows are
    /// exercised all the same.
    fn row_acts(s: &Settings, row: &'static Row) -> Vec<Act> {
        if !(row.enabled)(s) || !(row.when)(s) {
            return Vec::new();
        }
        match &row.ctrl {
            Ctrl::Toggle { act, .. }
            | Ctrl::Slider { act, .. }
            | Ctrl::Cycle { act, .. }
            | Ctrl::Button { act, .. } => vec![*act],
            Ctrl::Chips { values, act, .. } => values.iter().map(|v| act(*v)).collect(),
            // Every verb of an action bar, left to right.
            Ctrl::Bar { items } => items.iter().map(|&(_, a)| a).collect(),
            // The anchor alone: what the list holds is only on screen
            // while it is open, which is another test's question
            // (`an_open_list_offers_every_name_it_has`).
            Ctrl::Drop { list } => vec![Act::ListBtn(*list)],
            Ctrl::Section { .. }
            | Ctrl::Note { .. }
            | Ctrl::Hint { .. }
            | Ctrl::Custom { .. } => Vec::new(),
        }
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
