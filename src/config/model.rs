//! The typed shape of `nacelle-desktop.ron`, and the one place every
//! default it can fall back to is written down.
//!
//! The file this describes used to be `Key=Value`, read with a
//! `parse::<u32>()` and an `unwrap_or(…)` at each of forty-one call
//! sites. What that arrangement really did was scatter the DEFAULTS:
//! the number a setting takes when nobody set it lived next to the
//! reading rather than next to the setting, so the same key answered
//! differently depending on who asked, and a value the theme should
//! have owned could sit unreachable inside a parser. Deriving the
//! parser from the type moves every one of those numbers here, beside
//! the field it belongs to.
//!
//! Three things this shape has to carry, none of them optional:
//!
//! **Three states, not two.** A file can say nothing about a setting,
//! or it can say "nothing" — and those are different answers. Saying
//! nothing lets the next file down the cascade answer; saying
//! "nothing" OUTRANKS it. That is [`Choice`], and it is why clearing a
//! setting has to REMOVE the field rather than write an empty one: an
//! empty value beats the system file, so a reset that wrote empties
//! would pin the defaults off instead of letting them back in.
//!
//! **Everything defaulted.** RON parsing is all-or-nothing where
//! `Key=Value` lost one line per mistake, so a file that is merely
//! INCOMPLETE — an old version's file, a half-written one — must still
//! parse. `#[serde(default)]` on every struct here is what makes a
//! missing field ordinary; a field this program has never heard of is
//! ignored for the same reason, so a file written by a newer build
//! still opens.
//!
//! **Nothing about the LOOK.** A default here is a default for a
//! setting, never for an appearance: the band around a panel, for one,
//! has no number in this file at all — an unset [`GridConf::padding`]
//! is answered by the theme's `layout.panel_gutter`, which is the
//! whole reason it is `Option` and not a `u32` with an eight in it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Whether a field is worth writing down: a field nothing was said
/// about is left out of the file entirely, which is what makes a
/// cleared setting indistinguishable from one that was never set.
fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// What one file says about one setting that NAMES something — a
/// theme, a layaut, a font family, a colour space.
///
/// The middle variant is the one a two-state type could not express.
/// `Off` is a user saying "none", and it has to beat a system file
/// that names one — a LUT switched off in the settings window may not
/// come back because `/etc/xdg` mentions one. Absence is the opposite
/// answer: it hands the question to the next file down.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    /// Not written down. The rest of the cascade answers, and when
    /// nothing does, the program's own default stands.
    ///
    /// This is what an ABSENT field parses as, so it is never written
    /// out: a field holding it is left out of the file.
    #[default]
    Inherit,
    /// Written down as "nothing" — an explicit off that outranks
    /// whatever a system file names.
    Off,
    /// The name the setting takes.
    Named(String),
}

impl Choice {
    /// The name settled on, if one was. `Off` and `Inherit` both
    /// answer `None` — they differ in the CASCADE, not in the value.
    pub fn name(&self) -> Option<&str> {
        match self {
            Choice::Named(n) => Some(n.as_str()),
            _ => None,
        }
    }

    /// A name the user picked. An empty one is not a name: it means
    /// nothing was chosen, so the question goes back to the cascade.
    pub fn named(name: &str) -> Choice {
        let n = name.trim();
        if n.is_empty() {
            Choice::Inherit
        } else {
            Choice::Named(n.to_string())
        }
    }

    /// A control that offers "none" as one of its answers: nothing
    /// chosen is an explicit off, not a question passed on.
    pub fn or_off(name: Option<&str>) -> Choice {
        match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => Choice::Named(n.to_string()),
            None => Choice::Off,
        }
    }

    /// The old format's reading of a value for a key whose control
    /// offered NO "none" — a theme, a layaut, a sound set, a font
    /// family. Empty is an absence there, and reading it as an off is
    /// what broke the reset; see [`DesktopConf::from_legacy`].
    fn from_legacy(value: &str) -> Choice {
        Choice::named(value)
    }

    /// The same, for a key whose control DID offer one: the contrast
    /// variant, the grading LUT, the ICC profile. Empty was how the
    /// settings window wrote that answer, and it has to keep beating a
    /// system file that names something.
    fn from_legacy_offable(value: &str) -> Choice {
        Choice::or_off(Some(value))
    }
}

