//! The compositor's own settings: the rows the COMPOSITOR tab shows,
//! the file they are kept in, and how a change reaches a running
//! Hyprland.
//!
//! # Why the file is the storage
//!
//! Every other setting in this window is a field of `Settings` written
//! into `nacelle-desktop.ron`. These are not, and the reason is that
//! they have a second reader: `nacelle-session` writes Hyprland's own
//! config at session start and ends it by loading THIS file, so
//! whatever is here is what the compositor comes up with. Keeping the
//! values in a RON file as well would mean two places to disagree, and
//! a settings window that shows one thing while the compositor does
//! another. So the emitted Lua is the storage: [`read`] parses back the
//! handful of lines [`write`] produces, and there is exactly one truth.
//!
//! The parse is deliberately narrow. It reads only what this module
//! itself wrote — `key = value` at one indent inside a table this
//! module opened — and a file a person has edited by hand past that
//! shape simply reads as defaults rather than being rewritten around
//! their work. It is not, and does not try to be, a Lua parser.
//!
//! # Applying without a restart
//!
//! Hyprland reloads its config when the file changes
//! (`misc.disable_autoreload` is false by default), so writing is
//! enough to make a change stick. Writing alone is not enough to make
//! it VISIBLE at once for every option, so [`apply_live`] also pokes
//! the running compositor over `hyprctl`. Both, not either: the poke
//! is instant and forgotten on the next reload, the file is permanent
//! and takes a moment.

use std::ffi::OsStr;
use std::path::PathBuf;
// `Path` is wanted only by the test-only helper further down. Imported
// unconditionally it warns on every ordinary build; deleted to silence
// that warning it breaks the test target — which is exactly how this
// line was lost once, taking the whole binary's tests with it while the
// build stayed green. The cfg keeps both from happening.
#[cfg(test)]
use std::path::Path;

/// What kind of control a row is, and what the value means in Lua.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `true` / `false`.
    Flag,
    /// A plain integer.
    Int,
    /// Stored as a percentage 0..100 here, written as 0.0..1.0 — the
    /// same shape this window's own OPACITY slider already uses, so a
    /// reader meets one convention and not two.
    Percent,
    /// One of a fixed few named states, stored and WRITTEN as its
    /// position in the list.
    ///
    /// Hyprland types these `Int` with an `OptionMap` beside them —
    /// `{.min = 0, .max = 2, .map = {{"disable", 0}, {"enable", 1},
    /// {"auto", 2}}}` — so the compositor takes the number and the word
    /// alike, and the number is what this module writes. A slider would
    /// be the wrong control for them twice over: the numbers are not a
    /// quantity, and nothing on a track says what 2 means.
    ///
    /// THE WORDS ARE HYPRLAND'S OWN, in the source's numeric order, and
    /// they are here for the same reason [`Opt::key`] is spelled the
    /// compositor's way rather than ours: they are the interface's
    /// names for its own states, not prose about them. Only the LABEL
    /// beside them is nacelle's wording.
    Choice(&'static [&'static str]),
    /// One of a fixed few named states, stored as its position in the
    /// list but WRITTEN as the word itself, quoted.
    ///
    /// The distinction from [`Kind::Choice`] is the compositor's, not a
    /// preference: these are `MS<String>` options, and a number arriving
    /// where a string is read is not the same value.
    Word(&'static [&'static str]),
}

/// One row of the COMPOSITOR tab.
///
/// `key` is Hyprland's own dotted name — `decoration.blur.enabled` —
/// which is the Lua path the file is written with AND the name
/// `hyprctl` answers queries about, so the table carries one string
/// rather than two spellings of it. What it is NOT is a guarantee that
/// every hyprctl subcommand takes it; see [`apply_live`], which had to
/// go looking for one that does.
///
/// The names, ranges and defaults are Hyprland's, read from
/// `src/config/values/ConfigValues.cpp` at the tag this project ships —
/// NOT from the wiki. The wiki said `rounding` accepted [0 - 100]; the
/// source says `{.min = 0, .max = 20}`, and the source is what the
/// binary enforces. A slider offering a value the compositor rejects is
/// a control that lies, and this table had one for a day. Those are facts about an interface;
/// the wording of every LABEL here is nacelle's own, so that nothing
/// of the wiki's prose is carried into this binary.
pub struct Opt {
    pub key: &'static str,
    /// Read by `settings.rs`'s own
    /// `the_compositor_rows_are_the_option_table`, which is what keeps
    /// the page's rows and this table saying the same thing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub label: &'static str,
    pub kind: Kind,
    pub min: u32,
    pub max: u32,
    pub default: u32,
}

