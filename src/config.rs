//! User configuration and theme data.
//!
//! Configuration (XDG_CONFIG_HOME):
//!   ~/.config/nacelle-desktop/nacelle-desktop.conf        — main configuration (Key=Value)
//!   ~/.config/nacelle-desktop/shellrc             — generated bash startup file
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
//! Panels: clock, sysinfo, uptime, hardware, cpu, memory, processes,
//! shell, network, filesystem, keyboard, control.
//!
//! Legacy — "<panel> = x y w h" percentages at the 16:9 reference,
//! re-adapted to the window continuously (edge-anchored transform on
//! landscape, a vertical restack on portrait).
//!
//! In nacelle-desktop.conf the Theme= option picks one of the engine's
//! themes; the Layaut= and Sounds= options name a file from layauts/
//! (without an extension) and a directory from sounds/. Empty values or
//! missing options = defaults built into the code.

use crate::widgets::{LayoutSpec, Panel, PanelSpec};
use std::collections::HashMap;
use std::path::{Path, PathBuf};












pub use nacelle::layout::{board_key, BoardId, LayoutDef, ResOverride};
use nacelle::assets::AssetRoots;
use nacelle::layout::LayautStore;

/// The one widget factory of this application: the four core widgets
/// linked in, plugins honouring NACELLE_SAFE, everything on the XDG
/// search path. Widgets are made through it and the registry is built
/// from it, so the two can never disagree about what exists.
pub fn widget_factory() -> &'static nacelle::widget::factory::WidgetFactory {
    static F: std::sync::OnceLock<nacelle::widget::factory::WidgetFactory> =
        std::sync::OnceLock::new();
    F.get_or_init(|| {
        nacelle::widget::factory::WidgetFactory::new(AssetRoots::xdg("nacelle-desktop"))
            .with_builtin("control", nacelle_widget_control::builtin_attach)
            .with_builtin("filesystem", nacelle_widget_filesystem::builtin_attach)
            .with_builtin("keyboard", nacelle_widget_keyboard::builtin_attach)
            .with_builtin("shell", nacelle_widget_shell::builtin_attach)
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
    init_tree(&config_dir());
    // The dead Look=/Style= keys retire on sight — before anything
    // reads the layout or the theme (u3 §6.3).
    migrate_look_style_in(
        &config_dir().join(CONF_FILE),
        &AssetRoots::xdg("nacelle-desktop"),
    );
    // The registry must exist before anything parses a layout: panels
    // are resolved by name against it.
    let roots = AssetRoots::xdg("nacelle-desktop");
    let scanned = nacelle::widget::registry::scan(&roots).len();
    let regs = widget_factory().registry();
    if scanned == 0 {
        eprintln!(
            "nacelle-desktop: no widgets installed \u{2014} running on the built-in set; looked in {}",
            roots.read.iter().map(|d| d.join("widgets").display().to_string()).collect::<Vec<_>>().join(", ")
        );
        eprintln!(
            "nacelle-desktop: the rest install with `make install` in the \
             nacelle-widgets repository"
        );
    }
    eprintln!("nacelle-desktop: {} widgets", regs.len());
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

/// The layout half of the configuration: `Layaut=` names one, and
/// anything missing falls back to the responsive built-in default.
fn resolve_layout(warning: &mut Option<String>) -> LayoutDef {
    let lname = current_layaut_name().unwrap_or_else(|| "default".into());
    match layaut_by_name(&lname) {
        Some(l) => l,
        None => {
            eprintln!("nacelle-desktop: layaut '{lname}' is not installed");
            *warning = Some(format!(
                "Layaut '{lname}' is not installed \u{2014} using the default layaut"
            ));
            layaut_by_name("default").unwrap_or_default()
        }
    }
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
    nacelle::theme::load_with(nacelle::theme::LoadRequest {
        name: current_engine_theme(),
        ..Default::default()
    })
}

/// The themes the settings panel offers: the eight compiled into the toolkit
/// plus anything installed on the search path, `default` first.
pub fn list_engine_themes() -> Vec<String> {
    nacelle::theme::available_themes()
}

/// Path of the bash startup file generated by nacelle-desktop.
pub fn shellrc_path() -> PathBuf {
    config_dir().join("shellrc")
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

fn conf_kv() -> HashMap<String, String> {
    parse_kv(&std::fs::read_to_string(config_dir().join(CONF_FILE)).unwrap_or_default())
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






pub fn set_board_in_layaut(
    name: &str,
    k: BoardId,
    spec: &LayoutSpec,
) -> std::io::Result<()> {
    store().set_board(name, k, spec)
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







pub fn save_layaut_full(
    name: &str,
    spec: &LayoutSpec,
    key: (u32, u32, u32),
) -> std::io::Result<()> {
    store().save_full(name, spec, key)
}





pub fn save_layaut_overrides(
    name: &str,
    key: (u32, u32, u32),
    changes: &[(Panel, PanelSpec)],
    full: &LayoutSpec,
) -> std::io::Result<()> {
    store().save_overrides(name, key, changes, full)
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

/// Sets Key=Value in nacelle-desktop.conf, preserving the rest of the file.
fn set_conf_kv(key: &str, value: &str) {
    let path = config_dir().join(CONF_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut replaced = false;
    for line in lines.iter_mut() {
        let t = line.trim_start();
        if t.starts_with(&format!("{key}=")) {
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
    if let Err(e) = std::fs::write(&path, out) {
        eprintln!("nacelle-desktop: cannot write {}: {e}", path.display());
    }
}

fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("nacelle-desktop");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("nacelle-desktop")
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

/// Makes sure the user's own directories exist.
///
/// That is all the program does to the filesystem at startup. It
/// installs nothing and generates nothing: the looks, styles, sound
/// themes, layouts and widgets are installed from their own
/// repositories, and what is not installed is simply not offered. These
/// two directories are created because the program WRITES to them — the
/// layout editor saves here, and so does every settings change.
fn init_tree(config: &Path) {
    for d in [config, &data_dir().join("layauts")] {
        if let Err(e) = std::fs::create_dir_all(d) {
            eprintln!("nacelle-desktop: cannot create {}: {e}", d.display());
        }
    }
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
    use crate::widgets::LayoutMode;

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
        let dir = std::env::temp_dir().join(format!("nacelle-theme-switch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nacelle-desktop")).unwrap();
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

    /// SAVE AS writes the full base recording its screen; SAVE on the
    /// base's screen rewrites the base, SAVE on other screens stores
    /// only the changes in their sections; everything else is preserved.
    #[test]
    fn safe_component_blocks_traversal() {
        assert!(safe_component("tron").is_some());
        assert!(safe_component("my-layaut_2").is_some());
        assert!(safe_component("../../etc/passwd").is_none());
        assert!(safe_component("..").is_none());
        assert!(safe_component("a/b").is_none());
        assert!(safe_component("/abs").is_none());
        assert!(safe_component("").is_none());
        assert!(safe_component("x\\y").is_none());
    }

    /// Test widgets, resolved by name against the registry the same way
    /// the rest of the program does.
    fn wp(name: &str) -> Panel {
        Panel::from_name(name).expect("built-in widget must be registered")
    }

    /// The registry is built from the directory: the widget IS its
    /// file — `<name>.rhai` or `<name>.so` — its directory name is its
    /// name, and a directory holding neither is not a widget.
    #[test]
    fn widget_registry_reads_the_directory() {
        let base = std::env::temp_dir().join("nacelle-desktop-widget-registry-test");
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("widgets");
        std::fs::create_dir_all(root.join("mywidget")).unwrap();
        std::fs::write(root.join("mywidget").join("mywidget.rhai"), "fn draw() { [] }")
            .unwrap();
        // A directory without the widget's file is not a widget — a
        // leftover metadata file does not count either.
        std::fs::create_dir_all(root.join("notawidget")).unwrap();
        std::fs::write(root.join("notawidget").join("widget"), "Label=NOPE\n").unwrap();
        // A compiled widget is its library; a shipped name keeps its
        // built-in label and sizes.
        std::fs::create_dir_all(root.join("shell")).unwrap();
        std::fs::write(root.join("shell").join("shell.so"), b"not really").unwrap();

        let defs = nacelle::widget::registry::scan(&nacelle::assets::AssetRoots::new(vec![base.clone()], base.clone()));
        assert_eq!(defs.len(), 2, "only directories with the widget's file count");
        // Sorted, so panel order never depends on the filesystem.
        // An unknown widget gets its name as the label and the
        // standard sizes; a layout overrides them per panel.
        assert_eq!(defs[0].name, "mywidget");
        assert_eq!(defs[0].label, "MYWIDGET");
        assert_eq!(defs[0].ref_h_vh, 10.0);
        assert_eq!(defs[1].name, "shell");
        assert_eq!(defs[1].label, "SHELL");
        assert_eq!(defs[1].ref_h_vh, 60.0);

        let empty = nacelle::assets::AssetRoots::new(vec![base.join("nope")], base.join("nope"));
        assert!(nacelle::widget::registry::scan(&empty).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Boards travel inside the .layaut file: parsed from [board k]
    /// sections, normalised to sit next to each other, and preserved by
    /// every path that rewrites the file for other reasons.
    #[test]
    fn boards_live_in_the_layaut_file() {
        let text = "screen = 1920x1080@27\nclock = 1.00 2.00 10.00 10.00\n\n\
                    [1280x720@7]\nclock = 5.00 5.00 20.00 20.00\n\n\
                    [board 4]\ncpu = 10.00 10.00 30.00 30.00\n\n[board -3]\n\n[board 0 2]\nmemory = 1.00 1.00 20.00 20.00\n";
        let def = nacelle::layout::layaut::parse(text, "t");
        assert_eq!(def.boards.len(), 3);
        // A board parsed from rect lines is a fixed board.
        let fixed = |bd: &nacelle::layout::BoardDef| -> LayoutSpec {
            match &bd.base {
                LayoutMode::Fixed(s) => s.clone(),
                _ => panic!("expected a fixed board"),
            }
        };
        // Gaps close: the only positive board is board 1, the only
        // negative is board -1, wherever the file put them.
        let b1 = def.boards.iter().find(|(k, _)| *k == (1, 0)).expect("board 1");
        assert!(def.boards.iter().any(|(k, _)| *k == (-1, 0)), "board -1");
        assert!((fixed(&b1.1).p(wp("cpu")).w - 30.0).abs() < 0.01);
        // The empty one is a place with nothing on it.
        let bm = def.boards.iter().find(|(k, _)| *k == (-1, 0)).unwrap();
        // The vertical arm renumbers just the same: [board 0 2] with
        // nothing above it is the first board below home.
        assert!(def.boards.iter().any(|(k, _)| *k == (0, 1)), "board (0,1)");
        assert!(fixed(&bm.1).p(wp("cpu")).x >= 100.0);

        // A full-base rewrite (SAVE on the base's screen) keeps them.
        let dir = std::env::temp_dir().join("nacelle-desktop-boards-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("layauts")).unwrap();
        std::fs::write(dir.join("layauts/t.layaut"), text).unwrap();
        let mut full = LayoutSpec::default();
        full.set(wp("clock"), PanelSpec { x: 7.0, y: 8.0, w: 11.0, h: 12.0 });
        test_store(&dir).save_full("t", &full, (1920, 1080, 27)).unwrap();
        let after = std::fs::read_to_string(dir.join("layauts/t.layaut")).unwrap();
        let def2 = nacelle::layout::layaut::parse(&after, "t");
        assert_eq!(def2.boards.len(), 3, "boards must survive a base rewrite");
        assert!(after.contains("[1280x720@7]"), "overrides must survive too");

        // And an overrides rewrite keeps them just the same.
        test_store(&dir).save_overrides("t", (2560, 1440, 32), &[], &full).unwrap();
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
        let root = std::env::temp_dir().join("nacelle-desktop-widget-shadow-test");
        let _ = std::fs::remove_dir_all(&root);
        let user = root.join("user").join("widgets");
        let system = root.join("system").join("widgets");
        // The same name in both, plus one only the system has.
        for (base, script) in [(&user, "fn draw() { [] }"), (&system, "fn draw() { [1] }")] {
            std::fs::create_dir_all(base.join("clock")).unwrap();
            std::fs::write(base.join("clock").join("clock.rhai"), script).unwrap();
        }
        std::fs::create_dir_all(system.join("uptime")).unwrap();
        std::fs::write(system.join("uptime").join("uptime.rhai"), "fn draw() { [] }").unwrap();

        let defs = nacelle::widget::registry::scan(&nacelle::assets::AssetRoots::new(
            vec![root.join("user"), root.join("system")],
            root.join("user"),
        ));
        assert_eq!(defs.len(), 2, "the shadowed copy must not appear twice");
        let clock = defs.iter().find(|d| d.name == "clock").unwrap();
        assert_eq!(
            std::fs::read_to_string(user.join("clock").join("clock.rhai")).unwrap(),
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
            assert_eq!(a.panels, b.panels, "column {i} panels/weights");
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
            let mut seen = std::collections::HashSet::new();
            for c in &fl.columns {
                for (p, _) in &c.panels {
                    assert!(seen.insert(*p), "{} appears twice in {name}", p.name());
                }
            }
            assert_eq!(
                seen.len(),
                nacelle::base::panel_count(),
                "{name}.layaut must place every registered widget"
            );
        }
    }

    /// The units and pad_x keys live OUTSIDE any [column]; a board
    /// section holding [column] lines parses as a flexbox board and
    /// round-trips through the write path.
    #[test]
    fn units_pad_and_flex_boards_parse_and_roundtrip() {
        let (fl, _) = nacelle::layout::layaut::parse_flex(
            "units = px\npad_x = 3.2\n[column]\nbasis = 20\npanel = clock 7\n",
        )
        .expect("parses");
        assert!(fl.units_px);
        assert_eq!(fl.pad_x, Some(3.2));
        // Omitting both keeps the defaults.
        let (fl2, _) = nacelle::layout::layaut::parse_flex("[column]\npanel = clock 7\n").unwrap();
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
        nacelle::layout::layaut::serialize_boards(&mut out, &def.boards);
        let def2 = nacelle::layout::layaut::parse(&format!("clock = 1 2 10 10\n{out}"), "t");
        assert_eq!(def2.boards.len(), 1);
        assert!(matches!(def2.boards[0].1.base, LayoutMode::Custom(_)));
        assert_eq!(def2.boards[0].1.sizes.len(), 1);
    }

    /// clear_screen_section removes exactly one [WxH@D] section: the
    /// base, the other screens and the boards survive.
    #[test]
    fn clear_screen_section_removes_only_its_screen() {
        let dir = std::env::temp_dir().join("nacelle-desktop-clear-screen-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        let (fl, sizes) = nacelle::layout::layaut::parse_flex(
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
        let name = "unittest-roundtrip";
        let dir = std::env::temp_dir().join("nacelle-desktop-overrides-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layauts").join(format!("{name}.layaut"));

        // SAVE AS on a 2560x1440 32" screen: the full base.
        let mut full = LayoutSpec::default();
        full.set(wp("clock"), PanelSpec { x: 1.0, y: 2.0, w: 10.0, h: 10.0 });
        full.set(wp("shell"), PanelSpec { x: 20.0, y: 2.0, w: 60.0, h: 60.0 });
        test_store(&dir).save_full(name, &full, (2560, 1440, 32)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("screen = 2560x1440@32"));
        assert!(text.contains("clock = 1.00 2.00 10.00 10.00"));

        // SAVE on the SAME screen: the base itself is rewritten in full.
        let mut full2 = full.clone();
        full2.set(wp("clock"), PanelSpec { x: 3.0, y: 4.0, w: 11.0, h: 11.0 });
        test_store(&dir).save_overrides(name, (2560, 1440, 32), &[], &full2).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("clock = 3.00 4.00 11.00 11.00"));
        assert!(!text.contains("[2560x1440@32]"));

        // First save on a DIFFERENT screen: one changed panel -> section.
        let fs_spec = PanelSpec { x: 30.0, y: 10.0, w: 20.0, h: 40.0 };
        test_store(&dir).save_overrides(
            name,
            (1920, 1080, 27),
            &[(wp("filesystem"), fs_spec)],
            &full2,
        )
        .unwrap();
        // Another screen: another panel.
        let kb_spec = PanelSpec { x: 5.0, y: 60.0, w: 90.0, h: 30.0 };
        test_store(&dir).save_overrides(
            name,
            (1280, 720, 7),
            &[(wp("keyboard"), kb_spec)],
            &full2,
        )
        .unwrap();
        // First screen again: update the same panel.
        let fs_spec2 = PanelSpec { x: 40.0, y: 12.0, w: 22.0, h: 44.0 };
        test_store(&dir).save_overrides(
            name,
            (1920, 1080, 27),
            &[(wp("filesystem"), fs_spec2)],
            &full2,
        )
        .unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let def = nacelle::layout::layaut::parse(&text, name);
        // Base preserved (rewritten clock position from the same-screen SAVE).
        assert!(text.contains("clock = 3.00 4.00 11.00 11.00"));
        assert!(matches!(def.base, LayoutMode::Fixed(_)));
        // Two sections, exact matches only.
        assert_eq!(def.overrides.len(), 2);
        assert!(def.pick((2560, 1440, 27)).is_none());
        let big = def.pick((1920, 1080, 27)).unwrap();
        assert_eq!(big.panels.len(), 1);
        let (p, ps) = &big.panels[0];
        assert_eq!(*p, wp("filesystem"));
        assert!((ps.x - 40.0).abs() < 0.01 && (ps.h - 44.0).abs() < 0.01);
        let small = def.pick((1280, 720, 7)).unwrap();
        assert_eq!(small.panels.len(), 1);
        assert_eq!(small.panels[0].0, wp("keyboard"));

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
        assert!(matches!(def.base, LayoutMode::Fixed(_)));

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
}