/// A document — or a part of one — laid over the same thing read from
/// a less specific file.
///
/// The cascade is per FIELD and always was: the user's file answering
/// `Theme` does not stop the system file answering `Sounds`. With one
/// key per line that fell out of merging two maps; with one document
/// per file it has to be spelled out, and this is where.
pub trait Layered {
    /// `self` is the more specific file. Everything it does not carry
    /// comes from `base`.
    fn over(self, base: Self) -> Self;
}

impl Layered for Choice {
    fn over(self, base: Self) -> Self {
        match self {
            Choice::Inherit => base,
            settled => settled,
        }
    }
}

impl<T> Layered for Option<T> {
    fn over(self, base: Self) -> Self {
        self.or(base)
    }
}

impl Layered for BTreeMap<String, Choice> {
    /// Key by key, exactly like the document itself: a screen the
    /// user's file says nothing about keeps the layaut the system file
    /// assigned it.
    fn over(self, mut base: Self) -> Self {
        base.extend(self);
        base
    }
}

/// Everything `nacelle-desktop.ron` can say.
///
/// The FOLDER is the family and the FILE is the program, so this type
/// is the whole of one program's configuration: `nacelle-ai.ron` will
/// have a type of its own beside it rather than sharing keys with this.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopConf {
    /// The theme the engine loads. Absent = the built-in master.
    #[serde(skip_serializing_if = "is_default")]
    pub theme: Choice,
    /// The contrast variant on top of it — `hc` is the one the
    /// master ships. Its own key rather than part of the theme's,
    /// because liking a colour is not a reason to give up contrast.
    #[serde(skip_serializing_if = "is_default")]
    pub variant: Choice,
    /// The DEFAULT desktop arrangement: every screen not named in
    /// [`screens`](Self::screens) shows this one.
    #[serde(skip_serializing_if = "is_default")]
    pub layaut: Choice,
    /// One screen, one desktop: screen key → layaut.
    ///
    /// A map rather than the old `Layaut[DP-1]=` — a bracket
    /// convention invented because the format had nowhere to put a
    /// second dimension.
    ///
    /// The key is what the MONITOR says it is — `edid:DEL-41B2-
    /// 0123ABCD` — and a connector name (`DP-1`, `eDP-1`) is the second
    /// vocabulary, for a monitor that says nothing and for files
    /// written before 2026-08-18. Both are read, the monitor's own name
    /// first; [`crate::screens::screen_key`] is the one statement of
    /// what either looks like.
    ///
    /// Neither the ORDER screens come up in nor the position of one in
    /// this program's list is a name: which monitor is switched on
    /// first is not a property of anything.
    #[serde(skip_serializing_if = "is_default")]
    pub screens: BTreeMap<String, Choice>,
    /// Which screen carries the MAIN SCREEN role — a screen key, the
    /// same vocabulary [`screens`](Self::screens) is keyed by.
    ///
    /// What the role MEANS is written down once, in
    /// [`crate::screens::MainScreenDuty`], and nowhere else.
    ///
    /// The three states are all three needed here. Absent hands the
    /// question to the next file down and finally to the display
    /// server, which is what answered it alone until this field
    /// existed. [`Choice::Off`] is a user saying "the display server's
    /// answer, whatever it is" — an explicit choice that has to beat a
    /// system file naming a screen, on a machine whose administrator
    /// named one that is not on this desk.
    #[serde(skip_serializing_if = "is_default")]
    pub main_screen: Choice,
    /// The sound set: a directory name under `sounds/`.
    #[serde(skip_serializing_if = "is_default")]
    pub sounds: Choice,
    /// The terminal's own font.
    #[serde(skip_serializing_if = "is_default")]
    pub term_font: FontConf,
    /// The interface font — every panel that is not the terminal.
    #[serde(skip_serializing_if = "is_default")]
    pub ui_font: FontConf,
    #[serde(skip_serializing_if = "is_default")]
    pub sound: SoundConf,
    #[serde(skip_serializing_if = "is_default")]
    pub grid: GridConf,
    #[serde(skip_serializing_if = "is_default")]
    pub blur: BlurConf,
}

