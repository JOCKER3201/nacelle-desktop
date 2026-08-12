//! User configuration and theme data.
//!
//! Configuration is read as an XDG cascade — the arrangement GTK, Qt,
//! libadwaita and COSMIC all use. The user's own file comes first and
//! the system ones after it, key by key: a key the user never set is
//! answered by the system file, so a distribution or an administrator
//! can change a default without anything being copied into anybody's
//! home directory.
//!
//!   $XDG_CONFIG_HOME/nacelle-desktop/nacelle-desktop.conf  — the user's own (Key=Value)
//!       (~/.config/nacelle-desktop/… when the variable is unset)
//!   $XDG_CONFIG_DIRS/nacelle-desktop/nacelle-desktop.conf  — the system defaults
//!       (/etc/xdg/nacelle-desktop/… when the variable is unset)
//!   <either of those>/shellrc                     — bash startup file, first one found
//!
//! Writes go to the user's directory and nowhere else, and only when
//! the user changes something: the program creates no directory and
//! copies no file at startup.
//!
//! Everything a theme is made of is DATA, not configuration, so it lives
//! under XDG_DATA_HOME:
//!   ~/.local/share/nacelle-desktop/layauts/       — custom layout files (*.layaut)
//!   ~/.local/share/nacelle-desktop/sounds/<set>/  — sound themes, one directory
//!       each; the DIRECTORY NAME is the theme name. Inside, a "meta"
//!       file maps every interface event to the sound file that plays
//!       for it, next to the audio files themselves.
//!
//! EVERY layout is computed from the ACTUAL window size every frame (see
//! src/flex.rs), so resizing or moving the window reflows the interface
//! live. Layout files come in two formats:
//!
//! Flexbox (recommended) — CSS-like columns, same engine as the built-in
//! default (min/max widths, collapse priorities, portrait restack):
//!   units = du          # OUTSIDE any [column]: du (default) = min/max are
//!                       # written at a 1080-line reference and scale with
//!                       # the window height; px = literal device pixels
//!   pad_x = 3.2         # OUTSIDE any [column]: page padding per side, %
//!                       # of the window width (omit = the engine's margin)
//!   [column]            # columns are laid out left to right
//!   basis = 16.4        # preferred width, % of the row (flex-basis)
//!   min = 168           # min-width (du or px, see units)
//!   max = 340           # max-width (omit = unlimited)
//!   grow = 0            # share of leftover space (flex-grow)
//!   collapse = 2        # 1 disappears first when space runs out; 0 = never
//!   gap = 2.5           # vertical gap between panels (weight units)
//!   panel = cpu 26 ref 15.5 min 9.0   # panels top->bottom with height
//!   panel = control 12                # weights and optional ref/min (vh)
//! A panel is named by the addon that draws it — whatever is installed
//! under addons/scripts and addons/plugins, which the program lists at
//! startup. A layout naming an addon this machine does not have simply
//! leaves that panel out.
//!
//! Legacy — "<panel> = x y w h" percentages at the 16:9 reference,
//! re-adapted to the window continuously (edge-anchored transform on
//! landscape, a vertical restack on portrait).
//!
//! In nacelle-desktop.conf the Theme= option picks one of the engine's
//! themes; the Layaut= and Sounds= options name a file from layauts/
//! (without an extension) and a directory from sounds/. Empty values or
//! missing options = defaults built into the code.
//!
//! Variant= is the second half of the colour axis: it names one of the
//! theme's contrast variants — hc, the high-contrast one, is the variant
//! the engine's master ships — and an empty or missing value is the plain
//! theme. It is a key of its own rather than part of Theme= because the
//! two are independent: a variant is an accessibility setting, and liking
//! a colour is not a reason to give one up.
//!
//! A machine with several screens gives each of them a desktop of its
//! own, so Layaut= is only the DEFAULT arrangement. A screen takes a
//! layaut of its own when the file names it by connector:
//!
//!   Layaut=console          # every screen not named below
//!   Layaut[DP-1]=cockpit    # the monitor on DisplayPort 1
//!   Layaut[eDP-1]=panel     # the laptop's own screen
//!
//! The connector — DP-1, HDMI-A-1, eDP-1 — is what the display server
//! calls the socket a screen hangs off, and the program prints it for
//! every screen at startup. It is the only stable name a screen has:
//! the order screens come up in depends on which monitor is switched
//! on first, so a number in that order would name a different screen
//! every morning. Case is not significant, an empty value means "no
//! layaut of its own", and a name no layauts/ file answers to costs
//! that screen nothing but a line in the log — it takes the default.

use crate::widgets::PanelSpec;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};












pub use nacelle::layout::{BoardId, InstanceId, LayoutDef};
use nacelle::assets::AssetRoots;
use nacelle::layout::LayautStore;

/// The widget crates linked into this binary.
///
/// This is the only place in the program where linking a widget in is
/// arranged at all, and it has to exist: a linked-in addon is a SYMBOL
/// in this executable, and no directory scan can find a symbol. What it
/// does NOT do is describe those widgets. Each crate exports its own
/// [`BuiltinWidget`](nacelle::widget::factory::BuiltinWidget) — the
/// name the addon would have as a file, and the very bytes of the
/// `.meta` that ships beside that file — so the list below names
/// CRATES, never widgets: no label, no size, no category and no place
/// in the arrangement of any widget is written down anywhere in this
/// program.
///
/// Everything else comes from the addons directory, exactly as
/// third-party addons do. A machine with nothing installed and nothing
/// linked has no widgets, and says so.
const LINKED: [nacelle::widget::factory::BuiltinWidget; 8] = [
    nacelle_widget_ai::WIDGET,
    nacelle_widget_appcats::WIDGET,
    nacelle_widget_appgrid::WIDGET,
    nacelle_widget_control::WIDGET,
    nacelle_widget_filesystem::WIDGET,
    nacelle_widget_keyboard::WIDGET,
    nacelle_widget_search::WIDGET,
    nacelle_widget_shell::WIDGET,
];

/// The one widget factory of this application: the linked-in crates,
/// plugins honouring NACELLE_SAFE, everything on the XDG search path.
/// Widgets are made through it and the registry is built from it, so
/// the two can never disagree about what exists.
pub fn widget_factory() -> &'static nacelle::widget::factory::WidgetFactory {
    static F: std::sync::OnceLock<nacelle::widget::factory::WidgetFactory> =
        std::sync::OnceLock::new();
    F.get_or_init(|| {
        LINKED
            .into_iter()
            .fold(
                nacelle::widget::factory::WidgetFactory::new(AssetRoots::xdg("nacelle-desktop")),
                |f, w| f.with_builtin(w),
            )
            .plugins_enabled(!crate::plugins::disabled())
    })
}

/// The toolkit's layaut store over this application's XDG roots.
fn store() -> LayautStore {
    LayautStore::new(AssetRoots::xdg("nacelle-desktop"))
}

pub struct Config {
    pub layout: LayoutDef,
}

pub fn load() -> (Config, Option<String>) {
    // The dead Look=/Style= keys retire on sight — before anything
    // reads the layout or the theme (u3 §6.3).
    migrate_look_style_in(
        &config_dir().join(CONF_FILE),
        &AssetRoots::xdg("nacelle-desktop"),
    );
    // The registry must exist before anything parses a layout: panels
    // are resolved by name against it.
    let roots = AssetRoots::xdg("nacelle-desktop");
    migrate_widgets_to_addons(&roots);
    let scanned = nacelle::widget::registry::scan(&roots).len();
    let regs = widget_factory().registry();
    if scanned == 0 {
        eprintln!(
            "nacelle-desktop: no addons installed \u{2014} looked in {}",
            roots.read.iter().map(|d| d.join("addons").display().to_string()).collect::<Vec<_>>().join(", ")
        );
        eprintln!(
            "nacelle-desktop: addons install with `make install` in the \
             nacelle-addons repository"
        );
    }
    // No addons and no linked crates = no widgets. That is a real
    // state of the program, not a fault to be papered over with an
    // invented set: it is a console with nothing to show, exactly as a
    // program with no theme installed is a page with no stylesheet.
    if regs.is_empty() {
        eprintln!(
            "nacelle-desktop: no widgets at all \u{2014} every board will be empty \
             until an addon is installed"
        );
    } else {
        eprintln!("nacelle-desktop: {} widgets", regs.len());
    }
    nacelle::base::set_registry(regs);
    // The theme engine, before anything asks for a colour. Every warning it
    // has about the theme file — an unknown key, a bad value, a cycle — is
    // printed once here rather than swallowed: a theme that is wrong must say
    // so and keep running, which is the engine's own first rule.
    let diags = load_engine_theme();
    for w in &diags.warnings {
        eprintln!("{w}");
    }
    eprintln!(
        "nacelle-desktop: theme '{}' ({} tokens)",
        diags.localised_name(""),
        nacelle::theme::resolved().len()
    );
    let (cfg, warning) = resolve();
    nacelle::base::set_panel_sizes(&cfg.layout.sizes);
    (cfg, warning)
}

/// Layout by name, through the toolkit's store.
fn layaut_by_name(name: &str) -> Option<LayoutDef> {
    store().load(name)
}

/// One-time migration of the pre-addons install layout:
/// `widgets/{board,appgrid,search_and_ai}/<name>/<name>.{rhai,so}` —
/// and the pre-split top level — becomes the flat `addons/scripts/`
/// and `addons/plugins/`. The category a directory used to carry
/// moves into the addon: a script gets a header pragma of its own, a
/// compiled plugin the `<name>.meta` file beside it, and neither
/// depends on the program remembering anything. Only the WRITE root is
/// migrated — a system install is somebody else's file to move — and
/// nothing is ever overwritten: a name already present under addons/
/// keeps its file, the old copy stays where it was, and the
/// collision is said out loud.
fn migrate_widgets_to_addons(roots: &AssetRoots) {
    let old = roots.write.join("widgets");
    if !old.is_dir() {
        return;
    }
    let cats = ["board", "appgrid", "search_and_ai"];
    // Category subdirectories first, then the top level itself (the
    // pre-split arrangement) — the order the old scan walked.
    let mut units: Vec<(std::path::PathBuf, Option<&str>)> = Vec::new();
    for sub in cats {
        if let Ok(rd) = std::fs::read_dir(old.join(sub)) {
            let pragma = if sub == "board" { None } else { Some(sub) };
            units.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).map(|p| (p, pragma)));
        }
    }
    if let Ok(rd) = std::fs::read_dir(&old) {
        units.extend(
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && !cats
                            .iter()
                            .any(|s| p.file_name().map(|n| n == *s).unwrap_or(false))
                })
                .map(|p| (p, None)),
        );
    }
    let (mut moved, mut kept) = (0usize, 0usize);
    for (dir, pragma) in units {
        let Some(name) = dir.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        for (ext, sub) in [("rhai", "scripts"), ("so", "plugins")] {
            let src = dir.join(format!("{name}.{ext}"));
            if !src.is_file() {
                continue;
            }
            let Ok(destdir) = roots.ensure(&format!("addons/{sub}")) else { continue };
            let dest = destdir.join(format!("{name}.{ext}"));
            if dest.exists() {
                eprintln!(
                    "nacelle-desktop: addons/{sub}/{name}.{ext} already exists \u{2014} \
                     old copy kept at {}",
                    src.display()
                );
                kept += 1;
                continue;
            }
            let ok = match pragma {
                // The directory carried the category; the ADDON
                // carries it now — a script in a header pragma of its
                // own, a compiled plugin in the metadata file beside
                // it. Nothing in the program remembers it for them.
                Some(cat) if ext == "rhai" => std::fs::read_to_string(&src)
                    .and_then(|body| std::fs::write(&dest, format!("// category: {cat}\n{body}")))
                    .map(|()| std::fs::remove_file(&src).is_ok())
                    .unwrap_or(false),
                Some(cat) => {
                    let moved = std::fs::rename(&src, &dest).is_ok();
                    let meta = dest.with_extension("meta");
                    if moved && !meta.exists() {
                        let _ = std::fs::write(&meta, format!("category = {cat}\n"));
                    }
                    moved
                }
                None => std::fs::rename(&src, &dest).is_ok(),
            };
            if ok {
                moved += 1;
            }
        }
        // Only an emptied directory disappears: a widget directory
        // holding anything else (notes, assets) stays for its owner.
        let _ = std::fs::remove_dir(&dir);
    }
    for sub in cats {
        let _ = std::fs::remove_dir(old.join(sub));
    }
    let _ = std::fs::remove_dir(&old);
    if moved > 0 || kept > 0 {
        eprintln!(
            "nacelle-desktop: retired the widgets/ layout \u{2014} {moved} addon(s) \
             carried over to addons/ ({kept} kept in place)"
        );
    }
}





















