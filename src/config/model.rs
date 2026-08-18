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
    /// One screen, one desktop: connector name → layaut.
    ///
    /// A map rather than the old `Layaut[DP-1]=` — a bracket
    /// convention invented because the format had nowhere to put a
    /// second dimension. The connector (DP-1, eDP-1, HDMI-A-1) is the
    /// only stable name a screen has; the order screens come up in
    /// depends on which monitor was switched on first.
    #[serde(skip_serializing_if = "is_default")]
    pub screens: BTreeMap<String, Choice>,
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
    pub color: ColorConf,
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
            sounds: self.sounds.over(base.sounds),
            term_font: self.term_font.over(base.term_font),
            ui_font: self.ui_font.over(base.ui_font),
            sound: self.sound.over(base.sound),
            grid: self.grid.over(base.grid),
            color: self.color.over(base.color),
            blur: self.blur.over(base.blur),
        }
    }
}

impl DesktopConf {
    /// The connector → layaut assignments worth acting on: a key that
    /// names no screen is dropped here and nowhere else.
    ///
    /// `Dell Inc. U2720Q` is a make and a model, not a socket, and a
    /// key nothing could ever be matched to is worth neither writing
    /// nor reading. Which layaut a screen then takes — and whether
    /// the one named is installed at all — is a separate judgement,
    /// made where it can be reported.
    pub fn screens(&self) -> BTreeMap<String, String> {
        self.screens
            .iter()
            .filter(|(k, _)| crate::screens::connector_of(k).as_deref() == Some(k.as_str()))
            .filter_map(|(k, v)| v.name().map(|n| (k.clone(), n.to_string())))
            .collect()
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
            color: ColorConf {
                depth: num("ColorDepth"),
                space: text("ColorSpace"),
                lut: offable("ColorLut"),
                icc: offable("ColorIcc"),
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
/// Empty is an OFF here, mirroring `set_layaut_for_connector`: a screen
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

/// The dynamic range a colour space asks the display for.
///
/// The COLOR view offers ONE list of spaces and the HDR switch decides
/// which half of the table it holds, so the half a space belongs to is
/// part of the space's own record and not a second table written out
/// beside it: a space added to [`COLOR_SPACE_TABLE`] cannot go missing
/// from an offer, because the row does not compile without saying which
/// offer it stands in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpaceRange {
    /// A standard-range space: the offer the switch shows when it is off.
    Sdr,
    /// A high-range space: the offer the switch shows when it is on.
    Hdr,
    /// Neither, and therefore both. Only "auto", which names no space at
    /// all — it hands the choice back to the compositor — so it is never
    /// the wrong one to be looking at and stands in both offers.
    Either,
}

impl SpaceRange {
    /// Whether a space of this range stands in the offer the switch is
    /// showing.
    pub const fn in_offer(self, hdr: bool) -> bool {
        match self {
            SpaceRange::Either => true,
            SpaceRange::Sdr => !hdr,
            SpaceRange::Hdr => hdr,
        }
    }
}

/// The colour spaces the COLOR view offers, in display order, each with
/// the dynamic range it asks for. Names map to the Color Management
/// protocol's named primaries + transfer function pairs in the
/// application.
///
/// THE one statement of the split. Both offers are read off this table
/// ([`color_spaces`]) and so is the switch's own state
/// ([`space_range`]); nothing anywhere lists three names of its own.
pub const COLOR_SPACE_TABLE: [(&str, SpaceRange); 7] = [
    ("auto", SpaceRange::Either),
    ("srgb", SpaceRange::Sdr),
    ("display p3", SpaceRange::Sdr),
    ("adobe rgb", SpaceRange::Sdr),
    // ST 2084 is the display's own curve; HLG is a broadcast one and
    // scRGB linear is a compositing space — all three are high range,
    // and which of them a switch should reach for is the settings
    // window's business, not this table's.
    ("bt2020 pq", SpaceRange::Hdr),
    ("bt2020 hlg", SpaceRange::Hdr),
    ("scrgb linear", SpaceRange::Hdr),
];

/// Every name the table holds, in the same order — what a written value
/// is validated against, which is a question about the whole vocabulary
/// and not about either offer. DERIVED, so it cannot fall behind.
pub const COLOR_SPACES: [&str; COLOR_SPACE_TABLE.len()] = {
    let mut out = [""; COLOR_SPACE_TABLE.len()];
    let mut i = 0;
    while i < COLOR_SPACE_TABLE.len() {
        out[i] = COLOR_SPACE_TABLE[i].0;
        i += 1;
    }
    out
};

/// The range a name asks for. A name from outside the table asks for
/// nothing this program knows — and cannot reach here from the
/// configuration, because [`ColorConf::space`] has already turned such a
/// name into "auto".
pub fn space_range(name: &str) -> SpaceRange {
    COLOR_SPACE_TABLE
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, r)| r)
        .unwrap_or(SpaceRange::Either)
}

/// The names ONE offer holds, in the table's display order.
pub fn color_spaces(hdr: bool) -> Vec<&'static str> {
    COLOR_SPACE_TABLE
        .iter()
        .filter(|(_, r)| r.in_offer(hdr))
        .map(|&(n, _)| n)
        .collect()
}

/// Every bit depth the swapchain may be asked for, ASCENDING — which is
/// what makes an offer a slice of this and not a second list.
///
/// Twelve rides in sixteen-bit float buffers (Vulkan has no twelve-bit
/// swapchain format) and what the wire carries is between the compositor
/// and the display; the numbers here are what the program asks for.
pub const COLOR_DEPTHS: [u32; 4] = [8, 10, 12, 16];