impl Layered for DesktopConf {
    fn over(self, base: Self) -> Self {
        DesktopConf {
            theme: self.theme.over(base.theme),
            variant: self.variant.over(base.variant),
            layaut: self.layaut.over(base.layaut),
            screens: self.screens.over(base.screens),
            main_screen: self.main_screen.over(base.main_screen),
            sounds: self.sounds.over(base.sounds),
            term_font: self.term_font.over(base.term_font),
            ui_font: self.ui_font.over(base.ui_font),
            sound: self.sound.over(base.sound),
            grid: self.grid.over(base.grid),
            blur: self.blur.over(base.blur),
        }
    }
}

impl DesktopConf {
    /// The screen → layaut assignments worth acting on: a key that
    /// names no screen is dropped here and nowhere else.
    ///
    /// `Dell Inc. U2720Q` is a make and a model, not a screen key, and
    /// a key nothing could ever be matched to is worth neither writing
    /// nor reading. Which layaut a screen then takes — and whether
    /// the one named is installed at all — is a separate judgement,
    /// made where it can be reported.
    pub fn screens(&self) -> BTreeMap<String, String> {
        self.screens
            .iter()
            .filter(|(k, _)| crate::screens::screen_key(k).is_some())
            .filter_map(|(k, v)| v.name().map(|n| (k.clone(), n.to_string())))
            .collect()
    }

    /// Rewrites every setting written against a SOCKET into one written
    /// against the MONITOR now plugged into it.
    ///
    /// Answers whether anything moved, so that a machine with nothing
    /// to migrate is a machine whose configuration file is not touched
    /// — this runs at every start, and a program that has installed
    /// nothing must go on having installed nothing.
    ///
    /// WITHOUT THIS, EVERY PER-SCREEN ASSIGNMENT WRITTEN BEFORE
    /// 2026-08-18 WOULD BE ORPHANED the moment the identity took over
    /// as the key a screen is looked up by. It moves rather than
    /// copies, so the same file migrated twice is the same file.
    ///
    /// IT RUNS ON A DOCUMENT THAT HAS NEVER NAMED A MONITOR, AND ON NO
    /// OTHER — and that is what makes it an act rather than a policy.
    /// A document naming a monitor anywhere, in the screen map or in
    /// the role, was written after monitors could be named: its socket
    /// keys are somebody's rule about a SOCKET — "whatever hangs off
    /// DP-1" — and rewriting those would be an opinion about a decision
    /// that has already been made. Migrating makes the document name a
    /// monitor, so it happens once per file, announces itself in the
    /// log, and a rule written afterwards stands for good. Without that
    /// bound the user could rewrite the rule and the program would
    /// rewrite it back at every start, which is not a migration but an
    /// argument.
    ///
    /// Three more things it will not do, each for a reason:
    ///
    /// It never touches a key whose screen it cannot see. A monitor
    /// that is off today is not a monitor whose settings are stale, and
    /// a socket nothing is plugged into is a socket somebody may plug
    /// something into tomorrow.
    ///
    /// IT NEVER WRITES A NAME THAT TWO OF TODAY'S SCREENS ANSWER TO.
    /// Two units of one model can be one identity — the AORUS FO32U2P
    /// prints the same serial in every unit — and the identity is
    /// looked up before the socket, so a single such line would answer
    /// for BOTH screens and the second one would silently take the
    /// first one's desktop. `screens::shared_identities` is the
    /// question; the answer is to leave both socket entries exactly
    /// where they are, which is the one vocabulary that still tells
    /// those two apart.
    ///
    /// It never removes anything whose value it has not carried
    /// somewhere else. Every path here either moves an entry whole or
    /// leaves it alone.
    pub fn migrate_screens(&mut self, live: &[crate::screens::ScreenId]) -> bool {
        if self.names_a_monitor() {
            return false;
        }
        // A name two screens answer to is a name neither of them can be
        // given a setting under.
        let shared = crate::screens::shared_identities(live);
        let ambiguous = |edid: &str| shared.iter().any(|s| s.eq_ignore_ascii_case(edid));
        let mut moved = false;
        for id in live {
            let (Some(edid), Some(connector)) = (&id.edid, &id.connector) else { continue };
            if ambiguous(edid) {
                continue;
            }
            // Nothing here can overwrite: the document named no monitor
            // when this began, and no two screens of this survey are
            // being written under one name.
            let new_key = format!("{}{edid}", crate::screens::EDID_PREFIX);
            let Some(old_key) = self
                .screens
                .keys()
                .find(|k| k.eq_ignore_ascii_case(connector))
                .cloned()
            else {
                continue;
            };
            let Some(value) = self.screens.remove(&old_key) else { continue };
            self.screens.insert(new_key, value);
            moved = true;
        }
        // The role travels the same road, and for the same reason: a
        // main screen named by its socket stops being that screen the
        // moment somebody moves a cable. It stays on the socket when
        // the monitor there shares its name with another, because "the
        // main screen" cannot be two screens.
        if let Choice::Named(key) = &self.main_screen {
            if let Some(edid) = live
                .iter()
                .find(|id| {
                    id.connector
                        .as_deref()
                        .map(|c| c.eq_ignore_ascii_case(key.trim()))
                        .unwrap_or(false)
                })
                .and_then(|id| id.edid.as_deref())
                .filter(|edid| !ambiguous(edid))
            {
                self.main_screen =
                    Choice::Named(format!("{}{edid}", crate::screens::EDID_PREFIX));
                moved = true;
            }
        }
        moved
    }

