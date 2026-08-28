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
//! TWO PANELS, NOT A STACK OF PAGES (owner, 2026-08-16, the
//! specification's annex; one column instead of two by his mock-up of
//! 2026-08-18). The window carries a permanent navigation RAIL down its
//! left edge — every section of the window under the headings its group
//! stands for — and what is left is the page. There is no MENU page any
//! more: the window opens on LOOK AND FEEL, the rail is how a section is
//! reached, and Escape from a section is the window's own last layer
//! rather than a step back to a menu that no longer exists.
//!
//! A SECTION THAT HAS PAGES OF ITS OWN UNFOLDS THEM UNDER ITSELF
//! ([`Ctrl::Expander`]), indented by `settings.rail_indent` and propped
//! against the hairline `settings.rail_guide_*` describes — the shape
//! GNOME calls an expander row. They used to stand in a SECOND column
//! beside the rail, and the column is gone with the token that sized it:
//! two columns of navigation spent a fifth of the window saying what one
//! indent says, and the deeper the sections go the worse that trade
//! gets.
//!
//! THE FOLD IS A STATE OF ITS OWN and every section comes up SHUT
//! ([`Settings::rail_open`], owner's reports 1 and 2 of 2026-08-18). A
//! press on a section turns its fold over and goes nowhere; the entries
//! it reveals are the doors. It was the VIEW read a second way until
//! then — which made the rail open on the section the window opens on
//! and left that section's own entry with nothing it could do.
//!
//! The layout is FLEX, and on ONE measurement: WIDTH. Where the two
//! panels cannot both have their width — `settings.col_min_w` for the
//! page, with the usual device-px floor ([`Panes::of`]) — the whole
//! window folds into ONE vertical list: the rail's entries first, a
//! section's pages still under it, then the page itself, all inside the
//! one scroll. The Tab order is the same in both shapes, because
//! registration follows the DESCRIPTION and never the geometry.
//!
//! THE RAIL SCROLLS, WHICH IS WHY HEIGHT IS NOT A SECOND THRESHOLD. A
//! section's pages live in the rail now, so the column can want more
//! height than the window has — at 720p on the master it wants about
//! 440 px and has about 418. A first draft folded the whole window at
//! those heights, and that was a regression the size of a laptop: it
//! took the two-column shape away from every screen from 720p up to
//! 768p (and, on a machine with no colour manager, to 800p), which had
//! stood in columns before. The toolkit already answers "content that
//! does not fit" — [`nacelle::view::scroll`] — so the rail answers with
//! it: its own offset, its own bar in its own lane, the wheel where the
//! pointer stands over it, and the same off-frame registration the page
//! uses. Two scrolls in one window, and the walker says which one
//! carries a run ([`Carrier`]) so a keyboard chase moves the right one.
//! It follows the frame as well: a row a scroll has carried out of
//! sight is not drawn and is not a target, but it keeps its place in
//! the route out of the rect the layout gave it, and the scroll goes
//! and fetches back whatever the keyboard lands on
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
use nacelle::access::{AccessInfo, Role};
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
use nacelle::view::{CtxSurface, Snap, Surface};
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
/// so every section of the rail and every page a section unfolds
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

/// Which single number of the editor a track moves.
///
/// NO COLOUR IS ON THIS LIST ANY MORE (owner, ZGŁOSZENIE 4, 2026-08-18).
/// It used to hold thirty-nine of them — three tracks apiece for the
/// border, the two glass quads, the accent, a severity role, the focus
/// ring, three menu tokens, three tooltip tokens and the scrollbar's
/// groove — because the theme writes a colour as `oklch(L, C, H)` and
/// three tracks is the shape of that value. Three tracks is a poor way to
/// CHOOSE one, though, and every one of the thirteen is a
/// [`Ctrl::Picker`] now ([`PickerId`]). What is left here is what a track
/// is genuinely good at: one number along one axis, with two ends and a
/// middle.
///
/// Named rather than indexed because a swapped pair would be a value that
/// is merely wrong instead of a compile error.
///
/// Most of these variants lost their row in 2026-08-23's picker-only
/// simplification of `EDITOR_ROWS` (the ADVANCED page): `editor_edits`
/// still answers every one of them correctly for the `Settings` fields
/// that feed it, and the tests pinning that answer are the reason each
/// variant stayed a working, tested read on the model rather than a stub
/// waiting to be reinvented if a control for it returns — which is
/// exactly what happened to [`EdgeWidth`](Knob::EdgeWidth) on
/// 2026-08-25, BORDER SIZE's own row reusing it whole. The rest are
/// still waiting, which is what the blanket allow below is for.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Knob {
    /// The BORDER's own thickness — `border.edge.width`, ZGŁOSZENIE 6's
    /// first reading of "promień borderu". NOT `stroke.hair`, which is
    /// the global kerf 72 derivations share and which this page already
    /// offers under HAIRLINE.
    ///
    /// BORDER SIZE reads and writes this (BASIC, 2026-08-25) — back
    /// after standing rowless from 2026-08-23, when it left unconditionally
    /// and never stood on ADVANCED in between. `border_width_edit` and the
    /// two tests pinning it (`the_borders_thickness_and_its_lights_reach_
    /// are_two_answers...`, `a_light_that_is_not_drawn_is_not_asked_how_
    /// far_it_reaches`) were the working, tested answer the new row
    /// reused rather than re-derived.
    EdgeWidth,
    /// The whole effect's opacity, every kind — and the editor's ONE
    /// transparency, standing on BOTH pages since 2026-08-18.
    ///
    /// HOW FAR IT REACHES IS THE PAGE'S, not this knob's: on BASIC the
    /// alpha lands on the object's own bed alone (the owner's ZGŁOSZENIE
    /// 7), on ADVANCED it dresses every reachable rung as it always did.
    /// Both are written from one field, so the two pages can never be
    /// showing different numbers for one thing — the answer is in
    /// [`Settings::editor_edits`], where the kind is already known.
    BgOpacity,
    /// The blur pyramid depth, BLUR and FROSTED.
    BgDepth,
    /// The wash's coverage, FROSTED only.
    BgCoverage,
    // ---- the whole-theme groups (2026-08-16): one knob per number the
    // ---- model in theme/edit.rs takes, nothing that has no set to join.
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
    RingDash,
    RingGap,
    /// `glow.focus_ring.alpha`, 0..1.
    HaloAlpha,
    /// `focus.unfocused_dim`, the declared 0.3..1.0 floor kept by the
    /// track's own range (30..100).
    UnfocusedDim,
    /// The context menu's four tokens: bed, ring, ring width, hint ink.
    MenuEdgeW,
    /// The tooltip's four, the menu's sibling float.
    TipEdgeW,
    /// The scrollbar's widths (0..100 over 0.5u..4u), its fade
    /// (0..100 over 0..2000ms) and the groove's colour.
    BarW,
    BarWHover,
    BarFade,
    // ---- BASIC's three knobs stood here from 2026-08-17 to
    // ---- 2026-08-18: a rotation, a multiplier and an offset over the
    // ---- ten authors. The MOVE they wrote is still exactly what that
    // ---- page sends the theme (`Settings::tone`, `theme::edit::Tone`);
    // ---- what is gone is the three TRACKS, which the owner replaced
    // ---- with a picker. A knob is a slider's name, so a page with no
    // ---- slider has no knob — and `Act::Picker*` names the parts of
    // ---- the control that took their place.
}

/// Which of the editor's switches a toggle row flips. Named like [`Knob`]
/// and for the same reason: a swapped pair would be a switch that is
/// merely wrong instead of a compile error.
///
/// Every toggle row that used to flip one of these left `EDITOR_ROWS` in
/// 2026-08-23's picker-only simplification — see [`Knob`]'s note, which
/// applies here the same way.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flip {
    /// OFF restores `surface.hue = @hue.accent` as a REFERENCE (the
    /// model's `SurfaceHue::FollowAccent`); ON cuts the surfaces loose
    /// with plain degrees from the HUE track under it.
    SurfaceOwnHue,
    /// `focus.ring.enabled`. OFF is the flag alone — the model leaves
    /// the ring's whole dress standing, LINE's lesson.
    Ring,
    /// `glow.focus_ring.enabled`, dressing itself like a lit border kind
    /// on a theme whose halo has no radius yet. The ring's halo took no
    /// part in the tube of 2026-08-18 — the owner's scope was the frame of
    /// the whole object and nothing inside it.
    Halo,
    /// `scrollbar.auto_hide`; the FADE track appears with it, because
    /// the declaration reads the fade only while this is on.
    BarAutoHide,
    /// `scrollbar.track`; OFF is the switch alone and the theme's own
    /// groove colour survives the trip.
    BarTrack,
}

#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// COLOR's HDR switch. It persists NOTHING of its own: what it
    /// writes is a colour space, and "HDR is on" is read back off that
    /// space. See [`Settings::flip_hdr`] for the whole of it.
    ColorHdr,
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
    /// BASIC's corner SIZE, stepped through the master's own named scale
    /// — see [`corner_step_word`].
    EditorCornerStep,
    /// One of the theme editor's colour tracks.
    EditorTrack(Knob),
    /// One of the theme editor's switches ([`Flip`]). No row constructs
    /// this any more — see [`Flip`]'s note.
    #[allow(dead_code)]
    EditorFlip(Flip),
    /// Write the edit set into the theme in force — or, for `default`,
    /// fall through to SAVE AS: the master is not a file.
    EditorSave,
    /// Write the edit set under a new name, asked for in a prompt.
    EditorSaveAs,
    /// Drop the preview and reseed the controls from the theme.
    EditorCancel,
    /// Opens the wallpaper path prompt ([`NamingFor::WallpaperPath`]),
    /// pre-filled with `Settings::backdrop_image` when one is already
    /// chosen — the BACKDROP's own door, matching SAVE AS's for the
    /// theme's name.
    EditorWallpaperEdit,
    /// Drops the chosen wallpaper and marks the backdrop touched, so
    /// SAVE writes `backdrop.source = solid` rather than leaving the
    /// theme's own word standing unexamined.
    EditorWallpaperClear,
    /// The colour picker's parts, one act each
    /// ([`nacelle::object::color_picker::Part`]).
    ///
    /// SIX KINDS AND NOT ONE, because they are six answers to different
    /// questions and the window's whole event road is built on an act
    /// naming what a press MEANS: a slider is dragged (with WHICH channel
    /// carried as its own field, since the wheel's two-axis field and its
    /// bar collapsed into one bank of interchangeable 1-D tracks — the
    /// slider-bank rewrite, 2026-08-24), the notation plate steps, the
    /// value plate opens the inline editor, a ready-made colour is picked
    /// in one press, and the bank cell writes the current colour into the
    /// row of the user's own. One act with a part inside it would have
    /// been shorter and would have put the same focus id on every rect.
    ///
    /// AND EACH ONE NAMES ITS PICKER ([`PickerId`]), since 2026-08-18 —
    /// the owner asked for a picker at EVERY place a colour is chosen,
    /// and there are fourteen of them. The alternative was one picker and
    /// a "which colour is it standing on" field somewhere else, which is
    /// the same fault the rail's fold had: two facts that can disagree,
    /// and no half able to notice.
    PickerSlider(PickerId, usize),
    PickerFormat(PickerId),
    /// The value written out. A press opens the inline editor over it
    /// (`Settings::editing_picker`) — see that field's own doc for the
    /// whole of how typing a colour in place works end to end.
    PickerText(PickerId),
    PickerBase(PickerId, usize),
    PickerCustom(PickerId, usize),
    PickerAdd(PickerId),
    FamilyBtn(Sect),
    WeightBtn(Sect),
    FamilyPick(Sect, usize),
    WeightPick(Sect, usize),
}

/// Font section: terminal or the rest of the interface.
#[derive(Clone, Copy, PartialEq, Debug)]
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
        // The pages a section unfolds under itself on the rail.
        OpenSets => FocusId::of("settings.lookfeel.sets"),
        OpenFont => FocusId::of("settings.lookfeel.fonts"),
        OpenSoundLevels => FocusId::of("settings.lookfeel.sound_levels"),
        EditorSave => FocusId::of("settings.editor.save"),
        EditorSaveAs => FocusId::of("settings.editor.saveas"),
        EditorCancel => FocusId::of("settings.editor.cancel"),
        EditorWallpaperEdit => FocusId::of("settings.editor.backdrop.edit"),
        EditorWallpaperClear => FocusId::of("settings.editor.backdrop.clear"),
        EditorMode => FocusId::of("settings.editor.mode"),
        EditorCornerStep => FocusId::of("settings.editor.corner.step"),
        EditorTrack(k) => FocusId::of(match k {
            Knob::EdgeWidth => "settings.editor.border.width",
            Knob::BgOpacity => "settings.editor.bg.opacity",
            Knob::BgDepth => "settings.editor.bg.depth",
            Knob::BgCoverage => "settings.editor.bg.coverage",
            Knob::SurfHue => "settings.editor.surface.hue",
            Knob::SurfLift => "settings.editor.surface.lift",
            Knob::SurfChroma => "settings.editor.surface.chroma",
            Knob::TextLift => "settings.editor.text.lift",
            Knob::TextChroma => "settings.editor.text.chroma",
            Knob::CornerSm => "settings.editor.corner.sm",
            Knob::CornerMd => "settings.editor.corner.md",
            Knob::CornerLg => "settings.editor.corner.lg",
            Knob::CornerSeg => "settings.editor.corner.segments",
            Knob::Hairline => "settings.editor.stroke.hair",
            Knob::RingW => "settings.editor.ring.w",
            Knob::RingOffset => "settings.editor.ring.offset",
            Knob::RingDash => "settings.editor.ring.dash",
            Knob::RingGap => "settings.editor.ring.gap",
            Knob::HaloAlpha => "settings.editor.ring.halo_alpha",
            Knob::UnfocusedDim => "settings.editor.focus.dim",
            Knob::MenuEdgeW => "settings.editor.menu.edge.w",
            Knob::TipEdgeW => "settings.editor.tooltip.edge.w",
            Knob::BarW => "settings.editor.scrollbar.w",
            Knob::BarWHover => "settings.editor.scrollbar.w_hover",
            Knob::BarFade => "settings.editor.scrollbar.fade",
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
            ListId::Backgrounds => "settings.editor.background",
            ListId::Severities => "settings.editor.severity",
            ListId::Corners => "settings.editor.corner",
            ListId::RingStyles => "settings.editor.ring.style",
            ListId::ScrollModes => "settings.editor.scrollbar.mode",
            ListId::ScrollEdges => "settings.editor.scrollbar.edge",
            // ONE id across the switch, deliberately: the anchor keeps
            // its place in the Tab round when HDR turns, because it is
            // the same control wearing a different word.
            ListId::Spaces => "settings.color.space",
        }),
        // A name's row is its index, with nothing added: the list
        // object is handed the names alone, so `base.item(i)` is what
        // it registers and what a click must agree with.
        Pick(l, i) => dropdown_base(Dropdown::List(l)).item(i),
        // A path of its own, and not `themes.list.item(0)`, because the
        // door is no longer a row of that list — an id derived from the
        // list would now collide with its first theme.
        ThemesEditor => FocusId::of("settings.lookfeel.themes.editor"),
        // The picker's parts, under one path each, DERIVED BY THE PICKER
        // THEY BELONG TO: `item` is a child id and it chains, so a part
        // is `<part path> -> which picker -> which cell` (a slider adds a
        // THIRD link, which channel) and fourteen pickers can stand on
        // one page without two of them sharing a ring. The index is the
        // picker's place in [`PickerId::ALL`], which is a declaration
        // order like a list's rows'.
        PickerSlider(p, i) => {
            FocusId::of("settings.editor.picker.slider").item(p.idx()).item(i)
        }
        PickerFormat(p) => FocusId::of("settings.editor.picker.format").item(p.idx()),
        PickerText(p) => FocusId::of("settings.editor.picker.text").item(p.idx()),
        PickerBase(p, i) => {
            FocusId::of("settings.editor.picker.base").item(p.idx()).item(i)
        }
        PickerCustom(p, i) => {
            FocusId::of("settings.editor.picker.custom").item(p.idx()).item(i)
        }
        PickerAdd(p) => FocusId::of("settings.editor.picker.add").item(p.idx()),
        LookFeelReset => FocusId::of("settings.lookfeel.reset"),
        LookFeelResetConfirm => FocusId::of("settings.lookfeel.reset.confirm"),
        BlurRadiusTrack => FocusId::of("settings.blur.radius"),
        BlurOpacityTrack => FocusId::of("settings.blur.opacity"),
        ColorDepth(bits) => FocusId::of("settings.color.depth").item(bits as usize),
        ColorHdr => FocusId::of("settings.color.hdr"),
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
        Dropdown::List(ListId::Backgrounds) => "settings.editor.background.list",
        Dropdown::List(ListId::Severities) => "settings.editor.severity.list",
        Dropdown::List(ListId::Corners) => "settings.editor.corner.list",
        Dropdown::List(ListId::RingStyles) => "settings.editor.ring.style.list",
        Dropdown::List(ListId::ScrollModes) => "settings.editor.scrollbar.mode.list",
        Dropdown::List(ListId::ScrollEdges) => "settings.editor.scrollbar.edge.list",
        Dropdown::List(ListId::Spaces) => "settings.color.space.list",
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
    // TODO(a11y): `act`'s Debug form is a placeholder, not a curated
    // name — this window's `Act` enum has hundreds of variants and
    // giving each a proper human-readable label (the way libnacelle's
    // own object/ widgets do) is its own follow-up pass, out of scope
    // for wiring the merge through.
    let ring = ctx.focus.as_deref_mut().map_or(false, |fc| {
        fc.register(focus_id(act), r, Caps::NONE, AccessInfo::new(Role::Button, format!("{act:?}")))
            .ring
    });
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
/// The lightness/chroma anchors the two family pickers lead their ladders
/// off: the base bed's L (`surface.base`, default.theme 5.5) and the
/// primary text's L, each with a representative low chroma. They fix where
/// a picked colour's L and C land on the lift/chroma seeds — approximate by
/// design (the depth relationship is exact, the anchor is a live-tuning
/// number), which is why they sit here as named constants and not in a bake.
const BG_ANCHOR_L: f32 = 0.178;
const BG_ANCHOR_C: f32 = 0.05;
const TXT_ANCHOR_L: f32 = 0.905;
const TXT_ANCHOR_C: f32 = 0.02;

fn hsv_track_of(c: nacelle::theme::Color) -> [u32; 3] {
    let (h, sat, v) = rgb_to_hsv(c.r, c.g, c.b);
    [
        (v * 100.0).round().clamp(0.0, 100.0) as u32,
        (sat * 100.0).round().clamp(0.0, 100.0) as u32,
        h.rem_euclid(360.0).round().clamp(0.0, 359.0) as u32,
    ]
}

/// An HSV track back to a colour: the inverse of [`hsv_track_of`], and
/// the ONE map this way for the reason there is one map the other.
///
/// The readers are the write-out (`editor_edits`, where a track becomes a
/// value in the file), BASIC's fold, which carries a track BASIC's move
/// touched but does not author, the SEEDING OF THE PICKERS
/// ([`Settings::seed_pickers_from_tracks`], since 2026-08-18 — a control
/// has to open on the value it stands for), and the tests that measure
/// any of them.
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
static LABEL_MIN: OnceLock<TokenId> = OnceLock::new();
static LABEL_MAX: OnceLock<TokenId> = OnceLock::new();
static VALUE_COL: OnceLock<TokenId> = OnceLock::new();
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

/// Which digit the face sets widest, at this size.
///
/// A face sets `1` narrower than `0`, so a column measured against the
/// number a track happens to carry is a column the next number overflows:
/// HUE reads 359 at the end of its range and 300 in the middle of it, and
/// the wider of the two is the one nobody measured.
///
/// The face's own digits, never a table of numbers here: `type.data` is
/// declared `tabular` in the master and the roles this window writes in
/// are not, so which digit is widest is the FONT's answer to give.
///
/// The answer depends on the face and the size and on nothing else, so
/// it is asked ONCE PER BAND and carried into [`widest_run`] from there
/// ([`Settings::columns`]). Asked per row it would rasterise the same
/// ten glyphs once for every track on the page, and the editor's
/// ADVANCED page is eighty-six rows drawn sixty times a second.
fn widest_digit(ctx: &mut Ctx, t: &Type) -> char {
    let mut buf = [0u8; 4];
    let mut widest = ('0', 0.0f32);
    for d in '0'..='9' {
        let seen = ctx.fonts.measure(t.face, t.px, d.encode_utf8(&mut buf), 0.0);
        if seen > widest.1 {
            widest = (d, seen);
        }
    }
    widest.0
}

/// How wide `text` can get once its DIGITS are allowed to be any digit —
/// the width to reserve for a number that has not been written yet.
///
/// Every value a track can reach has at most the digit count of its top
/// of range, and no digit is wider than `widest` ([`widest_digit`]), so
/// the substitution is an upper bound that no position of the slider can
/// beat. What that bound is worth is measured against the values
/// themselves, not against this arithmetic
/// (`a_row_does_not_write_its_label_its_control_and_its_value_over_one_another`).
fn widest_run(ctx: &mut Ctx, t: &Type, text: &str, widest: char) -> f32 {
    let worst: String =
        text.chars().map(|c| if c.is_ascii_digit() { widest } else { c }).collect();
    ctx.fonts.measure(t.face, t.px, &worst, t.track)
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ListId {
    Looks,
    Layauts,
    Sounds,
    /// The editor's background kind. Unlike the three above it names no
    /// file: its members are the shapes a surface's back can take, and
    /// choosing one lays a value over the theme instead of writing a
    /// config line.
    Backgrounds,
    /// The severity role the picker under it pins — §5.10's closed
    /// set, offered whole because each role is its own author token.
    /// No dropdown offers it any more (2026-08-23) — see [`Knob`]'s note.
    #[allow(dead_code)]
    Severities,
    /// The one corner shape the whole interface wears (`corner.mode`) —
    /// a right angle, a radius or a chamfer.
    Corners,
    /// How the focus ring is stroked (`focus.ring.style`). No dropdown
    /// offers it any more (2026-08-23) — see [`Knob`]'s note.
    #[allow(dead_code)]
    RingStyles,
    /// Whether the scrollbar takes layout space (`scrollbar.mode`). No
    /// dropdown offers it any more (2026-08-23) — see [`Knob`]'s note.
    #[allow(dead_code)]
    ScrollModes,
    /// Which side of the content the bar sits on (`scrollbar.edge`). No
    /// dropdown offers it any more (2026-08-23) — see [`Knob`]'s note.
    #[allow(dead_code)]
    ScrollEdges,
    /// COLOR's colour space. The one list in the window whose NAME and
    /// whose CONTENTS both move: the HDR switch under it turns SPACE
    /// into SPACE HDR and swaps the half of `config::COLOR_SPACE_TABLE`
    /// it offers. ONE list and not two rows — the owner's rule is that
    /// SPACE and SPACE HDR may never stand on the screen together, and
    /// a hidden row beside a shown one is exactly the arrangement where
    /// they could.
    Spaces,
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
    ///
    /// A QUESTION put to the window and no longer a property of the
    /// identity alone, because one list's word is not one word: COLOR's
    /// SPACE wears SPACE HDR while the switch under it is on. The list
    /// is the SAME list either way — same `ListId`, same focus id, same
    /// anchor rect — and that is the point of asking here rather than
    /// describing a second row that would sometimes be hidden.
    fn label(self, s: &Settings) -> &'static str {
        match self {
            ListId::Looks => "THEMES",
            ListId::Layauts => "LAYAUTS",
            ListId::Sounds => "SOUNDS",
            ListId::Backgrounds => "BACKGROUND",
            ListId::Severities => "SEVERITY ROLE",
            // NOT "corner cut". The set it offers is SQUARE, ROUND and
            // CHAMFER — three SHAPES, of which a cut is one — and the
            // owner looked straight past this row for exactly that
            // reason: a word that names one member cannot name the
            // question. `corner.mode` is still what it writes; only the
            // word the eye reads changed.
            ListId::Corners => "CORNER SHAPE",
            ListId::RingStyles => "RING STYLE",
            ListId::ScrollModes => "SCROLLBAR MODE",
            ListId::ScrollEdges => "SCROLLBAR EDGE",
            ListId::Spaces if s.color_hdr => "SPACE HDR",
            ListId::Spaces => "SPACE",
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
            ListId::Backgrounds => "NO BACKGROUND KINDS",
            ListId::Severities => "NO SEVERITY ROLES",
            ListId::Corners => "NO CORNER SHAPES",
            ListId::RingStyles => "NO RING STYLES",
            ListId::ScrollModes => "NO SCROLLBAR MODES",
            ListId::ScrollEdges => "NO SCROLLBAR EDGES",
            // Reachable, unlike the built-in sets above: a compositor
            // that offers none of the spaces this program can name
            // leaves the list holding nothing. The switch is gone by
            // then, so this is the standard-range side saying so.
            ListId::Spaces => "NO COLOUR SPACES",
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

/// Which act one part of the colour picker answers to.
///
/// The ONE map between the toolkit's parts and this window's acts. Both
/// directions of the seam go through it — what the drawing registers,
/// what the hit map records and what the description says the page
/// offers — so the three cannot name the same rect differently.
/// WHICH colour a picker on the page is pointing at.
///
/// The owner's ZGŁOSZENIE 4, 2026-08-18: "the picker is to be everywhere
/// there are colours, ADVANCED included". ADVANCED asked for a colour with
/// THREE SLIDERS — brightness, saturation, hue — thirteen times over, and
/// three tracks is the shape of the VALUE and not a way of choosing one.
/// Every one of those thirteen is a picker now, and BASIC's is the
/// fourteenth.
///
/// WHAT EACH ONE IS ANCHORED TO. `Tone` is BASIC's and stands alone: it
/// writes no field of this window, it writes a RELATIVE MOVE
/// ([`Settings::set_tone_from_picker`]), because the whole of BASIC is
/// "how far from what the theme already says". The other thirteen each
/// stand on the `[u32; 3]` HSV track the three sliders used to write, and
/// that is deliberate rather than lazy: `editor_edits` reads those arrays
/// and knows nothing about controls, so the page's meaning did not move
/// when its controls did.
///
/// AND THE STATE OF A PICKER IS A MODEL, NOT A NUMBER, which is why the
/// window keeps fourteen [`nacelle::object::color_picker::Picker`]s beside
/// the tracks instead of rebuilding one from the track each frame: the
/// object holds the hue a drag onto the grey axis would otherwise lose,
/// and a colour with no chroma has to remember which way it came from or
/// the field's cursor jumps to red the moment a hand crosses the middle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PickerId {
    /// BASIC's ONE question — the theme colour.
    Tone,
    /// The ONE border colour, worn by every object's edge — the window and
    /// panel border (`border.default`), the menu's and the tooltip's. Both
    /// lit kinds included; the halo has none of its own (`theme::edit::Border`).
    Edge,
    /// The ONE text colour: it LEADS the whole text ladder through the hue,
    /// lift and chroma seeds, and is worn by the menu's hint and the
    /// tooltip's text besides.
    Text,
    /// The main background — the picked colour LEADS the six-level surface
    /// ladder (hue, lift, chroma), so the beds keep their depth.
    BgMain,
    /// The glass TINT — the multiply quad, which can only darken.
    Tint,
    /// The glass WASH — the alpha-over quad, the only one that brightens,
    /// and what SOLID reads its colour from.
    Wash,
    /// `palette.accent`, ADVANCED's own hand on the one seed.
    Accent,
    /// The CHOSEN severity role's author colour. Which role that is, is
    /// the SEVERITY list's answer; this picker only ever writes the one
    /// standing, and marks it touched.
    Severity,
    /// The focus ring's stroke.
    Ring,
    /// The menu's own background.
    MenuFill,
    /// The tooltip's own background.
    TipFill,
    /// The scrollbar's groove.
    BarTrack,
}

impl PickerId {
    /// Declaration order, which is what [`PickerId::idx`] and every focus
    /// id derived from it stand on.
    const ALL: [PickerId; 12] = [
        PickerId::Tone,
        PickerId::Edge,
        PickerId::Text,
        PickerId::BgMain,
        PickerId::Tint,
        PickerId::Wash,
        PickerId::Accent,
        PickerId::Severity,
        PickerId::Ring,
        PickerId::MenuFill,
        PickerId::TipFill,
        PickerId::BarTrack,
    ];

    fn idx(self) -> usize {
        PickerId::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

fn picker_act(id: PickerId, part: nacelle::object::color_picker::Part) -> Act {
    use nacelle::object::color_picker::Part;
    match part {
        Part::Slider(i) => Act::PickerSlider(id, i),
        Part::Format => Act::PickerFormat(id),
        Part::Text => Act::PickerText(id),
        Part::Base(i) => Act::PickerBase(id, i),
        Part::Custom(i) => Act::PickerCustom(id, i),
        Part::Add => Act::PickerAdd(id),
    }
}

// `active_gamut_space` and its `GamutSpace` wiring left with the wheel
// (the slider-bank rewrite, 2026-08-24): a bank of 1-D tracks has no
// honest place for a 2-D gamut-boundary shape, and
// `nacelle::object::color_picker`'s own module header explains why at
// length rather than silently dropping the question. `color_space` and
// `color_enabled` stay on `Settings` regardless — the HDR/ICC switches on
// the COLOR page still read them directly.

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
    /// A row of segments, one of them on: COLOR's DEPTH.
    ///
    /// `values` is a QUESTION and not a table, for the reason
    /// [`Ctrl::Slider`]'s `step` is one: the set on offer is not the
    /// same set in every state of the page. Eight bits is not among the
    /// depths while HDR is on ([`Settings::flip_hdr`] says why), and a
    /// segment that cannot give a good picture is not shown greyed —
    /// what the machine cannot do is not on the screen.
    Chips {
        label: &'static str,
        values: fn(&Settings) -> &'static [u32],
        get: fn(&Settings) -> u32,
        act: fn(u32) -> Act,
    },
    /// A value that steps to the next on every press: COLOR's LUT and
    /// ICC, and the theme editor's BASIC/ADVANCED. SPACE was one of
    /// these until it grew a second offer — a cycler cannot say which
    /// of two sets it is stepping through, and the HDR switch needs a
    /// control that can ([`Ctrl::Drop`]).
    Cycle { label: &'static str, get: fn(&Settings) -> String, act: Act },
    /// One of LOOK AND FEEL's three lists: an anchor wearing the LIST'S
    /// OWN NAME (decision §2b), and the list itself unfolding from its
    /// bottom edge when it is the open one
    /// ([`Settings::draw_open_dropdown`]). Which member is in force is
    /// said by the open list, on the row that wears the ladder's
    /// `selected` rung.
    Drop { list: ListId },
    Button { label: Text, kind: BtnKind, act: Act },
    /// A rail entry that HAS PAGES OF ITS OWN: the section's button,
    /// the disclosure triangle that promises them, and the entries
    /// themselves, which stand under it — one `settings.rail_indent`
    /// in, propped against `settings.rail_guide_*` — while the section
    /// stands UNFOLDED ([`Settings::rail_open`]). Which it is standing
    /// on the page it belongs to has nothing to do with, since
    /// 2026-08-18: the fold is a state of its own that a press turns
    /// over, not the view read a second way.
    ///
    /// THE PAGES ARE A FIELD OF THE ROW AND NOT A SECOND TABLE. A
    /// lookup keyed by the act (which is what the second column was)
    /// can disagree with the row that owns it — a triangle on an entry
    /// with nothing behind it, or pages under an entry that does not
    /// draw one — and neither half can notice. Here the arrow, the
    /// indent, the guide, the hit map and the focus chain are all read
    /// off the same `kids`, so "this entry has pages" is one fact.
    ///
    /// WHICH MEANS THE ARROW IS ALWAYS A PROMISE (the owner's mock-up,
    /// §3: an arrow on every entry would be half the entries lying).
    /// A section that IS its page stays a plain [`Ctrl::Button`] and
    /// cannot grow one: there is no field to put it in.
    Expander { label: Text, kind: BtnKind, act: Act, kids: &'static [Row] },
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
    /// The colour picker (`nacelle::object::color_picker`), taking the
    /// whole band: a field of hue by saturation, a value bar, the
    /// chosen colour as a patch, the value written out in one of six
    /// notations, and two grids of ready-made colours.
    ///
    /// NO LABEL COLUMN AND NO VALUE GUTTER, unlike every other control
    /// here, and the reason is the control's own shape: the label
    /// column exists so a word and a track do not collide on one line,
    /// and this is not a line — it is a block as tall as ten rows. The
    /// word above it is a `Ctrl::Section`, which is what a heading over
    /// a block is everywhere else in this window.
    ///
    /// It takes no `get`/`set` pair either, and that is deliberate
    /// rather than lazy: the picker is a MODEL and not a number
    /// ([`color_picker::Picker`] keeps the hue a drag onto the grey
    /// axis would otherwise lose), so the window owns one per control
    /// and the row says WHICH — and not a closure returning a colour,
    /// or the hue would be lost on the way through. The slot is the one
    /// this variant's own comment promised when a second picker joined,
    /// and on 2026-08-18 thirteen of them did (owner, ZGŁOSZENIE 4: a
    /// picker everywhere a colour is chosen, ADVANCED included).
    Picker(PickerId),
}

impl Ctrl {
    /// The word this row writes IN THE LABEL COLUMN, if it writes one
    /// there at all — the THREE kinds [`Settings::row_label`] serves, and
    /// exactly those three.
    ///
    /// A checkbox's word is inside its own band and a button's is inside
    /// its plate, so neither of them stands in the column and neither may
    /// widen it: a column measured against SNAP TO GRID would push three
    /// tracks aside for a word that is nowhere near them.
    ///
    /// A kind that falls out of this list keeps drawing its word at the
    /// content's left edge and loses the column that was holding the
    /// control off it, which is the plate-on-its-own-label fault. Nothing
    /// that reads this list can notice that, so the test that watches for
    /// it matches on the KIND instead
    /// (`a_row_does_not_write_its_label_its_control_and_its_value_over_one_another`).
    fn column_label(&self) -> Option<&'static str> {
        match self {
            Ctrl::Slider { label, .. }
            | Ctrl::Chips { label, .. }
            | Ctrl::Cycle { label, .. } => Some(label),
            _ => None,
        }
    }

    /// The WIDEST this row's value can ever be written, as the string to
    /// measure — not the value it happens to carry.
    ///
    /// A column measured against the number on screen would move while
    /// the hand drags the track it stands beside, so the reserve is taken
    /// against the range's own top: `hi` has at least as many digits as
    /// anything the track can reach, `Unit` says what stands after them,
    /// and [`widest_run`] answers for the digits themselves.
    ///
    /// Only a slider writes here. A cycler's value is inside its plate
    /// and a segment's inside its segment, exactly as their labels are.
    fn column_value(&self) -> Option<String> {
        match self {
            Ctrl::Slider { unit, range: (_, hi), .. } => Some(unit.text(*hi)),
            _ => None,
        }
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
// per press; cell counts, pixels and every 0..100 track step 1. Exactly
// the numbers the descriptions carried as literals before the field
// became a question — one function each, the way [`always`] is the
// constant answer to `Row::when`. `step_2` (the blur pyramid's own step)
// left with BLUR DEPTH in 2026-08-23's picker-only simplification.
fn step_1(_: &Settings) -> u32 {
    1
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

/// Whether a wallpaper is chosen this session. The condition on BASIC's
/// CLEAR WALLPAPER row — a button that undoes a choice has nothing to do
/// while there is no choice standing (`Row::when`, the same absent-not-
/// greyed rule [`corner_sized`]'s own header cites).
fn wallpaper_chosen(s: &Settings) -> bool {
    s.backdrop_image.is_some()
}

/// The word BASIC's WALLPAPER row shows on its button: the chosen file's
/// own name where one is picked (the path in full would run the row off
/// the window's edge on anything but the widest screens), or the plain
/// invitation where none is.
fn wallpaper_label(s: &Settings) -> String {
    match s.backdrop_image.as_deref() {
        Some(path) => {
            let file = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string());
            format!("WALLPAPER: {file}")
        }
        None => "CHOOSE WALLPAPER\u{2026}".to_string(),
    }
}

/// Whether the corner cut standing is one that has a SIZE. The condition
/// on BASIC's CORNER SIZE row.
///
/// `SQUARE` is the one cut with nothing to measure: a square corner is the
/// absence of a corner treatment, and every radius the theme states is
/// simply not taken. ROUND and CHAMFER both consume all three radii
/// (`corner.sm/md/lg`), so both wear the control.
///
/// A list NEVER TOUCHED (`None`) shows the row: the seeding puts the
/// theme's own `corner.mode` in this field on the way in, so `None` here
/// means a page that has not been seeded at all, and hiding a control on
/// that ground would be hiding it because we do not know rather than
/// because there is nothing to ask.
fn corner_sized(s: &Settings) -> bool {
    !matches!(s.current_corner.as_deref(), Some("SQUARE"))
}

/// The word BASIC's CORNER SIZE row is showing, DERIVED from the radii
/// rather than remembered beside them.
///
/// THE SCALE, AND WHY IT IS A SCALE. `[corner]` is a NAMED ladder —
/// `none`, `sm`, `md`, `lg`, `pill` — and 41 `@corner.*` references
/// through the master reach it BY NAME. A free number here would have
/// answered the owner's question ("wielkość zaokrąglenia") and drifted
/// away from those names the moment it was moved: `corner.sm` would be a
/// key whose name said "small" and whose value was whatever a drag left.
/// So the control names a STEP and the value is the theme's own number
/// for that step. Nothing about a look is decided in this file — the
/// three numbers come from [`Settings::corner_seed`], which is read off
/// the theme when the editor opens.
///
/// WHAT ONE STEP MEANS: the whole interface wears it. BASIC's rule is
/// "one move to the whole thing" (`.gap-program/audyt-basic.md` §3), and
/// the three-way split between a badge, a panel and a modal is exactly
/// the "what should this one token do" question ADVANCED answers with its
/// CORNER SM / MD / LG. Choosing SMALL here says every corner on the
/// screen is the theme's own small; the ladder is still there, on the
/// other page, untouched.
///
/// `pill` is deliberately NOT a step of this control. It is a SENTINEL
/// word, not a length — it bakes to -2.0, "half the shorter side" — so a
/// track that carries whole units cannot express it and a step that
/// pretended to would be writing a number where the file wants a word.
/// The same reason [`nacelle::theme::edit::shape_edits`] leaves it alone.
///
/// AS WRITTEN is first, and it is the answer whenever the three radii
/// stand exactly where the theme put them — including the case where a
/// theme's own ladder is flat, which is why it is tested before the three.
fn corner_step_word(s: &Settings) -> String {
    if [s.corner_sm, s.corner_md, s.corner_lg] == s.corner_seed {
        return CORNER_STEPS[0].to_string();
    }
    for (i, seed) in s.corner_seed.iter().enumerate() {
        if [s.corner_sm, s.corner_md, s.corner_lg] == [*seed; 3] {
            return CORNER_STEPS[i + 1].to_string();
        }
    }
    // ADVANCED's three tracks can put the radii anywhere; the word says so
    // rather than naming a step the radii are not on.
    "CUSTOM".to_string()
}

/// The words [`corner_step_word`] cycles through. Index 0 is the theme's
/// own ladder; 1..=3 are its three named steps, in the master's order.
const CORNER_STEPS: [&str; 4] = ["AS WRITTEN", "SMALL", "MEDIUM", "LARGE"];

// The whole-theme sections' conditions used to live here — one small
// function per question, so the rows and `editor_edits` asked the SAME
// one. `bg_blurs`, `bg_frosted`, `bg_solid_or_frosted`, `surface_own`,
// `severity_chosen`, `corner_chosen`, `ring_dressed`, `ring_dashed`,
// `ring_haloed`, `bar_chosen`, `bar_fades` and `bar_tracked` all guarded
// rows that stood in `EDITOR_ROWS` before 2026-08-23's picker-only
// simplification removed every row that was not a `Ctrl::Picker` — with
// the dropdowns and toggles that fed these conditions gone too, none of
// them could ever answer true again, so they went with the rows they
// guarded rather than stand here unreachable. `editor_edits` still writes
// every token the removed rows used to (a set's write does not depend on
// this page ever having offered a knob for it), so nothing a saved theme
// carries changed — only what this page's controls can reach did.

/// A row that exists only while `when` holds — see `Row::when`.
const fn row_shown(ctrl: Ctrl, when: fn(&Settings) -> bool) -> Row {
    Row { ctrl, after: Gap::Row, enabled: always, when }
}

/// …and the same with a break under it. Both halves of a row's own
/// description at once, for a conditional control that ends a group:
/// the COLOR page's HDR switch closes the controls and opens the two
/// lines that report on them.
const fn row_shown_after(ctrl: Ctrl, when: fn(&Settings) -> bool, after: Gap) -> Row {
    Row { ctrl, after, enabled: always, when }
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
    Flow { when: fn(&Settings) -> bool, rows: &'static [Row] },
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
    Pinned { rows: &'static [Row] },
}

/// One column of a columned band, with its OWN label/value measurement:
/// the sliders on the left do not inherit the width of the labels on
/// the right ("the widest label IN THE BLOCK", `rhythm.label_col`).
#[derive(Clone, Copy)]
struct ZCol {
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
/// One reader for the bands and one for the window's own two panels:
/// the page and the columns inside it fold on the same word, which is
/// why "there is no room" means one thing in this window and not two.
///
/// Measured 2026-08-17 and RE-MEASURED 2026-08-18, and the THEME's to
/// answer, not this file's: at the master's `72u` the threshold scales
/// with the screen, so whether a band stands in columns is very nearly
/// the question of how much of the content box its page HAS.
///
/// THE SECOND NAVIGATION COLUMN IS GONE and every page is now the wide
/// case. A page used to keep a little over half the content box while
/// two columns of navigation stood beside it; beside the rail alone it
/// keeps about three quarters — 1 078 px of 1 410 at 1080p, where the
/// rail takes 310 — so a band inside it stands in columns from 1080 px
/// up. The paragraph this replaces drew its conclusion about FONT from
/// the narrow case and that case no longer exists.
///
/// AND THE WINDOW'S OWN FOLD IS THE SAME NUMBER, which is why one
/// reader serves both: with one column instead of two the master keeps
/// its two panels at every height the program is built for, so the
/// folded window is a shape for a genuinely narrow one — or for a theme
/// that asks a wider page than the screen can give. Moving the number
/// is the owner's call and a `libnacelle` commit; nothing here may
/// hard-code around it.
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
fn zone_regions(zone: &'static Zone, box_: Rect) -> Vec<(Rect, &'static [Row])> {
    match zone {
        Zone::Flow { rows, .. } | Zone::Pinned { rows } => vec![(box_, *rows)],
        Zone::Cols { columns } => {
            if zone_folded(zone, box_) {
                return columns.iter().map(|c| (box_, c.rows)).collect();
            }
            let gap = col_gap();
            let n = columns.len().max(1) as f32;
            let w = ((box_.w - gap * (n - 1.0)) / n).max(0.0);
            columns
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let x = box_.x + i as f32 * (w + gap);
                    (Rect::new(x, box_.y, w, box_.h), c.rows)
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
static RAIL_ROWS: [Row; 10] = [
    row_after(Ctrl::Section { title: "APPEARANCE" }, Gap::None),
    // The one section with pages of its own, and therefore the one
    // entry that carries a triangle ([`Ctrl::Expander`]).
    row(Ctrl::Expander {
        label: Text::Fixed("LOOK AND FEEL"),
        kind: BtnKind::Wide,
        act: Act::OpenLookFeel,
        kids: &LOOKFEEL_PAGES,
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
    // …and WHY it is shut, on the machines where it is. R6 paints an
    // unofferable section grey and takes it out of the focus chain, and
    // that is right — but grey alone says "not now" and the truth is
    // "not here, and nothing you press will change it". One short line
    // under the inscription, and only there: a rail carrying it on every
    // machine would be a permanent apology for a feature that works.
    row_shown(
        Ctrl::Note { text: Text::Fixed("NO COLOR MANAGER") },
        |s| !s.color_enabled,
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

/// The pages of LOOK AND FEEL, in reading order — the rows that unfold
/// UNDER it on the rail ([`Ctrl::Expander`]).
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
static LOOKFEEL_PAGES: [Row; 3] = [
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

/// The navigation as a BAND, for the folded window: the same table,
/// laid down the one list instead of beside the page. A static because
/// a band is `&'static` everywhere else in this file.
///
/// ONE TABLE AND NOT TWO SINCE THE COLUMNS BECAME ONE. A section's
/// pages are rows of this table now ([`Ctrl::Expander`]), so the folded
/// window inherits the unfolding for nothing — the same entry, the same
/// triangle, the same indent, one walker.
static RAIL_ZONE: Zone = Zone::Flow { when: always, rows: &RAIL_ROWS };

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

/// The entry UNDER a section that stands for this view, where the
/// section unfolds one for it. The two pages [`LOOKFEEL_PAGES`] does
/// not list answer `None`, and nothing under the section is marked
/// while they stand — which is true: neither of them is one of its
/// entries.
fn kid_act(view: View) -> Option<Act> {
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
/// ([`LOOKFEEL_PAGES`]). They are the same two pages, reached in
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

// The BASIC/ADVANCED switch that stood here (a Ctrl::Cycle at the head of
// the page) is gone: BASIC is the only page now, and its ADVANCED COLOUR
// button opens the per-element grid in place. `Act::EditorMode` — the
// state the switch toggled — is what the button and the grid's back button
// press, so the fold logic behind it is untouched.

/// THE WHOLE THEME ON ONE COLOUR — the editor's BASIC page.
///
/// FOUR QUESTIONS, and every one of them about the whole desktop: what
/// colour is it, what are its borders, what are its beds, what shape are
/// its corners. That is the page's rule, written down in
/// `.gap-program/audyt-basic.md` on 2026-08-18 and applied here: BASIC
/// answers "how should this desktop look", one move to the whole thing;
/// ADVANCED answers "what exactly should this one token do".
///
/// THE MOVE IS STILL RELATIVE. Until 2026-08-18 the colour was three
/// sliders — a hue rotation, a chroma multiplier and a lightness offset
/// — and the owner replaced them with a picker, having seen one and
/// asked for "coś podobnego, dopasowane do projektu". What changed is
/// how a person SAYS how far to move: they point at the colour they want
/// instead of hunting for it by turning what is there. What did NOT
/// change is the arithmetic behind it ([`Settings::set_tone_from_picker`]
/// turns the picked colour into the same [`Tone`] the sliders wrote), so
/// every sentence the old page's head made is still true of this one:
///
/// The model turns the move into edits to the AUTHORS everything else is
/// derived from (`theme::edit::tone_edits` — `palette.accent`, the
/// palette's three grounds, `surface.lift` and `text.lift`), and the
/// cascade does what it already does. Being RELATIVE is what leaves
/// every difference the theme's author wrote exactly where it was: it
/// keeps a theme from flattening into one colour, and it makes ONE HUE
/// FOR THE INTERFACE and A ROTATION FOR SEVERITY the same mechanism
/// instead of two — the chrome family has one author, so the move lands
/// surfaces, containers, controls and text on a single shared hue, while
/// the severity family has seven, so the same turn carries all of them
/// and green `ok` stays as far from red `critical` as it was. What tells
/// the families apart afterwards is SHADE, and the shades are the
/// master's own ladders, which this page never touches — the owner's
/// ŻYCZENIE 2b, and it needed no second mechanism.
///
/// The move still lands on whole notches of what the pipeline can SHOW
/// ([`Settings::tone_step`]): the picker is continuous and the tone it
/// writes is not, which is the same statement the three tracks made with
/// their `step` and is now made once, where the colour crosses over.
///
/// AND THREE KINDS UNDER THEM (owner, 2026-08-17). A kind is a choice
/// between shapes, not a number to nudge, so asking it costs the page
/// one row and no arithmetic — which is why the page that asks the
/// fewest questions can afford all three. They are the SAME control over
/// the SAME list as ADVANCED's, writing the same field: `editor_edits`
/// reads `current_border`, `current_background` and `current_corner`
/// whichever band drew them, so a kind chosen here shows in the preview
/// and lands in the file with nothing added to the builder.
///
/// WHAT STILL DOES NOT FOLLOW THEM HERE are the kinds' COLOUR knobs. The
/// owner asked for the KIND of the background and not for its colour, and
/// BASIC has a second reason besides: its move already carries the window
/// body's bed, which the theme writes as an absolute colour
/// ([`Settings::editor_edits`], `carry`) — a TINT or WASH picker on this
/// page would land that shift on top of itself. The four numbers below
/// are not colours: three are lengths and one is an alpha, and none of
/// them is carried by the tone move.
///
/// AND FOUR NUMBERS UNDER THE KINDS (owner, ZGŁOSZENIE 6 and 7,
/// 2026-08-18). A kind alone was half an answer and the owner said so:
/// picking ROUND on a theme whose radii are near zero changes nothing
/// visible, and picking GLOW gave one reach with no say in it. Each of
/// the four appears WITH the thing it belongs to and is ABSENT otherwise
/// — `Row::when` and not a greyed-out row, which is what the owner asked
/// for in as many words ("nie wyszarzonej, tylko nieobecnej"):
///
/// * EFFECT SIZE (GLOW REACH until 2026-08-25) — with GLOW and NEON
///   ([`border_lit`]).
/// * OPACITY — with a background kind chosen ([`bg_chosen`]). The SAME
///   control ADVANCED wears, writing the same field, exactly as the three
///   lists are the same control on both pages. What differs is HOW FAR IT
///   REACHES, which is ZGŁOSZENIE 7 and is answered in
///   [`Settings::editor_edits`], not here.
/// * CORNER SIZE — with a cut that has a size ([`corner_sized`]).
///
/// BORDER SIZE is the one row here NOT under a `Row::when`: every kind
/// draws a line, lit or not, so its condition is always true and it
/// stands unconditionally, the way it did before it left on 2026-08-23
/// and the way it stands again since 2026-08-25 — under EFFECT SIZE now
/// rather than above every kind-specific row, `Knob::EdgeWidth` reusing
/// the `edge_width`/`edge_width_touched` fields and the `border_width_
/// edit` answer in [`Settings::editor_edits`] that never left with it.
static EDITOR_BASIC_ROWS: [Row; 13] = [
    row_after(Ctrl::Section { title: "THEME COLOUR" }, Gap::None),
    row(Ctrl::Picker(PickerId::Tone)),
    row_after(Ctrl::Section { title: "BORDER" }, Gap::None),
    // BORDER SIZE, back on this page (2026-08-25, the owner's word — it
    // stood here unconditionally until 2026-08-23, when it left; see
    // `Knob::EdgeWidth`'s own doc for the plumbing that never left with
    // it). Unconditional again, under EFFECT SIZE rather than above it:
    // every kind draws a line, lit or not, so there is no `Row::when` to
    // gate it on. `border.edge.width`, NOT the global `stroke.hair` kerf
    // 72 other derivations share.
    row(Ctrl::Slider {
        label: "BORDER SIZE",
        act: Act::EditorTrack(Knob::EdgeWidth),
        unit: Unit::None,
        // 0..100 over 0u..1u — the wall `the_borders_thickness_and_
        // its_lights_reach_are_two_answers...` already pins: past the
        // master's own heaviest stroke, `[stroke] bold = 0.7u`.
        range: (0, 100),
        step: step_1,
        get: |s| s.edge_width,
        set: |s, v| {
            s.edge_width = v;
            s.edge_width_touched = true;
        },
        save: |s| s.apply_editor_preview(),
    }),
    row_after(Ctrl::Section { title: "BACKGROUND" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Backgrounds }),
    // OPACITY is BACK on this page (2026-08-19, owner's second word: the
    // Tone picker reads RGB now, no alpha of its own to answer for
    // transparency with — this slider is the one control that does, on
    // both pages alike, and `editor_edits` reads it here exactly as
    // ADVANCED does (`theme::edit::panel_fill_edit`'s alpha argument).
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
    row_after(Ctrl::Section { title: "SHAPE" }, Gap::None),
    row(Ctrl::Drop { list: ListId::Corners }),
    row_shown(
        Ctrl::Cycle {
            label: "CORNER SIZE",
            get: corner_step_word,
            act: Act::EditorCornerStep,
        },
        corner_sized,
    ),
    // WALLPAPER (2026-08-23): `[backdrop]`, the plate BEHIND the board —
    // an axis of its own and not a fourth BACKGROUND kind, per the
    // section's own header above: `backdrop.source` names where the
    // pixels behind EVERYTHING come from, and SOLID (the theme's own
    // `backdrop.solid`) is the only alternative this row offers to IMAGE.
    // `fit`, `treat.*` and the `plate`/`passthrough` sources are theme-
    // file business, same as every token this window does not carry a
    // control for (`theme/edit.rs`'s own rule, "only what the renderer
    // actually draws" — narrower still here: only what a FILE PICKER can
    // hand a person).
    //
    // ONE BUTTON OPENS THE PATH PROMPT, the same `NamingFor::WallpaperPath`
    // shape SAVE AS already wears for a theme's name — there is no file-
    // browse dialog anywhere in this program to reuse (checked: the
    // layout and theme SAVE AS prompts are both a typed name, never a
    // native picker), so a typed/pasted path is the established pattern
    // and not a bespoke one. CLEAR is a second row and not a second
    // press of the first, so a chosen path stays readable while a hand
    // decides whether to keep it.
    row_after(Ctrl::Section { title: "WALLPAPER" }, Gap::None),
    row(Ctrl::Button {
        label: Text::Of(wallpaper_label),
        kind: BtnKind::Wide,
        act: Act::EditorWallpaperEdit,
    }),
    row_shown(
        Ctrl::Button {
            label: Text::Fixed("CLEAR WALLPAPER"),
            kind: BtnKind::Wide,
            act: Act::EditorWallpaperClear,
        },
        wallpaper_chosen,
    ),
    // The door to per-element colour used to stand here as its own row
    // (`Ctrl::Button { label: "ADVANCED COLOUR", ... }`, `Act::EditorMode`
    // — the state the deleted BASIC/ADVANCED switch used to toggle). Moved
    // 2026-08-23 into the Tone picker's own row, at the end of its
    // notation strip — drawn in the `Ctrl::Picker` arm, not listed here.
];

/// The editor's first section. The border is one kind and one colour, and
/// the colour is three numbers because the theme writes colours as
/// `oklch(L, C, H)` — three numbers is the shape of the value, not a
/// choice about how many controls to offer.
///
/// Neither lit kind's light has a colour of its own; both wear the line's.
/// So there is one colour here and not two, and the list above it switches
/// the light on and shapes it rather than introducing a second thing to
/// tint — GLOW spills the border's colour, NEON drives its core white and
/// lets the colour live in the band just outside it, and the colour they
/// are both made of is the one this picker sets.
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
/// Since 2026-08-23, at the owner's word: everything here that is not a
/// colour picker is gone. The eighty-plus dropdowns, sliders and toggles
/// this page used to carry — border kind, background kind and its glass
/// knobs, the surface ladder's hue/lift/chroma, corner radii, the focus
/// ring's dash and width, severity's role picker — are removed, not
/// hidden; a `Row::when` guard tied to one of them would only ever answer
/// false now, which is a picker nobody can reach, not a simplification.
/// Every picker below is therefore unconditional (`row`, never
/// `row_shown`) even where its guard used to gate it on a choice this
/// page no longer offers a way to make.
static EDITOR_ROWS: [Row; 12] = [
    // Back to BASIC — the only navigation this page keeps; a page with
    // color pickers and no way off it is not a simplification either.
    row(Ctrl::Button {
        label: Text::Fixed("‹ BASIC"),
        kind: BtnKind::Wide,
        act: Act::EditorMode,
    }),
    row(Ctrl::Picker(PickerId::Edge)),
    row(Ctrl::Picker(PickerId::Tint)),
    row(Ctrl::Picker(PickerId::Wash)),
    row(Ctrl::Picker(PickerId::Accent)),
    row(Ctrl::Picker(PickerId::BgMain)),
    row(Ctrl::Picker(PickerId::Text)),
    row(Ctrl::Picker(PickerId::Severity)),
    row(Ctrl::Picker(PickerId::Ring)),
    row(Ctrl::Picker(PickerId::MenuFill)),
    row(Ctrl::Picker(PickerId::TipFill)),
    row(Ctrl::Picker(PickerId::BarTrack)),
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

/// What the swapchain is asked for: its depth, the space it is asked in
/// and the range that space is of. The three are one question and stand
/// together (§2 of the screen decision — DEPTH and SPACE are not to be
/// separated), which is why they are one column and not three rows of a
/// wider one. The range is last because it is a statement ABOUT the list
/// above it, and because turning it changes both of the other two.
static COLOR_SWAPCHAIN_ROWS: [Row; 5] = [
    row(Ctrl::Chips {
        label: "DEPTH",
        values: depth_values,
        get: |s| s.color_depth,
        act: Act::ColorDepth,
    }),
    // ONE list, whatever the switch under it says. Its word and its
    // members both come from the window's state, so SPACE and SPACE HDR
    // are two readings of this row and never two rows.
    row(Ctrl::Drop { list: ListId::Spaces }),
    // And the switch itself, UNDER the list it turns, so the eye reads
    // "these spaces, and the range they are for" in that order.
    // `row_shown` and not `row_when`: a compositor that cannot be asked
    // for a single high-range space is a machine with no HDR on it, and
    // the screen decision forbids a grey ghost offered "just in case".
    row_shown_after(
        Ctrl::Toggle { label: "HDR", get: |s| s.color_hdr, act: Act::ColorHdr },
        hdr_possible,
        Gap::Section,
    ),
    // WHAT CAME OF IT. Every control above this pair can be turned
    // without the picture moving — a compositor may refuse a space, or
    // never answer for one, and a surface may have no format above eight
    // bits — and until these two lines existed the program said so on
    // stderr and the window said nothing at all. That is the difference
    // between a setting and a control that pretends: these rows are the
    // page's answer, not its question.
    row_shown(Ctrl::Note { text: Text::Of(color_space_note) }, color_was_answered),
    row_shown(Ctrl::Note { text: Text::Of(color_depth_note) }, color_depth_fell_short),
];

/// Whether the application has told this window anything about the last
/// request. `Row::when` and not an empty string inside the note, because
/// an empty note is still a row: it would reserve its height and open a
/// hole under the switch on every page that has nothing to report.
fn color_was_answered(s: &Settings) -> bool {
    !s.color_status.is_empty()
}

/// What the compositor did with the space that was last asked for.
fn color_space_note(s: &Settings) -> String {
    format!("space: {}", s.color_status)
}

/// Whether the swapchain gave LESS than the page asked for. Zero is "not
/// measured" — no legal depth is zero — so a window the application has
/// not told yet says nothing rather than claiming a shortfall of eight
/// bits it has not seen.
///
/// LESS AND NOT MERELY DIFFERENT, and the difference is a lie this line
/// told until it was caught. Twelve has no swapchain format of its own
/// and rides the sixteen-bit float one on purpose (`nacelle-renderer`,
/// `pick_format`: `16 | 12` share a tier) — so asking for twelve and
/// being given sixteen is the arrangement working, not failing, and a
/// page that read "different" would tell a user picking the one depth in
/// four that behaves this way that "the surface offers no more" than the
/// MORE it just got. A number above the wish is never a shortfall: the
/// wish is a floor on precision, and a floor cleared is nothing to
/// report.
fn color_depth_fell_short(s: &Settings) -> bool {
    s.color_depth_now != 0 && s.color_depth_now < s.color_depth_asked
}

/// Asked against given. Only ever drawn where the swapchain came up
/// short ([`color_depth_fell_short`]) — a line repeating the number
/// already standing in the DEPTH chips would be noise, and a line about
/// a swapchain that gave MORE than the wish would be a complaint about
/// good news.
fn color_depth_note(s: &Settings) -> String {
    format!(
        "depth: {} bits asked, {} in the swapchain — the surface offers no more",
        s.color_depth_asked, s.color_depth_now
    )
}

/// The depths the swapchain may be asked for, and the whole of that
/// question ([`Ctrl::Chips`]).
///
/// Eight bits is missing from the HDR offer. PQ spends its code points
/// on a range eight bits does not have, so eight-bit HDR bands visibly
/// — and this page has no way to say so: it carries no warning control,
/// only a fixed note about where the LUT and ICC files live. The owner's
/// rule settles it (`decyzja-ustawienia-ekranu.md`): what cannot give a
/// picture is not on the screen.
///
/// The floor itself is the MODEL's (`SpaceRange::depth_floor`), not this
/// page's. The configuration is read through the same statement, so a
/// depth the page will not offer is also a depth the swapchain will not
/// be asked for, however the file arrived at it.
fn depth_values(s: &Settings) -> &'static [u32] {
    config::color_depths(s.color_hdr)
}

/// Whether this machine can be asked for high dynamic range at all: at
/// least one space of the table's HDR half survived the compositor's
/// report of what it offers. When none did, the switch IS NOT THERE.
fn hdr_possible(s: &Settings) -> bool {
    config::COLOR_SPACE_TABLE
        .iter()
        .any(|&(n, r)| r == config::SpaceRange::Hdr && s.space_offered(n))
}

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
    Zone::Flow { when: always, rows: &LOOKFEEL_ROWS },
    Zone::Pinned { rows: &LOOKFEEL_FOOTER },
];

static LOOKFEEL_RESET_ZONES: [Zone; 1] =
    [Zone::Flow { when: always, rows: &LOOKFEEL_RESET_ROWS }];

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
static EDITOR_ZONES: [Zone; 3] = [
    Zone::Flow { when: editor_basic, rows: &EDITOR_BASIC_ROWS },
    Zone::Flow { when: editor_advanced, rows: &EDITOR_ROWS },
    Zone::Pinned { rows: &EDITOR_BAR },
];

/// Two symmetrical columns, each measuring its OWN rows: the two tracks
/// are the same length because the two columns carry the same word
/// (SIZE) and not because one inherited the other's label width.
static FONT_COLUMNS: [ZCol; 2] =
    [ZCol { rows: &FONT_TERM_ROWS }, ZCol { rows: &FONT_UI_ROWS }];

static FONT_ZONES: [Zone; 1] = [Zone::Cols { columns: &FONT_COLUMNS }];

static GRID_ZONES: [Zone; 1] = [Zone::Flow { when: always, rows: &GRID_ROWS }];

static SOUND_ZONES: [Zone; 1] = [Zone::Flow { when: always, rows: &SOUND_ROWS }];

static BOARDS_ZONES: [Zone; 2] = [
    Zone::Flow { when: always, rows: &BOARDS_ROWS },
    Zone::Pinned { rows: &BOARDS_HINT },
];

static COLOR_COLUMNS: [ZCol; 2] =
    [ZCol { rows: &COLOR_SWAPCHAIN_ROWS }, ZCol { rows: &COLOR_FILE_ROWS }];

static COLOR_ZONES: [Zone; 1] = [Zone::Cols { columns: &COLOR_COLUMNS }];

static BLUR_ZONES: [Zone; 1] = [Zone::Flow { when: always, rows: &BLUR_ROWS }];

static ADDONS_ZONES: [Zone; 1] = [Zone::Flow { when: always, rows: &ADDONS_ROWS }];

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
///
/// A FIELD and not a question, and the difference is the whole of the
/// 2026-08-18 `strace` finding. `Text::Of` is evaluated on the row it
/// belongs to, every frame the page is drawn; the answer used to come
/// from [`config::active_sounds_dir`], which walks the configuration
/// cascade and then the asset roots. With this page open that was 1121
/// bytes of RON parsed twice, eight paths knocked on and two directory
/// stats — forty-six times a second, for a sentence that changes when
/// the user chooses a different sound set and at no other moment.
///
/// So it is worked out where every other answer on a page is worked
/// out: on the way in ([`Settings::refresh_sound_set`]). What that
/// trades away is noticing a set INSTALLED while the page stands open,
/// which is the same staleness the three lists on LOOK AND FEEL have
/// always had and for the same reason.
fn sound_set_note(s: &Settings) -> String {
    s.sound_set.clone()
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
    /// The width of the chrome button — BACK or CLOSE — WHERE THE
    /// WINDOW HAS FOLDED, from `settings.back_w_frac` and its two
    /// floors. Unfolded, that button is the head of the RAIL and takes
    /// the rail's own room instead ([`Panes`]): a button one width and
    /// the bed under it another is the "amatorka" the owner reported on
    /// 2026-08-18.
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

    /// The same metrics with the RAIL's own vertical rhythm
    /// (`settings.rail_row_gap`) in place of the form's
    /// (`modal.row_gap`).
    ///
    /// TWO RHYTHMS BECAUSE THERE ARE TWO QUESTIONS. A page's break is
    /// how far apart two CONTROLS have to stand to be read as two
    /// things you operate; a rail's is how far apart two NAMES have to
    /// stand to be read as two places you can go — and a name is the
    /// smaller claim. It stopped being one question the day a section's
    /// pages moved into the rail: the column now has to hold every
    /// section AND the open one's pages, so the rhythm it can afford is
    /// not the rhythm a page can afford. At the FORM's break the
    /// unfolded rail outgrows its column even at 1080p — the rail
    /// scrolls ([`Settings::rail_scroll`]) rather than folding the
    /// window for it, so what this number really buys is how much of
    /// the navigation a screen shows without the reader touching the
    /// wheel.
    ///
    /// ASKED BY THE DRAWING AND BY THE MEASUREMENT, from the one place,
    /// so the rail cannot be laid at one rhythm and measured at another.
    /// FOLDED there is no rail: its entries are rows of the page's one
    /// list and take that list's rhythm, which is what being part of
    /// the list means.
    fn rail(self) -> Metrics {
        static RAIL_GAP: OnceLock<TokenId> = OnceLock::new();
        Metrics {
            gap: theme::resolved().px(tok(&RAIL_GAP, "settings.rail_row_gap")).max(0.0),
            ..self
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

/// Where a page's body starts: under the chrome's own row, and under
/// the row WHEREVER IT STANDS.
///
/// ASKED OF THE BUTTON AND NOT OF THE BOX. This used to read
/// `content.y + button.h + lead`, which is the same sentence only while
/// the chrome button starts at the top edge of the content box. Since
/// 2026-08-18 it does not: unfolded it is the head of the RAIL and
/// stands `settings.band_pad_y` down from the top of the rail's bed
/// ([`Panes::of`]), so a body still measured from the box would leave
/// the page's first row `lead - band_pad_y` under the button instead of
/// `lead` — 5.4 px of the master's 16.2 at 1080p — and would put it a
/// whole `band_pad_y` ABOVE the rail's first entry, which the two used
/// to share a line with. Both faults come from asking the wrong
/// rectangle, so this asks the rectangle the button was really given.
///
/// Folded there is no bed and no air, `Panes::of` puts the corner back
/// at the head of the content box, and this is the old sentence again.
fn body_top(page: &Page, m: Metrics, nav: &Panes) -> f32 {
    nav.corner.bottom() + m.space(page.lead)
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

/// The navigation column's width: a fraction of the content box, never
/// under the theme's own minimum and never under its device-px floor —
/// the three-part rule every width in this window is written with.
///
/// ONE READER SINCE THERE IS ONE COLUMN. `settings.subrail_w_frac` sized
/// the column of a section's pages, and that column is gone: the pages
/// stand UNDER their section now, inside this width, one
/// `settings.rail_indent` in.
fn nav_w(content: Rect) -> f32 {
    static FRAC: OnceLock<TokenId> = OnceLock::new();
    static MIN: OnceLock<TokenId> = OnceLock::new();
    static MIN_PX: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    (content.w * th.px(tok(&FRAC, "settings.rail_w_frac")))
        .max(th.px(tok(&MIN, "settings.rail_w_min")))
        .max(th.px(tok(&MIN_PX, "settings.rail_w_min_min_px")))
}

/// How far a section's own pages stand in from the section they belong
/// to (`settings.rail_indent`), and the hairline they are propped
/// against: its width, and where across that step it stands.
///
/// THE STEP IS THE WHOLE OF WHAT THE SECOND COLUMN USED TO SAY by
/// standing somewhere else, so it is the theme that says it. `x` is a
/// fraction of the step and not a length, because what it answers is
/// "which end of the gutter" — 0 flush with the section's own edge, 1
/// flush against its pages' — and the master's answer (the middle) is
/// a look decision that would otherwise be a `0.5` in Rust.
fn rail_indent() -> f32 {
    static INDENT: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().px(tok(&INDENT, "settings.rail_indent")).max(0.0)
}

/// The box a section's own pages are laid in: the section's own, one
/// `settings.rail_indent` narrower and that much further in.
///
/// Answered HERE and not at each of the two places that ask — the
/// walker that draws the run and the one that measures it — because a
/// run measured in one box and drawn in another is a run that is one
/// height for the scroll and another for the eye.
fn indent_region(region: Rect) -> Rect {
    let step = rail_indent();
    Rect::new(region.x + step, region.y, (region.w - step).max(0.0), region.h)
}

/// The guide's stroke and its place in the indent step, as the rect it
/// occupies beside a run of indented rows standing in `region`.
///
/// The line's LEFT EDGE runs from the section's own edge to its pages'
/// as `settings.rail_guide_x` goes 0 to 1, so neither end of the range
/// pushes the stroke out of the step it brackets.
fn rail_guide_x(region: Rect) -> (f32, f32) {
    static W: OnceLock<TokenId> = OnceLock::new();
    static AT: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let w = th.px(tok(&W, "settings.rail_guide_w")).max(0.0);
    let at = th.px(tok(&AT, "settings.rail_guide_x")).clamp(0.0, 1.0);
    (region.x + (rail_indent() - w).max(0.0) * at, w)
}

/// The air the navigation's bed keeps around what stands on it:
/// `settings.band_pad_x` across and `settings.band_pad_y` down.
///
/// TWO NUMBERS AND NOT ONE, because they answer two different questions.
/// The horizontal one competes with `settings.col_gap` — the gutter to
/// the page beside it — and has to stay under it or the two columns fuse;
/// the vertical one competes with `modal.row_gap` between the buttons
/// themselves. A theme that wants them equal says so by giving them one
/// value, which is what the master does.
fn band_pad() -> (f32, f32) {
    static PAD_X: OnceLock<TokenId> = OnceLock::new();
    static PAD_Y: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    (
        th.px(tok(&PAD_X, "settings.band_pad_x")).max(0.0),
        th.px(tok(&PAD_Y, "settings.band_pad_y")).max(0.0),
    )
}

/// The navigation column: the bed that is painted, and the box the rows
/// that stand on it are laid in.
///
/// TWO RECTANGLES BECAUSE THERE ARE TWO QUESTIONS, and the window used
/// to answer both with one. "Where does the column's colour go" is the
/// whole column, top edge to bottom edge; "where do its buttons go" is
/// that box less the bed's own air and less the corner button's row.
/// While one rect answered both, the bed could only start where the
/// buttons started — which is what left `button.h + modal.row_gap`
/// (45.4 + 16.2 = 61.6 px at 1080p) of bare window body above every
/// navigation band and none above the page, the islands the owner
/// photographed on 2026-08-18 — and a button could only ever sit flush
/// against the bed's sides, which is the other half of the same report.
#[derive(Clone, Copy)]
struct Column {
    /// What [`Settings::draw_bands`] paints: the column's whole
    /// rectangle of the content box.
    bed: Rect,
    /// What [`Settings::draw_nav`] clips to and lays rows in: the bed
    /// less `settings.band_pad_*`, and less the corner button's row at
    /// the top.
    rows: Rect,
}

/// Where the window's two panels stand this frame.
///
/// The rows of the navigation hang one ordinary row gap under the
/// corner button — a FIXED lead, not the page's, so the sections do not
/// step up and down as the pages behind them change what they lead
/// with. What is left over is the page.
///
/// THE SPLIT NO LONGER ASKS WHICH VIEW IS IN FORCE, and that is the
/// point of the one column. While a section's pages stood in a column of
/// their own, this had to reserve room for that column on EVERY page
/// whether it was shown or not, or moving between sections would have
/// re-shaped the window under the reader's hand. Pages that unfold under
/// their section take no width at all, so there is nothing left for the
/// view to decide and the parameter is gone.
///
/// THE BEDS FILL THE CONTENT BOX FROM TOP TO BOTTOM. The corner button
/// is the head of the rail and stands ON the rail's bed rather than in a
/// notch cut out of it, so both columns start on one line and end on one
/// line. Nothing here reaches past `content_rect`, which keeps
/// `modal.pad` clear of the frame on the sides and the bottom and drops
/// `modal.body_top` for the title band: the bands fill their AREA, and
/// the window's own margin is still the window's.
///
/// FOLDED: below the threshold there is no column at all — the
/// navigation goes into the flow as a band ahead of the page, and the
/// window is the one vertical list it has always been able to fall back
/// to. There is no bed then and no bed's air either, so the corner
/// button goes back to the head of the content box at its own width
/// (`settings.back_w_frac`).
///
/// THE THRESHOLD IS STILL ONE QUESTION, AND IT IS WIDTH. A first draft
/// of the one-column rail added a second, a HEIGHT: a rail with no
/// scroll of its own that is taller than its box is a section cut off
/// with no way to reach it, and the rail grew taller the day a
/// section's pages moved into it. Folding the whole window at those
/// heights was the wrong answer and a measurable regression — it took
/// the two-column shape away from 720p and 768p, which had stood in
/// columns before. The rail scrolls instead ([`Settings::rail_scroll`]),
/// so the height it WANTS has stopped being a question about the shape
/// of the window at all.
#[derive(Clone, Copy)]
struct Panes {
    rail: Option<Column>,
    /// What the page has: the whole content box when folded.
    page: Rect,
    /// Where the chrome's own button — BACK or CLOSE — stands. Said
    /// here because it is the head of the rail and has to keep the
    /// rail's own air; a second opinion about it in [`Settings::draw`]
    /// is how a button ends up flush with a bed it is supposed to be
    /// lying on.
    corner: Rect,
    folded: bool,
}

impl Panes {
    /// The one vertical list: no column, no bed, the corner button back
    /// at the head of the content box at its own width.
    fn folded(m: Metrics, content: Rect) -> Panes {
        Panes {
            rail: None,
            page: content,
            corner: Rect::new(content.x, content.y, m.corner_w, m.btn_h),
            folded: true,
        }
    }

    /// The split, and the ONE question every walker, measurement and
    /// test asks, so no two of them can answer it differently.
    ///
    /// It takes no `&Settings` and that is worth keeping: the shape of
    /// the window is a function of the room and the theme alone, never
    /// of which section is open. A split that could read the state
    /// could re-shape the window under the reader's hand every time
    /// they changed section, which is the fault the panelled layout has
    /// always been careful to avoid.
    fn of(m: Metrics, content: Rect) -> Panes {
        let gap = col_gap();
        let rail_w = nav_w(content);
        if content.w - rail_w - gap < col_min_w() {
            return Panes::folded(m, content);
        }
        let (pad_x, pad_y) = band_pad();
        // The bed is the whole column; the rows are the bed less its air
        // and less the corner button's row, which the rail carries. The
        // break under that button is the RAIL's ([`Metrics::rail`]) — it
        // is the first of the rail's own breaks and not the page's.
        let rows_top = content.y + pad_y + m.btn_h + m.rail().gap;
        let rows_h = (content.bottom() - pad_y - rows_top).max(0.0);
        let rail = Column {
            bed: Rect::new(content.x, content.y, rail_w, content.h),
            rows: Rect::new(
                content.x + pad_x,
                rows_top,
                (rail_w - 2.0 * pad_x).max(0.0),
                rows_h,
            ),
        };
        let x = rail.bed.right() + gap;
        Panes {
            rail: Some(rail),
            page: Rect::new(x, content.y, (content.right() - x).max(0.0), content.h),
            // The head of the rail, at the width of the ENTRIES under it
            // — not `settings.back_w_frac`, which is the width of a
            // column that no longer exists once the rail does, and not
            // the rail's whole room either. The room includes the lane
            // the rail's own scrollbar stands in ([`rows_box`]), and a
            // button 16 px wider than every button beneath it reads as
            // a button that failed to line up. It costs the lane at
            // every window, scrolling or not, which is the trade the
            // page already makes and for the same reason: a lane that
            // appeared only while scrolling would reflow the column
            // under the reader's hand.
            corner: Rect::new(
                rows_box(rail.rows).x,
                content.y + pad_y,
                rows_box(rail.rows).w,
                m.btn_h,
            ),
            folded: false,
        }
    }
}

/// Where a scrolled view is, when there is more of it than fits. Drawn
/// after what it reports on so it sits over it.
///
/// `scrollbar.auto_hide` is on in the master, so a view at rest shows
/// nothing. A HELD thumb is not at rest: it counts as hover for the
/// width, for the fade and for the class ladder, because a hand that
/// wandered off the lane sideways is still holding the thumb — and a
/// thumb that thinned and faded mid-travel would say it had been let go
/// when it had not.
///
/// TAKES THE VIEW RATHER THAN BEING A METHOD, because this window has
/// two of them since 2026-08-18: the page's flow and the navigation
/// column ([`Settings::rail_scroll`]). A bar that could only ever read
/// one field would have had to be written twice to report on two, and
/// two copies of "what a bar looks like" is two chances for the rail's
/// to drift from the page's.
fn draw_bar(ctx: &mut Ctx, sv: &ScrollView, view: Rect, length: f32) {
    let look = ScrollbarLook::from_theme();
    let dragging = sv.dragging();
    let hovered = dragging || ctx.mouse.over(bar_band(view, &look));
    let Some(geom) = scroll::scrollbar(view, &look, sv.offset(), view.h, length, hovered)
    else {
        return;
    };
    let alpha =
        if hovered { 1.0 } else { sv.fade_alpha(ctx.t, look.auto_hide, look.fade_ms) };
    nacelle::view::paint::scrollbar(&mut CtxSurface::new(ctx), &geom, alpha, hovered, dragging);
}

/// The box a scrolled view's ROWS really stand in: its own box less the
/// lane the scrollbar keeps beside them.
///
/// TWO CALLERS SINCE THE RAIL SCROLLS — the page's flow and the
/// navigation column — and one sentence for both, which is the point:
/// a lane the rail reserved by a rule of its own would be a second
/// opinion about where a bar lives.
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

/// The lane a bar answers the pointer in: the band it could occupy AT
/// ITS WIDEST, its margin included.
///
/// A bar grows under the pointer (`scrollbar.w_hover`), so a lane
/// measured at the resting width would let go of the very thumb the
/// hover had just widened. Asked by the drawing and by the press, of
/// the page's bar and of the open list's alike — one sentence about
/// where a bar lives, so the hand and the eye cannot be told two
/// different things.
fn bar_band(area: Rect, look: &ScrollbarLook) -> Rect {
    let reach = look.w_hover.max(look.w) + look.margin;
    match look.edge {
        scroll::ScrollbarEdge::Left => Rect::new(area.x, area.y, reach, area.h),
        scroll::ScrollbarEdge::Right => {
            Rect::new(area.right() - reach, area.y, reach, area.h)
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

/// The same, for the navigation column, plus the rectangle the POINTER
/// has to be over for a wheel notch to belong to it.
///
/// The bed and not the rows box, because the air a bed keeps round its
/// buttons (`settings.band_pad_*`) and the corner button's own row are
/// part of the column to a reader's eye. A notch two pixels inside the
/// rail's edge that turned the PAGE instead would be the window telling
/// the hand it had missed something it had not missed.
#[derive(Clone, Copy)]
struct RailFrame {
    bed: Rect,
    flow: Flow,
}

/// Which of the window's scrolls carries a run of rows — the walker's
/// own word, written into a ledger as it lays them.
///
/// THERE ARE TWO SCROLLS SINCE 2026-08-18 and a keyboard chase has to
/// move the right one: bringing a rail entry back into view by moving
/// the PAGE would carry the page off under a column that had not
/// budged. Nothing about a rect says which offset it was laid at, so
/// the geometry cannot be asked; the walker knows, and this is the
/// walker saying it ([`Settings::flowed`], [`Settings::railed`]).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Carrier {
    /// Nothing carries it: the chrome, and a pinned band standing
    /// outside the box the flow is read in.
    Still,
    /// The page's own flow ([`Settings::scroll`]).
    Page,
    /// The navigation column ([`Settings::rail_scroll`]).
    Rail,
}

/// Which question the SAVE-AS-shaped text prompt ([`Settings::naming`])
/// is asking. Both are "type a string, Enter commits it, Esc throws it
/// away" over the SAME `InputModel`, and this is the one fact that tells
/// the two apart: what Enter does with the string, which charset a
/// keystroke is checked against, and the two words the box's title and
/// the field's placeholder wear.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NamingFor {
    /// SAVE / SAVE AS: the theme's own file name, `Self::theme_name_char`.
    /// Enter runs [`Settings::editor_save_named`], which writes to disk.
    ThemeName,
    /// The BACKDROP's wallpaper file, `Self::wallpaper_path_char`. Enter
    /// only sets `Settings::backdrop_image` and previews it — nothing
    /// reaches disk until SAVE / SAVE AS, exactly like every other
    /// control on the page.
    WallpaperPath,
}

pub struct Settings {
    pub open: bool,
    view: View,
    /// WHICH SECTIONS OF THE RAIL STAND UNFOLDED — the expander's own
    /// state, and the answer to the owner's report of 2026-08-18: the
    /// list came up open and pressing its entry did nothing.
    ///
    /// It was not a field until then. `rail_open` read
    /// `act == rail_act(self.view)`, so the unfold was a RESTATEMENT of
    /// which page you were on: LOOK AND FEEL is the section the window
    /// opens on, so the rail opened unfolded, and a press on the entry
    /// went to a page that was already in force — nothing moved, and
    /// there was nothing that COULD move, because no state said "shut".
    /// The reasoning behind that is at [`Settings::rail_open`], where it
    /// is now the record of what was replaced.
    ///
    /// A `Vec` and not an `Option`: two sections may stand open at once
    /// (decision (a) at [`Settings::rail_open`]), so this is a set and
    /// the type says so. Membership is O(n) over a rail that holds ten
    /// rows.
    unfolded: Vec<Act>,
    /// The engine's theme names, for the THEMES list.
    themes: Vec<String>,
    layauts: Vec<String>,
    sounds: Vec<String>,
    /// Current selections from nacelle-desktop.ron (highlighted in the lists).
    current_look: Option<String>,
    current_layaut: Option<String>,
    current_sounds: Option<String>,
    /// The sentence SOUND LEVELS closes with, worked out when the page
    /// is entered rather than while it is drawn — see [`sound_set_note`].
    sound_set: String,
    /// Font view state, indexed by section (0 = Term, 1 = Ui).
    families: [Vec<String>; 2],
    cur_family: [Option<String>; 2],
    cur_weight: [Option<String>; 2],
    /// Font sizes in percent (50-200).
    cur_size: [u32; 2],
    /// The three shapes a border can take, as the list offers them. Built
    background_kinds: Vec<String>,
    current_background: Option<String>,
    /// Glass tint colour, HSV in whole slider units, like `edge`.
    tint: [u32; 3],
    /// Glass wash colour, HSV in whole slider units, like `edge`.
    wash: [u32; 3],
    /// Effect opacity in percent, every background kind — the editor's
    /// one transparency, driven from either page ([`Knob::BgOpacity`]).
    bg_opacity: u32,
    /// Blur pyramid depth, 1..=3.
    bg_depth: u32,
    /// Wash coverage in percent, FROSTED only.
    bg_coverage: u32,
    /// The BACKDROP's own wallpaper — `[backdrop]`, the plate BEHIND the
    /// board (elev 0), never glass, and a different axis than BACKGROUND
    /// above: that section is `Glass` over a panel's bed, this is what
    /// stands under everything, before any panel is drawn at all.
    ///
    /// `Some(path)` is a file chosen this session; `None` is "no wallpaper
    /// — the theme's own `backdrop.solid` stands". Seeded off the live
    /// theme like `current_border`/`current_background`
    /// ([`Settings::seed_editor_from_theme`]), and read back the COLD way
    /// (`nacelle::theme::diagnostics().text(...)`) because `backdrop.image`
    /// is a TEXT token, off `ResolvedTheme` entirely — the same split
    /// `theme::backdrop::bake_wallpaper`'s own module header draws.
    backdrop_image: Option<String>,
    /// Whether this session's hand has touched the wallpaper picker or
    /// its CLEAR button at all — the same "untouched leaves the theme's
    /// own word standing" mark [`Settings::edge_width_touched`] and
    /// [`Settings::glow_reach_touched`] carry, and for the same reason:
    /// a page opened and left alone must save the theme exactly as it
    /// was, not narrate a source of `solid` nobody asked for.
    backdrop_touched: bool,
    /// The SAVE AS prompt, when it is open — the same `InputModel` the
    /// layout editor names its files with, driven here purely by the
    /// keyboard: the field is focused on open, Enter saves, Esc closes.
    ///
    /// Since the wallpaper path prompt (2026-08-23), this ONE field asks
    /// two different questions — [`NamingFor`] says which — because both
    /// are "type a string, Enter commits it, Esc throws it away" and
    /// nothing about the model or the modal around it differs; only what
    /// Enter DOES with the string does.
    naming: Option<nacelle::object::text_input::InputModel>,
    /// Which picker's value plate is open for inline typing, if any — the
    /// "one at a time" bookkeeping [`naming`](Settings::naming) already
    /// has, mirrored for a control that has FOURTEEN plates instead of
    /// one. The typed text itself is NOT here: it lives in that picker's
    /// own `editing: Option<InputModel>`
    /// (`nacelle::object::color_picker::Picker`), because with fourteen
    /// pickers on the page "which one, and what's in the box" is a fact
    /// about the picker being typed into, not a fact about the window.
    /// `Settings::perform`'s own head guard and `Settings::click`'s
    /// "hit nothing" branch are where a press ELSEWHERE blurs
    /// (commits) whichever picker this names — see
    /// `Settings::blur_editing_picker`.
    editing_picker: Option<PickerId>,
    /// Which question [`Settings::naming`] is currently asking. Read only
    /// while `naming` is `Some`; the value left over from the last prompt
    /// otherwise, which is never looked at.
    naming_for: NamingFor,
    /// When the editor last re-baked the desktop during a drag; the pulse
    /// that keeps a live slider from leaking a bake per frame.
    editor_pulse: Option<Instant>,
    /// Which of the editor's two pages is showing: BASIC's one
    /// sliders, or the ADVANCED page that has always been here. The
    /// window keeps BOTH pages' state at all times — that is the whole
    /// of "switching modes loses no work", and the reason this is one
    /// bool beside the rest of the editor rather than two editors.
    editor_basic: bool,
    /// BASIC's move in TRACK units, indexed HUE, SATURATION, LIGHTNESS.
    /// [`TONE_REST`] is the theme untouched.
    ///
    /// Written by three sliders until 2026-08-18 and by
    /// [`Settings::set_tone_from_picker`] since. It stayed a triple of
    /// whole track units through that change on purpose: everything
    /// downstream of it — `tone_of`, the fold into ADVANCED, the write-out
    /// — is about a RELATIVE move and knows nothing about how a person
    /// said how far.
    tone: [u32; 3],
    /// EVERY colour control on the two editor pages, indexed by
    /// [`PickerId::idx`] — BASIC's one and ADVANCED's thirteen.
    ///
    /// A model and not a colour, because the hue of a grey is not in the
    /// grey: drag the field's handle down to the axis and the colour has
    /// no hue left to answer with, so the handle must remember where it
    /// stands ([`nacelle::object::color_picker::Picker`]).
    ///
    /// FOURTEEN MODELS AND NOT ONE SHARED ONE, since 2026-08-18. A single
    /// model handed round would carry the hue of whichever control was
    /// last touched into the next one a hand landed on, and the hue it
    /// carried would be invisible — only a drag onto the grey axis makes
    /// it show, which is the one case the model exists for. The BANK
    /// (`picker_custom`) is shared on purpose and is the opposite case: a
    /// colour put by is a colour the person wants to reach again, and
    /// reaching it from another control is the point of putting it by.
    pickers: [nacelle::object::color_picker::Picker; 14],
    /// The colours the user banked with the picker's own bank cell, in
    /// the order they were banked. THE WINDOW's and not the picker's:
    /// they outlive the frame the control is drawn in, and a control that
    /// kept them would lose them on the next page turn.
    ///
    /// They live for the session and no longer. Writing them to the
    /// config is a decision about a file's shape and belongs with the
    /// rest of `nacelle-desktop.ron`, which is another stage's.
    picker_custom: Vec<nacelle::theme::Color>,
    /// What the last activation SAID, so that "one press, one sound" can
    /// be asked of this window instead of assumed of it. Written only by
    /// [`Settings::say`], cleared at the head of [`Settings::perform`],
    /// and absent from the shipping build entirely.
    #[cfg(test)]
    heard: Vec<nacelle::sound::Event>,
    /// What BASIC's relative move is relative TO: the theme's own
    /// authors, read off the live bake when the page was seeded. `None`
    /// until it has been — an unseeded BASIC writes nothing, the same
    /// neutrality `current_border`'s `None` earned.
    tone_seeds: Option<nacelle::theme::edit::ToneSeeds>,
    /// The MAIN BACKGROUND's own seed — `component.panel.fill`, read live
    /// alongside `tone_seeds` and shown in the SAME picker. BASIC's one
    /// question is "what colour is the desktop", answered by pointing at
    /// its background: the picker opens on THIS, not on the accent, and
    /// `Settings::set_tone_from_picker` measures the move against it, so
    /// dragging the field a little moves the accent a little too — the
    /// same delta, from a bed that starts near the wall rather than an
    /// accent that starts near the ceiling. Kept apart from `tone_seeds`
    /// (a libnacelle type `tone_edits` never reads this field of) rather
    /// than folded in, because nothing downstream of THAT struct needs it:
    /// the background is written literally now (`editor_edits`,
    /// `theme::edit::panel_fill_edit`), never through the ten-author
    /// cascade the rest of BASIC's move rides.
    tone_bed: nacelle::theme::color::Oklch,
    /// WHAT THE THEME ITSELF DRESSES, read ONCE off the file's own state
    /// and never off the screen: the `halo_dressed` answer `theme::edit`
    /// asks its caller for (the focus ring's — the panel edge's went
    /// with the whole effect, 2026-08-27, the owner's order).
    ///
    /// A flag and not a reading, because the reading was a LOOP. The edit
    /// set is rebuilt ten times a second while a track is dragged, and
    /// `set_preview` REPLACES the set rather than merging it, so a token
    /// left out of one pulse is a token switched off. Asking the live
    /// bake "does the theme already dress its halo" asked about the
    /// PREVIOUS PULSE'S OWN ANSWER: pulse one saw the master's zero, wrote
    /// the radius; pulse two saw the radius it had just written, called
    /// the theme dressed and wrote nothing; the halo went out; pulse three
    /// saw zero again. Two pulses to a cycle is the ~5 Hz flicker the
    /// owner reported — and, far worse than the flicker, a SAVE landing on
    /// the wrong side of it wrote a NEON theme with no radius and no
    /// alpha at all, a glow permanently invisible in the file.
    ///
    /// Seeded where every other opening value is ([`Settings::seed_editor_from_theme`]),
    /// which runs on the way in and on CANCEL, with no preview standing in
    /// either case. The intent `halo_dressed` was written for is untouched:
    /// a theme that dressed its own halo still keeps its own numbers.
    ring_halo_dressed: bool,
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
    /// The three radii AS THE THEME WROTE THEM, in the tracks' own units
    /// — the scale BASIC's CORNER SIZE names its steps off
    /// ([`corner_step_word`]).
    ///
    /// Kept beside the live tracks and not derived from them, because the
    /// first press of that control makes all three equal and the ladder
    /// would be gone: pressing SMALL and then LARGE has to reach the
    /// theme's `corner.lg`, not the value SMALL left behind. Read once,
    /// on the way into the editor, from the same numbers the tracks are
    /// seeded with.
    corner_seed: [u32; 3],
    /// `border.edge.width` — the border's OWN thickness, 0..100 over
    /// 0..1u, and whether a hand has moved it.
    ///
    /// THE MARK IS WHAT KEEPS IT OUT OF UNTOUCHED FILES. Written
    /// unconditionally, this key would replace the master's
    /// `@stroke.hair` REFERENCE with a literal in every theme anybody
    /// ever saved — the §5.5 fault the severity marks exist to prevent,
    /// and the one BASIC was caught committing on 2026-08-18.
    edge_width: u32,
    edge_width_touched: bool,
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
    /// The menu's and tooltip's BACKGROUNDS, HSV plus the SEED's alpha kept
    /// beside it: the model passes a colour's alpha through, the sliders
    /// have no say over the channel, and flattening a translucent bed to
    /// opaque just by saving would be an edit nobody made. Their borders
    /// and text are no longer separate colours — the one BORDER picker and
    /// the one TEXT picker wear them (role, not object).
    menu_fill: [u32; 3],
    menu_fill_a: f32,
    /// 0..100 over 1u.
    menu_edge_w: u32,
    tip_fill: [u32; 3],
    tip_fill_a: f32,
    tip_edge_w: u32,
    /// The text ladder's hue seed and whether the FONT picker has cut it
    /// loose from the accent — symmetric with surface_hue / surface_own_hue,
    /// so a picked font colour leads the whole ladder's hue.
    text_hue: u32,
    text_own_hue: bool,
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
    /// When the dropdown was opened, on the FRAME clock (`Ctx.t`, kept
    /// in [`Settings::now`]) — the moment
    /// [`nacelle::object::dropdown::accordion_at`] unfolds the blind
    /// from. Not an `Instant`: the whole unfold belongs to
    /// `motion.menu_unfold` now, and the toolkit's resolver takes time
    /// as a parameter precisely so a test can wind the clock by hand.
    /// `None` is a list standing at rest, fully open.
    dropdown_since: Option<f64>,
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
    /// **What came of the last colour request**, in the compositor's own
    /// terms, for the page to say out loud.
    ///
    /// Written by the application after every `apply` and read by one
    /// note row. It exists because every failure on this page is
    /// INVISIBLE otherwise: a space the compositor will not take, a
    /// description it never answers for, an ICC profile quietly
    /// outranking the list — each of them leaves the picture exactly as
    /// it was while the list draws its mark on the new name. The
    /// program used to say all of it on stderr, and a desktop session
    /// has nowhere to show a stderr (the same reason ADDONS carries the
    /// loader's complaints).
    ///
    /// Private, like the two numbers under it: the application writes
    /// them through [`Settings::color_answered`],
    /// [`Settings::color_asked`] and [`Settings::color_measured`],
    /// because the wish and the measurement have a rule BETWEEN them
    /// (asking unmeasures) and a field anyone may assign cannot keep it.
    color_status: String,
    /// The depth the swapchain was ASKED for, and the depth it GAVE.
    ///
    /// Two numbers because they disagree, and the disagreement is the
    /// interesting part: a surface offering nothing above eight bits
    /// answers eight to a page showing sixteen, and a user hunting for
    /// the difference would find none. The first is the configuration's
    /// (`ColorConf::depth`), the second is read off the renderer's own
    /// swapchain format after the frame in which the rebuild happened.
    color_depth_asked: u32,
    color_depth_now: u32,
    /// The BLUR sliders moved; main re-reads blur_settings().
    pub blur_dirty: bool,
    color_depth: u32,
    color_space: String,
    /// Which half of `config::COLOR_SPACE_TABLE` the SPACE list is
    /// offering, which is also what the HDR switch shows.
    ///
    /// NOT a setting and not written anywhere: the configuration names
    /// a colour space and nothing else, and `bt2020 pq` in the file IS
    /// this being on. It is a field only because the file cannot say
    /// which side "auto" belongs to — "auto" belongs to both — so
    /// somebody has to remember which half the user is looking at while
    /// they look at it. Every other name settles it on sight
    /// ([`Settings::set_space`]).
    color_hdr: bool,
    /// The names the SPACE list is offering right now, rebuilt whenever
    /// the switch turns or the compositor's report arrives. Held as a
    /// field because [`Settings::names`] answers with a reference into
    /// the window, the way every other list does.
    color_spaces: Vec<String>,
    /// The colour spaces THIS compositor said it can be asked for, or
    /// None while nobody has said — no colour manager, or a window
    /// under test. None is not "none of them": a window that has not
    /// been told has learnt nothing, and offers the whole table, which
    /// is what it did before it could ask at all.
    color_supported: Option<Vec<String>>,
    /// The space last standing on each side of the switch, so a trip
    /// across and back is not a trip to the default. Window state and
    /// not configuration: after a restart the file is the only memory
    /// there is, and it says "auto".
    last_sdr: Option<String>,
    last_hdr: Option<String>,
    /// The depth the HDR switch RAISED, kept so that turning HDR off
    /// gives back what turning it on took — and nothing else. Cleared
    /// the moment the user presses a depth themselves, because a depth
    /// they chose is theirs and must survive the switch.
    depth_before_hdr: Option<u32>,
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
    /// THE NAVIGATION COLUMN'S OWN OFFSET, and its own physics.
    ///
    /// A section's pages stand IN the rail since 2026-08-18, so the
    /// column can want more height than the window has. The toolkit's
    /// answer to content that does not fit is a bar and a wheel
    /// (`nacelle::view::scroll`) and this is the rail taking it, rather
    /// than the window folding both panels away at heights that used to
    /// hold two.
    ///
    /// NOT RESET BY [`Settings::go`], unlike the page's. The rail is
    /// PERMANENT — the same column on every page — so where the reader
    /// scrolled it to is a property of the window and not of the section
    /// they happen to be in; resetting it on every section change would
    /// throw away the position with every press it took to get there.
    rail_scroll: ScrollView,
    /// How the last frame laid that column out, or `None` where the
    /// window had folded and there was no column at all.
    rail_flow: Option<RailFrame>,
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
    /// The same ledger for the RAIL's scroll: what the navigation
    /// column registered this frame, and nothing else. Empty whenever
    /// the window has folded — there is no column then, and the
    /// entries are bands of the flow and belong to `flowed`.
    railed: Vec<FocusId>,
    hits: Vec<(Rect, Act)>,
    /// The act whose click flash is decaying, and the frame clock it was
    /// pressed on. On `Ctx.t` for the same reason as
    /// [`Settings::dropdown_since`]: `motion.press` answers the length,
    /// and its resolver is wound by the caller's clock.
    ///
    /// A press arrives BETWEEN frames, so the moment it is dated from is
    /// the last frame's — at most one frame early against a decay
    /// measured in tenths of a second, and the alternative is the
    /// private wall clock this replaced.
    flash: Option<(Act, f64)>,
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
            // SHUT, which is the owner's rule for the state the window
            // comes up in and the only state a rail can be in before
            // anybody has pressed anything.
            unfolded: Vec::new(),
            themes: Vec::new(),
            layauts: Vec::new(),
            sounds: Vec::new(),
            current_look: None,
            current_layaut: None,
            current_sounds: None,
            sound_set: String::new(),
            families: [Vec::new(), Vec::new()],
            cur_family: [None, None],
            cur_weight: [None, None],
            cur_size: [100, 100],
            editor_pulse: None,
            // BLUR and FROSTED GLASS are pulled for now: both sampled a
            // static magenta behind the settings modal (the blur base read
            // uninitialised) while SOLID stayed correct, so the two glass
            // modes leave the list until the blur path is rewritten. SOLID
            // is the only background the editor offers meanwhile.
            background_kinds: vec!["SOLID".to_string()],
            current_background: None,
            // The glass tint's placeholder is the IDENTITY MULTIPLY —
            // full brightness, no saturation — and it is not a colour
            // choice: this triple multiplies the blurred scene, so the
            // only value that decides nothing is the one that changes
            // nothing. `seed_editor_from_theme` overwrites it from
            // `elev.panel.glass.tint` on every visit; it survives only
            // where a theme declares no such key, and there the frost
            // should be exactly what it frosts. It used to open at
            // 60/20/210, which took 46% of the light off every frosted
            // surface in the program the first time BLUR was pressed.
            tint: [100, 0, 0],
            // The wash's placeholder is COLOURLESS for the same reason,
            // and it is the closest thing this slot has to the tint's
            // identity: full brightness, no saturation, so no hue is
            // chosen here. A wash cannot decide nothing the way a
            // multiply can — its alpha is the WASH COVERAGE slider's,
            // beside it — so the honest opening is the one that adds
            // light without adding a colour. `seed_editor_from_theme`
            // overwrites it from `component.panel.fill`, and from
            // `elev.panel.glass.wash` where the theme lays one, on every
            // visit; it survives only where a theme declares NEITHER
            // key, and an unstyled body is what the governing principle
            // asks to see there. It used to open at 20/15/210, a violet
            // belonging to no theme, and that violet reached the file
            // the first time FROSTED GLASS was pressed on any theme
            // wearing a rank with no wash — which is precisely what this
            // editor's own BLUR preset saves.
            wash: [100, 0, 0],
            bg_opacity: 100,
            bg_depth: 50,
            bg_coverage: 42,
            backdrop_image: None,
            backdrop_touched: false,
            naming: None,
            editing_picker: None,
            // Unread while `naming` is `None`; `ThemeName` is the older
            // of the two questions and the harmless default.
            naming_for: NamingFor::ThemeName,
            // The editor opens on ADVANCED — the page that has always
            // been there — with BASIC's move at rest and no
            // seeds yet, so a BASIC page that was somehow reached before
            // `seed_editor_from_theme` ran would write nothing at all.
            // The editor lands on BASIC now — the one page — and the
            // ADVANCED COLOUR button drills into the per-element grid; there
            // is no ADVANCED mode to open on any more.
            editor_basic: true,
            tone: TONE_REST,
            tone_seeds: None,
            // Overwritten by `seed_tone_from_theme` before anything reads
            // it; the literal here is never shown.
            tone_bed: nacelle::theme::color::Oklch { l: 0.232, c: 0.02, h: 0.0, alpha: 1.0 },
            // The picker opens on nothing in particular and is seeded off
            // the theme with everything else the moment the editor is
            // entered ([`Settings::seed_tone_from_theme`]). "Nothing in
            // particular" is `component.picker.rest`, which the toolkit
            // reads for us: this used to be `Color::GREY`, defended as
            // neutrality rather than look — but the grey WAS on screen
            // for as long as it took somebody to reach the editor, and a
            // neutral is a choice like any other. The theme names its own.
            pickers: std::array::from_fn(|_| {
                nacelle::object::color_picker::Picker::at_rest()
            }),
            picker_custom: Vec::new(),
            #[cfg(test)]
            heard: Vec::new(),
            // Nothing dressed until the seeding says so — the same
            // opening neutrality the seeds themselves keep, and the
            // answer the master's own zeroes give.
            ring_halo_dressed: false,
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
            // The same three, so an unseeded page reads AS WRITTEN rather
            // than CUSTOM; the seeding overwrites both sides together.
            corner_seed: [20, 30, 55],
            edge_width: 20,
            edge_width_touched: false,
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
            menu_edge_w: 25,
            tip_fill: [33, 10, 210],
            tip_fill_a: 1.0,
            tip_edge_w: 25,
            text_hue: 210,
            text_own_hue: false,
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
            // Empty and not a cheerful word: nothing has been applied
            // yet, and the note reads that as "nothing to report".
            color_status: String::new(),
            // Zero is "not measured", which is exactly true until the
            // application has told this window. Neither number is a
            // legal depth, so neither can be mistaken for one.
            color_depth_asked: 0,
            color_depth_now: 0,
            blur_dirty: false,
            color_depth: 8,
            color_space: "auto".to_string(),
            // The standard-range half, matching the "auto" above: a
            // window that has read nothing is a window nobody has asked
            // for high range. `set_space` holds the three in step from
            // here on.
            color_hdr: false,
            color_spaces: config::color_spaces(false)
                .into_iter()
                .map(String::from)
                .collect(),
            color_supported: None,
            last_sdr: None,
            last_hdr: None,
            depth_before_hdr: None,
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
            rail_scroll: ScrollView::new(),
            rail_flow: None,
            now: 0.0,
            clip: None,
            flowed: Vec::new(),
            railed: Vec::new(),
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
        // And any road at all drops a held drag — a page it changed under
        // has nothing left of that press to keep answering to, and a
        // mouse-up that was lost on the way here (focus lost mid-drag,
        // say) must not leave one armed to hijack whatever the next page
        // draws in the same spot (2026-08-28's fix).
        self.dragging = None;
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

    /// The picker's dragged slider, from a point in the window.
    ///
    /// The rect comes from the hit map, exactly as a track's does, so the
    /// hand and the ink are reading one layout — ONE COORDINATE now, and
    /// not two: every slider in the bank is a single 0..1 axis, which is
    /// the whole reason a bank of them replaced the old two-axis field
    /// (`nacelle::object::color_picker`'s own module header). Clamping is
    /// the object's ([`Picker::pick_slider`]), so a hand that wanders off
    /// the track keeps dragging along its edge rather than letting go —
    /// the behaviour every slider in this window already has.
    ///
    /// AND IT ENDS WHERE THE PICKER'S OWN PAGE KEEPS ITS ANSWER
    /// ([`Settings::commit_picker`]) — a relative move for BASIC, the
    /// HSV track for each of ADVANCED's thirteen — so the control and the
    /// value it stands for are re-derived together on every step of the
    /// drag and can never be a frame apart.
    fn set_picker_from(&mut self, act: Act, x: f32) {
        let Act::PickerSlider(id, i) = act else { return };
        let Some(r) = self.rect_of_act(act) else { return };
        let frac = nacelle::object::color_picker::slider_frac(r, x);
        self.pickers[id.idx()].pick_slider(i, frac);
        self.commit_picker(id);
    }

    /// What the page DOES with the colour one of its pickers is standing
    /// on. The one crossing between "a control holds a colour" and "the
    /// theme is told about it", so the two roads out of a press — a drag
    /// and a press on a ready-made swatch — cannot answer differently.
    ///
    /// BASIC'S IS NOT A COLOUR, and that asymmetry is the page's, not an
    /// accident of this function: BASIC writes how far the picked colour
    /// is FROM the theme ([`Settings::set_tone_from_picker`]), which is
    /// what keeps every difference the theme's author wrote. ADVANCED's
    /// thirteen are absolute and land on the `[u32; 3]` HSV tracks
    /// `editor_edits` has always read, through [`hsv_track_of`] — the
    /// pair of [`oklch_of_track`], and the reason the two must stay a
    /// pair is written there.
    fn commit_picker(&mut self, id: PickerId) {
        // The three pickers that do NOT land on a track of their own.
        // BASIC's Tone measures a move from the accent; the two family
        // pickers decompose the picked colour into the surface / text
        // ladder seeds, so the whole ladder is re-coloured and keeps its
        // depth ([`Settings::set_surface_family_from_picker`]).
        match id {
            PickerId::Tone => return self.set_tone_from_picker(),
            PickerId::BgMain => return self.set_surface_family_from_picker(),
            PickerId::Text => return self.set_text_family_from_picker(),
            _ => {}
        }
        let colour = self.pickers[id.idx()].colour();
        let track = hsv_track_of(colour);
        match id {
            // The three handled above; named so a new picker cannot be
            // added without deciding where its value goes.
            PickerId::Tone | PickerId::BgMain | PickerId::Text => {}
            PickerId::Edge => self.edge = track,
            PickerId::Tint => self.tint = track,
            PickerId::Wash => self.wash = track,
            PickerId::Accent => self.accent = track,
            // The role standing in the SEVERITY list, and marked touched
            // — the mark `editor_edits` gates the write on, so the six
            // roles nobody pointed at keep the theme's own words.
            PickerId::Severity => {
                for c in 0..3 {
                    self.set_severity(c, track[c]);
                }
            }
            PickerId::Ring => self.ring_colour = track,
            // These three ALSO carry their own alpha (`editor_edits`'s
            // `of(&x, x_a)` pairs, unlike every other field above, which
            // hardcodes 1.0) — `hsv_track_of` only ever reads RGB, so the
            // `_a` field is this picker's one other landing spot, kept in
            // step with the same drag/blur/Enter-commit `track` already
            // goes through (2026-08-28's fix: a typed alpha used to reach
            // the picker's own swatch and nowhere past it).
            PickerId::MenuFill => {
                self.menu_fill = track;
                self.menu_fill_a = colour.a;
            }
            PickerId::TipFill => {
                self.tip_fill = track;
                self.tip_fill_a = colour.a;
            }
            PickerId::BarTrack => {
                self.bar_track_colour = track;
                self.bar_track_a = colour.a;
            }
        }
    }

    /// Closes whichever picker's inline editor is open, COMMITTING it —
    /// never cancelling. `Picker::commit_edit` closes on a good parse by
    /// itself; a bad one is force-closed with `Picker::cancel_edit`,
    /// which is safe precisely because a bad parse never touched the
    /// colour (`set_text`'s own contract) — there is nothing left to
    /// discard but the typed text. This is BLUR, not Enter: Enter's own
    /// handling (`Settings::key`) leaves a bad parse OPEN for another
    /// try, because focus has not left yet and there is still an
    /// affordance to fix it; a blur has none, so it takes the last-good
    /// colour and moves on.
    ///
    /// A no-op when nothing is open, so every caller can reach for this
    /// unconditionally before a press it does not otherwise know is
    /// "elsewhere".
    fn blur_editing_picker(&mut self) {
        let Some(id) = self.editing_picker.take() else { return };
        if !self.pickers[id.idx()].commit_edit() {
            self.pickers[id.idx()].cancel_edit();
        }
        self.commit_picker(id);
        self.apply_editor_preview();
    }

    /// The main-background picker leads the surface family: the picked
    /// colour's hue becomes the surfaces' OWN hue, its lightness the
    /// ladder's lift and its chroma the chroma scale, so the six beds keep
    /// their depth while wearing the picked colour. The anchors are the
    /// base bed's (`surface.base` L, default.theme 5.5) and a low chroma;
    /// exact fidelity is a live-tuning matter, the depth relationship is not.
    fn set_surface_family_from_picker(&mut self) {
        let c = self.pickers[PickerId::BgMain.idx()].oklch();
        self.surface_own_hue = true;
        self.surface_hue = c.h.rem_euclid(360.0).round().clamp(0.0, 359.0) as u32;
        self.surface_lift =
            span_back((c.l - BG_ANCHOR_L).clamp(-SURFACE_LIFT_WALL, SURFACE_LIFT_WALL), SURFACE_LIFT_WALL);
        self.surface_chroma =
            scale_back((c.c / BG_ANCHOR_C).clamp(0.0, SURFACE_CHROMA_CEILING), SURFACE_CHROMA_CEILING);
    }

    /// The FONT picker leads the text family the same way, off `text.primary`'s
    /// lightness — hue to `text.hue`, lightness to `text.lift`, chroma to the
    /// scale — so every text role wears the picked colour and keeps its rung.
    fn set_text_family_from_picker(&mut self) {
        let c = self.pickers[PickerId::Text.idx()].oklch();
        self.text_own_hue = true;
        self.text_hue = c.h.rem_euclid(360.0).round().clamp(0.0, 359.0) as u32;
        self.text_lift =
            span_back((c.l - TXT_ANCHOR_L).clamp(-TEXT_LIFT_WALL, TEXT_LIFT_WALL), TEXT_LIFT_WALL);
        self.text_chroma =
            scale_back((c.c / TXT_ANCHOR_C).clamp(0.0, TEXT_CHROMA_CEILING), TEXT_CHROMA_CEILING);
    }

    /// The colour a family picker SHOWS — its seeds read back as one colour
    /// at the ladder's anchor, the inverse of the decompose above.
    fn surface_family_track(&self) -> [u32; 3] {
        let ok = nacelle::theme::color::Oklch {
            l: BG_ANCHOR_L + span_of(self.surface_lift, SURFACE_LIFT_WALL),
            c: BG_ANCHOR_C * scale_of(self.surface_chroma, SURFACE_CHROMA_CEILING),
            h: self.surface_hue as f32,
            alpha: 1.0,
        };
        // from_oklch answers in LINEAR light; hsv_track_of reads sRGB.
        hsv_track_of(nacelle::theme::Color::from_oklch(ok).to_srgb())
    }

    fn text_family_track(&self) -> [u32; 3] {
        let ok = nacelle::theme::color::Oklch {
            l: TXT_ANCHOR_L + span_of(self.text_lift, TEXT_LIFT_WALL),
            c: TXT_ANCHOR_C * scale_of(self.text_chroma, TEXT_CHROMA_CEILING),
            h: self.text_hue as f32,
            alpha: 1.0,
        };
        // from_oklch answers in LINEAR light; hsv_track_of reads sRGB.
        hsv_track_of(nacelle::theme::Color::from_oklch(ok).to_srgb())
    }

    /// The colour an ADVANCED picker should be SHOWING — its track, read
    /// back as a colour.
    ///
    /// `None` for BASIC's, which stands on no track of this window at
    /// all: its handles are put on the theme's accent by
    /// [`Settings::seed_tone_from_theme`], because BASIC's move is
    /// measured from there.
    fn picker_track(&self, id: PickerId) -> Option<[u32; 3]> {
        Some(match id {
            PickerId::Tone => return None,
            PickerId::Edge => self.edge,
            PickerId::Text => self.text_family_track(),
            PickerId::BgMain => self.surface_family_track(),
            PickerId::Tint => self.tint,
            PickerId::Wash => self.wash,
            PickerId::Accent => self.accent,
            PickerId::Severity => self.severity[self.severity_idx()?],
            PickerId::Ring => self.ring_colour,
            PickerId::MenuFill => self.menu_fill,
            PickerId::TipFill => self.tip_fill,
            PickerId::BarTrack => self.bar_track_colour,
        })
    }

    /// Put every ADVANCED picker's handles on the colour its track says,
    /// which is what makes a picker OPEN ON THE THEME the way every other
    /// control on the page does.
    ///
    /// CALLED AFTER THE TRACKS ARE SEEDED AND NOT INSTEAD OF IT: the
    /// tracks are still the value — `editor_edits` reads them and knows
    /// nothing about controls — and this is the control catching up with
    /// it. That is also why the SEVERITY picker is re-seeded whenever the
    /// role list changes: the track under it is a different role's.
    ///
    /// A GREY TRACK KEEPS THE HANDLE'S HUE. `Picker::set_colour` is the
    /// object's own door and it holds the hue when the colour has none,
    /// so seeding a colourless track does not throw the field's cursor
    /// back to red.
    fn seed_pickers_from_tracks(&mut self) {
        for id in PickerId::ALL {
            let Some(track) = self.picker_track(id) else { continue };
            let colour = nacelle::theme::Color::from_oklch(oklch_of_track(&track, 1.0));
            self.pickers[id.idx()].set_colour(colour.to_srgb());
        }
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
    ///
    /// IT TAKES THE POINTER because the window has TWO scrolls now — the
    /// page's and the navigation column's ([`Settings::rail_scroll`]) —
    /// and the only thing that can say which one a notch is aimed at is
    /// where the hand is. It had no need of it while there was one.
    pub fn wheel(&mut self, notches: f32, x: f32, y: f32) {
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
        let p = ScrollPhysics::from_theme();
        // The column the hand stands over takes the turn, measured
        // against the BED it was drawn on ([`RailFrame`]). Folded there
        // is no bed and no `rail_flow`, and the entries are bands of the
        // page's own flow — one scroll, exactly as before.
        match self.rail_flow {
            Some(rail) if rail.bed.contains(x, y) => {
                self.rail_scroll.wheel(-notches, &p, self.now)
            }
            _ => self.scroll.wheel(-notches, &p, self.now),
        }
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

    /// BASIC's picked colour as the model's own relative move.
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
        // The BED's chroma now (2026-08-19), not the accent's: `sat` and
        // `hue_deg` are measured against the bed (`set_tone_from_picker`),
        // so a notch fine enough to matter has to be fine relative to
        // WHAT IS BEING DIVIDED BY, and that is the bed's own C.
        let seed_chroma = self.tone_seeds.map_or(0.0, |_| self.tone_bed.c);
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

    /// The picked colour, as the RELATIVE move BASIC has always written.
    ///
    /// THE PICKER IS ABSOLUTE AND THE PAGE IS NOT, and this is the one
    /// line between them. A person points at the colour they want the
    /// desktop to be; what the theme receives is how far that is from the
    /// colour it already has —
    ///
    ///     hue    = picked.h − seed.h        (degrees round the circle)
    ///     sat    = picked.c ÷ seed.c        (a multiplier, 100 % = as written)
    ///     light  = picked.l − seed.l        (an offset on the ladder)
    ///
    /// — which is exactly the triple the three sliders wrote, so nothing
    /// downstream of `self.tone` can tell which control moved it. That is
    /// what keeps the difference the theme's author put between `ok` and
    /// `critical`, between a surface and the surface above it: an
    /// absolute write would flatten every one of them onto the picked
    /// colour.
    ///
    /// THE NOTCH IS STILL THE PIPELINE'S. The picker is continuous and
    /// the tone is not: each number lands on a whole multiple of what one
    /// output code can show ([`Settings::tone_step`]), which is the
    /// statement the sliders made with their `step`. Without it a drag
    /// across the field would write hundreds of tones the swapchain
    /// cannot tell apart, and every one of them would cost a bake.
    ///
    /// A GREY THEME CANNOT BE SCALED, and the multiplier says so by
    /// standing still: with `seed.c` at zero there is no chroma to scale
    /// and the ratio is not a number. The hue and the lightness still
    /// move, which is all a grey theme has to move.
    fn set_tone_from_picker(&mut self) {
        let Some(seeds) = self.tone_seeds else { return };
        // THE MOVE IS MEASURED AGAINST THE SEED AS THIS CONTROL CAN SHOW
        // IT, and the reason is like-against-like. The control holds a
        // colour the way a screen holds one (sRGB, in the field's own
        // coordinates), so the theme's accent put in and read back out is
        // not the same number to the fourth decimal — the master's mint
        // comes back with about 0.6 % less chroma. Both sides of a
        // subtraction should have been through the same pipe, or the
        // difference carries the pipe's loss as if a hand had made it.
        //
        // WHAT THIS IS NOT: it is not what fixed the untouched page
        // asking for a multiplier of 99. That was the notch grid being
        // laid from zero (see below), and it is worth stating plainly,
        // because attributing a fix to the wrong cause is the mistake
        // `.gap-program/obalone-naprawy.md` exists to record. On the
        // master's numbers the loss is under half a track unit, so this
        // line changes nothing that can be measured today; it is the
        // discipline that keeps it that way when a theme's seed sits
        // nearer a gamut wall.
        //
        // Pure, and no fourth piece of state: the origin is a function
        // of the seed and of the control's own arithmetic, so it cannot
        // fall out of step with either.
        //
        // MEASURED AGAINST THE BED, NOT THE ACCENT (2026-08-19). The
        // picker opened on `self.tone_bed` (`Settings::seed_tone_from_theme`)
        // and that is what "no move" has to mean here too, or a picker
        // sitting still would still write a distance — the accent's own,
        // which is nowhere near what the field is showing. The SAME
        // `tone` this produces still shifts `seeds.accent` by
        // `tone_edits` below, unchanged: measuring from a bed near the
        // dark wall instead of an accent near the light one is what
        // keeps the accent "jaśniejszy odcień" of whatever the bed
        // becomes — a small requested move stays small on a bed that
        // starts close to it, where it would have dragged the accent
        // most of the way down to match a dark pick, wiping out the very
        // gap that reads as "lighter."
        let seed = nacelle::object::color_picker::Picker::of(
            nacelle::theme::Color::from_oklch(self.tone_bed).to_srgb(),
        )
        .oklch();
        let want = self.pickers[PickerId::Tone.idx()].oklch();
        let step = self.tone_step();
        // One number onto its track: rounded to a whole notch, then held
        // inside the track's own ends — the same two walls the sliders
        // had, stated once instead of three times.
        //
        // THE GRID OF NOTCHES IS ANCHORED AT REST, not at zero, and the
        // difference is the whole of whether this page can be left alone.
        // The saturation track's notch on the master is THREE units and
        // its rest is a hundred, which is not a multiple of three: a grid
        // laid from zero has no notch at rest, so an untouched picker
        // rounded to 99 and quietly desaturated the theme. Measured from
        // rest, every track's rest is on the grid by construction, and
        // "no move" is a value the arithmetic can express.
        let notch = |v: f32, rest: u32, s: u32, hi: u32| {
            let s = s.max(1) as f32;
            let rest = rest as f32;
            (rest + ((v - rest) / s).round() * s).clamp(0.0, hi as f32) as u32
        };
        let sat = if seed.c > 1e-4 { want.c / seed.c * 100.0 } else { 100.0 };
        self.tone = [
            notch((want.h - seed.h).rem_euclid(360.0), TONE_REST[0], step[0], TONE_HUE_MAX),
            notch(sat, TONE_REST[1], step[1], TONE_SAT_MAX),
            notch(
                span_back(want.l - seed.l, TONE_LIGHT_SPAN) as f32,
                TONE_REST[2],
                step[2],
                100,
            ),
        ];
        // THE BORDER RIDES THE SAME PICKER AS THE REST. BASIC has one
        // colour and the ring is it: sync the border-colour field to the
        // accent this move lands on, so `editor_edits` writes the ring in
        // the picked colour and `fold_tone_into_advanced` carries it into
        // ADVANCED with no second control on the page. The move is measured
        // to whole notches above, so the ring is read from the SAME notched
        // tone (`tone_of`) the surfaces and text are — not from `want`,
        // which would put the ring a fraction of a notch off every bed.
        let moved = seeds.shifted(self.tone_of());
        self.edge = hsv_track_of(nacelle::theme::Color::from_oklch(moved.accent).to_srgb());
    }

    /// BASIC's move, folded into the ADVANCED page's own controls.
    ///
    /// This is what makes leaving BASIC keep its work. BASIC is
    /// RELATIVE, so its edits exist only while its band is standing;
    /// the moment the page turns, `editor_edits` stops writing them and
    /// the ADVANCED controls answer for the same authors alone. So
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
    /// Nothing outside the authors BASIC writes is touched.
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
        // THE SEVEN SEVERITY ROLES USED TO BE FOLDED HERE, AND MARKED
        // TOUCHED, AND THAT WAS THE COST OF A BUG. BASIC turned them the
        // whole way, so every one of them had to become this page's own
        // word or the next preview would have put the theme's back —
        // which meant that MERELY VISITING BASIC pinned all seven into
        // every file the editor ever saved afterwards, and a theme's own
        // amber `contained` was gone. BASIC does not touch them any more
        // (`theme::edit::tone_edits`): they lean toward `palette.accent`
        // in the theme itself, so there is nothing to fold and nothing to
        // mark, and a role only becomes this page's word when somebody
        // points at it with the SEVERITY picker.
        self.surface_lift = span_back(moved.surface_lift, SURFACE_LIFT_WALL);
        self.text_lift = span_back(moved.text_lift, TEXT_LIFT_WALL);
        // THE BODY'S BED COMES TOO. The BACKGROUND section holds it as an
        // absolute colour and not as one of BASIC's authors, so the move
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

    /// The theme's AUTHORS as they stand, and BASIC's move back at
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
        // THE THREE GROUNDS. They are what a re-colour used to leave
        // behind — hex literals on the accent's own hue, and the only
        // targets `shade()` and `tint()` have — so the move has to carry
        // them, and to carry them it has to know where they stand. A
        // ground this build cannot read falls back to the accent, which
        // is the hue the move would put it on in any case.
        let ground = |n: &str| col_of(n).map_or(accent, |c| c.to_oklch());
        self.tone_seeds = Some(nacelle::theme::edit::ToneSeeds {
            accent,
            black: ground("palette.black"),
            white: ground("palette.white"),
            neutral: ground("palette.neutral"),
            surface_lift: px("surface.lift"),
            text_lift: px("text.lift"),
        });
        // THE BACKGROUND'S OWN SEED (2026-08-19). BASIC's one question is
        // "what colour is the desktop", and the desktop's answer is its
        // panels' bed — `component.panel.fill` — not the accent, which is
        // a POP colour standing far up the ladder from it. A token this
        // build cannot read falls back to the accent too, the same
        // neutrality `ground` keeps for the palette's poles.
        self.tone_bed = col_of("component.panel.fill").map_or(accent, |c| c.to_oklch());
        // AND THE CONTROL OPENS ON THE BACKGROUND, like every other one on
        // this page opens on the theme. The move is measured FROM the
        // bed now (`Settings::set_tone_from_picker`), so the picker's
        // handles stand there at rest: a picker showing anything else
        // would say the desktop is a colour it is not, and the first drag
        // would then write that lie's distance into the file — the accent
        // included, since the accent still shifts by the SAME delta this
        // origin measures (`Settings::tone_of`, `theme::edit::tone_edits`
        // unchanged: only what the move is measured against moved).
        //
        // Seeded opaque (ZGŁOSZENIE, 2026-08-19 — the owner's second word
        // on this page, after OPACITY's slider came back: transparency
        // is that slider's question alone now, so this picker answers a
        // question with no alpha in it at all) — `self.tone_bed`'s OWN
        // alpha (whatever the live `component.panel.fill` carries) has
        // nothing to do with what this picker is FOR any more, and
        // showing it would be a number nobody asked. HSL is the only
        // notation left (2026-08-28) and it CAN spell alpha, unlike the
        // RGB this page forced back when the picker's own default still
        // could — but locking `alpha` to 1.0 here keeps `with_alpha`
        // from ever appending one, so the guarantee holds the same way.
        let bed_rgb =
            nacelle::theme::color::Oklch { alpha: 1.0, ..self.tone_bed };
        self.pickers[PickerId::Tone.idx()].set_oklch(bed_rgb);
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
            accent_edit, border_colour_edit, border_width_edit,
            focus_ring_edits, glass_edits, menu_edits, panel_fill_edit,
            scrollbar_edits, severity_role_edit, shape_edits, surface_edits, text_edits,
            tooltip_edits, unfocused_dim_edit, CornerCut, Edit, FocusRing, Glass,
            GlassReach, RingStyle, Scope, ScrollbarEdge, ScrollbarMode, SurfaceHue,
        };
        /// ONE ASSIGNMENT PER TOKEN. A list carrying a token twice would
        /// save a file with the key written twice in one section, and the
        /// file and the screen would then be answering to two different
        /// rules about which of the two wins. Used wherever a later
        /// control has to outrank an earlier set's own seed.
        fn set_edit(
            edits: &mut Vec<nacelle::theme::edit::Edit>,
            e: nacelle::theme::edit::Edit,
        ) {
            match edits.iter_mut().find(|b| b.token == e.token) {
                Some(slot) => *slot = e,
                None => edits.push(e),
            }
        }
        // The sliders are HSV — brightness, saturation, hue — and the file
        // wants OKLCh, so every value below crosses HSV -> sRGB -> LINEAR
        // -> OKLCh on the way out. That map is [`oklch_of_track`], written
        // once and paired with [`hsv_track_of`] going the other way; the
        // decode in the middle of it is why they must stay a pair, and
        // what happens when they do not is recorded there. See
        // [`hsv_to_rgb`] for why HSV at all: brightness 100 % must be the
        // hue's own full brightness, never white.
        let of = oklch_of_track;
        let colour = of(&self.edge, 1.0);
        // The border: one colour on the shared root, and — once a hand
        // has moved it — one thickness. (The border EFFECT and its whole
        // edit set left with the effect itself, 2026-08-27, the owner's
        // order.)
        let mut edits = vec![border_colour_edit(Scope::Theme, colour)];
        // ZGŁOSZENIE 6: the THICKNESS, only once a hand has moved it (the
        // mark is the field's, and why it exists is written there). It is
        // unconditional in everything else: every border draws a line.
        if self.edge_width_touched {
            set_edit(&mut edits, border_width_edit(Scope::Theme, scale_of(self.edge_width, 1.0)));
        }
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
            // rail and every other bed and left the BODY on the theme's
            // old hue. Measured on the master at the first slider position
            // the gate takes: rail 203.46 deg, the body still 166.22. It is the same case
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
            // ZGŁOSZENIE 7 (owner, 2026-08-18): "w trybie BASIC zmiana
            // przezroczystości wpływa TYLKO na główne tło obiektu".
            //
            // The knob is the same one on both pages — one OPACITY, one
            // field — and what the page decides is HOW FAR its alpha
            // travels. On ADVANCED it still dresses every reachable rung,
            // which is the page for "what exactly should this token do"
            // and is what keeps a menu from being the one flat plate over
            // a frosted window. On BASIC the RANK still travels (the float
            // is still glass) and the two colour keys — where the alpha
            // lives — stop at the body's own rung.
            //
            // WHAT THIS TOOK AWAY, said plainly because a narrowing must
            // name the tokens it orphans: on BASIC, `elev.popover.glass
            // .tint` and `elev.popover.glass.wash` are no longer written
            // by anything. Who sets them now — the THEME FILE, and nobody
            // else; the master ships `#FFFFFF / 1.0` and `none`, an
            // identity multiply and no wash. ADVANCED is unchanged.
            let reach =
                if self.editor_basic { GlassReach::BodyOnly } else { GlassReach::EveryRung };
            edits.extend(glass_edits(
                Scope::Theme,
                kind,
                carry.shift(of(&self.tint, 1.0)),
                carry.shift(of(&self.wash, 1.0)),
                self.bg_opacity as f32 / 100.0,
                1.0 + self.bg_depth.min(100) as f32 / 50.0,
                self.bg_coverage as f32 / 100.0,
                reach,
            ));
            // BASIC'S PROMISE, TAKEN LITERALLY (2026-08-19). The Tone
            // picker no longer just MOVES the background through the
            // wash-and-opacity pair above — on SOLID it IS the
            // background: `component.panel.fill` becomes the bed
            // (`self.tone_bed`) shifted by the SAME notched `tone` every
            // other author on this page moves by, outranking the
            // wash-derived seed `glass_edits` just wrote (`set_edit`, the
            // same rule ZGŁOSZENIE 6's width and reach already stand on).
            //
            // THE COLOUR RIDES `carry` (BASIC's notched move), NOT THE
            // PICKER WIDGET'S OWN LIVE STATE. A control that moved `self.
            // tone` without also dragging the field's handle — a test
            // poking the array directly, or any future control this page
            // grows — must still turn the body, exactly as it already
            // turns the accent and the rail; reading the widget's raw
            // `.oklch()` here answered only the LAST drag and stood still
            // for everything else, which is the bug this shape fixes.
            //
            // THE ALPHA DOES NOT RIDE IT. `Tone` has no fourth number —
            // hue, saturation and lightness are the whole of what a
            // notched move carries — so transparency is OPACITY's answer
            // still (ZGŁOSZENIE, 2026-08-19: the slider came back and the
            // picker went RGB), the SAME `self.bg_opacity` `glass_edits`
            // just read above — one number, read twice, never two numbers
            // that could disagree.
            if self.editor_basic && matches!(kind, Glass::Solid) {
                let picked = nacelle::theme::color::Oklch {
                    alpha: self.bg_opacity as f32 / 100.0,
                    ..carry.shift(self.tone_bed)
                };
                set_edit(&mut edits, panel_fill_edit(Scope::Theme, picked));
            }
        }
        // WALLPAPER — `[backdrop]`, the plate behind the board (elev 0),
        // never glass and no `Glass`/`Border` kind of its own in
        // `theme/edit.rs` to build on: nobody has written a
        // `backdrop_edit` there yet (out of that module's own stated
        // scope — its header names `backdrop` among the sets it
        // deliberately does not offer), so this writes the two tokens
        // directly, by name, the same way the theme FILE spells them.
        // `Edit`'s fields are public for exactly this: a token this
        // window's model has no builder for is still one construction
        // away, not a wall.
        //
        // Untouched (`backdrop_touched` false) writes NOTHING, same rule
        // as `edge_width_touched`/`glow_reach_touched` above: a page
        // opened and left alone must save the theme exactly as it was.
        // Touched with a path writes `image` + the file; touched with
        // none (CLEAR) writes `solid` — an EXPLICIT revert, not a
        // silence, because a silence here would leave the file's OLD
        // image standing after a hand asked for it gone.
        if self.backdrop_touched {
            match self.backdrop_image.as_deref().filter(|p| !p.trim().is_empty()) {
                Some(path) => {
                    set_edit(&mut edits, Edit { token: "backdrop.source", value: "image".to_string() });
                    set_edit(
                        &mut edits,
                        Edit { token: "backdrop.image", value: Self::quote_theme_text(path) },
                    );
                }
                None => {
                    set_edit(&mut edits, Edit { token: "backdrop.source", value: "solid".to_string() });
                }
            }
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
        // The FONT picker leads the text ladder's hue when it has cut it
        // loose from the accent; OFF restores the reference, so a later
        // accent drag keeps carrying the text with it (the same neutrality
        // `surface_edits` above earns from `surface_own_hue`).
        edits.extend(text_edits(
            Scope::Theme,
            if self.text_own_hue {
                SurfaceHue::Own(self.text_hue as f32)
            } else {
                SurfaceHue::FollowAccent
            },
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
                // The same dress contract as the border's NEON, off the
                // same seeding and for the same reason: a theme that has
                // dressed its halo keeps its radius, and the question is
                // about the FILE, never about the picture on screen.
                halo_dressed: self.ring_halo_dressed,
            };
            edits.extend(focus_ring_edits(Scope::Theme, self.ring_on, &ring));
        }
        edits.push(unfocused_dim_edit(
            Scope::Theme,
            self.unfocused_dim.min(100) as f32 / 100.0,
        ));
        // The floats' colours carry the SEED's alphas — the model passes
        // a colour's channel through, and the sliders have no say in it.
        // Menu and tooltip wear the ROLE colours, not their own: the one
        // BORDER picker (`self.edge`) paints their edges the same as the
        // window's, and the one TEXT picker paints their text the same as
        // the interface's — only their FILLS are their own.
        let border = of(&self.edge, 1.0);
        let text_colour = of(&self.text_family_track(), 1.0);
        edits.extend(menu_edits(
            Scope::Theme,
            of(&self.menu_fill, self.menu_fill_a),
            border,
            scale_of(self.menu_edge_w, 1.0),
            text_colour,
        ));
        edits.extend(tooltip_edits(
            Scope::Theme,
            of(&self.tip_fill, self.tip_fill_a),
            border,
            scale_of(self.tip_edge_w, 1.0),
            text_colour,
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
        // The one move writes AUTHORS — the accent, the palette's three
        // grounds and the two ladder lifts; everything above either
        // agrees with them or is about something else entirely, and the
        // ones that overlap are what BASIC is FOR.
        if self.editor_basic {
            if let Some(seeds) = self.tone_seeds {
                let tone = self.tone_of();
                for e in nacelle::theme::edit::tone_edits(Scope::Theme, &seeds, tone) {
                    set_edit(&mut edits, e);
                }
            }
        }
        edits
    }

    /// Shows what the editor is set to, without writing anything.
    ///
    /// Called when a value SETTLES — a slider released, an arrow pressed, a
    /// kind chosen — and, since the owner asked for the picture to follow
    /// the hand, TEN TIMES A SECOND WHILE A TRACK IS DRAGGED
    /// ([`Settings::drag`]). Each call re-bakes the theme, and a bake is
    /// 76 031 bytes that is never freed, which is what the pulse is for:
    /// ten a second is affordable where sixty is not. The slider itself
    /// still moves every frame; only the picture behind it waits.
    ///
    /// The line that used to stand here said "never while a slider is
    /// being dragged", and the invariant that sentence guarded — one
    /// preview per gesture, built from the theme AS IT IS IN THE FILE —
    /// was what let [`Settings::editor_edits`] read the live bake for its
    /// two dress questions. The pulse broke it and nobody looked at what
    /// had been standing on it; the halo blinked at ~5 Hz for a day. The
    /// set is a pure function of the controls now
    /// ([`Settings::edge_halo_dressed`]), so a set laid over the theme and
    /// then rebuilt is the same set, and calling this at any rate at all
    /// is a question about cost and nothing else.
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
        // THE DRESS QUESTION, ASKED HERE AND NOWHERE ELSE. The road onto
        // this line runs with no preview standing: CANCEL clears it a
        // statement earlier, and the door is reached from another page,
        // which means [`Settings::go`] has already dropped whatever the
        // last visit laid. So the bake being read is the THEME's, which
        // is the only state the question means anything about.
        // `editor_edits` reads this field, which is what keeps it a pure
        // function of the controls: build a set, lay it over the theme,
        // build it again, and the two sets agree. Asked of the live bake
        // instead, the second answer was the first set's own output and
        // the halo blinked at ~5 Hz.
        //
        // ZGŁOSZENIE 6's number, seeded like every other control on this
        // page: off the theme, and MARKED UNTOUCHED, so a visit that
        // does not move it leaves nothing in the saved file. The width's
        // wall is the master's own (`[stroke] bold = 0.7u`), stated once
        // here and once in the row.
        self.edge_width = scale_back(px("border.edge.width") / unit, 1.0);
        self.edge_width_touched = false;
        self.ring_halo_dressed =
            px("glow.focus_ring.radius") > 0.0 && px("glow.focus_ring.alpha") > 0.0;

        // The background: kind from the rank and the wash, and then EVERY
        // control the kind owns — two colours and three amounts — off the
        // theme, whatever the kind. Not one of them off this file.
        let rank_px = px("elev.panel.glass.rank");
        let rank = rank_px.round() as u32;
        let wash_a = col_of("elev.panel.glass.wash").map_or(0.0, |c| c.a);
        // The kind read straight off the rank and the wash. BLUR and
        // FROSTED GLASS are no longer OFFERED (`background_kinds` lists only
        // SOLID until the blur path is rewritten), but the reading stands:
        // the machinery that round-trips a rank-bearing theme is kept whole
        // for that rewrite, and a theme carrying one opens showing its own
        // amounts. The dropdown simply has no row to mark for a kind it does
        // not offer — `current_row` answers `None` there, by design — so the
        // editor cannot SELECT blur, which is what pulling it means.
        self.current_background = Some(
            match (rank, wash_a > 0.0) {
                (0, _) => "SOLID",
                (_, false) => "BLUR",
                (_, true) => "FROSTED GLASS",
            }
            .to_string(),
        );
        // THE TINT IS SEEDED WHATEVER THE RANK, and that is the whole of
        // this paragraph's business. `elev.panel.glass.tint` is declared
        // on every one of the ladder's nine rungs and every shipped theme
        // writes `#FFFFFF / 1.0` there — the IDENTITY MULTIPLY, which is
        // exactly what `edit::Glass::Blur` means by "the tint left
        // neutral". Read only where the theme already had glass, the
        // slider opened on a triple written in Rust instead, and the
        // first press of BLUR wrote that triple into the file: measured
        // 2026-08-18, HSV 60/20/210 -> sRGB(0.480, 0.540, 0.600). The
        // tint MULTIPLIES the blurred scene and can only darken (the
        // ladder's own head says so), so the frost lost 46% of its light
        // before it had any — and it reached `elev.popover` with the
        // panel, which is why the menus went dark with the windows.
        //
        // A LOOK DECIDED IN RUST is the one thing this program may not
        // do, and a slider seeded from nothing is a look decided in Rust
        // the moment somebody presses the control beside it.
        if let Some(c) = col_of("elev.panel.glass.tint") {
            seed(&mut self.tint, c);
        }
        // AND SO IS THE WASH GROUP, for the same reason and against the
        // same hole. One slot serves two keys — SOLID writes it to
        // `component.panel.fill`, FROSTED to `elev.*.glass.wash` — and it
        // was read only where the kind that writes it was the kind the
        // theme already wore. The case in between is not exotic: it is
        // the theme THIS EDITOR SAVES when the owner picks BLUR, which
        // writes a rank and `wash = none` by definition
        // (`edit::glass_edits`). Reopen that file and the wash tracks
        // held 20/15/210 out of `Settings::new`; press FROSTED GLASS and
        // that violet went into the file as the panels' own colour.
        //
        // The fill FIRST and the wash OVER it, which is the order the two
        // keys stand in rather than a preference: `component.panel.fill`
        // is the body every theme declares and the seam SOLID writes
        // back through, so it is the answer whenever the frost has no
        // light of its own; a wash with alpha is the theme saying
        // something more particular, so it wins where it exists.
        if let Some(c) = col_of("component.panel.fill") {
            seed(&mut self.wash, c);
        }
        if let Some(c) = col_of("elev.panel.glass.wash") {
            if c.a > 0.0 {
                seed(&mut self.wash, c);
            }
        }
        // THE THREE AMOUNTS, on the same rule as the two colours. This
        // page's own head promises that the maps back onto the tracks are
        // the exact inverses of `editor_edits`' maps out, "so a theme
        // saved and reopened lands the sliders where the hand left them",
        // and for OPACITY, BLUR DEPTH and WASH COVERAGE that was simply
        // not so: they opened on 100, 50 and 42 out of `Settings::new`
        // every visit, so reopening a theme saved at depth 2.6 offered
        // depth 2.0, and an editor opened on the master and put back on
        // SOLID turned the panels opaque — the master's own body carries
        // alpha 0.82.
        //
        // Each is read from the key the kind it belongs to WRITES, and
        // only where that key has something to say: a coverage cannot be
        // read off a wash that is not there, and a depth cannot be read
        // off a rank of zero. Where it has nothing to say the opening
        // value stands, which is the one place these three numbers are
        // still this file's — and the controls they drive are not on
        // screen there (`Row::when`: no depth without a blur, no coverage
        // without a frost).
        let alpha_key = if rank == 0 { "component.panel.fill" } else { "elev.panel.glass.tint" };
        if let Some(c) = col_of(alpha_key) {
            self.bg_opacity = (c.a * 100.0).round().clamp(0.0, 100.0) as u32;
        }
        if rank_px > 0.0 {
            // `editor_edits` maps the track by `1.0 + track / 50`, so the
            // way back is the way out, run backwards.
            self.bg_depth =
                ((rank_px.clamp(1.0, 3.0) - 1.0) * 50.0).round().clamp(0.0, 100.0) as u32;
        }
        if wash_a > 0.0 {
            self.bg_coverage = (wash_a * 100.0).round().clamp(0.0, 100.0) as u32;
        }

        // WALLPAPER: `backdrop.source` is an ENUM token — POD, so `word`
        // reads it the same way it reads `corner.mode` and every other
        // kind on this page. `backdrop.image` is not: a TEXT token off
        // `ResolvedTheme` entirely (`theme::backdrop`'s own module header
        // says so), read the one way a text token can be, by name, off
        // the diagnostics side of the theme. Read ONLY while the source
        // says IMAGE — a path a theme happens to carry while showing
        // SOLID is not a wallpaper this page has any business opening on.
        self.backdrop_image = if word("backdrop.source").as_deref() == Some("image") {
            nacelle::theme::diagnostics()
                .text("backdrop.image")
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty())
        } else {
            None
        };
        self.backdrop_touched = false;

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
        // TEXT: the ladder's hue seed read like the surface's — OWN when it
        // has been cut loose, FOLLOW when it still resolves to the accent.
        let t_hue = px("text.hue").rem_euclid(360.0);
        self.text_own_hue = (t_hue - a_hue).abs() > 0.5;
        self.text_hue = t_hue.round().clamp(0.0, 359.0) as u32;
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
        // BASIC's named scale, off the same three numbers the tracks took
        // — one read, so the control and the ladder can never disagree
        // about what the theme said.
        self.corner_seed = [self.corner_sm, self.corner_md, self.corner_lg];
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
        // MENU and TOOLTIP: only their FILLS are their own now (the border
        // and text are the one BORDER and one TEXT picker's), and a fill
        // keeps its own alpha beside the sliders, because the model passes
        // the channel through. The border WIDTHS stay per-object.
        if let Some(c) = col_of("component.menu.fill") {
            seed(&mut self.menu_fill, c);
            self.menu_fill_a = c.a;
        }
        self.menu_edge_w = scale_back(px("menu.border") / unit, 1.0);
        if let Some(c) = col_of("component.tooltip.fill") {
            seed(&mut self.tip_fill, c);
            self.tip_fill_a = c.a;
        }
        self.tip_edge_w = scale_back(px("tooltip.border") / unit, 1.0);
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
        // re-seeds the other. BASIC's move comes back to rest here,
        // which is what makes CANCEL and the door leave BASIC showing
        // "the theme as it stands" rather than a move already made.
        self.seed_tone_from_theme();
        // AND THE THIRTEEN PICKERS CATCH UP WITH THE TRACKS above. Last,
        // because they read what those lines have just written: a picker
        // seeded before its track would open on the previous theme, and
        // the first press on a swatch would then write that stale colour
        // back over the new one.
        self.seed_pickers_from_tracks();
    }

    /// A theme's name may be its file's name, nothing more.
    fn theme_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    }

    /// What the wallpaper path prompt accepts a keystroke of. Wide open
    /// compared to [`Self::theme_name_char`] on purpose — a path is a
    /// filesystem's own business, not this program's, and the one thing
    /// that would actually corrupt the saved theme is a literal `"`
    /// closing the token's quoted string early. Control characters are
    /// refused too: none is legal in a path this program can open, and a
    /// stray one pasted from elsewhere is never what a hand meant to
    /// type.
    fn wallpaper_path_char(c: char) -> bool {
        !c.is_control() && c != '"'
    }

    /// Spells a TEXT token's value the way this file's own strings are
    /// written: a quoted literal, `"` and `\` escaped — the two bytes
    /// that would otherwise end it early or change what the next one
    /// means. `backdrop.image`'s own contract is "quoted; the bytes ARE
    /// the value" (`default.theme`, `[backdrop]`), so nothing here
    /// interprets the path — it only keeps the quote that carries it
    /// intact.
    fn quote_theme_text(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            if c == '"' || c == '\\' {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    }

    /// The theme the editor is EDITING — the file the preview on screen is
    /// standing on — or `None` when that is the master, which is no file.
    ///
    /// A save patches this theme's own bytes, so naming it is how SAVE AS
    /// stays a copy of what the person is looking at.
    fn editor_source_theme(&self) -> Option<String> {
        Self::editor_source_of(
            config::current_engine_theme().as_deref(),
            &nacelle::theme::available_themes(),
        )
    }

    /// The rule behind [`Self::editor_source_theme`], asked as a question
    /// about two lists instead of about two process-wide stores — the
    /// configuration file and the theme search path — so it can be read and
    /// tested without steering a process.
    ///
    /// The three ways there is nothing to copy are the three ways there is
    /// no file: nothing configured, the master (which is compiled in), and
    /// a `theme:` naming a file that no longer exists — for which the
    /// loader ALREADY fell back to the master, so the person is looking at
    /// the master whatever the line says. `Act::EditorSave` makes the same
    /// test for the same reason when it decides that SAVE is really SAVE
    /// AS: what is in force, not what is written down.
    fn editor_source_of(current: Option<&str>, known: &[String]) -> Option<String> {
        let name = current?;
        if name.eq_ignore_ascii_case("default") {
            return None;
        }
        known.iter().find(|n| n.eq_ignore_ascii_case(name)).cloned()
    }

    /// Writes the edit set under `name` and, when the write lands, makes
    /// the saved theme the one in force. Answers whether the
    /// configuration changed — the caller's cue to re-resolve, which is
    /// what reloads the theme off the file just written.
    ///
    /// `name` is where it LANDS; [`Self::editor_source_theme`] is what is
    /// being saved. They are one for SAVE and two for SAVE AS, and passing
    /// only the first is how a SAVE AS onto a taken name came out wearing
    /// that name's halo instead of the one on screen — the set is silent
    /// about a dress the theme wore itself, and a silence is answered by
    /// whichever file the save is laid against.
    fn editor_save_named(&mut self, name: &str) -> bool {
        let source = self.editor_source_theme();
        match nacelle::theme::save_theme_as(source.as_deref(), name, &self.editor_edits()) {
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

    /// Mouse move while a track is held — a slider's, the open list's
    /// thumb, or the page's.
    pub fn drag(&mut self, x: f32, y: f32) {
        // While the SAVE AS prompt stands, nothing under it moves —
        // `click`'s own guard states the reason (the prompt is
        // keyboard-shaped, and a slider that fell through would drag the
        // theme mid-naming); a drag that was ALREADY held when the
        // prompt opened by keyboard is the one road that guard alone
        // does not close, since it never sees a fresh press to refuse
        // (2026-08-28's fix).
        if self.naming.is_some() {
            return;
        }
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
        // The page's thumb, on the same terms: only the y matters, and a
        // hand that wandered off the lane sideways is still holding it.
        // Asked for at the HOVER width, because that is the width it was
        // grabbed at — the track's length is all `drag` reads of it, so
        // the two agree wherever the pointer has got to.
        if self.scroll.dragging() {
            let look = ScrollbarLook::from_theme();
            let (area, viewport, content) =
                (self.flow.view, self.flow.view.h, self.flow.length);
            if let Some(geom) =
                scroll::scrollbar(area, &look, self.scroll.offset(), viewport, content, true)
            {
                self.scroll.drag(y, viewport, content, geom.track);
            }
            return;
        }
        // And the RAIL's thumb, on the page thumb's terms exactly: the
        // column scrolls, so its bar takes the hand the same way. Only
        // the frame it aims with differs ([`RailFrame`]).
        if self.rail_scroll.dragging() {
            if let Some(rail) = self.rail_flow {
                let look = ScrollbarLook::from_theme();
                let (viewport, content) = (rail.flow.view.h, rail.flow.length);
                if let Some(geom) = scroll::scrollbar(
                    rail.flow.view,
                    &look,
                    self.rail_scroll.offset(),
                    viewport,
                    content,
                    true,
                ) {
                    self.rail_scroll.drag(y, viewport, content, geom.track);
                }
            }
            return;
        }
        let Some(act) = self.dragging else { return };
        // A dragged slider follows the same pulse as the tracks below: a
        // colour dragged across the bank re-bakes the desktop on the
        // theme's pulse, not on every frame.
        if let Act::PickerSlider(..) = act {
            self.set_picker_from(act, x);
            if self.preview_pulse_due() {
                self.apply_editor_preview();
            }
            return;
        }
        self.set_from_x(act, x);
        self.mark_dirty(act);
        // The editor's tracks show themselves WHILE dragged — the owner asked
        // for the picture to follow the hand, not the release.
        if let Act::EditorTrack(_) = act {
            if self.preview_pulse_due() {
                self.apply_editor_preview();
            }
        }
    }

    /// Whether a control held under the hand may re-bake the desktop
    /// behind it yet, and books the pulse if it may.
    ///
    /// THE RATE IS `settings.preview_pulse_ms` AND NOT A NUMBER HERE. How
    /// fast the picture follows the hand is something a person sees, so
    /// it is the theme's; and it is ONE reader because two controls hold
    /// this pulse — the editor's sliders and the picker's two areas —
    /// and they were two copies of `100` in this file until 2026-08-18,
    /// which is one copy away from the day they disagree.
    ///
    /// It lives in `[settings]` and not in `[motion]` although it is a
    /// time. `[motion]` is a CLOSED catalogue — eighteen effects, eight
    /// keys each, two globals, counted by a test in the toolkit — and
    /// this is not an effect. It is a rate limiter, and it must not be
    /// multiplied by `motion.scale`: reduced motion sets that to zero,
    /// which for an animation means "show the end state at once" and for
    /// a limiter would mean "re-bake every frame" — 76 KB a bake, none
    /// of it freed, ~4.5 MB for a second of dragging. Reduced motion is
    /// a promise about movement, not about heat.
    fn preview_pulse_due(&mut self) -> bool {
        static PULSE: OnceLock<TokenId> = OnceLock::new();
        let ms = theme::resolved().px(tok(&PULSE, "settings.preview_pulse_ms")).max(0.0) as u128;
        if self.editor_pulse.map_or(true, |t| t.elapsed().as_millis() >= ms) {
            self.editor_pulse = Some(Instant::now());
            true
        } else {
            false
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
        // The two thumbs, symmetric with their grabs: nothing about the
        // configuration changes when a scrollbar is let go. Both are
        // released unconditionally — a `release` with no grab behind it
        // is a no-op in the model, and asking which one was held would
        // be a third copy of a state the model already keeps.
        self.list_scroll.release();
        self.scroll.release();
        self.rail_scroll.release();
        let Some(act) = self.dragging.take() else { return false };
        if let Some(&Ctrl::Slider { save, .. }) = slider_of(act) {
            save(self);
        }
        matches!(act, Act::SizeTrack(_))
    }

    /// Opens the window where the rail opens it: on LOOK AND FEEL, the
    /// first section. There is no menu to land on any more, and landing
    /// on a section means landing on a section's PAGE — so this is the
    /// same road [`Act::OpenSets`] takes, scan of the three directories
    /// included, and not a bare `go`. (It was `Act::OpenLookFeel`'s road
    /// until 2026-08-18, when that act stopped being a door and became
    /// the fold — the SECTION's own page is what opens a section now,
    /// and it is the same page it always was.)
    ///
    /// The rail comes up SHUT, which [`Settings::opening`] is the one
    /// writer of.
    pub fn show(&mut self) {
        self.opening();
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
        self.opening();
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
        // A closed window draws nothing this drag's rect could still be
        // read against on reopen — leaving it armed would let the next
        // `opening()` inherit a press from a session ago.
        self.dragging = None;
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
                if bar_band(area, &look).contains(x, y) {
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
        // THE PAGE'S OWN BAR, on the list bar's terms exactly. It was
        // drawn and never asked: [`draw_bar`] built a geometry, put
        // it on screen and threw it away, so a thumb the eye reads as
        // draggable took no hand at all and the whole bar was an
        // indicator. The frame the bar was drawn against is the one the
        // flow already keeps for the page keys ([`Flow`]), so the press
        // aims with that and no second copy of the geometry is kept.
        //
        // Only while no list is unfolded, which is the rule the rest of
        // the window already answers by ([`Settings::button_state`]: with
        // an open dropdown only its items react to the mouse). The list
        // is drawn OVER the page, its own bar included, so a press in
        // this lane while it stands belongs to whatever the list put
        // there — the branch above, or one of its rows.
        //
        // NOTHING IS TAKEN FROM A LANE THAT HAS NO BAR IN IT, and the two
        // ways that could happen are both already answered. A page that
        // fits gets no geometry at all (`scroll::scrollbar` answers None
        // on `content <= viewport` and on `mode = none`), so the press
        // falls straight through to the rows. And a bar hidden by
        // `scrollbar.auto_hide` is a bar AT REST — [`draw_bar`] reads
        // its hover off `bar_band` OF THIS SAME `self.flow.view` and
        // draws at full alpha whenever the pointer stands in it, so the
        // pointer cannot be in this lane and the lane be empty at once.
        if self.dropdown.is_none() {
            let look = ScrollbarLook::from_theme();
            let (area, viewport, content) =
                (self.flow.view, self.flow.view.h, self.flow.length);
            if bar_band(area, &look).contains(x, y) {
                if let Some(geom) = scroll::scrollbar(
                    area,
                    &look,
                    self.scroll.offset(),
                    viewport,
                    content,
                    true,
                ) {
                    // Beside the thumb: one viewport toward the click,
                    // the toolkit's own word on a track press. The press
                    // is taken either way — an overlay bar lies ON TOP of
                    // the rows, and letting it through would move a
                    // slider the hand never aimed at.
                    if !self.scroll.press_thumb(y, geom.thumb) {
                        self.scroll.page(y >= geom.thumb.bottom(), viewport, self.now);
                    }
                    return false;
                }
            }
            // AND THE RAIL'S, which is the same paragraph again with the
            // navigation column's frame in it. Its lane is inside the
            // rail's own room and cannot overlap the page's, so the order
            // of the two branches decides nothing; they are two because
            // the frames are two.
            if let Some(rail) = self.rail_flow {
                let (area, viewport, content) =
                    (rail.flow.view, rail.flow.view.h, rail.flow.length);
                if bar_band(area, &look).contains(x, y) {
                    if let Some(geom) = scroll::scrollbar(
                        area,
                        &look,
                        self.rail_scroll.offset(),
                        viewport,
                        content,
                        true,
                    ) {
                        if !self.rail_scroll.press_thumb(y, geom.thumb) {
                            self.rail_scroll.page(
                                y >= geom.thumb.bottom(),
                                viewport,
                                self.now,
                            );
                        }
                        return false;
                    }
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
            // No element hit: a click on blank space is "elsewhere" too —
            // it blurs (commits) an open inline editor exactly as a press
            // on another control would (`Settings::perform`'s own head
            // guard handles every act that IS one; this is the one gap
            // that guard cannot close, since there is no act here at
            // all) — and then swallows the click; a click inside the
            // window closes an open dropdown.
            self.blur_editing_picker();
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

    /// Every sound an activation makes leaves the window through here.
    ///
    /// ONE PRESS, ONE SOUND is a statement about the whole of
    /// [`Settings::perform`] and there was no way to put it TO the
    /// window: `nacelle::sound::emit` shouts into a queue shared by the
    /// entire process, so a count taken from it during a test run is a
    /// count of whatever else the run was doing at the time. So the
    /// window keeps its own log of what it said, under `cfg(test)`, and
    /// the test reads that instead of the queue. Nothing about the
    /// shipping build changes: the log does not exist in it.
    fn say(&mut self, e: nacelle::sound::Event) {
        #[cfg(test)]
        self.heard.push(e);
        nacelle::sound::emit(e);
    }

    /// The body every activation runs — mouse ([`Settings::click`])
    /// and keyboard ([`Settings::key`]) share it, so the two ways of
    /// pressing a control cannot drift apart (F1 §1.5). `x` is where
    /// along a slider track the press landed; buttons ignore it.
    fn perform(&mut self, act: Act, x: f32) -> bool {
        // ANY PRESS ELSEWHERE BLURS AN OPEN INLINE EDITOR FIRST —
        // including a different picker's own value plate — the one
        // funnel every activation runs through (this function's own
        // head note), so `Settings::click`'s hit-dispatch, `Settings::
        // key`'s Enter/Space path and a direct test call all answer the
        // same way. Pressing the SAME picker's own plate again is the one
        // exception: that act IS `Act::PickerText(id)` and leaves the
        // editor exactly where it stands.
        if let Some(id) = self.editing_picker {
            if act != Act::PickerText(id) {
                self.blur_editing_picker();
            }
        }
        self.flash = Some((act, self.now));
        #[cfg(test)]
        self.heard.clear();
        // Every button clicks; the actions below that mean more than a
        // plain press replace it with their own sound.
        use nacelle::sound::Event as Sfx;
        match act {
            Act::Close | Act::Back => {}
            Act::ToggleSnap | Act::ToggleTyping | Act::ToggleAmbient => {}
            // The editor's switches speak toggle, like every other switch.
            Act::EditorFlip(_) => {}
            // And COLOR's, which is a switch whatever it goes on to
            // write: a press that clicked AND toggled would be the only
            // one in the window that made two sounds.
            Act::ColorHdr => {}
            Act::VolumeTrack => {}
            Act::Pick(..) => {}
            // THE PICKER'S TWO GRIDS, for the same reason `Pick` is here:
            // a ready-made colour moves the theme's live preview, so it
            // speaks `Theme` in its own arm below. The other five picker
            // acts are NOT on this list, and that is the whole of the
            // fix: they were not on it before either, but three of them
            // emitted a sound of their own anyway, so pressing the
            // notation plate or the bank cell made two clicks and
            // pressing a ready-made colour made a click and a theme.
            Act::PickerBase(..) | Act::PickerCustom(..) => {}
            _ => self.say(Sfx::Click),
        }
        match act {
            Act::Close => {
                // Closing the window does not pass through `go`, so the
                // editor's preview is dropped here as well — and so is a
                // held drag, for the same reason `close` itself drops one
                // (2026-08-28's fix).
                self.leave_editor_preview();
                self.open = false;
                self.dragging = None;
                self.say(Sfx::PanelClose);
            }
            Act::EditorCancel => {
                nacelle::theme::clear_preview();
                self.editor_pulse = None;
                self.seed_editor_from_theme();
                // The seed above puts every picker back on the theme's
                // own colour; a drag still armed on one would immediately
                // move it again off the very value Cancel just restored.
                self.dragging = None;
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
                // BASIC and ADVANCED draw an entirely different set of
                // pickers at entirely different rects; a drag armed on
                // one side has nothing of itself left to answer to on
                // the other (2026-08-28's fix).
                self.dragging = None;
            }
            // BASIC's CORNER SIZE: step to the next word of the master's
            // own scale and put every radius on the theme's number for
            // that step. Nothing about a look is decided here — the three
            // numbers come from `corner_seed`, read off the theme.
            //
            // CUSTOM (ADVANCED's three tracks left the radii off every
            // step) steps to AS WRITTEN, which is the only word that can
            // be reached from anywhere and the only one that undoes a
            // press without a second guess about which step was meant.
            Act::EditorCornerStep => {
                let now = corner_step_word(self);
                let next = CORNER_STEPS
                    .iter()
                    .position(|w| *w == now)
                    .map_or(0, |i| (i + 1) % CORNER_STEPS.len());
                let [sm, md, lg] = match next {
                    0 => self.corner_seed,
                    i => [self.corner_seed[i - 1]; 3],
                };
                self.corner_sm = sm;
                self.corner_md = md;
                self.corner_lg = lg;
                self.apply_editor_preview();
            }
            Act::EditorSaveAs => {
                use nacelle::object::text_input::{InputModel, Validator};
                self.naming_for = NamingFor::ThemeName;
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
                    self.naming_for = NamingFor::ThemeName;
                    self.naming = Some(
                        InputModel::new()
                            .with_validator(Validator::Charset(Self::theme_name_char))
                            .with_max_len(40),
                    );
                } else {
                    return self.editor_save_named(&name);
                }
            }
            // The BACKDROP's own door: the same InputModel SAVE AS opens,
            // asking the other question ([`NamingFor::WallpaperPath`]) and
            // pre-filled with whatever is chosen already — editing a path
            // is retyping it from nothing otherwise, which a SAVE AS
            // prompt never asks of a theme's own name either.
            Act::EditorWallpaperEdit => {
                use nacelle::object::text_input::{InputModel, Validator};
                let mut model = InputModel::new()
                    .with_validator(Validator::Charset(Self::wallpaper_path_char))
                    .with_max_len(1024);
                if let Some(path) = self.backdrop_image.as_deref() {
                    model.set_value(path);
                }
                self.naming_for = NamingFor::WallpaperPath;
                self.naming = Some(model);
            }
            // CLEAR drops the choice AND marks the backdrop touched in
            // the same press — the mark is what tells `editor_edits` this
            // is a hand asking for `solid`, not a page nobody has visited.
            Act::EditorWallpaperClear => {
                self.backdrop_image = None;
                self.backdrop_touched = true;
                self.apply_editor_preview();
            }
            Act::Back => {
                self.say(Sfx::Click);
                // The same answer Escape peels a layer by, so the two
                // ways out of a page cannot lead to different places. A
                // page with no layer above it wears CLOSE and never has
                // this act at all ([`chrome_of`]), so the fallback is
                // only ever the section the window opens on.
                self.go(parent_view(self.view).unwrap_or(View::LookFeel))
            }
            // THE SECTION TURNS ITS FOLD OVER AND STAYS WHERE IT IS;
            // the page under it is the door. Until 2026-08-18 the two
            // shared this arm — one page reached from two entries —
            // which is what left the rail's own entry with nothing to
            // do on the page it opens the window on
            // ([`Settings::toggle_rail`]).
            Act::OpenLookFeel => self.toggle_rail(Act::OpenLookFeel),
            Act::OpenSets => self.enter_look_feel(),
            Act::ListBtn(list) => {
                let d = Dropdown::List(list);
                self.dropdown = if self.dropdown == Some(d) {
                    None
                } else {
                    self.dropdown_since = Some(self.now);
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
                        ListId::Backgrounds => {
                            self.current_background = Some(name.clone());
                            self.apply_editor_preview();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        // The whole-theme lists follow the two above: a
                        // pick lays a value over the theme until SAVE,
                        // writes no config line, and must answer FALSE —
                        // true would reload the theme and erase the very
                        // preview the pick just sent (the border pick's
                        // verified bug, not to be re-made five more times).
                        ListId::Severities => {
                            // Choosing a role EDITS nothing: the picker
                            // re-aims at the role's stored colour, and only
                            // a press on the picker marks it touched.
                            self.current_severity = Some(name.clone());
                            self.seed_pickers_from_tracks();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        ListId::Corners => {
                            self.current_corner = Some(name.clone());
                            self.apply_editor_preview();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        ListId::RingStyles => {
                            self.current_ring_style = Some(name.clone());
                            self.apply_editor_preview();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        ListId::ScrollModes => {
                            self.current_scroll_mode = Some(name.clone());
                            self.apply_editor_preview();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        ListId::ScrollEdges => {
                            self.current_scroll_edge = Some(name.clone());
                            self.apply_editor_preview();
                            self.say(Sfx::Theme);
                            return false;
                        }
                        // Not a theme: it writes its config line at once,
                        // like the depth chips beside it, and answers
                        // FALSE because nothing about the THEME changed
                        // — true is main's word for "re-resolve the
                        // theme", and a swapchain is not a theme.
                        ListId::Spaces => {
                            self.set_space(&name);
                            config::set_color_space(&self.color_space);
                            self.color_dirty = true;
                            self.say(Sfx::Click);
                            return false;
                        }
                    }
                    self.refresh_current();
                    self.say(Sfx::Theme);
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
                // The page's closing sentence, on the way in with the
                // rest of its state: the row that shows it is drawn
                // every frame and may not ask the disk (`sound_set_note`).
                self.refresh_sound_set();
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
            // A DRAGGED SLIDER, one coordinate: the wheel's two-axis field
            // is gone, so every picker act that answers to a drag now
            // reads `x` alone, through `set_picker_from`.
            Act::PickerSlider(..) => {
                self.dragging = Some(act);
                self.set_picker_from(act, x);
                self.apply_editor_preview();
            }
            // The notation steps. Nothing about the COLOUR moves, so
            // there is no preview to show and no theme to disturb: this
            // is a change to how the value is spelled.
            // It takes the plain click from the head of this function
            // like any other button. It emitted a second one here until
            // 2026-08-18, which was audible: two clicks for one press.
            Act::PickerFormat(id) => self.pickers[id.idx()].cycle_format(),
            // Opens the inline editor over the value plate — a no-op if
            // this picker's own plate is already open, so a second press
            // on it does not restart the typed text. `Settings::perform`'s
            // own head guard has already blurred (committed) any OTHER
            // picker's editor before this arm ever runs.
            Act::PickerText(id) => {
                if self.editing_picker != Some(id) {
                    self.pickers[id.idx()].begin_edit();
                    self.editing_picker = Some(id);
                }
            }
            // A ready-made colour, in one press. `Base` reads the
            // THEME's grid and `Custom` the user's own, and both are
            // taken from the same lists the drawing and the hit map used.
            Act::PickerBase(id, i) => {
                if let Some(c) = nacelle::object::color_picker::base_colours().get(i) {
                    self.pickers[id.idx()].set_colour(*c);
                    self.commit_picker(id);
                    self.apply_editor_preview();
                    self.say(Sfx::Theme);
                }
            }
            Act::PickerCustom(id, i) => {
                if let Some(c) = self.picker_custom.get(i).copied() {
                    self.pickers[id.idx()].set_colour(c);
                    self.commit_picker(id);
                    self.apply_editor_preview();
                    self.say(Sfx::Theme);
                }
            }
            // The bank. A colour already in the row is not banked twice
            // — the grid is a set of places to come back to, and two
            // identical cells are one cell that wastes the other. The
            // press still CLICKS either way, from the head of this
            // function: whether the colour was new is answered by the
            // row growing or not, and a button that fell silent on the
            // second press would read as a button that missed it.
            Act::PickerAdd(id) => {
                let c = self.pickers[id.idx()].colour();
                if !self.picker_custom.iter().any(|k| *k == c) {
                    self.picker_custom.push(c);
                }
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
                self.say(if on { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::ToggleTyping => {
                self.sound_typing = !self.sound_typing;
                config::set_sound_typing(self.sound_typing);
                self.sound_dirty = true;
                self.say(if self.sound_typing { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::ToggleAmbient => {
                self.sound_ambient = !self.sound_ambient;
                config::set_sound_ambient(self.sound_ambient);
                self.sound_dirty = true;
                self.say(if self.sound_ambient { Sfx::ToggleOn } else { Sfx::ToggleOff });
            }
            Act::OpenColor => {
                if self.color_enabled {
                    self.seed_color(config::color_prefs());
                    self.color_luts = config::color_files("lut", &[".cube"]);
                    self.color_iccs = config::color_files("icc", &[".icc", ".icm"]);
                    self.go(View::Color);
                }
            }
            Act::ColorDepth(bits) => {
                self.set_depth(bits);
                config::set_color_depth(bits);
                self.color_dirty = true;
            }
            // The switch turns the window's state ([`Settings::flip_hdr`]
            // holds the whole decision); this writes what came of it. The
            // depth line is written only when the flip actually moved the
            // depth — a press that leaves it alone leaves the file alone
            // too, rather than writing out a number nobody chose.
            Act::ColorHdr => {
                let depth_moved = self.flip_hdr();
                config::set_color_space(&self.color_space);
                if depth_moved {
                    config::set_color_depth(self.color_depth);
                }
                self.color_dirty = true;
                self.say(if self.color_hdr { Sfx::ToggleOn } else { Sfx::ToggleOff });
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
                self.say(if self.grid_snap { Sfx::ToggleOn } else { Sfx::ToggleOff });
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
                    self.dropdown_since = Some(self.now);
                    self.list_scroll.reset();
                    Some(Dropdown::Family(sect))
                };
            }
            Act::WeightBtn(sect) => {
                self.dropdown = if self.dropdown == Some(Dropdown::Weight(sect)) {
                    None
                } else {
                    self.dropdown_since = Some(self.now);
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
        // The SAVE-AS-shaped prompt owns the keyboard while it stands:
        // Enter commits, Esc closes, everything else is the field's.
        // Purely keyboard-driven — the field needs no focus bookkeeping
        // because nothing else can hear a key while it is open. What
        // Enter DOES with the text, and which charset a paste is filtered
        // through, is [`NamingFor`]'s one branch in here.
        if self.naming.is_some() {
            use nacelle::object::text_input::{self, InputEdited, InputMsg};
            if ev.mods == Mods::NONE && ev.key == FKey::Escape {
                self.naming = None;
                return KeyOut::Consumed;
            }
            if ev.mods == Mods::NONE && ev.key == FKey::Enter {
                let text = self
                    .naming
                    .as_ref()
                    .map(|m| m.value().trim().to_string())
                    .unwrap_or_default();
                return match self.naming_for {
                    NamingFor::ThemeName => {
                        if text.is_empty() {
                            return KeyOut::Consumed;
                        }
                        if self.editor_save_named(&text) {
                            KeyOut::Changed
                        } else {
                            KeyOut::Consumed
                        }
                    }
                    // Nothing reaches disk here — same as every other
                    // control on the page, this only sets the field and
                    // previews it; SAVE / SAVE AS is what writes a file.
                    // An empty field is a CLEAR said with the keyboard
                    // rather than the button beside it.
                    NamingFor::WallpaperPath => {
                        self.naming = None;
                        self.backdrop_image = (!text.is_empty()).then_some(text);
                        self.backdrop_touched = true;
                        self.apply_editor_preview();
                        KeyOut::Consumed
                    }
                };
            }
            if let Some(msg) = text_input::key_msg(ev) {
                let filter = match self.naming_for {
                    NamingFor::ThemeName => Self::theme_name_char,
                    NamingFor::WallpaperPath => Self::wallpaper_path_char,
                };
                let out = self.naming.as_mut().map(|m| m.apply(msg));
                match out {
                    Some(InputEdited::CopyRequest { text, .. }) => {
                        nacelle::clipboard::store(nacelle::clipboard::Board::Clipboard, &text);
                    }
                    Some(InputEdited::PasteRequest) => {
                        if let Some(text) =
                            nacelle::clipboard::load(nacelle::clipboard::Board::Clipboard)
                        {
                            let text: String =
                                text.chars().filter(|&c| filter(c)).collect();
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
        // A PICKER'S INLINE EDITOR OWNS THE KEYBOARD WHILE IT STANDS, THE
        // SAME WAY THE SAVE AS PROMPT DOES ABOVE — Enter commits, Escape
        // cancels, everything else is the field's — but it does NOT claim
        // the mouse the way `naming` does (`draw_naming`'s own
        // `ctx.mouse.cover(win)`): this editor is a plate INSIDE the page
        // the owner explicitly asked to keep working, so a click
        // elsewhere (`Settings::perform`'s and `Settings::click`'s own
        // blur guards) is what closes it, never a scrim.
        if let Some(id) = self.editing_picker {
            use nacelle::object::text_input::{self, InputEdited, InputMsg};
            if ev.mods == Mods::NONE && ev.key == FKey::Escape {
                self.pickers[id.idx()].cancel_edit();
                self.editing_picker = None;
                return KeyOut::Consumed;
            }
            if ev.mods == Mods::NONE && ev.key == FKey::Enter {
                if self.pickers[id.idx()].commit_edit() {
                    self.editing_picker = None;
                    self.commit_picker(id);
                    self.apply_editor_preview();
                    return KeyOut::Changed;
                }
                // A bad parse STAYS OPEN, text untouched — `commit_edit`'s
                // own contract, and the SAVE AS prompt's own
                // `if name.is_empty()` guard above, read the other way
                // round: there, an empty name leaves ITS field open
                // rather than discarding what was typed.
                return KeyOut::Consumed;
            }
            if let Some(msg) = text_input::key_msg(ev) {
                let out = self.pickers[id.idx()].editing_mut().map(|m| m.apply(msg));
                match out {
                    Some(InputEdited::CopyRequest { text, .. }) => {
                        nacelle::clipboard::store(nacelle::clipboard::Board::Clipboard, &text);
                    }
                    Some(InputEdited::PasteRequest) => {
                        // No charset filter, unlike the SAVE AS prompt's
                        // own paste: a colour's own parser is already
                        // forgiving about punctuation (`color_picker::
                        // parse`'s own doc), and a live filter here would
                        // reject perfectly normal in-progress typing.
                        if let Some(text) =
                            nacelle::clipboard::load(nacelle::clipboard::Board::Clipboard)
                        {
                            if let Some(m) = self.pickers[id.idx()].editing_mut() {
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
                // AND NEITHER HAS A PICKER SLIDER, for the same reason
                // written larger: a synthetic press at the centre of a
                // track would set that one channel to whatever happens to
                // stand at its own midpoint, which is a colour nobody
                // chose — the old two-axis field's reasoning, unchanged
                // by the axis count. The swatches are how this control is
                // used from the keyboard today.
                if let Act::PickerSlider(..) = act {
                    return KeyOut::Consumed;
                }
                // The CENTRE of whatever the chain is standing on: a
                // keyboard press has no point of its own, and the middle
                // of the target is the only honest answer for a control
                // that reads one (a picker slider; the tracks take the
                // arrows instead and never arrive here).
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
                self.chase_focus(fc);
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
    /// What no scroll carries is not chased, and the frame said which
    /// that is rather than the geometry being asked to guess
    /// ([`Carrier`]): the corner button, a pinned bar and the rows of an
    /// open list all stand over a lane that does not move with them —
    /// chasing them would carry a panel off under something that had not
    /// moved.
    ///
    /// TWO SCROLLS, AND THE LEDGERS SAY WHICH. A rail entry is brought
    /// back by the RAIL's offset and a page row by the page's; the
    /// window folded has no rail at all, and its entries are in
    /// `flowed` with everything else, which is exactly when they scroll
    /// with everything else.
    fn chase_focus(&mut self, fc: &FocusCtl) {
        let Some(id) = fc.focused() else { return };
        // Read before anything is moved: `rail_flow` is the LAST
        // COMPLETED frame's, the same rule the page's `flow` follows.
        let carried = if self.flowed.contains(&id) {
            Some((Carrier::Page, self.flow))
        } else if self.railed.contains(&id) {
            self.rail_flow.map(|r| (Carrier::Rail, r.flow))
        } else {
            None
        };
        let Some((by, flow)) = carried else { return };
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
        let at = flow.offset + travel;
        match by {
            Carrier::Rail => self.rail_scroll.set_offset(at),
            _ => self.scroll.set_offset(at),
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
        self.refresh_sound_set();
    }

    /// The sentence SOUND LEVELS closes with, asked of the disk HERE
    /// and nowhere else — see [`sound_set_note`] for why that matters.
    ///
    /// Two questions in one, and the second is why the name alone will
    /// not do: `sounds:` says which set was chosen, and only the asset
    /// roots say whether it is installed. A desktop that is silent
    /// because the set it names is not there has to be able to say so.
    fn refresh_sound_set(&mut self) {
        self.sound_set = match config::active_sounds_dir() {
            Some(dir) => format!(
                "SET: {}",
                dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_uppercase()
            ),
            None => "NO SOUND SET SELECTED".to_string(),
        };
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
        // The frame clock, taken before anything is drawn ON it. Every
        // animation this window runs now dates from `Ctx.t` — the
        // press flash, the list's unfold, the scroll's settle — so the
        // chrome and the body cannot be a frame apart about what time
        // it is. ([`Settings::draw_body`] sets it again, for the reader
        // who arrives there first.)
        self.now = ctx.t;
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
        // The panels are cut BEFORE the corner button is placed, because
        // the corner button is the head of the rail and stands inside the
        // rail's own air ([`Panes`]).
        let nav = Panes::of(m, content);
        let corner = nav.corner;
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
            fc.register(
                focus_id(chrome_act),
                corner,
                Caps::NONE,
                AccessInfo::new(Role::Button, chrome_label),
            )
            .ring
        });
        // The navigation comes next, and the page after it: the chain is
        // corner button, rail, the section's pages, the page. Where the
        // window has folded the columns are empty and the same entries
        // are the first bands of the flow instead — same order, one
        // shape fewer.
        //
        // The beds first, under everything: one bed under both
        // navigation columns where the window has them, none where it
        // has folded.
        self.draw_bands(ctx, &nav);
        self.draw_nav(ctx, m, &nav);
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
        // Which question is standing decides the title, the field's
        // placeholder, the focus path and the hint's verb — the four
        // words this modal wears for its two callers ([`NamingFor`]).
        let (title, placeholder, focus_path, hint_text) = match self.naming_for {
            NamingFor::ThemeName => {
                ("SAVE THEME AS", "theme name", "settings.editor.naming", "ENTER SAVES \u{2014} ESC CANCELS")
            }
            NamingFor::WallpaperPath => (
                "WALLPAPER IMAGE PATH",
                "path to an image file",
                "settings.editor.backdrop.naming",
                "ENTER SETS \u{2014} ESC CANCELS",
            ),
        };
        ctx.dl.module_title(
            ctx.fonts,
            bx + pad,
            by + pad,
            bw - 2.0 * pad,
            title_px,
            title,
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
                FocusId::of(focus_path),
                &InputStyle {
                    placeholder,
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
            hint_text,
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
        // The WHOLE split and not the page column alone: the body starts
        // under the chrome button, and where that button stands is the
        // rail's business ([`body_top`]).
        let nav = Panes::of(m, content);
        let box_ = nav.page;
        // The box is the FULL one — the clip and the bar are drawn on it
        // — but what a pinned band COSTS is measured where its rows
        // stand, which is beside the bar's lane ([`rows_box`]).
        let rows = rows_box(box_);
        let top = body_top(page, m, &nav);
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
    /// navigation stands in its column, outside the scroll. Folded, the
    /// rail comes FIRST, as an ordinary band ahead of the page: one
    /// list, one scroll, and the same order the column registers in. The
    /// section's pages ride along inside that band, because they are
    /// rows of the rail's own table ([`Ctrl::Expander`]) and no longer a
    /// second one to remember here.
    fn frame_zones(&self, page: &'static Page, nav: &Panes) -> Vec<&'static Zone> {
        let mut out: Vec<&'static Zone> = Vec::with_capacity(page.zones.len() + 1);
        if nav.folded {
            out.push(&RAIL_ZONE);
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
        for (region, rows) in &regions {
            out.push(y);
            y += self.rows_h(rows, m, *region) + m.gap;
        }
        out
    }

    /// How tall one run of rows stands: every page's own arithmetic
    /// since there were pages, asked of a band's rows instead of a
    /// page's. The last row's trailing gap is not content.
    fn rows_h(&self, rows: &'static [Row], m: Metrics, region: Rect) -> f32 {
        let (h, trailing) = self.rows_span(rows, m, region);
        (h - trailing).max(0.0)
    }

    /// [`Settings::rows_h`] before the last gap is taken off: the run's
    /// whole reach, and how much of it that gap was.
    ///
    /// Split out because an unfolded section makes the two answers come
    /// apart. The gap after the LAST thing in the run is the last PAGE's
    /// and not the section's, so a reader that only carried a total
    /// could not know which to subtract — and this walker has to hand
    /// exactly what [`Settings::draw_rows`] returns, or a section's
    /// pages would be one gap taller for the scroll than for the eye.
    fn rows_span(&self, rows: &'static [Row], m: Metrics, region: Rect) -> (f32, f32) {
        let mut h = 0.0;
        let mut trailing = 0.0;
        for row in rows {
            if !(row.when)(self) {
                continue;
            }
            h += self.row_h(&row.ctrl, m, region) + m.space(row.after);
            trailing = m.space(row.after);
            // An unfolded section is as tall as itself PLUS its pages,
            // measured in the box they are drawn in ([`indent_region`]).
            // A section standing SHUT costs nothing at all, which is the
            // same sentence the walker says by not recursing — one rule,
            // said twice because this file has always measured and drawn
            // with two readers.
            //
            // A DISABLED SECTION IS SHUT, whatever the view says. R6
            // takes the row out of the frame's offering, and
            // [`Settings::draw_rows`] therefore lays no pages under it;
            // a height that counted them would reserve room for a run
            // nothing draws, and the two readers would be measuring two
            // different rails.
            if let (true, Ctrl::Expander { act, kids, .. }) =
                ((row.enabled)(self), row.ctrl)
            {
                if self.rail_open(act) {
                    let (kids_h, kids_gap) =
                        self.rows_span(kids, m, indent_region(region));
                    h += kids_h;
                    trailing = kids_gap;
                }
            }
        }
        (h, trailing)
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
            .map(|((region, rows), dy)| dy + self.rows_h(rows, m, region))
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
        let nav = Panes::of(m, content);
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
        let nav = Panes::of(m, content);
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
            self.draw_zone(ctx, zone, m, rows_box, y, Some(view), Carrier::Page);
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
            self.draw_zone(ctx, zone, m, rows_box, anchor - zh, None, Carrier::Still);
            anchor -= zh + m.gap;
        }
        draw_bar(ctx, &self.scroll, view, length);
    }

    /// The two columns' beds: the navigation's, and the page's.
    ///
    /// Nothing here decides what those colours are. TWO COLUMNS, TWO
    /// NAMES: `component.settings.rail_fill` and `.page_fill`, both the
    /// master's, so a theme re-beds either on its own. Naming a rung
    /// here instead would weld the settings columns to the desktop field
    /// and no theme could ever part them again.
    ///
    /// THERE WAS A THIRD, and it went with the column it bedded
    /// (2026-08-18). `.sub_fill` was the bed of the second navigation
    /// column, which the owner had already asked to be one colour with
    /// the rail — and a section's pages stand UNDER their section now,
    /// on the rail's own bed, so there is nothing left for a second name
    /// to paint.
    ///
    /// The master writes the rail off ONE ANCHOR — the window body,
    /// lifted once by `settings.band_lift` — which is what keeps the two
    /// together when a theme, BASIC or the editor's BACKGROUND section
    /// moves the body. That is the master's arrangement and not this
    /// function's: what stands here is a name, a box and a paint.
    ///
    /// Painted BEFORE the navigation and the body, and over each
    /// column's whole rectangle exactly as [`Panes`] cut it — top edge
    /// of the content box to bottom edge, the corner button included,
    /// because the button lies ON the rail and not in a notch out of it.
    /// The beds are the ground everything else in the window stands on.
    ///
    /// FOLDED, there are no columns at all. Below `settings.col_min_w`
    /// the rail's rows become the first band of one vertical flow, so
    /// `rail` is nothing and the page is the whole interior.
    ///
    /// A BAND WHOSE COLOUR IS THE SENTINEL IS NOT PAINTED, which is how
    /// the master ships the page's: `none` answers `color()` with alpha
    /// 0, the toolkit's own spelling of "there is nothing to draw here"
    /// (`libnacelle/tests/sentinel_none_colour.rs`), and the page's bed
    /// is already on the screen — the WINDOW BODY, laid by
    /// `window::frame` over that whole area. Two things make a second
    /// coat of it wrong rather than merely redundant, and both are why
    /// this is a sentinel and not a comparison of two colours:
    ///
    /// * ALPHA COMPOSES TWICE. `component.panel.fill` is translucent
    ///   (`@surface.panel`, alpha 0.82). Over the field the window
    ///   stands on, the body's own pixel is #131E19 and the doubled one
    ///   #15201B, an OKLab dE of 0.0078 — plainly a lighter panel, and
    ///   larger on a theme whose backdrop is further from the rung.
    /// * GLASS. Where `elev.panel.glass.rank` lifts the body off its
    ///   fill altogether the body is a BLUR, and any opaque bed over it
    ///   is the end of the blur the BACKGROUND section just put there.
    ///
    /// A theme that wants the page bedded anyway says so by giving
    /// `page_fill` a colour, and then it is painted like the other two.
    ///
    /// AND THE BEDS WEAR THE THEME'S CORNER. `settings.band_corner` is
    /// the radius and `settings.band_corner_mode` the cut, both read
    /// through the toolkit's own pair so that SQUARE, ROUND and CHAMFER
    /// all answer here exactly as they answer everywhere else. All four
    /// corners take it, and that survived the bands growing to full
    /// height: they are laid inside `content_rect`, which holds
    /// `modal.pad` clear of the frame on every side one could meet and
    /// `modal.body_top` clear at the top, so no band corner reaches the
    /// modal's own edge — a band is a plate lying ON the body, not a
    /// piece cut out of it. The master carries the measurement and the
    /// one case that would change the answer.
    fn draw_bands(&self, ctx: &mut Ctx, nav: &Panes) {
        static RAIL_FILL: OnceLock<TokenId> = OnceLock::new();
        static PAGE_FILL: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        let bands = [
            (nav.rail.map(|c| c.bed), &RAIL_FILL, "component.settings.rail_fill"),
            (Some(nav.page), &PAGE_FILL, "component.settings.page_fill"),
        ];
        let mut sf = CtxSurface::new(ctx);
        let cut = nacelle::view::paint::corner_style(&mut sf, "settings.band_corner_mode");
        for (box_, cell, name) in bands {
            let Some(r) = box_ else { continue };
            let fill = col(th.color(tok(cell, name)));
            if fill.a <= 0.0 {
                continue;
            }
            // Asked per band, not once: `pill` and the rest of §5.0's
            // sentinels are words ABOUT THE BOX, and the two boxes are
            // two different shapes.
            let radius =
                nacelle::view::paint::corner_radius(&mut sf, "settings.band_corner", r, 1.0);
            sf.ring_fill(r, cut, radius, fill);
        }
    }

    /// The navigation column, where the window has not folded.
    ///
    /// It is clipped to its ROWS box and not to its bed — the bed is the
    /// paint, the rows box is the room, and the difference between them
    /// is the air `settings.band_pad_*` keeps. A rail longer than the
    /// room is cut off, not painted over the page, and no entry can
    /// bleed into the padding it is supposed to stand inside. It is
    /// walked by the SAME row walker the pages are, so an entry is a
    /// button, a heading is a heading and a disabled section is grey by
    /// exactly the rules a setting is — and a section's own pages are
    /// rows of the same table, one indent in ([`Ctrl::Expander`]).
    ///
    /// Drawn before the body, so the chain runs corner button, rail
    /// (a section's pages inside it, where they stand), page — reading
    /// order, and the same order the folded window registers them in.
    ///
    /// AND IT SCROLLS, on the page's own terms: the toolkit's offset,
    /// the toolkit's physics and the toolkit's bar, its own instance of
    /// each ([`Settings::rail_scroll`]). Nothing here is a second
    /// mechanism — what a column that carries the open section's pages
    /// needed was the mechanism the page already had, pointed at the
    /// other box. The entries stand BESIDE the bar's lane
    /// ([`rows_box`]) while the clip, the span and the bar keep the
    /// whole room, which is the page's arrangement to the letter.
    fn draw_nav(&mut self, ctx: &mut Ctx, m: Metrics, nav: &Panes) {
        let Some(col_) = nav.rail else {
            // Folded: the entries are bands of the page's flow and the
            // page's ledger answers for them. A frame left over from
            // the last unfolded window would aim the wheel and the
            // press at a column that is not on the screen.
            self.rail_flow = None;
            self.railed.clear();
            return;
        };
        let box_ = col_.rows;
        let rows = rows_box(box_);
        let length = self.rows_h(&RAIL_ROWS, m.rail(), rows);
        self.rail_scroll.tick(
            ctx.t,
            box_.h,
            length,
            Snap::None,
            &ScrollPhysics::from_theme(),
        );
        let off = self.rail_scroll.offset();
        // What the wheel, the press and the chase read back between
        // frames ([`RailFrame`]).
        self.rail_flow = Some(RailFrame { bed: col_.bed, flow: Flow { view: box_, length, offset: off } });
        ctx.dl.push_clip(box_.x, box_.y, box_.w, box_.h);
        self.clip = Some(box_);
        self.railed.clear();
        self.draw_rows(
            ctx,
            &RAIL_ROWS,
            m.rail(),
            rows,
            box_.y - off,
            Some(box_),
            Carrier::Rail,
        );
        self.clip = None;
        ctx.dl.pop_clip();
        draw_bar(ctx, &self.rail_scroll, box_, length);
    }

    /// One band, at the top edge it was given. A flow lays its rows in
    /// the whole box; a columned band lays each column's rows in that
    /// column's box — beside one another from the one top edge, or, once
    /// the band has folded, one under the other down the whole width
    /// ([`Settings::zone_offsets`]).
    ///
    /// `cull` is the viewport a flowed band is held to; a pinned band
    /// passes `None`, because it stands outside the clip and is always
    /// on screen. `by` says WHICH scroll the band rides — the ledger
    /// the chase reads ([`Carrier`]).
    fn draw_zone(
        &mut self,
        ctx: &mut Ctx,
        zone: &'static Zone,
        m: Metrics,
        box_: Rect,
        top: f32,
        cull: Option<Rect>,
        by: Carrier,
    ) {
        let offsets = self.zone_offsets(zone, m, box_);
        for ((region, rows), dy) in zone_regions(zone, box_).into_iter().zip(offsets) {
            self.draw_rows(ctx, rows, m, region, top + dy, cull, by);
        }
    }

    /// One run of rows, from `top` downwards inside `region`.
    ///
    /// `region` is what the rows measure and align against — its x and
    /// width are the column's, its height the page's — and it is what
    /// each row is handed as its content box, so a slider in the left
    /// column ends at the left column's right edge and not at the page's.
    /// Returns where the run ended: the next free line, and how much of
    /// the way there was the last row's own trailing gap. An unfolded
    /// section needs both — the first to place the entries that follow
    /// its pages, the second to know where the pages themselves STOP, so
    /// the guide beside them is drawn from the numbers the rows were
    /// really laid with and not from a second copy of this arithmetic.
    fn draw_rows(
        &mut self,
        ctx: &mut Ctx,
        rows: &'static [Row],
        m: Metrics,
        region: Rect,
        top: f32,
        cull: Option<Rect>,
        by: Carrier,
    ) -> (f32, f32) {
        // Measured for THIS region, and from THESE rows: the sliders of
        // one column do not inherit the label width of the next (M3).
        let (label_w, value_w) = self.columns(ctx, rows, region.w);
        let mut y = top;
        let mut trailing = 0.0;
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
            // on screen or off it — AND NEITHER DO ITS PAGES, which is
            // why this is read once and asked twice. An expander is a
            // row like any other: a grey one offers no way in, so the
            // pages behind it are not a way in either, and a run drawn
            // under a section nothing can press would be four buttons
            // belonging to a heading that says they are unavailable.
            // [`Settings::rows_span`] leaves them out of the height for
            // the same reason and by the same test, so the measurement
            // and the picture stay one answer.
            let live = (row.enabled)(self);
            if !live {
                if on_screen {
                    self.draw_disabled(ctx, &row.ctrl, rc);
                }
            } else {
                // What the row offers, asked ONCE: the off-frame
                // registration places it, and a band a scroll carries
                // writes it into the ledger the chase reads.
                let targets = (by != Carrier::Still || !on_screen)
                    .then(|| self.targets(ctx, &row.ctrl, rc))
                    .unwrap_or_default();
                if on_screen {
                    self.draw_row(ctx, &row.ctrl, rc);
                } else {
                    self.register_offscreen(ctx, &row.ctrl, &targets);
                }
                let ledger = match by {
                    Carrier::Page => Some(&mut self.flowed),
                    Carrier::Rail => Some(&mut self.railed),
                    Carrier::Still => None,
                };
                if let Some(l) = ledger {
                    l.extend(targets.iter().map(|&(_, a)| focus_id(a)));
                }
            }
            y += h + m.space(row.after);
            trailing = m.space(row.after);
            // A SECTION'S OWN PAGES, WHERE THE SECTION IS THE ONE OPEN.
            // They are laid by this same walker in a region one
            // `settings.rail_indent` narrower, so the indent is the only
            // thing that makes them look nested and nothing about them
            // is a second kind of row.
            //
            // FOLDED SHUT THEY ARE NOT HERE AT ALL — not drawn, not
            // measured, not a target and not a step in the Tab order —
            // because the recursion simply does not happen. That is the
            // toolkit's own rule for a list that is not all the way out
            // (`object::dropdown::accordion`: an element joins the chain
            // only when the whole of it is standing), said here by the
            // one thing that can never disagree with the picture: the
            // walker that draws the picture.
            if let (true, Ctrl::Expander { act, kids, .. }) = (live, row.ctrl) {
                if self.rail_open(act) {
                    let inner = indent_region(region);
                    let (end, gap) = self.draw_rows(ctx, kids, m, inner, y, cull, by);
                    self.draw_rail_guide(ctx, region, y, end - gap);
                    y = end;
                    trailing = gap;
                }
            }
        }
        (y, trailing)
    }

    /// The hairline a section's unfolded pages are propped against, from
    /// the top of the first to the bottom of the last.
    ///
    /// WHY A LINE AT ALL, and why not a bed. The pages used to stand in
    /// a COLUMN, and a column says "these are a group" by having edges.
    /// Indent alone does not: four buttons a little further in read as
    /// four buttons that failed to line up. The line is the bracket that
    /// says which entry they belong to — and it is a line and not a bed
    /// because a bed of their own would be the second column again, in
    /// less room.
    ///
    /// Every number is the theme's: the stroke and its place across the
    /// step are `settings.rail_guide_w` and `settings.rail_guide_x`, the
    /// step itself is `settings.rail_indent`, the colour is
    /// `component.settings.rail_guide`. The two ENDS are the drawing's,
    /// handed in by the walker that laid the rows, so the line cannot
    /// stand beside a run of a different length.
    fn draw_rail_guide(&mut self, ctx: &mut Ctx, region: Rect, top: f32, bottom: f32) {
        static INK: OnceLock<TokenId> = OnceLock::new();
        let (x, w) = rail_guide_x(region);
        let h = bottom - top;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let ink = col(theme::resolved().color(tok(&INK, "component.settings.rail_guide")));
        if ink.a <= 0.0 {
            return;
        }
        ctx.dl.rect(x, top, w, h, ink);
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
            Ctrl::Chips { values, act, .. } => {
                let values = values(self);
                chip_rects(values.len(), rc)
                    .into_iter()
                    .zip(values.iter())
                    .map(|(r, bits)| (r, act(*bits)))
                    .collect()
            }
            Ctrl::Cycle { act, .. } => vec![(cycle_rect(rc), *act)],
            Ctrl::Drop { list } => {
                vec![(Self::button_rect(BtnKind::Wide, rc), Act::ListBtn(*list))]
            }
            // The section's own plate, and nothing else: its pages are
            // ROWS, and the walker asks each of them for itself.
            Ctrl::Button { kind, act, .. } | Ctrl::Expander { kind, act, .. } => {
                vec![(Self::button_rect(*kind, rc), *act)]
            }
            Ctrl::Bar { items } => self
                .bar_plates(ctx, items, rc)
                .into_iter()
                .map(|(r, _, act)| (r, act))
                .collect(),
            // The picker's parts, ASKED OF THE OBJECT: one enumeration
            // serves the hit map, the Tab chain and the drawing, so a
            // part that is painted is a part that can be pressed and
            // reached, and no third list can fall behind the other two.
            Ctrl::Picker(id) => self.picker_targets(*id, rc),
            Ctrl::Section { .. }
            | Ctrl::Note { .. }
            | Ctrl::Hint { .. }
            | Ctrl::Custom { .. } => Vec::new(),
        }
    }

    /// The parts of ONE picker, as rects and acts.
    ///
    /// Split out of [`Settings::targets`] so the drawing arm can ask for
    /// the same list without holding a `&'static Ctrl` it has no way to
    /// make: the id it needs is a value now, not a variant. Both callers
    /// go through here, so a part that is painted is still a part that
    /// can be pressed and reached, and no third list can fall behind.
    fn picker_targets(&self, id: PickerId, rc: RowCtx) -> Vec<(Rect, Act)> {
        let mut out: Vec<(Rect, Act)> = nacelle::object::color_picker::parts(&self.picker_layout(id, rc))
            .into_iter()
            .map(|(part, r)| (r, picker_act(id, part)))
            .collect();
        if let Some(r) = self.advanced_colour_button(id, rc) {
            out.push((r, Act::EditorMode));
        }
        out
    }

    /// Where the picker's parts stand in a row's band — ONE statement of
    /// it, so the drawing, the hit map and the Tab chain cannot disagree
    /// about where the slider bank is.
    ///
    /// BASIC's Tone row is the one exception: its band is narrowed by
    /// [`ADVANCED_BUTTON_FRAC`] so the picker's own notation strip stops
    /// short of the row's right edge, leaving room for the ADVANCED
    /// COLOUR button ([`Settings::advanced_colour_button`]) — moved here
    /// 2026-08-23 from a standalone row at the foot of the page.
    fn picker_layout(&self, id: PickerId, rc: RowCtx) -> nacelle::object::color_picker::Layout {
        let band = if id == PickerId::Tone && !editor_advanced(self) {
            let w = (rc.band.w * (1.0 - Self::ADVANCED_BUTTON_FRAC) - rc.m.gap).max(0.0);
            Rect::new(rc.band.x, rc.band.y, w, rc.band.h)
        } else {
            rc.band
        };
        nacelle::object::color_picker::layout(
            band,
            self.pickers[id.idx()].slider_count(),
            self.picker_custom.len(),
        )
    }

    /// The fraction of the Tone row's band the ADVANCED COLOUR button
    /// takes, once it is on that row at all — the button and the picker's
    /// narrowed strip both read this, so neither can leave the other a
    /// row too wide or too narrow for the space actually reserved.
    const ADVANCED_BUTTON_FRAC: f32 = 0.28;

    /// The ADVANCED COLOUR button's own rect: the end of the Tone row,
    /// the same height as the notation strip beside it (`picker_layout`'s
    /// narrowed [`Layout::text`]) — `None` everywhere else, including
    /// Tone's OWN row once ADVANCED is open, where the picker owns the
    /// whole band again and this button does not exist to press.
    fn advanced_colour_button(&self, id: PickerId, rc: RowCtx) -> Option<Rect> {
        if id != PickerId::Tone || editor_advanced(self) {
            return None;
        }
        let l = self.picker_layout(id, rc);
        let x = l.text.x + l.text.w + rc.m.gap;
        let w = (rc.band.x + rc.band.w - x).max(0.0);
        Some(Rect::new(x, l.text.y, w, l.text.h))
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
        // TODO(a11y): same placeholder-name gap as `hit_into` above —
        // `act`'s Debug form stands in for a curated label pending a
        // dedicated pass over this window's `Act` enum.
        let role = if matches!(ctrl, Ctrl::Slider { .. }) { Role::Slider } else { Role::Button };
        for &(r, act) in targets {
            if let Some(fc) = ctx.focus.as_deref_mut() {
                fc.register(focus_id(act), r, caps, AccessInfo::new(role, format!("{act:?}")));
            }
        }
    }

    /// A REGION's label and value columns, in px — THE MEASURING COLUMN,
    /// `rhythm.label_col = auto`.
    ///
    /// Asked once per band, and once per column of a columned band, OF
    /// THE BAND'S OWN ROWS: the widest label in the BLOCK, which is what
    /// the master's word means and what it could not have until a caller
    /// handed the rows over. A band that carries no label writes no
    /// column at all, so the rail's buttons and the boards' hint reserve
    /// nothing, exactly as they did.
    ///
    /// This retires the two rules that stood here — a fraction of the
    /// width, and a WORD WRITTEN OUT BY HAND for each page to measure
    /// against. The hand-written word is what the theme editor's page
    /// went without: it declared no columns at all, so its tracks began
    /// under their own labels and ended under their own numbers. A word
    /// beside a table is a second statement of what the table says, and
    /// the eighty-six rows of the editor's ADVANCED page are eighty-six
    /// chances for the two to disagree in silence.
    ///
    /// EVERY ROW THE BAND DECLARES is measured, standing or not
    /// ([`Row::when`]): a column that grew and shrank as a conditional
    /// row came and went would move every track beside it, which is the
    /// jump this measurement exists to prevent.
    ///
    /// `w` is the region's width, so a share is a share of the column and
    /// not of the whole content box.
    fn columns(&self, ctx: &mut Ctx, rows: &[Row], w: f32) -> (f32, f32) {
        let th = theme::resolved();
        let f = role_label(ctx);
        let v = role_value(ctx);
        // The widest word each column has to hold. `None` — nothing in
        // the band writes there — is the empty column, and it must stay
        // empty: a floor applied to nothing would indent a page of
        // buttons by a label that does not exist.
        let mut label = None::<f32>;
        let mut value = None::<f32>;
        // The face's widest digit, once for the band: it is an answer
        // about the face and the size, and both are fixed for the whole
        // of this loop ([`widest_digit`]). Asked lazily, so a band with
        // no number in it rasterises nothing at all.
        let mut widest = None::<char>;
        for row in rows {
            if let Some(text) = row.ctrl.column_label() {
                let seen = ctx.fonts.measure(f.face, f.px, text, f.track);
                label = Some(label.map_or(seen, |had: f32| had.max(seen)));
            }
            if let Some(text) = row.ctrl.column_value() {
                let d = *widest.get_or_insert_with(|| widest_digit(ctx, &v));
                let seen = widest_run(ctx, &v, &text, d);
                value = Some(value.map_or(seen, |had: f32| had.max(seen)));
            }
        }
        // What the theme asks for, where it asks for something else. The
        // sentinel is negative (§5.0's table, `auto` -> -1.0); a share
        // bakes to a fraction and a length to device px, and under one
        // device pixel a number can only be the share.
        let want = |cell: &'static OnceLock<TokenId>, name: &'static str, measured: f32| {
            let asked = th.px(tok(cell, name));
            if asked < 0.0 {
                measured
            } else if asked < 1.0 {
                w * asked
            } else {
                asked
            }
        };
        let label_w = label.map_or(0.0, |measured| {
            let col = want(&LABEL_COL, "rhythm.label_col", measured);
            // The floor first and the ceiling second, so a region too
            // narrow for the minimum gives the label the share it is
            // allowed and not the whole of itself.
            (col.max(th.px(tok(&LABEL_MIN, "rhythm.label_min")))
                .min(w * th.px(tok(&LABEL_MAX, "rhythm.label_max")))
                + th.px(tok(&LABEL_PAD, "rhythm.label_pad")))
            .max(0.0)
        });
        let value_w = value.map_or(0.0, |measured| {
            want(&VALUE_COL, "rhythm.value_col", measured)
                + th.px(tok(&VALUE_GUTTER, "rhythm.value_gutter"))
        });
        // Whatever the two of them asked for, a track keeps a width: the
        // value gives way, because the label is the row's subject and the
        // number is a reading of it.
        (label_w, value_w.min((w - label_w).max(0.0)))
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
            // A bar is ONE row however many verbs it carries — and so is
            // an expander: what stands UNDER it is rows of its own, laid
            // and measured by the walkers, never a taller row here.
            Ctrl::Button { .. }
            | Ctrl::Expander { .. }
            | Ctrl::Drop { .. }
            | Ctrl::Bar { .. } => m.btn_h,
            Ctrl::Section { .. } => m.block_h,
            Ctrl::Note { .. } => m.note_h,
            Ctrl::Hint { .. } => m.hint_h,
            Ctrl::Custom { h, .. } => h(m, content),
            // Asked of the object, at the width it will be drawn in and
            // with the count that decides its last row: the control is a
            // block whose height depends on how many rows of swatches its
            // grids come to, and only the object knows that.
            Ctrl::Picker(_) => {
                nacelle::object::color_picker::height(content.w, self.picker_custom.len())
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
                self.draw_chips(ctx, label, values(self), *get, *act, rc)
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
            // The same plate, and the triangle that says there is more
            // under it. The TREE grammar and not the DROP one: a drop's
            // caret points at where a list will unfold OVER the page,
            // and these pages unfold INSIDE the row's own column, which
            // is the sentence every file tree ever drawn already speaks
            // (`view::paint::Disclosure`).
            Ctrl::Expander { label, kind, act, .. } => {
                let r = Self::button_rect(*kind, rc);
                let text = self.text_of(*label);
                self.button(ctx, r, &text, *act);
                self.expander_arrow(ctx, r, *act);
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
            // The picker draws itself; this row says WHERE and registers
            // what the hand may reach. The rects come from the same
            // `picker_layout` the targets do, so a part that is drawn is a
            // part that can be pressed and the two cannot come apart.
            // `nacelle::object::color_picker::draw_focusable` now takes
            // `&mut Picker` — `text_input::draw` mutates its own
            // `InputModel` even while only drawing (blink phase, scroll,
            // its measure cache), and that model lives on the picker
            // since the slider-bank rewrite (2026-08-24). So every `&self`
            // computation this arm needs (the layout, the ADVANCED COLOUR
            // button's rect, the hit targets) is hoisted BEFORE the
            // `&mut self.pickers[id.idx()]` borrow below — the mechanical
            // consequence that rewrite's own report named ahead of time.
            Ctrl::Picker(id) => {
                let l = self.picker_layout(*id, rc);
                let adv = self.advanced_colour_button(*id, rc);
                let targets = self.picker_targets(*id, rc);
                nacelle::object::color_picker::draw_focusable(
                    ctx,
                    &l,
                    &mut self.pickers[id.idx()],
                    &self.picker_custom,
                    |part| focus_id(picker_act(*id, part)),
                );
                if let Some(r) = adv {
                    let text = self.text_of(Text::Fixed("ADVANCED COLOUR"));
                    self.button(ctx, r, &text, Act::EditorMode);
                }
                for (r, act) in targets {
                    self.push_hit(r, act);
                }
            }
        }
    }

    /// A row the page turned off. Only the two kinds of row made of
    /// buttons have a disabled form — the ladder's Disabled rung, an
    /// inscription, and nothing in the hit map or the focus chain (R6).
    fn draw_disabled(&mut self, ctx: &mut Ctx, ctrl: &Ctrl, rc: RowCtx) {
        let plates: Vec<(Rect, Cow<'static, str>)> = match ctrl {
            Ctrl::Button { label, kind, .. } | Ctrl::Expander { label, kind, .. } => {
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
            // The same cap an ENABLED plate would carry. This form draws
            // its own inscription rather than going through
            // `button::draw` — it wants the Disabled rung and no plate —
            // so it has to ask the object what the label says, or a
            // theme's case transform would apply to a row the page has
            // turned on and not to the same row turned off.
            let cap = nacelle::object::button::cap_of(&s);
            ctx.dl.text_center(
                ctx.fonts,
                f.face,
                f.px,
                r.cx(),
                ty,
                &cap,
                col(st.text),
                f.track,
            );
        }
    }

    /// A row's label in the label column, written once for the three
    /// kinds of row that have one — a track, a set of segments and a
    /// cycler (settings.row_label_role). The same three
    /// [`Ctrl::column_label`] names, because the word is written HERE at
    /// `rc.content.x` and the column that keeps the control off it is
    /// reserved THERE.
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
    ///
    /// The plate's own width comes from the OBJECT
    /// ([`nacelle::object::button::plate_w`]) and not from this file's
    /// reading of the object's keys. It used to be spelled out here —
    /// `fonts.measure(...) + 2 * button.pad_x`, floored at
    /// `button.min_w` — which was a private copy of the button's rule
    /// and drifted from it the moment `button::draw` learned to apply
    /// `type.<button.role>.case`: the plate was sized on the label as
    /// this file spells it, and the cap was drawn in the case the theme
    /// asks for. Under a master saying `upper` and a source full of
    /// capitals the two agree by luck. Asking the object is how they go
    /// on agreeing when either of those stops being true.
    fn bar_plates(
        &self,
        ctx: &mut Ctx,
        items: &'static [(Text, Act)],
        rc: RowCtx,
    ) -> Vec<(Rect, Cow<'static, str>, Act)> {
        static BAR_GAP: OnceLock<TokenId> = OnceLock::new();
        let gap = theme::resolved().px(tok(&BAR_GAP, "settings.bar_gap"));
        // Resolved before anything is drawn: a label may be read from
        // the window, and the window cannot be borrowed while it draws.
        let labels: Vec<Cow<'static, str>> =
            items.iter().map(|(t, _)| self.text_of(*t)).collect();
        let widths: Vec<f32> = labels
            .iter()
            .map(|s| nacelle::object::button::plate_w(ctx, s))
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
        // THE PLATE'S RING, CUT THE WAY THE PAGE IS CUT. It was
        // `rect_outline` until 2026-08-18 — four straight bars, square
        // whatever the theme said — so the MODE row was the one row of
        // the editor whose corners did not match the buttons above and
        // below it (owner's report 3). No theme could have said
        // otherwise: `[cycler]` stated a border weight and no shape at
        // all. The keys are its own now and the master points both at
        // the button's, so a theme that pins its buttons moves this row
        // with them; the CUT comes from the toolkit's one corner
        // dictionary (`nacelle::corner`) and not from a word spelled
        // here, and `Corner::sized` is what makes `pill` a length on
        // this box rather than a negative sentinel compared against
        // zero.
        static CYC_RADIUS: OnceLock<TokenId> = OnceLock::new();
        static CYC_MODE: OnceLock<TokenId> = OnceLock::new();
        static CYC_CUTS: OnceLock<nacelle::corner::Cuts> = OnceLock::new();
        static CYC_SEG: OnceLock<TokenId> = OnceLock::new();
        let cut = nacelle::draw::Corner::sized(
            nacelle::corner::style(th, tok(&CYC_MODE, "cycler.corner_style"), &CYC_CUTS),
            th.px(tok(&CYC_RADIUS, "cycler.corner")),
            r,
        );
        ctx.dl.ring(
            r,
            &[cut; 4],
            nacelle::corner::segments(th, &CYC_SEG, cut.size),
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

    /// What this compositor said it can be asked for, from the
    /// application — `None` when there is nobody to ask.
    ///
    /// The report decides whether the switch is on the page at all
    /// ([`hdr_possible`]), so learning it is a way of stranding a window
    /// that is standing on the high range: SPACE HDR over a list holding
    /// nothing but "auto", and no switch anywhere to turn it back. The
    /// standing name therefore goes back through [`Settings::set_space`]
    /// — the ONE writer, which settles the side, applies that rule and
    /// rebuilds the offer — instead of the offer being rebuilt behind
    /// the window's back. Told twice, or told what it already knew, it
    /// lands on exactly the same state.
    pub fn set_supported_spaces(&mut self, names: Option<Vec<String>>) {
        self.color_supported = names;
        let standing = self.color_space.clone();
        self.set_space(&standing);
    }

    /// A depth has been ASKED of the swapchain, and with it the last
    /// measurement stops being about anything.
    ///
    /// THE ONE WRITER OF THE WISH, and it exists for the clearing and
    /// not for the assignment. The renderer moves its format at the
    /// rebuild, which happens inside the next frame's `render` — so
    /// between the request and that rebuild the window is holding a NEW
    /// wish beside an OLD measurement, and [`color_depth_fell_short`]
    /// reads exactly that pair. Ask for sixteen on a machine that had
    /// been given ten and the page would say "16 asked, 10 in the
    /// swapchain — the surface offers no more" about a swapchain that
    /// had not been asked yet; a page that is not redrawn again would
    /// leave the sentence standing. Zeroing the measurement says the
    /// only true thing there is to say in that gap: not measured.
    ///
    /// Only when the number actually moves, because a request that
    /// leaves the depth alone (the SPACE list, the LUT — `apply_color!`
    /// carries all of them at once) rebuilds nothing, and blinking a
    /// standing line off and on again would be its own small lie.
    pub fn color_asked(&mut self, depth: u32) {
        if self.color_depth_asked != depth {
            self.color_depth_now = 0;
        }
        self.color_depth_asked = depth;
    }

    /// What the swapchain GAVE, read off the renderer after a frame has
    /// been drawn — which is when the rebuild has happened and the
    /// number describes the picture that was just put on the screen.
    pub fn color_measured(&mut self, bits: u32) {
        self.color_depth_now = bits;
    }

    /// What came of the last request, in the compositor's own terms.
    pub fn color_answered(&mut self, status: String) {
        self.color_status = status;
    }

    /// Whether the compositor said it can be asked for this name.
    fn space_offered(&self, name: &str) -> bool {
        match &self.color_supported {
            None => true,
            Some(list) => list.iter().any(|n| n == name),
        }
    }

    /// The ONE writer of the chosen colour space.
    ///
    /// It sets the name, settles which half of the table is on offer,
    /// re-derives the offer itself and remembers the name on its own
    /// side of the switch. Four things that have to agree, written in
    /// one place so they cannot come apart: a list whose members no
    /// longer hold the standing name draws no mark at all
    /// ([`Settings::current_row`] answers `None`, honestly), and the
    /// user would be looking at a set with nothing standing in it.
    fn set_space(&mut self, name: &str) {
        self.color_space = name.to_string();
        // "auto" stands in BOTH offers, so it says nothing about which
        // one is showing and the switch is left exactly where it is —
        // picking "auto" out of the high-range list must not throw the
        // page back to the other half under the pointer. Every other
        // name settles the question by being what it is.
        let range = config::space_range(&self.color_space);
        if range != config::SpaceRange::Either {
            self.color_hdr = range == config::SpaceRange::Hdr;
        }
        // And never onto a side there is no way back from. A file
        // written on other hardware can name `bt2020 pq` to a compositor
        // that cannot be asked for it; the switch is not on the page
        // then ([`hdr_possible`]), so a window standing on the high
        // range would be showing SPACE HDR over a list holding nothing
        // but "auto", with no control anywhere to turn it back. It
        // stands on the standard range instead, and the space the file
        // names stands in no list — which is the truth: this machine is
        // not showing it.
        self.color_hdr &= hdr_possible(self);
        if self.color_hdr {
            self.last_hdr = Some(self.color_space.clone());
        } else {
            self.last_sdr = Some(self.color_space.clone());
        }
        self.rebuild_spaces();
    }

    /// The names the SPACE list offers: the half of the table the switch
    /// is showing, less whatever this compositor cannot be asked for.
    ///
    /// A space the configuration names but the machine has no answer for
    /// is NOT smuggled back in. That is the same doctrine the theme
    /// lists follow — a look nobody has installed is a look with no mark
    /// on it — and the alternative would be a row that says "in force"
    /// about a picture the screen is not showing.
    fn rebuild_spaces(&mut self) {
        self.color_spaces = config::color_spaces(self.color_hdr)
            .into_iter()
            .filter(|n| self.space_offered(n))
            .map(String::from)
            .collect();
    }

    /// The chosen depth, and the whole of that state: a depth the user
    /// pressed is theirs, so it cancels the memo the HDR switch keeps of
    /// what it raised.
    fn set_depth(&mut self, bits: u32) {
        self.color_depth = bits;
        self.depth_before_hdr = None;
    }

    /// Turns the HDR switch, and answers whether the colour depth moved
    /// with it. Everything the window knows about high range is here.
    ///
    /// THE SWITCH PERSISTS NOTHING OF ITS OWN. There is no `hdr` field
    /// in the configuration and there must not be: the file would then
    /// be able to say `hdr: true` and `space: "srgb"` in one breath, the
    /// cascade merges the two fields independently, and nothing in
    /// `ColorConf` could rule on the contradiction. What it writes is a
    /// colour space — which is also the only thing `wl_color` ever reads
    /// — and "HDR is on" is read back off that name.
    ///
    /// ON: the high-range space this window last stood in, or else the
    /// first one on offer. The table's order puts `bt2020 pq` first for
    /// a reason: ST 2084 is the display's own transfer function, HLG is
    /// a broadcast curve carried for completeness, and scRGB linear is a
    /// compositing space rather than something to ask a monitor for.
    ///
    /// OFF: the standard-range space this window last stood in, or else
    /// "auto" — truthful for a window that has stood in none, because
    /// the file never said otherwise.
    ///
    /// And the depth. Eight-bit PQ bands visibly, the page has no way to
    /// warn (it carries no warning control at all), so the eight is
    /// taken off the offer and the depth comes up to ten with it. What
    /// was replaced is remembered and given back on the way out — but
    /// only what THIS took: pressing a depth clears the memo
    /// ([`Settings::set_depth`]), so a twelve the user chose while HDR
    /// was on stays twelve when HDR goes off.
    fn flip_hdr(&mut self) -> bool {
        // The list is a list of the other half from here on, and an open
        // one is showing rows that are about to stop existing. Fold it,
        // and put its scroll back to the head: an offset measured
        // against the old members means nothing against the new.
        self.dropdown = None;
        self.list_scroll.reset();

        let want_hdr = !self.color_hdr;
        let remembered = if want_hdr { &self.last_hdr } else { &self.last_sdr };
        let name = remembered
            .as_deref()
            .filter(|n| {
                config::space_range(n).in_offer(want_hdr) && self.space_offered(n)
            })
            .map(String::from)
            .or_else(|| {
                config::color_spaces(want_hdr)
                    .into_iter()
                    .find(|n| self.space_offered(n))
                    .map(String::from)
            })
            // The standard-range half always holds "auto", and the
            // switch is not on screen at all unless the high-range half
            // holds something, so this is unreachable — and stated
            // rather than unwrapped, because "no space at all" has a
            // right answer and it is the one the file means.
            .unwrap_or_else(|| config::model::ColorConf::SPACE.to_string());

        self.color_hdr = want_hdr;
        self.set_space(&name);

        let floor = depth_values(self).first().copied().unwrap_or(self.color_depth);
        if self.color_hdr {
            if self.color_depth < floor {
                self.depth_before_hdr = Some(self.color_depth);
                self.color_depth = floor;
                return true;
            }
        } else if let Some(bits) = self.depth_before_hdr.take() {
            self.color_depth = bits;
            return true;
        }
        false
    }

    /// Seeds the COLOR page from the configuration.
    ///
    /// The switch is READ OFF the space and stored nowhere. The one
    /// thing the file cannot say is which side "auto" belongs to — it
    /// belongs to both — so a page opened on "auto" opens on the
    /// standard-range side, which is what the file means: nobody asked
    /// for high range.
    ///
    /// A fresh reading is also a fresh memory. What the window
    /// remembered about the other side of the switch belonged to the
    /// visit that made it, and the depth memo belongs to the flip that
    /// took the depth.
    ///
    /// A file naming a space this machine cannot show does not strand
    /// the page on a side with no switch to leave by: [`Settings::
    /// set_space`] holds that rule, for every writer at once.
    fn seed_color(&mut self, prefs: config::ColorPrefs) {
        self.color_lut = prefs.lut;
        self.color_icc = prefs.icc;
        self.last_sdr = None;
        self.last_hdr = None;
        self.depth_before_hdr = None;
        self.color_hdr =
            config::space_range(&prefs.space) == config::SpaceRange::Hdr;
        self.set_space(&prefs.space);
        // THE PAGE OPENS ON A MEMBER OF THE OFFER IT OPENS WITH. The
        // depth and the space are two lines of a file and one statement
        // (`ColorConf::depth`), and this is the second reader of that
        // pair: a page seeded with a depth below the floor of the side
        // it lands on would draw a DEPTH row with nothing marked in it
        // and no way to mark anything — the missing number cannot be
        // pressed, because it is not on the screen to press.
        //
        // No memo is left behind. `depth_before_hdr` is what the SWITCH
        // took and owes back, and no switch was turned here; the raised
        // depth is what the file's own pair means, so turning HDR off
        // afterwards has nothing to give back.
        let floor = depth_values(self).first().copied().unwrap_or(prefs.depth);
        self.color_depth = prefs.depth.max(floor);
    }

    /// The names of one list, in the order they are offered.
    fn names(&self, list: ListId) -> &[String] {
        match list {
            ListId::Looks => &self.themes,
            ListId::Layauts => &self.layauts,
            ListId::Sounds => &self.sounds,
            ListId::Backgrounds => &self.background_kinds,
            ListId::Severities => &self.severity_kinds,
            ListId::Corners => &self.corner_kinds,
            ListId::RingStyles => &self.ring_style_kinds,
            ListId::ScrollModes => &self.scroll_mode_kinds,
            ListId::ScrollEdges => &self.scroll_edge_kinds,
            ListId::Spaces => &self.color_spaces,
        }
    }

    /// The name the configuration carries for one list.
    fn current_of(&self, list: ListId) -> Option<&String> {
        match list {
            ListId::Looks => self.current_look.as_ref(),
            ListId::Layauts => self.current_layaut.as_ref(),
            ListId::Sounds => self.current_sounds.as_ref(),
            ListId::Backgrounds => self.current_background.as_ref(),
            ListId::Severities => self.current_severity.as_ref(),
            ListId::Corners => self.current_corner.as_ref(),
            ListId::RingStyles => self.current_ring_style.as_ref(),
            ListId::ScrollModes => self.current_scroll_mode.as_ref(),
            ListId::ScrollEdges => self.current_scroll_edge.as_ref(),
            // Always a name, never nothing: `ColorConf::space` answers
            // "auto" for anything it cannot read, so the window has a
            // space in hand from the moment it opens the page. Whether
            // that name is on OFFER is a different question, and the
            // one `current_row` goes on to ask.
            ListId::Spaces => Some(&self.color_space),
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
        let word = list.label(self);
        self.button(ctx, r, word, act);
        self.caret(ctx, r, act);
    }

    /// The triangle at the tail of an anchor: the toolkit's own
    /// disclosure glyph ([`nacelle::view::paint::disclosure`]) in its
    /// DROP grammar — closed it points down, at the direction the list
    /// will unfold, and open it points back up at the edge the list
    /// folds into. The tree's grammar (closed points along the row) is
    /// the other sentence the same primitive speaks, and this window
    /// speaks it too — on the rail, where a section's pages take their
    /// place IN the column ([`Settings::expander_arrow`]). A `▷` here
    /// would read as "go into this row", which is that other sentence.
    /// The state turns the GLYPH and not its colour, which is the
    /// primitive's rule and not this window's.
    fn caret(&mut self, ctx: &mut Ctx, r: Rect, act: Act) {
        let open = self.dropdown.map_or(false, |d| anchor_act(d) == act);
        self.disclosure(ctx, r, act, nacelle::view::paint::Disclosure::Drop, open);
    }

    /// The triangle on a rail entry that HAS PAGES ([`Ctrl::Expander`]),
    /// in the toolkit's TREE grammar: shut it points along the row at
    /// what opening would reveal, open it points down at the pages it
    /// just revealed.
    ///
    /// THE OTHER GRAMMAR WOULD BE THE WRONG SENTENCE, and the two are a
    /// paragraph apart in the primitive that speaks both. A drop-down's
    /// caret announces where a list will unfold — over the page, from
    /// the anchor's bottom edge — and these pages do not unfold over
    /// anything: they take their place IN the column, under the entry
    /// they belong to, which is what a tree row does and what a tree
    /// row's triangle has always said.
    ///
    /// Only an expander is ever asked, so the arrow cannot appear on an
    /// entry with nothing behind it: [`Ctrl::Expander`] is the only kind
    /// with a `kids` field to put pages in, and the only kind this is
    /// drawn for (owner's mock-up §3 — an arrow on every entry would be
    /// half of them lying).
    fn expander_arrow(&mut self, ctx: &mut Ctx, r: Rect, act: Act) {
        let open = self.rail_open(act);
        self.disclosure(ctx, r, act, nacelle::view::paint::Disclosure::Tree, open);
    }

    /// A disclosure triangle at the tail of a plate, in whichever of the
    /// primitive's two grammars the caller is speaking.
    ///
    /// Sized and inked like the BACK arrow at the other end of a button
    /// — `button.icon_size` glyph, `button.pad_x` from the edge, the
    /// ladder's own text colour — because a glyph on a button is a glyph
    /// on a button, wherever it stands and whatever it means.
    fn disclosure(
        &mut self,
        ctx: &mut Ctx,
        r: Rect,
        act: Act,
        kind: nacelle::view::paint::Disclosure,
        open: bool,
    ) {
        static ICON_SIZE: OnceLock<TokenId> = OnceLock::new();
        static ICON_MIN: OnceLock<TokenId> = OnceLock::new();
        static PAD_X: OnceLock<TokenId> = OnceLock::new();
        let th = theme::resolved();
        let s = th
            .px(tok(&ICON_SIZE, "button.icon_size"))
            .max(th.px(tok(&ICON_MIN, "button.icon_size_min_px")));
        let pad = th.px(tok(&PAD_X, "button.pad_x"));
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
            kind,
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
        // Nothing about the standing row's dress is stated here: the
        // wash, the ring's colour, its width and the label's brightness
        // all come off the `menu.item` class's ladder inside the object.
        let style = nacelle::object::dropdown::AccordionStyle {
            focus: Some(base),
            current,
            ..Default::default()
        };
        // THE UNFOLD IS THE TOOLKIT'S. `accordion_at` asks
        // `motion.menu_unfold` itself, so the duration, the easing WORD,
        // the global `motion.scale` and the effect's own `enabled` flag
        // are all the theme's. What stood here was a private clock with
        // `ease_out` written into Rust beside it, honouring exactly one
        // of the four: a theme that wrote `ease_in_out`, switched the
        // unfold off, or asked for reduced motion moved this list not at
        // all. The object's own documentation named this call site as
        // the one to migrate.
        let rows = match self.dropdown_since {
            Some(opened) => nacelle::object::dropdown::accordion_at(
                ctx,
                anchor,
                item_h,
                names,
                opened,
                &style,
                &mut self.list_scroll,
            ),
            // A list standing at rest — the object's documented entry
            // for a caller that already has its progress.
            None => nacelle::object::dropdown::accordion(
                ctx,
                anchor,
                item_h,
                names,
                1.0,
                &style,
                &mut self.list_scroll,
            ),
        };
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
        // restatement [`draw_bar`] makes for the page's own bar).
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

    /// Whether an act is a navigation entry standing for the view in
    /// force: its SECTION, and, where the section unfolds pages, the
    /// PAGE among them. Both are true in one frame, of two different
    /// buttons — which is the mock-up's §4 in one line. An unfolded
    /// section that did not stay marked would leave the reader looking
    /// at a list of four entries with no way to tell which of them the
    /// page on the right belongs to; a page that did not mark itself
    /// would leave the section marked and the page anonymous.
    fn nav_marks(&self, act: Act) -> bool {
        act == rail_act(self.view) || kid_act(self.view) == Some(act)
    }

    /// Whether a section of the rail stands UNFOLDED — its pages drawn
    /// under it, in the focus chain and in the hit map.
    ///
    /// IT IS A FIELD SINCE 2026-08-18 ([`Settings::unfolded`]), and what
    /// it replaced is worth keeping because the replacement is only
    /// legible against it. It used to read `act == rail_act(self.view)`:
    /// the unfold was not a state at all but a second way of saying
    /// which page was in force. The owner reported the two symptoms that
    /// come out of that as one fault — the rail came up OPEN (the window
    /// opens on LOOK AND FEEL, so its section was by definition the open
    /// one) and pressing LOOK AND FEEL did NOTHING (the press went to a
    /// page already in force, and there was no state that could have
    /// answered differently). Its own argument for having no field —
    /// that a field would be a second statement of where the reader is,
    /// free to disagree with `self.view` — was answered by making it
    /// state something ELSE: this says what the reader asked to SEE
    /// LISTED, `self.view` says where they are, and the two were never
    /// the same sentence. `nav_marks` is still the only reader of "where
    /// am I", and it still reads `self.view`.
    ///
    /// THE THREE THINGS THE REPORT DID NOT SETTLE, settled:
    ///
    /// * (a) MAY TWO SECTIONS STAND OPEN AT ONCE? YES. The old text
    ///   forbade it, and its reason was true when it was written: a rail
    ///   that did not scroll could be outgrown by the sum of every
    ///   section's pages, hiding an entry with no way to reach it, so
    ///   single-open bounded the worst case at the deepest section. THE
    ///   RAIL SCROLLS NOW ([`Settings::rail_scroll`], and
    ///   `every_section_the_rail_holds_can_be_reached_at_every_window`
    ///   was rewritten from FITTING to REACHABILITY when it did) — the
    ///   bound buys nothing that the wheel does not already give, and
    ///   the price of keeping it would be a rail that shuts the section
    ///   you were reading because you asked to see a second one. That is
    ///   the same reason the old text gave for why GNOME's expander rows
    ///   may all stand open: they live in a scrolled page. So does this.
    /// * (b) WHAT BECOMES OF THE FOLDS WHEN THE READER GOES TO A PAGE IN
    ///   ANOTHER SECTION? NOTHING. This is the one answer the change
    ///   cannot dodge — the view is exactly what used to drive the fold,
    ///   so leaving the coupling in anywhere would be the reported fault
    ///   surviving in a corner. A fold is the reader's own request to
    ///   see a list; walking to another page is not a retraction of it,
    ///   and a rail that reshaped itself under the hand on every
    ///   navigation would lose a section the reader had just opened to
    ///   compare against. Nothing is hidden by leaving it open: the
    ///   entry for the page in force is marked wherever it stands
    ///   (`nav_marks`), which is what says where you are.
    /// * (c) DOES A FOLD SURVIVE THE WINDOW CLOSING? NO — the rail comes
    ///   up shut every time ([`Settings::opening`]). A fold is a view
    ///   state and not a preference: preferences in this program live in
    ///   the configuration file and are written through `config`, and
    ///   nothing here writes one. "Shut by default" would otherwise be
    ///   true exactly once per session, which is not what a default is.
    ///
    /// AND THE UNFOLD IS STILL NOT ANIMATED, which was the old (c) and
    /// is unchanged: while a blind moves it has to leave the focus chain
    /// and the hit map (`object::dropdown::accordion` — "a ring on a
    /// moving rect is the board-ride pitfall in miniature"), and a
    /// drop-down may go dead for a moment because it is a transient over
    /// one page, where the window's PERMANENT navigation may not.
    fn rail_open(&self, act: Act) -> bool {
        self.unfolded.contains(&act)
    }

    /// A section's entry was pressed: the fold turns over, and nothing
    /// else does.
    ///
    /// THE PRESS NO LONGER TRAVELS. It used to open the section's first
    /// page, and the unfold was what that looked like; now that the fold
    /// is a state of its own, a press that navigated as well would have
    /// to navigate on the way OUT too — shutting the list would carry
    /// the reader to a page they were shutting the list to get away
    /// from. The section's pages are the doors, and SETS is the first of
    /// them, standing where the section's own page always did.
    fn toggle_rail(&mut self, act: Act) {
        match self.unfolded.iter().position(|a| *a == act) {
            Some(i) => {
                self.unfolded.remove(i);
            }
            None => self.unfolded.push(act),
        }
    }

    /// The window is being opened. The rail comes up SHUT, whichever
    /// door was used ([`Settings::rail_open`], decision (c)).
    ///
    /// ONE WRITER, and it is on the way IN rather than on the way out
    /// because there are two doors in and three ways out — `Act::Close`,
    /// the layout editor's `Act::EditGrid` and [`Settings::close`] all
    /// lower the flag, and a rule spelled at three exits is a rule with
    /// two chances of being forgotten.
    fn opening(&mut self) {
        self.open = true;
        self.unfolded.clear();
        // A fresh open starts with nothing held — a drag left over from
        // however the window was last closed (a lost mouse-up, an
        // Alt-Tab mid-press) belongs to a session that is now over.
        self.dragging = None;
    }

    /// Whether an act's click flash is still decaying
    /// (`motion.press.duration_ms`). Split out of
    /// [`Settings::button_state`] because the door inside an open list
    /// wants the flash and NOT that method's hover rule.
    fn flashing(&self, act: Act) -> bool {
        // `one_shot_secs` is zero when the theme switches the press off
        // OR asks for reduced motion, and a decay with no length never
        // lights the button at all — the one-shot's end state IS an
        // unlit button, so freezing at it means never lighting. The
        // hand-rolled read this replaces asked for `duration_ms` alone,
        // so neither switch ever reached this window (the editor's own
        // buttons already honoured both, from the same tokens: the two
        // halves of one desktop disagreed about the same theme).
        let lit = nacelle::motion::Effect::of("press").one_shot_secs() as f64;
        // Clamped, not trusted: the frame clock is the caller's and a
        // pixel-guard run restarts it at zero mid-session.
        self.flash
            .map(|(a, t0)| a == act && (self.now - t0).max(0.0) < lit)
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
            // The anchor of the ONE space list, a name inside it and
            // the switch that turns it: three controls that stand on
            // the COLOR page together and must not share a chain
            // position.
            Act::ListBtn(ListId::Spaces),
            Act::Pick(ListId::Spaces, 0),
            Act::ColorHdr,
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
            Act::EditorWallpaperEdit,
            Act::EditorWallpaperClear,
            Act::EditorTrack(Knob::SurfHue),
            Act::EditorTrack(Knob::CornerSm),
            Act::EditorTrack(Knob::Hairline),
            Act::EditorTrack(Knob::RingW),
            Act::EditorTrack(Knob::HaloAlpha),
            Act::EditorTrack(Knob::UnfocusedDim),
            Act::EditorTrack(Knob::MenuEdgeW),
            Act::EditorTrack(Knob::TipEdgeW),
            Act::EditorTrack(Knob::BarW),
            // AND THE TWELVE PICKERS AGAINST EACH OTHER. Every part of
            // every picker is `<part> -> which picker -> which cell`, and
            // the pairs below are the ones a collision would be invisible
            // in: the same part of two different pickers, and the same
            // cell of two different pickers.
            Act::PickerSlider(PickerId::Tone, 0),
            Act::PickerSlider(PickerId::Accent, 0),
            Act::PickerSlider(PickerId::BarTrack, 0),
            Act::PickerSlider(PickerId::Tone, 1),
            Act::PickerSlider(PickerId::Accent, 1),
            Act::PickerFormat(PickerId::Tone),
            Act::PickerFormat(PickerId::MenuFill),
            Act::PickerText(PickerId::Tone),
            Act::PickerText(PickerId::Text),
            Act::PickerAdd(PickerId::Tone),
            Act::PickerAdd(PickerId::BgMain),
            Act::PickerBase(PickerId::Tone, 0),
            Act::PickerBase(PickerId::Tone, 1),
            Act::PickerBase(PickerId::Edge, 0),
            Act::PickerBase(PickerId::Edge, 1),
            Act::PickerCustom(PickerId::Tone, 0),
            Act::PickerCustom(PickerId::Severity, 0),
            Act::EditorFlip(Flip::SurfaceOwnHue),
            Act::EditorFlip(Flip::Ring),
            Act::EditorFlip(Flip::Halo),
            Act::EditorFlip(Flip::BarAutoHide),
            Act::EditorFlip(Flip::BarTrack),
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
        let mut s = furnished();
        assert!(ListId::Looks.carries_door(), "the THEMES list lost its door");
        for list in [ListId::Layauts, ListId::Sounds] {
            assert!(
                !list.carries_door(),
                "{} carries a door it has no editor for",
                list.label(&s)
            );
        }

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

    /// The page's three anchors are one column, and so is the run of
    /// pages the section unfolds on the rail.
    ///
    /// FONTS used to be a `Listed` button — `settings.list_w_frac` of
    /// the content, centred — under three anchors that ran the full
    /// width, and it read as a different class of control although it
    /// is the same kind of thing: another way into the same subject.
    /// It is a navigation entry now, so the rule it has to keep is the
    /// RAIL's, not the page's: everything a section unfolds shares one
    /// edge, everything on the page shares another, and no control of
    /// either straddles the two. The footer is deliberately in neither
    /// set: it is pinned, it is destructive, and looking unlike the
    /// page is its job.
    #[test]
    fn the_pages_choices_and_doors_stand_in_one_column() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        // The section is unfolded, because the entries this measures are
        // the ones it unfolds — a shut rail has no column of pages to
        // hold to a single edge.
        let mut s = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
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
        let levels = button_at(&LOOKFEEL_PAGES, Act::OpenSoundLevels)
            .expect("LOOK AND FEEL has no SOUND LEVELS page");
        let fonts_at = button_at(&LOOKFEEL_PAGES, Act::OpenFont)
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
            LOOKFEEL_PAGES[levels].ctrl
        else {
            panic!("the entry lost its fixed label")
        };
        let mut s = furnished();
        assert_ne!(
            word,
            ListId::Sounds.label(&s),
            "the entry and the list wear one word: a reader cannot tell the \
             set from the levels"
        );

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
            kid_act(View::SoundLevels) == Some(Act::OpenSoundLevels),
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
        // BACKGROUND is in the loop with the three file-backed lists on
        // purpose: its members are built in rather than found on disk, and
        // that is exactly the kind of difference that makes a list behave
        // subtly unlike its neighbours unless something checks.
        for list in [
            ListId::Looks,
            ListId::Layauts,
            ListId::Sounds,
            ListId::Backgrounds,
            ListId::Severities,
            ListId::Corners,
            ListId::RingStyles,
            ListId::ScrollModes,
            ListId::ScrollEdges,
            // And SPACE, whose members are neither on disk nor built
            // into this file: they are half of a table in `config`,
            // filtered by what the compositor said it can show. A third
            // kind of provenance is a third chance for the mark and the
            // pick to disagree.
            ListId::Spaces,
        ] {
            for i in 0..s.names(list).len() {
                let name = s.names(list)[i].clone();
                match list {
                    ListId::Looks => s.current_look = Some(name),
                    ListId::Layauts => s.current_layaut = Some(name),
                    ListId::Sounds => s.current_sounds = Some(name),
                    ListId::Backgrounds => s.current_background = Some(name),
                    ListId::Severities => s.current_severity = Some(name),
                    ListId::Corners => s.current_corner = Some(name),
                    ListId::RingStyles => s.current_ring_style = Some(name),
                    ListId::ScrollModes => s.current_scroll_mode = Some(name),
                    ListId::ScrollEdges => s.current_scroll_edge = Some(name),
                    ListId::Spaces => s.set_space(&name),
                }
                assert_eq!(
                    s.current_row(list),
                    Some(i),
                    "{}: the mark is not on the name in force",
                    list.label(&s)
                );
            }
            // A set whose standing member is not installed here has no
            // standing member — and no mark, rather than a mark on the
            // first name.
            match list {
                ListId::Looks => s.current_look = Some("not installed".into()),
                ListId::Layauts => s.current_layaut = Some("not installed".into()),
                ListId::Sounds => s.current_sounds = Some("not installed".into()),
                ListId::Backgrounds => s.current_background = Some("not installed".into()),
                ListId::Severities => s.current_severity = Some("not installed".into()),
                ListId::Corners => s.current_corner = Some("not installed".into()),
                ListId::RingStyles => s.current_ring_style = Some("not installed".into()),
                ListId::ScrollModes => s.current_scroll_mode = Some("not installed".into()),
                ListId::ScrollEdges => s.current_scroll_edge = Some("not installed".into()),
                // Straight into the field and past `set_space` on
                // purpose: this is the state a configuration file can
                // put the window in when the compositor cannot show
                // what it names, and the answer must be no mark.
                ListId::Spaces => s.color_space = "not installed".into(),
            }
            assert_eq!(
                s.current_row(list),
                None,
                "{}: a name nobody has is marked as standing",
                list.label(&s)
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
                    list.map_or("the page at rest", |l| l.label(&s)),
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
        /// The closed three-point outlines one frame drew INSIDE a box —
        /// the shape `paint::disclosure` makes, and nothing else.
        ///
        /// The box is the PAGE's. The rail speaks the same primitive in
        /// its other grammar ([`Settings::expander_arrow`]) and an open
        /// section's triangle is the same three points pointing the same
        /// way as a shut list's, so a sweep of the whole frame would
        /// count it as a fourth list. Where a triangle stands is the one
        /// thing that tells the two apart, and it is the right thing:
        /// this test is about the anchors ON THE PAGE.
        fn carets(dl: &nacelle::draw::DrawList, box_: Rect) -> Vec<Vec<[f32; 2]>> {
            dl.cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Polyline { pts, closed: true, .. }
                        if pts.len() == 3
                            && pts.iter().all(|p| box_.contains(p[0], p[1])) =>
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
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let page = Panes::of(Metrics::of(&ctx, content), content).page;
            s.draw(&mut ctx);
            carets(&dl, page)
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
                drawn.iter().any(|t| t == list.label(&s)),
                "{} does not wear its own name",
                list.label(&s)
            );
            let value = s.drop_value(list);
            assert!(
                !drawn.iter().any(|t| t.contains(&value)),
                "{} still wears its choice: {value}",
                list.label(&s)
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
        // certainly shorter than eighty lines. It stands in COLUMNS at
        // that height and did before the rail was one column; the draft
        // that folded it here on the rail's HEIGHT is why this line said
        // 1080 for a day.
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
                list.label(&s)
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
                    list.label(&s)
                );
            }
            assert_eq!(
                acts.contains(&Act::ThemesEditor),
                list.carries_door(),
                "{}: the editor door is on the wrong list",
                list.label(&s)
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
        let at = on_the_page(&s);
        s.wheel(-1.0, at.0, at.1);
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
        s.wheel(-1.0, at.0, at.1);
        assert!(
            s.scroll.offset() > page_before,
            "with the list closed the page must take the wheel back"
        );
    }

    // -------------------------------------- the unfold is motion.menu_unfold

    /// One frame of an open list, at `t` seconds after it opened, with
    /// `body` cascaded over the master. Answers the rectangle every name
    /// was registered at.
    ///
    /// The measuring instrument for the three tests below: all of them
    /// ask what the THEME's `motion.menu_unfold` did to a list caught at
    /// a known instant, which is a question that could not be put at all
    /// while this window ran a private `Instant` and an `ease_out`
    /// written into Rust.
    fn unfolding(tag: &str, body: &str, t: f64) -> Vec<Rect> {
        let _t = crate::widgets::Themed::new(tag, body);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.view = View::LookFeel;
        s.dropdown = Some(Dropdown::List(ListId::Looks));
        // Opened at zero on the frame clock, drawn at `t` on the same
        // clock — time as a parameter, which is the whole reason the
        // toolkit's resolver takes it.
        s.dropdown_since = Some(0.0);
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        ctx.t = t;
        s.draw(&mut ctx);
        (0..s.names(ListId::Looks).len())
            .map(|i| {
                s.hits
                    .iter()
                    .find(|&&(_, a)| a == Act::Pick(ListId::Looks, i))
                    .map(|&(r, _)| r)
                    .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
            })
            .collect()
    }

    /// `[motion.menu_unfold] enabled = false` is a theme saying "do not
    /// animate this" — and §5.22's answer for a one-shot that is not to
    /// run is the END state, not a run in zero time. So the very first
    /// frame after the press shows the whole list, pressable.
    ///
    /// It did not. The window read `duration_ms` and nothing else, so
    /// the flag it was switched off by never reached it: on frame one
    /// the progress was zero and the list was a closed blind with
    /// nothing on it to press. A theme could switch the unfold off and
    /// find its lists took 150 ms to arrive anyway.
    #[test]
    fn a_disabled_unfold_puts_the_list_on_screen_at_once() {
        let _g = crate::widgets::theme_test_lock();
        theme::set_viewport(1080.0, 1.0);
        let rows = unfolding("no-unfold", "[motion.menu_unfold]\nenabled = false\n", 0.0);
        for (i, r) in rows.iter().enumerate() {
            assert!(
                r.w > 0.0 && r.h > 0.0,
                "name {i} has no area to press on the first frame: {r:?}"
            );
        }
    }

    /// `motion.scale = 0` — §5.23's reduced motion, reaching §5.22's
    /// catalogue — says the same thing about every effect at once, and
    /// this window heard none of it. The same first frame, the same
    /// whole list.
    #[test]
    fn reduced_motion_opens_the_list_at_once() {
        let _g = crate::widgets::theme_test_lock();
        theme::set_viewport(1080.0, 1.0);
        let rows = unfolding("still-unfold", "[motion]\nscale = 0.0\n", 0.0);
        for (i, r) in rows.iter().enumerate() {
            assert!(
                r.w > 0.0 && r.h > 0.0,
                "name {i} still slid in under reduced motion: {r:?}"
            );
        }
    }

    /// And the CURVE is the theme's word. Halfway through the unfold,
    /// `ease_out` stands at 0.75 of the way and `linear` at 0.5 — so the
    /// last element of the same list, at the same instant, under two
    /// themes that differ in one word, is at two different heights.
    ///
    /// The `ease_out` this window used to run was written into Rust
    /// beside the clock, so both themes drew the identical frame.
    #[test]
    fn the_unfold_follows_the_easing_word() {
        let _g = crate::widgets::theme_test_lock();
        theme::set_viewport(1080.0, 1.0);
        // Half of `motion.menu_unfold.duration_ms`, in seconds.
        let half = f64::from(crate::widgets::token_px("motion.menu_unfold.duration_ms"))
            / 2000.0;
        let last = |tag: &str, word: &str| -> Rect {
            let body = format!("[motion.menu_unfold]\neasing = {word}\n");
            *unfolding(tag, &body, half).last().expect("the list drew no rows")
        };
        let linear = last("unfold-linear", "linear");
        let eased = last("unfold-easeout", "ease_out");
        assert!(
            eased.bottom() > linear.bottom() + 0.5,
            "the easing word did not move the blind: ease_out ends at {}, \
             linear at {}",
            eased.bottom(),
            linear.bottom()
        );
    }

    // ------------------------------------------- the flash is motion.press

    /// Whether the settings window lights a button one frame after it
    /// was pressed, under `body` cascaded over the master.
    fn lights_on_press(tag: &str, body: &str) -> bool {
        let _t = crate::widgets::Themed::new(tag, body);
        let mut s = furnished();
        s.now = 1.0;
        s.perform(Act::OpenFont, 0.0);
        // The next frame, a millisecond later: well inside the master's
        // 150 ms and outside a decay of no length at all.
        s.now = 1.001;
        s.flashing(Act::OpenFont)
    }

    /// The editor's buttons already honoured `[motion.press] enabled`
    /// and `motion.scale`; the settings window's did not, out of the
    /// same two tokens — it asked for `duration_ms` alone. Two halves of
    /// one desktop disagreeing about one theme is the defect a shared
    /// resolver exists to end.
    ///
    /// A press whose decay has no length never lights the button at all:
    /// the one-shot's end state IS an unlit button, and freezing at the
    /// end state means never leaving it.
    #[test]
    fn the_settings_flash_obeys_the_switches_the_editors_already_did() {
        let _g = crate::widgets::theme_test_lock();
        assert!(
            lights_on_press("press-master", "[meta]\nschema = 1\n"),
            "the master's settings buttons do flash"
        );
        assert!(
            !lights_on_press("nopress-settings", "[motion.press]\nenabled = false\n"),
            "a switched-off press still lit a settings button"
        );
        assert!(
            !lights_on_press("still-settings", "[motion]\nscale = 0.0\n"),
            "reduced motion still lit a settings button"
        );
    }

    /// Every effect this binary names by string, against the master's
    /// closed catalogue.
    ///
    /// `motion::Effect::of` takes a `&str`, and an id outside §5.22's
    /// catalogue does not fail — it warns once to stderr and then
    /// freezes at fully visible forever. So a typo in one of these
    /// names is a silently dead animation, which is precisely the
    /// failure a compiler cannot catch and a screenshot barely can.
    /// This is the fail-closed guard for it: the names the desktop
    /// passes, and the names the toolkit passes on the desktop's behalf.
    ///
    /// ONE id is left off deliberately, and it is left off in the open:
    /// `widgets/boot.rs` asks for `boot_sub_blink`, which the master
    /// does not declare and never has. That file's own comment says so —
    /// the boot line stands still and the effect is reported once,
    /// rather than a fourth blink being invented in Rust. It belongs in
    /// this list the day the master grows the section, and not before.
    #[test]
    fn every_motion_effect_the_desktop_names_is_declared() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        for id in [
            // named in this file (`Settings::flashing`) and in
            // `widgets/editor.rs` (`press_ms`)
            "press",
            // `widgets/editor.rs` (`grow_progress`)
            "widget_grow",
            // `mood.rs` (`Fade::read`)
            "mood_change",
            // named by `object::dropdown::accordion_at` for this
            // window's lists — the desktop's animation, resolved on the
            // other side of the seam
            "menu_unfold",
        ] {
            assert!(
                nacelle::theme::id(&format!("motion.{id}.duration_ms")).is_some(),
                "motion.{id} is not in the master's catalogue — every ask \
                 for it freezes at fully visible and says so only once"
            );
            assert!(
                nacelle::theme::id(&format!("motion.{id}.enabled")).is_some(),
                "motion.{id} has no enabled flag"
            );
            assert!(
                nacelle::theme::id(&format!("motion.{id}.easing")).is_some(),
                "motion.{id} has no easing word"
            );
        }
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
        // The whole-theme sections' knobs used to be sliders like any
        // other here — one witness per section shape, arrows not Enter
        // (`SurfHue`, `CornerSeg`, `UnfocusedDim`, `MenuEdgeW`,
        // `TipEdgeW`, `BarW`). Every row that drove one of them left
        // `EDITOR_ROWS` in 2026-08-23's picker-only simplification, so
        // `is_track` correctly answers false for all six now — there is
        // no live witness left to assert true on, per [`Knob`]'s note.
        // A PICKER IS NOT A TRACK, and its sliders least of all: a
        // synthetic Enter/Space press must not set one to a midpoint
        // nobody chose (`Settings::key`'s own guard).
        assert!(!is_track(Act::PickerSlider(PickerId::Accent, 0)));
        assert!(!is_track(Act::PickerSlider(PickerId::Accent, 1)));
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
        let at = on_the_page(&s);
        s.wheel(-3.0, at.0, at.1);
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
        let at = on_the_page(&shut);
        shut.wheel(-3.0, at.0, at.1);
        assert_eq!(
            before,
            shut.scroll.offset(),
            "a closed settings window moved on the wheel"
        );
    }

    /// The COLOR page, opened on what a configuration that says nothing
    /// resolves to. `color_supported` is left at None, which is the
    /// window that has not been told — the whole table on offer, the
    /// state every machine with a working colour manager is in.
    fn color_open() -> Settings {
        let mut s = furnished();
        s.view = View::Color;
        s.seed_color(config::ColorPrefs {
            depth: crate::config::model::ColorConf::DEPTH,
            space: crate::config::model::ColorConf::SPACE.to_string(),
            lut: None,
            icc: None,
        });
        s
    }

    /// A COLOR page seeded from one written space, and nothing else said.
    fn color_on(space: &str) -> Settings {
        let mut s = color_open();
        s.seed_color(config::ColorPrefs {
            depth: crate::config::model::ColorConf::DEPTH,
            space: space.to_string(),
            lut: None,
            icc: None,
        });
        s
    }

    /// Every drop-down the COLOR page describes right now, by identity.
    fn color_drops(s: &Settings) -> Vec<ListId> {
        page_rows(page(View::Color), s)
            .filter_map(|r| match r.ctrl {
                Ctrl::Drop { list } => Some(list),
                _ => None,
            })
            .collect()
    }

    /// The owner's rule, and the hard half of it: SPACE and SPACE HDR are
    /// ONE control.
    ///
    /// Two rows, one shown per range, would satisfy any test that only
    /// read the names on offer — and would be exactly the arrangement the
    /// owner forbade, because a mistake in either condition puts both
    /// words on the screen at once. So this reads the CONTROL and not its
    /// contents: the page describes one drop-down either way, it is the
    /// same `ListId`, it registers the same act at the same focus id, and
    /// the frame draws its anchor at the same rectangle. Only the word on
    /// it and what hangs from it move.
    #[test]
    fn the_hdr_switch_turns_one_list_and_never_reveals_a_second() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = color_open();

        assert_eq!(
            color_drops(&s),
            vec![ListId::Spaces],
            "COLOR at rest does not describe exactly one list"
        );
        let names_off: Vec<String> = s.names(ListId::Spaces).to_vec();
        assert_eq!(ListId::Spaces.label(&s), "SPACE");
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let a = s.rect_of_act(Act::ListBtn(ListId::Spaces)).expect("no SPACE anchor");
        let anchor_off = (a.x, a.y, a.w, a.h);

        s.flip_hdr();

        assert_eq!(
            color_drops(&s),
            vec![ListId::Spaces],
            "turning HDR on revealed a SECOND list — the two words can now \
             stand on the screen together"
        );
        assert_eq!(
            ListId::Spaces.label(&s),
            "SPACE HDR",
            "the one list did not change its word"
        );
        assert_ne!(
            s.names(ListId::Spaces).to_vec(),
            names_off,
            "the one list did not change its contents"
        );
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let a = s.rect_of_act(Act::ListBtn(ListId::Spaces)).expect("no SPACE HDR anchor");
        assert_eq!(
            (a.x, a.y, a.w, a.h),
            anchor_off,
            "the anchor moved: this is a different control wearing the word, \
             not the same one"
        );

        // And on the screen, where the rule was actually written: exactly
        // one of the two words, whichever side is showing.
        for want_hdr in [true, false] {
            if s.color_hdr != want_hdr {
                s.flip_hdr();
            }
            let drawn = page_runs(&mut fonts, &mut s);
            let said = |w: &str| drawn.iter().filter(|t| t.as_str() == w).count();
            assert_eq!(
                (said("SPACE"), said("SPACE HDR")),
                if want_hdr { (0, 1) } else { (1, 0) },
                "with HDR {}, the page wrote SPACE {} time(s) and SPACE HDR \
                 {} time(s)",
                if want_hdr { "on" } else { "off" },
                said("SPACE"),
                said("SPACE HDR")
            );
        }
    }

    /// The list holds one range at a time, and between the two of them it
    /// holds the whole table.
    ///
    /// The second half is what keeps the split honest: a filter that
    /// merely dropped names would pass the first assertion and quietly
    /// lose a space. `COLOR_SPACE_TABLE` is the only statement of which
    /// side a name is on, so the test reads the offers against the table
    /// rather than against a list of its own.
    #[test]
    fn the_space_list_never_holds_both_ranges() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = color_open();
        let mut seen: Vec<String> = Vec::new();
        for want_hdr in [false, true] {
            if s.color_hdr != want_hdr {
                s.flip_hdr();
            }
            for &(name, range) in config::COLOR_SPACE_TABLE.iter() {
                let offered = s.names(ListId::Spaces).iter().any(|n| n == name);
                assert_eq!(
                    offered,
                    range.in_offer(want_hdr),
                    "with HDR {}, '{name}' is {} the list",
                    if want_hdr { "on" } else { "off" },
                    if offered { "in" } else { "missing from" }
                );
            }
            seen.extend(s.names(ListId::Spaces).iter().cloned());

            // And on the screen, unfolded — the first list this page has
            // ever carried, so that it draws and answers at all is part
            // of the statement. Every name it holds is pressable, and no
            // name of the other range is anywhere on the page.
            s.dropdown = Some(Dropdown::List(ListId::Spaces));
            // Fully unfolded: a row still in flight registers nothing,
            // by the list object's own rule.
            s.dropdown_since = None;
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let acts: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
            for i in 0..s.names(ListId::Spaces).len() {
                assert!(
                    acts.contains(&Act::Pick(ListId::Spaces, i)),
                    "name {i} of the unfolded space list is not on the screen"
                );
            }
            let drawn = text_runs(&dl);
            for &(name, range) in config::COLOR_SPACE_TABLE.iter() {
                if range.in_offer(want_hdr) {
                    continue;
                }
                let word = name.to_uppercase();
                assert!(
                    !drawn.iter().any(|t| *t == word),
                    "'{name}' is drawn with HDR {}, where it does not belong",
                    if want_hdr { "on" } else { "off" }
                );
            }
            s.dropdown = None;
        }
        for name in config::COLOR_SPACES {
            assert!(
                seen.iter().any(|n| n == name),
                "'{name}' is in neither offer: the switch cannot reach it at all"
            );
        }
    }

    /// The choice never points into a list that no longer holds it.
    ///
    /// The state of an open list is measured against the names it was
    /// opened over — a scroll offset, a mark, a row under the pointer —
    /// and every one of those becomes a statement about a set that no
    /// longer exists the moment the switch turns. So the switch folds the
    /// list and puts its scroll back to the head, and whatever it lands
    /// on is a member of the offer it landed in.
    #[test]
    fn a_flip_leaves_the_choice_standing_and_the_open_list_folded() {
        for start in config::COLOR_SPACES {
            let mut s = color_on(start);
            assert!(
                s.current_row(ListId::Spaces).is_some(),
                "opened on '{start}' with nothing standing in the list"
            );
            // Two flips: over and back, which is where a memory that
            // remembered the wrong side would show.
            for step in 0..2 {
                s.dropdown = Some(Dropdown::List(ListId::Spaces));
                s.list_scroll.set_offset(120.0);
                s.flip_hdr();
                assert!(
                    s.current_row(ListId::Spaces).is_some(),
                    "'{start}', flip {step}: the standing space '{}' is not in \
                     the list that is now on offer",
                    s.color_space
                );
                assert!(
                    s.dropdown.is_none(),
                    "'{start}', flip {step}: the list stayed open over names \
                     that no longer exist"
                );
                assert_eq!(
                    s.list_scroll.offset(),
                    0.0,
                    "'{start}', flip {step}: the scroll kept an offset measured \
                     against the other set"
                );
            }
            // And back where it started, when the machine offers
            // everything: the trip over and back is not a trip to a
            // default.
            assert_eq!(
                s.color_space, start,
                "a trip across the switch and back lost '{start}'"
            );
        }
    }

    /// The switch and the configuration say ONE thing, in both
    /// directions.
    ///
    /// Reading: the space written in the file settles the switch, with no
    /// second field to contradict it — that is the whole reason there is
    /// no `hdr` in `ColorConf`. Writing: turning the switch produces a
    /// space of the range it was turned to, so the next reading agrees
    /// with this one.
    #[test]
    fn the_switch_and_the_written_space_agree_both_ways() {
        for &(name, range) in config::COLOR_SPACE_TABLE.iter() {
            let s = color_on(name);
            assert_eq!(
                s.color_hdr,
                range == config::SpaceRange::Hdr,
                "'{name}' in the file put the switch in the wrong position"
            );
        }
        // "auto" is the one name the file cannot settle — it stands in
        // both offers — and a page opened on it opens on the standard
        // range, because that is what a file naming no high-range space
        // means. Asked of a window that IS showing the high range, which
        // is the only way to ask it: a reading that merely left the
        // switch alone would answer correctly on a window that had never
        // been turned, and go on showing SPACE HDR over a file that says
        // "auto" on every window that had.
        let mut been_high = color_on("bt2020 pq");
        assert!(been_high.color_hdr, "the fixture did not reach the high range");
        been_high.seed_color(config::ColorPrefs {
            depth: crate::config::model::ColorConf::DEPTH,
            space: "auto".to_string(),
            lut: None,
            icc: None,
        });
        assert!(
            !been_high.color_hdr,
            "a configuration naming 'auto' left the switch where the last \
             visit had put it"
        );

        let mut s = color_open();
        for _ in 0..4 {
            let was = s.color_hdr;
            s.flip_hdr();
            assert_ne!(was, s.color_hdr, "the switch did not turn");
            let range = config::space_range(&s.color_space);
            assert!(
                range.in_offer(s.color_hdr),
                "the switch turned {} and wrote '{}', which is not of that range",
                if s.color_hdr { "on" } else { "off" },
                s.color_space
            );
            // And the writing survives a re-reading, which is the whole
            // of "the state lives in the space". "auto" is the exception
            // and the reason it is an exception is stated above: it is
            // the one name that belongs to both offers, so a file that
            // holds it opens on the standard range whatever the window
            // was showing when it wrote it.
            let reread = color_on(&s.color_space).color_hdr;
            if s.color_space == crate::config::model::ColorConf::SPACE {
                assert!(!reread, "'auto' read back as a request for HDR");
            } else {
                assert_eq!(
                    reread, s.color_hdr,
                    "'{}' read back into the other position",
                    s.color_space
                );
            }
        }
    }

    /// HDR lifts the depth to ten bits, and gives back only what it took.
    ///
    /// Eight-bit PQ bands, and this page has no way to say so — no
    /// warning control exists on it — so the eight is taken off the offer
    /// rather than left there to be pressed. The other half is the one
    /// that is easy to get wrong: a depth the USER pressed is theirs, and
    /// turning HDR off must not put it back to what it was before they
    /// touched it.
    #[test]
    fn hdr_lifts_the_depth_and_gives_back_only_what_it_took() {
        let mut s = color_open();
        s.set_depth(8);
        assert!(depth_values(&s).contains(&8), "eight is missing from the SDR offer");

        assert!(s.flip_hdr(), "the flip did not report moving the depth");
        assert!(s.color_depth >= 10, "HDR left the depth at {}", s.color_depth);
        assert!(
            !depth_values(&s).contains(&8),
            "eight bits is still on offer under HDR"
        );
        assert!(
            !described_acts(&s, page(View::Color)).contains(&Act::ColorDepth(8)),
            "the page still promises an eight-bit press under HDR"
        );

        assert!(s.flip_hdr(), "the flip back did not report moving the depth");
        assert_eq!(s.color_depth, 8, "the depth HDR raised was not given back");

        // The user's own press survives the trip. Twelve, chosen while
        // HDR was on, is still twelve when HDR goes off.
        s.set_depth(8);
        s.flip_hdr();
        s.set_depth(12);
        assert!(!s.flip_hdr(), "turning HDR off moved a depth it had not taken");
        assert_eq!(
            s.color_depth, 12,
            "turning HDR off took back a depth the user chose"
        );
    }

    /// What the machine cannot show is not on the screen — the screen
    /// decision's rule, applied to the switch itself.
    ///
    /// A compositor that can be asked for no high-range space at all is a
    /// machine with no HDR on it, and the switch is not drawn, not
    /// registered and not in the Tab round. `row_shown` and not
    /// `row_when`: a grey ghost is the "just in case" the owner forbade.
    #[test]
    fn a_compositor_with_no_high_range_has_no_switch() {
        let _g = crate::widgets::theme_test_lock();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = color_open();
        assert!(
            described_acts(&s, page(View::Color)).contains(&Act::ColorHdr),
            "the switch is missing where every space is on offer"
        );

        s.set_supported_spaces(Some(
            config::COLOR_SPACE_TABLE
                .iter()
                .filter(|(_, r)| *r != config::SpaceRange::Hdr)
                .map(|&(n, _)| n.to_string())
                .collect(),
        ));
        assert!(
            !described_acts(&s, page(View::Color)).contains(&Act::ColorHdr),
            "the switch stands on a machine that can show no high range"
        );
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        assert!(
            s.rect_of_act(Act::ColorHdr).is_none(),
            "the switch is unreachable by description and pressable all the same"
        );
        // And the list it would have turned still offers the standard
        // range whole.
        for &(name, range) in config::COLOR_SPACE_TABLE.iter() {
            assert_eq!(
                s.names(ListId::Spaces).iter().any(|n| n == name),
                range != config::SpaceRange::Hdr,
                "'{name}' is on the wrong side of a standard-range-only \
                 compositor's offer"
            );
        }

        // A configuration written on other hardware names a space this
        // one cannot show. The page must not open on the side whose
        // switch is not drawn: there would be no control anywhere to
        // turn it back, and the list under SPACE HDR would hold "auto"
        // alone.
        s.seed_color(config::ColorPrefs {
            depth: crate::config::model::ColorConf::DEPTH,
            space: "bt2020 pq".to_string(),
            lut: None,
            icc: None,
        });
        assert!(
            !s.color_hdr,
            "a file naming a space this machine cannot show stranded the page \
             on the high range, where no switch stands"
        );
        assert_eq!(ListId::Spaces.label(&s), "SPACE");
        // The space it names stands in no list, which is the truth: this
        // machine is not showing it.
        assert_eq!(s.current_row(ListId::Spaces), None);
        assert!(
            depth_values(&s).contains(&8),
            "the standard-range depth offer did not come back with the page"
        );
    }

    /// The page opens on a depth it can show, whatever pair the file
    /// holds.
    ///
    /// `depth` and `space` are two lines and one statement, and the
    /// switch is READ OFF the space — so a file saying eight bits and
    /// `bt2020 pq` opens the page on the high range with a DEPTH row
    /// that has nothing marked in it and no way to mark anything: the
    /// eight is not on the screen to be pressed. The rule that takes it
    /// off the offer used to live on the switch's path alone, where a
    /// file never goes.
    #[test]
    fn a_page_opened_from_the_file_stands_on_a_depth_it_offers() {
        // Every pair the file can hold, and the two answers each of them
        // has to satisfy: the depth is a member of the offer the page
        // opened with, and the page promises a press for it.
        for &(space, _) in config::COLOR_SPACE_TABLE.iter() {
            for bits in crate::config::model::COLOR_DEPTHS {
                let mut s = color_open();
                s.seed_color(config::ColorPrefs {
                    depth: bits,
                    space: space.to_string(),
                    lut: None,
                    icc: None,
                });
                assert!(
                    depth_values(&s).contains(&s.color_depth),
                    "'{space}' at {bits} bits opened the page on {} bits, which \
                     is not in the offer {:?} it opened with",
                    s.color_depth,
                    depth_values(&s)
                );
                assert!(
                    described_acts(&s, page(View::Color))
                        .contains(&Act::ColorDepth(s.color_depth)),
                    "'{space}' at {bits} bits: the standing depth {} has no \
                     press on the page",
                    s.color_depth
                );
                // Raised and never lowered, and never raised where the
                // pair was not contradictory in the first place.
                assert!(
                    s.color_depth >= bits,
                    "'{space}' at {bits} bits: the page took the depth DOWN to {}",
                    s.color_depth
                );
                if depth_values(&s).contains(&bits) {
                    assert_eq!(
                        s.color_depth, bits,
                        "'{space}': a depth the page can show was changed anyway"
                    );
                }
            }
        }
    }

    /// Learning what the machine offers never strands the page on the
    /// high range.
    ///
    /// The report decides whether the switch is on the page at all, so
    /// it is a writer of the same state `set_space` guards — and a
    /// second writer that skipped the guard would produce exactly the
    /// dead end the guard exists for: SPACE HDR over a list holding
    /// nothing but "auto", with no switch anywhere to turn it back. Told
    /// from the high range, which is the only position it can strand.
    #[test]
    fn learning_the_offer_never_leaves_the_page_on_a_side_it_cannot_leave() {
        let mut s = color_on("bt2020 pq");
        assert!(s.color_hdr, "the fixture did not reach the high range");

        s.set_supported_spaces(Some(
            config::COLOR_SPACE_TABLE
                .iter()
                .filter(|(_, r)| *r != config::SpaceRange::Hdr)
                .map(|&(n, _)| n.to_string())
                .collect(),
        ));

        assert!(
            !s.color_hdr,
            "a report with no high range in it left the page standing on the \
             high range"
        );
        assert_eq!(ListId::Spaces.label(&s), "SPACE");
        assert!(
            !described_acts(&s, page(View::Color)).contains(&Act::ColorHdr),
            "this test is measuring nothing: the switch is still on the page"
        );
        assert!(
            depth_values(&s).contains(&s.color_depth),
            "the page kept a depth of {} that the side it came back to does \
             not offer",
            s.color_depth
        );
        // The list under the word is the standard range whole, and the
        // space the file named stands in none of it — which is the
        // truth: this machine is not showing it.
        for &(name, range) in config::COLOR_SPACE_TABLE.iter() {
            assert_eq!(
                s.names(ListId::Spaces).iter().any(|n| n == name),
                range != config::SpaceRange::Hdr,
                "'{name}' is on the wrong side of the offer"
            );
        }

        // And the report that says nothing new says nothing at all: a
        // window told twice is in the state it was told once.
        let mut a = color_on("srgb");
        let mut b = color_on("srgb");
        let all: Vec<String> = config::COLOR_SPACES.iter().map(|n| n.to_string()).collect();
        a.set_supported_spaces(Some(all.clone()));
        b.set_supported_spaces(Some(all.clone()));
        b.set_supported_spaces(Some(all));
        assert_eq!(
            (a.color_hdr, a.color_space.clone(), a.names(ListId::Spaces).to_vec()),
            (b.color_hdr, b.color_space.clone(), b.names(ListId::Spaces).to_vec()),
            "being told the same offer twice moved the page"
        );
    }

    /// The row that set the 2026-08-18 storm going: SOUND LEVELS closes
    /// with a line naming the set in use, and it used to work that line
    /// out by asking the disk — the configuration cascade and then the
    /// asset roots, on every frame the page was drawn.
    ///
    /// `Text::Of` is evaluated where the row is DRAWN, so a provider
    /// that touches a file touches it sixty times a second. This is the
    /// shape of the guard: the provider is a function of the window's
    /// own state and of nothing else, and what fills that state is the
    /// door onto the page.
    ///
    /// The second half opens the page, and opening it is the ONE place
    /// allowed to ask the disk — so the disk it asks has to belong to
    /// this test. The lock is the crate's, not this module's: XDG
    /// variables are process-wide, `cargo test` runs on many threads,
    /// and a second lock beside the one in `config` would guard
    /// nothing.
    #[test]
    fn the_sound_pages_closing_line_is_not_a_question_for_the_disk() {
        let _env = config::env_lock();
        let root = std::env::temp_dir()
            .join(format!("nacelle-sound-note-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch tree must be writable");
        for (var, at) in [
            ("XDG_CONFIG_HOME", root.clone()),
            ("XDG_CONFIG_DIRS", root.join("etc")),
            ("XDG_DATA_HOME", root.clone()),
            ("XDG_DATA_DIRS", root.join("share")),
        ] {
            std::env::set_var(var, at);
        }

        let mut s = furnished();
        s.sound_set = "SET: SOMETHING NOBODY HAS INSTALLED".to_string();
        assert_eq!(
            sound_set_note(&s),
            "SET: SOMETHING NOBODY HAS INSTALLED",
            "the closing line came from somewhere other than the window's own state, \
             which on a drawn row means it came from the disk"
        );

        // And the door is what fills it, so the sentence is right when
        // the page arrives rather than right by the time it is drawn.
        s.sound_set.clear();
        s.view = View::LookFeel;
        assert!(!s.perform(Act::OpenSoundLevels, 0.0));
        assert!(
            !s.sound_set.is_empty(),
            "the page opened with nothing to say about its own sound set"
        );

        for var in ["XDG_CONFIG_HOME", "XDG_CONFIG_DIRS", "XDG_DATA_HOME", "XDG_DATA_DIRS"] {
            std::env::remove_var(var);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Every line one set of rows is currently saying, in order —
    /// `Row::when` obeyed, so a note that is not on screen is not here
    /// either.
    fn notes_of(s: &Settings, rows: &'static [Row]) -> Vec<String> {
        rows.iter()
            .filter(|r| (r.when)(s))
            .filter_map(|r| match r.ctrl {
                Ctrl::Note { text } => Some(s.text_of(text).into_owned()),
                _ => None,
            })
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// The same for a whole page, band by band. `page_rows` walks the
    /// description and leaves `Row::when` to its caller — the way
    /// `row_acts` does — so the filter is here.
    fn page_notes(s: &Settings, page: &'static Page) -> Vec<String> {
        page_rows(page, s)
            .filter(|r| (r.when)(s))
            .filter_map(|r| match r.ctrl {
                Ctrl::Note { text } => Some(s.text_of(text).into_owned()),
                _ => None,
            })
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// **The page says what came of the request, and says nothing when
    /// there is nothing to say.**
    ///
    /// This is the owner's report ("changing HDR and the colour space
    /// changes nothing") turned into a rule the window has to keep. A
    /// space the compositor refuses, one it never answers for, an ICC
    /// profile outranking the list — each leaves the picture exactly as
    /// it was while the SPACE list draws its mark on the new name, and
    /// the program used to say so on stderr alone. Whatever the
    /// application reports has to reach the page.
    #[test]
    fn the_color_page_repeats_what_the_compositor_answered() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = color_open();
        // Nothing applied yet is nothing to report — a window under
        // test, and a session in its first frame.
        assert!(
            page_notes(&s, page(View::Color))
                .iter()
                .all(|n| !n.contains("space:")),
            "a page nobody has told reported on a request nobody made"
        );

        s.color_answered("the compositor refused bt2020 pq: unsupported".to_string());
        let said = page_notes(&s, page(View::Color));
        assert!(
            said.iter().any(|n| n.contains("the compositor refused bt2020 pq")),
            "the compositor's refusal never reached the page — the control \
             keeps its mark and the user is told nothing: {said:?}"
        );
    }

    /// **Asked and given, and only where the giving came up SHORT.**
    ///
    /// A surface with no format above eight bits answers eight to a page
    /// showing sixteen, and that is what the line is for. Repeating a
    /// number the DEPTH chips already carry would be noise, and — the
    /// trap this test exists to hold shut — calling a swapchain that
    /// gave MORE than the wish a shortfall would be a lie.
    ///
    /// TWELVE IS THE LIE'S ADDRESS. It is one of the four depths the
    /// page offers (`COLOR_DEPTHS`), it has no swapchain format of its
    /// own, and by the renderer's own arrangement it rides the sixteen
    /// bit float one — so on a healthy machine picking twelve makes the
    /// two numbers differ, upward, every time. A rule written as
    /// "different" put "the surface offers no more" under a user who had
    /// just been given more, on the one depth in four where everything
    /// worked.
    #[test]
    fn the_depth_line_stands_only_where_the_swapchain_gave_less() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = color_open();
        let depth_lines = |s: &Settings| -> Vec<String> {
            page_notes(s, page(View::Color))
                .into_iter()
                .filter(|n| n.starts_with("depth:"))
                .collect()
        };
        assert!(depth_lines(&s).is_empty(), "an unmeasured window claimed a depth");

        s.color_asked(16);
        s.color_measured(16);
        assert!(
            depth_lines(&s).is_empty(),
            "the page repeated a number the DEPTH chips already carry"
        );

        // Twelve asked, sixteen given: the renderer working as built.
        s.color_asked(12);
        s.color_measured(16);
        let said = depth_lines(&s);
        assert!(
            said.is_empty(),
            "the page told a user who asked for 12 and was given 16 that the \
             surface offers no more than 16 — a shortfall reported over a \
             swapchain that overshot the wish: {said:?}"
        );

        s.color_asked(16);
        s.color_measured(8);
        let said = depth_lines(&s);
        assert_eq!(said.len(), 1, "expected one line about the depth: {said:?}");
        assert!(
            said[0].contains("16") && said[0].contains('8'),
            "the line has to carry BOTH numbers — the wish alone is what the \
             page was already saying wrongly: {}",
            said[0]
        );
    }

    /// **Asking again unmeasures — the page cannot report on a
    /// swapchain that has not been asked yet.**
    ///
    /// The renderer's format moves at the REBUILD, and the rebuild is
    /// inside the next frame's `render`. Between the request and that
    /// frame the window holds a new wish beside the previous
    /// measurement, and that pair is exactly what the depth line reads:
    /// ask for sixteen on a machine that had been given ten and the page
    /// would say "16 asked, 10 in the swapchain — the surface offers no
    /// more" about a swapchain nobody had asked. One frame if the page
    /// is redrawn, and until something redraws it if not.
    ///
    /// And only when the number MOVES: `apply_color!` carries the space,
    /// the LUT and the depth together, so a user turning the SPACE list
    /// re-asks for a depth that never changed, rebuilds nothing, and
    /// must not blink a standing line off and on again.
    #[test]
    fn a_fresh_request_puts_out_the_depth_line_until_it_is_measured_again() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = color_open();
        let depth_lines = |s: &Settings| -> Vec<String> {
            page_notes(s, page(View::Color))
                .into_iter()
                .filter(|n| n.starts_with("depth:"))
                .collect()
        };

        // A machine sitting at eight, asked for eight and given eight:
        // agreement, and nothing to report.
        s.color_asked(8);
        s.color_measured(8);
        assert!(depth_lines(&s).is_empty(), "agreement was reported as a shortfall");

        // The user picks sixteen. The rebuild is armed and has not run;
        // the only measurement in the window is the EIGHT of the format
        // on its way out. Read as a pair it says "16 asked, 8 in the
        // swapchain — the surface offers no more", which is a verdict on
        // a surface nobody has asked, and may be flatly wrong about a
        // machine that does offer sixteen.
        s.color_asked(16);
        let said = depth_lines(&s);
        assert!(
            said.is_empty(),
            "the page passed sentence on a swapchain that has not been \
             asked yet, using the depth of the format being replaced: {said:?}"
        );

        // The frame draws, the rebuild happens, and the surface did have
        // sixteen after all — so the sentence above would have been a
        // lie, not merely an early truth.
        s.color_measured(16);
        assert!(
            depth_lines(&s).is_empty(),
            "sixteen asked and sixteen given is nothing to say"
        );

        // And a real shortfall still speaks.
        s.color_asked(12);
        s.color_measured(8);
        assert_eq!(depth_lines(&s).len(), 1, "a real shortfall went unsaid");

        // A request that leaves the depth where it was — the SPACE list,
        // the LUT; `apply_color!` carries all of them in one call —
        // rebuilds nothing, so it must not disturb a standing
        // measurement.
        s.color_asked(12);
        assert_eq!(
            depth_lines(&s).len(),
            1,
            "re-asking for the depth already standing blinked the line out"
        );
    }

    /// **A section painted shut says why.**
    ///
    /// R6 greys a section the machine cannot offer and takes it out of
    /// the focus chain, and that stands. But grey alone reads as "not
    /// now", and the truth is "not here, and nothing you press will
    /// change it" — a compositor that does not speak the Color
    /// Management protocol is not a state the user can leave. The line
    /// is there on exactly those machines and on no other: a rail
    /// carrying it where the feature works would be a permanent apology.
    #[test]
    fn the_shut_color_section_says_why_it_is_shut() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = furnished();
        assert!(s.color_enabled, "the furnished window has a colour manager");
        assert!(
            notes_of(&s, &RAIL_ROWS).is_empty(),
            "the rail apologised on a machine where COLOR SPACE works"
        );

        s.color_enabled = false;
        let said = notes_of(&s, &RAIL_ROWS);
        assert!(
            said.iter().any(|n| n.contains("NO COLOR MANAGER")),
            "COLOR SPACE is painted shut and the rail gives no reason: {said:?}"
        );
        assert!(
            !rail_acts(&s).contains(&Act::OpenColor),
            "this test is measuring nothing: the section is still a target"
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

    /// [`furnished`], standing on `view`, with `open` unfolded on the
    /// rail.
    ///
    /// A FIXTURE THAT HAS TO SAY BOTH THINGS SINCE 2026-08-18. Setting
    /// `s.view` used to unfold the section the view belongs to, because
    /// the fold WAS the view read a second way — which is the fault the
    /// owner reported ([`Settings::rail_open`]). Every fixture that
    /// leaned on that coupling states the two separately here, and a
    /// test that sets only the view is now asking for a SHUT rail on
    /// purpose, which is the state the window opens in.
    fn railed_at(view: View, open: &[Act]) -> Settings {
        let mut s = furnished();
        s.view = view;
        s.unfolded = open.to_vec();
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
    /// A pointer standing over the PAGE and never over the navigation
    /// column — what a wheel test that is about the page's own offset
    /// has to hand [`Settings::wheel`] since the rail took a scroll of
    /// its own. Just past the rail's right edge is inside the page at
    /// every split this window can make; with no rail on the last frame
    /// there is nothing to be beside and any point will do.
    fn on_the_page(s: &Settings) -> (f32, f32) {
        match s.rail_flow {
            Some(r) => (r.bed.right() + 1.0, r.bed.y + 1.0),
            None => (0.0, 0.0),
        }
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
            access: None,
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

    // `choosing_a_border_kind_does_not_reload_the_theme` — ADVANCED's leg
    // of this gesture — stood here until 2026-08-23: EDITOR_ROWS carries
    // no BORDER list (no dropdown at all) since that page became
    // picker-only, so the gesture it drove ("open the editor page, unfold
    // the BORDER list, click NEON") can no longer happen on ADVANCED.
    // BASIC still carries the list and still needs the guarantee, and
    // still has it: [`a_kind_picked_on_basic_does_not_reload_the_theme`].

    /// The three lists BASIC grew on 2026-08-17: each anchor is drawn,
    /// and each one already stands on the member THE THEME is wearing.
    ///
    /// A page whose job is to change a kind has to say which kind is in
    /// force before it is touched, and the state it says it from is the
    /// one the door leaves behind — `seed_editor_from_theme`, which
    /// `Act::ThemesEditor` runs on the way in. Nothing here invents a
    /// selection; the corner's is read back out of the theme by name,
    /// because that one is a single word the file states outright.
    #[test]
    fn the_basic_page_carries_three_kinds_and_each_stands_on_the_themes_own() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.editor_basic = true;
        s.seed_editor_from_theme();
        s.view = View::ThemeEditor;
        let targets = targets_over_the_whole_page(&mut s, &mut fonts, 1080.0);
        for list in [ListId::Backgrounds, ListId::Corners] {
            assert!(
                targets.iter().any(|&(_, a)| a == Act::ListBtn(list)),
                "the BASIC page drew no {} anchor",
                list.label(&s)
            );
            assert!(
                s.current_row(list).is_some(),
                "the {} list opened on {:?}, which is not one of {:?}",
                list.label(&s),
                s.current_of(list),
                s.names(list)
            );
        }
        let mode = nacelle::theme::id("corner.mode")
            .and_then(nacelle::theme::enum_word_of)
            .expect("the master states a corner mode")
            .to_uppercase();
        assert_eq!(
            s.current_of(ListId::Corners),
            Some(&mode),
            "the corner list stands on a shape the theme is not wearing"
        );
    }

    /// The border pick's trap, re-set on the page that now carries three
    /// of these lists.
    ///
    /// A kind is laid OVER the theme until SAVE and writes no config
    /// line, so a pick must answer FALSE. `true` tells main the
    /// configuration changed; main re-resolves it, the theme reloads,
    /// and the fresh engine carries an EMPTY preview — the click would
    /// erase the very picture it had just asked for. Made once per list,
    /// because the fix is per-arm in `Act::Pick` and one arm answering
    /// right says nothing about the next.
    #[test]
    fn a_kind_picked_on_basic_does_not_reload_the_theme() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let (w, h) = (1080.0 * 16.0 / 9.0, 1080.0);
        for list in [ListId::Backgrounds, ListId::Corners] {
            let mut s = furnished();
            s.editor_basic = true;
            s.seed_editor_from_theme();
            s.view = View::ThemeEditor;
            // The page is taller than the window since ZGŁOSZENIE 6, so
            // the anchor is FOUND over a scroll and then brought on
            // screen: a click is aimed at a rect from the frame that is
            // standing, never at one from a frame that has scrolled away.
            let found = targets_over_the_whole_page(&mut s, &mut fonts, h)
                .into_iter()
                .any(|(_, a)| a == Act::ListBtn(list));
            assert!(found, "the BASIC page drew no {} anchor", list.label(&s));
            let anchor = loop {
                if let Some(&(r, _)) =
                    s.hits.iter().find(|&&(_, a)| a == Act::ListBtn(list))
                {
                    break r;
                }
                let before = s.scroll.offset();
                s.scroll.set_offset(before + h / 2.0);
                let mut dl = nacelle::draw::DrawList::new();
                let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
                s.draw(&mut ctx);
                assert!(
                    s.scroll.offset() > before,
                    "the page stopped scrolling before the {} anchor came into view",
                    list.label(&s)
                );
            };
            s.click(anchor.cx(), anchor.y + anchor.h / 2.0, w, h, None);
            assert!(
                matches!(s.dropdown, Some(Dropdown::List(l)) if l == list),
                "the {} anchor did not open its list",
                list.label(&s)
            );
            // A second frame: the list is drawn and its rows registered.
            let mut dl2 = nacelle::draw::DrawList::new();
            let mut ctx2 = probe(&mut dl2, &mut fonts, h, 1.0);
            s.dropdown_since = None; // fully unfolded, no animation
            s.draw(&mut ctx2);
            // The LAST member, so the pick is a real change whatever the
            // theme happened to be wearing at the head of the list.
            let i = s.names(list).len() - 1;
            let want = s.names(list)[i].clone();
            let row = s
                .hits
                .iter()
                .find(|&&(_, a)| a == Act::Pick(list, i))
                .map(|&(r, _)| r)
                .unwrap_or_else(|| {
                    panic!("the open {} list registered no row {i}", list.label(&s))
                });
            assert!(
                !s.click(row.cx(), row.y + row.h / 2.0, w, h, None),
                "a {} pick reported a configuration change — main will reload \
                 the theme and erase the preview the pick just set",
                list.label(&s)
            );
            assert_eq!(
                s.current_of(list),
                Some(&want),
                "the {} pick did not set the kind",
                list.label(&s)
            );
        }
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

    /// Whether BASIC's own description offers a control at all — the
    /// `Row::when` question, asked of the table and not of a drawing, so
    /// "absent" means absent from the page and not merely off screen.
    fn basic_shows(s: &Settings, act: Act) -> bool {
        EDITOR_BASIC_ROWS.iter().any(|r| {
            (r.when)(s)
                && matches!(&r.ctrl,
                    Ctrl::Slider { act: a, .. } | Ctrl::Cycle { act: a, .. } if *a == act)
        })
    }

    /// One token's value out of an edit set.
    fn wrote(edits: &[nacelle::theme::edit::Edit], token: &str) -> Option<String> {
        edits.iter().find(|e| e.token == token).map(|e| e.value.clone())
    }

    /// A length the theme states, in the tracks' own units.
    fn theme_u(token: &str) -> f32 {
        let t = nacelle::theme::resolved();
        let unit = t.unit_px.max(f32::MIN_POSITIVE);
        nacelle::theme::id(token).map(|i| t.px(i)).unwrap_or(0.0) / unit
    }

    /// `"1.20u"` back to 1.20 — reading what was WRITTEN, so a test can
    /// compare it with what the theme said instead of with the function
    /// that wrote it.
    fn u_of(value: &str) -> f32 {
        value.trim_end_matches('u').parse::<f32>().unwrap_or_else(|_| {
            panic!("`{value}` is not a length the theme language spells")
        })
    }

    /// Every target the page holds, gathered over a SCROLL rather than
    /// over one frame.
    ///
    /// A page taller than the window registers only what is on screen —
    /// which is right, and is the same rule a shut rail section obeys —
    /// so a test that wants to know whether a control EXISTS has to walk
    /// the page. BASIC crossed that line on 2026-08-18, when the owner's
    /// ZGŁOSZENIE 6 and 7 put four numbers under its three kinds; before
    /// that its eight rows fitted at 1080 and one frame was the whole
    /// page. Two frames are enough for a page of this length; the second
    /// asks for an offset past the end and the scroll's own tick clamps
    /// it to the bottom.
    fn targets_over_the_whole_page(
        s: &mut Settings,
        fonts: &mut nacelle::font::FontSystem,
        h: f32,
    ) -> Vec<(Rect, Act)> {
        let mut all: Vec<(Rect, Act)> = Vec::new();
        for offset in [0.0, 1.0e6] {
            s.scroll.set_offset(offset);
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, fonts, h, 1.0);
            s.draw(&mut ctx);
            for hit in &s.hits {
                if !all.iter().any(|(_, a)| *a == hit.1) {
                    all.push(*hit);
                }
            }
        }
        all
    }



    // ================= ZGŁOSZENIE 6 and 7 (owner, 2026-08-18) =========

    /// ZGŁOSZENIE 6(a): the corner's SIZE appears with a cut that has one,
    /// and is ABSENT — not greyed — with the cut that has not.
    ///
    /// The question is asked of the DESCRIPTION and not of a drawing, so a
    /// row that is merely scrolled off cannot be mistaken for a row that
    /// is not there. `Row::when` is the owner's own answer to how ("nie
    /// wyszarzonej, tylko nieobecnej") and the mechanism this window
    /// already had; the sibling half — that the row registers no target
    /// while hidden — is `row_acts`', which every reachability sweep runs.
    #[test]
    fn a_square_corner_has_no_size_to_ask_about_and_a_cut_one_has() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        let mut s = furnished();
        s.editor_basic = true;
        for (cut, want) in [("SQUARE", false), ("ROUND", true), ("CHAMFER", true)] {
            s.current_corner = Some(cut.to_string());
            assert_eq!(
                basic_shows(&s, Act::EditorCornerStep),
                want,
                "on {cut} the CORNER SIZE row should {} the page",
                if want { "stand on" } else { "be absent from" }
            );
        }
    }

    /// ZGŁOSZENIE 6(a) again, and this is the decision the owner asked to
    /// see reasoned: the control names a STEP OF THE MASTER'S OWN SCALE
    /// and never a free number.
    ///
    /// What is measured is that the three radii land on the theme's own
    /// numbers — read here out of `[corner]` by name, so both sides of the
    /// comparison cannot move together. The tolerance is the TRACK's, not
    /// a fudge: these are 0..100 tracks over 4u, so a value can only be
    /// stated to a twenty-fifth of a unit and the theme's `0.80u` is the
    /// nearest stop, not the exact one, in general.
    #[test]
    fn a_corner_size_names_a_step_of_the_themes_own_scale() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let mut s = editor_open();
        s.editor_basic = true;
        s.current_corner = Some("ROUND".to_string());
        // A page just seeded says the ladder is the theme's, because it
        // is: nothing has been pressed.
        assert_eq!(
            corner_step_word(&s),
            "AS WRITTEN",
            "a freshly seeded page is not standing on the theme's own ladder"
        );
        // 0..100 over 4u, so one stop of a radius track is 0.04u.
        let stop = 4.0 / 100.0 + 1e-4;
        for (step, token) in
            [("SMALL", "corner.sm"), ("MEDIUM", "corner.md"), ("LARGE", "corner.lg")]
        {
            s.perform(Act::EditorCornerStep, 0.0);
            assert_eq!(corner_step_word(&s), step, "the control did not step to {step}");
            let edits = s.editor_edits();
            let want = theme_u(token);
            let mut got = Vec::new();
            for radius in ["corner.sm", "corner.md", "corner.lg"] {
                let v = wrote(&edits, radius)
                    .unwrap_or_else(|| panic!("{step}: nothing wrote `{radius}`"));
                got.push(u_of(&v));
            }
            assert!(
                got.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6),
                "{step}: the three radii are {got:?} — one step means ONE size for the \
                 whole interface, which is what makes it a BASIC question"
            );
            assert!(
                (got[0] - want).abs() <= stop,
                "{step}: the radii landed on {}u and the theme's `{token}` is {want}u — \
                 this control must name the theme's own numbers, never invent one",
                got[0]
            );
        }
        // And the round is closed: one more press is back on the theme's
        // own ladder, so a press can always be undone.
        s.perform(Act::EditorCornerStep, 0.0);
        assert_eq!(corner_step_word(&s), "AS WRITTEN");
        assert_eq!([s.corner_sm, s.corner_md, s.corner_lg], s.corner_seed);
        nacelle::theme::clear_preview();
    }

    /// ZGŁOSZENIE 6(b): the border's thickness waits to be asked.
    ///
    /// (Its sibling — the REACH of a lit border's light — left with the
    /// whole panel-edge effect on 2026-08-27, the owner's order; the
    /// thickness's own marks and wall still need their witness.)
    #[test]
    fn the_borders_thickness_waits_to_be_asked() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let mut s = editor_open();
        s.editor_basic = true;
        // Untouched: the key is not in the set. The master writes
        // `border.edge.width = @stroke.hair`, a REFERENCE, and a page
        // that pinned it to a literal on every visit would cost every
        // saved theme that reference — §5.5's fault, wearing a new face.
        let quiet = s.editor_edits();
        assert!(
            wrote(&quiet, "border.edge.width").is_none(),
            "an untouched BORDER WIDTH pinned the master's `@stroke.hair` reference \
             into the file"
        );
        // Now a hand moves it.
        s.edge_width = 100;
        s.edge_width_touched = true;
        let moved = s.editor_edits();
        let width = wrote(&moved, "border.edge.width").expect("the width was not written");
        // The master's heaviest stroke is `[stroke] bold = 0.7u`; the top
        // of this track is past it, which is the whole of why the wall is
        // where it is. Read off the THEME, so the expectation is not the
        // production arithmetic run twice.
        assert!(
            u_of(&width) > theme_u("stroke.bold"),
            "the top of the BORDER WIDTH track ({width}) does not reach past the \
             master's heaviest stroke ({}u) — the wall is in the wrong place",
            theme_u("stroke.bold")
        );
        assert!(
            wrote(&moved, "stroke.hair").is_none()
                || wrote(&moved, "stroke.hair") != Some(width.clone()),
            "the border's thickness landed on the GLOBAL kerf; `stroke.hair` is worn by \
             72 derivations and the editor already offers it under HAIRLINE"
        );
        nacelle::theme::clear_preview();
    }

    /// ZGŁOSZENIE 7, the owner's rule: "w trybie BASIC zmiana
    /// przezroczystości wpływa TYLKO na główne tło obiektu".
    ///
    /// MEASURED AS A DIFFERENCE, which is the only way to answer "how far
    /// does it reach" without trusting a list somebody wrote down. Two
    /// edit sets are built from the same controls with ONE number changed
    /// — the transparency — and every token whose value moved is the
    /// reach, by definition.
    ///
    /// This is what fails if the alpha ever reaches the border or the
    /// text: those tokens would appear in `moved` and the assertion names
    /// the whole list.
    ///
    /// WHAT WAS MEASURED BEFORE THE CHANGE, on the master, FROSTED GLASS:
    /// `elev.panel.glass.tint` AND `elev.popover.glass.tint` — two rungs,
    /// and the second is the context menu and the tooltip, which are not
    /// "the object's main background" by any reading. On SOLID the reach
    /// was already one token (`component.panel.fill`), so the breach was
    /// the glassy kinds' alone.
    #[test]
    fn basics_transparency_stops_at_the_body_and_advanceds_still_dresses_the_float() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let mut s = editor_open();

        /// Every token whose value the transparency knob MOVED — ADVANCED's
        /// own, the OPACITY slider (`bg_opacity`), unchanged.
        fn reach_of(s: &mut Settings) -> Vec<&'static str> {
            s.bg_opacity = 40;
            let low = s.editor_edits();
            s.bg_opacity = 90;
            let high = s.editor_edits();
            let mut out: Vec<&'static str> = Vec::new();
            for e in &high {
                if wrote(&low, e.token).as_deref() != Some(e.value.as_str()) {
                    out.push(e.token);
                }
            }
            // And a token that vanished entirely counts as moved too.
            for e in &low {
                if wrote(&high, e.token).is_none() {
                    out.push(e.token);
                }
            }
            out.sort_unstable();
            out
        }

        for (kind, body) in
            [("SOLID", "component.panel.fill"), ("FROSTED GLASS", "elev.panel.glass.tint")]
        {
            s.current_background = Some(kind.to_string());
            // ADVANCED first, because what BASIC narrows has to still be
            // there on the page whose question is "what exactly should
            // this one token do".
            s.editor_basic = false;
            let wide = reach_of(&mut s);
            assert!(
                wide.contains(&body),
                "{kind}: ADVANCED's transparency does not reach the body at all ({wide:?})"
            );
            // BASIC: the body, and nothing else in the world. `bg_opacity`
            // is the one transparency knob again either way (2026-08-19):
            // on SOLID the literal write reads it for the alpha it puts
            // on `component.panel.fill`, on the glassy kinds `glass_edits`
            // still does — the same field, the same lever, unchanged.
            s.editor_basic = true;
            s.seed_tone_from_theme();
            let narrow = reach_of(&mut s);
            assert_eq!(
                narrow,
                vec![body],
                "{kind}: BASIC's transparency reached {narrow:?}. The owner's rule is \
                 the body alone — a border, a text role or another object's bed in \
                 this list is the bug this test exists for"
            );
            if kind == "FROSTED GLASS" {
                assert!(
                    wide.contains(&"elev.popover.glass.tint"),
                    "ADVANCED stopped dressing the float's glass; that was not narrowed"
                );
                // The KIND still travels on BASIC, or a menu over a
                // frosted window would be the one flat plate on screen.
                assert!(
                    wrote(&s.editor_edits(), "elev.popover.glass.rank").is_some(),
                    "BASIC stopped telling the float that the theme is glassy"
                );
            }
        }
        nacelle::theme::clear_preview();
    }

    /// ZGŁOSZENIE 4, THE OWNER'S SENTENCE: "the picker is to be
    /// EVERYWHERE there are colours, ADVANCED included".
    ///
    /// The sweep is over the DESCRIPTIONS of both editor pages, so it
    /// cannot be satisfied by a picker that exists in the code and stands
    /// on no page. Two halves, and neither is enough alone: every picker
    /// this window knows about is offered SOMEWHERE (or a colour would be
    /// unreachable), and no page still asks for a colour with a stack of
    /// tracks (or the picker would be an addition rather than a
    /// replacement, and the owner asked for a replacement).
    ///
    /// WHY "BRIGHTNESS AND SATURATION AND HUE TOGETHER" AND NOT ANY ONE
    /// OF THEM. `SURFACES -> HUE` is a lone hue in degrees and stays a
    /// track on purpose: it is one number on one axis with two ends and a
    /// middle, which is what a track is good at, and there is no colour
    /// there to point at. What a picker replaces is the TRIPLE — the
    /// shape of an `oklch(L, C, H)` value spread over three controls.
    #[test]
    fn every_colour_the_editor_offers_is_pointed_at_and_none_is_hunted_for() {
        let _s = furnished();
        let mut offered: Vec<PickerId> = Vec::new();
        let mut labels: Vec<&'static str> = Vec::new();
        for rows in [&EDITOR_BASIC_ROWS[..], &EDITOR_ROWS[..]] {
            for row in rows {
                match row.ctrl {
                    Ctrl::Picker(id) => offered.push(id),
                    Ctrl::Slider { label, .. } => labels.push(label),
                    _ => {}
                }
            }
        }
        for id in PickerId::ALL {
            assert!(
                offered.contains(&id),
                "{id:?} has a picker and no page offers it — its colour cannot be reached"
            );
        }
        assert_eq!(
            offered.len(),
            PickerId::ALL.len(),
            "a picker is drawn twice, so two rows would answer to one focus id: {offered:?}"
        );
        // No section still spells a colour out as three tracks.
        let has = |w: &str| labels.iter().any(|l| l.contains(w));
        assert!(
            !(has("BRIGHTNESS") && has("SATURATION") && has("HUE")),
            "the editor still asks for a colour with three tracks: {labels:?}"
        );
    }

    /// …AND EACH PICKER WRITES ITS OWN COLOUR AND NOBODY ELSE'S.
    ///
    /// Thirteen pickers stand on one page over thirteen different fields,
    /// and the wire between "which control was pressed" and "which field
    /// it writes" is a match arm apiece ([`Settings::commit_picker`]). A
    /// swapped pair there is a colour that is merely wrong: pick a menu
    /// bed and the tooltip's changes, with nothing to say so. So every
    /// one of them is pressed here, and every OTHER one is required to
    /// stand still.
    ///
    /// Driven through `perform` and a ready-made swatch rather than by
    /// calling the writer, so the act, the picker model and the field all
    /// have to agree — pressing is what a person does.
    #[test]
    fn a_press_on_one_picker_moves_one_colour() {
        let _g = crate::widgets::theme_test_lock();
        let swatches = nacelle::object::color_picker::base_colours();
        assert!(swatches.len() >= 3, "the toolkit offers no ready-made colours to press");
        for id in PickerId::ALL {
            if id == PickerId::Tone {
                continue; // BASIC writes a MOVE, not a colour; see below.
            }
            let mut s = editor_open();
            s.current_severity = Some(s.severity_kinds[0].clone());
            s.seed_pickers_from_tracks();
            let before: Vec<Option<[u32; 3]>> =
                PickerId::ALL.iter().map(|o| s.picker_track(*o)).collect();
            // A swatch that is NOT what the picker already stands on, so
            // "nothing happened" cannot pass for "the right thing did".
            let k = swatches
                .iter()
                .position(|c| hsv_track_of(*c) != s.picker_track(id).unwrap())
                .expect("every ready-made colour is the one this picker already holds");
            s.perform(Act::PickerBase(id, k), 0.0);
            let after: Vec<Option<[u32; 3]>> =
                PickerId::ALL.iter().map(|o| s.picker_track(*o)).collect();
            for (i, other) in PickerId::ALL.iter().enumerate() {
                if *other == id {
                    assert_ne!(
                        before[i], after[i],
                        "pressing {id:?} left its own colour where it was"
                    );
                } else {
                    assert_eq!(
                        before[i], after[i],
                        "pressing {id:?} moved {other:?} as well"
                    );
                }
            }
        }
        nacelle::theme::clear_preview();
    }

    // `active_gamut_space_reads_the_pages_own_state_and_nothing_else` left
    // with `active_gamut_space` itself (the slider-bank rewrite,
    // 2026-08-24) — there is no gamut-boundary curve left on the page for
    // any state to feed. `color_on` stays: COLOR's own SDR/HDR tests below
    // still use it.

    /// The SEVERITY picker writes THE ROLE THAT IS STANDING, and marks
    /// that one alone — the mark `editor_edits` gates the write on, so
    /// the six roles nobody pointed at keep the theme's own words.
    #[test]
    fn the_severity_picker_touches_the_role_the_list_is_showing_and_no_other() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let swatches = nacelle::object::color_picker::base_colours();
        // The SECOND role in the list, so "it wrote the first one" is a
        // failure and not a pass.
        let role = 1;
        s.current_severity = Some(s.severity_kinds[role].clone());
        s.seed_pickers_from_tracks();
        assert_eq!(s.severity_touched, [false; 7], "the editor opened with roles pinned");
        let before = s.severity;
        s.perform(Act::PickerBase(PickerId::Severity, swatches.len() - 1), 0.0);
        for i in 0..7 {
            if i == role {
                assert!(s.severity_touched[i], "the chosen role was not marked touched");
                assert_ne!(before[i], s.severity[i], "the chosen role did not take the colour");
            } else {
                assert!(!s.severity_touched[i], "an unchosen role was pinned");
                assert_eq!(before[i], s.severity[i], "an unchosen role took the colour");
            }
        }
        nacelle::theme::clear_preview();
    }

    /// TWENTY TRIPS, AND THE COLOUR HAS TO BE WHERE IT WAS AFTER THE
    /// FIRST ONE.
    ///
    /// The trip is the one a person makes without meaning to: the page
    /// SHOWS a colour, they leave it alone, they come back, and the page
    /// shows it again. Every leg of it crosses a coordinate system —
    /// picker to OKLCh to a token, token to bake to sRGB, sRGB back to
    /// the picker — and a system that loses a little on any crossing
    /// loses it again on the next trip and the next.
    ///
    /// THIS PROJECT HAS PAID FOR THIS ONCE ALREADY, which is why the
    /// owner asked for the check by name. Reading the bake as if it were
    /// linear light made BASIC seed itself from the colour it had just
    /// written, and the master's accent climbed L 0.8200 -> 0.8904 ->
    /// 0.9413 -> 0.9715 over three visits with nothing touched
    /// (`seed_tone_from_theme`). Three visits was enough to see it;
    /// twenty is enough that a loss a hundredth that size would show.
    ///
    /// THE FIRST TRIP IS THE BASELINE AND NOT THE START, deliberately.
    /// The picked colour lands on whole notches of what the swapchain can
    /// show ([`Settings::tone_step`]) and an ADVANCED track is a whole
    /// number, so the FIRST landing is allowed to move the colour — that
    /// is quantisation, and it is a wall, not a slope. What is forbidden
    /// is the second landing moving it again.
    #[test]
    fn twenty_round_trips_leave_the_colour_where_the_first_one_put_it() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        nacelle::theme::clear_preview();
        let lch = |c: nacelle::theme::Color| c.to_linear().to_oklch();
        let live_accent = || {
            let t = theme::resolved();
            lch(col(t.color(nacelle::theme::id("palette.accent").unwrap())))
        };

        // ---- BASIC: picker -> tone -> preview -> bake -> picker -------
        let mut s = editor_open();
        assert!(s.editor_basic, "the editor did not open on BASIC");
        // A colour that is NOT the theme's, so the loop is exercising a
        // move and not the neutral case another test already holds.
        s.pickers[PickerId::Tone.idx()]
            .set_colour(nacelle::theme::Color::new(0.85, 0.35, 0.20, 1.0));
        s.commit_picker(PickerId::Tone);
        s.apply_editor_preview();

        let mut first = None;
        for trip in 1..=20u32 {
            // OUT: what the page would show on a fresh visit, read off
            // the live bake exactly as arriving on BASIC reads it.
            s.seed_tone_from_theme();
            let shown = s.pickers[PickerId::Tone.idx()].colour();
            // IN: the same colour put back — a person looking and not
            // touching. Nothing here may move.
            s.pickers[PickerId::Tone.idx()].set_colour(shown);
            s.commit_picker(PickerId::Tone);
            s.apply_editor_preview();
            let now = live_accent();
            let base = *first.get_or_insert(now);
            let dh: f32 = (now.h - base.h).rem_euclid(360.0);
            assert!(
                (now.l - base.l).abs() < 0.002
                    && (now.c - base.c).abs() < 0.002
                    && dh.min(360.0 - dh) < 0.5,
                "BASIC drifted by trip {trip}: L {} -> {}, C {} -> {}, h {} -> {}",
                base.l, now.l, base.c, now.c, base.h, now.h
            );
        }

        // ---- THE AUDIT'S OPEN HYPOTHESIS (§1a), MEASURED -------------
        // `.gap-program/audyt-kolory-bazowe.md` wrote down a suspicion it
        // had no measurement for: `Color::from_oklch` holds L and hue
        // exactly and CLAMPS CHROMA to the sRGB boundary by bisection, and
        // `seed_tone_from_theme` reads the seed back off the bake — so a
        // colour outside the gamut might be measured from an already
        // clipped chroma and shrink a little on every visit, the same
        // monotonic slide the lightness once had, in the other axis.
        //
        // SATURATION at 200 % is the way to ask: it doubles the seed's
        // chroma, and the master's mint at C 0.153 doubled is well past
        // anything sRGB can show. If the slide is real it shows here.
        let mut wide = editor_open();
        wide.perform(Act::EditorMode, 0.0);
        wide.tone[1] = TONE_SAT_MAX;
        wide.apply_editor_preview();
        let mut settled_c = None;
        for trip in 1..=20u32 {
            wide.seed_tone_from_theme();
            let shown = wide.pickers[PickerId::Tone.idx()].colour();
            wide.pickers[PickerId::Tone.idx()].set_colour(shown);
            wide.commit_picker(PickerId::Tone);
            wide.apply_editor_preview();
            let c = live_accent().c;
            let base = *settled_c.get_or_insert(c);
            assert!(
                (c - base).abs() < 0.002,
                "an out-of-gamut colour lost chroma on trip {trip}: {base} -> {c}"
            );
        }

        // ---- ADVANCED: picker -> HSV track -> picker ------------------
        // The other crossing, and a NEW one as of 2026-08-18: the page's
        // value is a whole-number HSV track, so a picker seeded from it
        // and committed back is a quantisation applied twice. Twice must
        // be the same as once, or every visit to the page would grind the
        // colour down a step.
        let mut a = editor_open();
        a.pickers[PickerId::MenuFill.idx()]
            .set_colour(nacelle::theme::Color::new(0.31, 0.72, 0.44, 1.0));
        a.commit_picker(PickerId::MenuFill);
        let settled = a.picker_track(PickerId::MenuFill);
        for trip in 1..=20u32 {
            a.seed_pickers_from_tracks();
            a.commit_picker(PickerId::MenuFill);
            assert_eq!(
                a.picker_track(PickerId::MenuFill),
                settled,
                "an ADVANCED picker ground its own track down by trip {trip}"
            );
        }
        nacelle::theme::clear_preview();
    }

    /// `MenuFill`/`TipFill`/`BarTrack` are the three ADVANCED fields that
    /// carry their OWN alpha into the theme (`editor_edits`'s `of(&x,
    /// x_a)` pairs, unlike every other field, which hardcodes 1.0) — but
    /// `commit_picker` used to read the picker's colour through
    /// `hsv_track_of`, which only ever touches r/g/b, so a typed alpha
    /// reached the picker's own swatch and stopped there (2026-08-28's
    /// fix). Checked on every road a picker's colour reaches the theme
    /// from: a direct `commit_picker` (the drag/press road) and the
    /// inline text editor's blur-commit.
    #[test]
    fn menu_tip_and_bar_track_carry_their_typed_alpha_past_the_picker() {
        let _g = crate::widgets::theme_test_lock();
        let cases = [
            (PickerId::MenuFill, "menu_fill_a" as &str),
            (PickerId::TipFill, "tip_fill_a"),
            (PickerId::BarTrack, "bar_track_a"),
        ];
        let field = |s: &Settings, name: &str| match name {
            "menu_fill_a" => s.menu_fill_a,
            "tip_fill_a" => s.tip_fill_a,
            _ => s.bar_track_a,
        };
        for (id, name) in cases {
            let mut s = editor_open();
            s.pickers[id.idx()].set_colour(nacelle::theme::Color::new(0.31, 0.72, 0.44, 0.502));
            s.commit_picker(id);
            assert!(
                (field(&s, name) - 0.502).abs() < 1e-3,
                "{name} after a direct commit: {} vs 0.502",
                field(&s, name)
            );

            // The other road: typed into the value plate, committed on
            // blur — `blur_editing_picker`'s own call into `commit_picker`.
            // hsl(160.61, 74.55%, 56.86%) is rgb8(0x3F, 0xE3, 0xAE); 0.502
            // is 0x80/255 — the same fixture and alpha byte this test used
            // while ARGB still existed.
            let mut s = editor_open();
            s.perform(Act::PickerText(id), 0.0);
            s.pickers[id.idx()]
                .editing_mut()
                .unwrap()
                .set_value("hsl(160.61, 74.55%, 56.86% / 0.502)");
            s.blur_editing_picker();
            let want = 0x80 as f32 / 255.0;
            assert!(
                (field(&s, name) - want).abs() < 1e-3,
                "{name} after a typed, blurred edit: {} vs {want}",
                field(&s, name)
            );
            nacelle::theme::clear_preview();
        }
    }

    /// The ADVANCED COLOUR button and the grid's back button, and the swap
    /// they make. The editor opens on BASIC — the one page — and pressing
    /// ADVANCED COLOUR puts the per-element grid where BASIC stood; the
    /// grid's own back button returns. Both are `Act::EditorMode`, the
    /// state the deleted BASIC/ADVANCED switch used to toggle.
    #[test]
    fn the_advanced_colour_button_swaps_the_page_in_place() {
        let _g = crate::widgets::theme_test_lock();
        let page = &PAGES[View::ThemeEditor as usize];
        let mut s = editor_open();

        // OPENS ON BASIC: its own picker, its door to the grid, and not one
        // of ADVANCED's per-element controls.
        assert!(s.editor_basic, "the editor did not open on BASIC");
        let basic = described_acts(&s, page);
        // ADVANCED COLOUR moved onto the Tone row itself in 2026-08-23's
        // picker-only simplification (at the end of its notation strip,
        // [`Settings::advanced_colour_button`]) — a STATIC row scan no
        // longer finds it, because it is not a row of its own any more.
        // A real draw pass and the hits it registers are the only way to
        // ask "is this button on screen", which is exactly what pressing
        // it means, two lines down.
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        assert!(
            s.hits.iter().any(|&(_, a)| a == Act::EditorMode),
            "BASIC has no ADVANCED COLOUR button"
        );
        // The picker and every part of it: the one control the three tone
        // sliders became on 2026-08-18.
        for part in [
            Act::PickerSlider(PickerId::Tone, 0),
            Act::PickerSlider(PickerId::Tone, 1),
            Act::PickerFormat(PickerId::Tone),
            Act::PickerText(PickerId::Tone),
            Act::PickerBase(PickerId::Tone, 0),
            Act::PickerAdd(PickerId::Tone),
        ] {
            assert!(basic.contains(&part), "BASIC is missing a part of its picker");
        }
        assert!(
            !basic.iter().any(|a| matches!(
                a,
                Act::PickerSlider(PickerId::Edge, _) | Act::EditorTrack(Knob::CornerSm)
            )),
            "BASIC is showing ADVANCED's controls"
        );

        // The button opens the grid IN PLACE: the per-element pickers, with
        // BASIC's own picker gone and a back door of the grid's own.
        s.perform(Act::EditorMode, 0.0);
        assert!(!s.editor_basic, "ADVANCED COLOUR did not open the grid");
        let advanced = described_acts(&s, page);
        assert!(
            advanced.contains(&Act::PickerSlider(PickerId::Edge, 0)),
            "the grid is not showing the border picker"
        );
        assert!(
            !advanced.contains(&Act::PickerSlider(PickerId::Tone, 0)),
            "the grid is showing BASIC's picker"
        );
        assert!(advanced.contains(&Act::EditorMode), "the grid has no back button");

        // The verbs belong to BOTH pages: the bar is pinned, not banded.
        for verb in [Act::EditorSave, Act::EditorSaveAs, Act::EditorCancel] {
            assert!(basic.contains(&verb), "BASIC lost one of the editor's verbs");
            assert!(advanced.contains(&verb), "the grid lost one of the editor's verbs");
        }
        // Back returns to BASIC, on the same one control.
        s.perform(Act::EditorMode, 0.0);
        assert!(s.editor_basic, "the back button did not return to BASIC");
        // The preview these presses pushed is this test's, and it does
        // not leave the room in it: another test reading the theme
        // would be reading this one's editor session.
        nacelle::theme::clear_preview();
    }

    /// BASIC's move writes the theme's AUTHORS, and nothing that the
    /// cascade derives.
    ///
    /// The set is the model's ([`nacelle::theme::edit::tone_edits`]) and
    /// libnacelle holds it to its six tokens; what this measures is the
    /// WINDOW's half — that the control is wired to it at all, that a
    /// move changes what would be written, and that the rest of the
    /// editor's set is still in the edit underneath.
    #[test]
    fn the_basic_sliders_move_the_authors_and_leave_the_rest_standing() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
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
            "palette.black",
            "palette.white",
            "palette.neutral",
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
        // THE SEVERITY FAMILY IS NOT WRITTEN HERE AT ALL, since
        // 2026-08-18 (owner, ZGŁOSZENIE 5). This page used to write all
        // seven with the full rotation — which is how a green success
        // came out red — and the roles lean toward `palette.accent` in
        // the theme itself now, capped at `severity.pull_clamp`. Silence
        // is the whole point: a theme that dressed its own `contained`
        // amber used to lose it the moment anybody opened this page.
        assert_eq!(
            turned.iter().filter(|e| e.token.starts_with("severity.")).count(),
            0,
            "BASIC repainted the severity roles"
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
        // The border colour is now the shared root `border.default`, not
        // the `elev.panel` leaf, so one edit moves every frame alike
        // (`edit::border_colour_edit`).
        for token in ["border.default", "corner.mode", "scrollbar.mode"] {
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

    /// THE PICKER OPENS ON THE THEME AND ASKS FOR NOTHING.
    ///
    /// The scar this stands on is written up at `oklch_of_track` and in
    /// `.gap-program/obalone-naprawy.md`: BASIC used to seed itself from
    /// what it had just written, so the accent's lightness climbed on
    /// every visit with every control at rest. The picker moved the road
    /// but not the hazard — it reads the accent on the way in and writes
    /// a distance from the accent on the way out, so if those two
    /// crossings ever disagree the theme walks again. TWENTY visits
    /// here, for the same reason the toolkit's own test takes twenty
    /// trips.
    #[test]
    fn the_picker_opens_on_the_theme_and_asks_for_no_move() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.tone_seeds.expect("the editor opened without seeds");
        // What the control SHOWS is the theme's own background bed
        // (2026-08-19 — was the accent; BASIC's one picker answers "what
        // colour is the desktop" by pointing at it now).
        let seed = s.tone_bed;
        let shown = s.pickers[PickerId::Tone.idx()].oklch();
        assert!(
            (shown.l - seed.l).abs() < 2e-3 && (shown.c - seed.c).abs() < 5e-3,
            "the picker opened on {shown:?}, the theme says {seed:?}"
        );
        // And asking it for a move, untouched, is asking for none.
        s.set_tone_from_picker();
        assert_eq!(s.tone, TONE_REST, "an untouched picker asked for a move");
        // AND AT THE FINEST NOTCH THE PIPELINE HAS. The eight-bit grid
        // is coarse enough to round a small error away, so a page that
        // only ever asked at eight bits could be quietly wrong; sixteen
        // bits is one track unit per notch and rounds nothing away.
        s.color_depth = 16;
        s.set_tone_from_picker();
        assert_eq!(
            s.tone, TONE_REST,
            "an untouched picker asked for a move at the finest notch"
        );
        for visit in 0..20 {
            s.seed_editor_from_theme();
            s.set_tone_from_picker();
            assert_eq!(s.tone, TONE_REST, "visit {visit} asked for a move");
            let now = s.pickers[PickerId::Tone.idx()].oklch();
            assert!(
                (now.l - seed.l).abs() < 2e-3,
                "visit {visit} shows lightness {} where the theme says {}",
                now.l,
                seed.l
            );
        }
        nacelle::theme::clear_preview();
    }

    /// ONE PRESS, ONE SOUND — and the picker's seven were the exception.
    ///
    /// `perform` gives every activation a plain click and exempts the
    /// ones that mean more than a press and say so themselves. None of
    /// the picker's acts was on that exemption list, and three of them
    /// emitted a sound of their own regardless: the notation plate and
    /// the bank cell made TWO CLICKS for one press, and a ready-made
    /// colour made a click and a theme. The comment two lines above that
    /// catch-all already stated the rule the code was breaking — "a
    /// press that clicked AND toggled would be the only one in the
    /// window that made two sounds".
    ///
    /// Counted off [`Settings::heard`] and not off `nacelle::sound`'s
    /// queue: that queue belongs to the whole process and every other
    /// test in this binary shouts into it, so a count taken from there
    /// would be a count of the test run.
    #[test]
    fn every_press_on_the_picker_makes_exactly_one_sound() {
        use nacelle::sound::Event as Sfx;
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::EditorMode, 0.0);
        s.picker_custom = vec![nacelle::theme::Color::WHITE, nacelle::theme::Color::BLACK];
        // Named by hand because `Act` carries no `Debug` and this window
        // has never wanted one; the names are what a failure has to say.
        for (name, act) in [
            ("the first slider", Act::PickerSlider(PickerId::Tone, 0)),
            ("the second slider", Act::PickerSlider(PickerId::Tone, 1)),
            ("the notation plate", Act::PickerFormat(PickerId::Tone)),
            ("the value plate", Act::PickerText(PickerId::Tone)),
            ("a ready-made colour", Act::PickerBase(PickerId::Tone, 0)),
            ("a banked colour", Act::PickerCustom(PickerId::Tone, 0)),
            ("the bank cell", Act::PickerAdd(PickerId::Tone)),
            // Pressed a second time: the colour is already banked, so
            // the row does not grow — and the press is still a press.
            ("the bank cell again", Act::PickerAdd(PickerId::Tone)),
        ] {
            s.perform(act, 0.5);
            assert_eq!(
                s.heard.len(),
                1,
                "{name} made {:?}, and a press makes one sound",
                s.heard
            );
        }
        // AND THE RIGHT ONE. The two grids move the live preview, which
        // is what `Theme` is the window's word for; the rest are plain
        // presses. A test that only counted would pass on a picker that
        // clicked when it should have spoken.
        for (name, act, want) in [
            ("a ready-made colour", Act::PickerBase(PickerId::Tone, 0), Sfx::Theme),
            ("a banked colour", Act::PickerCustom(PickerId::Tone, 1), Sfx::Theme),
            ("the notation plate", Act::PickerFormat(PickerId::Tone), Sfx::Click),
            ("the value plate", Act::PickerText(PickerId::Tone), Sfx::Click),
        ] {
            s.perform(act, 0.5);
            assert_eq!(s.heard, vec![want], "{name} said the wrong thing");
        }
        nacelle::theme::clear_preview();
    }

    fn escape_key() -> KeyEv {
        KeyEv { key: FKey::Escape, mods: Mods::NONE, repeat: false, text: None }
    }

    fn enter_key() -> KeyEv {
        KeyEv { key: FKey::Enter, mods: Mods::NONE, repeat: false, text: None }
    }

    /// Opening the value plate seeds the inline editor from what it
    /// already shows, exactly `Picker::begin_edit`'s own contract, and
    /// puts EXACTLY that picker in `editing_picker` — the "one at a time"
    /// bookkeeping the field's own doc names.
    #[test]
    fn pressing_the_value_plate_opens_the_inline_editor_seeded_from_it() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let seeded = s.pickers[PickerId::Edge.idx()].text();
        assert!(s.editing_picker.is_none(), "nothing is open before the press");
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        assert_eq!(s.editing_picker, Some(PickerId::Edge));
        assert!(s.pickers[PickerId::Edge.idx()].is_editing());
        assert_eq!(
            s.pickers[PickerId::Edge.idx()].editing_mut().unwrap().value(),
            seeded
        );
        nacelle::theme::clear_preview();
    }

    /// Enter, on a value the picker's own parser reads, commits and
    /// closes — and the colour reaches the FIELD this picker writes
    /// (`Settings::commit_picker`), not merely the picker's own model, so
    /// typing a colour behaves exactly like dragging one.
    #[test]
    fn enter_commits_a_good_value_reaches_the_field_and_closes_the_editor() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let mut fc = FocusCtl::new();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        s.pickers[PickerId::Edge.idx()]
            .editing_mut()
            .unwrap()
            .set_value("hsl(120, 100%, 50%)");
        assert!(matches!(s.key(&enter_key(), &mut fc), KeyOut::Changed));
        assert!(s.editing_picker.is_none(), "Enter on a good value closes the editor");
        assert!(!s.pickers[PickerId::Edge.idx()].is_editing());
        let c = s.pickers[PickerId::Edge.idx()].colour();
        assert_eq!((q8(c.r), q8(c.g), q8(c.b)), (0x00, 0xFF, 0x00), "the picker took the typed colour");
        assert_eq!(
            s.edge,
            hsv_track_of(c),
            "the typed colour reached `edge`, the field this picker writes"
        );
        nacelle::theme::clear_preview();
    }

    fn q8(v: f32) -> u8 {
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// `oklch(L, C, H)` or `oklch(L, C, H / A)` back to its three (or
    /// four) numbers — the same plain split libnacelle's own
    /// `theme::edit` tests read a written token with, now that the
    /// picker's `Format::Oklch` is gone (HSL is the one notation the
    /// control itself reads). Reading the numbers straight out of the
    /// FILE'S OWN string, rather than round-tripping through the
    /// picker's sRGB `Color`, is a closer check of what got written, not
    /// a looser one.
    fn parse_oklch_token(s: &str) -> nacelle::theme::color::Oklch {
        let inner = s.trim().trim_start_matches("oklch(").trim_end_matches(')');
        let (head, alpha) = match inner.split_once('/') {
            Some((h, a)) => (h, a.trim().parse::<f32>().unwrap()),
            None => (inner, 1.0),
        };
        let mut n = head.split(',').map(|p| p.trim().parse::<f32>().unwrap());
        nacelle::theme::color::Oklch {
            l: n.next().unwrap(),
            c: n.next().unwrap(),
            h: n.next().unwrap(),
            alpha,
        }
    }

    /// A bad parse on Enter STAYS OPEN, text untouched — the SAVE AS
    /// prompt's own `if name.is_empty()` guard, read the other way round
    /// (`Settings::key`'s own note on its editing-picker block).
    #[test]
    fn enter_on_a_bad_parse_stays_open_and_keeps_the_colour() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let mut fc = FocusCtl::new();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        let before = s.pickers[PickerId::Edge.idx()].colour();
        s.pickers[PickerId::Edge.idx()]
            .editing_mut()
            .unwrap()
            .set_value("not a colour");
        assert!(matches!(s.key(&enter_key(), &mut fc), KeyOut::Consumed));
        assert_eq!(s.editing_picker, Some(PickerId::Edge), "a bad parse leaves the editor OPEN");
        assert_eq!(
            s.pickers[PickerId::Edge.idx()].editing_mut().unwrap().value(),
            "not a colour",
            "the typed text is untouched"
        );
        assert_eq!(s.pickers[PickerId::Edge.idx()].colour(), before, "a bad parse never destroys the good value");
        nacelle::theme::clear_preview();
    }

    /// Escape discards the typed text and reverts — nothing here ever
    /// touched the colour, so "revert" is simply "never changed it".
    #[test]
    fn escape_cancels_the_inline_editor_without_touching_the_colour() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let mut fc = FocusCtl::new();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        let before = s.pickers[PickerId::Edge.idx()].colour();
        s.pickers[PickerId::Edge.idx()]
            .editing_mut()
            .unwrap()
            .set_value("hsl(0, 0%, 0% / 0)");
        assert!(matches!(s.key(&escape_key(), &mut fc), KeyOut::Consumed));
        assert!(s.editing_picker.is_none());
        assert!(!s.pickers[PickerId::Edge.idx()].is_editing());
        assert_eq!(s.pickers[PickerId::Edge.idx()].colour(), before);
        nacelle::theme::clear_preview();
    }

    /// A PRESS ELSEWHERE BLURS AND COMMITS — including a press on a
    /// DIFFERENT picker entirely, which is the one place this design
    /// deliberately diverges from the SAVE AS prompt's playbook (that
    /// prompt covers the whole window, so a blur is structurally
    /// impossible there; this editor does not, so it is a real event).
    #[test]
    fn a_press_on_a_different_picker_blurs_and_commits_the_open_editor() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        // hsl(210, 50%, 13.33%) is rgb8(0x11, 0x22, 0x33).
        s.pickers[PickerId::Edge.idx()]
            .editing_mut()
            .unwrap()
            .set_value("hsl(210, 50%, 13.33%)");
        // A slider on ACCENT — a different picker altogether.
        s.perform(Act::PickerSlider(PickerId::Accent, 0), 0.5);
        assert!(s.editing_picker.is_none(), "the other picker's press blurred the open editor");
        assert!(!s.pickers[PickerId::Edge.idx()].is_editing());
        let c = s.pickers[PickerId::Edge.idx()].colour();
        assert_eq!((q8(c.r), q8(c.g), q8(c.b)), (0x11, 0x22, 0x33), "the blur committed the typed value");
        assert_eq!(s.edge, hsv_track_of(c), "and it reached the field, the same as Enter does");
        nacelle::theme::clear_preview();
    }

    /// A drag already held when the SAVE AS prompt opens by KEYBOARD is
    /// the one road `click`'s own naming guard never sees, since it only
    /// ever runs on a fresh press: nothing refuses this drag a second
    /// time, so it kept moving the theme underneath the prompt until
    /// `drag` grew the same guard (2026-08-28's fix).
    #[test]
    fn a_drag_already_held_stops_moving_the_theme_once_save_as_opens() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::PickerSlider(PickerId::Edge, 0), 0.5);
        let before = s.edge;
        s.naming = Some(nacelle::object::text_input::InputModel::new());
        s.drag(0.9, 0.0);
        assert_eq!(s.edge, before, "a drag under the SAVE AS prompt still moved the theme");
        nacelle::theme::clear_preview();
    }

    /// Nothing else clears a stale `self.dragging` but `release()` — a
    /// mouse-up the app never sees (focus lost mid-drag, a fast release
    /// past the window's own edge) leaves one armed. Every road that
    /// changes what is drawn under it must drop it too, or a later
    /// press elsewhere on the SAME page can hijack a slider the hand
    /// left minutes ago (2026-08-28's fix).
    #[test]
    fn a_stale_drag_is_cleared_by_every_road_that_leaves_it_behind() {
        let _g = crate::widgets::theme_test_lock();
        let arm = |s: &mut Settings| {
            s.perform(Act::PickerSlider(PickerId::Edge, 0), 0.5);
            assert!(s.dragging.is_some(), "the fixture press did not arm a drag");
        };

        let mut s = editor_open();
        arm(&mut s);
        s.go(View::LookFeel);
        assert!(s.dragging.is_none(), "go() left a drag armed");

        let mut s = editor_open();
        arm(&mut s);
        s.close();
        assert!(s.dragging.is_none(), "close() left a drag armed");

        let mut s = editor_open();
        arm(&mut s);
        s.perform(Act::Close, 0.0);
        assert!(s.dragging.is_none(), "Act::Close left a drag armed");

        let mut s = editor_open();
        arm(&mut s);
        s.perform(Act::EditorCancel, 0.0);
        assert!(s.dragging.is_none(), "Act::EditorCancel left a drag armed");

        let mut s = editor_open();
        arm(&mut s);
        s.perform(Act::EditorMode, 0.0);
        assert!(s.dragging.is_none(), "Act::EditorMode left a drag armed");
        nacelle::theme::clear_preview();

        let mut s = editor_open();
        arm(&mut s);
        s.opening();
        assert!(s.dragging.is_none(), "opening() left a drag armed");
    }

    /// A click that hits NOTHING is "elsewhere" too, and
    /// `Settings::click`'s own "hit nothing" branch is the one path that
    /// never reaches `Settings::perform`'s head guard — the gap that
    /// branch exists to close.
    #[test]
    fn a_click_on_blank_space_blurs_the_open_editor() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        assert!(s.editing_picker.is_some());
        s.hits.clear(); // nothing under the point below
        assert!(!s.click(-1.0, -1.0, 1920.0, 1080.0, None));
        assert!(s.editing_picker.is_none(), "a click that hits nothing still blurs");
        nacelle::theme::clear_preview();
    }

    /// Pressing the SAME picker's own plate again, while it is already
    /// open, is a no-op — it must not restart the typed text.
    #[test]
    fn pressing_the_same_plate_again_does_not_reset_the_typed_text() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        // hsl(210, 16.67%, 60%) is rgb8(0x88, 0x99, 0xAA).
        s.pickers[PickerId::Edge.idx()]
            .editing_mut()
            .unwrap()
            .set_value("hsl(210, 16.67%, 60%)");
        s.perform(Act::PickerText(PickerId::Edge), 0.0);
        assert_eq!(s.editing_picker, Some(PickerId::Edge), "still the same picker, still open");
        assert_eq!(
            s.pickers[PickerId::Edge.idx()].editing_mut().unwrap().value(),
            "hsl(210, 16.67%, 60%)",
            "a second press on its own plate did not restart the typed text"
        );
        nacelle::theme::clear_preview();
    }

    /// The picker opens on a colour THE THEME NAMED.
    ///
    /// It opened on `Color::GREY` — 0.5, 0.5, 0.5, written into this
    /// file — and the defence was that the value is neutral rather than
    /// decorative and lives only until `seed_editor_from_theme` runs.
    /// Both halves of that are true and neither makes it not a look: the
    /// grey is on the screen for as long as it takes somebody to reach
    /// the editor, and a neutral is a colour somebody chose. On a page
    /// whose entire thesis is that no colour lives in Rust, it was the
    /// one colour that did.
    #[test]
    fn the_picker_opens_on_a_colour_the_theme_named() {
        let _g = crate::widgets::theme_test_lock();
        let want = theme::resolved().color(
            nacelle::theme::id("component.picker.rest")
                .expect("the master names what a picker holds at rest"),
        );
        let shown = Settings::new().pickers[PickerId::Tone.idx()].colour();
        for (got, want, ch) in
            [(shown.r, want.r, 'r'), (shown.g, want.g, 'g'), (shown.b, want.b, 'b')]
        {
            assert!(
                (got - want).abs() < 1e-4,
                "the picker opened on channel {ch} = {got}, the theme says {want}"
            );
        }
    }

    /// How fast a dragged control re-bakes the desktop is the THEME's
    /// number.
    ///
    /// It was `100` written twice in this file — once for the editor's
    /// sliders, once for the picker's two areas — which is one edit away
    /// from two controls that follow the hand at different speeds. A
    /// rate a person can see is a look, and looks live in the theme.
    #[test]
    fn the_drag_pulse_is_the_themes_number() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = furnished();
        {
            // A theme that asks for no throttle at all: every drag frame
            // re-bakes, so two calls running are both due.
            let _t = crate::widgets::Themed::new("pulse-open", "[settings]\npreview_pulse_ms = 0ms\n");
            assert!(s.preview_pulse_due(), "the first drag frame is always due");
            assert!(s.preview_pulse_due(), "at 0 ms every frame is due");
        }
        {
            // And one that asks for a pulse longer than this test takes:
            // the second call is refused.
            let _t =
                crate::widgets::Themed::new("pulse-slow", "[settings]\npreview_pulse_ms = 5000ms\n");
            s.editor_pulse = None;
            assert!(s.preview_pulse_due(), "the first drag frame is always due");
            assert!(!s.preview_pulse_due(), "at 5 s the next frame waits");
        }
    }

    /// A COLOUR POINTED AT BECOMES THE DISTANCE TO IT — the one line
    /// between an absolute control and a page whose every write is
    /// relative ([`Settings::set_tone_from_picker`]).
    ///
    /// It is measured in both directions: the tone the window computes,
    /// and the colour that tone puts in the file, read back with the
    /// toolkit's own parser so nothing here is checked against a string
    /// this test wrote itself.
    /// A minimal reader for the exact grammar `oklch_literal` writes —
    /// `oklch(L, C, H)` or `oklch(L, C, H / A)`, three or four plain
    /// numbers, nothing the fixtures below ever ask the full theme
    /// expression language for (references, ratios). Kept here and not
    /// in the picker: the picker's own notation is HSL now (2026-08-28),
    /// and this reads back the FILE's notation, not the control's.
    fn parse_oklch_literal(s: &str) -> nacelle::theme::color::Oklch {
        let body = s.trim().trim_start_matches("oklch(").trim_end_matches(')');
        let (head, tail) = match body.split_once('/') {
            Some((h, a)) => (h, a.trim().parse::<f32>().ok()),
            None => (body, None),
        };
        let n: Vec<f32> =
            head.split(',').map(|p| p.trim().parse::<f32>().expect("a plain number")).collect();
        assert_eq!(n.len(), 3, "not the plain oklch(L, C, H[/A]) grammar: {s}");
        nacelle::theme::color::Oklch { l: n[0], c: n[1], h: n[2], alpha: tail.unwrap_or(1.0) }
    }

    #[test]
    fn a_colour_picked_in_basic_travels_as_the_distance_from_the_theme() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.tone_seeds.expect("BASIC opened without seeds");
        // The move is measured against the BACKGROUND now (2026-08-19),
        // not the accent — see `Settings::set_tone_from_picker`.
        let seed = s.tone_bed;

        // A QUARTER TURN. Taken at HALF THE CHROMA on purpose: the
        // picker is an sRGB control and a colour ninety degrees round
        // from the seed may have no sRGB match at the seed's own chroma
        // — `Color::from_oklch` bisects the chroma down until there is
        // one, and the honest report of that is a move the theme's
        // author did not ask for. What is measured here is the
        // arithmetic, so the colour is one the control can actually
        // show.
        let step = s.tone_step();
        let half = nacelle::theme::color::Oklch { c: seed.c * 0.5, ..seed };
        s.pickers[PickerId::Tone.idx()].set_oklch(nacelle::theme::color::Oklch { h: half.h + 90.0, ..half });
        s.set_tone_from_picker();
        assert!(
            (s.tone[0] as i32 - 90).abs() <= step[0] as i32,
            "a quarter turn asked for {} degrees",
            s.tone[0]
        );
        assert!(
            (s.tone[1] as i32 - 50).abs() <= 2 * step[1] as i32,
            "half the chroma asked for a multiplier of {}",
            s.tone[1]
        );
        assert!(
            (s.tone[2] as i32 - 50).abs() <= step[2] as i32,
            "a turn and a chroma change moved the lightness to {}",
            s.tone[2]
        );

        // WHAT THE FILE WOULD RECEIVE IS WHAT THE CONTROL SHOWS — read
        // back with the toolkit's own parser, so nothing here is checked
        // against a string this test wrote itself.
        let edits = s.editor_edits();
        let written = edits
            .iter()
            .find(|e| e.token == "palette.accent")
            .map(|e| e.value.clone())
            .expect("BASIC wrote no accent");
        let got = parse_oklch_literal(&written);
        let shown = s.pickers[PickerId::Tone.idx()].oklch();
        let off = (got.h - shown.h).rem_euclid(360.0);
        let off = off.min(360.0 - off);
        assert!(off <= step[0] as f32 + 1.0, "the file receives hue {}, shown {}", got.h, shown.h);
        // THE FILE'S CHROMA IS THE ACCENT'S OWN, SCALED — not the picker's
        // raw number. `shown.c` sits on the BACKGROUND's scale (2026-08-19:
        // the seed is the bed, ~an eighth of the accent's own chroma), so
        // comparing the two directly compares two different rulers. What
        // has to hold is the ARITHMETIC `tone_shift` actually runs: the
        // accent's seed chroma times the SAME sat ratio `tone[1]` names —
        // gamut mapping may clamp it down from there, never up, so the
        // check is a ceiling, not an equality.
        let accent_seed_c = s.tone_seeds.expect("BASIC opened without seeds").accent.c;
        let expect_c = accent_seed_c * (s.tone[1] as f32 / 100.0);
        assert!(
            got.c <= expect_c + 0.01 && got.c > 0.0,
            "the file receives chroma {}, the arithmetic asks for at most {} \
             ({}% of the accent's own {})",
            got.c,
            expect_c,
            s.tone[1],
            accent_seed_c
        );

        // A CHROMA MOVE ALONE DOES NOT TURN THE THEME.
        s.pickers[PickerId::Tone.idx()].set_oklch(half);
        s.set_tone_from_picker();
        // Within a notch of no turn at all: the control holds its colour
        // in the field's own coordinates, so a chroma move re-read out of
        // them lands a fraction of a degree from where it started, and a
        // fraction of a degree is what a notch is for.
        assert!(
            s.tone[0].min(360 - s.tone[0]) <= step[0],
            "a chroma change turned the theme by {} degrees",
            s.tone[0]
        );
        assert!(
            (s.tone[1] as i32 - 50).abs() <= 2 * step[1] as i32,
            "half the chroma asked for a multiplier of {}",
            s.tone[1]
        );
        nacelle::theme::clear_preview();
    }

    /// THE BORDER RIDES THE THEME COLOUR PICKER. BASIC has one colour and
    /// the ring is it: moving the one picker moves the border too, with no
    /// second control on the page.
    #[test]
    fn the_border_follows_the_theme_colour_picker_in_basic() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        let seed = s.tone_seeds.expect("BASIC opened without seeds").accent;
        let before = s.edge;

        // A quarter turn at half the chroma — a colour the sRGB picker can
        // actually show, as the arithmetic test beside this one takes it.
        let half = nacelle::theme::color::Oklch { c: seed.c * 0.5, ..seed };
        s.pickers[PickerId::Tone.idx()]
            .set_oklch(nacelle::theme::color::Oklch { h: half.h + 90.0, ..half });
        s.set_tone_from_picker();

        assert_ne!(s.edge, before, "the border did not follow the theme colour picker");
        // The ring's colour reaches the file on the moved accent's hue, not
        // the seed's — read back with the toolkit's own parser.
        let edits = s.editor_edits();
        let ring = edits
            .iter()
            .find(|e| e.token == "border.default")
            .map(|e| e.value.clone())
            .expect("BASIC dropped the ring's colour");
        let got = parse_oklch_literal(&ring);
        let off = (got.h - seed.h).rem_euclid(360.0);
        let off = off.min(360.0 - off);
        assert!(off > 30.0, "the ring stayed near the seed hue {} (got {})", seed.h, got.h);
        nacelle::theme::clear_preview();
    }

    /// The grids: a press picks, and the bank keeps one cell per colour.
    ///
    /// THE FIRST CELL IS THE ACCENT (the master points `picker.base.1` at
    /// it) — a POP colour, standing far up the ladder from the BACKGROUND
    /// BASIC's picker now seeds against (2026-08-19), so pressing it is a
    /// real move and not the no-op it was while the seed was the accent
    /// itself. The seeding test's "no move" sentence now belongs to the
    /// BED (`the_picker_opens_on_the_theme_and_asks_for_no_move`); this
    /// one keeps the OTHER half — a press this control's own grid offers
    /// takes hold and reads back exactly.
    #[test]
    fn the_pickers_grids_choose_a_colour_and_bank_it_once() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        s.perform(Act::EditorMode, 0.0);
        let base = nacelle::object::color_picker::base_colours();
        assert!(base.len() > 2, "the master ships a grid to press");

        s.perform(Act::PickerBase(PickerId::Tone, 0), 0.0);
        assert_ne!(
            s.tone, TONE_REST,
            "pressing the accent, now far from the background, asked for no move"
        );

        // A cell that is NOT the accent moves the theme, and the picker
        // shows exactly what was pressed.
        s.perform(Act::PickerBase(PickerId::Tone, 2), 0.0);
        // Compared as the rounded text a screen can show and a file can
        // spell: the control holds its colour in the field's own
        // coordinates, so a channel may come back a single float ulp
        // away, and an assertion about ulps would be an assertion about
        // nothing anybody can see.
        let spelled = |c: nacelle::theme::Color| {
            nacelle::object::color_picker::write(c, nacelle::object::color_picker::Format::Hsl)
        };
        assert_eq!(
            spelled(s.pickers[PickerId::Tone.idx()].colour()),
            spelled(base[2]),
            "the press did not take the cell's colour"
        );
        assert_ne!(s.tone, TONE_REST, "a different colour asked for no move");

        // The bank: one cell per colour, however many times it is asked
        // for, and the colour banked is the one on screen.
        s.perform(Act::PickerAdd(PickerId::Tone), 0.0);
        s.perform(Act::PickerAdd(PickerId::Tone), 0.0);
        assert_eq!(s.picker_custom.len(), 1, "the bank kept the same colour twice");
        assert_eq!(spelled(s.picker_custom[0]), spelled(base[2]), "the bank kept another colour");
        // And a banked colour can be pressed back.
        s.perform(Act::PickerBase(PickerId::Tone, 0), 0.0);
        s.perform(Act::PickerCustom(PickerId::Tone, 0), 0.0);
        assert_eq!(
            spelled(s.pickers[PickerId::Tone.idx()].colour()),
            spelled(base[2]),
            "the banked colour did not come back"
        );
        nacelle::theme::clear_preview();
    }

    /// NEITHER MODE EATS THE OTHER'S WORK — the owner's condition on the
    /// switch, in both directions.
    #[test]
    fn switching_editor_modes_loses_no_work() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = editor_open();
        // This test drives the ADVANCED -> BASIC -> ADVANCED trip, so it
        // starts on ADVANCED (the editor itself opens on BASIC now).
        s.editor_basic = false;
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
        // AND THE SEVERITY ROLES ARE STILL THE THEME'S. This assertion
        // was its own opposite until 2026-08-18: the fold marked all
        // seven TOUCHED, because BASIC had turned them and ADVANCED
        // writes only touched roles, so the rotation would otherwise
        // have vanished. The cost was that MERELY VISITING BASIC pinned
        // seven literals into every file the editor saved afterwards,
        // and a theme's own `contained` amber was one of the things it
        // overwrote. BASIC does not turn them any more (they lean in the
        // theme, `toward()` on each role's own expression), so there is
        // nothing to hand over and nothing to mark — and the six roles
        // nobody pointed at keep their author's words.
        assert_eq!(
            s.severity_touched,
            [false; 7],
            "the fold pinned severity roles nobody had chosen"
        );
        let folded = s.editor_edits();
        assert_eq!(
            folded.iter().filter(|e| e.token.starts_with("severity.")).count(),
            0,
            "ADVANCED wrote a severity role the user never picked"
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
        //
        // THE GAP WAS 121.0 UNTIL 2026-08-18 and is 117.7 now, and the
        // 3.3 deg is the master's own severity lean arriving: `ok` at 148
        // is 18.5 deg from the mint accent and leans a fifth of that
        // (+3.7), while `critical` at 27 is 139.5 away and is stopped by
        // `severity.pull_clamp` at +7. Both roles moved toward the same
        // place from the same side, so they closed by the difference.
        // This is a CANARY and not a claim about the lean — the claims
        // are libnacelle's, measured over the theme rather than over a
        // trip through this window.
        assert!(
            (start.l - 0.8200).abs() < 0.001 && (start_gap - 117.7).abs() < 0.5,
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
        // The fold happens on the way OUT of BASIC, so start on ADVANCED
        // and let the loop trip BASIC -> ADVANCED under it.
        turned.editor_basic = false;
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

    /// THE OWNER'S SECOND SIGHTING, 2026-08-17: the glow BLINKED, about
    /// five times a second, while a slider was dragged.
    ///
    /// The set the editor lays over the theme has to be a function of the
    /// CONTROLS and of nothing else. `set_preview` REPLACES the set it is
    /// given, so a token left out of a pulse is a token switched off, and
    /// the pulse runs ten times a second — a set that reads the live bake
    /// reads its own previous output and can oscillate. It did: the halo's
    /// radius was written only where the theme had not dressed one, the
    /// theme was asked WITH THE PREVIEW STANDING, and so every second
    /// pulse found the radius it had written a tenth of a second earlier,
    /// called the theme dressed, and dropped it.
    ///
    /// The measurement is the loop itself — build, lay, build — and it is
    /// taken FOUR times over, because the fault has a period of two and a
    /// single repeat could agree by parity. What made this worth a test
    /// rather than a patch is where else that set goes: SAVE writes it,
    /// so a click on the wrong side of the cycle saved a NEON theme with
    /// no radius and no alpha — a glow invisible for good, in a file.
    #[test]
    fn the_editors_edit_set_does_not_read_its_own_preview() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let mut s = editor_open();
        // The ADVANCED pulse comes first, so start on ADVANCED (the editor
        // opens on BASIC now); the later EditorMode press crosses to BASIC.
        s.editor_basic = false;
        // The set that carries a dress question, switched on: a haloed
        // focus ring. (The panel edge's own dress question left with the
        // whole effect, 2026-08-27, the owner's order.)
        s.ring_on = true;
        s.current_ring_style = Some("SOLID".to_string());
        s.ring_halo = true;
        s.ring_halo_alpha = 30;

        /// One pulse of the editor's own loop: the set it would show, and
        /// the showing. Returns the set, so the caller can compare pulses.
        fn pulse(s: &Settings) -> Vec<(&'static str, String)> {
            let edits = s.editor_edits();
            let pairs: Vec<(&str, &str)> =
                edits.iter().map(|e| (e.token, e.value.as_str())).collect();
            let refused = nacelle::theme::set_preview(&pairs);
            assert!(
                refused.is_empty(),
                "the engine refused {refused:?} — a set this build cannot lay is a set \
                 this test is not measuring"
            );
            edits.into_iter().map(|e| (e.token, e.value)).collect()
        }

        let first = pulse(&s);
        // The set really does dress the ring's halo on the master, or the
        // repetition below would be agreeing about nothing.
        for token in ["glow.focus_ring.radius"] {
            assert!(
                first.iter().any(|(t, _)| *t == token),
                "the master's own halo is undressed and the editor did not write {token}"
            );
        }
        let repeats = |s: &Settings, first: &[(&'static str, String)], page: &str| {
            for n in 1..=4 {
                let again = pulse(s);
                let (lost, gained) = (
                    first.iter().find(|e| !again.contains(e)),
                    again.iter().find(|e| !first.contains(e)),
                );
                assert!(
                    again == first,
                    "{page}: pulse {n} laid a different set over the same controls — \
                     dropped {lost:?}, added {gained:?}"
                );
            }
        };
        repeats(&s, &first, "ADVANCED");

        // And BASIC, which is the page the owner was dragging on. Its ten
        // authors are a move measured from seeds taken off the live bake,
        // so a set that moved by its own last move would climb instead of
        // blinking — the same fault wearing another face.
        s.perform(Act::EditorMode, 0.0);
        assert!(s.editor_basic, "the switch did not reach BASIC");
        s.tone[0] = 40;
        let first = pulse(&s);
        repeats(&s, &first, "BASIC");
        nacelle::theme::clear_preview();
    }

    /// And the reason the dress question is asked at all is intact: a
    /// theme that has dressed its own halo KEEPS ITS OWN NUMBERS.
    ///
    /// That was `theme::edit`'s own finding — writing this window's seeds
    /// over five themes' dress flattened all five — and the fix for the
    /// blink must not become a quiet undoing of it. A dressed theme is
    /// staged here with the preview, because the editor reads its opening
    /// state off the live bake and this is the one way to give it a bake
    /// that is not the master's without a file on disk.
    ///
    /// WHAT THIS PINS IS THE OVERLAY. An edit set laid over a bake keeps
    /// whatever it does not mention, so saying nothing about a radius is
    /// how a dress survives.
    ///
    /// THE HOLE THIS DOC USED TO DESCRIBE IS CLOSED ON THE OTHER SIDE OF
    /// THE BOUNDARY, and it closed the other way round from the guess
    /// written here. The guess was that `edit.rs` would have to hand the
    /// dressed numbers back for a save, because a FILE keeps nothing it
    /// does not mention. What was wrong was the second half:
    /// `nacelle::theme::save_theme` regenerated the file whole, and the
    /// 2026-08-18 change makes it PATCH — the values the set names are
    /// replaced where they stand and every other byte, comments included,
    /// is left alone, in the theme's OWN file whatever name the save
    /// lands under (`save_theme_as`). A file keeps what it is not told
    /// about, exactly like a bake, so the set below is right for both and
    /// the assertions did not have to change after all. The owner's
    /// report that forced it: "the halo does not blink any more, but it
    /// disappears when I press save."
    ///
    /// WHAT THIS TEST CAN AND CANNOT SEE. It is the OVERLAY's contract,
    /// and that is all it runs: this crate pins `libnacelle` by git commit
    /// (`Cargo.toml`), so until that pin is bumped past the save change,
    /// the library this file is compiled against is the one that still
    /// regenerates. The paragraph above is a statement about libnacelle's
    /// branch, not about this crate's build, and the two only become the
    /// same statement in the merge order the plan fixes: libnacelle, then
    /// the pin, then here. The file half is pinned where it lives —
    /// `libnacelle/tests/theme_save_patch.rs` and `theme_save_as.rs`.
    #[test]
    fn a_theme_that_dressed_its_own_halo_keeps_its_numbers() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        nacelle::theme::clear_preview();
        let dress = [
            ("glow.focus_ring.radius", "2.40u"),
            ("glow.focus_ring.alpha", "0.500"),
        ];
        assert!(nacelle::theme::set_preview(&dress).is_empty(), "the dress was refused");
        let mut s = editor_open();
        nacelle::theme::clear_preview();
        s.ring_on = true;
        s.current_ring_style = Some("SOLID".to_string());
        s.ring_halo = true;
        s.ring_halo_alpha = 30;
        let edits = s.editor_edits();
        for token in ["glow.focus_ring.radius"] {
            assert!(
                !edits.iter().any(|e| e.token == token),
                "the editor wrote its own {token} over a theme that had dressed one"
            );
        }
        // The switch itself still lands — keeping a dress is not the same
        // as leaving the theme alone.
        assert!(
            edits.iter().any(|e| e.token == "glow.focus_ring.enabled" && e.value == "true"),
            "the haloed ring did not switch its glow on"
        );
    }

    /// And WHICH theme that dress belongs to, which is this window's half
    /// of the same question.
    ///
    /// A save patches a file, and the set above is silent about the dress,
    /// so the file it is laid against decides what the dress ends up being.
    /// Hand the save the name it is WRITING and a SAVE AS onto a taken name
    /// answers that silence out of somebody else's theme. Hand it the theme
    /// the editor has OPEN and the answer is the one on screen. What is
    /// pinned here is that this window names the second one — and the three
    /// cases where there is no file to name, each of which means the person
    /// is looking at the master.
    #[test]
    fn a_save_is_a_save_of_the_theme_the_editor_has_open() {
        let known: Vec<String> =
            ["default", "cockpit", "azure"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            Settings::editor_source_of(Some("cockpit"), &known).as_deref(),
            Some("cockpit"),
            "the editor could not name the theme it has open, so SAVE AS \
             would be laid against whatever file the new name already had"
        );
        // The master is compiled in; there is no file to copy.
        assert_eq!(Settings::editor_source_of(Some("default"), &known), None);
        // Nothing configured at all — the master again, by omission.
        assert_eq!(Settings::editor_source_of(None, &known), None);
        // A `theme:` naming a file that is gone. The loader already fell
        // back to the master, so the master is what is on screen, and
        // pointing a save at the missing name would resurrect it.
        assert_eq!(Settings::editor_source_of(Some("skasowany"), &known), None);
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
    /// leaves the interface ONE HUE in DIFFERENT SHADES — the page's bed
    /// under the one bed both navigation columns share (owner,
    /// 2026-08-18), and both of those a long way under a button's plate.
    ///
    /// libnacelle measures the same claim over the master's cascade;
    /// this measures the WINDOW's chain — slider, `editor_edits`,
    /// `set_preview`, the bake, and the colour that comes back out of a
    /// token the settings columns are painted with. Between them there
    /// is no step where a hue could be lost and nobody notice.
    ///
    /// The owner's own pair: a column's CONTAINER and a control's PLATE.
    ///
    /// AND THE COLUMNS ARE NOT BLACK WHEN THE THEME IS NOT BLACK. A HUE
    /// drag is a ROTATION, so this test could turn the whole interface
    /// and never notice that the navigation columns were the sRGB codes
    /// 6 and 19 — which is what the owner photographed on 2026-08-17 and
    /// what a hue check structurally cannot see. `off_black` is the
    /// second ruler; the gate is on the LIGHTNESS slider standing still,
    /// because dragged to its floor BASIC really may take the whole
    /// interface down to a black theme and a black theme's beds are
    /// allowed to be black.
    ///
    /// EVERY READING IS DECODED FIRST. "One hue at three lightnesses" is
    /// a claim about OKLCh, OKLCh is defined over LINEAR light, and the
    /// bake answers sRGB-encoded — so a reading taken without
    /// `to_linear` measures a different quantity and the sentence stops
    /// meaning what it says. On the master's own bands it is not a
    /// rounding either: the three lightnesses read 0.2320 / 0.2784 /
    /// 0.3341 decoded, where the page's own bed alone reads 0.4840
    /// encoded — twice its decoded number — and where, read encoded, the
    /// hue the three are supposed to SHARE spreads nearly three degrees
    /// instead of standing on one.
    #[test]
    fn a_basic_hue_drag_turns_the_window_and_keeps_one_hue_across_its_columns() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut s = editor_open();

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

        let before = lch(live("component.settings.rail_fill"));
        // FOUR positions of the slider, not one: a claim that only holds
        // where the numbers happen to land is not the claim.
        for turn in [37u32, 90, 180, 251] {
            s.tone = TONE_REST;
            s.tone[0] = turn;
            s.apply_editor_preview();

            let rail = lch(live("component.settings.rail_fill"));
            // The page's band is the window BODY's own token. The master
            // ships `settings.page_fill` as the sentinel and anchors the
            // rail to this one, so this is both the page's bed and the
            // number the rail is measured from.
            let page = lch(live("component.panel.fill"));
            // The window really turned, by the slider's own degrees.
            assert!(
                hue_gap(rail.h, before.h + turn as f32) < 6.0,
                "at {turn} deg the navigation did not follow the slider: {} -> {}",
                before.h,
                rail.h
            );
            // ONE HUE for the whole interface — the columns and the
            // plate a button stands on, which is the owner's own pair.
            let plate = lch(col(theme::resolved()
                .class_state(theme::class_id("button").expect("no button class"), State::Idle)
                .fill));
            assert!(
                hue_gap(rail.h, plate.h) < 6.0,
                "at {turn} deg a column and a button plate are two COLOURS: {} vs {}",
                rail.h,
                plate.h
            );
            // DIFFERENT SHADES — the container is a bed and the plate is
            // a control, and no reader may have to guess which is which.
            // Measured on the master, decoded: 0.1780 against 0.8200.
            assert!(
                (plate.l - rail.l).abs() > 0.40,
                "at {turn} deg a column and a button plate share a lightness: {} vs {}",
                rail.l,
                plate.l
            );
            // And the navigation is still one step above the page: the
            // page is the well and the navigation the rim.
            assert!(
                page.l < rail.l,
                "at {turn} deg the navigation stopped standing off the page: {} {}",
                page.l,
                rail.l
            );
            // AND NOT ONE OF THEM IS BLACK. The rotation this test drives
            // cannot darken anything, so a black band here is a black
            // band in the theme — the owner's screenshot, and the fault
            // no hue assertion above could ever have seen.
            for (name, c) in [
                ("rail", live("component.settings.rail_fill")),
                ("page", live("component.panel.fill")),
            ] {
                let black = nacelle::theme::Color::from_hex("#000000").expect("black");
                let off = nacelle::theme::Color::wcag_contrast(c.to_linear(), black.to_linear());
                // The floor libnacelle's band test sets and states: between
                // the master's own body rung (1.26) and `@surface.base`
                // (1.12), the rung a column used to be pinned to.
                assert!(
                    off >= 1.15,
                    "at {turn} deg the {name} column reads {off} against pure black — \
                     the window turned but one of its columns is a black stripe"
                );
            }
            // ONE hue between themselves, and this is the assertion the
            // SPACE is load-bearing for. The RAIL takes its h from the
            // one token (`@surface.hue`): 203.46 at the first position.
            // The BODY lands a quarter-degree off it (203.22) and not on
            // it, because its colour is not a reference — the BACKGROUND
            // section holds it on integer HSV sliders, and BASIC's hue
            // is carried onto it by `Tone::shift`. One notch of that
            // slider is the finest the body's bed can be stated at, and
            // 0.24 deg is well inside one notch.
            //
            // Read ENCODED instead of decoded the two spread nearly
            // three degrees — six times the quantisation and the thing
            // this tolerance is really here to catch, because a reader
            // could not tell that apart from a real drift.
            assert!(
                hue_gap(rail.h, page.h) < 0.5,
                "at {turn} deg the two bands are on two hues: {} {}",
                rail.h,
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

    /// THE OTHER HALF OF THE OWNER'S SCREENSHOT: the editor's BACKGROUND
    /// section moves the WHOLE window, and the navigation goes with it.
    ///
    /// WHAT WENT WRONG. BACKGROUND writes the window body as an ABSOLUTE
    /// colour off its own sliders — `component.panel.fill` on SOLID
    /// (`edit::glass_edits`) — while the navigation columns were pinned
    /// to rungs of the surface ladder that no slider on that page
    /// touches. Drag the background and the page turned; the columns did
    /// not. What the owner photographed was a window in two colours,
    /// which is the same fault as the black stripes and not a second one.
    ///
    /// WHERE IT IS FIXED, and why nothing here does the fixing. The
    /// master anchors both beds to ONE token — the body — so the rail
    /// follows it by construction, in the theme, and this window keeps
    /// carrying names to the theme and painting back what it is given.
    /// This test is the proof of the whole chain: slider,
    /// `editor_edits`, `set_preview`, bake, and the colours that come
    /// back out.
    ///
    /// AND IT GUARDS THE SECTION IT LEANS ON. SOLID, BLUR and FROSTED
    /// GLASS each still do their own job, because a fix to the columns
    /// that quietly cost the window its glass would be a worse trade
    /// than the fault.
    #[test]
    fn the_background_section_moves_both_columns_at_once() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        nacelle::theme::clear_preview();

        let lch = |c: nacelle::theme::Color| c.to_linear().to_oklch();
        let hue_gap = |a: f32, b: f32| {
            let d = (a - b).rem_euclid(360.0);
            d.min(360.0 - d)
        };
        fn live(name: &str) -> nacelle::theme::Color {
            let t = theme::resolved();
            col(t.color(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}"))))
        }
        fn live_px(name: &str) -> f32 {
            let t = theme::resolved();
            t.px(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}")))
        }

        let master_hue = lch(live("component.panel.fill")).h;
        let mut s = editor_open();
        // ADVANCED: WASH is its own control, not BASIC's. The editor now
        // opens on BASIC by default (2026-08-19), whose Tone picker would
        // otherwise answer for `component.panel.fill` itself and leave
        // this drag looking like it did nothing — this test is about the
        // OTHER page's slider, so it says so.
        s.editor_basic = false;
        // SOLID, with the background's own colour dragged a long way off
        // the master's mint: the HSV track is brightness, saturation, hue,
        // and 20 deg of HSV is nowhere near where this theme stands.
        s.current_background = Some("SOLID".to_string());
        s.wash = [70, 60, 20];
        s.bg_opacity = 100;
        s.apply_editor_preview();

        let page = lch(live("component.panel.fill"));
        let rail = lch(live("component.settings.rail_fill"));

        // The body really left the theme it opened on — or the assertion
        // below would be comparing two colours that never moved.
        assert!(
            hue_gap(page.h, master_hue) > 20.0,
            "the BACKGROUND slider did not move the body at all: {} vs {}",
            master_hue,
            page.h
        );
        // AND BOTH COLUMNS WENT WITH IT. This is the divergence,
        // measured: pinned to the ladder the navigation would have
        // stayed on the master's hue while the page took the slider's.
        assert!(
            hue_gap(rail.h, page.h) < 2.0,
            "the navigation stayed behind while BACKGROUND moved the page: {} vs {}",
            rail.h,
            page.h
        );
        // Still one step above the page, still not black.
        assert!(
            rail.l - page.l > 0.03,
            "the navigation flattened onto the page it lies on: {} {}",
            page.l,
            rail.l
        );
        let black = nacelle::theme::Color::from_hex("#000000").expect("black");
        for (name, c) in [
            ("page", live("component.panel.fill")),
            ("rail", live("component.settings.rail_fill")),
        ] {
            let off = nacelle::theme::Color::wcag_contrast(c.to_linear(), black.to_linear());
            assert!(off >= 1.15, "the {name} column came out black: {off}");
        }

        // THE SECTION STILL DOES ITS OWN JOB. BLUR raises the pyramid
        // rank and leaves the wash empty — the master's own `none`, alpha
        // 0 — and FROSTED raises the rank AND lays a wash over it. If
        // either had been lost to this fix the window would have gained
        // three shades and given up its glass.
        s.current_background = Some("BLUR".to_string());
        s.apply_editor_preview();
        assert!(live_px("elev.panel.glass.rank") > 0.0, "BLUR stopped blurring");
        assert_eq!(
            live("elev.panel.glass.wash").a,
            0.0,
            "BLUR laid a wash; the tint alone is what BLUR is"
        );
        s.current_background = Some("FROSTED GLASS".to_string());
        s.bg_coverage = 42;
        s.apply_editor_preview();
        assert!(live_px("elev.panel.glass.rank") > 0.0, "FROSTED stopped blurring");
        assert!(
            live("elev.panel.glass.wash").a > 0.0,
            "FROSTED lost its wash; a frosted pane you can read through is a blur"
        );

        nacelle::theme::clear_preview();
        viewport_home();
    }

    /// THE BLUR RADIUS LEAVES THIS PAGE THE WAY THE APPLICATION READS IT.
    ///
    /// The chain is slider -> `Settings::blur_radius` -> `blur_dirty` ->
    /// `blur_settings()` -> `Screen::set_blur_radius` -> `Gfx`, and only
    /// the first four links can be walked here. WHAT CANNOT BE TESTED
    /// WITHOUT A DEVICE, stated rather than skipped: `Gfx::set_blur_radius`
    /// turns the percentage into the pyramid depth and `Gfx::glass_ranks`
    /// reports it, and both live on a struct that cannot be built without
    /// a Vulkan device — so "the percentage became three downsample
    /// passes" is a claim only a screen can settle. What is settled here
    /// is that the page hands on the RADIUS and not the opacity beside
    /// it, and that moving either track raises the flag the frame loop
    /// watches; without that flag the value sits in this struct and the
    /// renderer never hears about it.
    ///
    /// Verified 2026-08-18 by reading the two ends as well: the sliders'
    /// `save` closures call `config::set_blur_radius` / `_opacity`, and
    /// `main.rs` calls `sc.set_blur_radius(radius)` from
    /// `blur_settings()` under `blur_dirty` — the chain is whole. This
    /// test is the guard on it, not the discovery of a fault in it.
    #[test]
    fn the_blur_radius_leaves_the_page_as_the_radius() {
        let _g = crate::widgets::theme_test_lock();
        let mut s = furnished();
        s.blur_radius = 0;
        s.blur_opacity = 0;
        s.blur_dirty = false;

        // Through the slider's own description, so the test cannot set a
        // field the control does not reach.
        let set_track = |s: &mut Settings, act: Act, v: u32| {
            let Some(&Ctrl::Slider { set, .. }) = slider_of(act) else {
                panic!("a blur track stopped being a slider");
            };
            set(s, v);
            s.mark_dirty(act);
        };
        set_track(&mut s, Act::BlurRadiusTrack, 70);
        set_track(&mut s, Act::BlurOpacityTrack, 25);

        assert!(s.blur_dirty, "the tracks moved and the frame loop was not told");
        assert_eq!(
            s.blur_settings(),
            (70, 25),
            "the page handed the application something other than (radius, opacity)"
        );
    }

    /// A SURFACE THAT TURNS TO GLASS OVER A COLOURED BED IS NOT BLACK.
    ///
    /// THE FAULT. `elev.*.glass.tint` MULTIPLIES the blurred scene and
    /// can only darken — the ladder's own head states it — so whatever
    /// stands at that key decides how much light a frosted surface is
    /// allowed to keep. This page seeded the tint group only where the
    /// theme ALREADY had glass, which no shipped theme does, so the
    /// three tint sliders opened on a triple written in Rust and the
    /// first press of BACKGROUND -> BLUR wrote it into the file:
    /// measured HSV 60/20/210 -> sRGB(0.480, 0.540, 0.600), 46% of the
    /// light gone before the frost had any. It reached `elev.popover`
    /// with `elev.panel`, so menus and tooltips went dark with the
    /// windows — "the background of every object is black".
    ///
    /// THE CLAIM, and it is the theme's and not a number of this test's:
    /// the editor may not INVENT a tint. Every rung of the master's
    /// ladder writes `#FFFFFF / 1.0` at that key, the identity multiply,
    /// which is exactly what `edit::Glass::Blur` means by "the tint left
    /// neutral" — so what BLUR writes has to be what the theme was
    /// already carrying, and the frost over a bed has to keep that bed's
    /// light rather than a fraction of it.
    ///
    /// The bed is `backdrop.solid`, because that is what a frosted
    /// surface on this desktop actually samples: the ground the frame
    /// lays under everything, blurred.
    #[test]
    fn a_glassed_surface_keeps_the_light_of_the_bed_it_frosts() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        nacelle::theme::clear_preview();
        fn live(name: &str) -> nacelle::theme::Color {
            let t = theme::resolved();
            col(t.color(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}"))))
        }

        // What the FILE carries at the key, before any control is
        // touched — the value the page has to come back with.
        let theme_tint = live("elev.panel.glass.tint");
        let bed = live("backdrop.solid");

        // Opened by hand rather than through `editor_open`, with a tint
        // on the tracks that is nobody's: the claim is that the SEEDING
        // fetches the theme's value, and a page that started on a
        // placeholder agreeing with the file would prove that whether it
        // fetched anything or not.
        let mut s = furnished();
        s.tint = [42, 77, 300];
        s.view = View::ThemeEditor;
        s.seed_editor_from_theme();
        // The seeding put the theme's own tint on the tracks, and that
        // is ALL this first claim settles: the tracks are compared with
        // the same map the seeding runs, so it answers "the seeding ran,
        // and it read this key" and nothing about whether sRGB -> HSV ->
        // whole slider units is a faithful trip. It is here to say WHICH
        // key went missing when it fails. The map itself is under the
        // claim below, which walks the value out through the file's own
        // spelling and back off the bake.
        assert_eq!(
            s.tint,
            hsv_track_of(theme_tint),
            "the tint sliders opened on a colour this program invented \
             instead of on the one the theme carries at elev.panel.glass.tint"
        );

        s.current_background = Some("BLUR".to_string());
        s.apply_editor_preview();

        // AND THE SAME COLOUR CAME BACK OUT, on both rungs the model
        // writes: a menu over a frosted window is the same material.
        for rung in ["elev.panel", "elev.popover"] {
            let written = live(&format!("{rung}.glass.tint"));
            for (ch, was, now) in [
                ('r', theme_tint.r, written.r),
                ('g', theme_tint.g, written.g),
                ('b', theme_tint.b, written.b),
            ] {
                assert!(
                    (was - now).abs() < 0.01,
                    "BLUR wrote a {rung} tint of its own: {ch} {now} against the \
                     theme's {was}"
                );
            }
            // THE PICTURE, stated as the fault was: the frost is the bed
            // times the tint, and it may not come out darker than the
            // bed it frosts. Against black, because black is what the
            // owner reported seeing.
            let black = nacelle::theme::Color::from_hex("#000000").expect("black");
            let frost = nacelle::theme::Color {
                r: bed.r * written.r,
                g: bed.g * written.g,
                b: bed.b * written.b,
                a: 1.0,
            };
            let off = nacelle::theme::Color::wcag_contrast(
                frost.to_linear(),
                black.to_linear(),
            );
            let bed_off = nacelle::theme::Color::wcag_contrast(
                bed.to_linear(),
                black.to_linear(),
            );
            assert!(
                off >= bed_off - 0.001,
                "a {rung} surface turned to glass came out darker than the bed it \
                 frosts: {off} against the bed's {bed_off}"
            );
        }

        nacelle::theme::clear_preview();
        viewport_home();
    }

    /// THE BACKGROUND SECTION REOPENS ON THE THEME IT SAVED, DOWN TO THE
    /// LAST KNOB — colours and amounts alike.
    ///
    /// THE FAULT, and it is the tint's fault one control to the right.
    /// The wash slot serves two keys — SOLID writes it to
    /// `component.panel.fill`, FROSTED to `elev.*.glass.wash` — and it
    /// was seeded only where the kind that writes it was already the
    /// kind the theme wore. The case in between is not a curiosity: it
    /// is exactly the file THIS EDITOR SAVES when the owner picks BLUR,
    /// which writes a rank and `wash = none` by its own definition. Open
    /// that theme and the three wash sliders held 20/15/210 out of
    /// `Settings::new`; press FROSTED GLASS and that violet — a colour
    /// in no theme in this repository — became the body of every panel,
    /// menu and tooltip in the program.
    ///
    /// The same hole ran under the three AMOUNTS, which were never
    /// seeded at all: OPACITY, BLUR DEPTH and WASH COVERAGE opened on
    /// 100, 50 and 42 whatever the file said, against this page's own
    /// promise that "a theme saved and reopened lands the sliders where
    /// the hand left them".
    ///
    /// THE CLAIM, in the theme's terms and not this test's numbers: what
    /// the editor writes for a theme nobody touched is what that theme
    /// already carried. So the trip below is the owner's — a theme wearing
    /// a rank with no wash is opened, FROSTED GLASS is pressed, and the
    /// body that lands in the file has to be the body the theme declares
    /// at `component.panel.fill` — and then the whole page is reopened on
    /// its own output, which may not move a single track.
    #[test]
    fn the_background_page_opens_on_the_theme_a_blur_preset_saves() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        nacelle::theme::clear_preview();
        fn live(name: &str) -> nacelle::theme::Color {
            let t = theme::resolved();
            col(t.color(nacelle::theme::id(name).unwrap_or_else(|| panic!("no {name}"))))
        }

        // A THEME SAVED BY THIS EDITOR'S OWN BLUR PRESET: a rank, a
        // neutral tint carrying the opacity, and the word `none` at the
        // wash — `edit::glass_edits` writes exactly these three keys for
        // `Glass::Blur`. The rank and the alpha are deliberately NOT the
        // openings the struct carries (2.0 and 1.0), or the amounts would
        // agree by accident of never having been read.
        let refused = nacelle::theme::set_preview(&[
            ("elev.panel.glass.rank", "2.60"),
            ("elev.panel.glass.tint", "oklch(1.0000, 0.0000, 0.00 / 0.600)"),
            ("elev.panel.glass.wash", "none"),
        ]);
        assert!(refused.is_empty(), "the theme would not wear a saved BLUR: {refused:?}");
        let body = live("component.panel.fill");

        let mut s = furnished();
        // FROSTED laying a wash on EVERY rung (elev.popover included) is
        // ADVANCED's reach; the editor opens on BASIC, whose reach is the
        // body alone, so this test asks its question on ADVANCED.
        s.editor_basic = false;
        // Nobody's violet on the tracks, so a page that fetched nothing
        // cannot pass by having started somewhere plausible.
        s.wash = [42, 77, 300];
        s.view = View::ThemeEditor;
        s.seed_editor_from_theme();

        assert_eq!(
            s.current_background.as_deref(),
            Some("BLUR"),
            "a rank with no wash is what BLUR means, and the page did not read it back"
        );
        // WHICH KEY: the same map the seeding runs, so this settles that
        // the seeding read the body and nothing about the map itself —
        // the map is under the claim that follows.
        assert_eq!(
            s.wash,
            hsv_track_of(body),
            "the wash sliders opened on a colour this program invented instead of \
             on the body the theme carries at component.panel.fill"
        );
        // THE AMOUNTS CAME BACK TOO. 2.60 is `1 + track / 50` at 80, and
        // the tint's alpha is what OPACITY writes.
        assert_eq!(s.bg_depth, 80, "BLUR DEPTH reopened on a number out of Rust");
        assert_eq!(s.bg_opacity, 60, "OPACITY reopened on a number out of Rust");

        // THE OWNER'S PRESS. FROSTED GLASS is the one preset that lays a
        // wash, and what it lays has to be the theme's own body.
        s.current_background = Some("FROSTED GLASS".to_string());
        s.apply_editor_preview();
        for rung in ["elev.panel", "elev.popover"] {
            let written = live(&format!("{rung}.glass.wash"));
            assert!(written.a > 0.0, "FROSTED laid no wash on {rung}");
            for (ch, was, now) in [
                ('r', body.r, written.r),
                ('g', body.g, written.g),
                ('b', body.b, written.b),
            ] {
                assert!(
                    (was - now).abs() < 0.02,
                    "FROSTED washed {rung} in a colour of its own: {ch} {now} against \
                     the theme's body {was}"
                );
            }
        }

        // AND THE PAGE REOPENS ON ITS OWN OUTPUT WITHOUT MOVING. This is
        // the promise at `seed_editor_from_theme`'s head, asked of the
        // whole section at once: kind, both colours, all three amounts.
        let before = (s.wash, s.tint, s.bg_opacity, s.bg_depth, s.bg_coverage);
        s.seed_editor_from_theme();
        assert_eq!(
            s.current_background.as_deref(),
            Some("FROSTED GLASS"),
            "the kind the page just wrote is not the kind it reads back"
        );
        assert_eq!(
            (s.wash, s.tint, s.bg_opacity, s.bg_depth, s.bg_coverage),
            before,
            "a theme saved and reopened did not land the sliders where the hand \
             left them"
        );

        nacelle::theme::clear_preview();
        viewport_home();
    }

    /// The BASIC sliders notch by what the PIPELINE can show, and the
    /// depth that says so comes off SETTINGS -> COLOR.
    ///
    /// AND PAST TWELVE BITS THE TRACK IS THE LIMIT, WHICH IS NOT A DEFECT.
    /// On the master's seed the notches divide by the BACKGROUND's chroma
    /// now (2026-08-19 — `component.panel.fill`, 0.01853, not the accent's
    /// 0.1531: `Settings::set_tone_from_picker` measures against the bed,
    /// so the grid that avoids re-baking indistinguishable tones has to be
    /// fine relative to what is being divided by), which is roughly an
    /// eighth the accent's own — so the SAME bit depth buys a coarser grid
    /// than it used to; the pipeline's own notch is
    ///
    /// ```text
    ///    8 bits   12.12 deg   0.2116   0.003922   ->  12, 21, 2 units
    ///   10 bits    3.02 deg   0.0527   0.000978   ->   3,  5, 1
    ///   12 bits    0.75 deg   0.0132   0.000244   ->   1,  1, 1
    ///   16 bits    0.05 deg   0.0008   0.000015   ->   1,  1, 1
    /// ```
    ///
    /// — a degree of hue, a percent of saturation and a fiftieth of the
    /// lightness span are what these tracks HAVE, so from twelve bits up
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
        // it is named here — the BACKGROUND's, not the accent's, since
        // 2026-08-19: `component.panel.fill`, read in linear light.
        assert!(
            (s.tone_bed.c - 0.01853).abs() < 0.001,
            "the bed's chroma moved to {} and the notches below with it",
            s.tone_bed.c
        );

        let mut last = [0u32; 3];
        for (i, (bits, want)) in
            [(8u32, [12u32, 21, 2]), (10, [3, 5, 1]), (12, [1, 1, 1]), (16, [1, 1, 1])]
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

    /// A theme under which this window ALWAYS folds, whatever height it
    /// is drawn at.
    ///
    /// THE MASTER NO LONGER REACHES THE FOLD BY BEING SHORT, and that is
    /// the change rather than a hole in the tests. Two navigation
    /// columns took 44 % of the content box and a 500 px screen could
    /// not seat them and a page as well; ONE column takes 22 %, so every
    /// height the program is built for keeps its columns. The folded
    /// shape is still there — a genuinely narrow window still reaches
    /// it — and still has to be measured, so the tests that measure it
    /// ask for it the way the rule is written: the threshold is
    /// `settings.col_min_w`, the threshold is the THEME's, and a theme
    /// that wants a wider page than any of these windows can give folds
    /// every one of them.
    ///
    /// It folds the columned BANDS with it, through the same token —
    /// which is the point of `col_min_w` having one reader for both:
    /// "there is no room" means one thing in this window and not two.
    fn folding_theme() -> crate::widgets::Themed {
        crate::widgets::Themed::new(
            "always-folds",
            "[settings]\ncol_min_w_min_px = 4000px\n",
        )
    }

    /// The offsets a sweep has to drive the RAIL through to have seen
    /// all of it, on exactly the rule the page's stops are built with:
    /// half a box at a time, so consecutive stops overlap, and the
    /// clamp's own far end last.
    ///
    /// EMPTY WHERE THE RAIL FITS OR THERE IS NO RAIL, because then the
    /// stops the page is already walked with have shown the whole of it
    /// — a folded window has no column at all and its entries are bands
    /// of the flow.
    fn rail_stops(s: &Settings, m: Metrics, content: Rect) -> Vec<f32> {
        let Some(rail) = Panes::of(m, content).rail else { return Vec::new() };
        let length = s.rows_h(&RAIL_ROWS, m.rail(), rows_box(rail.rows));
        if length <= rail.rows.h {
            return Vec::new();
        }
        let stride = (rail.rows.h * 0.5).max(1.0);
        let mut out = Vec::new();
        let mut at = stride;
        while at < length {
            out.push(at);
            at += stride;
        }
        out.push(f32::MAX / 4.0);
        out
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
                let nav = Panes::of(m, content);
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
                    for ((region, rows), dy) in zone_regions(zone, band)
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

    // --------------------------------------------------- the page's bar

    /// The FONT page drawn at 500 px, once — a page that does not fit,
    /// which is the only state a scrollbar exists in — with the pointer
    /// nowhere near it.
    fn long_page(fonts: &mut nacelle::font::FontSystem) -> Settings {
        let mut s = furnished();
        s.view = View::Font;
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, fonts, PAGE_H, 1.0);
        s.draw(&mut ctx);
        assert!(
            s.flow.length > s.flow.view.h,
            "the FONT page ought not to fit at {PAGE_H} px — there would be no bar to press"
        );
        s
    }

    /// The bar AS THE PRESS AIMS WITH IT: `hovered = true`, the widest
    /// the lane can be, which is what a hand reaching for the thumb has
    /// under it. Off the flow the last frame left behind, which is the
    /// one thing the press has to go on.
    fn bar_geom(s: &Settings) -> scroll::ScrollbarGeom {
        scroll::scrollbar(
            s.flow.view,
            &ScrollbarLook::from_theme(),
            s.scroll.offset(),
            s.flow.view.h,
            s.flow.length,
            true,
        )
        .expect("a page longer than its box has a bar")
    }

    /// The window height every test of the page's bar is taken at, and
    /// the width a frame gives it ([`probe`]).
    const PAGE_H: f32 = 500.0;
    const PAGE_W: f32 = PAGE_H * 16.0 / 9.0;

    /// The owner's report, in one gesture: press the thumb, move, let go.
    ///
    /// The bar was drawn and never asked — [`draw_bar`] worked out a
    /// geometry, painted it and threw it away, so nothing on screen
    /// carried the press. The model has had the whole gesture since
    /// F2 (`view::scroll`), and the open list's thumb was already using
    /// it in this very file; only the page's bar was left an indicator.
    #[test]
    fn the_pages_thumb_takes_the_hand_and_the_page_follows() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = long_page(&mut fonts);
        let geom = bar_geom(&s);
        let grab = geom.thumb.y + geom.thumb.h / 2.0;
        assert!(
            !s.click(geom.thumb.cx(), grab, PAGE_W, PAGE_H, None),
            "a press on a scrollbar reported a configuration change"
        );
        assert!(s.scroll.dragging(), "the press on the thumb took no hold");
        let before = s.scroll.offset();
        // A quarter of the track down, and sideways off the lane on the
        // way: a hand that wanders out of the bar is still holding it.
        s.drag(0.0, grab + geom.track.h / 4.0);
        let after = s.scroll.offset();
        assert!(
            after > before,
            "the held thumb moved a quarter of its track and the page stayed at {before}"
        );
        assert!(!s.release(), "letting a scrollbar go changed the configuration");
        assert!(!s.scroll.dragging(), "the thumb was never let go");
        // And nothing follows the pointer once it holds nothing.
        let parked = s.scroll.offset();
        s.drag(0.0, grab + geom.track.h / 2.0);
        assert_eq!(
            s.scroll.offset(),
            parked,
            "the page followed a pointer that was holding nothing"
        );
        viewport_home();
    }

    /// A press on the TRACK beside the thumb pages one viewport toward
    /// it — the toolkit's own word on a track click
    /// ([`nacelle::view::scroll::ScrollView::page`]), and the same
    /// answer the open list's bar and the filesystem's overlay give.
    #[test]
    fn a_press_on_the_track_beside_the_thumb_pages_toward_it() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = long_page(&mut fonts);
        let geom = bar_geom(&s);
        let below = geom.track.bottom() - 1.0;
        assert!(
            below > geom.thumb.bottom(),
            "the thumb fills its track; there is no track left to press"
        );
        let (before, viewport) = (s.scroll.offset(), s.flow.view.h);
        assert!(!s.click(geom.thumb.cx(), below, PAGE_W, PAGE_H, None));
        assert!(
            !s.scroll.dragging(),
            "a press BESIDE the thumb took hold of the thumb"
        );
        let after = s.scroll.offset();
        assert!(
            (after - before - viewport).abs() < 0.5,
            "a press below the thumb moved the page from {before} to {after}, \
             and one viewport is {viewport}"
        );
        viewport_home();
    }

    /// The bar says the thumb is HELD — told apart from resting and from
    /// merely pointed at, with the pointer nowhere near it.
    ///
    /// Three frames and two witnesses, and every number compared against
    /// is the theme's own rather than one written here. The width comes
    /// from `scrollbar.w` against `scrollbar.w_hover`: a thumb that
    /// thinned out from under the hand would be a defect the eye could
    /// see. The fill comes from the `scrollbar.thumb` class's ladder,
    /// where the master writes `state.idle.fill`, `state.hover.fill` and
    /// `state.dragging.fill` as three different alphas over one base —
    /// so a held thumb drawn in the HOVER ink fails here just as loudly
    /// as one drawn in the resting ink, and only the drag rung passes.
    ///
    /// The rung changes land at once because every frame is taken at the
    /// same instant: `motion::state_mix` fades between rungs across a
    /// frame BOUNDARY and jumps when there is none.
    #[test]
    fn the_bar_says_a_held_thumb_is_held() {
        let _g = crate::widgets::theme_test_lock();
        viewport_home();
        let mut fonts = nacelle::font::FontSystem::new();
        let look = ScrollbarLook::from_theme();
        assert!(
            look.w_hover > look.w,
            "this theme draws one width for both rungs; the measurement means nothing"
        );
        let mut s = long_page(&mut fonts);
        // An auto-hiding bar starts HIDDEN and is painted only once the
        // page has moved (`ScrollView::last_move_t`) — which is how a
        // person meets it in any case: the wheel first, the hand second.
        let at = on_the_page(&s);
        s.wheel(-1.0, at.0, at.1);
        let (rest_w, rest_fill) = drawn_thumb(&mut s, &mut fonts, None);
        assert!(
            (rest_w - look.w).abs() < 0.5,
            "the resting bar is {rest_w} px wide, and scrollbar.w is {}",
            look.w
        );
        let geom = bar_geom(&s);
        let on_thumb = (geom.thumb.cx(), geom.thumb.y + geom.thumb.h / 2.0);
        // Pointed at and not held: the rung the drag rung has to be told
        // apart from, and the one a bar that merely widened would sit on.
        let (hover_w, hover_fill) = drawn_thumb(&mut s, &mut fonts, Some(on_thumb));
        assert!(
            (hover_w - look.w_hover).abs() < 0.5,
            "the hovered bar is {hover_w} px wide, and scrollbar.w_hover is {}",
            look.w_hover
        );
        s.click(on_thumb.0, on_thumb.1, PAGE_W, PAGE_H, None);
        assert!(s.scroll.dragging(), "the press on the thumb took no hold");
        let (held_w, held_fill) = drawn_thumb(&mut s, &mut fonts, None);
        assert!(
            (held_w - look.w_hover).abs() < 0.5,
            "the held bar is {held_w} px wide, and scrollbar.w_hover is {}",
            look.w_hover
        );
        let differs = |a: nacelle::theme::Color, b: nacelle::theme::Color| {
            (a.a - b.a).abs() > 0.001
                || (a.r - b.r).abs() > 0.001
                || (a.g - b.g).abs() > 0.001
                || (a.b - b.b).abs() > 0.001
        };
        assert!(
            differs(held_fill, rest_fill),
            "the held thumb is drawn in its RESTING ink; the class ladder was \
             never told the thumb is being dragged"
        );
        assert!(
            differs(held_fill, hover_fill),
            "the held thumb is drawn in its HOVER ink; the bar was told the \
             pointer is over it, which it is not, and nothing about the grab"
        );
        viewport_home();
    }

    /// One frame of the window with the pointer where `mouse` says (away
    /// altogether when `None`), and the thumb AS IT WAS PAINTED: its
    /// width and its fill.
    ///
    /// Read out of the command register rather than recomputed, because
    /// what is in question is what the frame DID. The thumb is the one
    /// filled ring standing in the bar's lane and taller than it is wide;
    /// the rows never reach there — `scrollbar.mode = inset` keeps the
    /// lane out of their box ([`rows_box`]).
    fn drawn_thumb(
        s: &mut Settings,
        fonts: &mut nacelle::font::FontSystem,
        mouse: Option<(f32, f32)>,
    ) -> (f32, nacelle::theme::Color) {
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, fonts, PAGE_H, 1.0);
        if let Some((x, y)) = mouse {
            ctx.mouse = nacelle::pointer::Pointer::new(x, y);
        }
        s.draw(&mut ctx);
        let band = bar_band(s.flow.view, &ScrollbarLook::from_theme());
        dl.cmds()
            .iter()
            .rev()
            .find_map(|c| match c {
                nacelle::draw::DrawCmd::RingFill { r, color, .. }
                    if r[0] >= band.x - 0.5
                        && r[0] + r[2] <= band.right() + 0.5
                        && r[3] > r[2] =>
                {
                    Some((r[2], *color))
                }
                _ => None,
            })
            .expect("the frame painted no thumb in the bar's lane")
    }

    // ------------------------------------------------------- the bands

    /// A two-column band, made only here: the pages are all one flow in
    /// this step, and a mechanism nothing builds is a mechanism nobody
    /// has measured.
    static PROBE_LEFT: [Row; 1] = [row(Ctrl::Slider {
        // Two DIFFERENT labels on purpose, and the widths they measure
        // to are what the columns are: if the label column were still
        // the page's, both tracks would start on one pixel.
        label: "A",
        act: Act::BlurRadiusTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: step_5,
        get: |s| s.blur_radius,
        set: |s, v| s.blur_radius = v,
        save: |_| {},
    })];

    static PROBE_RIGHT: [Row; 1] = [row(Ctrl::Slider {
        label: "A MUCH LONGER LABEL",
        act: Act::BlurOpacityTrack,
        unit: Unit::Percent,
        range: (0, 100),
        step: step_5,
        get: |s| s.blur_opacity,
        set: |s, v| s.blur_opacity = v,
        save: |_| {},
    })];

    static PROBE_COLUMNS: [ZCol; 2] =
        [ZCol { rows: &PROBE_LEFT }, ZCol { rows: &PROBE_RIGHT }];

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
        for (region, _) in &regions {
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
        for (region, _) in &regions {
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
    /// REGION's OWN ROWS and held to the theme's walls.
    ///
    /// This is the whole reason `columns` stopped being the page's: two
    /// columns of one band ask it separately, so the sliders on the left
    /// do not inherit the width of the labels on the right. What it asks
    /// WITH is the band's own rows now (`rhythm.label_col = auto`), so
    /// the words a page measures against cannot fall out of step with the
    /// words it draws — there is only the one set of words.
    #[test]
    fn a_column_measures_its_own_labels() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let s = furnished();
        let (short, _) = s.columns(&mut ctx, PROBE_COLUMNS[0].rows, 600.0);
        let (long, _) = s.columns(&mut ctx, PROBE_COLUMNS[1].rows, 600.0);
        assert!(
            long > short + 0.01,
            "two different labels gave one label column ({short} px and {long} px)"
        );
        // A band that writes in neither column reserves neither: the
        // rail is buttons, and a floor applied to a column nothing
        // stands in would indent a page by a label it does not have.
        let (none_l, none_v) = s.columns(&mut ctx, &RAIL_ROWS, 600.0);
        assert!(
            none_l == 0.0 && none_v == 0.0,
            "a band of buttons reserved {none_l} px of label and {none_v} px of value"
        );
        // The ceiling is a share of the REGION, not of the page: the same
        // words in a narrow column are held to `rhythm.label_max`.
        let th = theme::resolved();
        let max = th.px(theme::id("rhythm.label_max").expect("the master declares it"));
        let pad = th.px(theme::id("rhythm.label_pad").expect("the master declares it"));
        let (held, _) = s.columns(&mut ctx, PROBE_COLUMNS[1].rows, 120.0);
        assert!(
            (held - (120.0 * max + pad)).abs() < 0.01,
            "a label column of {held} px ignored the {} px ceiling its region allows",
            120.0 * max
        );
        viewport_home();
    }

    /// THE OWNER'S OWN SIGHTING, 2026-08-17: on the theme editor's page
    /// the track ran THROUGH the letters of its label and the number sat
    /// on the knob. Not a drawing fault — the page declared no columns,
    /// so its rows reserved nothing and the track took the whole width,
    /// label to number, with the label written over the left end of it
    /// and the number over the right.
    ///
    /// Asked of EVERY PAGE and every row of it, at three window heights
    /// and with the editor's conditional rows standing, because the page
    /// that had it was the page nobody had measured. A row's three parts
    /// stand in this order and never overlap: the label in its column,
    /// the control after it, the number in the gutter at the far edge.
    ///
    /// The number is measured at its WIDEST, not at the value the window
    /// happens to carry — a column that only holds for the number on
    /// screen is a column the next number overflows.
    ///
    /// NOTHING BELOW ASKS THE CODE UNDER TEST WHAT THE ANSWER IS. There
    /// are two ways this could have agreed with itself instead of
    /// checking anything, and it is written against both:
    ///
    /// * Which rows write in the label column is matched ON THE KIND and
    ///   never read out of [`Ctrl::column_label`]. That list is what
    ///   reserves the column, so a walk through it cannot see a kind fall
    ///   OUT of it — and a kind that falls out goes on writing its word
    ///   at the content's left edge with nothing holding its control off
    ///   it. The editor's mode switch is a band of ONE cycler, so a
    ///   cycler dropped from that list puts its plate on its own label,
    ///   which is the owner's sighting again. The arms below are spelled
    ///   out with no wildcard, so a new kind of row cannot join a page
    ///   without an answer here.
    ///
    /// * The widest number a track can write is found by WRITING OUT
    ///   every value in its range and measuring them, never by asking
    ///   [`widest_run`] for its own upper bound. HUE reads 359 at the top
    ///   of its range and 300 in the middle of it, and in the master's
    ///   value face 300 is the wider of the two — a reserve taken against
    ///   the top of range alone is a reserve the middle of the range
    ///   overflows.
    #[test]
    fn a_row_does_not_write_its_label_its_control_and_its_value_over_one_another() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        // What the walk actually reached, by kind. A page that stopped
        // offering one of the three would leave this test passing and
        // saying nothing about it.
        let (mut tracks, mut segments, mut cyclers) = (0u32, 0u32, 0u32);
        // And how many tracks the digit substitution EARNS ITS KEEP on:
        // rows whose widest reachable number is wider than the number at
        // the top of their range. None of them and the reserve is not
        // being measured here at all, whatever else passes.
        let mut earned = 0u32;
        for h in [720.0f32, 1080.0, 1440.0] {
            theme::set_viewport(h, 1.0);
            let mut fonts = nacelle::font::FontSystem::new();
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let (f, v) = (role_label(&ctx), role_value(&ctx));
            let digit = widest_digit(&mut ctx, &v);
            // The widest a range can be written, kept per range: three
            // hundred and sixty strings measured once and not once per
            // row, per band and per page. Dropped at every height,
            // because a role's size follows the viewport.
            let mut span = std::collections::HashMap::<(String, u32), f32>::new();
            let mut s = furnished();
            // Every `Row::when` on the editor's page satisfied at once:
            // the rows that come and go are rows too, and a column that
            // only fits the ones standing today is the same fault later.
            editor_ajar(&mut s);
            // BOTH of the editor's pages: a band that is not standing is
            // a band the walk does not reach, and the page the owner was
            // looking at when he reported this is BASIC.
            for basic in [false, true] {
                s.editor_basic = basic;
                for page in PAGES.iter() {
                    let nav = Panes::of(m, content);
                    let box_ = rows_box(nav.page);
                    for zone in s.frame_zones(page, &nav) {
                        for (region, rows) in zone_regions(zone, box_) {
                            let (label_w, value_w) = s.columns(&mut ctx, rows, region.w);
                            for row in rows.iter().filter(|r| (r.when)(&s)) {
                                let band = Rect::new(
                                    region.x,
                                    region.y,
                                    region.w,
                                    s.row_h(&row.ctrl, m, region),
                                );
                                let rc =
                                    RowCtx { content: region, band, label_w, value_w, m };
                                let seen = (page.title, row.ctrl.column_label());
                                // The word this kind writes in the label
                                // column, and where its control's left
                                // edge lands — both off the KIND.
                                let placed: Option<(&'static str, f32)> = match row.ctrl {
                                    Ctrl::Slider { label, .. } => {
                                        tracks += 1;
                                        Some((label, track_rect(rc).x))
                                    }
                                    Ctrl::Chips { label, values, .. } => {
                                        segments += 1;
                                        let first = chip_rects(values(&s).len(), rc);
                                        let first =
                                            first.first().expect("a row of no segments");
                                        Some((label, first.x))
                                    }
                                    Ctrl::Cycle { label, .. } => {
                                        cyclers += 1;
                                        Some((label, cycle_rect(rc).x))
                                    }
                                    // The picker writes NO word in the
                                    // label column — its heading is the
                                    // section row above it — so there is
                                    // no collision here to measure.
                                    Ctrl::Toggle { .. }
                                    | Ctrl::Drop { .. }
                                    | Ctrl::Button { .. }
                                    | Ctrl::Expander { .. }
                                    | Ctrl::Bar { .. }
                                    | Ctrl::Section { .. }
                                    | Ctrl::Note { .. }
                                    | Ctrl::Hint { .. }
                                    | Ctrl::Picker(_)
                                    | Ctrl::Custom { .. } => None,
                                };
                                if let Some((label, ctrl_x)) = placed {
                                    // The word starts at the content's
                                    // left edge ([`Settings::row_label`]),
                                    // so where it ENDS is where the
                                    // control may begin and no sooner.
                                    let ink =
                                        ctx.fonts.measure(f.face, f.px, label, f.track);
                                    assert!(
                                        region.x + ink <= ctrl_x + 0.01,
                                        "{seen:?}: the label {label:?} runs to {} and \
                                         the control begins at {ctrl_x}, so one is \
                                         drawn over the other",
                                        region.x + ink
                                    );
                                    // And the column is THIS row's own,
                                    // not the one its neighbour paid
                                    // for. The whole program has ONE
                                    // segmented row, COLOR's DEPTH, and
                                    // it stands beside SPACE: drop the
                                    // segments from
                                    // [`Ctrl::column_label`] and DEPTH
                                    // would keep standing in a column
                                    // SPACE had bought, with nothing
                                    // above this line able to tell. A
                                    // band of exactly this row asks for
                                    // exactly what this row is owed.
                                    let alone = [*row];
                                    let (own, _) = s.columns(&mut ctx, &alone, region.w);
                                    assert!(
                                        ink <= own + 0.01,
                                        "{seen:?}: {label:?} alone in a band reserves \
                                         {own} px for a word {ink} px wide, so it only \
                                         fits where a neighbour widens the column"
                                    );
                                }
                                let Ctrl::Slider { unit, range: (lo, hi), .. } = row.ctrl
                                else {
                                    continue;
                                };
                                let track = track_rect(rc);
                                assert!(
                                    track.w > 0.0,
                                    "{seen:?}: the track has {} px to stand in",
                                    track.w
                                );
                                // Every number this track can reach,
                                // written out and measured.
                                let key = (unit.text(hi), lo);
                                let widest = match span.get(&key) {
                                    Some(&had) => had,
                                    None => {
                                        let had = (lo..=hi).fold(0.0f32, |wide, n| {
                                            let written = unit.text(n);
                                            wide.max(ctx.fonts.measure(
                                                v.face, v.px, &written, v.track,
                                            ))
                                        });
                                        span.insert(key, had);
                                        had
                                    }
                                };
                                let top =
                                    ctx.fonts.measure(v.face, v.px, &unit.text(hi), v.track);
                                if widest > top + 0.01 {
                                    earned += 1;
                                }
                                // The reserve the row asks for holds
                                // every one of them — this is the whole
                                // of what the substitution is for.
                                let asked = widest_run(&mut ctx, &v, &unit.text(hi), digit);
                                assert!(
                                    widest <= asked + 0.01,
                                    "{seen:?}: the widest number this track can write \
                                     is {widest} px and the row reserves {asked} px \
                                     for it"
                                );
                                // And the track, as it is laid out,
                                // stops short of where that number
                                // begins.
                                assert!(
                                    track.right() <= region.right() - widest + 0.01,
                                    "{seen:?}: the track ends at {} and the widest \
                                     number this row can write begins at {}",
                                    track.right(),
                                    region.right() - widest
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(
            tracks > 0 && segments > 0 && cyclers > 0,
            "the walk reached {tracks} tracks, {segments} segmented rows and {cyclers} \
             cyclers — a kind it never reaches is a kind it never checks"
        );
        assert!(
            earned > 0,
            "no track on any page can write a number wider than the one at the top of \
             its range, so nothing above measured the reserve itself"
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
        for (i, (r, label, act)) in plates.iter().enumerate() {
            assert!(*act == EDITOR_BAR_ITEMS[i].1, "the bar reordered its verbs");
            assert!((r.y - band.y).abs() < 0.01, "a plate left the row");
            // Measured on the CAP — the run `button::draw` will actually
            // put on the plate, which is the label under
            // `type.<button.role>.case`. Reading `fonts.measure` on the
            // label as this file spells it was the seam: the bar sized
            // its plates for one string and the object drew another.
            let wanted =
                (nacelle::object::button::cap_width(&mut ctx, label) + 2.0 * pad).max(min_w);
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

    /// A plate is sized for the run the OBJECT will draw on it, not for
    /// the string this file spells.
    ///
    /// The two parted company when `button::draw` learned to apply
    /// `type.<button.role>.case`: the bar went on measuring its own
    /// literal and the button went on drawing the transform of it. Under
    /// the shipped master the two agree by luck — `upper` applied to
    /// `"SAVE"` is `"SAVE"` — so the defect is invisible today and
    /// arrives the moment either the master or one literal changes,
    /// which is the whole point of the token existing. The theme below
    /// makes them disagree on purpose.
    #[test]
    fn a_plate_is_sized_for_the_cap_the_button_will_draw() {
        let _g = crate::widgets::theme_test_lock();
        // Every verb in this bar is written in capitals, so a role that
        // lower-cases makes the DRAWN run narrower than the spelling.
        let _t = crate::widgets::Themed::new("bar-case-lower", "[type]\nbutton.case = lower\n");
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

        let th = theme::resolved();
        let min_w = th.px(theme::id("button.min_w").expect("the master declares it"));
        let pad = th.px(theme::id("button.pad_x").expect("the master declares it"));
        let f = role_button(&ctx);
        // A plate this theme sizes differently from the way the old
        // reading would have. Without one the assertion below passes on
        // a bar that never exercised the difference.
        let mut told_apart = 0;
        for (i, (r, label, _)) in plates.iter().enumerate() {
            let spelled =
                (ctx.fonts.measure(f.face, f.px, label, f.track) + 2.0 * pad).max(min_w);
            let drawn =
                (nacelle::object::button::cap_width(&mut ctx, label) + 2.0 * pad).max(min_w);
            assert!(
                (r.w - drawn).abs() < 0.01,
                "plate {i} ({label}) is {} px wide; the cap it will carry wants {drawn} px \
                 and the spelling it was written in wants {spelled} px",
                r.w
            );
            if (spelled - drawn).abs() > 0.01 {
                told_apart += 1;
            }
        }
        assert!(
            told_apart > 0,
            "no plate in this bar tells the spelled width from the drawn one, \
             so this test would pass on a bar that ignores the case token"
        );
        viewport_home();
    }

    /// The DISABLED form of a bar carries the same cap the live one does.
    ///
    /// It writes its own inscription — it wants the ladder's Disabled
    /// rung and no plate under it — so it is the one place in this window
    /// where a button's label reaches the screen BESIDE `button::draw`
    /// instead of through it. Reading the raw literal there would apply a
    /// theme's case transform to a row the page has turned on and not to
    /// the same row turned off: one control, two spellings, depending on
    /// whether you may press it.
    #[test]
    fn a_disabled_plate_is_inscribed_with_the_cap_a_live_one_would_draw() {
        let _g = crate::widgets::theme_test_lock();
        let _t =
            crate::widgets::Themed::new("disabled-case-lower", "[type]\nbutton.case = lower\n");
        let runs = crate::widgets::drawn_text(1080.0, 0.0, 1.0, |ctx| {
            let mut s = furnished();
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(ctx, content);
            let band = Rect::new(content.x, content.y, content.w, m.btn_h);
            let rc = RowCtx { content, band, label_w: 0.0, value_w: 0.0, m };
            s.draw_disabled(ctx, &Ctrl::Bar { items: &EDITOR_BAR_ITEMS }, rc);
        });
        let words: Vec<&str> = runs.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(
            words,
            ["save", "save as", "cancel"],
            "a disabled bar was inscribed with the literals this file spells, \
             not with the caps type.button.case makes of them"
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

    // -------------------------------------------------- the two panels

    /// The rail stands on every page of the window, and it says which
    /// page that is out of the theme's own ladder — ON BOTH LEVELS AT
    /// ONCE where the page is inside an unfolded section (the owner's
    /// mock-up, §4).
    ///
    /// WHY BOTH. While a section's pages stood in a COLUMN of their own
    /// the two marks were in two places and could not be confused. Under
    /// one rail they are four entries in one list, so a mark on the
    /// section alone would leave the reader with an open section and no
    /// word about which of its pages is on the right, and a mark on the
    /// page alone would leave the section it belongs to anonymous. Both,
    /// and this test counts them: on a page inside a section it asserts
    /// that TWO DIFFERENT buttons of the one column wear the rung.
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
        let mut both_levels = 0;
        for p in PAGES.iter() {
            // The page's own section is UNFOLDED, said by the fixture
            // rather than implied by the view: the second half of the
            // double mark is only on the screen while the entry that
            // wears it is, and since 2026-08-18 nothing about the view
            // opens a section.
            let mut s = railed_at(p.view, &[rail_act(p.view)]);
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let at = |act: Act| {
                s.hits.iter().find(|&&(_, a)| a == act).map(|&(r, _)| r)
            };
            // Everything the rail offers on this page — its sections and,
            // under the open one, that section's pages. Exactly the two
            // the view names wear the rung, and nothing else does.
            let mut marked: Vec<Act> = Vec::new();
            for act in rail_acts(&s) {
                let r = at(act).unwrap_or_else(|| {
                    panic!("{}: the rail lost an entry", p.title)
                });
                let want = act == rail_act(p.view) || kid_act(p.view) == Some(act);
                let got = rung(s.button_state(&ctx, r, act)) == State::Selected;
                assert_eq!(got, want, "{}: the rail marks the wrong entry", p.title);
                if got {
                    marked.push(act);
                }
            }
            // BOTH LEVELS, counted. A page the section does not list
            // (the editor, the reset confirmation) marks its section
            // alone — which is true and not a gap: neither of them is
            // one of its entries.
            let want = 1 + usize::from(kid_act(p.view).is_some());
            assert_eq!(
                marked.len(),
                want,
                "{}: {} entries of the rail wear the rung and the page names {}",
                p.title,
                marked.len(),
                want
            );
            if want == 2 {
                both_levels += 1;
                // Named by the id the chain knows them as: `Act` has no
                // Debug and giving it one drags four more enums with it.
                assert!(
                    marked[0] != marked[1],
                    "{}: one entry was counted as both levels of the mark ({})",
                    p.title,
                    focus_id(marked[0]).0
                );
            }
        }
        assert!(
            both_levels > 0,
            "no page in the window stands inside an unfolded section, so the \
             double mark was never measured"
        );
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

    /// ONE COLUMN OF NAVIGATION AND THE PAGE (owner's mock-up,
    /// 2026-08-18, §1) — and the page's width is the SAME width whatever
    /// section stands open.
    ///
    /// WHAT THIS REPLACED. A section's pages used to stand in a column of
    /// their own, so this window had to reserve that column's width on
    /// every page whether the section showed one or not: reserving it
    /// only where it stood would have re-shaped the window under the
    /// reader's hand every time they changed section. A fifth of the
    /// window was therefore spent, permanently, on a column most sections
    /// never used. Pages that unfold UNDER their section take no width at
    /// all, and the second half of this test is what says so — the page
    /// is the same rectangle on the section that has pages and on the one
    /// that does not.
    #[test]
    fn the_navigation_is_one_column_and_the_page_takes_the_rest() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        let nav = Panes::of(m, content);
        assert!(!nav.folded, "the window folded at a width it fits in");
        // The BED is what a column IS: where the gutter falls and how
        // wide the column reads are questions about the paint, not about
        // the room its rows were given inside it.
        let rail = nav.rail.expect("no rail").bed;
        assert!(
            (rail.x - content.x).abs() < 0.01,
            "the rail does not start at the content box's own edge"
        );
        assert!(
            (nav.page.x - rail.right() - col_gap()).abs() < 0.01,
            "the gutter between the rail and the page is not settings.col_gap"
        );
        assert!(
            (nav.page.right() - content.right()).abs() < 0.01,
            "the page does not take the whole of the rest"
        );

        // AND THE SPLIT IS THE SAME SPLIT ON EVERY PAGE. The section with
        // pages of its own and the one without are measured through the
        // frames the window really draws, because the split is no longer
        // even a question `Panes` is asked — a test that only called
        // `Panes::of` twice would be comparing one value with itself.
        let mut widths: Vec<f32> = Vec::new();
        for view in [View::LookFeel, View::Grid] {
            let mut s = furnished();
            s.view = view;
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            s.draw(&mut ctx);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            widths.push(s.body_box(page(view), m, content).w);
        }
        assert!(
            (widths[0] - widths[1]).abs() < 0.01,
            "the section with pages leaves the page {} px and the one without {} px \
             — the pages took width the mock-up says they take by standing under \
             their section",
            widths[0],
            widths[1]
        );

        // AND THE SHAPE IS THE WINDOW'S, NOT THE SECTION'S, at every
        // height the program is built for. It holds STRUCTURALLY:
        // [`Panes::of`] takes no `&Settings` at all, so there is no
        // path by which which-section-is-open could reach the split. A
        // threshold that could ask would stand in columns on GRID and
        // fold on LOOK AND FEEL — the window re-shaping itself under
        // the reader's hand every time they changed section — and a
        // first draft of the one-column rail came within one parameter
        // of exactly that.
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let mut shapes: Vec<(&str, bool)> = Vec::new();
            for p in PAGES.iter() {
                let mut s = furnished();
                s.view = p.view;
                shapes.push((p.title, Panes::of(m, content).folded));
            }
            let first = shapes[0].1;
            if let Some((title, _)) = shapes.iter().find(|(_, f)| *f != first) {
                panic!(
                    "at {h}px {} and {title} put the window in two different \
                     shapes — the fold follows the section instead of the window",
                    shapes[0].0
                );
            }
        }
        viewport_home();
    }


    /// One frame of the settings window, with the focus chain walked:
    /// what the pointer was offered, what Tab can reach, and where the
    /// rail stood.
    ///
    /// Both halves of "is this on screen" in one place, because the
    /// expander's whole claim is that they answer TOGETHER — a page the
    /// hand cannot press and the keyboard can reach is exactly the fault
    /// the toolkit's own list rule exists to prevent
    /// (`object::dropdown::accordion`).
    fn rail_frame(
        fonts: &mut nacelle::font::FontSystem,
        view: View,
        open: &[Act],
    ) -> (Vec<(Rect, Act)>, Vec<FocusId>, Panes) {
        let mut s = railed_at(view, open);
        let mut fc = FocusCtl::new();
        let mut dl = nacelle::draw::DrawList::recording();
        fc.begin_frame();
        let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
        ctx.focus = Some(&mut fc);
        s.draw(&mut ctx);
        // The chain answers about the last COMPLETED frame.
        fc.begin_frame();
        let mut chain: Vec<FocusId> = Vec::new();
        fc.focus(None);
        for _ in 0..s.hits.len() * 2 + 16 {
            fc.nav(Nav::Next);
            if let Some(id) = fc.focused() {
                if chain.contains(&id) {
                    break;
                }
                chain.push(id);
            }
        }
        let content = content_rect(modal_rect(1080.0 * 16.0 / 9.0, 1080.0));
        let mut dl2 = nacelle::draw::DrawList::new();
        let ctx2 = probe(&mut dl2, fonts, 1080.0, 1.0);
        let nav = Panes::of(Metrics::of(&ctx2, content), content);
        (s.hits.clone(), chain, nav)
    }

    /// A SECTION THAT IS NOT THE ONE IN FORCE HANDS OUT NOTHING. Its
    /// pages are not drawn, are not in the hit map and are not a step in
    /// the Tab order.
    ///
    /// This is the toolkit's own rule, borrowed rather than re-invented:
    /// `object::dropdown::accordion` puts an element in the focus chain
    /// only when the whole of it is standing, "because a ring on a
    /// sliver says «this is the whole object» about a part". A ring on
    /// a page that is not on the screen at all says it about nothing —
    /// the keyboard would land somewhere the eye cannot follow, and
    /// Enter would open a page the reader never chose to look for.
    ///
    /// Said in the walker and not in a filter beside it
    /// ([`Settings::draw_rows`]): a shut section's pages are not
    /// recursed into, so there is no path by which one could be drawn,
    /// pressed or focused, and no second rule to keep in step.
    #[test]
    fn a_shut_section_hands_out_no_target_and_no_place_in_the_chain() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        // ON THE SECTION'S OWN PAGE, WITH THE SECTION SHUT — which is
        // the state the window OPENS in since the fold stopped being
        // the view read a second way ([`Settings::rail_open`]), and the
        // state the coupling made unaskable: LOOK AND FEEL is the view
        // this window comes up on, so under the old reading this frame
        // could not exist. If the fold ever falls back to following the
        // page, this is the assertion that says so.
        let (hits, chain, nav) = rail_frame(&mut fonts, View::LookFeel, &[]);
        assert!(!nav.folded, "the window folded, so there is no rail to measure");
        let shut = furnished();
        let pages = nav_row_acts(&shut, &LOOKFEEL_PAGES);
        assert!(
            !pages.is_empty(),
            "LOOK AND FEEL lists no pages at all, so this test measures nothing"
        );
        for act in pages {
            assert!(
                !hits.iter().any(|&(_, a)| a == act),
                "a page of a shut section ({}) answers the pointer",
                focus_id(act).0
            );
            assert!(
                !chain.contains(&focus_id(act)),
                "a page of a shut section ({}) is a step in the Tab order",
                focus_id(act).0
            );
        }
        // And the section itself is still there — shut is not gone.
        assert!(
            hits.iter().any(|&(_, a)| a == Act::OpenLookFeel),
            "the shut section lost its own entry"
        );
        viewport_home();
    }

    /// AND THE SECTION IN FORCE HANDS OUT BOTH. Every page it unfolds is
    /// on the screen, answers the pointer, and stands in the Tab order —
    /// in the order the description writes them, under their section.
    #[test]
    fn an_open_section_hands_out_every_page_to_the_hand_and_the_keyboard() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let (hits, chain, nav) =
            rail_frame(&mut fonts, View::LookFeel, &[Act::OpenLookFeel]);
        assert!(!nav.folded, "the window folded, so there is no rail to measure");
        let open = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
        let pages = nav_row_acts(&open, &LOOKFEEL_PAGES);
        assert!(pages.len() > 1, "one page is not an unfold");
        let rail = nav.rail.expect("no rail").rows;
        let mut chain_at: Vec<usize> = Vec::new();
        for act in &pages {
            let r = hits
                .iter()
                .find(|&&(_, a)| a == *act)
                .map(|&(r, _)| r)
                .unwrap_or_else(|| {
                    panic!("a page of the open section ({}) is not a target", focus_id(*act).0)
                });
            assert!(
                r.w > 0.0 && r.h > 0.0,
                "a page of the open section was offered with no area to press"
            );
            assert!(
                r.y >= rail.y - 0.01 && r.bottom() <= rail.bottom() + 0.01,
                "a page of the open section stands outside the rail's own room"
            );
            let at = chain
                .iter()
                .position(|id| *id == focus_id(*act))
                .expect("a page of the open section is not in the Tab order");
            chain_at.push(at);
        }
        // …and in the order the table writes them, which is reading
        // order down the column.
        assert!(
            chain_at.windows(2).all(|w| w[0] < w[1]),
            "the open section's pages are in the Tab order out of the order they \
             are drawn in: {chain_at:?}"
        );
        // The section stands ABOVE its own pages, because they belong to
        // it and not the other way round.
        let section = hits
            .iter()
            .find(|&&(_, a)| a == Act::OpenLookFeel)
            .map(|&(r, _)| r)
            .expect("the open section lost its own entry");
        for act in &pages {
            let r = hits.iter().find(|&&(_, a)| a == *act).map(|&(r, _)| r).unwrap();
            assert!(
                r.y >= section.bottom() - 0.01,
                "a page of the section stands above the section it belongs to"
            );
        }
        viewport_home();
    }

    /// THE OWNER'S REPORTS 1 AND 2 OF 2026-08-18, IN ONE FRAME-BY-FRAME
    /// READING: the rail comes up SHUT, and a press on a section turns
    /// its fold over.
    ///
    /// Both symptoms had one cause, so both are asserted here or neither
    /// is proved. `rail_open` used to read `act == rail_act(self.view)`,
    /// and the window opens on LOOK AND FEEL — so the list came up
    /// unfolded, and pressing its entry led to the page already in
    /// force, which is a press that does nothing anybody can see.
    ///
    /// AND THE PRESS DOES NOT TRAVEL, which is the decision the report
    /// left open ([`Settings::toggle_rail`]): `self.view` is read before
    /// and after and must not have moved. A press that navigated as well
    /// would have to navigate on the way OUT too — shutting the list
    /// would carry the reader off to a page.
    ///
    /// Read off REAL FRAMES rather than off the field: what a fold is
    /// worth is what the hand can press and the keyboard can reach, and
    /// a test that only asked `rail_open` would pass with the walker
    /// laying pages under a shut section.
    #[test]
    fn the_rail_comes_up_shut_and_a_press_on_a_section_turns_its_fold_over() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();

        /// One frame of `s`, with the chain walked: what the pointer was
        /// offered and what Tab can reach.
        fn frame(
            s: &mut Settings,
            fonts: &mut nacelle::font::FontSystem,
        ) -> (Vec<Act>, Vec<FocusId>) {
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::new();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            ctx.focus = Some(&mut fc);
            s.draw(&mut ctx);
            fc.begin_frame();
            let mut chain: Vec<FocusId> = Vec::new();
            fc.focus(None);
            for _ in 0..s.hits.len() * 2 + 16 {
                fc.nav(Nav::Next);
                if let Some(id) = fc.focused() {
                    if chain.contains(&id) {
                        break;
                    }
                    chain.push(id);
                }
            }
            (s.hits.iter().map(|&(_, a)| a).collect(), chain)
        }

        // The window as the owner meets it: opened by the road the
        // desktop opens it by, not by a fixture.
        let mut s = furnished();
        s.show();
        assert!(
            s.view == View::LookFeel,
            "the window stopped opening on the section this test is about"
        );
        let pages = nav_row_acts(&s, &LOOKFEEL_PAGES);
        assert!(
            pages.len() > 1,
            "LOOK AND FEEL lists no pages, so a fold here would hide nothing"
        );

        let (hits, chain) = frame(&mut s, &mut fonts);
        assert!(
            hits.contains(&Act::OpenLookFeel),
            "the rail lost the entry the report is about"
        );
        for act in &pages {
            assert!(
                !hits.contains(act),
                "the window opened with the section's pages already on the rail — \
                 the list starts unfolded ({})",
                focus_id(*act).0
            );
            assert!(
                !chain.contains(&focus_id(*act)),
                "a page of the shut section is a step in the Tab order ({})",
                focus_id(*act).0
            );
        }

        // THE PRESS. It turns the fold over and leaves the page alone.
        let was = s.view;
        s.perform(Act::OpenLookFeel, 0.0);
        assert!(s.view == was, "pressing a section walked off the page as well");
        let (hits, chain) = frame(&mut s, &mut fonts);
        for act in &pages {
            assert!(
                hits.contains(act),
                "a press on the section did not unfold it ({})",
                focus_id(*act).0
            );
            assert!(
                chain.contains(&focus_id(*act)),
                "an unfolded page answers the pointer and not the keyboard ({})",
                focus_id(*act).0
            );
        }

        // AND A SECOND PRESS SHUTS IT AGAIN, which is what makes it a
        // toggle rather than a one-way door.
        s.perform(Act::OpenLookFeel, 0.0);
        assert!(s.view == was, "shutting the section walked off the page");
        let (hits, chain) = frame(&mut s, &mut fonts);
        for act in &pages {
            assert!(
                !hits.contains(act) && !chain.contains(&focus_id(*act)),
                "a second press left the section open ({})",
                focus_id(*act).0
            );
        }
        viewport_home();
    }

    /// DECISION (b): A FOLD OUTLIVES A WALK TO ANOTHER SECTION'S PAGE.
    ///
    /// This is the assertion the change cannot dodge. The view is
    /// exactly what used to drive the fold, so a coupling left anywhere
    /// would be the reported fault surviving in a corner — and it would
    /// surface as a rail that reshapes itself under the hand every time
    /// the reader opens a page.
    ///
    /// The other half is asserted with it, because the pair is the whole
    /// decision: walking INTO a section does not open it either. A rail
    /// that unfolded on arrival would be the old reading wearing a new
    /// field.
    #[test]
    fn a_fold_outlives_a_walk_to_another_sections_page() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = furnished();
        s.show();
        s.perform(Act::OpenLookFeel, 0.0);
        assert!(s.rail_open(Act::OpenLookFeel), "the section did not open");

        // Away to a section of its own — GRID is a section that IS its
        // page, so nothing about it has an opinion on this fold.
        s.perform(Act::OpenGrid, 0.0);
        assert!(s.view == View::Grid, "the rail's other entry stopped being a door");
        assert!(
            s.rail_open(Act::OpenLookFeel),
            "walking to another section's page shut a section the reader had opened"
        );
        let mut dl = nacelle::draw::DrawList::new();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let offered: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
        for act in nav_row_acts(&s, &LOOKFEEL_PAGES) {
            assert!(
                offered.contains(&act),
                "the fold survived in the field and not on the screen ({})",
                focus_id(act).0
            );
        }

        // AND ARRIVING SOMEWHERE OPENS NOTHING. Back into the section by
        // its own page, with the fold put away first.
        s.perform(Act::OpenLookFeel, 0.0);
        assert!(!s.rail_open(Act::OpenLookFeel), "the fold would not shut");
        s.perform(Act::OpenSets, 0.0);
        assert!(s.view == View::LookFeel, "the section's own page stopped opening");
        assert!(
            !s.rail_open(Act::OpenLookFeel),
            "arriving on a section's page unfolded the section — the fold is \
             following the view again"
        );
        viewport_home();
    }

    /// DECISION (a): TWO SECTIONS MAY STAND OPEN AT ONCE.
    ///
    /// The rail holds one expander today, so the claim is made against a
    /// table of two — the same way `a_section_the_page_turned_off_hands_
    /// out_no_pages_either` makes its claim about a predicate the
    /// shipped rail never exercises. What is being tested is the
    /// WALKER's rule, and the walker takes any table.
    ///
    /// Under the reading this replaced the assertion was unwritable: the
    /// fold was `act == rail_act(self.view)`, and one view cannot equal
    /// two acts. That is not an argument for the change — the argument
    /// is at [`Settings::rail_open`] — but it is why nothing here could
    /// have been checked before.
    #[test]
    fn two_sections_of_the_rail_may_stand_open_at_once() {
        static FIRST_KIDS: [Row; 1] = [row(Ctrl::Button {
            label: Text::Fixed("ONE"),
            kind: BtnKind::Wide,
            act: Act::OpenBlur,
        })];
        static SECOND_KIDS: [Row; 1] = [row(Ctrl::Button {
            label: Text::Fixed("TWO"),
            kind: BtnKind::Wide,
            act: Act::OpenAddons,
        })];
        static PAIR: [Row; 2] = [
            row(Ctrl::Expander {
                label: Text::Fixed("FIRST"),
                kind: BtnKind::Wide,
                act: Act::OpenLookFeel,
                kids: &FIRST_KIDS,
            }),
            row(Ctrl::Expander {
                label: Text::Fixed("SECOND"),
                kind: BtnKind::Wide,
                act: Act::OpenGrid,
                kids: &SECOND_KIDS,
            }),
        ];

        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();

        /// What the walker hands out for `PAIR` with `open` unfolded,
        /// and how tall it measures the run.
        fn walk(
            fonts: &mut nacelle::font::FontSystem,
            open: &[Act],
        ) -> (Vec<Act>, Vec<FocusId>, f32) {
            let mut s = railed_at(View::LookFeel, open);
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::new();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            ctx.focus = Some(&mut fc);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content).rail();
            let region = Panes::of(m, content).rail.expect("no rail").rows;
            let span = s.rows_span(&PAIR, m, region).0;
            s.draw_rows(&mut ctx, &PAIR, m, region, region.y, None, Carrier::Rail);
            fc.begin_frame();
            let hits: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
            let chain: Vec<FocusId> = hits
                .iter()
                .map(|a| focus_id(*a))
                .filter(|id| fc.rect_of(*id).is_some())
                .collect();
            (hits, chain, span)
        }

        let (none, _, shut_span) = walk(&mut fonts, &[]);
        assert!(
            !none.contains(&Act::OpenBlur) && !none.contains(&Act::OpenAddons),
            "a table with nothing unfolded handed out pages, so this test cannot \
             tell an open section from a broken walker"
        );
        let (both, chain, open_span) =
            walk(&mut fonts, &[Act::OpenLookFeel, Act::OpenGrid]);
        for act in [Act::OpenBlur, Act::OpenAddons] {
            assert!(
                both.contains(&act),
                "one of two open sections handed out nothing ({})",
                focus_id(act).0
            );
            assert!(
                chain.contains(&focus_id(act)),
                "a page of an open section is not in the Tab order ({})",
                focus_id(act).0
            );
        }
        // AND THE MEASUREMENT CARRIES BOTH. A height that counted one
        // unfold would lay the second section's pages over the first's.
        let (one, _, one_span) = walk(&mut fonts, &[Act::OpenLookFeel]);
        assert!(
            one.contains(&Act::OpenBlur) && !one.contains(&Act::OpenAddons),
            "unfolding one section unfolded the other as well"
        );
        assert!(
            open_span > one_span + 1.0 && one_span > shut_span + 1.0,
            "the rail did not grow by each unfold in turn: {shut_span} shut, \
             {one_span} with one open, {open_span} with both"
        );
        viewport_home();
    }

    /// DECISION (c): A FOLD DOES NOT SURVIVE THE WINDOW CLOSING.
    ///
    /// A fold is a view state and not a preference — nothing here writes
    /// a `config` line, and "shut by default" that held exactly once per
    /// session would not be a default. Asserted through BOTH doors into
    /// the window, because the reset is written on the way in
    /// ([`Settings::opening`]) and a door that forgot to use it is the
    /// one way this can rot.
    #[test]
    fn a_fold_does_not_survive_the_window_closing() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut s = furnished();
        s.show();
        s.perform(Act::OpenLookFeel, 0.0);
        assert!(s.rail_open(Act::OpenLookFeel), "the section did not open");
        s.close();
        s.show();
        assert!(
            !s.rail_open(Act::OpenLookFeel),
            "the settings window came back with yesterday's folds"
        );

        s.perform(Act::OpenLookFeel, 0.0);
        assert!(s.rail_open(Act::OpenLookFeel), "the section did not open again");
        s.close();
        s.show_grid();
        assert!(
            !s.rail_open(Act::OpenLookFeel),
            "the window opened at GRID carrying a fold from the session before"
        );
    }

    /// THE OWNER'S REPORT 3 OF 2026-08-18: the MODE row's plate is cut
    /// like every other plate on the page, and the cut comes from the
    /// theme.
    ///
    /// What the owner saw on the screen was one row of the editor whose
    /// corners did not match the buttons above and below it. The cause
    /// was not a theme that disagreed with itself — it was that
    /// [`Settings::draw_cycle`] drew `rect_outline`, four straight bars
    /// with no corners in them at all, and `[cycler]` stated a border
    /// weight and no shape, so there was no key a theme could have used
    /// to say otherwise.
    ///
    /// TWO CLAIMS, AND THE SECOND IS THE OWNER'S SENTENCE:
    ///
    /// * the ring the frame really laid carries the cut the `[cycler]`
    ///   TOKENS name — read out of the engine by the word reading
    ///   (`corner::cut`), which is the other of the toolkit's two paths
    ///   into one dictionary and not the index reading the drawing uses.
    ///   An expectation taken from `draw_cycle`'s own arithmetic would
    ///   move with it and prove nothing;
    /// * and it is the SAME cut a plain button plate on the same frame
    ///   got — measured off that button's own recorded command, not off
    ///   a second reading of the button's tokens. "The same as the rest"
    ///   is a claim about two things on one screen, so both are read
    ///   from one screen.
    ///
    /// FAIL-CLOSED ON THE MASTER'S OWN NUMBERS: a zero radius is a
    /// square corner under every word there is, and a square cut is what
    /// the broken drawing produced — so if the master ever stops asking
    /// for a visible corner here, this test says so instead of passing
    /// on a shape nobody can see.
    // Re-home needed: this measured the BASIC/ADVANCED MODE cycler, which
    // sat at row 0 (always on screen). That switch is now the ADVANCED
    // COLOUR button (a plain Button, RingFill not Ring), and the only
    // `draw_cycle` plate left in the editor — CORNER SIZE — sits below the
    // tall picker and is off-screen in this 1080px probe, so `at()` finds
    // no plate to read. Ignored until it is pointed at a cycler that is
    // reliably drawn (e.g. COLOR's SPACE on its own page). See
    // .gap-program/projekt-edytor-advanced-color.md.
    #[ignore = "MODE cycler removed; needs re-homing onto an on-screen draw_cycle plate"]
    #[test]
    fn a_cycle_plate_is_cut_like_every_other_plate_on_the_page() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();
        let mut s = railed_at(View::ThemeEditor, &[Act::OpenLookFeel]);
        let mut dl = nacelle::draw::DrawList::recording();
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        s.draw(&mut ctx);
        let at = |act: Act| {
            s.hits
                .iter()
                .find(|&&(_, a)| a == act)
                .map(|&(r, _)| r)
                .unwrap_or_else(|| panic!("the frame drew no {}", focus_id(act).0))
        };
        // The BASIC/ADVANCED switch this measured is gone; the CORNER SIZE
        // cycler on BASIC is the same `draw_cycle` plate and carries the
        // same claim.
        let cycler = at(Act::EditorCornerStep);
        // A plain button of the same frame: the rail's own section
        // entry, which `object::button` dresses like every other plate
        // in this window.
        let button = at(Act::OpenLookFeel);

        let same = |a: &[f32; 4], r: &Rect| {
            (a[0] - r.x).abs() < 0.01
                && (a[1] - r.y).abs() < 0.01
                && (a[2] - r.w).abs() < 0.01
                && (a[3] - r.h).abs() < 0.01
        };
        let mut worn: Option<[nacelle::draw::Corner; 4]> = None;
        let mut plain: Option<[nacelle::draw::Corner; 4]> = None;
        for cmd in dl.cmds() {
            match cmd {
                nacelle::draw::DrawCmd::Ring { r, corners, .. } if same(r, &cycler) => {
                    worn = Some(*corners)
                }
                nacelle::draw::DrawCmd::RingFill { r, corners, .. }
                    if same(r, &button) && plain.is_none() =>
                {
                    plain = Some(*corners)
                }
                _ => {}
            }
        }
        let worn = worn.expect(
            "the MODE row's plate is no ring at all — a bare rectangle outline has \
             no corners for a theme to cut",
        );
        let plain = plain.expect("the rail's own entry was drawn with no plate");

        // WHAT THE FILE ASKS FOR, straight out of the engine.
        let t = theme::resolved();
        let id = |n: &str| {
            nacelle::theme::id(n).unwrap_or_else(|| panic!("the master declares no {n}"))
        };
        let word = nacelle::theme::enum_word_of(id("cycler.corner_style"))
            .expect("a corner-style token names a word");
        let want = nacelle::draw::Corner {
            style: nacelle::corner::cut(&word),
            size: nacelle::theme::corner_radius(t.px(id("cycler.corner")), cycler.w, cycler.h),
        };
        assert!(
            want.size > 0.5 && want.style != nacelle::draw::CornerStyle::Square,
            "the master asks for a {:?} of {} px here, which draws the same square \
             plate the fault did — this test would pass on the bug",
            want.style,
            want.size
        );
        for (i, corner) in worn.iter().enumerate() {
            assert_eq!(
                *corner, want,
                "corner {i} of the MODE row is not the cut `[cycler]` asks for"
            );
        }
        // …AND IT IS THE PAGE'S OWN CUT. Both keys point at the button's
        // in the master, so a row that came out different is a row that
        // is not reading them.
        for (i, corner) in worn.iter().enumerate() {
            assert_eq!(
                corner.style, plain[i].style,
                "the MODE row is cut in a different shape from the buttons around it"
            );
            assert!(
                (corner.size - plain[i].size).abs() < 0.01,
                "the MODE row's corner {i} is {} px and the buttons around it are {}",
                corner.size,
                plain[i].size
            );
        }
        viewport_home();
    }

    /// AN ARROW IS A PROMISE (the owner's mock-up, §3). Only a section
    /// that has pages wears one; a section that IS its page has nothing
    /// to reveal and shows nothing that says it has.
    ///
    /// STRUCTURAL FIRST, THEN PAINTED. The description can only give a
    /// section pages through [`Ctrl::Expander`], which is also the only
    /// kind the drawing puts a triangle on — so the first half of this
    /// is a property of the grammar and the second half is the frame
    /// agreeing with it. A `kids: &[]` written on an ordinary button
    /// would be the failure this guards, and there is no field to write
    /// it in.
    #[test]
    fn a_section_with_no_pages_wears_no_arrow() {
        let _g = crate::widgets::theme_test_lock();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        // THE GRAMMAR. Every entry of the rail is one kind or the other,
        // and only the expanders carry pages.
        let mut expanders = 0;
        let mut plain = 0;
        for row in RAIL_ROWS.iter() {
            match row.ctrl {
                Ctrl::Expander { kids, .. } => {
                    expanders += 1;
                    assert!(
                        !kids.is_empty(),
                        "an expander with no pages is an arrow that promises nothing"
                    );
                }
                Ctrl::Button { .. } => plain += 1,
                _ => {}
            }
        }
        assert!(expanders > 0 && plain > 0, "the rail is all of one kind: this test \
             needs a section with pages and one without");

        // THE FRAME. One triangle in the whole navigation column,
        // standing on the one entry that has pages — read off the
        // recorded draw list, so a second arrow drawn by hand somewhere
        // would be caught however it got there.
        let mut fonts = nacelle::font::FontSystem::new();
        // THE ARROW FOLLOWS THE FOLD AND NOT THE PAGE, and the four
        // combinations are what says so. Two of them were unaskable
        // before 2026-08-18 — a section shut on its own page, and a
        // section open from another one — because the fold was the view
        // read a second way, and it is exactly those two the owner saw
        // as "the list starts open and pressing it does nothing".
        for (view, open, down) in [
            (View::LookFeel, false, false),
            (View::LookFeel, true, true),
            (View::Grid, false, false),
            (View::Grid, true, true),
        ] {
            let mut s =
                railed_at(view, if open { &[Act::OpenLookFeel][..] } else { &[][..] });
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let nav = Panes::of(Metrics::of(&ctx, content), content);
            let bed = nav.rail.expect("no rail").bed;
            s.draw(&mut ctx);
            let arrows: Vec<Vec<[f32; 2]>> = dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Polyline { pts, closed: true, .. }
                        if pts.len() == 3
                            && pts.iter().all(|p| bed.contains(p[0], p[1])) =>
                    {
                        Some(pts.clone())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                arrows.len(),
                expanders,
                "the rail drew {} triangles for {expanders} section(s) with pages",
                arrows.len()
            );
            // And it stands on THAT entry's plate and on no other.
            let owner = s
                .hits
                .iter()
                .find(|&&(_, a)| a == Act::OpenLookFeel)
                .map(|&(r, _)| r)
                .expect("the section with pages was not drawn");
            for pts in &arrows {
                assert!(
                    pts.iter().all(|p| p[1] >= owner.y - 0.01 && p[1] <= owner.bottom() + 0.01),
                    "a triangle stands on a rail entry that has no pages behind it"
                );
            }
            // The GLYPH turns and the colour does not: shut it points
            // along the row at what opening would reveal, open it points
            // down at what it revealed. The toolkit's TREE grammar, which
            // is the sentence a row that keeps its place in the column
            // speaks (`view::paint::Disclosure`).
            let pts = &arrows[0];
            if down {
                assert!(
                    (pts[0][1] - pts[1][1]).abs() < 0.01 && pts[2][1] > pts[0][1],
                    "an unfolded section's arrow is not pointing at its pages"
                );
            } else {
                assert!(
                    (pts[0][0] - pts[2][0]).abs() < 0.01 && pts[1][0] > pts[0][0],
                    "a shut section's arrow is not pointing along its row"
                );
            }
        }
        viewport_home();
    }

    /// A SECTION'S PAGE STANDS IN FROM THE SECTION IT BELONGS TO, and
    /// the step is the THEME's — `settings.rail_indent` — with the
    /// hairline `settings.rail_guide_*` describes standing in it.
    ///
    /// THE INDENT IS THE WHOLE OF WHAT THE SECOND COLUMN USED TO SAY by
    /// standing somewhere else, so it is the one thing here that may not
    /// be a number in Rust. The theme is moved under the window and the
    /// pages are measured again: a reader that had baked a step of its
    /// own would keep the old offset and be caught, which a test that
    /// only compared against `rail_indent()` could not do.
    ///
    /// AND THE GUIDE IS MEASURED WITH IT, from the recorded draw list.
    /// Indent alone does not group anything — four buttons a little
    /// further in read as four buttons that failed to line up — so the
    /// line that brackets them is part of the claim and not decoration.
    #[test]
    fn a_sections_page_stands_in_from_the_section_it_belongs_to() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut fonts = nacelle::font::FontSystem::new();

        /// The section's plate, its pages' plates, and every vertical
        /// hairline the rail laid, at the theme in force.
        fn measured(
            fonts: &mut nacelle::font::FontSystem,
        ) -> (Rect, Vec<Rect>, Vec<[f32; 4]>) {
            theme::resolved();
            theme::set_viewport(1080.0, 1.0);
            // Unfolded, said outright: there is no indent to measure and
            // no guide to find beside a section that is standing shut.
            let mut s = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let nav = Panes::of(Metrics::of(&ctx, content), content);
            let bed = nav.rail.expect("no rail").bed;
            s.draw(&mut ctx);
            let at = |act: Act| {
                s.hits.iter().find(|&&(_, a)| a == act).map(|&(r, _)| r).expect("no entry")
            };
            let section = at(Act::OpenLookFeel);
            let pages: Vec<Rect> = nav_row_acts(&s, &LOOKFEEL_PAGES).into_iter().map(at).collect();
            // A rect taller than it is wide, inside the rail: the guide,
            // and nothing else the rail draws is that shape.
            let rules: Vec<[f32; 4]> = dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Rect { r, .. }
                        if r[2] < r[3] && bed.contains(r[0], r[1]) =>
                    {
                        Some(*r)
                    }
                    _ => None,
                })
                .collect();
            (section, pages, rules)
        }

        /// The stroke the THEME asks for, rebuilt from the tokens
        /// themselves and never from [`rail_guide_x`] — the reader this
        /// is about. Both sides of an equation drawn from one function
        /// move together, and a `0.5` or a `4.0` baked into that
        /// function would satisfy it.
        fn guide_from_the_theme(section_x: f32) -> (f32, f32) {
            let t = theme::resolved();
            let px = |n: &str| {
                t.px(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}")))
            };
            let (w, at, step) = (
                px("settings.rail_guide_w"),
                px("settings.rail_guide_x"),
                px("settings.rail_indent"),
            );
            (section_x + (step - w).max(0.0) * at, w)
        }

        let (section, pages, rules) = measured(&mut fonts);
        let step = rail_indent();
        assert!(step > 0.0, "the theme states no indent, so nothing is nested");
        assert!(pages.len() > 1, "one page is not a run to bracket");
        for (i, p) in pages.iter().enumerate() {
            assert!(
                (p.x - section.x - step).abs() < 0.01,
                "page #{i} stands {} px in from its section and the theme asked \
                 for {step}",
                p.x - section.x
            );
            // …and it gives that room UP, rather than hanging off the
            // column: a nested row is narrower, not shifted.
            assert!(
                (p.right() - section.right()).abs() < 0.01,
                "page #{i} was shifted instead of nested — its right edge left \
                 the section's by {} px",
                p.right() - section.right()
            );
        }

        // THE GUIDE: one hairline, of the theme's width, standing in the
        // step, and spanning exactly the run it brackets.
        assert_eq!(rules.len(), 1, "the rail laid {} vertical rules", rules.len());
        let (want_x, want_w) = guide_from_the_theme(section.x);
        let g = rules[0];
        assert!(want_w > 0.0, "the theme states no width for the guide");
        assert!(
            (g[0] - want_x).abs() < 0.01 && (g[2] - want_w).abs() < 0.01,
            "the guide stands at {} px wide {} px; the theme asked for {want_x} / \
             {want_w}",
            g[0],
            g[2]
        );
        assert!(
            g[0] >= section.x - 0.01 && g[0] + g[2] <= pages[0].x + 0.01,
            "the guide left the step it brackets"
        );
        let (top, bottom) =
            (pages[0].y, pages[pages.len() - 1].bottom());
        assert!(
            (g[1] - top).abs() < 0.01 && (g[1] + g[3] - bottom).abs() < 0.01,
            "the guide runs {}..{} and the pages it brackets run {top}..{bottom}",
            g[1],
            g[1] + g[3]
        );

        // AND THE STEP IS THE THEME'S. Double it in a file and the pages
        // move with it — a step baked into this window would not.
        {
            let _t = crate::widgets::Themed::new(
                "wide-rail-indent",
                "[settings]\nrail_indent = 6u\n",
            );
            let (section, pages, rules) = measured(&mut fonts);
            let step = rail_indent();
            assert!(
                (step - 32.4).abs() < 0.5,
                "the theme's own indent did not reach the window: {step}"
            );
            for p in &pages {
                assert!(
                    (p.x - section.x - step).abs() < 0.01,
                    "a page kept its old step under a theme that asked for {step}"
                );
            }
            let (want_x, _) = guide_from_the_theme(section.x);
            assert!(
                (rules[0][0] - want_x).abs() < 0.01,
                "the guide stayed where the old step put it"
            );
        }
        viewport_home();
    }

    /// R6 REACHES A SECTION'S PAGES. A row the page has turned off
    /// registers nothing at all — and if that row is an EXPANDER, its
    /// pages are nothing too: not drawn, not measured, not targets, not
    /// steps in the Tab order, and no hairline beside them.
    ///
    /// WHY IT MATTERS THOUGH NOTHING SHIPS IT. The rail's one expander
    /// (LOOK AND FEEL) carries no `enabled` predicate today, so this
    /// combination cannot arise from `RAIL_ROWS` — but the GRAMMAR
    /// allows it, and the grammar is what the walker obeys. Left as it
    /// was, a greyed section standing on its own page would hand out
    /// four buttons under an inscription that says the section is
    /// unavailable: a way in behind a door marked shut. The rule was
    /// already written three lines above the fault
    /// ([`Settings::draw_rows`]), which is the kind of gap that survives
    /// review by looking like it is being followed.
    ///
    /// BOTH READERS, and that is half the claim. The walker draws and
    /// [`Settings::rows_span`] measures; a fix in one alone would
    /// reserve height for a run nothing draws, and the rail would be one
    /// length for the scroll and another for the eye.
    ///
    /// The two tables differ in ONE thing — the predicate — so what the
    /// assertions compare is that predicate and nothing else. The
    /// enabled one is measured first: a test in which neither table
    /// hands anything out would pass while proving nothing.
    #[test]
    fn a_section_the_page_turned_off_hands_out_no_pages_either() {
        static KIDS: [Row; 2] = [
            row(Ctrl::Button {
                label: Text::Fixed("ONE"),
                kind: BtnKind::Wide,
                act: Act::OpenBlur,
            }),
            row(Ctrl::Button {
                label: Text::Fixed("TWO"),
                kind: BtnKind::Wide,
                act: Act::OpenGrid,
            }),
        ];
        static OPEN: [Row; 1] = [row(Ctrl::Expander {
            label: Text::Fixed("SECTION"),
            kind: BtnKind::Wide,
            act: Act::OpenLookFeel,
            kids: &KIDS,
        })];
        static SHUT: [Row; 1] = [row_when(
            Ctrl::Expander {
                label: Text::Fixed("SECTION"),
                kind: BtnKind::Wide,
                act: Act::OpenLookFeel,
                kids: &KIDS,
            },
            |_| false,
        )];

        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let mut fonts = nacelle::font::FontSystem::new();

        /// One run of the walker over one table: what it registered,
        /// what joined the chain, how many hairlines it laid, and what
        /// the MEASUREMENT says the same run is worth.
        fn walk(
            fonts: &mut nacelle::font::FontSystem,
            rows: &'static [Row],
        ) -> (Vec<Act>, Vec<FocusId>, usize, f32) {
            // The section is UNFOLDED, so only the predicate can shut
            // it — which is the whole experiment. Asked of the fixture
            // since 2026-08-18; it used to come free with the view.
            let mut s = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::recording();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            ctx.focus = Some(&mut fc);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content).rail();
            let region = Panes::of(m, content).rail.expect("no rail").rows;
            let span = s.rows_span(rows, m, region).0;
            s.draw_rows(&mut ctx, rows, m, region, region.y, None, Carrier::Rail);
            fc.begin_frame();
            let hits: Vec<Act> = s.hits.iter().map(|&(_, a)| a).collect();
            let chain: Vec<FocusId> = hits
                .iter()
                .map(|a| focus_id(*a))
                .filter(|id| fc.rect_of(*id).is_some())
                .collect();
            let rules = dl
                .cmds()
                .iter()
                .filter(|c| {
                    matches!(c, nacelle::draw::DrawCmd::Rect { r, .. } if r[2] < r[3])
                })
                .count();
            (hits, chain, rules, span)
        }

        let (open_hits, open_chain, open_rules, open_span) = walk(&mut fonts, &OPEN);
        assert!(
            open_hits.contains(&Act::OpenBlur) && open_hits.contains(&Act::OpenGrid),
            "the enabled section handed out no pages, so this test cannot tell a \
             shut one from a broken walker"
        );
        assert!(!open_chain.is_empty(), "the enabled section joined no chain at all");
        assert_eq!(open_rules, 1, "the enabled section laid {open_rules} hairlines");

        let (hits, chain, rules, span) = walk(&mut fonts, &SHUT);
        for act in [Act::OpenBlur, Act::OpenGrid] {
            assert!(
                !hits.contains(&act),
                "a page of a section the window turned off is still a target"
            );
            assert!(
                !chain.contains(&focus_id(act)),
                "a page of a section the window turned off is still a step in the \
                 Tab order"
            );
        }
        assert_eq!(
            rules, 0,
            "a section the window turned off still braced its pages with a hairline"
        );
        // AND THE MEASUREMENT AGREES. The shut section is as tall as its
        // own row and no taller; the open one is taller by its pages.
        assert!(
            span < open_span - 1.0,
            "a section the window turned off is measured as tall as an open one \
             ({span} against {open_span}) — the height reserves room for a run \
             nothing draws"
        );
        viewport_home();
    }

    /// A RAIL TALLER THAN ITS COLUMN SCROLLS; IT DOES NOT FOLD THE
    /// WINDOW. Point 10 of the programme, on the navigation column.
    ///
    /// THE FAULT THIS CLOSES was made by the change beside it. A
    /// section's pages moved INTO the rail on 2026-08-18, so the column
    /// can want more height than it has — 440 px against 418 at 720p on
    /// the master, 455 against 454 at 768p on a machine with no colour
    /// manager. The first draft answered by folding the whole window at
    /// those heights, which took the two-panel shape away from every
    /// screen between 720p and 800p that had stood in columns before.
    /// The toolkit has had the answer to content that does not fit since
    /// the page adopted it: an offset, a bar and a wheel.
    ///
    /// FOUR THINGS, and the first is what makes the other three mean
    /// something:
    ///
    /// * at 720p the window stands in COLUMNS and the rail really does
    ///   overflow — no fold, and something to scroll;
    /// * a notch over the RAIL moves the rail and leaves the page where
    ///   it was, and a notch over the PAGE does the opposite — the
    ///   pointer is what tells them apart, and a window that answered
    ///   one wheel with both scrolls would be unusable in a way no
    ///   offset-only test can see;
    /// * the rail's bar is DRAWN, in the rail's own room, where the
    ///   entries overflow it and nowhere else;
    /// * and the bar takes the hand: a press on the thumb grabs it and
    ///   dragging moves the rail rather than the page.
    #[test]
    fn a_rail_taller_than_its_column_scrolls_and_the_window_keeps_its_panels() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut fonts = nacelle::font::FontSystem::new();
        theme::resolved();
        theme::set_viewport(720.0, 1.0);

        /// One drawn frame of the window at 720p, with the recorder on.
        fn frame<'a>(
            s: &mut Settings,
            dl: &'a mut nacelle::draw::DrawList,
            fonts: &mut nacelle::font::FontSystem,
        ) {
            let mut ctx = probe(dl, fonts, 720.0, 1.0);
            ctx.t = 1.0;
            s.draw(&mut ctx);
        }

        // Unfolded, because an unfold is what makes the rail outgrow its
        // column at all — and since the fold stopped following the view,
        // the fixture is the only thing that can ask for one.
        let mut s = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
        let mut dl = nacelle::draw::DrawList::recording();
        frame(&mut s, &mut dl, &mut fonts);
        let rail = s.rail_flow.expect("the window folded at 720px — the regression is back");
        assert!(
            rail.flow.length > rail.flow.view.h + 0.01,
            "the rail wants {} px and has {} px, so nothing here is scrolling and \
             this test is measuring the wrong window",
            rail.flow.length,
            rail.flow.view.h
        );

        // THE POINTER DECIDES. Over the rail's bed, then over the page.
        let (page_before, rail_before) = (s.scroll.offset(), s.rail_scroll.offset());
        s.wheel(-3.0, rail.bed.cx(), rail.bed.y + rail.bed.h / 2.0);
        assert!(
            s.rail_scroll.offset() > rail_before + 0.01,
            "a notch over the navigation column moved nothing"
        );
        assert!(
            (s.scroll.offset() - page_before).abs() < 0.01,
            "a notch over the navigation column moved the page as well"
        );
        let rail_at = s.rail_scroll.offset();
        let on_page = (rail.bed.right() + 1.0, rail.bed.y + rail.bed.h / 2.0);
        s.wheel(-3.0, on_page.0, on_page.1);
        assert!(
            (s.rail_scroll.offset() - rail_at).abs() < 0.01,
            "a notch over the page moved the navigation column as well"
        );

        // AND THE RAIL REALLY MOVED WHAT IT DRAWS. The same entry, two
        // frames apart, stands higher by exactly what the offset took.
        let mut dl2 = nacelle::draw::DrawList::recording();
        frame(&mut s, &mut dl2, &mut fonts);
        let after = s.rail_flow.expect("the rail went away mid-test");
        let moved = after.flow.offset - rail.flow.offset;
        assert!(moved > 0.01, "the rail's offset did not survive into the next frame");

        // THE BAR IS ON THE SCREEN, in the rail's room and not over the
        // page's — read off the frame that was just drawn and not from a
        // geometry this test worked out for itself, because a bar that
        // is only computed is the very fault the page's own bar had.
        // The pointer is off the window in a probe, so the RESTING width
        // is the one the frame painted.
        let look = ScrollbarLook::from_theme();
        let lane = bar_band(after.flow.view, &look);
        assert!(
            lane.right() <= rail.bed.right() + 0.01 && lane.x >= rail.bed.x - 0.01,
            "the rail's bar lane {:?} left the rail's own bed {:?}",
            (lane.x, lane.right()),
            (rail.bed.x, rail.bed.right())
        );
        let at_rest = scroll::scrollbar(
            after.flow.view,
            &look,
            after.flow.offset,
            after.flow.view.h,
            after.flow.length,
            false,
        )
        .expect("an overflowing rail was given no bar geometry at all");
        let same = |r: &[f32; 4], t: Rect| {
            (r[0] - t.x).abs() < 0.5
                && (r[1] - t.y).abs() < 0.5
                && (r[2] - t.w).abs() < 0.5
                && (r[3] - t.h).abs() < 0.5
        };
        assert!(
            dl2.cmds().iter().any(|c| match c {
                nacelle::draw::DrawCmd::RingFill { r, .. }
                | nacelle::draw::DrawCmd::Rect { r, .. } => same(r, at_rest.thumb),
                _ => false,
            }),
            "the rail overflows and no thumb was painted at {:?}",
            [at_rest.thumb.x, at_rest.thumb.y, at_rest.thumb.w, at_rest.thumb.h]
        );
        // The press aims at the HOVER width, which is what the hand
        // grabs: a lane is reserved at the bar's widest and the press
        // path reads it that way ([`Settings::click`]).
        let geom = scroll::scrollbar(
            after.flow.view,
            &look,
            after.flow.offset,
            after.flow.view.h,
            after.flow.length,
            true,
        )
        .expect("an overflowing rail was given no bar geometry at all");

        // AND IT TAKES THE HAND. A press on the thumb grabs the RAIL's
        // view; the drag that follows moves the rail and not the page.
        let page_at = s.scroll.offset();
        let took = s.click(
            geom.thumb.cx(),
            geom.thumb.y + geom.thumb.h / 2.0,
            720.0 * 16.0 / 9.0,
            720.0,
            None,
        );
        assert!(!took, "a press on the rail's thumb was answered as a control");
        assert!(s.rail_scroll.dragging(), "the rail's thumb did not take the press");
        // Dragged back toward the TOP, which is the direction with room
        // in it: the wheel above has already moved the rail down, and a
        // drag toward an end it may already be resting against would
        // measure the clamp instead of the grab.
        assert!(after.flow.offset > 0.01, "the rail is at its top, so a drag up moves nothing");
        s.drag(geom.thumb.cx(), geom.track.y);
        assert!(
            s.rail_scroll.offset() < after.flow.offset - 0.01,
            "dragging the rail's thumb upward did not move the rail"
        );
        assert!(
            (s.scroll.offset() - page_at).abs() < 0.01,
            "dragging the rail's thumb moved the page"
        );
        s.release();
        assert!(!s.rail_scroll.dragging(), "the rail's thumb was never let go");
        viewport_home();
    }

    /// THE HAIRLINE IS THE THEME'S IN ALL THREE OF THE THINGS IT IS:
    /// how wide it is, where across the indent step it stands, and what
    /// colour it is drawn in.
    ///
    /// WHY A SWEEP OF THEMES AND NOT ONE MEASUREMENT. A reader that had
    /// baked the master's own answers — `1.08` for the stroke, `0.5`
    /// for the place, the resolved ink for the colour — draws exactly
    /// the frame the master asks for, and one measurement against the
    /// master cannot tell it from a reader that asked. So the theme is
    /// MOVED under the window, once per token, and the frame has to
    /// move with it: `rail_guide_x` is driven to both ENDS of its range
    /// (flush with the section's own edge and flush against its pages'),
    /// which is the whole of what that token can say.
    ///
    /// The expectation is rebuilt from the tokens themselves and never
    /// from [`rail_guide_x`], for the reason that function's own test
    /// gives: both sides of an equation drawn from one reader move
    /// together.
    #[test]
    fn the_guide_wears_the_width_the_place_and_the_ink_the_theme_states() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut fonts = nacelle::font::FontSystem::new();

        /// The section's plate and the one vertical hairline the rail
        /// laid beside its pages, at whatever theme is in force.
        fn drawn(fonts: &mut nacelle::font::FontSystem) -> (Rect, [f32; 4], nacelle::theme::Color) {
            theme::resolved();
            theme::set_viewport(1080.0, 1.0);
            // Unfolded: the guide brackets a section's pages, and a shut
            // section has none to bracket.
            let mut s = railed_at(View::LookFeel, &[Act::OpenLookFeel]);
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let bed = Panes::of(Metrics::of(&ctx, content), content)
                .rail
                .expect("the window folded, so there is no rail to measure")
                .bed;
            s.draw(&mut ctx);
            let section = s
                .hits
                .iter()
                .find(|&&(_, a)| a == Act::OpenLookFeel)
                .map(|&(r, _)| r)
                .expect("the rail drew no section to bracket");
            // A rect taller than it is wide, inside the rail: the guide,
            // and nothing else the rail draws is that shape.
            let mut rules: Vec<([f32; 4], nacelle::theme::Color)> = dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::Rect { r, color }
                        if r[2] < r[3] && bed.contains(r[0], r[1]) =>
                    {
                        Some((*r, *color))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(rules.len(), 1, "the rail laid {} vertical rules", rules.len());
            let (r, ink) = rules.remove(0);
            (section, r, ink)
        }

        /// What the FILE asks for, read straight out of the engine.
        fn asked(section_x: f32) -> (f32, f32, nacelle::theme::Color) {
            let t = theme::resolved();
            let px = |n: &str| {
                t.px(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}")))
            };
            let (w, at, step) = (
                px("settings.rail_guide_w"),
                px("settings.rail_guide_x"),
                px("settings.rail_indent"),
            );
            let ink = t.color(
                nacelle::theme::id("component.settings.rail_guide")
                    .expect("no component.settings.rail_guide"),
            );
            (section_x + (step - w).max(0.0) * at, w, ink)
        }

        // The master first, then one theme per thing the guide is. Each
        // body states a value the master does NOT ship, so a reader that
        // had baked the master's answer is caught by the very case it
        // was baked from.
        let cases: [(&str, &str); 5] = [
            ("master", ""),
            ("guide-wide", "[settings]\nrail_guide_w = 1u\n"),
            ("guide-left", "[settings]\nrail_guide_x = 0%\n"),
            ("guide-right", "[settings]\nrail_guide_x = 100%\n"),
            (
                "guide-ink",
                "[component]\nsettings.rail_guide = oklch(0.7000, 0.1500, 30.00 / 1.000)\n",
            ),
        ];
        let mut moved_x: Vec<f32> = Vec::new();
        let mut moved_w: Vec<f32> = Vec::new();
        let mut moved_ink: Vec<[f32; 4]> = Vec::new();
        for (tag, body) in cases {
            let _t = (!body.is_empty()).then(|| crate::widgets::Themed::new(tag, body));
            let (section, g, ink) = drawn(&mut fonts);
            let (want_x, want_w, want_ink) = asked(section.x);
            assert!(want_w > 0.0, "under {tag} the theme states no width for the guide");
            assert!(
                (g[2] - want_w).abs() < 0.01,
                "under {tag} the guide is {} px wide and the theme asked for {want_w}",
                g[2]
            );
            assert!(
                (g[0] - want_x).abs() < 0.01,
                "under {tag} the guide stands at {} and the theme asked for {want_x}",
                g[0]
            );
            let want = col(want_ink);
            assert!(
                (ink.r - want.r).abs() < 0.002
                    && (ink.g - want.g).abs() < 0.002
                    && (ink.b - want.b).abs() < 0.002
                    && (ink.a - want.a).abs() < 0.002,
                "under {tag} the guide is drawn in {} and the theme asked for {}",
                ink.to_hex(),
                want.to_hex()
            );
            // …and the stroke never leaves the step it brackets, at
            // either end of the range.
            assert!(
                g[0] >= section.x - 0.01
                    && g[0] + g[2] <= section.x + rail_indent() + 0.01,
                "under {tag} the guide left the step it brackets"
            );
            moved_x.push(g[0] - section.x);
            moved_w.push(g[2]);
            moved_ink.push([ink.r, ink.g, ink.b, ink.a]);
        }
        // THE SWEEP REALLY SWEPT. Three tokens, three things that had to
        // come out different somewhere — otherwise every case above
        // measured one frame five times and a baked reader walks through.
        assert!(
            moved_x[2] < moved_x[0] - 0.5 && moved_x[3] > moved_x[0] + 0.5,
            "the two ends of settings.rail_guide_x put the stroke in the same \
             place as the middle did: {moved_x:?}"
        );
        assert!(
            (moved_w[1] - moved_w[0]).abs() > 0.5,
            "settings.rail_guide_w did not change the stroke: {moved_w:?}"
        );
        assert!(
            moved_ink[4] != moved_ink[0],
            "component.settings.rail_guide did not change the ink: {moved_ink:?}"
        );
        viewport_home();
    }

    /// THE RAIL'S BREAK BETWEEN TWO ENTRIES IS `settings.rail_row_gap`,
    /// and nothing else — not `modal.row_gap`, which is the rhythm of a
    /// FORM, and not a number in this file.
    ///
    /// [`Metrics::rail`] is the whole of that claim: one field replaced
    /// on the metrics the page uses. It is worth a test of its own
    /// because it is the kind of reader that passes every OTHER test in
    /// this file while baked — the rail is laid and measured through the
    /// same `Metrics`, so a rail whose break was a constant would still
    /// draw and measure identically to itself. Only a THEME that says a
    /// different number can tell the two apart, so that is what this
    /// asks: the master, and a file that doubles the break.
    ///
    /// READ OFF THE FRAME. Two entries that stand one under the other
    /// with nothing between them — GRID and BOARDS, both plain buttons
    /// under one heading — and the distance between the rects the window
    /// really registered them at.
    #[test]
    fn the_rails_break_between_two_entries_is_the_one_the_theme_names() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut fonts = nacelle::font::FontSystem::new();

        /// The gap the frame really left between the two adjacent
        /// entries, at the theme in force.
        fn between(fonts: &mut nacelle::font::FontSystem) -> f32 {
            theme::resolved();
            theme::set_viewport(1080.0, 1.0);
            let mut s = furnished();
            s.view = View::Grid;
            let mut dl = nacelle::draw::DrawList::new();
            let mut ctx = probe(&mut dl, fonts, 1080.0, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            assert!(
                !Panes::of(Metrics::of(&ctx, content), content).folded,
                "the window folded, and a folded window has no rail rhythm"
            );
            s.draw(&mut ctx);
            let at = |act: Act| {
                s.hits
                    .iter()
                    .find(|&&(_, a)| a == act)
                    .map(|&(r, _)| r)
                    .expect("the rail drew neither GRID nor BOARDS")
            };
            at(Act::OpenBoards).y - at(Act::OpenGrid).bottom()
        }

        /// What the FILE asks for, read straight out of the engine.
        fn asked() -> (f32, f32) {
            let t = theme::resolved();
            let px = |n: &str| {
                t.px(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}")))
            };
            (px("settings.rail_row_gap"), px("modal.row_gap"))
        }

        for (tag, body) in [("master", ""), ("dense-rail", "[settings]\nrail_row_gap = 4u\n")] {
            let _t = (!body.is_empty()).then(|| crate::widgets::Themed::new(tag, body));
            let (rail_gap, form_gap) = asked();
            let got = between(&mut fonts);
            assert!(
                (got - rail_gap).abs() < 0.01,
                "under {tag} the rail broke {got} px between two entries and \
                 settings.rail_row_gap is {rail_gap}"
            );
            assert!(
                (rail_gap - form_gap).abs() > 0.5,
                "under {tag} the rail's break and the form's are the same number \
                 ({rail_gap} vs {form_gap}), so this measurement cannot tell a \
                 rail that reads the wrong token from one that reads the right one"
            );
        }
        viewport_home();
    }

    /// ŻYCZENIE 1, MEASURED HERE — RE-DECIDED TWICE ON 2026-08-18. The
    /// NAVIGATION is ONE bed, the page keeps the window body under it,
    /// every colour comes out of the theme, and neither of them is BLACK.
    ///
    /// WHAT THE OWNER ASKED AND WHAT ANSWERED IT. First: "mają być po
    /// całości i obie w jednakowym kolorze, tym w środkowej kolumnie" —
    /// two adjacent navigation strips at two shades read as a seam
    /// through one object. That was answered by pointing both at one
    /// colour, and this test then read the two painted beds back out of a
    /// recorded draw list and compared their channels. Then his mock-up
    /// took the second column away altogether, and the strongest form of
    /// the same claim came with it: there is ONE bed to paint, so there
    /// is no seam to measure and no second token to drift.
    ///
    /// The window's half of the claim is what this can check: that the
    /// bed is the box [`Panes`] cut, wears the corner the THEME states,
    /// and carries the colour ITS OWN TOKEN resolves to — no colour mixed
    /// in Rust, no radius written here, and no bed painted off a name it
    /// does not own. That the rail's token resolves to one colour a step
    /// above the body is the MASTER's arrangement, measured over the
    /// master in libnacelle
    /// (`the_navigation_band_is_one_bed_over_the_body`).
    ///
    /// ONE OF THE TWO IS PAINTED under the master and the other is
    /// NAMED: `component.settings.page_fill` ships as the sentinel `none`
    /// because the page's bed is the WINDOW BODY, `component.panel.fill`,
    /// and that rung is translucent — laying a bed of it over the body
    /// composes its alpha twice (#131E19 against #15201B over the field
    /// the window stands on, an OKLab dE of 0.0078). A theme that gives
    /// the token a colour gets a second band, and that is measured here as
    /// well: a name with no reader is exactly what the previous shape of
    /// this window was rightly held to account for.
    ///
    /// WHY BLACKNESS IS AN ASSERTION OF ITS OWN. The step gate below reads
    /// OKLab L, and OKLab L cannot tell a dark shade from black — the two
    /// beds the owner photographed were an honest 0.063 apart by that
    /// ruler and the sRGB codes 6 and 19 on the screen. `off_black` is
    /// WCAG contrast against pure black, whose 0.05 flare pedestal makes
    /// it steepest exactly where OKLab L flattens.
    ///
    /// Every lightness and hue below is read in LINEAR light: OKLCh is
    /// defined over it, the bake answers sRGB-encoded, and the two are
    /// far apart — the master's two beds are 0.2320 and 0.2784 decoded,
    /// and the first of them alone reads 0.4840 encoded.
    #[test]
    fn the_navigation_is_one_bed_the_theme_chose() {
        let _g = crate::widgets::theme_test_lock();
        // The MASTER's own bands, so a theme-editor preview left standing
        // by another test is not what this measures — a preview moves
        // `component.panel.fill` (`edit::glass_edits`) and would answer
        // for the body with a colour nobody in this test chose.
        nacelle::theme::clear_preview();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        /// Every bed one call to [`Settings::draw_bands`] laid down: the
        /// box, the four corners it was cut with, and the colour.
        type Bed = ([f32; 4], [nacelle::draw::Corner; 4], nacelle::theme::Color);
        fn bands(
            s: &Settings,
            fonts: &mut nacelle::font::FontSystem,
            h: f32,
        ) -> (Vec<Bed>, Panes, Rect) {
            // Recording, so the bands can be read back as commands: the
            // shipping list keeps no register at all.
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(m, content);
            s.draw_bands(&mut ctx, &nav);
            let out = ctx
                .dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    // A BED IS A RING FILL, whatever the cut. `ring_fill`
                    // keeps its own name in the register even when every
                    // corner is square and the tessellator takes the
                    // one-quad path, which is what lets this test ask a
                    // SQUARE theme the same question it asks a round one.
                    nacelle::draw::DrawCmd::RingFill { r, corners, color } => {
                        Some((*r, *corners, *color))
                    }
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
        /// How far a bed stands off pure black, as WCAG 2.x contrast.
        /// The same ruler libnacelle's band test uses, and for the reason
        /// given there: `(Y + 0.05)/0.05` moves fastest in the region
        /// OKLab L flattens, so it is the one measure that answers "is
        /// this a shade or a black stripe".
        fn off_black(c: nacelle::theme::Color) -> f32 {
            let black = nacelle::theme::Color::from_hex("#000000").expect("black");
            nacelle::theme::Color::wcag_contrast(c.to_linear(), black.to_linear())
        }
        /// The floor a whole COLUMN of interface must clear, set between
        /// two of the master's own rungs: `@surface.panel` — the window
        /// body, the darkest bed this master stands a control on — reads
        /// 1.26, and `@surface.base`, one of the two rungs the columns
        /// used to be pinned to, reads 1.12.
        const NOT_BLACK: f32 = 1.15;

        theme::resolved();
        theme::set_viewport(1080.0, 1.0);
        let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
        assert!(!nav.folded, "the window folded at a width it fits in");
        assert_eq!(drawn.len(), 1, "one column to bed; the page's is the sentinel");
        // The band is the column's own rectangle — the same cut the rows
        // are laid in, or the bed and what stands on it would disagree
        // about where the column is.
        let boxes: Vec<[f32; 4]> = drawn.iter().map(|(r, _, _)| *r).collect();
        let same = |b: &[f32; 4], r: Rect| {
            let want = [r.x, r.y, r.w, r.h];
            b.iter().zip(want.iter()).all(|(a, w)| (a - w).abs() < 0.01)
        };
        assert!(
            boxes.iter().any(|b| same(b, nav.rail.expect("no rail").bed)),
            "the rail has no bed of its own"
        );
        // Every colour is the one ITS token resolves to: the window
        // carries a name to the theme and paints back what it is given.
        let th = theme::resolved();
        let of = |n: &str| {
            col(th.color(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}"))))
        };
        let px = |n: &str| th.px(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}")));
        // The page's bed is DECLARED, and what it is declared as is the
        // sentinel: the name exists so a theme may bed the page, and the
        // master declines because the body is already standing there.
        assert_eq!(
            of("component.settings.page_fill").a,
            0.0,
            "the master bedded the page a second time; the body's `panel.fill` is it"
        );
        let want = [of("component.settings.rail_fill")];
        for (i, (_, _, got)) in drawn.iter().enumerate() {
            let w = want[i];
            assert!(
                (got.r - w.r).abs() < 1e-6
                    && (got.g - w.g).abs() < 1e-6
                    && (got.b - w.b).abs() < 1e-6,
                "band #{i} was not painted in the colour its token names"
            );
        }

        // THE CORNER COMES FROM THE THEME, and from both halves of the
        // pair: `settings.band_corner` is the length and
        // `settings.band_corner_mode` the cut. All four corners carry the
        // same one — a band stands `modal.pad` clear of the modal's frame
        // on every side that could meet it, so it is a plate lying ON the
        // body and not a piece cut out of it, and a plate is cut the same
        // all round.
        let radius = px("settings.band_corner");
        assert!(radius > 0.0, "the master states no radius for a column's bed");
        for (i, (_, corners, _)) in drawn.iter().enumerate() {
            for (k, c) in corners.iter().enumerate() {
                assert_eq!(
                    c.style,
                    nacelle::draw::CornerStyle::Round,
                    "band #{i} corner {k} is not the cut `corner.mode` states"
                );
                assert!(
                    (c.size - radius).abs() < 0.01,
                    "band #{i} corner {k} is {} px where the theme says {radius}",
                    c.size
                );
            }
        }

        // AND NEITHER BED IS BLACK. The one the window painted, and the
        // body standing where the page is — which is the second bed as
        // far as the eye is concerned.
        let body = col(th.bed(nacelle::theme::id("component.panel.fill").expect("no body")));
        for (name, c) in [("rail", want[0]), ("page", body)] {
            assert!(
                off_black(c) >= NOT_BLACK,
                "the {name} column reads {} against pure black — a black stripe, \
                 not a shade of the theme",
                off_black(c)
            );
        }
        // The gate is the one that catches the defect and not one every
        // colour passes: the rung the rail used to be pinned to is below
        // it, measured rather than remembered.
        assert!(
            off_black(of("surface.void")) < NOT_BLACK,
            "`surface.void` now clears the black floor, so this gate no longer \
             separates a column's bed from the swapchain clear colour"
        );

        // AND THE PAGE KEEPS THE BODY'S OWN PIXEL. Nothing is laid over
        // it, so what stands there is what `window::frame` laid.
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
        // THE OWNER'S FIRST ASK IS NOW STRUCTURAL. It was "one colour
        // across both navigation columns", and this test used to read
        // the two painted beds back and compare their channels. There is
        // ONE bed since the columns became one, so the assertion that
        // replaces it is the count above — a second navigation band
        // would have to be painted before it could be painted wrong.

        // ONE HUE, AND THE NAVIGATION A STEP OFF THE PAGE — the owner's
        // "hue ten sam, odcień koloru inny", read off what the window
        // really shows: the bed it laid, and the BODY standing where the
        // page is.
        let (page, rail) = (lch(body), lch(want[0]));
        // Two degrees, which is what libnacelle holds each rung to
        // against the SEED over the master — read in linear light they
        // sit on ONE number because they come out of ONE token, and the
        // tolerance is float noise and the sRGB rounding. Read encoded
        // they spread nearly three degrees and this assertion would
        // fail, which is the point of the space.
        assert!(
            hue_gap(page.h, rail.h) < 2.0,
            "page/rail: two settings beds are two COLOURS ({} vs {} deg), not two shades",
            page.h,
            rail.h
        );
        // The master's two, decoded: 0.2320 and 0.2784, a step of 0.046.
        assert!(
            (page.l - rail.l).abs() > 0.03,
            "the navigation and the page are the same shade ({} vs {})",
            page.l,
            rail.l
        );
        // AND THE NAVIGATION LIES ON THE PAGE, not under it: the page is
        // the well and the chrome you steer with lies on the thing you
        // are steering.
        assert!(
            page.l < rail.l,
            "the navigation is not a bed over the body: {} {}",
            page.l,
            rail.l
        );

        // FOLDED: NO bed at all. There are no columns to shade
        // differently — the page is the whole interior and the body is
        // already standing on the page's own bed — so the folded window
        // looks exactly as it did before any of this existed.
        //
        // ASKED FOR RATHER THAN FOUND. Since the navigation became one
        // column the master keeps its columns at every height the
        // program is built for, so a sweep over HEIGHTS would never
        // reach this shape at all ([`folding_theme`] says why, and why
        // that is the change rather than a hole).
        {
            let _t = folding_theme();
            let mut folded_seen = false;
            for h in [HEIGHTS[0], HEIGHTS[4]] {
                theme::set_viewport(h, 1.0);
                let (drawn, nav, _) = bands(&s, &mut fonts, h);
                assert!(nav.folded, "the folding theme did not fold the window at {h}px");
                folded_seen = true;
                assert!(drawn.is_empty(), "a folded window bedded its interior twice");
            }
            assert!(folded_seen, "the folded band was never measured");
        }
        theme::set_viewport(1080.0, 1.0);

        // A THEME MOVES BOTH AT ONCE, through the one token the master
        // anchors them to. This is the divergence the owner
        // photographed — the page followed the editor's BACKGROUND
        // sliders and the navigation did not — written as the theme
        // those sliders write.
        {
            let _t = crate::widgets::Themed::new(
                "page-bed",
                "[component]\npanel.fill = oklch(0.4200, 0.0400, 292.00 / 0.820)\n",
            );
            theme::set_viewport(1080.0, 1.0);
            let th = theme::resolved();
            let of = |n: &str| {
                col(th.color(nacelle::theme::id(n).unwrap_or_else(|| panic!("no {n}"))))
            };
            let moved = lch(col(th.bed(
                nacelle::theme::id("component.panel.fill").expect("no body"),
            )));
            assert!((moved.l - 0.42).abs() < 0.01, "the fixture did not move the body");
            let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
            assert!(!nav.folded);
            assert_eq!(drawn.len(), 1, "a moved body grew the window a second bed");
            let c = lch(of("component.settings.rail_fill"));
            assert!(
                hue_gap(c.h, moved.h) < 2.0,
                "the rail stayed on the old hue while the body moved: {} vs {}",
                c.h,
                moved.h
            );
            assert!(
                c.l > moved.l + 0.03,
                "the rail did not climb off the body it follows: {} vs {}",
                c.l,
                moved.l
            );
        }

        // AND THE PAGE'S NAME IS NOT DECORATION: give it a colour and
        // this window beds the page too, in the box `Panes` cut for it.
        {
            let _t = crate::widgets::Themed::new(
                "page-bedded",
                "[component]\nsettings.page_fill = @surface.sunken\n",
            );
            theme::set_viewport(1080.0, 1.0);
            let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
            assert!(!nav.folded);
            assert_eq!(drawn.len(), 2, "the page's own token was not honoured");
            assert!(
                drawn.iter().any(|(b, _, _)| same(b, nav.page)),
                "the second bed is not the page's box"
            );
        }

        // AND THE CUT IS THE THEME'S, not this window's: a master word
        // away, every bed goes square.
        {
            let _t = crate::widgets::Themed::new("square-beds", "[corner]\nmode = square\n");
            theme::set_viewport(1080.0, 1.0);
            let (drawn, nav, _) = bands(&s, &mut fonts, 1080.0);
            assert!(!nav.folded);
            assert!(!drawn.is_empty(), "nothing was drawn to check the cut on");
            for (i, (_, corners, _)) in drawn.iter().enumerate() {
                for c in corners {
                    assert_eq!(
                        c.style,
                        nacelle::draw::CornerStyle::Square,
                        "band #{i} kept its arc under a theme that asked for square corners"
                    );
                }
            }
        }
        viewport_home();
    }

    /// "PO CAŁOŚCI" (owner, 2026-08-18). A navigation column's bed fills
    /// the column it beds — the top edge of the content box to the bottom
    /// edge — and the paint really lands there, at every window height
    /// the program is built for.
    ///
    /// WHAT MADE THE ISLANDS, measured rather than remembered. One
    /// rectangle used to answer two questions — where a column's colour
    /// goes and where its buttons go — so the bed could only begin where
    /// the first button did: `content.y + button.h + modal.row_gap`. On
    /// the master at 1080p that is 45.4 + 16.2 = 61.6 px of bare window
    /// body above EVERY navigation band and none at all above the PAGE
    /// column, which was always cut from `content.y`. Two columns short
    /// at the top beside one that was not is exactly the "wyspy" in the
    /// screenshot. [`Column`] splits the two questions, and this is the
    /// assertion that says so.
    ///
    /// THE BOTTOM EDGE WAS NEVER THE FAULT and is asserted all the same,
    /// so a later change cannot open the gap at the other end instead.
    ///
    /// THE WINDOW'S OWN MARGIN IS NOT IN THIS CLAIM. `content_rect` keeps
    /// `modal.pad` clear of the frame on the sides and the bottom and
    /// drops `modal.body_top` for the title band. A band fills its AREA;
    /// the window's margin is still the window's, which is also why
    /// `settings.band_corner` still describes a plate lying ON the body
    /// rather than a piece cut out of it.
    #[test]
    fn every_navigation_bed_fills_the_column_it_beds() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut measured = 0;
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            // Recorded, so the question is what was PAINTED and not only
            // what was computed.
            let mut dl = nacelle::draw::DrawList::recording();
            let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(m, content);
            if nav.folded {
                continue;
            }
            measured += 1;
            s.draw_bands(&mut ctx, &nav);
            let painted: Vec<[f32; 4]> = ctx
                .dl
                .cmds()
                .iter()
                .filter_map(|c| match c {
                    nacelle::draw::DrawCmd::RingFill { r, .. } => Some(*r),
                    _ => None,
                })
                .collect();
            for (name, bed) in [("rail", nav.rail.expect("no rail").bed)] {
                assert!(
                    (bed.y - content.y).abs() < 0.01,
                    "at {h}px the {name} bed starts {} px below the content box",
                    bed.y - content.y
                );
                assert!(
                    (bed.bottom() - content.bottom()).abs() < 0.01,
                    "at {h}px the {name} bed stops {} px above the content box",
                    content.bottom() - bed.bottom()
                );
                assert!(
                    painted.iter().any(|r| (r[1] - bed.y).abs() < 0.01
                        && (r[3] - bed.h).abs() < 0.01
                        && (r[0] - bed.x).abs() < 0.01
                        && (r[2] - bed.w).abs() < 0.01),
                    "at {h}px the {name} bed was not painted over the whole column: \
                     wanted {:?}, painted {painted:?}",
                    [bed.x, bed.y, bed.w, bed.h]
                );
            }
            // AND ALL THREE COLUMNS START AND END ON ONE LINE. The page
            // is the one that never had the notch, so it is the ruler.
            assert!(
                (nav.page.y - content.y).abs() < 0.01
                    && (nav.page.bottom() - content.bottom()).abs() < 0.01,
                "at {h}px the page column no longer spans the content box"
            );
        }
        assert!(measured > 0, "no window height in HEIGHTS stands in columns at all");
        viewport_home();
    }

    /// "ŻADNYCH PADDINGÓW NIE MA, TOTALNA AMATORKA" (owner, 2026-08-18):
    /// nothing standing in a navigation column touches the bed it stands
    /// on, on any of its four sides.
    ///
    /// WHAT THIS IS ABOUT. A control flush with the plate under it stops
    /// reading as a thing LYING ON the plate and starts reading as a
    /// slice OF it — which is what the rail's entries and the section
    /// headings did, left edge to right edge, because the box they were
    /// laid in WAS the box that was painted.
    ///
    /// The air comes from the theme (`settings.band_pad_x` and
    /// `band_pad_y`) and is asserted to be a real length first: a test
    /// that only checked "inside the bed" would pass a theme, or a Rust
    /// reader, that had quietly gone back to no padding at all.
    ///
    /// READ OFF THE FRAME THE WINDOW REALLY DREW. `Settings::draw` fills
    /// the hit map, so every entry, every heading's row and the chrome
    /// button at the head of the rail are asked the same question — and
    /// the chrome button matters most, because it is the one that used to
    /// be placed by a rule of its own (`settings.back_w_frac` against the
    /// content box) with no idea a bed was under it.
    ///
    /// AT EVERY HEIGHT, like its twin above. The air is a THEME LENGTH
    /// (`@space.4`), so it doubles with the viewport — 10.8 px at 1080p,
    /// 21.6 at 2160p — while the widths it has to stay inside of scale
    /// on a different rule (`rail_w_frac` against the content box, under
    /// two floors in device px). A reader that took the padding once and
    /// spent it at every scale would pass at 1080p and eat the column
    /// somewhere else, and a measurement taken at one height could not
    /// tell.
    #[test]
    fn nothing_in_a_navigation_column_touches_the_bed_it_stands_on() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut measured = 0;
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            // Read INSIDE the sweep: the air is a viewport length, so the
            // number this height was drawn with is the only one that can
            // judge it.
            let (pad_x, pad_y) = band_pad();
            assert!(
                pad_x > 0.5 && pad_y > 0.5,
                "at {h}px the theme states no air around a column's bed \
                 ({pad_x} x {pad_y}) — the fault the owner reported is the \
                 absence of it"
            );

            let mut s = furnished();
            s.view = View::LookFeel;
            let mut fc = FocusCtl::new();
            let mut dl = nacelle::draw::DrawList::new();
            fc.begin_frame();
            let mut ctx = probe(&mut dl, &mut fonts, h, 1.0);
            ctx.focus = Some(&mut fc);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(m, content);
            // Folded there is no bed and nothing stands on one: the
            // entries are ordinary bands in the flow, which is the
            // scroll's ground and another test's.
            if nav.folded {
                continue;
            }
            measured += 1;
            s.draw(&mut ctx);

            for (name, col_) in [("rail", nav.rail.expect("no rail"))] {
                // The room inside the paint, stated once: the rows box is
                // the bed less its air, and every side of it is checked,
                // because a fix that only insets the sides leaves the
                // owner's other complaint — the heading welded to the top
                // edge — standing.
                let (bed, rows) = (col_.bed, col_.rows);
                for (side, got) in [
                    ("left", rows.x - bed.x),
                    ("right", bed.right() - rows.right()),
                    ("top", rows.y - bed.y),
                    ("bottom", bed.bottom() - rows.bottom()),
                ] {
                    let want =
                        if side == "left" || side == "right" { pad_x } else { pad_y };
                    assert!(
                        got >= want - 0.01,
                        "at {h}px the {name} column's rows stand {got} px from the \
                         {side} edge of their bed; the theme asked for {want}"
                    );
                }
                // AND THE FRAME AGREES. Everything the window registered
                // inside this column — its entries and, for the rail, the
                // chrome button at its head — stands inside that room.
                let air = Rect::new(
                    bed.x + pad_x,
                    bed.y + pad_y,
                    bed.w - 2.0 * pad_x,
                    bed.h - 2.0 * pad_y,
                );
                let mut seen = 0;
                for (r, act) in s.hits.iter() {
                    if !bed.contains(r.cx(), r.y + r.h / 2.0) {
                        continue;
                    }
                    seen += 1;
                    // Named by the id the chain knows it as: `Act` has no
                    // Debug and giving it one drags four more enums with
                    // it, and the id is the same handle the focus tests
                    // print.
                    assert!(
                        r.x >= air.x - 0.01
                            && r.right() <= air.right() + 0.01
                            && r.y >= air.y - 0.01
                            && r.bottom() <= air.bottom() + 0.01,
                        "at {h}px a control ({}) in the {name} column is flush with \
                         the bed it stands on: {:?} against the room {:?}",
                        focus_id(*act).0,
                        [r.x, r.y, r.w, r.h],
                        [air.x, air.y, air.w, air.h]
                    );
                }
                assert!(
                    seen > 0,
                    "at {h}px the {name} column registered nothing to measure"
                );
            }
            // THE CHROME BUTTON IS ONE OF THEM, named rather than left to
            // the sweep: it is the head of the RAIL and the one control
            // this window used to place against the content box instead.
            let corner = s
                .hits
                .iter()
                .find(|(_, a)| matches!(a, Act::Back | Act::Close))
                .map(|&(r, _)| r);
            let corner = corner.unwrap_or_else(|| {
                panic!("at {h}px the frame carried no way out, so the sweep never measured it")
            });
            // AND IT LINES UP WITH WHAT STANDS UNDER IT. The rail keeps
            // a lane for its own scrollbar out of the box its ENTRIES
            // are laid in ([`rows_box`]); a chrome button placed against
            // the room instead of against the entries would be some
            // sixteen pixels wider than every button beneath it, which
            // reads as a button that failed to line up rather than as
            // the head of the column it is.
            let entry = s
                .hits
                .iter()
                .find(|&&(_, a)| a == Act::OpenLookFeel)
                .map(|&(r, _)| r)
                .expect("the rail drew no top-level entry to line the button up with");
            assert!(
                (corner.x - entry.x).abs() < 0.01
                    && (corner.right() - entry.right()).abs() < 0.01,
                "at {h}px the chrome button runs {:?} and the entry under it {:?} — \
                 the head of the rail is not the width of the rail",
                (corner.x, corner.right()),
                (entry.x, entry.right())
            );
        }
        assert!(measured > 0, "no window height in HEIGHTS stands in columns at all");
        viewport_home();
    }

    /// THE PAGE FOLLOWS THE BUTTON IT HANGS UNDER. A page's first row
    /// stands its own lead below the chrome button's row — wherever that
    /// row is — and the rail's first entry stands its OWN break under
    /// the same button.
    ///
    /// THE TWO USED TO BE ONE LINE and are deliberately not any more.
    /// While the rail and the pages shared `modal.row_gap` the first
    /// entry and the first row landed together, and this test held them
    /// there. Since 2026-08-18 the rail has a rhythm of its own
    /// (`settings.rail_row_gap`, [`Metrics::rail`]) because it carries
    /// the open section's pages and cannot afford a form's breaks — so
    /// the rail's first entry stands HIGHER than the page's first row,
    /// by exactly the difference between the two tokens. That is the
    /// claim now, and it is a claim about the theme's two numbers rather
    /// than about one: a rail that drifted off the button for any other
    /// reason still fails.
    ///
    /// THE FAULT THIS CATCHES was made by the fix beside it. Moving the
    /// chrome button onto the rail's bed (`settings.band_pad_y` down from
    /// the bed's top edge) left [`body_top`] still measuring the page
    /// from `content.y`, so the room under the button shrank by the whole
    /// padding — 16.2 px to 5.4 at 1080p on the master — and the first
    /// rail entry, which had always started on the page's first line,
    /// dropped `band_pad_y` below it. Two spacing faults introduced by a
    /// spacing fix, neither of them visible to a test that asked only
    /// where the beds and the rows boxes were.
    ///
    /// BOTH SIDES OF IT ARE ASSERTED. The computed side sweeps every page
    /// at every height, because `lead` differs page by page and the
    /// padding is a viewport length; the drawn side takes the frame the
    /// window really laid at 1080p and compares the rect the CHROME
    /// BUTTON was registered with against the box the flow was really
    /// given ([`Flow`]), so a `body_top` that agreed with `Panes` while
    /// the drawing did something else would still be caught.
    #[test]
    fn a_page_starts_its_own_lead_under_the_chrome_row_it_shares_with_the_rail() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        let s = furnished();
        let mut fonts = nacelle::font::FontSystem::new();
        let mut dl = nacelle::draw::DrawList::new();
        let mut lined_up = 0;
        for h in HEIGHTS {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            for p in PAGES.iter() {
                let nav = Panes::of(m, content);
                let top = s.body_box(p, m, content).y;
                let want = nav.corner.bottom() + m.space(p.lead);
                assert!(
                    (top - want).abs() < 0.01,
                    "{} at {h}px: the body starts {} px under the chrome button \
                     and the page asked for {}",
                    p.title,
                    top - nav.corner.bottom(),
                    m.space(p.lead)
                );
                // AND THE RAIL HANGS ITS OWN BREAK UNDER THE SAME
                // BUTTON ([`Panes::of`]). A page leading with `Gap::Row`
                // therefore stands exactly `modal.row_gap −
                // settings.rail_row_gap` below the rail's first entry —
                // both numbers the theme's, neither of them written
                // here. Set the two tokens equal in a theme and the two
                // are one line again, which is the alignment this
                // window had before the rail needed a rhythm of its own.
                if let (Some(rail), true) = (nav.rail, p.lead == Gap::Row) {
                    lined_up += 1;
                    let step = m.gap - m.rail().gap;
                    assert!(
                        (top - rail.rows.y - step).abs() < 0.01,
                        "{} at {h}px: the page's first row stands {} px under the \
                         rail's first entry and the two rhythms differ by {step}",
                        p.title,
                        top - rail.rows.y
                    );
                }
            }
        }
        assert!(lined_up > 0, "no page in HEIGHTS ever stood beside a rail at all");

        // AND THE FRAME AGREES, read off one real draw: the rect the
        // chrome button was registered with, against the box the flow was
        // given.
        let mut s = furnished();
        s.view = View::LookFeel;
        let page = page(View::LookFeel);
        let mut fc = FocusCtl::new();
        let mut dl = nacelle::draw::DrawList::new();
        fc.begin_frame();
        theme::set_viewport(1080.0, 1.0);
        let mut ctx = probe(&mut dl, &mut fonts, 1080.0, 1.0);
        ctx.focus = Some(&mut fc);
        let content = content_rect(modal_rect(ctx.w, ctx.h));
        let m = Metrics::of(&ctx, content);
        assert!(
            !Panes::of(m, content).folded,
            "the window folded at a width it fits in"
        );
        s.draw(&mut ctx);
        let corner = s
            .hits
            .iter()
            .find(|(_, a)| matches!(a, Act::Back | Act::Close))
            .map(|(r, _)| *r)
            .expect("the frame carried no way out to measure from");
        let slack = s.flow.view.y - corner.bottom();
        assert!(
            (slack - m.space(page.lead)).abs() < 0.01,
            "the frame left {slack} px between the chrome button and the page, \
             where the page's lead is {} px",
            m.space(page.lead)
        );
        viewport_home();
    }


    /// EVERY SECTION THE RAIL HOLDS CAN BE REACHED — on every page, on
    /// both machines, at every window height the program is built for.
    ///
    /// Fail-closed, and the property it guards changed shape on
    /// 2026-08-18. A rail is clipped to its column, so an entry past the
    /// bottom edge is drawn nowhere and is in no hit map; the first
    /// draft of the one-column rail answered that by FOLDING the whole
    /// window wherever the rail wanted more height than it had, which
    /// took the two-panel shape away from 720p and 768p — screens that
    /// had stood in columns before. The rail scrolls now
    /// ([`Settings::rail_scroll`]), so the claim is REACHABILITY and no
    /// longer fitting: an entry may be off the frame, and the wheel has
    /// to be able to fetch it back.
    ///
    /// THREE THINGS ARE ASSERTED, and the middle one is what keeps the
    /// other two honest:
    ///
    /// * the window stands in COLUMNS at every height in the ladder and
    ///   at the two heights the regression was measured at (768 and
    ///   800), on both machines — a fold here is the regression coming
    ///   back;
    /// * somewhere in that ladder the rail really does want more than
    ///   its box, or the scroll this test is about is never exercised;
    /// * every act the rail describes — the sections AND the pages the
    ///   open one unfolds — is in the hit map at one of the offsets
    ///   [`rail_stops`] walks.
    ///
    /// AND IT IS THE MEASUREMENT THAT PAID FOR THE SINGLE-OPEN RULE
    /// BEING DROPPED ([`Settings::rail_open`], decision (a)). It used to
    /// be cited the other way round — the unfold has to COST height, or
    /// the bound single-open buys is a bound on nothing — and that
    /// reading died with the fitting claim it stood on. What matters now
    /// is the middle assertion below: the rail really does outgrow its
    /// column somewhere in the ladder AND the wheel fetches the overflow
    /// back, which is why the sum of every section's pages is no longer
    /// something the window has to be protected from.
    ///
    /// EVERY SECTION UNFOLDED AT ONCE, which is the same claim made as
    /// hard as this rail can make it: the longest rail the description
    /// can produce is reachable end to end. A sweep that unfolded one
    /// section would leave the case decision (a) allows untested.
    ///
    /// BOTH MACHINES, and the second is the taller one. `furnished()`
    /// has a colour manager, and a rail measured only there never
    /// carries the NO COLOR MANAGER note at all — while the machine that
    /// DOES carry it keeps the greyed COLOR SPACE entry too (R6 paints
    /// an unofferable section shut, it does not remove it), so the shut
    /// rail is strictly the longer of the two. Measuring the short one
    /// and calling the property proved is how a fail-closed test comes
    /// to guard everything except the case that grew.
    #[test]
    fn every_section_the_rail_holds_can_be_reached_at_every_window() {
        let _g = crate::widgets::theme_test_lock();
        nacelle::theme::clear_preview();
        /// Every section the description can unfold. Read off the table
        /// rather than named: a section given pages tomorrow is swept by
        /// this test without anybody remembering to add it here.
        fn expander_acts() -> Vec<Act> {
            RAIL_ROWS
                .iter()
                .filter_map(|r| match r.ctrl {
                    Ctrl::Expander { act, .. } => Some(act),
                    _ => None,
                })
                .collect()
        }
        /// One window, on one page, on one of the two machines, with
        /// the sections in `open` unfolded.
        fn rail_of(view: View, colour_manager: bool, open: &[Act]) -> Settings {
            let mut s = railed_at(view, open);
            s.color_enabled = colour_manager;
            s
        }
        assert!(
            furnished().color_enabled,
            "the fixture lost its colour manager, so the two machines are one"
        );
        let mut fonts = nacelle::font::FontSystem::new();
        let mut measured = 0;
        let mut unfolded = 0;
        let mut overflowed = 0;
        // The ladder, plus the two heights the fold regression was
        // measured at: 768 is a 1366x768 laptop and 800 is where the
        // machine with no colour manager crossed over.
        let ladder: Vec<f32> =
            HEIGHTS.iter().copied().chain([768.0, 800.0]).collect();
        for h in ladder {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            for p in PAGES.iter() {
                let all = expander_acts();
                let (open, shut) =
                    (rail_of(p.view, true, &all), rail_of(p.view, false, &all));
                let nav = Panes::of(m, content);
                // The point of the second state, stated so it cannot
                // quietly stop being true: a machine with no colour
                // manager keeps the greyed entry AND gains the note, so
                // its rail is the longer one. If the two ever measure
                // the same, this loop is running twice over one rail.
                if let Some(rail) = nav.rail.map(|c| c.rows) {
                    assert!(
                        shut.rows_h(&RAIL_ROWS, m.rail(), rail)
                            > open.rows_h(&RAIL_ROWS, m.rail(), rail),
                        "at {h}px the shut rail is no taller than the open one — \
                         the case this test was widened for is not being measured"
                    );
                }
                for (which, s) in [("with a colour manager", &open), ("without one", &shut)] {
                    // THE REGRESSION GUARD. The master keeps its columns
                    // at every height this program is built for; a fold
                    // here means the window has gone back to trading its
                    // whole shape for a rail that would not fit.
                    let Some(rail) = nav.rail.map(|c| c.rows) else {
                        panic!(
                            "at {h}px, {which}, {} folded the whole window — the \
                             master keeps two panels at every height the program \
                             is built for",
                            p.title
                        );
                    };
                    measured += 1;
                    let want = s.rows_h(&RAIL_ROWS, m.rail(), rows_box(rail));
                    if want > rail.h + 0.01 {
                        overflowed += 1;
                    }
                    // The pages the open sections unfold are IN that
                    // number: `rows_h` recurses into every section the
                    // window has open ([`Settings::rows_span`]).
                    // Measured against the SAME rail on the SAME page
                    // with nothing unfolded, so the difference is
                    // exactly what the unfolds cost. The reference used
                    // to be another PAGE — a section that had no pages
                    // to unfold — which stopped being a difference in
                    // the fold the day the fold stopped following the
                    // page ([`Settings::rail_open`]).
                    let plain = rail_of(p.view, s.color_enabled, &[])
                        .rows_h(&RAIL_ROWS, m.rail(), rows_box(rail));
                    if all.is_empty() {
                        assert!(
                            (want - plain).abs() < 0.01,
                            "at {h}px, {which}, {} unfolds nothing and still costs \
                             {want} px against the plain rail's {plain}",
                            p.title
                        );
                    } else {
                        unfolded += 1;
                        assert!(
                            want > plain + 0.01,
                            "at {h}px, {which}, {} unfolds pages and the rail did not \
                             grow for them: {want} against {plain}",
                            p.title
                        );
                    }
                }
            }
        }
        assert!(measured > 0, "no height in the ladder drew a rail at all");
        assert!(
            unfolded > 0,
            "no page in the sweep unfolded a section, so the height this test was \
             widened for was never measured"
        );
        assert!(
            overflowed > 0,
            "the rail never wanted more room than it had anywhere in the ladder, so \
             the scroll this test is about was never exercised"
        );

        // AND EVERY ENTRY IS REACHED, off the frames the window really
        // draws, at the shortest window in the ladder — the one where
        // the rail overflows hardest. The wheel is what the reader has;
        // [`rail_stops`] is that wheel, walked to the end.
        theme::resolved();
        theme::set_viewport(HEIGHTS[0], 1.0);
        for p in PAGES.iter() {
            for colour_manager in [true, false] {
                let all = expander_acts();
                let reference = rail_of(p.view, colour_manager, &all);
                // The unfolded sections' pages are already IN this:
                // `row_acts` recurses into every section the window has
                // open. It used to need `kid_acts` welded on beside it,
                // because the description's walk and the window's fold
                // could disagree about which section that was.
                let want: Vec<Act> = nav_row_acts(&reference, &RAIL_ROWS);
                assert!(!want.is_empty(), "the rail describes nothing to reach");
                let stops: Vec<f32> = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, HEIGHTS[0], 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    let m = Metrics::of(&ctx, content);
                    let mut out = vec![0.0];
                    out.extend(rail_stops(&reference, m, content));
                    out
                };
                let mut seen: Vec<Act> = Vec::new();
                for stop in stops {
                    let mut s = rail_of(p.view, colour_manager, &all);
                    s.rail_scroll.set_offset(stop);
                    let mut dl = nacelle::draw::DrawList::new();
                    let mut ctx = probe(&mut dl, &mut fonts, HEIGHTS[0], 1.0);
                    s.draw(&mut ctx);
                    for &(_, act) in s.hits.iter() {
                        if !seen.contains(&act) {
                            seen.push(act);
                        }
                    }
                }
                if let Some(missing) = want.iter().position(|a| !seen.contains(a)) {
                    panic!(
                        "{} at {}px: entry #{missing} of the {} the rail holds is \
                         reachable at no offset the wheel can take it to",
                        p.title,
                        HEIGHTS[0],
                        want.len()
                    );
                }
            }
        }
        viewport_home();
    }

    /// M4 in the large — the whole window folds, and the FOCUS CHAIN
    /// does not move a step when it does.
    ///
    /// At the smallest window the two panels cannot both have their
    /// width — or the rail cannot show what it holds — so there are no
    /// panels: the rail's sections, the open section's pages and the
    /// page itself become one vertical list inside the one scroll, and a band of columns runs its columns one after the
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
            stop: (f32, f32),
            named: &[(FocusId, Act)],
        ) -> (Vec<Act>, Vec<Act>) {
            let mut s = furnished();
            s.view = view;
            // Every `Row::when` condition set at once, so the sweep
            // walks the conditional rows as well.
            editor_ajar(&mut s);
            // The page's offset and the rail's: two scrolls, and a sweep
            // that drove only one would call the other's far end
            // unreachable.
            s.scroll.set_offset(stop.0);
            s.rail_scroll.set_offset(stop.1);
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

        // BOTH SHAPES, AND THE FOLDED ONE IS ASKED FOR. The master keeps
        // its columns at every height the program is built for since the
        // navigation became one column, so a sweep over HEIGHTS alone
        // would be five measurements of the SAME shape and this test's
        // whole claim would be untested. The folded shape is reached the
        // way the rule is written — through `settings.col_min_w`, the
        // theme's own threshold ([`folding_theme`]) — and at the two
        // ends of the ladder, which is enough: what is under test is the
        // ORDER, and the order is the description's at every height by
        // construction.
        for folded in [false, true] {
            let _t = folded.then(folding_theme);
            let ladder: &[f32] = if folded { &[HEIGHTS[0], HEIGHTS[4]] } else { &HEIGHTS };
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
            for &h in ladder {
                theme::resolved();
                theme::set_viewport(h, 1.0);
                // Half a viewport per stop, so consecutive stops overlap
                // — every row is far shorter than half a viewport — and
                // the far end is the clamp's own, exactly as the
                // reachability sweep walks a page.
                let stops: Vec<(f32, f32)> = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    let m = Metrics::of(&ctx, content);
                    let mut s = furnished();
                    s.view = p.view;
                    editor_ajar(&mut s);
                    let stride = (s.body_box(p, m, content).h * 0.5).max(1.0);
                    let length = s.flow_h(p, m, content);
                    let mut out = vec![(0.0, 0.0)];
                    let mut at = stride;
                    while at < length {
                        out.push((at, 0.0));
                        at += stride;
                    }
                    out.push((f32::MAX / 4.0, 0.0));
                    out.extend(rail_stops(&s, m, content).into_iter().map(|r| (0.0, r)));
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
        }

        // The shapes really are different shapes, or all of the above is
        // one window measured twice over. In COLUMNS at every height the
        // program is built for; FOLDED — window and columned band alike,
        // through the one token both read — wherever the theme says the
        // page cannot have its width.
        let mut shape = |h: f32, window_folded: bool, band_folded: bool| {
            theme::resolved();
            theme::set_viewport(h, 1.0);
            let mut dl = nacelle::draw::DrawList::new();
            let ctx = probe(&mut dl, &mut fonts, h, 1.0);
            let content = content_rect(modal_rect(ctx.w, ctx.h));
            let m = Metrics::of(&ctx, content);
            let nav = Panes::of(m, content);
            assert_eq!(
                nav.folded, window_folded,
                "the window at {h}px is not the shape this test is about"
            );
            assert_eq!(
                zone_folded(&COLOR_ZONES[0], rows_box(nav.page)),
                band_folded,
                "the COLOR page's band at {h}px is not the shape this test is about"
            );
        };
        // On the master: the WINDOW keeps its columns at both ends of
        // the ladder, and the BAND inside it folds at the small end and
        // stands at the large one — so M4's two sides are both walked
        // above without a theme being asked for anything.
        shape(HEIGHTS[0], false, true);
        shape(HEIGHTS[4], false, false);
        // And the window's own fold, asked for through the threshold
        // that decides it.
        {
            let _t = folding_theme();
            shape(HEIGHTS[0], true, true);
            shape(HEIGHTS[4], true, true);
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
    /// two panels, and neither shape may hide what the other offers.
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
                //
                // TWO SCROLLS, TWO SETS OF STOPS since the rail took one
                // of its own. A sweep that only walked the page would
                // report every entry past the rail's bottom edge
                // unreachable, when what was unreachable was the sweep;
                // and the two lists are walked SEPARATELY rather than
                // crossed, because the column and the page hold disjoint
                // controls and no frame needs both offsets at once.
                let stops: Vec<(f32, f32)> = {
                    let mut dl = nacelle::draw::DrawList::new();
                    let ctx = probe(&mut dl, &mut fonts, h, 1.0);
                    let content = content_rect(modal_rect(ctx.w, ctx.h));
                    let m = Metrics::of(&ctx, content);
                    let view = reference.body_box(p, m, content);
                    let length = reference.flow_h(p, m, content);
                    let stride = (view.h * 0.5).max(1.0);
                    let mut out = vec![(0.0, 0.0)];
                    let mut at = stride;
                    while at < length {
                        out.push((at, 0.0));
                        at += stride;
                    }
                    out.push((f32::MAX / 4.0, 0.0));
                    out.extend(
                        rail_stops(&reference, m, content).into_iter().map(|r| (0.0, r)),
                    );
                    out
                };
                for (stop, rail_stop) in stops {
                    let mut s = furnished();
                    s.view = p.view;
                    // Every condition set at once, so the reachability sweep
                    // covers the conditional rows as well.
                    editor_ajar(&mut s);
                    s.editor_basic = basic;
                    if stop > 0.0 {
                        s.scroll.set_offset(stop);
                    }
                    if rail_stop > 0.0 {
                        s.rail_scroll.set_offset(rail_stop);
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
                             is not in the chain of the frame at {stop} / {rail_stop} px",
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
    /// does, and after every press whatever the chain landed on that a
    /// scroll CARRIES has to stand inside the box that scroll is read
    /// in. What the page PINS is not asked: it is outside that box by
    /// construction and always on screen.
    ///
    /// TWO SCROLLS AND TWO BOXES since 2026-08-18. A rail entry is
    /// brought back by the RAIL's offset and into the RAIL's box, and
    /// that is checked here beside the page's — a chase that moved the
    /// page to fetch a rail entry would leave the ring exactly where it
    /// was and carry the page off under it.
    ///
    /// Both shapes again: folded, the navigation is part of the flow and
    /// is chased with it, into the one box there then is.
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
        let (mut walked, mut on_the_rail) = (0, 0);
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
                    Panes::of(Metrics::of(&ctx, content), content).folded
                };
                let flowed = flowed_acts(&s, p, folded);
                // The rail's own, where there is a rail: its entries and
                // the open section's pages, chased by the rail's offset
                // into the rail's box.
                let railed: Vec<Act> = if folded { Vec::new() } else { rail_acts(&s) };
                let mut fc = FocusCtl::new();
                frame(&mut fonts, &mut s, &mut fc, h);
                // Once round the whole chain, and a few presses over.
                for _ in 0..window_acts(&s, p).len() + 8 {
                    s.key(&tab, &mut fc);
                    frame(&mut fonts, &mut s, &mut fc, h);
                    let Some(id) = fc.focused() else { continue };
                    let carried = flowed
                        .iter()
                        .position(|a| focus_id(*a) == id)
                        .map(|i| (i, flowed.len(), "the page", s.flow.view))
                        .or_else(|| {
                            let i = railed.iter().position(|a| focus_id(*a) == id)?;
                            Some((i, railed.len(), "the rail", s.rail_flow?.flow.view))
                        });
                    let Some((i, of, which, view)) = carried else { continue };
                    let r = fc.rect_of(id).expect("the chain lost what it just landed on");
                    walked += 1;
                    if which == "the rail" {
                        on_the_rail += 1;
                    }
                    assert!(
                        r.y >= view.y - 0.01 && r.bottom() <= view.bottom() + 0.01,
                        "{} at {h}px: the ring on #{i} of the {of} rows {which} \
                         carries stands {:?} outside the frame {:?} it is read in",
                        p.title,
                        (r.y, r.bottom()),
                        (view.y, view.bottom())
                    );
                }
            }
        }
        assert!(walked > 0, "the walk never landed on a row any scroll carries");
        // Fail-closed on the half that is new: a walk that never landed
        // on a rail entry would prove the page's chase and call the
        // rail's proved with it.
        assert!(
            on_the_rail > 0,
            "the walk never landed on an entry of the navigation column, so the \
             rail's own chase was never measured"
        );
        viewport_home();
    }

    /// The live acts of a run of navigation rows, in the order the
    /// column registers them — a section's own pages included, WHERE
    /// THE SECTION IS THE ONE OPEN, and at the place they stand.
    ///
    /// A disabled entry (COLOR SPACE with no colour compositor) is
    /// deliberately not one: R6 says it registers nothing at all. Nor is
    /// a shut section's page, which is the same sentence one level up
    /// and the one [`row_acts`] answers with.

    fn nav_row_acts(s: &Settings, rows: &'static [Row]) -> Vec<Act> {
        rows.iter().flat_map(|r| row_acts(s, r)).collect()
    }

    /// Everything the rail offers this window: its sections, and the
    /// pages of every section the window has unfolded.
    fn rail_acts(s: &Settings) -> Vec<Act> {
        nav_row_acts(s, &RAIL_ROWS)
    }

    // `kid_acts` stood here until 2026-08-18: the pages the section in
    // force unfolded, worked out from the VIEW so that a caller could
    // weld them onto a rail walk that had not included them. It has no
    // callers now, and the reason is the point — a section's pages are
    // in `rail_acts` whenever the window has that section open, and the
    // window's fold is a state anybody can ask about
    // ([`Settings::rail_open`]) instead of something rederived from the
    // page. A helper that answers "which pages does THIS VIEW unfold"
    // would be the coupling the owner reported, kept alive in the test
    // module.

    /// Everything the WINDOW promises on one page: the navigation, then
    /// the page's own acts. The order is the order the frame registers
    /// them in, which is what the fold has to keep.
    fn window_acts(s: &Settings, page: &'static Page) -> Vec<Act> {
        let mut out = described_acts(s, page);
        // The chrome first, then the navigation, then the rest of the
        // page: `described_acts` puts the corner button at its head.
        let rest = out.split_off(1);
        // The section's pages are IN `rail_acts`, at the place the rail
        // draws them — under their section and not after the last of
        // the sections, which is what the second column used to mean.
        out.extend(rail_acts(s));
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
            // A section, and then its pages where it is the one open —
            // the description's own reading of what the walker lays, and
            // the reason a shut section contributes NOTHING here.
            Ctrl::Expander { act, kids, .. } => {
                let mut out = vec![*act];
                if s.rail_open(*act) {
                    out.extend(kids.iter().flat_map(|k| row_acts(s, k)));
                }
                out
            }
            Ctrl::Chips { values, act, .. } => {
                values(s).iter().map(|v| act(*v)).collect()
            }
            // Every verb of an action bar, left to right.
            Ctrl::Bar { items } => items.iter().map(|&(_, a)| a).collect(),
            // The anchor alone: what the list holds is only on screen
            // while it is open, which is another test's question
            // (`an_open_list_offers_every_name_it_has`).
            Ctrl::Drop { list } => vec![Act::ListBtn(*list)],
            // The picker's parts, in the order `Settings::targets`
            // places them — this is the DESCRIPTION's copy of that list
            // and the two are checked against each other by the sweep
            // that calls both.
            Ctrl::Picker(id) => nacelle::object::color_picker::parts(
                &nacelle::object::color_picker::layout(
                    Rect::new(0.0, 0.0, 0.0, 0.0),
                    s.pickers[id.idx()].slider_count(),
                    s.picker_custom.len(),
                ),
            )
            .into_iter()
            .map(|(part, _)| picker_act(*id, part))
            .collect(),
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