/// The tab, in order. Curated rather than exhaustive: Hyprland has
/// hundreds of options and a settings page that lists them all is a
/// worse tool than one that answers the questions people actually ask.
/// Everything here is a single scalar the compositor applies without a
/// restart — keybinds, window rules and monitor layouts are none of
/// those things and are deliberately absent.
pub const OPTS: &[Opt] = &[
    // The two gaps are the one place this table is NARROWER than the
    // compositor. Hyprland types them `CssGap`, not `Int`: each takes
    // one to four numbers and spreads them over the edges the way a CSS
    // margin does, so `5 10` is a real and different setting from `5`. A
    // slider can say one number, so one number is what these rows write
    // — the form that means "every edge the same". What a person cannot
    // reach from here is any UNEVEN gap, and a config that already had
    // one loses it the first time this row saves.
    //
    // Their min and max are nacelle's own and not the compositor's: the
    // source clamps a CssGap at neither end, and a slider has to stop
    // somewhere. Only the DEFAULT is Hyprland's.
    Opt { key: "general.gaps_in", label: "INNER GAP", kind: Kind::Int, min: 0, max: 50, default: 5 },
    Opt { key: "general.gaps_out", label: "OUTER GAP", kind: Kind::Int, min: 0, max: 100, default: 20 },
    Opt { key: "general.border_size", label: "BORDER SIZE", kind: Kind::Int, min: 0, max: 20, default: 1 },
    Opt { key: "decoration.rounding", label: "CORNER ROUNDING", kind: Kind::Int, min: 0, max: 20, default: 0 },
    Opt { key: "decoration.active_opacity", label: "ACTIVE OPACITY", kind: Kind::Percent, min: 0, max: 100, default: 100 },
    Opt { key: "decoration.inactive_opacity", label: "INACTIVE OPACITY", kind: Kind::Percent, min: 0, max: 100, default: 100 },
    Opt { key: "decoration.blur.enabled", label: "WINDOW BLUR", kind: Kind::Flag, min: 0, max: 1, default: 1 },
    Opt { key: "decoration.blur.size", label: "BLUR SIZE", kind: Kind::Int, min: 0, max: 100, default: 8 },
    Opt { key: "decoration.blur.passes", label: "BLUR PASSES", kind: Kind::Int, min: 0, max: 10, default: 1 },
    Opt { key: "decoration.shadow.enabled", label: "WINDOW SHADOW", kind: Kind::Flag, min: 0, max: 1, default: 1 },
    Opt { key: "animations.enabled", label: "ANIMATIONS", kind: Kind::Flag, min: 0, max: 1, default: 1 },
    Opt { key: "input.repeat_rate", label: "KEY REPEAT RATE", kind: Kind::Int, min: 0, max: 200, default: 25 },
    Opt { key: "input.repeat_delay", label: "KEY REPEAT DELAY", kind: Kind::Int, min: 0, max: 2000, default: 600 },
    Opt { key: "input.natural_scroll", label: "NATURAL SCROLL", kind: Kind::Flag, min: 0, max: 1, default: 0 },
    // `render:` — how the compositor gets colour onto the wire. These
    // are the only options on this page that are NOT a matter of taste:
    // a wrong one here is a washed-out screen or a black one, which is
    // why they are on a page of their own rather than mixed in with the
    // gaps and the blur.
    //
    // Four of the seven are enumerations and not quantities
    // ([`Kind::Choice`]) — the source types them `Int` with an
    // `OptionMap`, so 2 means "auto" and there is no sense in which it
    // is twice 1. A slider would have offered "one and a half".
    Opt { key: "render.cm_enabled", label: "COLOR MANAGEMENT", kind: Kind::Flag, min: 0, max: 1, default: 1 },
    Opt {
        key: "render.cm_auto_hdr",
        label: "AUTO HDR",
        kind: Kind::Choice(&["disable", "hdr", "hdredid"]),
        min: 0,
        max: 2,
        default: 1,
    },
    // The one `MS<String>` on this page, and the one whose members are
    // NOT stated in ConfigValues.cpp: the source gives it no map at all,
    // and the words it accepts are the keys of the table in
    // `src/helpers/TransferFunction.cpp`. That table also takes "0",
    // "1", "2" and "3" as aliases of four of these; the aliases reach no
    // state the names do not, so the list below is the whole of what the
    // option can mean.
    Opt {
        key: "render.cm_sdr_eotf",
        label: "SDR TRANSFER",
        kind: Kind::Word(&["default", "auto", "srgb", "gamma22", "gamma22force"]),
        min: 0,
        max: 4,
        default: 0,
    },
    Opt {
        key: "render.use_fp16",
        label: "FP16 BUFFER",
        kind: Kind::Choice(&["disable", "enable", "auto"]),
        min: 0,
        max: 2,
        default: 2,
    },
    Opt {
        key: "render.fp16_sdr_tf",
        label: "FP16 SDR TRANSFER",
        kind: Kind::Choice(&["monitor", "linear"]),
        min: 0,
        max: 1,
        default: 0,
    },
    Opt { key: "render.icc_vcgt_enabled", label: "ICC VCGT RAMPS", kind: Kind::Flag, min: 0, max: 1, default: 1 },
    Opt {
        key: "render.non_shader_cm",
        label: "SHADERLESS COLOR",
        kind: Kind::Choice(&["disable", "always", "ondemand", "ignore"]),
        min: 0,
        max: 3,
        default: 3,
    },
];

/// The word an enumerated option's current value stands for, in the
/// compositor's own spelling.
///
/// Answers for every kind, because the control that asks is a cycler and
/// a cycler on a non-enumerated option would otherwise have to be
/// prevented by hand; here it simply reads as its own number, which is
/// visibly wrong on screen rather than quietly wrong in the file.
pub fn word_of(o: &Opt, v: u32) -> String {
    match o.kind {
        Kind::Choice(w) | Kind::Word(w) => {
            w.get(v as usize).map(|s| s.to_uppercase()).unwrap_or_else(|| v.to_string())
        }
        _ => v.to_string(),
    }
}

/// The next state of an enumerated option, wrapping at the end.
///
/// Wrapping and not stopping: a cycler that stopped on its last member
/// would be a control a person can get stuck at the wrong end of, with
/// nothing on screen saying which way to press.
pub fn next_choice(o: &Opt, v: u32) -> u32 {
    if v >= o.max { o.min } else { v + 1 }
}

/// Byte equality, written out because `==` on `str` is not something a
/// `const fn` may call. Nothing subtler is wanted: these are ASCII keys
/// this file spells itself.
const fn same_key(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Where `key` sits in [`OPTS`] — the way a UI row is allowed to name an
/// option.
///
/// The rows used to carry the bare number, `comp_slider!(7)`, and that
/// number meant nothing on its own: inserting an option in the middle of
/// the table silently renumbered every row below it, so a slider went on
/// working while it changed a DIFFERENT setting, and nothing in the
/// program was in a position to notice. A key cannot slide.
///
/// It is a `const fn` so the lookup happens while compiling and costs
/// nothing at runtime, and — the point of it — so that a key no option
/// answers to is a BUILD FAILURE rather than a panic the first time
/// somebody opens the tab. The panic below is reached during const
/// evaluation, where a panic is an error; rustc's message names the row
/// that asked.
pub const fn idx(key: &str) -> usize {
    let mut i = 0;
    while i < OPTS.len() {
        if same_key(OPTS[i].key.as_bytes(), key.as_bytes()) {
            return i;
        }
        i += 1;
    }
    // Const panics take no arguments, so the key cannot be named here.
    // The compiler names the invocation site instead, which is the half
    // that has to be found.
    panic!("no compositor option answers to that key")
}

/// The file both sides agreed on — the name is `nacelle-session`'s to
/// state (`hyprconf::SETTINGS_FILE`) and is repeated here because the
/// two crates do not depend on one another. The test below pins it.
pub const FILE: &str = "nacelle-compositor.lua";

/// Whether this process is running under Hyprland.
///
/// The instance signature is set by Hyprland itself for every client it
/// starts, so its presence is the compositor saying so rather than this
/// program guessing from a binary name — and it is what names the IPC
/// socket, so a tab that can be shown is also a tab that can act.
pub fn running_under_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

/// The variable nacelle-session hands the settings path down in
/// (`hyprconf::CONF_ENV`). Repeated here for the same reason [`FILE`]
/// is: the two crates do not depend on one another.
pub const CONF_ENV: &str = "NACELLE_COMPOSITOR_CONF";

/// The marker nacelle-session sets on everything it starts.
pub const SESSION_MARKER: &str = "NACELLE_SESSION";

/// Why there is no path to write to.
#[derive(Debug, PartialEq, Eq)]
pub enum NoPath {
    /// Inside a nacelle session, but the session never said where.
    ///
    /// This is not "no config home" and must not be reported as one: it
    /// means the session and the desktop are BUILD-SKEWED — an older
    /// launcher that does not export [`CONF_ENV`] running a newer
    /// desktop that expects it. The old behaviour here was to fall back
    /// to a directory of our own and say nothing, which writes a real
    /// file that the running compositor will never read. Refusing is
    /// the honest answer, and the message has to name the reason,
    /// because "cannot save" sends whoever reads it looking at
    /// permissions.
    VersionSkew,
    /// Not in a session and no config home either — nowhere to put it.
    NoConfigHome,
}

impl NoPath {
    pub fn why(&self) -> &'static str {
        match self {
            Self::VersionSkew => {
                "nacelle-session did not say where the compositor settings go \
                 — the session and the desktop are different versions"
            }
            Self::NoConfigHome => "no config home to write the compositor settings into",
        }
    }
}