    /// Whether anything in this document names a MONITOR — a key in the
    /// screen map, or the screen carrying the role.
    ///
    /// Which is the same question as WHEN this document was written:
    /// the vocabulary did not exist before 2026-08-18, so a document
    /// using it is one this program has already migrated or one
    /// somebody wrote knowing both halves of the format.
    fn names_a_monitor(&self) -> bool {
        self.screens
            .keys()
            .map(String::as_str)
            .chain(self.main_screen.name())
            .any(crate::screens::names_a_monitor)
    }

    /// The same document as the old `Key=Value` file said it.
    ///
    /// An empty value is read the way the SETTER of that same key
    /// writes one today, and the split is not a nicety — it is what
    /// makes LOOK AND FEEL RESET work on a machine with any history.
    ///
    /// Releases up to `be64867` installed
    /// `~/.config/nacelle-desktop/nacelle-desktop.conf` with every key
    /// present and blank — `Theme=`, `Layaut=`, `Sounds=`,
    /// `TermFontFamily=` — under a comment of its own saying "Empty
    /// values or missing options = defaults built into the program".
    /// That directory stands AHEAD of `/etc/xdg` in the cascade and is
    /// deliberately never rewritten, so reading those blanks as
    /// explicit offs pinned every system default off permanently: the
    /// reset removed the user's fields and the abandoned template went
    /// on answering "nothing" in their place, with no way back and
    /// nothing said.
    ///
    /// So: `Choice::named` where the control has no "none" to offer —
    /// a theme, a layaut, a sound set, a font family or weight, a
    /// colour space, all of which the settings window can only write a
    /// name into — and `Choice::or_off` for exactly the four the
    /// window writes empty on purpose: the contrast variant, a
    /// per-screen assignment, the grading LUT and the ICC profile.
    /// Each one mirrors its own setter in `config.rs`, which is the
    /// only reading under which a file written by this program and a
    /// file read by it mean the same thing.
    ///
    /// The keys that are SWITCHES and NUMBERS rather than names go the
    /// same way for the same reason: `SoundVolume=` never yielded a
    /// volume, and an empty `SoundTyping=` or `GridSnap=` took the
    /// built-in default just as an absent one did, so neither is an
    /// answer to carry forward. The settings window wrote `0` or `1`
    /// and nothing else; a blank is a line typed by hand and left, and
    /// the one thing it must not become is a switch that outranks the
    /// machine's own file.
    pub fn from_legacy(kv: &std::collections::HashMap<String, String>) -> DesktopConf {
        let text = |key: &str| kv.get(key).map(|v| Choice::from_legacy(v)).unwrap_or_default();
        let offable =
            |key: &str| kv.get(key).map(|v| Choice::from_legacy_offable(v)).unwrap_or_default();
        let num = |key: &str| kv.get(key).and_then(|v| v.trim().parse::<u32>().ok());
        // A blank is not an answer, on a switch any more than on a
        // number: `SoundVolume=` never yielded a volume, and the same
        // line has to mean the same nothing when the key beside it is a
        // switch. The old reader could not tell an empty `SoundTyping=`
        // from a missing one — both took the built-in default — so a
        // blank here is a line somebody typed and left, and turning it
        // into a value would let it beat a system file for a switch
        // nobody flipped. That is the blank template's failure exactly,
        // one rung down.
        let said = |key: &str| kv.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
        // `!= "0"` and not a parse: that is what the old readers did,
        // so `SoundTyping=yes` went on meaning yes.
        let flag = |key: &str| said(key).map(|v| v != "0");
        DesktopConf {
            theme: text("Theme"),
            variant: offable("Variant"),
            layaut: text("Layaut"),
            screens: legacy_screen_choices(kv),
            // The old format had no key for the MAIN SCREEN role and
            // could not have had one: which screen carried it was the
            // display server's answer alone, with nowhere in the file to
            // disagree. An absence here is therefore the truth about
            // every old file and not a key left unread.
            main_screen: Choice::Inherit,
            sounds: text("Sounds"),
            term_font: FontConf {
                size: kv.get("TermFontSize").and_then(|v| v.trim().parse::<f32>().ok()),
                family: text("TermFontFamily"),
                weight: text("TermFontWeight"),
            },
            ui_font: FontConf {
                size: kv.get("UIFontSize").and_then(|v| v.trim().parse::<f32>().ok()),
                family: text("UIFontFamily"),
                weight: text("UIFontWeight"),
            },
            sound: SoundConf {
                volume: num("SoundVolume"),
                typing: flag("SoundTyping"),
                ambient: flag("SoundAmbient"),
            },
            grid: GridConf {
                // Snap was the one flag written as a word as well as a
                // digit, and only "1"/"true" ever turned it on.
                snap: said("GridSnap").map(|v| v == "1" || v.eq_ignore_ascii_case("true")),
                cols: num("GridCols"),
                rows: num("GridRows"),
                padding: num("GridPadding"),
            },
            blur: BlurConf { radius: num("BlurRadius"), opacity: num("BlurOpacity") },
        }
    }
}