/// The name of the theme the new engine is to load: `Theme=` in
/// nacelle-desktop.conf, or the built-in master when nothing is set.
///
/// A theme name is a bare identifier — the engine refuses a path, because a
/// `[meta] base` that could name `../../etc/passwd` would be a file-read
/// primitive.
pub fn current_engine_theme() -> Option<String> {
    conf_kv()
        .get("Theme")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn set_engine_theme(name: &str) {
    set_conf_kv("Theme", name);
}

/// The contrast variant to select on top of the theme: `Variant=` in
/// nacelle-desktop.conf. `None` — the ordinary case — is the plain theme.
///
/// `hc` is the one the engine's master declares, and every theme resolves it:
/// a theme that declares no `[variant.*]` of its own inherits the master's,
/// so high contrast does not disappear as a side effect of choosing a colour.
pub fn current_engine_variant() -> Option<String> {
    conf_kv()
        .get("Variant")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Writes `Variant=`. `None` writes it EMPTY rather than dropping the line,
/// because an empty value is an explicit off that outranks a system file
/// naming one, while a missing key would inherit it (see [`cascade_kv`]).
///
/// Read here and written elsewhere: the settings screen's contrast switch
/// calls this and then re-applies the configuration, exactly as its theme
/// list already calls [`set_engine_theme`]. Until that switch exists the
/// user writes the line by hand — `allow(dead_code)` says only that, and
/// comes off the day it is called.
#[allow(dead_code)]
pub fn set_engine_variant(name: Option<&str>) {
    set_conf_kv("Variant", name.unwrap_or(""));
}

/// The layout half of the configuration: `Layaut=` names one, and
/// anything missing falls back to the responsive built-in default.
///
/// This is the DEFAULT desktop — the one every screen the
/// configuration does not name by connector shows. See
/// [`screen_layaut`] for the per-screen answer.
fn resolve_layout(warning: &mut Option<String>) -> LayoutDef {
    let lname = current_layaut_name().unwrap_or_else(|| "default".into());
    load_layaut_or_default(&lname, warning).1
}

/// Loads a layaut by name, degrading to the built-in default when it
/// is not installed and saying so. Returns the name actually loaded,
/// because a caller that saves back into "the layaut this screen
/// shows" must not write into a file that does not exist.
fn load_layaut_or_default(name: &str, warning: &mut Option<String>) -> (String, LayoutDef) {
    if let Some(l) = layaut_by_name(name) {
        return (name.to_string(), l);
    }
    eprintln!("nacelle-desktop: layaut '{name}' is not installed");
    *warning = Some(format!(
        "Layaut '{name}' is not installed \u{2014} using the default layaut"
    ));
    ("default".to_string(), layaut_by_name("default").unwrap_or_default())
}

/// The configuration key one screen's layaut is written under:
/// `Layaut[DP-1]`. None when the text is not a connector name — a key
/// nothing could ever match a screen to is not worth writing, and
/// keeping brackets and separators out of it is what keeps the file
/// parseable by the same two rules as every other line.
fn screen_layaut_key(connector: &str) -> Option<String> {
    let c = connector.trim();
    (crate::screens::connector_of(c).as_deref() == Some(c)).then(|| format!("Layaut[{c}]"))
}

/// Every connector→layaut assignment in an already-cascaded
/// configuration map, in connector order.
///
/// Takes the map rather than reading the files, so a test hands it
/// three lines and touches nothing process-wide.
///
/// A value is carried through as written and NOT checked against the
/// installed layauts here: whether it names something real is
/// [`choose_layaut`]'s judgement, which is also the only place that
/// can say so in the log. Dropping it here would lose the sentence.
fn screen_layauts_in(kv: &HashMap<String, String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in kv {
        // `Layaut [DP-1]` reads as `Layaut[DP-1]`: this is a file
        // people type into, and a space before the bracket is not a
        // different intention.
        let Some(inner) = key
            .trim()
            .strip_prefix("Layaut")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('['))
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            continue;
        };
        let inner = inner.trim();
        if crate::screens::connector_of(inner).as_deref() != Some(inner) {
            continue;
        }
        let name = value.trim();
        // An empty value is a value: it is how the user's own file
        // says "this screen has no layaut of its own" over a system
        // file that gave it one, exactly as ColorLut= says "off".
        if name.is_empty() {
            continue;
        }
        out.insert(inner.to_string(), name.to_string());
    }
    out
}

/// What one screen's layaut resolves to, and the one sentence the log
/// gets when the configuration asked for something that is not there.
struct ScreenLayaut {
    name: String,
    note: Option<String>,
}

/// Which layaut a connector takes: the one it is assigned when that
/// layaut is installed, the desktop's default in every other case.
///
/// Pure, and handed everything it judges by — the assignments, the
/// default and the names the store actually holds — because this
/// decision has to be testable on a machine that has no screens, and
/// because it must never be able to name a layaut the store did not
/// list: that is what keeps a hand-written value out of the paths
/// built from it.
fn choose_layaut(
    connector: Option<&str>,
    assigned: &BTreeMap<String, String>,
    default_name: &str,
    installed: &[String],
) -> ScreenLayaut {
    let default = || ScreenLayaut { name: default_name.to_string(), note: None };
    // Case is not significant: RandR says eDP-1 and a user typing
    // edp-1 means that same screen, not a screen the machine lacks.
    let Some((c, want)) = connector.and_then(|c| {
        assigned
            .iter()
            .find(|(k, _)| k.as_str().eq_ignore_ascii_case(c))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }) else {
        return default();
    };
    if installed.iter().any(|n| n == want) {
        return ScreenLayaut { name: want.to_string(), note: None };
    }
    ScreenLayaut {
        name: default_name.to_string(),
        note: Some(format!(
            "screen {c} is assigned layaut '{want}', which is not installed \u{2014} \
             that screen takes the default layaut '{default_name}' instead"
        )),
    }
}

// The per-screen layaut API below — read it, write it, resolve it —
// has no caller in this file. It is answered here and asked
// elsewhere: main.rs hands each screen the layaut its connector is
// assigned, and the settings screen writes the assignments once it
// exists (until then the user writes the line by hand, which is what
// the format is shaped for). `allow(dead_code)` says exactly that,
// and comes off the day each one is called.

/// Every connector→layaut assignment the configuration carries, the
/// user's file laid over the system ones.
#[allow(dead_code)]
pub fn screen_layauts() -> BTreeMap<String, String> {
    screen_layauts_in(&conf_kv())
}

/// The layaut assigned to one connector, if any. Case is not
/// significant.
#[allow(dead_code)]
pub fn layaut_for_connector(connector: &str) -> Option<String> {
    screen_layauts()
        .into_iter()
        .find(|(k, _)| k.as_str().eq_ignore_ascii_case(connector))
        .map(|(_, v)| v)
}

/// Assigns a layaut to a connector. An empty name CLEARS the
/// assignment — written as an empty value rather than deleted, so the
/// user's file also overrules one a system file makes.
#[allow(dead_code)]
pub fn set_layaut_for_connector(connector: &str, name: &str) {
    let Some(key) = screen_layaut_key(connector) else {
        eprintln!(
            "nacelle-desktop: '{connector}' is not a connector name \u{2014} \
             no screen was assigned a layaut"
        );
        return;
    };
    set_conf_kv(&key, name.trim());
}

/// The NAME of the layaut a screen takes, by the connector it hangs
/// off. None connector — a screen the display server would not name —
/// takes the default, and so does a screen nothing was written for.
///
/// Reads the configuration files and lists the layaut store, so it is
/// asked when a screen appears or the configuration changes, never
/// per frame. [`screen_layaut`] is the one that reports a fallback;
/// this one answers quietly, so a caller comparing two screens does
/// not fill the log.
#[allow(dead_code)]
pub fn screen_layaut_name(connector: Option<&str>) -> String {
    choose_layaut(
        connector,
        &screen_layauts(),
        &current_layaut_name().unwrap_or_else(|| "default".into()),
        &list_layauts(),
    )
    .name
}

/// The layaut a screen takes: the name and the layout itself.
///
/// This is the whole of "one screen, one desktop": a screen the
/// configuration names by connector takes that layaut, and every
/// other screen takes the default one. A configuration naming a
/// layaut this machine does not have is a mistake in a file, never a
/// reason not to start — the screen falls back to the default and the
/// log says which screen, which layaut and what it got instead.
#[allow(dead_code)]
pub fn screen_layaut(connector: Option<&str>) -> (String, LayoutDef) {
    let chosen = choose_layaut(
        connector,
        &screen_layauts(),
        &current_layaut_name().unwrap_or_else(|| "default".into()),
        &list_layauts(),
    );
    if let Some(note) = &chosen.note {
        eprintln!("nacelle-desktop: {note}");
    }
    let mut warning = None;
    load_layaut_or_default(&chosen.name, &mut warning)
}

/// What a retired look's directory bundled as its layout: a symlink
/// into the shared layauts/ (named by its target's stem) or an inline
/// `.layaut` file (its text).
enum LookLayaut {
    Linked(String),
    Inline(String),
}

/// The bundled layout of the look directory whose metafile says
/// `Name=<name>`, searching every root. Read by the migration only —
/// nothing resolves through a look any more.
fn look_bundled_layaut(roots: &AssetRoots, name: &str) -> Option<LookLayaut> {
    for dir in roots.dirs("look") {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let d = entry.path();
            if !d.is_dir() {
                continue;
            }
            let Some(meta_text) = read_meta(&d) else { continue };
            if parse_kv(&meta_text).get("Name").map(|n| n.as_str()) != Some(name) {
                continue;
            }
            let p = find_file(&d, "layaut")?;
            return match std::fs::read_link(&p) {
                Ok(target) => target
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| LookLayaut::Linked(s.to_string())),
                Err(_) => std::fs::read_to_string(&p).ok().map(LookLayaut::Inline),
            };
        }
    }
    None
}