/// Where the file goes, as a rule rather than as a lookup.
///
/// Split from the environment so it can be tested over every combination
/// of inputs without touching the process's own — a test that sets
/// variables races every other test in the binary.
///
/// The rule, in order:
///   1. The session said where. Take it literally, whatever it says.
///   2. In a session that said nothing → refuse; see [`NoPath::VersionSkew`].
///   3. Otherwise nacelle's OWN config root — never `<config home>/hypr`,
///      which belongs to the user and which nacelle does not write into.
fn resolve(conf: Option<&OsStr>, in_session: bool, config_home: Option<&OsStr>, home: Option<&OsStr>) -> Result<PathBuf, NoPath> {
    if let Some(c) = conf.filter(|c| !c.is_empty()) {
        return Ok(PathBuf::from(c));
    }
    if in_session {
        return Err(NoPath::VersionSkew);
    }
    // Empty is not a path. `var_os` hands back an empty OsString for a
    // variable that is set to nothing, and joining onto it yields a
    // RELATIVE path — a file written next to wherever the process
    // happens to be standing. config.rs guards this same case; this is
    // the second place that has to.
    let base = config_home
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".config")))
        .ok_or(NoPath::NoConfigHome)?;
    Ok(base.join("nacelle").join("config").join("hypr").join(FILE))
}

/// [`resolve`] against this process's environment.
pub fn path() -> Result<PathBuf, NoPath> {
    let conf = std::env::var_os(CONF_ENV);
    let ch = std::env::var_os("XDG_CONFIG_HOME");
    let home = std::env::var_os("HOME");
    resolve(
        conf.as_deref(),
        std::env::var_os(SESSION_MARKER).is_some(),
        ch.as_deref(),
        home.as_deref(),
    )
}

/// The Lua literal for one value.
fn lua_value(o: &Opt, v: u32) -> String {
    match o.kind {
        Kind::Flag => if v != 0 { "true".into() } else { "false".into() },
        Kind::Int => v.to_string(),
        // Two decimals is the whole precision a percent slider has, and
        // writing `1.00` rather than `1` keeps the value a Lua float —
        // Hyprland's opacity options are floats and an integer 1 would
        // be a different type arriving where a float is expected.
        Kind::Percent => format!("{:.2}", v as f32 / 100.0),
        // The number, because that is what the compositor's OptionMap
        // maps the word ONTO — writing "auto" would work too and would
        // put a second spelling of one value into the file for the
        // parser below to have to know about.
        Kind::Choice(_) => v.to_string(),
        // …and here the word, because the option is a string one and a
        // number arriving at it is not a member of anything. An
        // out-of-range index writes the first member rather than an
        // empty string: the compositor reads an unknown word as its own
        // default, so an empty one would silently mean something else.
        Kind::Word(w) => format!("\"{}\"", w.get(v as usize).copied().unwrap_or(w[0])),
    }
}

/// The whole file for one set of values.
///
/// Written as ONE `hl.config` call with each option under its FULL
/// dotted key in bracket form — `["decoration.blur.enabled"] = true` —
/// and not as nested tables. That is not a style choice; the nested
/// form is a trap this module fell into once. Three options share the
/// `decoration.blur` prefix, and writing each as its own inline table
/// puts three `blur` keys in one constructor, where Lua keeps only the
/// last and silently drops the other two before Hyprland ever sees
/// them. Flat keys cannot collide, and the wiki documents this form.
pub fn render(values: &[u32]) -> String {
    let mut out = String::new();
    out.push_str("-- Written by nacelle's COMPOSITOR settings.\n");
    out.push_str("-- Loaded last by the session's own config, so these win.\n");
    out.push_str("-- Hand edits are lost the next time that tab saves.\n\n");
    out.push_str("hl.config({\n");
    for (o, v) in OPTS.iter().zip(values.iter().copied()) {
        out.push_str(&format!("    [\"{}\"] = {},\n", o.key, lua_value(o, v)));
    }
    out.push_str("})\n");
    out
}


/// The values in `text`, defaults for anything it does not name.
pub fn parse(text: &str) -> Vec<u32> {
    OPTS.iter()
        .map(|o| {
            let want = format!("[\"{}\"]", o.key);
            text.lines()
                .find_map(|l| {
                    let (lhs, rhs) = l.trim().split_once('=')?;
                    if lhs.trim() != want {
                        return None;
                    }
                    let v = rhs.trim().trim_end_matches(',').trim();
                    Some(match o.kind {
                        Kind::Flag => u32::from(v == "true"),
                        Kind::Int => v.parse().ok()?,
                        Kind::Percent => {
                            (v.parse::<f32>().ok()? * 100.0).round().clamp(0.0, 100.0) as u32
                        }
                        Kind::Choice(_) => v.parse().ok()?,
                        // A word this list does not know is not an error
                        // to report — it is a file somebody edited by
                        // hand, and `None` here lands on the option's
                        // own default, which is what the compositor will
                        // make of the same word.
                        Kind::Word(w) => {
                            let want = v.trim_matches('"');
                            w.iter().position(|s| *s == want)? as u32
                        }
                    })
                })
                .unwrap_or(o.default)
                .clamp(o.min, o.max)
        })
        .collect()
}

/// The values on disk, or the defaults where there is no file yet.
pub fn read() -> Vec<u32> {
    path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse(&t))
        .unwrap_or_else(|| OPTS.iter().map(|o| o.default).collect())
}

/// Writes the file. The directory is nacelle-session's, and it exists
/// whenever a session started — but not when the desktop is run by
/// hand outside one, hence the create.
pub fn write(values: &[u32]) -> std::io::Result<()> {
    let p = path().map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.why()))?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(p, render(values))
}

/// Pokes the running compositor so one row's change shows at once.
///
/// Best-effort by design: `hyprctl` may not be installed, and the file
/// write is what actually persists. A failure here costs a moment's
/// delay before the reload catches up, and nothing else — so it is not
/// reported as an error the user has to act on.
/// The Lua one row of the file would have been, for [`apply_live`] to
/// hand to `hyprctl eval`. Split out from the call so a test can look at
/// it: this fragment has been wrong twice, and a string built inside a
/// process spawn is a string no test ever sees.
fn live_fragment(o: &Opt, v: u32) -> String {
    format!("hl.config({{ [\"{}\"] = {} }})", o.key, lua_value(o, v))
}