/// The `Layaut[<connector>]=` family of the old format, as choices.
///
/// `Layaut [DP-1]` reads as `Layaut[DP-1]`: it was a file people typed
/// into, and a space before the bracket was not a different intention.
/// Whether what stands in the brackets names a screen is
/// [`DesktopConf::screens`]'s judgement — this only has to recognise
/// the syntax it is replacing.
///
/// Empty is an OFF here, mirroring `set_layaut_for_screen`: a screen
/// switched off is an answer the settings window can write, and the
/// abandoned template never carried a key of this family at all.
fn legacy_screen_choices(
    kv: &std::collections::HashMap<String, String>,
) -> BTreeMap<String, Choice> {
    let mut out = BTreeMap::new();
    for (key, value) in kv {
        let Some(inner) = key
            .trim()
            .strip_prefix("Layaut")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('['))
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            continue;
        };
        out.insert(inner.trim().to_string(), Choice::from_legacy_offable(value));
    }
    out
}

/// One font section — the terminal's or the interface's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConf {
    /// Size as a PERCENTAGE of what the theme asks for, so an unset
    /// size is not a number of pixels this program knows: it is the
    /// theme's own size, untouched.
    #[serde(skip_serializing_if = "is_default")]
    pub size: Option<f32>,
    /// A family installed on the machine. Absent = the theme's.
    #[serde(skip_serializing_if = "is_default")]
    pub family: Choice,
    /// `regular`, `bold`, … Absent = the theme's.
    #[serde(skip_serializing_if = "is_default")]
    pub weight: Choice,
}

impl Layered for FontConf {
    fn over(self, base: Self) -> Self {
        FontConf {
            size: self.size.over(base.size),
            family: self.family.over(base.family),
            weight: self.weight.over(base.weight),
        }
    }
}

impl FontConf {
    /// A hundred percent: the theme's size as the theme wrote it.
    pub const SIZE: f32 = 100.0;

    /// The factor the theme's size is multiplied by, brought into the
    /// range this section allows — the two sections allow different
    /// ranges, which is why the caller names them.
    pub fn scale(&self, min: f32, max: f32) -> f32 {
        (self.size.unwrap_or(Self::SIZE) / 100.0).clamp(min, max)
    }

}

/// The sounds the interface makes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundConf {
    /// Master volume, percent.
    #[serde(skip_serializing_if = "is_default")]
    pub volume: Option<u32>,
    /// The key clicks the terminal makes.
    #[serde(skip_serializing_if = "is_default")]
    pub typing: Option<bool>,
    /// The ambient bed under everything.
    #[serde(skip_serializing_if = "is_default")]
    pub ambient: Option<bool>,
}

