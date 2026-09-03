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
/// The names, ranges and defaults are Hyprland's, read from its wiki
/// and its shipped example config. Those are facts about an interface;
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
    Opt { key: "general.gaps_in", label: "INNER GAP", kind: Kind::Int, min: 0, max: 50, default: 5 },
    Opt { key: "general.gaps_out", label: "OUTER GAP", kind: Kind::Int, min: 0, max: 100, default: 20 },
    Opt { key: "general.border_size", label: "BORDER SIZE", kind: Kind::Int, min: 0, max: 20, default: 1 },
    Opt { key: "decoration.rounding", label: "CORNER ROUNDING", kind: Kind::Int, min: 0, max: 100, default: 0 },
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
];

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

/// Where the file lives: beside the config nacelle-session wrote.
pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("hypr").join(FILE))
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
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse(&t))
        .unwrap_or_else(|| OPTS.iter().map(|o| o.default).collect())
}

/// Writes the file. The directory is nacelle-session's, and it exists
/// whenever a session started — but not when the desktop is run by
/// hand outside one, hence the create.
pub fn write(values: &[u32]) -> std::io::Result<()> {
    let Some(p) = path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no config home to write the compositor settings into",
        ));
    };
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