pub fn apply_live(o: &Opt, v: u32) {
    // `eval`, not `keyword`. The short one still exists, and reaching for
    // it is the obvious mistake, so the reason it is wrong is worth
    // writing down: hyprctl picks its config parser from the EXTENSION of
    // the main config file, and everything nacelle writes is Lua. Under
    // the Lua parser `keyword` refuses on its first line and answers
    // "keyword can't work with non-legacy parsers. Use eval." — an error
    // delivered as ordinary output, so it cannot be told from success by
    // an exit status either. There is no arrangement of nacelle's files
    // in which `keyword` would have worked.
    //
    // The fragment is one row of what `render` already emits. No shell is
    // involved — Command is an execvp, so the argument goes across whole
    // and there is nothing to quote against.
    let _ = std::process::Command::new("hyprctl")
        .arg("eval")
        .arg(live_fragment(o, v))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

// ------------------------------------------------------- the monitors

/// One display, as the running compositor describes it.
///
/// READ-ONLY, and that is a statement about where these values live
/// rather than an unfinished control. Everything on the COMPOSITOR pages
/// above is a global option written into one Lua file; a display's bit
/// depth is not an option at all — Hyprland carries it inside the
/// `monitor=` RULE for that output, which `CMonitorRuleParser::
/// parseBitdepth` reads as the single string "10". Writing one means
/// writing a rule per output into the config the session emits, which is
/// a different file with a different owner, so this page SHOWS what the
/// rules produced and does not yet edit them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    pub name: String,
    /// Hyprland's own short description — make and model, roughly.
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub refresh: f32,
    pub scale: f32,
    /// `currentFormat`, verbatim: `DRM_FORMAT_XRGB8888` and friends.
    pub format: String,
    /// `colorManagementPreset` — which colour pipeline the output is on.
    pub color: String,
    /// Whether the output is actually SCANNING OUT ten bits per channel,
    /// read off the format rather than off any setting.
    ///
    /// The two formats that count are the two Hyprland itself counts:
    /// `Monitor.cpp`'s `formats10bit` is `{XRGB2101010, XBGR2101010}`,
    /// which is exactly the pair whose names carry `2101010`. A rule
    /// asking for ten bits that the hardware refused looks no different
    /// from one that was never written, and this says which happened.
    pub ten_bit: bool,
}

/// The displays the compositor reports, or nothing at all.
///
/// NOTHING is the answer on every path that is not a running Hyprland
/// answering a question: no compositor, no `hyprctl` on the machine, a
/// non-zero exit, output that does not parse. The page draws an empty
/// list and says so — a settings page is not a place to fail, and there
/// is nothing here a user could have done wrong.
pub fn monitors() -> Vec<Monitor> {
    if !running_under_hyprland() {
        return Vec::new();
    }
    let Ok(out) = std::process::Command::new("hyprctl")
        .arg("monitors")
        .arg("-j")
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_monitors(&String::from_utf8_lossy(&out.stdout))
}

/// The end of the string literal starting at `i`, and its contents.
///
/// Only the two escapes Hyprland's own `escapeJSONStrings` produces are
/// undone. Anything else is left as written, which is what a reader
/// wants from a monitor description and what keeps this from pretending
/// to be a JSON library.
fn read_string(b: &[u8], i: usize) -> (String, usize) {
    let mut out = String::new();
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' if j + 1 < b.len() => {
                out.push(b[j + 1] as char);
                j += 2;
            }
            b'"' => return (out, j + 1),
            c => {
                out.push(c as char);
                j += 1;
            }
        }
    }
    (out, j)
}