impl Layered for SoundConf {
    fn over(self, base: Self) -> Self {
        SoundConf {
            volume: self.volume.over(base.volume),
            typing: self.typing.over(base.typing),
            ambient: self.ambient.over(base.ambient),
        }
    }
}

impl SoundConf {
    /// Everything on and loud: a fresh install should be heard.
    pub const VOLUME: u32 = 100;
    pub const ON: bool = true;

    pub fn volume(&self) -> u32 {
        self.volume.unwrap_or(Self::VOLUME).min(Self::VOLUME)
    }
    pub fn typing(&self) -> bool {
        self.typing.unwrap_or(Self::ON)
    }
    pub fn ambient(&self) -> bool {
        self.ambient.unwrap_or(Self::ON)
    }
}

/// How coarse or fine the grid editor's snap grid may be made.
///
/// Read where the file is read rather than in the settings window,
/// because a value already in the file has to be brought into range
/// too — a grid saved before these were the limits is still a number
/// this program has to draw.
pub const GRID_MIN: u32 = 15;
pub const GRID_MAX: u32 = 100;
/// How wide a gutter the settings window will let anyone type.
pub const GRID_PAD_MAX: u32 = 40;

/// The grid editor's own settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridConf {
    /// Snapping is opt-in.
    #[serde(skip_serializing_if = "is_default")]
    pub snap: Option<bool>,
    #[serde(skip_serializing_if = "is_default")]
    pub cols: Option<u32>,
    #[serde(skip_serializing_if = "is_default")]
    pub rows: Option<u32>,
    /// The band kept clear around every panel, in device pixels.
    ///
    /// There is NO default here and there must not be one. The band
    /// is a length like every other, so the theme owns it —
    /// `layout.panel_gutter` — and this field is the user's override
    /// of that one token. A number standing here as a fallback would
    /// be a piece of the look that no theme could reach, which is
    /// exactly what it used to be.
    #[serde(skip_serializing_if = "is_default")]
    pub padding: Option<u32>,
}

impl Layered for GridConf {
    fn over(self, base: Self) -> Self {
        GridConf {
            snap: self.snap.over(base.snap),
            cols: self.cols.over(base.cols),
            rows: self.rows.over(base.rows),
            padding: self.padding.over(base.padding),
        }
    }
}

impl GridConf {
    /// Snapping off, and the coarsest grid: the state the editor opens
    /// in when nobody has said otherwise.
    pub const SNAP: bool = false;

    pub fn snap(&self) -> bool {
        self.snap.unwrap_or(Self::SNAP)
    }
    pub fn cols(&self) -> u32 {
        Self::cells(self.cols)
    }
    pub fn rows(&self) -> u32 {
        Self::cells(self.rows)
    }
    fn cells(n: Option<u32>) -> u32 {
        n.unwrap_or(GRID_MIN).clamp(GRID_MIN, GRID_MAX)
    }

    /// The user's override of the theme's gutter, bounded. `None` —
    /// the ordinary case — is "the theme's own", and the bound is on
    /// the typed number alone: a length the theme wrote is not this
    /// program's to cap.
    pub fn padding(&self) -> Option<u32> {
        self.padding.map(|n| n.min(GRID_PAD_MAX))
    }
}

/// The frosted glass every panel is drawn on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlurConf {
    /// How deep the renderer's pyramid goes, in percent of the full
    /// blur the theme's glass asks for.
    #[serde(skip_serializing_if = "is_default")]
    pub radius: Option<u32>,
    /// The glass tint's alpha, percent: below a hundred the sharp
    /// boards beneath begin to show through the frost.
    #[serde(skip_serializing_if = "is_default")]
    pub opacity: Option<u32>,
}

impl Layered for BlurConf {
    fn over(self, base: Self) -> Self {
        BlurConf {
            radius: self.radius.over(base.radius),
            opacity: self.opacity.over(base.opacity),
        }
    }
}

impl BlurConf {
    /// The glass as the theme drew it — this pair scales the theme's
    /// own figures, so a hundred percent is "do not interfere".
    pub const FULL: u32 = 100;

    pub fn radius(&self) -> u32 {
        self.radius.unwrap_or(Self::FULL).min(Self::FULL)
    }
    pub fn opacity(&self) -> u32 {
        self.opacity.unwrap_or(Self::FULL).min(Self::FULL)
    }
}