impl SpaceRange {
    /// The fewest bits a picture of this range can be given in.
    ///
    /// ST 2084 spends its code points over a range eight bits has no
    /// steps for, so eight-bit PQ bands visibly — and neither the
    /// settings window nor the swapchain has anywhere to say so. THE
    /// ONE STATEMENT of that floor: the COLOR page takes its depth offer
    /// from it ([`color_depths`]) and so does the reading of the
    /// configuration ([`ColorConf::depth`]), which is why a file cannot
    /// hand the two of them different answers.
    ///
    /// "auto" ([`SpaceRange::Either`]) names no space at all and asks
    /// the compositor for nothing in particular, so it carries the
    /// standard floor: which range a window showing "auto" stands on is
    /// the switch's business, not the name's.
    pub const fn depth_floor(self) -> u32 {
        match self {
            SpaceRange::Hdr => HDR_DEPTH_FLOOR,
            _ => COLOR_DEPTHS[0],
        }
    }
}

/// Ten bits, and the reason is in [`SpaceRange::depth_floor`].
const HDR_DEPTH_FLOOR: u32 = 10;

/// The depths ONE offer holds. A floor cuts a prefix off an ascending
/// table, so an offer is a slice of [`COLOR_DEPTHS`] rather than a
/// second list that could fall behind it.
pub fn color_depths(hdr: bool) -> &'static [u32] {
    let floor = if hdr { SpaceRange::Hdr } else { SpaceRange::Sdr }.depth_floor();
    let cut = COLOR_DEPTHS
        .iter()
        .position(|&d| d >= floor)
        .unwrap_or(COLOR_DEPTHS.len());
    &COLOR_DEPTHS[cut..]
}

/// The colour pipeline — and it has TWO addressees, not one.
///
/// [`ColorConf::depth`] and [`ColorConf::lut`] are the RENDERER's: a
/// swapchain format and a 3D texture, neither of which any compositor is
/// asked about, so both apply in EVERY session — under gamescope, under
/// X11, under a Wayland compositor that has never heard of colour
/// management. [`ColorConf::space`] and [`ColorConf::icc`] are the
/// compositor's, over the Color Management protocol, and exist only in a
/// native Wayland session that announces it (`wl_color.rs`).
///
/// This whole struct used to be described as the second kind, and the
/// application matched the description: `apply_color!` sat inside `if
/// let Some(mgr)`, so a missing Wayland global threw away the depth and
/// the LUT along with the space. A `depth: 10` was read, parsed and
/// validated by [`ColorConf::depth`] — and then reached no swapchain,
/// with nothing anywhere saying so. Split apart 2026-08-18; the two
/// halves are applied separately in `main.rs`.
///
/// The visible consequence, worth knowing before changing it back: a
/// file that says `depth: 16` now rebuilds a swapchain in sessions where
/// it used to be silently ignored.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConf {
    /// 8, 10, 12 or 16 bits.
    #[serde(skip_serializing_if = "is_default")]
    pub depth: Option<u32>,
    /// A name from [`COLOR_SPACES`].
    #[serde(skip_serializing_if = "is_default")]
    pub space: Choice,
    /// A grading LUT: a file name under an assets `lut/` directory.
    #[serde(skip_serializing_if = "is_default")]
    pub lut: Choice,
    /// An ICC profile, likewise under `icc/`.
    #[serde(skip_serializing_if = "is_default")]
    pub icc: Choice,
}

impl Layered for ColorConf {
    fn over(self, base: Self) -> Self {
        ColorConf {
            depth: self.depth.over(base.depth),
            space: self.space.over(base.space),
            lut: self.lut.over(base.lut),
            icc: self.icc.over(base.icc),
        }
    }
}

impl ColorConf {
    /// What every machine can show, so it is what the program asks for
    /// until somebody asks for more.
    ///
    /// TAKEN FROM THE TOOLKIT, not written out again: the theme editor's
    /// BASIC sliders notch by this depth, and libnacelle states it as the
    /// depth to assume when nobody has said. Two copies of an eight would
    /// let the sliders and the swapchain disagree about what "nobody has
    /// said" means.
    pub const DEPTH: u32 = nacelle::theme::edit::DEFAULT_DEPTH_BITS;
    /// Leave the compositor's own choice in place.
    pub const SPACE: &'static str = "auto";

    /// The depth to ask the swapchain for — READ AGAINST THE SPACE
    /// BESIDE IT, because the two are one statement and the file writes
    /// them as two lines.
    ///
    /// A depth this program cannot ask for is not a reason to fail to
    /// start, so an unknown number falls back to [`ColorConf::DEPTH`].
    /// And a legal number can still be the wrong one FOR THE SPACE THIS
    /// FILE NAMES: `depth: 8` with `space: "bt2020 pq"` passes any test
    /// either field can make alone, and asks for a picture that bands
    /// (`SpaceRange::depth_floor`). This is the place that can rule on
    /// the pair — both fields are here — and it is the reason there is
    /// no `hdr` field for the file to contradict the space with: the
    /// range is READ OFF the space, and the depth is raised to what that
    /// range needs. The settings window and the swapchain read through
    /// this one method, so neither can be handed the banded picture.
    ///
    /// It raises and never lowers: a depth is a floor to meet, and what
    /// the user wrote above it is theirs.
    pub fn depth(&self) -> u32 {
        let bits = self.depth.filter(|d| COLOR_DEPTHS.contains(d)).unwrap_or(Self::DEPTH);
        bits.max(space_range(&self.space()).depth_floor())
    }

    /// A name the application can actually turn into primaries and a
    /// transfer function; anything else is the compositor's default.
    pub fn space(&self) -> String {
        self.space
            .name()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| COLOR_SPACES.contains(&s.as_str()))
            .unwrap_or_else(|| Self::SPACE.to_string())
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