/// Past the value starting at `i`, and the value itself where it is a
/// scalar. A nested object or array is skipped whole and reads as empty:
/// nothing this page shows comes from one, and the `activeWorkspace`
/// object carries its own `"name"` — which a scan that did not respect
/// nesting would hand back as the MONITOR's name.
fn read_value(b: &[u8], i: usize) -> (String, usize) {
    match b.get(i) {
        Some(b'"') => read_string(b, i),
        Some(b'{') | Some(b'[') => {
            let mut j = i;
            let mut depth = 0usize;
            while j < b.len() {
                match b[j] {
                    b'"' => j = read_string(b, j).1,
                    b'{' | b'[' => {
                        depth += 1;
                        j += 1;
                    }
                    b'}' | b']' => {
                        depth -= 1;
                        j += 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => j += 1,
                }
            }
            (String::new(), j)
        }
        _ => {
            let mut j = i;
            while j < b.len() && b[j] != b',' && b[j] != b'}' && b[j] != b']' {
                j += 1;
            }
            (String::from_utf8_lossy(&b[i..j]).trim().to_string(), j)
        }
    }
}

/// What ONE object says about itself — its own pairs, and not those of
/// any object nested inside it.
fn own_fields(obj: &str) -> Vec<(String, String)> {
    let b = obj.as_bytes();
    let mut out = Vec::new();
    let Some(start) = b.iter().position(|c| *c == b'{') else { return out };
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < b.len() && depth > 0 {
        match b[i] {
            b'"' if depth == 1 => {
                let (key, mut j) = read_string(b, i);
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if b.get(j) == Some(&b':') {
                    j += 1;
                    while j < b.len() && b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    let (value, next) = read_value(b, j);
                    out.push((key, value));
                    i = next;
                } else {
                    i = j;
                }
            }
            b'"' => i = read_string(b, i).1,
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// `hyprctl monitors -j`, read as far as this page needs it.
///
/// Written by hand rather than with a JSON crate because this crate has
/// none, and a dependency is a poor price for reading nine scalars out
/// of one command's output. What it must get right is NESTING — see
/// [`read_value`] — and a fixture in the tests below holds it to that.
/// Anything unrecognised reads as the field's default rather than
/// failing the whole list: one odd output should cost one line of the
/// page, not the page.
pub fn parse_monitors(text: &str) -> Vec<Monitor> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut depth = 0usize;
    let mut start = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' => i = read_string(b, i).1,
            b'{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
                i += 1;
            }
            b'}' => {
                i += 1;
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    out.push(monitor_of(&own_fields(&text[start..i])));
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn monitor_of(fields: &[(String, String)]) -> Monitor {
    let get = |k: &str| {
        fields.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or_default()
    };
    let format = get("currentFormat").to_string();
    Monitor {
        name: get("name").to_string(),
        description: get("description").to_string(),
        width: get("width").parse().unwrap_or(0),
        height: get("height").parse().unwrap_or(0),
        refresh: get("refreshRate").parse().unwrap_or(0.0),
        scale: get("scale").parse().unwrap_or(0.0),
        ten_bit: format.contains("2101010"),
        format,
        color: get("colorManagementPreset").to_string(),
    }
}

/// True when `p` is the file this module owns.
#[cfg(test)]
fn is_settings_file(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()) == Some(FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Vec<u32> {
        OPTS.iter().map(|o| o.default).collect()
    }

    /// Every value written comes back out. The file is the storage, so
    /// a value that does not survive its own round trip is a setting
    /// that silently resets.
    #[test]
    fn every_option_survives_its_own_round_trip() {
        // A value that is neither the default nor a limit, so a parse
        // that quietly fell back to either would be caught.
        let vals: Vec<u32> = OPTS
            .iter()
            .map(|o| match o.kind {
                Kind::Flag => 1 - o.default,
                _ => o.min + (o.max - o.min) / 3,
            })
            .collect();
        let got = parse(&render(&vals));
        for (i, o) in OPTS.iter().enumerate() {
            assert_eq!(got[i], vals[i], "{} did not survive the round trip", o.key);
        }
    }

    /// The defaults do too — the common case, and the one where a
    /// wrong fallback hides best.
    #[test]
    fn the_defaults_survive_the_round_trip() {
        assert_eq!(parse(&render(&defaults())), defaults());
    }

    /// An absent file is defaults, not zeroes: a person who has never
    /// opened the tab must see the compositor's own behaviour.
    #[test]
    fn an_empty_file_reads_as_the_defaults() {
        assert_eq!(parse(""), defaults());
    }

    /// Each option must be recognisable on its own line. Two options
    /// answering to one token would read each other's values — which is
    /// not hypothetical: `decoration.blur.enabled` and
    /// `decoration.shadow.enabled` share the leaf `enabled`, and an
    /// earlier parser matched on the leaf alone and got them both wrong.
    /// Writing the FULL key is what makes that impossible, so the full
    /// key is what this checks.
    #[test]
    fn every_option_is_recognised_by_something_unique() {
        let mut toks: Vec<&str> = OPTS.iter().map(|o| o.key).collect();
        toks.sort_unstable();
        let before = toks.len();
        toks.dedup();
        assert_eq!(before, toks.len(), "two options answer to one token: {toks:?}");
    }

    /// The collision above, as a behaviour rather than a property: two
    /// options that share a leaf must be able to hold OPPOSITE values.
    #[test]
    fn two_options_sharing_a_leaf_keep_their_own_values() {
        let (blur, shadow) = (
            OPTS.iter().position(|o| o.key == "decoration.blur.enabled").unwrap(),
            OPTS.iter().position(|o| o.key == "decoration.shadow.enabled").unwrap(),
        );
        let mut v = defaults();
        v[blur] = 1;
        v[shadow] = 0;
        let got = parse(&render(&v));
        assert_eq!(got[blur], 1, "blur lost its value to shadow");
        assert_eq!(got[shadow], 0, "shadow lost its value to blur");
    }

    /// [`idx`] finds each option where it actually is, and nothing else.
    ///
    /// The lookup the UI rows depend on, checked at run time because the
    /// interesting failures are ones a passing build cannot show: it is
    /// a build error only when it is WRONG, and a lookup that answered
    /// confidently with the wrong row would build just fine.
    #[test]
    fn a_key_finds_its_own_option_and_no_other() {
        for (i, o) in OPTS.iter().enumerate() {
            assert_eq!(idx(o.key), i, "{} is looked up as somebody else", o.key);
        }
        // A key that is a PREFIX of a real one is not that one —
        // `decoration.blur` is not `decoration.blur.enabled`, and a
        // comparison that stopped at the shorter length would say it is.
        assert!(!same_key(b"decoration.blur", b"decoration.blur.enabled"));
        assert!(!same_key(b"decoration.blur.enabled", b"decoration.blur"));
    }

    /// And an option that does not exist is refused rather than guessed
    /// at. In a row this is a build failure; called at run time, as here,
    /// the same refusal is a panic.
    #[test]
    #[should_panic(expected = "no compositor option answers to that key")]
    fn a_key_no_option_answers_to_is_refused() {
        idx(std::hint::black_box("decoration.rounding_power"));
    }

    /// What Hyprland's own source says about each option this tab shows,
    /// as the NUMBERS it says rather than as a copy of the file.
    ///
    /// Taken from `src/config/values/ConfigValues.cpp` at tag v0.56.2,
    /// where every entry reads
    /// `MS<Type>("section:name", "…", default, {.min = …, .max = …})`.
    /// [`ty`](Pin::ty) is that `Type`, verbatim, because it is the fact
    /// that decides how the number is read.
    ///
    /// The alternative was to vendor the file and `include_str!` it, and
    /// it was rejected for two reasons. It is 59 KB of somebody else's
    /// BSD-3 source, which is a licence notice this MIT crate would then
    /// have to carry and keep correct, for a test fixture; and the same
    /// module header that says the LABELS here are nacelle's own wording
    /// is making the same point — this program does not carry
    /// Hyprland's text around. A range and a default are facts about an
    /// interface, and facts are what is copied. What is lost by not
    /// vendoring is that the numbers below are transcribed by hand, so
    /// the check is against a TRANSCRIPTION and not against the source;
    /// the transcription is worth having anyway, because it is what
    /// turns a bare `max: 20` in the table above into a claim somebody
    /// can check, and because it records the C++ type the table does
    /// not — which is how the CssGap narrowing became visible at all.
    struct Pin {
        key: &'static str,
        /// The `MS<…>` template argument at the tag.
        ty: &'static str,
        /// `.min` and `.max` as the source writes them, or `None` where
        /// the source gives none — an absent clamp is itself a fact, and
        /// the one that says a range in [`OPTS`] is nacelle's invention.
        min: Option<f64>,
        max: Option<f64>,
        default: Def,
        /// The `.map` beside an `Int`, in the source's NUMERIC order, or
        /// the words a `String` option is read against. Empty where the
        /// option is a plain quantity.
        words: &'static [&'static str],
    }

    /// A default is a number for most of these and a WORD for the one
    /// string option, and the two cannot be flattened into each other: a
    /// `0.0` standing for `"default"` would be a claim about a number the
    /// source never makes.
    #[derive(PartialEq, Debug)]
    enum Def {
        Num(f64),
        Word(&'static str),
    }

    const PINS: &[Pin] = &[
        Pin { key: "general.gaps_in", ty: "CssGap", min: None, max: None, default: Def::Num(5.0), words: &[] },
        Pin { key: "general.gaps_out", ty: "CssGap", min: None, max: None, default: Def::Num(20.0), words: &[] },
        Pin { key: "general.border_size", ty: "Int", min: Some(0.0), max: Some(20.0), default: Def::Num(1.0), words: &[] },
        Pin { key: "decoration.rounding", ty: "Int", min: Some(0.0), max: Some(20.0), default: Def::Num(0.0), words: &[] },
        Pin { key: "decoration.active_opacity", ty: "Float", min: Some(0.0), max: Some(1.0), default: Def::Num(1.0), words: &[] },
        Pin { key: "decoration.inactive_opacity", ty: "Float", min: Some(0.0), max: Some(1.0), default: Def::Num(1.0), words: &[] },
        Pin { key: "decoration.blur.enabled", ty: "Bool", min: None, max: None, default: Def::Num(1.0), words: &[] },
        Pin { key: "decoration.blur.size", ty: "Int", min: Some(0.0), max: Some(100.0), default: Def::Num(8.0), words: &[] },
        Pin { key: "decoration.blur.passes", ty: "Int", min: Some(0.0), max: Some(10.0), default: Def::Num(1.0), words: &[] },
        Pin { key: "decoration.shadow.enabled", ty: "Bool", min: None, max: None, default: Def::Num(1.0), words: &[] },
        Pin { key: "animations.enabled", ty: "Bool", min: None, max: None, default: Def::Num(1.0), words: &[] },
        Pin { key: "input.repeat_rate", ty: "Int", min: Some(0.0), max: Some(200.0), default: Def::Num(25.0), words: &[] },
        Pin { key: "input.repeat_delay", ty: "Int", min: Some(0.0), max: Some(2000.0), default: Def::Num(600.0), words: &[] },
        Pin { key: "input.natural_scroll", ty: "Bool", min: None, max: None, default: Def::Num(0.0), words: &[] },
        Pin { key: "render.cm_enabled", ty: "Bool", min: None, max: None, default: Def::Num(1.0), words: &[] },
        Pin {
            key: "render.cm_auto_hdr",
            ty: "Int",
            min: Some(0.0),
            max: Some(2.0),
            default: Def::Num(1.0),
            words: &["disable", "hdr", "hdredid"],
        },
        // The one option whose members are NOT in ConfigValues.cpp: it is
        // an `MS<String>` with no map, and the words come from the table
        // in src/helpers/TransferFunction.cpp at the same tag. The
        // numeric aliases that table also accepts ("0", "1", "2", "3")
        // are left out on purpose — they name states already named here,
        // so offering them would be one control with two spellings of
        // four of its five positions.
        Pin {
            key: "render.cm_sdr_eotf",
            ty: "String",
            min: None,
            max: None,
            default: Def::Word("default"),
            words: &["default", "auto", "srgb", "gamma22", "gamma22force"],
        },
        Pin {
            key: "render.use_fp16",
            ty: "Int",
            min: Some(0.0),
            max: Some(2.0),
            default: Def::Num(2.0),
            words: &["disable", "enable", "auto"],
        },
        Pin {
            key: "render.fp16_sdr_tf",
            ty: "Int",
            min: Some(0.0),
            max: Some(1.0),
            default: Def::Num(0.0),
            words: &["monitor", "linear"],
        },
        Pin { key: "render.icc_vcgt_enabled", ty: "Bool", min: None, max: None, default: Def::Num(1.0), words: &[] },
        Pin {
            key: "render.non_shader_cm",
            ty: "Int",
            min: Some(0.0),
            max: Some(3.0),
            default: Def::Num(3.0),
            words: &["disable", "always", "ondemand", "ignore"],
        },
    ];

    /// The number a pin states, for the kinds whose default is one.
    fn pinned_number(p: &Pin) -> f64 {
        match p.default {
            Def::Num(n) => n,
            Def::Word(w) => panic!("{}: a numeric option defaulting to \"{w}\"", p.key),
        }
    }

    /// Every option says what the compositor says.
    ///
    /// The wiki said `decoration:rounding` took [0 - 100] and the source
    /// says `{.max = 20}`, so for a day this tab offered eighty values
    /// Hyprland throws away — a control that lies, and one nothing in
    /// the program could have caught. This is what catches the next one.
    ///
    /// Each C++ type is read the way that type is read:
    ///   * `Int` — the numbers are ours unchanged, and where the source
    ///     puts an `OptionMap` beside them the members are ours too:
    ///     [`Kind::Choice`] carries that map's words in its order, so a
    ///     member the compositor grew or lost fails here.
    ///   * `Bool` — no clamp of its own, drawn as a toggle, so 0..1.
    ///   * `Float` — 0.0..1.0 in the source, shown here as a percentage
    ///     ([`Kind::Percent`]), so the source's numbers times a hundred.
    ///   * `String` — no clamp and no map in ConfigValues.cpp at all; the
    ///     members are transcribed from the table that READS the option,
    ///     and the range is that list's own length.
    ///   * `CssGap` — one to four numbers with no clamp at either end.
    ///     Only the default can be compared; see the note in [`OPTS`].
    #[test]
    fn every_option_says_what_the_compositors_own_source_says() {
        assert_eq!(
            PINS.len(),
            OPTS.len(),
            "an option was added or removed without a line saying what Hyprland calls it"
        );
        for (o, p) in OPTS.iter().zip(PINS) {
            assert_eq!(o.key, p.key, "the table and the transcription are in different orders");
            let (want_min, want_max, want_default, want_kind) = match p.ty {
                "Int" => (
                    p.min.expect("an Int with no min in the source"),
                    p.max.expect("an Int with no max in the source"),
                    pinned_number(p),
                    // An Int WITH a map is not a quantity. The map's own
                    // numbering is what makes the index writable as the
                    // value, so it is checked: every one of these starts
                    // at 0 and runs without a gap, which is why an index
                    // and a value can be the same number.
                    if p.words.is_empty() {
                        Kind::Int
                    } else {
                        assert_eq!(
                            (p.min, p.max),
                            (Some(0.0), Some(p.words.len() as f64 - 1.0)),
                            "{}: the map does not run 0..n, so an index is not its value",
                            o.key
                        );
                        Kind::Choice(p.words)
                    },
                ),
                "Bool" => {
                    assert!(
                        p.min.is_none() && p.max.is_none(),
                        "{}: a Bool with a clamp — the transcription is wrong",
                        o.key
                    );
                    (0.0, 1.0, pinned_number(p), Kind::Flag)
                }
                "Float" => {
                    assert_eq!(
                        (p.min, p.max),
                        (Some(0.0), Some(1.0)),
                        "{}: a Float over some other range than 0..1 cannot be a percentage",
                        o.key
                    );
                    (0.0, 100.0, pinned_number(p) * 100.0, Kind::Percent)
                }
                // The source clamps nothing here and names no members;
                // both come from the reader, so what can be checked is
                // that the table offers exactly those words and opens on
                // the one the source calls the default.
                "String" => {
                    assert!(
                        p.min.is_none() && p.max.is_none() && !p.words.is_empty(),
                        "{}: a String with a clamp or with no members transcribed",
                        o.key
                    );
                    let Def::Word(word) = p.default else {
                        panic!("{}: a String option defaulting to a number", o.key)
                    };
                    let want = p
                        .words
                        .iter()
                        .position(|w| *w == word)
                        .unwrap_or_else(|| panic!("{}: the default is not one of the members", o.key));
                    (0.0, p.words.len() as f64 - 1.0, want as f64, Kind::Word(p.words))
                }
                // The narrowing. The range is nacelle's own, so the only
                // thing to check is that it is a range at all and that
                // the value the tab opens on is the compositor's.
                "CssGap" => {
                    assert!(
                        p.min.is_none() && p.max.is_none(),
                        "{}: the source grew a clamp — OPTS may now state it",
                        o.key
                    );
                    assert!(o.min < o.max, "{} has an empty range", o.key);
                    assert_eq!(o.kind, Kind::Int, "{} is no longer the one-number narrowing", o.key);
                    assert_eq!(
                        Def::Num(o.default as f64),
                        p.default,
                        "{}'s default is not Hyprland's",
                        o.key
                    );
                    continue;
                }
                other => panic!("{}: nothing here knows how to read a {other}", o.key),
            };
            assert_eq!(o.kind, want_kind, "{} is read as the wrong kind of number", o.key);
            assert_eq!(o.min as f64, want_min, "{}'s minimum is not the source's", o.key);
            assert_eq!(o.max as f64, want_max, "{}'s maximum is not the source's", o.key);
            assert_eq!(o.default as f64, want_default, "{}'s default is not the source's", o.key);
        }
    }

    /// A default outside its own range would make the slider open on a
    /// value it cannot return to.
    #[test]
    fn every_default_is_inside_its_own_range() {
        for o in OPTS {
            assert!(
                (o.min..=o.max).contains(&o.default),
                "{}'s default {} is outside {}..{}",
                o.key,
                o.default,
                o.min,
                o.max
            );
            assert!(o.min < o.max, "{} has an empty range", o.key);
        }
    }

    /// Flags are the one kind whose range is fixed, because the toggle
    /// that draws them has no other reading.
    #[test]
    fn a_flag_is_zero_or_one_and_nothing_else() {
        for o in OPTS.iter().filter(|o| o.kind == Kind::Flag) {
            assert_eq!((o.min, o.max), (0, 1), "{} is a flag with a wider range", o.key);
        }
    }

    /// Percentages reach Lua as floats. An integer `1` where Hyprland
    /// expects a float is a different type arriving at its parser.
    #[test]
    fn a_percentage_is_written_as_a_lua_float() {
        let i = OPTS.iter().position(|o| o.kind == Kind::Percent).expect("no percent option");
        let key = OPTS[i].key;
        let mut v = defaults();
        v[i] = 100;
        let t = render(&v);
        let line = t
            .lines()
            .find(|l| l.contains(key))
            .unwrap_or_else(|| panic!("{key} is not in the file: {t}"));
        assert!(line.contains("1.00"), "a full percentage is not a float: {line}");
    }


    /// An enumeration is written the way the compositor reads it, which
    /// is not the same answer for both kinds of enumeration.
    ///
    /// A `Choice` is an `Int` with a map, so the NUMBER goes into the
    /// file; a `Word` is an `MS<String>`, so the word does, quoted. Get
    /// these the wrong way round and Hyprland reads a number where it
    /// wants a name — which it answers by silently using its own
    /// default, so the page would show one state while the compositor
    /// was in another.
    #[test]
    fn a_named_state_is_written_the_way_its_own_option_is_read() {
        for o in OPTS {
            match o.kind {
                Kind::Choice(w) => {
                    for v in 0..w.len() as u32 {
                        assert_eq!(lua_value(o, v), v.to_string(), "{} wrote a word", o.key);
                    }
                }
                Kind::Word(w) => {
                    for (v, word) in w.iter().enumerate() {
                        assert_eq!(
                            lua_value(o, v as u32),
                            format!("\"{word}\""),
                            "{} wrote something other than its own quoted member",
                            o.key
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Every member of every enumeration survives the file, and the
    /// cycler can reach every one of them.
    ///
    /// Both halves matter and neither implies the other: a value the
    /// file loses is a setting that resets itself, and a member the
    /// cycler steps past is a state the compositor has and the user
    /// cannot ask for.
    #[test]
    fn every_member_of_an_enumeration_is_reachable_and_survives() {
        for (i, o) in OPTS.iter().enumerate() {
            let members = match o.kind {
                Kind::Choice(w) | Kind::Word(w) => w,
                _ => continue,
            };
            assert_eq!(
                members.len() as u32,
                o.max - o.min + 1,
                "{} offers {} members over a range of {}",
                o.key,
                members.len(),
                o.max - o.min + 1
            );
            let mut seen = vec![false; members.len()];
            let mut v = o.min;
            for _ in 0..members.len() {
                seen[v as usize] = true;
                let mut all = defaults();
                all[i] = v;
                assert_eq!(parse(&render(&all))[i], v, "{} lost member {v}", o.key);
                v = next_choice(o, v);
            }
            assert!(seen.iter().all(|s| *s), "{} has a member the cycler never lands on", o.key);
            assert_eq!(v, o.min, "{} does not come back round to its first member", o.key);
        }
    }

    /// A word this program does not know reads as the option's own
    /// default — which is what the COMPOSITOR does with the same word,
    /// so the page and the running session agree about a hand-edited
    /// file instead of disagreeing.
    #[test]
    fn a_member_nobody_knows_reads_as_the_default() {
        let i = OPTS
            .iter()
            .position(|o| matches!(o.kind, Kind::Word(_)))
            .expect("no string-valued option");
        let text = format!("hl.config({{\n    [\"{}\"] = \"gamma99\",\n}})\n", OPTS[i].key);
        assert_eq!(parse(&text)[i], OPTS[i].default);
    }

    /// A monitor's own name is its own, and not the one its workspace
    /// happens to carry.
    ///
    /// `hyprctl monitors -j` nests an `activeWorkspace` object inside
    /// every monitor, and that object has a `"name"` of its own. A scan
    /// that took the first `"name"` after the opening brace would be
    /// right, and a scan that took the LAST one would name every display
    /// after a workspace — which is the whole reason [`read_value`]
    /// skips a nested object whole rather than walking into it.
    #[test]
    fn a_monitor_is_read_past_the_objects_nested_inside_it() {
        // Shaped exactly as HyprCtl.cpp emits it, cut down to the fields
        // this page shows plus the nesting that makes it hard.
        let text = r#"[{
    "id": 0,
    "name": "DP-1",
    "description": "Some Maker Some Model",
    "width": 3840,
    "height": 2160,
    "refreshRate": 143.99899,
    "activeWorkspace": {
        "id": 1,
        "name": "not the monitor"
    },
    "reserved": [0, 44, 0, 0],
    "scale": 1.50,
    "currentFormat": "DRM_FORMAT_XRGB2101010",
    "availableModes": ["3840x2160@144.00Hz"],
    "colorManagementPreset": "hdr"
}, {
    "id": 1,
    "name": "HDMI-A-1",
    "description": "",
    "width": 1920,
    "height": 1080,
    "refreshRate": 60.00000,
    "activeWorkspace": {
        "id": 2,
        "name": "not this one either"
    },
    "scale": 1.00,
    "currentFormat": "DRM_FORMAT_XRGB8888",
    "colorManagementPreset": "srgb"
}]"#;
        let got = parse_monitors(text);
        assert_eq!(got.len(), 2, "the list is not one entry per monitor: {got:?}");
        assert_eq!(got[0].name, "DP-1", "a monitor took its workspace's name");
        assert_eq!(got[0].description, "Some Maker Some Model");
        assert_eq!((got[0].width, got[0].height), (3840, 2160));
        assert_eq!(got[0].scale, 1.5);
        assert_eq!(got[0].color, "hdr");
        assert!(got[0].ten_bit, "a 2101010 format did not read as ten bits");
        assert_eq!(got[1].name, "HDMI-A-1");
        assert!(!got[1].ten_bit, "an 8888 format read as ten bits");
        assert_eq!(got[1].refresh, 60.0);
    }

    /// Nothing about this page may PANIC on output it did not expect. A
    /// settings page that cannot list the displays shows no displays;
    /// there is no failure here a user could act on, so there is no
    /// failure to report.
    #[test]
    fn output_that_is_not_a_monitor_list_reads_as_no_monitors() {
        for text in [
            "",
            "[]",
            "not json at all",
            // Truncated mid-string, mid-object and mid-escape: the three
            // shapes a killed `hyprctl` leaves behind.
            "[{\"name\": \"DP-1",
            "[{\"name\": \"DP-1\", \"width\":",
            "[{\"description\": \"a \\",
        ] {
            let got = parse_monitors(text);
            assert!(
                got.iter().all(|m| m.width == 0 || !m.name.is_empty()),
                "{text:?} produced a half-read monitor: {got:?}"
            );
        }
        // And an empty list is what a machine with no Hyprland gets,
        // without anything being run to find out.
        if !running_under_hyprland() {
            assert!(monitors().is_empty(), "something was asked with no compositor to ask");
        }
    }

    /// The name of the file may be spelled ONCE in this crate, outside
    /// the tests and outside comments.
    ///
    /// A second spelling is not a compile error, is not a type error,
    /// and is exactly how the two halves of this seam came apart: one
    /// side derived a path, the other derived it again, and nothing
    /// compared them. Anyone adding a script, an addon or a doc that
    /// hardcodes the name gets this failure instead of a silent
    /// disagreement discovered months later.
    #[test]
    fn the_file_name_is_spelled_once_in_this_crate() {
        let src = include_str!("hyprsettings.rs");
        let body = src.split("mod tests").next().unwrap_or(src);
        let hits = body
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| l.contains("nacelle-compositor.lua"))
            .count();
        assert_eq!(hits, 1, "the file name is spelled {hits} times, not once");
    }

    /// And `<config home>/hypr` — the user's OWN Hyprland directory —
    /// is spelled nowhere outside comments. Nothing here writes there.
    #[test]
    fn the_users_own_hypr_directory_is_never_named_in_code() {
        let src = include_str!("hyprsettings.rs");
        let body = src.split("mod tests").next().unwrap_or(src);
        for (i, l) in body.lines().enumerate() {
            let t = l.trim_start();
            // Rust comments, and Lua comments this crate EMITS: the
            // generated config explains in prose that it leaves the
            // user's directory alone, which is the opposite of writing
            // there. What is being looked for is the path being USED.
            if t.starts_with("//") || t.contains("\"--") {
                continue;
            }
            assert!(
                !l.contains(".config/hypr"),
                "line {} names the user's own hypr directory: {l}",
                i + 1
            );
        }
    }

    /// The rule, over every combination of inputs, stated as a
    /// property rather than as a list of expected strings: whatever
    /// comes back, it is NEVER `<config home>/hypr`.
    ///
    /// That directory is the user's own Hyprland config, and nacelle
    /// does not write into it — the launcher's own module header says
    /// so. This test survives [`resolve`] being rewritten, and fails
    /// the moment anyone reintroduces the fallback that used to be
    /// here.
    #[test]
    fn no_input_whatsoever_makes_us_write_into_the_users_own_hypr_dir() {
        let ch = OsStr::new("/home/a/.config");
        let home = OsStr::new("/home/a");
        let confs = [None, Some(OsStr::new("")), Some(OsStr::new("/s/hypr/nacelle-compositor.lua"))];
        let homes = [None, Some(OsStr::new("")), Some(ch)];
        let mut seen = 0;
        for c in confs {
            for in_session in [false, true] {
                for h in homes {
                    seen += 1;
                    let Ok(p) = resolve(c, in_session, h, Some(home)) else { continue };
                    assert_ne!(
                        p,
                        PathBuf::from("/home/a/.config/hypr").join(FILE),
                        "conf={c:?} in_session={in_session} config_home={h:?} landed in the user's own dir"
                    );
                    assert!(
                        p.is_absolute(),
                        "conf={c:?} in_session={in_session} config_home={h:?} gave a relative path {}",
                        p.display()
                    );
                }
            }
        }
        assert_eq!(seen, 18, "the table of cases changed shape");
    }

    /// A session that says nothing is a VERSION SKEW, not a missing
    /// config home, and must be refused rather than papered over. The
    /// papering-over is what wrote a real file nothing would read.
    #[test]
    fn a_silent_session_is_refused_and_says_why() {
        let home = OsStr::new("/home/a");
        assert_eq!(resolve(None, true, None, Some(home)), Err(NoPath::VersionSkew));
        assert_eq!(resolve(Some(OsStr::new("")), true, None, Some(home)), Err(NoPath::VersionSkew));
        assert!(
            NoPath::VersionSkew.why().contains("different versions"),
            "the message does not name the reason: {}",
            NoPath::VersionSkew.why()
        );
        // Outside a session the same absence is ordinary: we pick our own.
        assert!(resolve(None, false, None, Some(home)).is_ok());
    }

    /// What the session says is taken LITERALLY. It computed the path
    /// from its own notion of where nacelle's config root is; second
    ///-guessing it here is how two derivations of one path appear.
    #[test]
    fn what_the_session_says_is_taken_as_given() {
        let told = OsStr::new("/run/somewhere/odd/nacelle-compositor.lua");
        for in_session in [false, true] {
            assert_eq!(
                resolve(Some(told), in_session, Some(OsStr::new("/home/a/.config")), Some(OsStr::new("/home/a"))),
                Ok(PathBuf::from(told)),
                "the session was told something and we went elsewhere"
            );
        }
    }

    /// The live fragment and the file must not drift apart: whatever
    /// `eval` is handed has to set the same key to the same value the
    /// saved file would, or the preview shows one thing and the reload
    /// another. Checked for every option, at both ends of its range.
    #[test]
    fn the_live_fragment_says_what_the_file_says() {
        for (i, o) in OPTS.iter().enumerate() {
            for v in [o.min, o.max, o.default] {
                let frag = live_fragment(o, v);
                let mut all = defaults();
                all[i] = v;
                let line = render(&all)
                    .lines()
                    .find(|l| l.contains(&format!("[\"{}\"]", o.key)))
                    .map(|l| l.trim().trim_end_matches(',').to_string())
                    .unwrap_or_else(|| panic!("{} is not in the file", o.key));
                assert!(
                    frag.contains(&line),
                    "{} = {v}: the file says `{line}` but eval gets `{frag}`",
                    o.key
                );
            }
        }
    }

    /// The file must PARSE AS LUA — the same guard the session's own
    /// emitter carries, for the same reason: nothing between here and
    /// the compositor checks it, and a syntax error costs the whole
    /// config, not one row. Skipped where no interpreter is installed.
    #[test]
    fn lua_keeps_every_option_and_not_just_the_last_of_a_prefix() {
        // Not a syntax check. An earlier version of render() wrote each
        // dotted key as a nested inline table, so the three decoration.blur
        // options became three `blur` keys in one constructor — legal Lua,
        // parses fine, and quietly keeps only the last. parse() could not
        // see it either, because parse() reads the TEXT and Lua had already
        // thrown two of them away. So the stub counts what actually
        // arrives, which is the only vantage point that catches this.
        let dir = std::env::temp_dir().join(format!("nacelle-hyprset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join(FILE);
        // `hl` does not exist outside Hyprland, so it is stubbed: this asks
        // what the table holds, not whether the machine runs a compositor.
        let stub = "local hl = { config = function(t) \
                     local n = 0 for k in pairs(t) do n = n + 1 print(k) end \
                     print('COUNT=' .. n) end }\n";
        std::fs::write(&f, format!("{stub}{}", render(&defaults()))).unwrap();
        let mut ran = false;
        for lua in ["lua", "lua5.5", "lua5.4", "luajit"] {
            let Ok(out) = std::process::Command::new(lua).arg(&f).output() else { continue };
            ran = true;
            assert!(
                out.status.success(),
                "the settings file is not valid Lua ({lua}): {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let seen = String::from_utf8_lossy(&out.stdout).into_owned();
            for o in OPTS {
                assert!(
                    seen.lines().any(|l| l == o.key),
                    "{} never reached hl.config — Lua dropped it",
                    o.key
                );
            }
            assert!(
                seen.contains(&format!("COUNT={}", OPTS.len())),
                "hl.config got a different number of keys than there are options:\n{seen}"
            );
            break;
        }
        if !ran {
            eprintln!("no Lua interpreter here — the parse check did not run");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Both crates must name the same file. nacelle-session declares it
    /// as `hyprconf::SETTINGS_FILE` and cannot be depended on from
    /// here, so the string is repeated — and pinned.
    #[test]
    fn the_file_name_is_the_one_the_session_loads() {
        assert_eq!(FILE, "nacelle-compositor.lua");
        assert!(is_settings_file(Path::new("/tmp/hypr/nacelle-compositor.lua")));
        assert!(!is_settings_file(Path::new("/tmp/hypr/hyprland.lua")));
    }
}