/// One-time retirement of the `Look=` and `Style=` keys (u3 §6.3).
///
/// A look bundled a stylesheet and, sometimes, a layout. Stylesheets
/// are not a thing any more, so the layout is the one thing carried
/// over — rescued BEFORE the keys are dropped: a `.layaut` symlink
/// becomes `Layaut=<its target's stem>`, an inline `.layaut` file is
/// copied into the layaut store under the look's name and selected. An
/// explicit `Layaut=` outranks the bundled one and is left alone. The
/// theme is its own axis and nothing is guessed from the look's name;
/// `Theme=default` is written only when `Theme=` is unset. A file
/// without the old keys is left untouched, which is what makes this
/// run-once.
fn migrate_look_style_in(conf: &Path, roots: &AssetRoots) {
    let Ok(text) = std::fs::read_to_string(conf) else { return };
    let kv = parse_kv(&text);
    if !kv.contains_key("Look") && !kv.contains_key("Style") {
        return;
    }
    let look = kv.get("Look").and_then(|s| safe_component(s));
    let style = kv.get("Style").and_then(|s| safe_component(s));
    let layaut_set = kv
        .get("Layaut")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    // The layout the look contributes, if any.
    let mut carried: Option<String> = None;
    if let Some(name) = &look {
        match look_bundled_layaut(roots, name) {
            Some(LookLayaut::Linked(stem)) => carried = Some(stem),
            Some(LookLayaut::Inline(body)) => match roots.ensure("layauts") {
                Ok(dir) => {
                    let dest = dir.join(format!("{name}.layaut"));
                    // Never clobber: a file the user already has under
                    // that name wins, exactly as it would at load time.
                    if dest.exists() || std::fs::write(&dest, body).is_ok() {
                        carried = Some(name.clone());
                    } else {
                        eprintln!(
                            "nacelle-desktop: cannot write {} \u{2014} the look's layaut stays where it is",
                            dest.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("nacelle-desktop: cannot create the layauts directory: {e}")
                }
            },
            None => {}
        }
    }

    /// Sets Key=Value on the lines of the file being rewritten,
    /// preserving everything else — set_conf_kv, minus the filesystem.
    fn set_line(lines: &mut Vec<String>, key: &str, value: &str) {
        let prefix = format!("{key}=");
        for line in lines.iter_mut() {
            if line.trim_start().starts_with(&prefix) {
                *line = format!("{key}={value}");
                return;
            }
        }
        lines.push(format!("{key}={value}"));
    }

    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    lines.retain(|l| {
        let t = l.trim_start();
        !(t.starts_with("Look=") || t.starts_with("Style="))
    });
    if let Some(name) = carried.as_ref().filter(|_| !layaut_set) {
        set_line(&mut lines, "Layaut", name);
    }
    let theme_unset = kv.get("Theme").map(|v| v.trim().is_empty()).unwrap_or(true);
    if theme_unset {
        set_line(&mut lines, "Theme", "default");
    }
    let mut out = lines.join("\n");
    out.push('\n');
    if let Err(e) = std::fs::write(conf, out) {
        eprintln!("nacelle-desktop: cannot write {}: {e}", conf.display());
        return;
    }

    // One line naming what was carried over and what was dropped.
    let fate = match (&carried, layaut_set) {
        (Some(l), false) => format!("layaut '{l}' carried over to Layaut="),
        (Some(l), true) => format!("layaut '{l}' rescued; the explicit Layaut= stays"),
        (None, _) => "no layaut to carry over".to_string(),
    };
    eprintln!(
        "nacelle-desktop: retired Look={} Style={} \u{2014} {}",
        look.as_deref().unwrap_or(""),
        style.as_deref().unwrap_or(""),
        fate
    );
}

/// Resolves the effective configuration.
///
/// Two independent axes. **Colour** comes from the theme engine in the
/// toolkit — `Theme=` selects one of the shipped themes or one installed on the
/// search path, and everything else derives from `default.theme`. **Layout**
/// comes from `Layaut=`.
///
/// The second value is an English warning for the on-screen popup when an
/// element is unavailable for the current screen size.
pub fn resolve() -> (Config, Option<String>) {
    let mut warning: Option<String> = None;
    let layout = resolve_layout(&mut warning);
    // Reload the engine: this runs on every configuration change, and Theme=
    // may be what changed. Parse, cascade, resolve and bake is under 5 ms for
    // the whole catalogue, so re-doing it on a settings click costs nothing a
    // user can perceive — and it is the only way a theme switch is live.
    let diags = load_engine_theme();
    if let Some(first) = diags.warnings.first() {
        warning.get_or_insert_with(|| first.clone());
    }
    (Config { layout }, warning)
}

/// Loads the theme the configuration selects, reporting whatever the engine
/// had to say about it. Always succeeds — a missing or broken theme degrades
/// to the master.
pub fn load_engine_theme() -> std::sync::Arc<nacelle::theme::ThemeDiagnostics> {
    let diags = nacelle::theme::load_with(nacelle::theme::LoadRequest {
        name: current_engine_theme(),
        ..Default::default()
    });
    // Here, on the far side of EVERY load, rather than once at startup: a
    // load rebuilds every sibling and lands on the plain one, so a `Theme=`
    // change would otherwise take high contrast off the screen without
    // anybody asking for it — and a setting that quietly turns itself off is
    // worse than one that was never offered.
    apply_engine_variant();
    diags
}

/// The variant name last refused, so a name that stays wrong costs one line
/// and not one line per settings click: [`resolve`] reloads the engine on
/// every configuration change, and the answer cannot change in between.
static REFUSED_VARIANT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Selects the configured variant on the theme just loaded.
///
/// A name this theme has no sibling for is a sentence in the log and nothing
/// more. Falling back to the plain theme costs the user contrast; refusing to
/// start costs them the desktop, and a typo in a text file may not be the
/// reason a machine does not come up.
fn apply_engine_variant() {
    let want = current_engine_variant();
    // An unset key has nothing to undo: the load already landed on plain.
    let refused = match &want {
        Some(name) if !nacelle::theme::set_variant(Some(name)) => Some(name.clone()),
        _ => None,
    };
    let Ok(mut said) = REFUSED_VARIANT.lock() else { return };
    if *said == refused {
        return;
    }
    if let Some(name) = &refused {
        let declared = list_engine_variants();
        eprintln!(
            "nacelle-desktop: no variant \"{name}\" in this theme (it declares {}) \u{2014} \
             running without one",
            if declared.is_empty() { "none".to_string() } else { declared.join(", ") }
        );
    }
    *said = refused;
}

/// The themes the settings panel offers: the eight compiled into the toolkit
/// plus anything installed on the search path, `default` first.
pub fn list_engine_themes() -> Vec<String> {
    nacelle::theme::available_themes()
}

/// The contrast variants the LOADED theme offers, `hc` being the master's.
///
/// Read off the resolved siblings rather than the file: a sibling is a mood,
/// a variant, or one of each, so what is left after the plain theme, the
/// moods and the combinations is exactly the variants. This is the list a
/// contrast switch offers and the list a refused name is measured against.
pub fn list_engine_variants() -> Vec<String> {
    let moods = nacelle::theme::mood_rules();
    nacelle::theme::siblings()
        .into_iter()
        .filter(|s| s != "plain" && !s.contains('+') && !moods.iter().any(|m| m.name == *s))
        .collect()
}

/// Path of the bash startup file nacelle-desktop hands to the shell:
/// the first `shellrc` on the configuration search path, so a
/// system-wide one works for a user who has none of their own. When
/// nothing is installed this names the user's own path — the file
/// simply does not exist, and the shell starts without it.
pub fn shellrc_path() -> PathBuf {
    config_dirs()
        .into_iter()
        .map(|d| d.join("shellrc"))
        .find(|p| p.is_file())
        .unwrap_or_else(|| config_dir().join("shellrc"))
}

/// Accepts a config value only if it is a single safe path component
/// (no separators, not "..", not absolute) — so Layaut=/Sounds=
/// values joined into data-directory paths cannot escape it.
fn safe_component(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() || n == "." || n == ".." {
        return None;
    }
    if n.contains('/') || n.contains('\\') || n.contains('\0') {
        return None;
    }
    // Must be exactly one normal path component.
    let mut comps = Path::new(n).components();
    match (comps.next(), comps.next()) {
        (Some(std::path::Component::Normal(c)), None) if c == n => Some(n.to_string()),
        _ => None,
    }
}

/// The effective configuration: the user's file laid over the system
/// ones, key by key.
fn conf_kv() -> HashMap<String, String> {
    cascade_kv(&conf_files())
}

/// Every `nacelle-desktop.conf` that takes part, most specific first.
fn conf_files() -> Vec<PathBuf> {
    config_dirs().into_iter().map(|d| d.join(CONF_FILE)).collect()
}

/// Sub-directories named `sub` that exist, in search order.
fn asset_dirs(sub: &str) -> Vec<PathBuf> {
    data_dirs()
        .into_iter()
        .map(|d| d.join(sub))
        .filter(|d| d.is_dir())
        .collect()
}

/// The first `<root>/<sub>/<rel>` that exists, searching the user's
/// directory before the system ones.
fn find_asset(sub: &str, rel: &str) -> Option<PathBuf> {
    data_dirs()
        .into_iter()
        .map(|d| d.join(sub).join(rel))
        .find(|p| p.symlink_metadata().is_ok())
}



/// Name of the main configuration file.
const CONF_FILE: &str = "nacelle-desktop.conf";

















/// Sound theme names — the subdirectories of sounds/ that carry a
/// metafile. The DIRECTORY NAME is the theme name.
pub fn list_sound_themes() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for dir in asset_dirs("sounds") {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_dir() || read_meta(&p).is_none() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else { continue };
            if !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

/// Directory of the sound theme the current configuration selects —
/// Sounds=, or the "default" set when nothing is. None when it names a
/// set that is not installed.
pub fn active_sounds_dir() -> Option<PathBuf> {
    let name = current_sounds_name().unwrap_or_else(|| "default".into());
    find_asset("sounds", &name).filter(|d| d.is_dir())
}

/// Current Sounds= value from nacelle-desktop.conf (if a safe, non-empty name).
pub fn current_sounds_name() -> Option<String> {
    conf_kv().get("Sounds").and_then(|s| safe_component(s))
}

pub fn set_sounds_option(name: &str) {
    set_conf_kv("Sounds", name);
}

pub fn list_layauts() -> Vec<String> {
    store().list()
}

/// Current Layaut= value from nacelle-desktop.conf (if a safe, non-empty name).
pub fn current_layaut_name() -> Option<String> {
    conf_kv().get("Layaut").and_then(|s| safe_component(s))
}

fn font_prefs_for(prefix: &str, min: f32, max: f32) -> (f32, Option<String>, Option<String>) {
    let kv = conf_kv();
    let scale = kv
        .get(&format!("{prefix}FontSize"))
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map(|p| (p / 100.0).clamp(min, max))
        .unwrap_or(1.0);
    let get = |key: String| {
        kv.get(&key)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    (
        scale,
        get(format!("{prefix}FontFamily")),
        get(format!("{prefix}FontWeight")),
    )
}

/// Terminal font preferences: (size scale, family, weight).
pub fn term_font_prefs() -> (f32, Option<String>, Option<String>) {
    font_prefs_for("Term", 0.5, 2.0)
}

/// Interface font preferences: (size scale, family, weight).
pub fn ui_font_prefs() -> (f32, Option<String>, Option<String>) {
    font_prefs_for("UI", 0.30, 1.25)
}

pub fn set_term_font_size(percent: u32) {
    set_conf_kv("TermFontSize", &percent.to_string());
}

pub fn set_term_font_family(name: &str) {
    set_conf_kv("TermFontFamily", name);
}

pub fn set_term_font_weight(name: &str) {
    set_conf_kv("TermFontWeight", name);
}

pub fn set_ui_font_size(percent: u32) {
    set_conf_kv("UIFontSize", &percent.to_string());
}

pub fn set_ui_font_family(name: &str) {
    set_conf_kv("UIFontFamily", name);
}

pub fn set_ui_font_weight(name: &str) {
    set_conf_kv("UIFontWeight", name);
}

pub fn set_layaut_option(name: &str) {
    set_conf_kv("Layaut", name);
}

/// Sound preferences: (master volume 0-100, typing sounds, ambient bed).
/// Everything on by default — a fresh install should be heard.
pub fn sound_prefs() -> (u32, bool, bool) {
    let kv = conf_kv();
    let volume = kv
        .get("SoundVolume")
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.min(100))
        .unwrap_or(100);
    let flag = |key: &str| {
        kv.get(key)
            .map(|v| v.trim() != "0")
            .unwrap_or(true)
    };
    (volume, flag("SoundTyping"), flag("SoundAmbient"))
}

/// The colour pipeline preferences: bit depth of the swapchain, the
/// colour space the program asks the compositor to show it in, and an
/// optional grading LUT (.cube) and ICC profile. All of it is a
/// Wayland-session matter — read, shown and applied only there; every
/// other session ignores the whole group.
pub struct ColorPrefs {
    /// 8, 10, 12 or 16. Twelve rides in sixteen-bit float buffers —
    /// Vulkan has no twelve-bit swapchain format — and what the wire
    /// carries is between the compositor and the display.
    pub depth: u32,
    /// A name from [`COLOR_SPACES`]; "auto" leaves the compositor's
    /// default in place.
    pub space: String,
    /// File name (with extension) under an assets `lut/` directory.
    pub lut: Option<String>,
    /// File name under an assets `icc/` directory.
    pub icc: Option<String>,
}

/// The colour spaces the COLOR view offers, in display order. Names
/// map to the Color Management protocol's named primaries + transfer
/// function pairs in the application.
pub const COLOR_SPACES: [&str; 7] = [
    "auto",
    "srgb",
    "display p3",
    "adobe rgb",
    "bt2020 pq",
    "bt2020 hlg",
    "scrgb linear",
];

pub fn color_prefs() -> ColorPrefs {
    let kv = conf_kv();
    let depth = kv
        .get("ColorDepth")
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|d| matches!(d, 8 | 10 | 12 | 16))
        .unwrap_or(8);
    let space = kv
        .get("ColorSpace")
        .map(|v| v.trim().to_lowercase())
        .filter(|v| COLOR_SPACES.contains(&v.as_str()))
        .unwrap_or_else(|| "auto".to_string());
    let file = |key: &str| {
        kv.get(key)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .and_then(|v| safe_component(&v))
    };
    ColorPrefs { depth, space, lut: file("ColorLut"), icc: file("ColorIcc") }
}

pub fn set_color_depth(bits: u32) {
    set_conf_kv("ColorDepth", &bits.to_string());
}

pub fn set_color_space(space: &str) {
    set_conf_kv("ColorSpace", space);
}

pub fn set_color_lut(name: Option<&str>) {
    set_conf_kv("ColorLut", name.unwrap_or(""));
}

pub fn set_color_icc(name: Option<&str>) {
    set_conf_kv("ColorIcc", name.unwrap_or(""));
}

/// File names (sorted) with one of the extensions, across every assets
/// directory of `sub` — the LUT and ICC pickers list these.
pub fn color_files(sub: &str, exts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for dir in asset_dirs(sub) {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let lower = name.to_lowercase();
                if exts.iter().any(|x| lower.ends_with(x)) && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out
}

/// Absolute path of a named LUT/ICC file, first assets directory wins.
pub fn color_file_path(sub: &str, name: &str) -> Option<std::path::PathBuf> {
    safe_component(name)?;
    find_asset(sub, name)
}

/// Frosted-glass preferences: (blur radius, glass opacity), both in
/// percent. The radius picks how deep the renderer's pyramid goes;
/// the opacity is the glass tint's alpha — below 100 the sharp boards
/// beneath begin to show through the frost.
pub fn blur_prefs() -> (u32, u32) {
    let kv = conf_kv();
    let pct = |key: &str, default: u32| {
        kv.get(key)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map(|v| v.min(100))
            .unwrap_or(default)
    };
    (pct("BlurRadius", 100), pct("BlurOpacity", 100))
}

pub fn set_blur_radius(percent: u32) {
    set_conf_kv("BlurRadius", &percent.min(100).to_string());
}

pub fn set_blur_opacity(percent: u32) {
    set_conf_kv("BlurOpacity", &percent.min(100).to_string());
}

pub fn set_sound_volume(percent: u32) {
    set_conf_kv("SoundVolume", &percent.min(100).to_string());
}

pub fn set_sound_typing(on: bool) {
    set_conf_kv("SoundTyping", if on { "1" } else { "0" });
}

pub fn set_sound_ambient(on: bool) {
    set_conf_kv("SoundAmbient", if on { "1" } else { "0" });
}

/// Grid editor preferences: (snap to grid, columns, rows, widget padding px).
/// How coarse or fine the editor's snap grid may be made. Read here
/// rather than in the settings window, because a value already in the
/// file has to be brought into range too — a grid saved before these
/// were the limits is still a number this program has to draw.
pub const GRID_MIN: u32 = 15;
pub const GRID_MAX: u32 = 100;

pub fn grid_prefs() -> (bool, u32, u32, u32) {
    let kv = conf_kv();
    // The range is the grid's own, applied once below. An older,
    // narrower clamp used to sit here as well, and it won: a grid set
    // to a hundred cells came back as sixty-four after a restart.
    let num = |key: &str, def: u32| {
        kv.get(key)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(def)
    };
    // Snap is opt-in (off by default).
    let snap = kv
        .get("GridSnap")
        .map(|v| v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let pad = kv
        .get("GridPadding")
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(8)
        .min(40);
    let cells = |key: &str| num(key, GRID_MIN).clamp(GRID_MIN, GRID_MAX);
    (snap, cells("GridCols"), cells("GridRows"), pad)
}

pub fn set_grid_snap(on: bool) {
    set_conf_kv("GridSnap", if on { "1" } else { "0" });
}






/// The board's placements BY IDENTITY: the grid editor hands back one
/// entry per instance standing on the board, and the store drops the
/// instances it no longer names. By identity and not by widget, because
/// a board may hold the same widget twice and only the id says which
/// of the two rectangles moved.
pub fn set_board_in_layaut(
    name: &str,
    k: BoardId,
    rects: &[(InstanceId, PanelSpec)],
) -> std::io::Result<()> {
    store().set_board(name, k, rects)
}

pub fn add_board_in_layaut(name: &str, side: i8) -> std::io::Result<()> {
    store().add_board(name, side)
}

pub fn remove_board_in_layaut(name: &str, k: BoardId) -> std::io::Result<()> {
    store().remove_board(name, k)
}

pub fn set_grid_cols(n: u32) {
    set_conf_kv("GridCols", &n.to_string());
}

pub fn set_grid_rows(n: u32) {
    set_conf_kv("GridRows", &n.to_string());
}

pub fn set_grid_padding(n: u32) {
    set_conf_kv("GridPadding", &n.to_string());
}

/// Selects a layout by name.
pub fn select_layaut(name: &str) {
    set_layaut_option(name);
}







/// SAVE AS. `def` is taken by mutable reference because writing a
/// layout down is what turns its COMPOSED placements into saved ones:
/// the caller's copy has to learn the new identities, or it would go on
/// holding ids the file does not have.
pub fn save_layaut_full(
    name: &str,
    def: &mut LayoutDef,
    key: (u32, u32, u32),
) -> std::io::Result<()> {
    store().save_full(name, def, key)
}





/// SAVE. `changes` names the INSTANCES the editor moved — the second
/// terminal the user dragged on his 4K screen is that instance, not
/// "the terminal" — and `def` is the caller's own model, written back
/// materialized (see [`save_layaut_full`]).
pub fn save_layaut_overrides(
    name: &str,
    key: (u32, u32, u32),
    changes: &[(InstanceId, PanelSpec)],
    def: &mut LayoutDef,
) -> std::io::Result<()> {
    store().save_overrides(name, key, changes, def)
}



pub fn stale_screen_section(
    def: &LayoutDef,
    key: (u32, u32, u32),
) -> Option<(usize, usize)> {
    nacelle::layout::stale_screen_section(def, key)
}

pub fn clear_screen_section(name: &str, key: (u32, u32, u32)) -> std::io::Result<()> {
    store().clear_screen_section(name, key)
}

pub fn parse_screen_key(s: &str) -> Option<(u32, u32, u32)> {
    nacelle::layout::layaut::parse_screen_key(s)
}

pub fn print_layaut(def: &LayoutDef) -> String {
    nacelle::layout::layaut::print(def)
}

/// Screen diagonal in inches of the monitor with the given connector
/// name (EDID bytes 21/22, physical size in cm); 0 = unknown.
pub fn monitor_diag_inches(monitor_name: &str) -> u32 {
    // Remembered per connector name. A monitor does not change size
    // while it is plugged in, and this was being asked several times a
    // frame: a directory scan of /sys/class/drm and an EDID read, over
    // two thousand of them in half a minute of running.
    thread_local! {
        static SEEN: std::cell::RefCell<std::collections::HashMap<String, u32>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    if let Some(known) = SEEN.with(|c| c.borrow().get(monitor_name).copied()) {
        return known;
    }
    let inches = monitor_diag_inches_uncached(monitor_name);
    SEEN.with(|c| c.borrow_mut().insert(monitor_name.to_string(), inches));
    inches
}

fn monitor_diag_inches_uncached(monitor_name: &str) -> u32 {
    let connector = monitor_name
        .split_whitespace()
        .next()
        .unwrap_or(monitor_name);
    let suffix = format!("-{connector}");
    let Some(dir) = std::fs::read_dir("/sys/class/drm")
        .ok()
        .and_then(|rd| {
            rd.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().ends_with(&suffix))
                    .unwrap_or(false)
            })
        })
    else {
        return 0;
    };
    let Ok(edid) = std::fs::read(dir.join("edid")) else { return 0 };
    if edid.len() >= 23 {
        let w = edid[21] as f32;
        let h = edid[22] as f32;
        if w > 0.0 && h > 0.0 {
            return ((w * w + h * h).sqrt() / 2.54).round() as u32;
        }
    }
    0
}

/// Sets Key=Value in the USER's nacelle-desktop.conf, preserving the
/// rest of the file. The system files are read-only to the program —
/// what a settings click writes is always the user's own copy, which
/// then outranks them.
///
/// This is also the moment the configuration directory is created.
/// The program makes no directory at startup and installs nothing:
/// what is not installed is simply not offered, and the home
/// directory stays untouched until the user changes something.
fn set_conf_kv(key: &str, value: &str) {
    let path = config_dir().join(CONF_FILE);
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("nacelle-desktop: cannot create {}: {e}", dir.display());
            return;
        }
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let out = set_kv_in_text(&text, key, value);
    if let Err(e) = std::fs::write(&path, out) {
        eprintln!("nacelle-desktop: cannot write {}: {e}", path.display());
    }
}

/// `Key=Value` set on the TEXT of a configuration file: the line is
/// replaced where it stands, and appended when there is none. Every
/// other line — the comments, the order, keys this program has never
/// heard of — survives verbatim.
///
/// The pure half of [`set_conf_kv`], and what the tests exercise: the
/// environment is shared by tests running in parallel, so a test that
/// wrote through it would be testing the other tests too.
fn set_kv_in_text(text: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let prefix = format!("{key}=");
    let mut replaced = false;
    for line in lines.iter_mut() {
        if line.trim_start().starts_with(&prefix) {
            *line = format!("{key}={value}");
            replaced = true;
            break;
        }
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The one configuration directory anything is ever WRITTEN to:
/// `$XDG_CONFIG_HOME/nacelle-desktop`, or `~/.config/nacelle-desktop`.
fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("nacelle-desktop");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("nacelle-desktop")
}

/// Every directory the configuration is READ from, most specific
/// first: the user's own, then the system ones from `XDG_CONFIG_DIRS`
/// (or `/etc/xdg` when it is unset).
///
/// The counterpart of [`data_dirs`] for configuration, and the reason
/// a package can ship defaults: they are read where they are
/// installed, never copied to the user.
fn config_dirs() -> Vec<PathBuf> {
    config_search_path(
        config_dir(),
        std::env::var("XDG_CONFIG_DIRS").ok().as_deref(),
    )
}

/// [`config_dirs`] without the environment: the user's directory
/// first, then `system` split on ':' and joined with the application
/// name, duplicates dropped. An unset or empty value means the
/// standard `/etc/xdg`, as the XDG base directory specification says.
fn config_search_path(user: PathBuf, system: Option<&str>) -> Vec<PathBuf> {
    let mut out = vec![user];
    let system = system.filter(|v| !v.is_empty()).unwrap_or("/etc/xdg");
    for base in system.split(':').filter(|b| !b.is_empty()) {
        let dir = PathBuf::from(base).join("nacelle-desktop");
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// Data directory: ~/.local/share/nacelle-desktop. Holds everything a theme is
/// made of (look/, style/, layauts/, sounds/) — those are data, not
/// configuration, so they belong under XDG_DATA_HOME while nacelle-desktop.conf
/// stays in the config directory.
fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("nacelle-desktop");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local").join("share").join("nacelle-desktop")
}

/// Every directory assets are READ from, most specific first: the
/// user's own, then the system ones from XDG_DATA_DIRS (or the two
/// standard prefixes when it is unset).
///
/// This is what makes `sudo make install` and a distribution package
/// mean something, while a user install still shadows both: the first
/// directory holding a given name wins, and nothing has to be copied
/// anywhere for that to work.
fn data_dirs() -> Vec<PathBuf> {
    let mut out = vec![data_dir()];
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    for base in system.split(':').filter(|b| !b.is_empty()) {
        let dir = PathBuf::from(base).join("nacelle-desktop");
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// Merges Key=Value files given MOST SPECIFIC FIRST: an earlier file
/// wins key by key, and a key only a later file has is inherited. A
/// file that does not exist contributes nothing, which is the normal
/// case on both ends — a machine with no system defaults and a user
/// who has never changed a setting are both perfectly ordinary.
///
/// An empty value is a value, not an absence: `ColorLut=` in the
/// user's file is how the settings panel says "off", and it has to
/// beat a system file that names one.
///
/// Takes its paths rather than reading the environment, so a test
/// hands it two temporary files and no process-wide state is touched.
fn cascade_kv(paths: &[PathBuf]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for path in paths.iter().rev() {
        if let Ok(text) = std::fs::read_to_string(path) {
            out.extend(parse_kv(&text));
        }
    }
    out
}

/// Parser for Key=Value files (# and ; comments).
fn parse_kv(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

/// Metafile: a file named "meta" or with the ".meta" extension.
fn read_meta(dir: &Path) -> Option<String> {
    let exact = dir.join("meta");
    if exact.is_file() {
        return std::fs::read_to_string(exact).ok();
    }
    find_file(dir, "meta").and_then(|p| std::fs::read_to_string(p).ok())
}

fn find_file(dir: &Path, ext: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(ext))
                    .unwrap_or(false)
        })
}




#[cfg(test)]
mod tests {
    // A widget KIND: only the tests still name one directly — the
    // interface below speaks in instance identities.
    use crate::widgets::{LayoutMode, Panel};

    fn test_store(dir: &std::path::Path) -> nacelle::layout::LayautStore {
        nacelle::layout::LayautStore::new(nacelle::assets::AssetRoots::new(
            vec![dir.to_path_buf()],
            dir.to_path_buf(),
        ))
    }

    use super::*;

    /// The path a click in the settings panel actually takes: write Theme=,
    /// re-resolve, and the colours the whole interface draws with are the new
    /// theme's. This is the mechanism the user sees as "switching the theme",
    /// and it was broken once already — the panel wrote `Style=`, which the
    /// resolver had stopped reading.
    #[test]
    fn selecting_a_theme_changes_the_colours_the_program_draws_with() {
        // This test SELECTS themes in a process-wide engine; nothing
        // that reads one may run beside it (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        let dir = std::env::temp_dir().join(format!("nacelle-theme-switch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Deliberately NOT created: the program makes no configuration
        // directory at startup, so the first settings click is what
        // has to bring it into being.
        let _env = env_lock();
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        let colour_of = |name: &str| {
            set_engine_theme(name);
            assert_eq!(current_engine_theme().as_deref(), Some(name));
            // resolve() is what reloads the engine on a settings click.
            let (_cfg, _) = resolve();
            nacelle::theme::resolved().color(
                nacelle::theme::id("accent.primary")
                    .expect("the master declares accent.primary"),
            )
        };

        let crimson = colour_of("crimson");
        assert!(
            dir.join("nacelle-desktop").join(CONF_FILE).is_file(),
            "the first settings change must create the user's configuration file"
        );
        let azure = colour_of("azure");
        let pure = colour_of("pure");

        // Each is its own hue, and each is the hue its reference image shows.
        assert!(crimson.r > crimson.b + 0.2, "crimson accent is not red: {crimson:?}");
        assert!(azure.b > azure.r + 0.2, "azure accent is not blue: {azure:?}");
        assert!(pure.g > pure.r + 0.2, "pure accent is not green: {pure:?}");
        // And switching really moves the value, rather than returning a cached
        // theme from the first load.
        assert!(crimson.r != azure.r, "the accent did not change at all");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A configuration directory this test alone writes into, empty to
    /// start with — the program creates it on the first setting written.
    fn variant_conf_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nacelle-variant-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Hands the process-wide engine back the way it was found. These tests
    /// SELECT a sibling in it, and a test running later that reads a colour
    /// must not be reading this one's high-contrast answer.
    fn restore_plain(dir: &Path) {
        nacelle::theme::set_variant(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The global edge weight, which `[variant.hc]` raises from
    /// `stroke.hair` to `stroke.regular` — one number that says which
    /// sibling the program is actually drawing from.
    fn edge_width() -> f32 {
        nacelle::theme::resolved().px(
            nacelle::theme::id("border.edge.width")
                .expect("the master declares border.edge.width"),
        )
    }

    /// `Variant=hc` in the configuration and the engine draws the
    /// high-contrast sibling. It has been resolved, baked and selectable
    /// since the engine was written; nothing in this program ever asked for
    /// it, so the headline accessibility feature could not be turned on.
    #[test]
    fn the_variant_the_configuration_names_is_the_one_the_engine_draws() {
        // Selects in a process-wide engine; nothing that reads one may run
        // beside it (see `theme_test_lock`).
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        let _env = env_lock();
        let dir = variant_conf_dir("named");
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        set_engine_theme("default");
        let (_cfg, _) = resolve();
        assert_eq!(nacelle::theme::current_variant(), None, "nothing asked for one yet");
        let plain = edge_width();

        set_engine_variant(Some("hc"));
        let (_cfg, _) = resolve();
        assert_eq!(nacelle::theme::current_variant().as_deref(), Some("hc"));
        // …and it is the SIBLING being drawn from, not a name held somewhere:
        // the high-contrast edge is a whole stroke rung heavier.
        assert!(
            edge_width() > plain,
            "the high-contrast edge is not heavier: {} vs {plain}",
            edge_width()
        );

        restore_plain(&dir);
    }

    /// A variant no theme declares is a typo in a text file, and a typo in a
    /// text file may not be the reason the desktop does not come up: the
    /// plain theme keeps drawing and the log carries the sentence.
    #[test]
    fn a_variant_no_theme_declares_leaves_the_plain_one_running() {
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        let _env = env_lock();
        let dir = variant_conf_dir("unknown");
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        set_engine_theme("default");
        set_engine_variant(Some("dinner"));
        // Reaching this at all is half the assertion: `resolve()` is what
        // startup runs, and it returned.
        let (_cfg, _) = resolve();
        assert_eq!(nacelle::theme::current_variant(), None, "an invented variant was selected");
        assert!(!nacelle::theme::resolved().is_empty(), "the theme did not load at all");
        assert!(edge_width() > 0.0, "the plain theme is not drawing");
        // And the sentence in the log can name the one that would have
        // worked: the master declares exactly one variant, and the moods it
        // declares beside it are not variants.
        assert_eq!(list_engine_variants(), vec!["hc".to_string()]);

        restore_plain(&dir);
    }

    /// High contrast is an ACCESSIBILITY setting, not a decoration, so
    /// changing the theme may not take it away. It only survives because the
    /// variant is re-selected on the far side of every load: a load rebuilds
    /// every sibling and lands on the plain one.
    #[test]
    fn switching_the_theme_keeps_the_high_contrast_variant() {
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        let _env = env_lock();
        let dir = variant_conf_dir("kept");
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        set_engine_variant(Some("hc"));
        // None of the shipped themes declares a `[variant.*]` of its own, so
        // each one inherits the master's — which is the property that makes
        // "keep the variant" answerable rather than a coin toss.
        for name in ["crimson", "azure", "default"] {
            set_engine_theme(name);
            let (_cfg, _) = resolve();
            assert_eq!(
                nacelle::theme::current_variant().as_deref(),
                Some("hc"),
                "theme '{name}' dropped the high-contrast variant"
            );
        }

        restore_plain(&dir);
    }

    /// SAVE AS writes the full base recording its screen; SAVE on the
    /// base's screen rewrites the base, SAVE on other screens stores
    /// only the changes in their sections; everything else is preserved.
    #[test]
    fn safe_component_blocks_traversal() {
        fixture_registry();
        assert!(safe_component("tron").is_some());
        assert!(safe_component("my-layaut_2").is_some());
        assert!(safe_component("../../etc/passwd").is_none());
        assert!(safe_component("..").is_none());
        assert!(safe_component("a/b").is_none());
        assert!(safe_component("/abs").is_none());
        assert!(safe_component("").is_none());
        assert!(safe_component("x\\y").is_none());
    }

    /// The XDG cascade every other toolkit implements: the user's file
    /// wins key by key, the system file answers everything it does not
    /// mention, and a missing file on either end is ordinary rather
    /// than an error. Nothing is copied for this to work — which is
    /// the whole point of reading a search path instead of seeding a
    /// home directory at install time.
    ///
    /// Hermetic: explicit paths, no process environment.
    #[test]
    fn the_user_file_wins_and_the_system_file_fills_the_gaps() {
        fixture_registry();
        let base =
            std::env::temp_dir().join(format!("nacelle-conf-cascade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let system = base.join("etc/xdg/nacelle-desktop");
        let user = base.join("config/nacelle-desktop");
        std::fs::create_dir_all(&system).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(
            system.join(CONF_FILE),
            "# the distribution's defaults\nTheme=azure\nLayaut=console\nColorLut=studio\n",
        )
        .unwrap();
        std::fs::write(user.join(CONF_FILE), "Theme=crimson\nColorLut=\n").unwrap();

        let paths = vec![user.join(CONF_FILE), system.join(CONF_FILE)];
        let kv = cascade_kv(&paths);
        assert_eq!(
            kv.get("Theme").map(String::as_str),
            Some("crimson"),
            "the user's own value must win"
        );
        assert_eq!(
            kv.get("Layaut").map(String::as_str),
            Some("console"),
            "a key the user never set comes from the system file"
        );
        assert_eq!(
            kv.get("ColorLut").map(String::as_str),
            Some(""),
            "an empty user value is an explicit off, not an absence"
        );

        // A user who has never changed a setting has no file at all,
        // and the system defaults stand on their own.
        std::fs::remove_file(user.join(CONF_FILE)).unwrap();
        let kv = cascade_kv(&paths);
        assert_eq!(kv.get("Theme").map(String::as_str), Some("azure"));
        assert_eq!(kv.get("ColorLut").map(String::as_str), Some("studio"));

        // And with nothing installed anywhere the program is left with
        // what is built into it, rather than with an error.
        assert!(cascade_kv(&[base.join("nowhere.conf")]).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The search path itself: the user's directory first — it is also
    /// the only one written to — then XDG_CONFIG_DIRS in its own
    /// order, `/etc/xdg` when it is unset or empty, and no directory
    /// twice.
    #[test]
    fn the_configuration_search_path_follows_xdg() {
        fixture_registry();
        let user = PathBuf::from("/home/somebody/.config/nacelle-desktop");
        let etc = PathBuf::from("/etc/xdg/nacelle-desktop");
        for unset in [None, Some(""), Some("/etc/xdg")] {
            assert_eq!(
                config_search_path(user.clone(), unset),
                vec![user.clone(), etc.clone()],
                "{unset:?} must resolve to the standard /etc/xdg"
            );
        }
        assert_eq!(
            config_search_path(user.clone(), Some("/opt/site/etc:/etc/xdg:/opt/site/etc")),
            vec![
                user.clone(),
                PathBuf::from("/opt/site/etc/nacelle-desktop"),
                etc,
            ],
            "the order of the variable is kept and duplicates drop"
        );
        assert_eq!(
            config_search_path(user.clone(), Some("/opt/site/etc"))[0],
            user,
            "the write target is always the head of the read path"
        );
    }

    /// The registry the tests resolve names against.
    ///
    /// There is no built-in table to fall back on, so the tests have to
    /// bring a registry — and a hand-written one here would BE such a
    /// table: a shipped addon could lose its metadata and not a single
    /// test would notice. So this stages the real thing instead: the
    /// crates linked into this binary, plus the addons repository next
    /// door copied into an `addons/scripts` tree and scanned exactly as
    /// an installed one is. What the tests see is what `make install`
    /// gives a machine.
    ///
    /// Staged once, and called by EVERY test in this module — the
    /// process-wide registry is fixed by the first call *or the first
    /// read*, so one test resolving a layout before the staging would
    /// freeze it empty for all the others.
    /// Serialises every test that writes an XDG variable.
    ///
    /// `std::env::set_var` is PROCESS-wide while `cargo test` runs its
    /// tests on many threads: one test pointing XDG_CONFIG_HOME at its
    /// own directory silently redirected another test's `resolve()`
    /// half way through, and the theme-switch test read somebody else's
    /// configuration and saw the wrong accent. Nothing about that was
    /// visible in the failure — the colour was simply not the one the
    /// theme names. Per-PID directories are not enough; the variable
    /// itself is the shared thing.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        // A poisoned lock only means some other test panicked while
        // holding it; the variable it set is being overwritten anyway.
        L.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn fixture_registry() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            // Per PROCESS: two test binaries running at once must not
            // stage over each other's tree mid-scan.
            let stage = std::env::temp_dir()
                .join(format!("nacelle-desktop-registry-fixture-{}", std::process::id()));
            let scripts = stage.join("addons").join("scripts");
            let _ = std::fs::remove_dir_all(&stage);
            std::fs::create_dir_all(&scripts).expect("the fixture tree must be writable");
            let shipped = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../nacelle-addons/scripts"
            ));
            let rd = std::fs::read_dir(shipped)
                .expect("the nacelle-addons repository must sit next to this one");
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                    let name = path.file_name().expect("a file has a name");
                    std::fs::copy(&path, scripts.join(name)).expect("stage the addon");
                }
            }
            let roots = AssetRoots::new(vec![stage.clone()], stage);
            let factory = LINKED.into_iter().fold(
                nacelle::widget::factory::WidgetFactory::new(roots),
                |f, w| f.with_builtin(w),
            );
            nacelle::base::set_registry(factory.registry());
        });
    }

    /// Test widgets, resolved by name against the registry the same way
    /// the rest of the program does.
    fn wp(name: &str) -> Panel {
        fixture_registry();
        Panel::from_name(name).expect("the staged addons must hold this widget")
    }

    /// The registry is built from the directory and from nothing else:
    /// the widget IS its file — `<name>.rhai` or `<name>.so` — its
    /// stem is its name, a file of another extension is not an addon,
    /// and what the program knows about a widget is what the widget
    /// declared. No name in this program's own code puts anything in
    /// the registry, so a directory with nothing in it yields nothing.
    #[test]
    fn widget_registry_reads_the_directory() {
        fixture_registry();
        let base = std::env::temp_dir().join("nacelle-desktop-widget-registry-test");
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("addons");
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        std::fs::write(
            root.join("scripts").join("mywidget.rhai"),
            "// label: MY WIDGET\n// ref_h: 12.5\nfn draw() { [] }",
        )
        .unwrap();
        // A stray file of the wrong extension is not an addon.
        std::fs::write(root.join("scripts").join("notes.txt"), "Label=NOPE\n").unwrap();
        // A compiled widget is its library, described by the metadata
        // file beside it — and a library WITHOUT one still counts.
        std::fs::write(root.join("plugins").join("meter.so"), b"not really").unwrap();
        std::fs::write(root.join("plugins").join("meter.meta"), "min_h = 3.5\n").unwrap();
        std::fs::write(root.join("plugins").join("bare.so"), b"not really").unwrap();

        let defs = nacelle::widget::registry::scan(&nacelle::assets::AssetRoots::new(vec![base.clone()], base.clone()));
        // Sorted, so panel order never depends on the filesystem.
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["bare", "meter", "mywidget"], "only addon files count");
        let d = |n: &str| defs.iter().find(|d| d.name == n).unwrap();
        assert_eq!(d("mywidget").label, "MY WIDGET", "the script's own pragma");
        assert_eq!(d("mywidget").ref_h_vh, 12.5);
        assert_eq!(d("mywidget").min_h_vh, 6.0, "what it did not name it kept");
        assert_eq!(d("meter").min_h_vh, 3.5, "the .meta file beside the library");
        assert_eq!(d("meter").label, "METER", "a name in capitals is the default");
        assert_eq!(d("bare").label, "BARE", "no metadata is not no addon");
        assert_eq!((d("bare").ref_h_vh, d("bare").min_h_vh), (10.0, 6.0));

        let empty = nacelle::assets::AssetRoots::new(vec![base.join("nope")], base.join("nope"));
        assert!(nacelle::widget::registry::scan(&empty).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The other half of the same rule: a widget crate LINKED into this
    /// binary is registered by the crate, not by this file. Every entry
    /// of [`LINKED`] carries its own name and its own metadata, and the
    /// registry the factory builds is those plus the directory — never
    /// a table written here.
    #[test]
    fn linked_widget_crates_register_themselves() {
        fixture_registry();
        let empty = std::env::temp_dir().join("nacelle-desktop-linked-crates-test");
        let _ = std::fs::remove_dir_all(&empty);
        let factory = LINKED.into_iter().fold(
            nacelle::widget::factory::WidgetFactory::new(AssetRoots::new(
                vec![empty.clone()],
                empty,
            )),
            |f, w| f.with_builtin(w),
        );
        let defs = factory.registry();
        assert_eq!(defs.len(), LINKED.len(), "an empty tree offers the linked crates");
        for w in LINKED {
            let d = defs
                .iter()
                .find(|d| d.name == w.name)
                .unwrap_or_else(|| panic!("{} must register itself", w.name));
            // Its description came from the crate, not from a default:
            // a core widget whose `.meta` went missing or empty would
            // come out bare, and this is what catches it.
            let bare = nacelle::widget::registry::bare_def(w.name.to_string());
            assert!(
                d.label != bare.label
                    || d.ref_h_vh != bare.ref_h_vh
                    || d.min_h_vh != bare.min_h_vh,
                "{} declares nothing about itself",
                w.name
            );
        }
    }

    /// The one-time migration: the pre-addons tree moves into
    /// `addons/scripts` and `addons/plugins`, a non-board category
    /// becomes the script's own header pragma, nothing is overwritten,
    /// and a second run finds nothing to do.
    #[test]
    fn the_widgets_layout_retires_into_addons() {
        fixture_registry();
        let base = std::env::temp_dir().join("nacelle-desktop-widget-migration-test");
        let _ = std::fs::remove_dir_all(&base);
        let old = base.join("widgets");
        std::fs::create_dir_all(old.join("board/clock")).unwrap();
        std::fs::write(old.join("board/clock/clock.rhai"), "fn draw() { [] }").unwrap();
        std::fs::create_dir_all(old.join("appgrid/launcher")).unwrap();
        std::fs::write(old.join("appgrid/launcher/launcher.rhai"), "fn draw() { [] }").unwrap();
        // The pre-split top level, and a compiled widget beside it.
        std::fs::create_dir_all(old.join("meter")).unwrap();
        std::fs::write(old.join("meter/meter.so"), b"not really").unwrap();
        // A compiled widget whose DIRECTORY carried the category: it
        // has nowhere to write a pragma, so the metadata file beside
        // the library is where the category goes.
        std::fs::create_dir_all(old.join("appgrid/tiles")).unwrap();
        std::fs::write(old.join("appgrid/tiles/tiles.so"), b"not really").unwrap();
        // A name already installed under addons/ must survive intact.
        let roots = nacelle::assets::AssetRoots::new(vec![base.clone()], base.clone());
        std::fs::create_dir_all(base.join("addons/scripts")).unwrap();
        std::fs::write(base.join("addons/scripts/clock.rhai"), "KEEP").unwrap();

        migrate_widgets_to_addons(&roots);

        assert_eq!(
            std::fs::read_to_string(base.join("addons/scripts/clock.rhai")).unwrap(),
            "KEEP",
            "an existing addon is never overwritten"
        );
        assert!(old.join("board/clock/clock.rhai").is_file(), "the colliding copy stays put");
        assert_eq!(
            std::fs::read_to_string(base.join("addons/scripts/launcher.rhai")).unwrap(),
            "// category: appgrid\nfn draw() { [] }",
            "the directory's category becomes the script's pragma"
        );
        assert!(base.join("addons/plugins/meter.so").is_file());
        assert!(
            !base.join("addons/plugins/meter.meta").exists(),
            "a board widget names no category, so it needs no metadata file"
        );
        assert!(base.join("addons/plugins/tiles.so").is_file());
        assert_eq!(
            std::fs::read_to_string(base.join("addons/plugins/tiles.meta")).unwrap(),
            "category = appgrid\n",
            "the directory's category becomes the plugin's metadata file"
        );
        assert!(!old.join("appgrid").exists(), "emptied category directories disappear");
        migrate_widgets_to_addons(&roots);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Every addon this project ships describes itself, and the
    /// program reads what it wrote. A shipped script that lost its
    /// header pragmas, a core crate whose `.meta` went missing, a
    /// typo in either — any of them comes out bare, with the file's
    /// name in capitals and the standard heights. That is the right
    /// answer for a stranger's addon and the wrong one for ours, and
    /// nothing else in the program would notice: there is no table
    /// left to fall back on.
    #[test]
    fn every_shipped_addon_declares_itself() {
        fixture_registry();
        let defs = nacelle::base::registry();
        assert!(!defs.is_empty(), "the staged tree must hold the shipped addons");
        for d in defs {
            let bare = nacelle::widget::registry::bare_def(d.name.clone());
            assert!(
                d.ref_h_vh != bare.ref_h_vh || d.min_h_vh != bare.min_h_vh,
                "{} was read without any metadata of its own",
                d.name
            );
            // And every board addon says WHERE it wants to stand. The
            // shipped arrangement is composed from these declarations
            // and from nothing else, so one that named no column would
            // be placed by the engine's fallback instead of by its
            // author — and the console layaut would stop matching.
            if d.category == nacelle::base::WidgetCategory::Board {
                assert_ne!(
                    d.slot,
                    nacelle::base::PanelSlot::Auto,
                    "{} names no column of the arrangement",
                    d.name
                );
            }
        }
    }

    /// The pinned edges of the shipped arrangement, and the widget the
    /// editor may not remove, are DECLARATIONS — counted here rather
    /// than named, because naming one would be the very table this
    /// program does not keep. One addon opens the work column, one
    /// closes it, one is the bar, and exactly one says that switching
    /// it off would leave the user no way back.
    #[test]
    fn the_shipped_addons_declare_the_arrangements_fixed_points() {
        fixture_registry();
        let count = |a: nacelle::base::PanelAnchor| {
            nacelle::base::registry().iter().filter(|d| d.anchor == a).count()
        };
        assert_eq!(count(nacelle::base::PanelAnchor::Top), 1);
        assert_eq!(count(nacelle::base::PanelAnchor::Bottom), 1);
        assert_eq!(count(nacelle::base::PanelAnchor::Bar), 1);
        assert_eq!(
            nacelle::base::registry().iter().filter(|d| d.essential).count(),
            1,
            "exactly one shipped addon carries the way back"
        );
    }

    /// Boards travel inside the .layaut file: parsed from [board k]
    /// sections, normalised to sit next to each other, and preserved by
    /// every path that rewrites the file for other reasons.
    #[test]
    fn boards_live_in_the_layaut_file() {
        fixture_registry();
        let text = "screen = 1920x1080@27\nclock = 1.00 2.00 10.00 10.00\n\n\
                    [1280x720@7]\nclock = 5.00 5.00 20.00 20.00\n\n\
                    [board 4]\ncpu = 10.00 10.00 30.00 30.00\n\n[board -3]\n\n[board 0 2]\nmemory = 1.00 1.00 20.00 20.00\n";
        let def = nacelle::layout::layaut::parse(text, "t");
        assert_eq!(def.boards.len(), 3);
        // A board parsed from rect lines places by rectangle, and the
        // rectangles are on its INSTANCES — the board itself only says
        // that it reads them.
        let rects_board = |bd: &nacelle::layout::BoardDef| {
            assert!(
                matches!(bd.base, LayoutMode::Rects),
                "a board written as rect lines places by rectangle"
            );
        };
        let rect_on = |k: BoardId, w: Panel| {
            def.board_instances(k).into_iter().find(|i| i.widget == w).and_then(|i| i.rect)
        };
        // Gaps close: the only positive board is board 1, the only
        // negative is board -1, wherever the file put them.
        let b1 = def.boards.iter().find(|(k, _)| *k == (1, 0)).expect("board 1");
        assert!(def.boards.iter().any(|(k, _)| *k == (-1, 0)), "board -1");
        rects_board(&b1.1);
        assert!((rect_on((1, 0), wp("cpu")).expect("cpu stands on board 1").w - 30.0).abs() < 0.01);
        // The empty one is a place with nothing on it. Where the old
        // per-widget table answered "off screen" for a widget it did
        // not place, an instance list simply has no entry for it.
        let bm = def.boards.iter().find(|(k, _)| *k == (-1, 0)).unwrap();
        // The vertical arm renumbers just the same: [board 0 2] with
        // nothing above it is the first board below home.
        assert!(def.boards.iter().any(|(k, _)| *k == (0, 1)), "board (0,1)");
        rects_board(&bm.1);
        assert!(def.board_instances((-1, 0)).is_empty(), "board -1 holds nothing");
        // The instances went with their boards through normalisation.
        assert!(rect_on((0, 1), wp("memory")).is_some(), "memory rode down with its board");

        // A full-base rewrite (SAVE on the base's screen) keeps them.
        let dir = std::env::temp_dir().join("nacelle-desktop-boards-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("layauts")).unwrap();
        std::fs::write(dir.join("layauts/t.layaut"), text).unwrap();
        // What the desktop actually saves: the model it loaded, with
        // one INSTANCE moved — the rectangle the editor dragged.
        let mut full = nacelle::layout::layaut::parse(text, "t");
        let clock = full.instances.first_of(wp("clock")).expect("the clock is placed");
        full.instances.set_rect(clock, Some(PanelSpec { x: 7.0, y: 8.0, w: 11.0, h: 12.0 }));
        test_store(&dir).save_full("t", &mut full, (1920, 1080, 27)).unwrap();
        let after = std::fs::read_to_string(dir.join("layauts/t.layaut")).unwrap();
        let def2 = nacelle::layout::layaut::parse(&after, "t");
        assert_eq!(def2.boards.len(), 3, "boards must survive a base rewrite");
        assert!(after.contains("[1280x720@7]"), "overrides must survive too");

        // And an overrides rewrite keeps them just the same.
        test_store(&dir).save_overrides("t", (2560, 1440, 32), &[], &mut full).unwrap();
        let def3 =
            nacelle::layout::layaut::parse(&std::fs::read_to_string(dir.join("layauts/t.layaut")).unwrap(), "t");
        assert_eq!(def3.boards.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A widget installed in the user's directory shadows a system one
    /// of the same name: the search order is what makes `make install`
    /// as a user override a distribution package without either copy
    /// being touched.
    #[test]
    fn a_user_widget_shadows_a_system_one() {
        fixture_registry();
        let root = std::env::temp_dir().join("nacelle-desktop-widget-shadow-test");
        let _ = std::fs::remove_dir_all(&root);
        let user = root.join("user").join("addons").join("scripts");
        let system = root.join("system").join("addons").join("scripts");
        // The same name in both, plus one only the system has.
        for (base, script) in [(&user, "fn draw() { [] }"), (&system, "fn draw() { [1] }")] {
            std::fs::create_dir_all(base).unwrap();
            std::fs::write(base.join("clock.rhai"), script).unwrap();
        }
        std::fs::write(system.join("uptime.rhai"), "fn draw() { [] }").unwrap();

        let defs = nacelle::widget::registry::scan(&nacelle::assets::AssetRoots::new(
            vec![root.join("user"), root.join("system")],
            root.join("user"),
        ));
        assert_eq!(defs.len(), 2, "the shadowed copy must not appear twice");
        let clock = defs.iter().find(|d| d.name == "clock").unwrap();
        assert_eq!(
            std::fs::read_to_string(user.join("clock.rhai")).unwrap(),
            "fn draw() { [] }"
        );
        assert_eq!(clock.label, "CLOCK");
        // A widget only the system directory has is still offered.
        assert!(defs.iter().any(|d| d.name == "uptime"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// u1 §5.4: the shipped console.layaut IS the built-in default —
    /// columns, weights, min/max, collapse, gap, units and the sizes
    /// table, compared field by field so the two cannot drift. The
    /// built-in stays the single source (no default.layaut may ever
    /// ship — it would shadow flex::default_flex for anyone with the
    /// themes installed); console.layaut is the same arrangement as a
    /// normal selectable file.
    #[test]
    fn shipped_console_layaut_matches_builtin_default() {
        fixture_registry();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nacelle-themes/layauts/console.layaut"
        );
        let text = std::fs::read_to_string(path)
            .expect("console.layaut must ship in nacelle-themes/layauts/ next to this repo");
        let def = nacelle::layout::layaut::parse(&text, "console");
        let LayoutMode::Custom(fl) = &def.base else {
            panic!("console.layaut must be a flexbox layout");
        };
        let want = nacelle::flex::default_flex();
        assert_eq!(fl.units_px, want.units_px, "units");
        assert_eq!(fl.pad_x, want.pad_x, "pad_x");
        assert_eq!(fl.columns.len(), want.columns.len(), "column count");
        for (i, (a, b)) in fl.columns.iter().zip(&want.columns).enumerate() {
            assert_eq!(a.basis, b.basis, "column {i} basis");
            assert_eq!(a.min, b.min, "column {i} min");
            assert!(
                a.max == b.max || (!a.max.is_finite() && !b.max.is_finite()),
                "column {i} max: {} vs {}",
                a.max,
                b.max
            );
            assert_eq!(a.grow, b.grow, "column {i} grow");
            assert_eq!(a.collapse, b.collapse, "column {i} collapse");
            assert_eq!(a.gap, b.gap, "column {i} gap");
            // Widget and weight, entry by entry — NOT the instance
            // ids: the file's placements carry saved identities and
            // the composed arrangement carries generated ones, so two
            // arrangements are the same arrangement when they stack
            // the same widgets in the same order at the same shares.
            let entries = |c: &nacelle::FlexColumn| -> Vec<(Panel, f32)> {
                c.panels.iter().map(|it| (it.widget, it.weight)).collect()
            };
            assert_eq!(entries(a), entries(b), "column {i} panels/weights");
        }
        // The sizes table: same panels, same reference and minimum.
        let as_map = |v: &[(Panel, f32, f32)]| -> std::collections::HashMap<Panel, (f32, f32)> {
            v.iter().map(|(p, r, m)| (*p, (*r, *m))).collect()
        };
        assert_eq!(
            as_map(&def.sizes),
            as_map(&nacelle::flex::builtin_sizes()),
            "the ref/min table of console.layaut must equal flex::builtin_sizes()"
        );
        // And the built-in default itself must carry that table when it
        // is selected by name (u1 §5.4's layaut_by_name change). Only
        // checkable when no default.layaut file shadows the built-in on
        // the machine running the tests — a user's own saved default is
        // ALLOWED to shadow it, which is the very reason no such file
        // may ever ship.
        // Hermetic: an empty store, immune to this machine's files and
        // to the env other tests mutate in parallel.
        let empty = std::env::temp_dir().join("nacelle-console-test-empty");
        let hermetic = nacelle::layout::LayautStore::new(nacelle::assets::AssetRoots::new(
            Vec::new(),
            empty,
        ));
        let builtin = hermetic.load("default").expect("the default always resolves");
        assert_eq!(as_map(&builtin.sizes), as_map(&nacelle::flex::builtin_sizes()));
    }

    /// u1 §5.5 (3): no widget is on two boards, over the shipped
    /// layauts, and each of them places all twelve.
    #[test]
    fn shipped_layauts_place_every_widget_exactly_once() {
        fixture_registry();
        for name in ["console", "instrument"] {
            let path = format!(
                "{}/../nacelle-themes/layauts/{name}.layaut",
                env!("CARGO_MANIFEST_DIR")
            );
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{name}.layaut must ship: {e}"));
            let def = nacelle::layout::layaut::parse(&text, name);
            let LayoutMode::Custom(fl) = &def.base else {
                panic!("{name}.layaut must be a flexbox layout");
            };
            // A layout MAY now place a widget twice — that is what an
            // instance list is for — but the SHIPPED arrangements are
            // the reference ones and hold exactly one of each.
            let mut seen = std::collections::HashSet::new();
            for c in &fl.columns {
                for it in &c.panels {
                    assert!(
                        seen.insert(it.widget),
                        "{} appears twice in {name}",
                        it.widget.name()
                    );
                }
            }
            // And one column entry per instance the file placed, so the
            // count above really is the count of placements.
            assert_eq!(
                fl.columns.iter().map(|c| c.panels.len()).sum::<usize>(),
                def.instances.len(),
                "{name}.layaut must place each of its instances in a column"
            );
            // Every BOARD widget: a shipped board layaut cannot place a
            // widget whose home is a fixture, and the fixtures ship
            // empty on purpose — the launcher pair is offered when the
            // APPGRID board is edited, not laid out here.
            let board_widgets = nacelle::base::registry()
                .iter()
                .filter(|d| d.category == nacelle::base::WidgetCategory::Board)
                .count();
            assert_eq!(
                seen.len(),
                board_widgets,
                "{name}.layaut must place every registered board widget"
            );
        }
    }

    /// The flexbox parser now records INSTANCES as it goes, so a caller
    /// hands it a board and a list to fill. These tests only care about
    /// the columns it returns, so they give it a scratch list and drop
    /// it — the instances are exercised by the toolkit's own tests.
    fn parse_flex_scratch(
        src: &str,
    ) -> Option<(nacelle::FlexLayaut, Vec<(nacelle::Panel, f32, f32)>)> {
        let mut insts = nacelle::layout::InstanceList::new();
        nacelle::layout::layaut::parse_flex_into(src, (0, 0), &mut insts)
    }

    /// The units and pad_x keys live OUTSIDE any [column]; a board
    /// section holding [column] lines parses as a flexbox board and
    /// round-trips through the write path.
    #[test]
    fn units_pad_and_flex_boards_parse_and_roundtrip() {
        fixture_registry();
        let (fl, _) = parse_flex_scratch(
            "units = px\npad_x = 3.2\n[column]\nbasis = 20\npanel = clock 7\n",
        )
        .expect("parses");
        assert!(fl.units_px);
        assert_eq!(fl.pad_x, Some(3.2));
        // Omitting both keeps the defaults.
        let (fl2, _) = parse_flex_scratch("[column]\npanel = clock 7\n").unwrap();
        assert!(!fl2.units_px);
        assert_eq!(fl2.pad_x, None);

        // A flexbox board section: parsed as Custom, written back as
        // columns, parsed again identically.
        let text = "clock = 1.00 2.00 10.00 10.00\n\n[board 1]\n[column]\nbasis = 30\npanel = cpu 10 ref 12 min 8\n";
        let def = nacelle::layout::layaut::parse(text, "t");
        assert_eq!(def.boards.len(), 1);
        let (_, bd) = &def.boards[0];
        let LayoutMode::Custom(bfl) = &bd.base else {
            panic!("a board with [column] lines must be flexbox");
        };
        assert_eq!(bfl.columns.len(), 1);
        assert_eq!(bd.sizes.len(), 1);
        let mut out = String::new();
        // The serializer takes the layout's instances too: a board's
        // rectangles are on them, and a flexbox column names them.
        nacelle::layout::layaut::serialize_boards(&mut out, &def.boards, &def.instances);
        let def2 = nacelle::layout::layaut::parse(&format!("clock = 1 2 10 10\n{out}"), "t");
        assert_eq!(def2.boards.len(), 1);
        assert!(matches!(def2.boards[0].1.base, LayoutMode::Custom(_)));
        assert_eq!(def2.boards[0].1.sizes.len(), 1);
    }

    /// clear_screen_section removes exactly one [WxH@D] section: the
    /// base, the other screens and the boards survive.
    #[test]
    fn clear_screen_section_removes_only_its_screen() {
        fixture_registry();
        let dir = std::env::temp_dir().join("nacelle-desktop-clear-screen-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _env = env_lock();
        std::env::set_var("XDG_DATA_HOME", &dir);
        let ldir = dir.join("nacelle-desktop").join("layauts");
        std::fs::create_dir_all(&ldir).unwrap();
        let text = "screen = 1920x1080@27\nclock = 1.00 2.00 10.00 10.00\n\n\
                    [1280x720@7]\nclock = 5.00 5.00 20.00 20.00\n\n\
                    [2560x1440@32]\nshell = 10.00 10.00 50.00 50.00\n\n\
                    [board 1]\ncpu = 10.00 10.00 30.00 30.00\n";
        std::fs::write(ldir.join("t.layaut"), text).unwrap();

        clear_screen_section("t", (1280, 720, 7)).unwrap();
        let after = std::fs::read_to_string(ldir.join("t.layaut")).unwrap();
        let def = nacelle::layout::layaut::parse(&after, "t");
        assert!(def.pick((1280, 720, 7)).is_none(), "the section must be gone");
        assert!(def.pick((2560, 1440, 32)).is_some(), "other screens stay");
        assert!(after.contains("clock = 1.00 2.00 10.00 10.00"), "base stays");
        assert_eq!(def.boards.len(), 1, "boards stay");

        // A stale pinned section is one that names fewer panels than
        // the registry places.
        let def3 = nacelle::layout::layaut::parse(
            "screen = 1920x1080@27\n\n[1920x1080@27]\nclock = 1 1 10 10\n",
            "t",
        );
        assert!(stale_screen_section(&def3, (1920, 1080, 27)).is_some());
        assert!(stale_screen_section(&def3, (1280, 720, 7)).is_none());
        std::env::remove_var("XDG_DATA_HOME");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sizes are a layout property: a .layaut names them next to the
    /// placement, and a widget directory carries none.
    #[test]
    fn a_layout_carries_the_panel_sizes() {
        fixture_registry();
        let (fl, sizes) = parse_flex_scratch(
            "[column]\n\
             basis = 20\n\
             panel = clock 7.0 ref 9.5 min 4.0\n\
             panel = cpu 15.0\n",
        )
        .expect("the column should parse");
        assert_eq!(fl.columns.len(), 1);
        assert_eq!(fl.columns[0].panels.len(), 2);
        // Only the panel that named sizes contributes any.
        assert_eq!(sizes.len(), 1);
        assert_eq!(sizes[0].0, wp("clock"));
        assert_eq!(sizes[0].1, 9.5);
        assert_eq!(sizes[0].2, 4.0);

        // Installing them changes what the panel reports, and a panel
        // the layout said nothing about keeps its default.
        nacelle::base::set_panel_sizes(&sizes);
        assert_eq!(wp("clock").ref_h_vh(), 9.5);
        assert_eq!(wp("clock").min_h_vh(), 4.0);
        let cpu_default = nacelle::base::default_sizes()[wp("cpu").idx()];
        assert_eq!(wp("cpu").ref_h_vh(), cpu_default.0);

        // Back to the defaults, so the other tests see a clean table.
        nacelle::base::set_panel_sizes(&[]);
        assert_eq!(wp("clock").ref_h_vh(), nacelle::base::default_sizes()[wp("clock").idx()].0);
    }

    #[test]
    fn overrides_roundtrip() {
        fixture_registry();
        let name = "unittest-roundtrip";
        let dir = std::env::temp_dir().join("nacelle-desktop-overrides-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layauts").join(format!("{name}.layaut"));

        // What the grid editor hands the store: one INSTANCE per placed
        // widget, each carrying the rectangle it was dragged to. Every
        // save below names those identities — a screen section moves
        // THIS rectangle, not "the filesystem panel", which is the whole
        // point of the instance model.
        let mut full = LayoutDef::from_base(LayoutMode::Rects);
        let clock = full.instances.add(
            wp("clock"),
            (0, 0),
            Some(PanelSpec { x: 1.0, y: 2.0, w: 10.0, h: 10.0 }),
        );
        full.instances.add(
            wp("shell"),
            (0, 0),
            Some(PanelSpec { x: 20.0, y: 2.0, w: 60.0, h: 60.0 }),
        );
        let fs = full.instances.add(
            wp("filesystem"),
            (0, 0),
            Some(PanelSpec { x: 1.0, y: 30.0, w: 20.0, h: 20.0 }),
        );
        let kb = full.instances.add(
            wp("keyboard"),
            (0, 0),
            Some(PanelSpec { x: 1.0, y: 60.0, w: 90.0, h: 20.0 }),
        );
        // The screen this base was authored on: SAVE rewrites the base
        // there and writes a [WxH@D] section on any other.
        full.base_screen = Some((2560, 1440, 32));

        // SAVE AS on a 2560x1440 32" screen: the full base.
        test_store(&dir).save_full(name, &mut full, (2560, 1440, 32)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("screen = 2560x1440@32"));
        assert!(text.contains(&format!("clock#{clock} = 1.00 2.00 10.00 10.00")));

        // SAVE on the SAME screen: the base itself is rewritten in full.
        let mut full2 = full.clone();
        full2
            .instances
            .set_rect(clock, Some(PanelSpec { x: 3.0, y: 4.0, w: 11.0, h: 11.0 }));
        test_store(&dir).save_overrides(name, (2560, 1440, 32), &[], &mut full2).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!("clock#{clock} = 3.00 4.00 11.00 11.00")));
        assert!(!text.contains("[2560x1440@32]"));

        // First save on a DIFFERENT screen: one changed instance -> section.
        let fs_spec = PanelSpec { x: 30.0, y: 10.0, w: 20.0, h: 40.0 };
        test_store(&dir).save_overrides(
            name,
            (1920, 1080, 27),
            &[(fs, fs_spec)],
            &mut full2,
        )
        .unwrap();
        // Another screen: another instance.
        let kb_spec = PanelSpec { x: 5.0, y: 60.0, w: 90.0, h: 30.0 };
        test_store(&dir).save_overrides(
            name,
            (1280, 720, 7),
            &[(kb, kb_spec)],
            &mut full2,
        )
        .unwrap();
        // First screen again: update the same instance.
        let fs_spec2 = PanelSpec { x: 40.0, y: 12.0, w: 22.0, h: 44.0 };
        test_store(&dir).save_overrides(
            name,
            (1920, 1080, 27),
            &[(fs, fs_spec2)],
            &mut full2,
        )
        .unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let def = nacelle::layout::layaut::parse(&text, name);
        // Base preserved (rewritten clock position from the same-screen SAVE).
        assert!(text.contains(&format!("clock#{clock} = 3.00 4.00 11.00 11.00")));
        assert!(matches!(def.base, LayoutMode::Rects));
        // The identities survived the round trip through the file, so
        // the sections still point at the instances they were written
        // for.
        assert_eq!(def.instances.get(fs).map(|i| i.widget), Some(wp("filesystem")));
        // Two sections, exact matches only.
        assert_eq!(def.overrides.len(), 2);
        assert!(def.pick((2560, 1440, 27)).is_none());
        let big = def.pick((1920, 1080, 27)).unwrap();
        assert_eq!(big.rects.len(), 1);
        let (id, ps) = &big.rects[0];
        assert_eq!(*id, fs);
        assert!((ps.x - 40.0).abs() < 0.01 && (ps.h - 44.0).abs() < 0.01);
        let small = def.pick((1280, 720, 7)).unwrap();
        assert_eq!(small.rects.len(), 1);
        assert_eq!(small.rects[0].0, kb);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// u3 §6.3: a configuration carrying the retired Look=/Style= keys
    /// migrates once, at startup. The look's inline .layaut — the one
    /// thing a user could lose — is copied into the layaut store under
    /// the look's name and selected as Layaut=; the dead keys leave the
    /// file; Theme=default is written because nothing was set; every
    /// other key survives verbatim. Hermetic: explicit paths and an
    /// explicit AssetRoots, no environment (the env is shared between
    /// parallel tests and races).
    #[test]
    fn look_and_style_retire_carrying_the_bundled_layaut() {
        fixture_registry();
        let dir = std::env::temp_dir()
            .join(format!("nacelle-look-migration-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let data = dir.join("data");
        let lookdir = data.join("look").join("retro");
        std::fs::create_dir_all(&lookdir).unwrap();
        std::fs::write(lookdir.join("meta"), "Name=Retro\n").unwrap();
        std::fs::write(
            lookdir.join("retro.layaut"),
            "clock = 1.00 2.00 10.00 10.00\n",
        )
        .unwrap();
        let conf = dir.join("nacelle-desktop.conf");
        std::fs::write(&conf, "# kept\nLook=Retro\nStyle=neon\nSoundVolume=40\n").unwrap();
        let roots = nacelle::assets::AssetRoots::new(vec![data.clone()], data.clone());

        migrate_look_style_in(&conf, &roots);

        let text = std::fs::read_to_string(&conf).unwrap();
        let kv = parse_kv(&text);
        assert!(
            !kv.contains_key("Look") && !kv.contains_key("Style"),
            "the dead keys must leave the file: {text}"
        );
        assert_eq!(kv.get("Layaut").map(String::as_str), Some("Retro"));
        assert_eq!(kv.get("Theme").map(String::as_str), Some("default"));
        assert_eq!(kv.get("SoundVolume").map(String::as_str), Some("40"));
        assert!(text.contains("# kept"), "comments survive the rewrite");
        // The rescued layaut is installed: the store loads it by name.
        let def = test_store(&data)
            .load("Retro")
            .expect("the look's layaut must be rescued into layauts/");
        // Rectangles, and the one the look carried: the rescued file is
        // the arrangement itself, not an empty placeholder under its
        // name.
        assert!(matches!(def.base, LayoutMode::Rects));
        assert_eq!(def.instances.count_of(wp("clock")), 1);

        // Run-once: with the keys gone the file is left untouched.
        let before = std::fs::read_to_string(&conf).unwrap();
        migrate_look_style_in(&conf, &roots);
        assert_eq!(before, std::fs::read_to_string(&conf).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The symlink half of u3 §6.3 step 2, plus its step 3: a look whose
    /// .layaut is a symlink contributes only its target's stem — nothing
    /// is copied — and an explicit Layaut= outranks the bundled layout
    /// and is left alone.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_look_layaut_yields_its_stem_and_an_explicit_choice_wins() {
        fixture_registry();
        let dir = std::env::temp_dir()
            .join(format!("nacelle-look-symlink-migration-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let data = dir.join("data");
        let lookdir = data.join("look").join("tron");
        std::fs::create_dir_all(&lookdir).unwrap();
        std::fs::write(lookdir.join("meta"), "Name=Tron\n").unwrap();
        // The shared layout the look linked to, installed where the
        // link points; a dangling symlink bundles nothing, exactly as
        // it always did.
        std::fs::create_dir_all(data.join("layauts")).unwrap();
        std::fs::write(
            data.join("layauts").join("neon.layaut"),
            "clock = 1.00 2.00 10.00 10.00\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            "../../layauts/neon.layaut",
            lookdir.join("tron.layaut"),
        )
        .unwrap();
        let roots = nacelle::assets::AssetRoots::new(vec![data.clone()], data.clone());

        // Nothing else set: the stem becomes the selection.
        let conf = dir.join("a.conf");
        std::fs::write(&conf, "Look=Tron\n").unwrap();
        migrate_look_style_in(&conf, &roots);
        let kv = parse_kv(&std::fs::read_to_string(&conf).unwrap());
        assert_eq!(kv.get("Layaut").map(String::as_str), Some("neon"));
        assert!(
            !data.join("layauts").join("Tron.layaut").exists(),
            "a symlinked layaut is named, never copied"
        );

        // An explicit Layaut= already in the file stays what it is.
        let conf2 = dir.join("b.conf");
        std::fs::write(&conf2, "Look=Tron\nLayaut=mine\n").unwrap();
        migrate_look_style_in(&conf2, &roots);
        let kv2 = parse_kv(&std::fs::read_to_string(&conf2).unwrap());
        assert_eq!(kv2.get("Layaut").map(String::as_str), Some("mine"));
        assert!(!kv2.contains_key("Look"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The layauts a hand-written file assigns, and what each screen
    /// then takes. Hermetic: an explicit configuration text and an
    /// explicit list of installed layauts, so this says what the rule
    /// is rather than what this machine's screens happen to be.
    #[test]
    fn every_screen_takes_the_layaut_its_connector_is_assigned() {
        fixture_registry();
        let kv = parse_kv(
            "# the desktop\n\
             Layaut=console\n\
             Layaut[DP-1]=cockpit\n\
             Layaut [eDP-1] = panel\n\
             Layaut[HDMI-A-1]=\n\
             Layaut[Dell Inc.]=nonsense\n\
             Theme=default\n",
        );
        let assigned = screen_layauts_in(&kv);
        assert_eq!(
            assigned.get("DP-1").map(String::as_str),
            Some("cockpit"),
            "the connector in the brackets names the screen"
        );
        assert_eq!(
            assigned.get("eDP-1").map(String::as_str),
            Some("panel"),
            "a file people type into forgives a space before the bracket and around the ="
        );
        assert!(
            !assigned.contains_key("HDMI-A-1"),
            "an empty value is 'no layaut of its own', not a layaut called ''"
        );
        assert!(
            assigned.keys().all(|k| k != "Dell Inc."),
            "a make and model names no screen and cannot be a key: {assigned:?}"
        );
        assert_eq!(
            kv.get("Layaut").map(String::as_str),
            Some("console"),
            "the per-screen keys leave the default one alone"
        );

        let installed = [
            "default".to_string(),
            "console".to_string(),
            "cockpit".to_string(),
            "panel".to_string(),
        ];
        for (connector, want) in [
            (Some("DP-1"), "cockpit"),
            (Some("eDP-1"), "panel"),
            // Assigned nothing, named nothing, and no name at all: all
            // three are the default desktop.
            (Some("HDMI-A-1"), "console"),
            (Some("DP-3"), "console"),
            (None, "console"),
        ] {
            let got = choose_layaut(connector, &assigned, "console", &installed);
            assert_eq!(got.name, want, "screen {connector:?} takes '{want}'");
            assert!(got.note.is_none(), "nothing to report for {connector:?}");
        }
        // The user typed the connector in another case than RandR says
        // it; it is the same socket and the same screen.
        assert_eq!(
            choose_layaut(Some("edp-1"), &assigned, "console", &installed).name,
            "panel"
        );
    }

    /// Writing an assignment, and taking it back. The file is a
    /// user-editable one, so what matters as much as the value is that
    /// everything else in it comes out untouched.
    #[test]
    fn an_assignment_is_written_beside_the_rest_of_the_file() {
        fixture_registry();
        let before = "# my desktop\nTheme=crimson\nLayaut=console\n";
        let text = set_kv_in_text(
            before,
            &screen_layaut_key("DP-1").expect("DP-1 is a connector"),
            "cockpit",
        );
        let kv = parse_kv(&text);
        assert_eq!(
            screen_layauts_in(&kv).get("DP-1").map(String::as_str),
            Some("cockpit"),
            "what was written must read back: {text}"
        );
        assert_eq!(kv.get("Layaut").map(String::as_str), Some("console"),
            "the default layaut is a different key and must not be touched");
        assert_eq!(kv.get("Theme").map(String::as_str), Some("crimson"));
        assert!(text.contains("# my desktop"), "comments survive: {text}");

        // Assigning again replaces the line rather than adding a second.
        let text2 = set_kv_in_text(&text, "Layaut[DP-1]", "hangar");
        assert_eq!(text2.matches("Layaut[DP-1]").count(), 1, "one line per screen: {text2}");
        assert_eq!(
            screen_layauts_in(&parse_kv(&text2)).get("DP-1").map(String::as_str),
            Some("hangar")
        );

        // Clearing writes an empty value: the assignment is gone, and
        // the line stays to overrule a system file that makes one.
        let text3 = set_kv_in_text(&text2, "Layaut[DP-1]", "");
        assert!(text3.contains("Layaut[DP-1]="), "the key stays as an explicit off: {text3}");
        assert!(screen_layauts_in(&parse_kv(&text3)).is_empty());

        // A key nothing could match a screen to is never written.
        assert!(screen_layaut_key("HDMI-A-1").is_some());
        for bad in ["", "Dell Inc. U2720Q", "DP-1]", "screen 2"] {
            assert!(screen_layaut_key(bad).is_none(), "'{bad}' must not become a key");
        }
    }

    /// A screen assigned a layaut this machine does not have. The
    /// desktop starts, that screen shows the default, and the log gets
    /// one sentence naming the screen, the layaut and what it took
    /// instead — a mistake in a file is not a reason not to start.
    #[test]
    fn an_assignment_to_a_layaut_that_is_not_installed_falls_back_to_the_default() {
        fixture_registry();
        let assigned = screen_layauts_in(&parse_kv("Layaut[DP-1]=cockpit\n"));
        let installed = ["default".to_string(), "console".to_string()];
        let got = choose_layaut(Some("DP-1"), &assigned, "console", &installed);
        assert_eq!(got.name, "console", "the screen falls back to the default layaut");
        let note = got.note.expect("a fallback must say so");
        assert!(note.contains("DP-1"), "the sentence names the screen: {note}");
        assert!(note.contains("cockpit"), "and the layaut that is missing: {note}");
        assert!(note.contains("console"), "and what it took instead: {note}");

        // The same rule keeps a hand-written value out of the paths
        // built from it: only a name the store listed is ever chosen.
        let evil = screen_layauts_in(&parse_kv("Layaut[DP-1]=../../etc/passwd\n"));
        assert_eq!(choose_layaut(Some("DP-1"), &evil, "console", &installed).name, "console");
    }

    /// The point of keying screens by connector: which monitor comes
    /// up first is not a property of anything. The same two screens
    /// surveyed in either order take the same two layauts, and a
    /// position in the list would have swapped them.
    #[test]
    fn an_assignment_survives_the_screens_coming_up_in_another_order() {
        fixture_registry();
        let assigned = screen_layauts_in(&parse_kv(
            "Layaut[DP-1]=cockpit\nLayaut[HDMI-A-1]=hangar\n",
        ));
        let installed = [
            "default".to_string(),
            "console".to_string(),
            "cockpit".to_string(),
            "hangar".to_string(),
        ];
        let survey = |order: [&str; 2]| -> Vec<String> {
            order
                .iter()
                .map(|c| choose_layaut(Some(c), &assigned, "console", &installed).name)
                .collect()
        };
        let monday = survey(["DP-1", "HDMI-A-1"]);
        let tuesday = survey(["HDMI-A-1", "DP-1"]);
        assert_eq!(monday, vec!["cockpit".to_string(), "hangar".to_string()]);
        assert_eq!(
            tuesday,
            vec!["hangar".to_string(), "cockpit".to_string()],
            "each screen keeps its own layaut; only the order of the list changed"
        );
        assert_ne!(monday, tuesday, "the two layauts differ, so the check means something");
    }
}
