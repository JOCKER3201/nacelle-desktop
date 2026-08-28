//! User configuration and theme data.
//!
//! Configuration is read as an XDG cascade — the arrangement GTK, Qt,
//! libadwaita and COSMIC all use. The user's own file comes first and
//! the system ones after it, FIELD by field: a setting the user never
//! made is answered by the system file, so a distribution or an
//! administrator can change a default without anything being copied
//! into anybody's home directory.
//!
//!   $XDG_CONFIG_HOME/nacelle/nacelle-desktop.ron  — the user's own
//!       (~/.config/nacelle/… when the variable is unset)
//!   $XDG_CONFIG_DIRS/nacelle/nacelle-desktop.ron  — the system defaults
//!       (/etc/xdg/nacelle/… when the variable is unset)
//!   <either of those>/shellrc                     — bash startup file, first one found
//!
//! The FOLDER is the family and the FILE is the program: `nacelle-ai`
//! reads these very directories, so naming them after one member was an
//! accident rather than a design, and `nacelle/nacelle-ai.ron` can
//! stand beside `nacelle/nacelle-desktop.ron` the day it is needed.
//! Both search paths still carry the folder's old name one rung lower —
//! see [`FAMILY_DIR`] and [`LEGACY_FAMILY_DIR`].
//!
//! The file is Rusty Object Notation and its shape is a TYPE, in
//! [`model`]: the parser is derived from it, so every default a setting
//! can fall back to is written once, beside the field, instead of at
//! each `unwrap_or` that read it. [`model::DesktopConf`] says what the
//! whole of it looks like.
//!
//! `shellrc` is the exception that stays as it is, and is not an
//! inconsistency: it is a bash startup file — the SHELL consumes it,
//! not this program — and RON is not a language bash reads. "All the
//! configuration in RON" is about configuration, not about every file
//! that happens to lie in the configuration directory. Theme files
//! (`*.theme`) stay in their own format for the mirror-image reason:
//! their master is a schema and a document for a person to read, with
//! expressions over palette seeds that RON has no way to carry.
//!
//! A `nacelle-desktop.conf` in the old `Key=Value` format is still
//! read where no `.ron` stands beside it, and is never rewritten,
//! moved or deleted. A format change that loses somebody's settings is
//! not a format change, it is a bug with a version number.
//!
//! The user has ONE configuration, and it may only live in one place.
//! The first setting changed writes `nacelle/nacelle-desktop.ron` from
//! everything the user's own files say — including a file under the
//! folder's OLD name — and from then on that file answers alone. The
//! old one stays on disk untouched and stops being consulted, which is
//! what makes taking a field back out of the new file mean something:
//! with two files of the user's own in the cascade, removing a field
//! from the first merely hands the question to the second, which is a
//! reset that does nothing and says nothing.
//!
//! Writes go to the user's directory and nowhere else, and only when
//! the user changes something: the program creates no directory and
//! copies no file at startup.
//!
//! Everything a theme is made of is DATA, not configuration, so it lives
//! under XDG_DATA_HOME:
//!   ~/.local/share/nacelle/layauts/               — custom layout files (*.layaut)
//!   ~/.local/share/nacelle/sounds/<set>/          — sound themes, one directory
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
//! In nacelle-desktop.ron `theme:` picks one of the engine's themes;
//! `layaut:` and `sounds:` name a file from layauts/ (without an
//! extension) and a directory from sounds/. A field that is not there
//! is answered by the system file, and after that by the defaults the
//! model carries.
//!
//! `variant:` is the second half of the colour axis: it names one of the
//! theme's contrast variants — hc, the high-contrast one, is the variant
//! the engine's master ships — and `Off` or nothing is the plain theme.
//! It is a field of its own rather than part of `theme:` because the
//! two are independent: a variant is an accessibility setting, and liking
//! a colour is not a reason to give one up.
//!
//! A machine with several screens gives each of them a desktop of its
//! own, so `layaut:` is only the DEFAULT arrangement. A screen takes a
//! layaut of its own when the file names it, and one screen carries the
//! MAIN SCREEN role:
//!
//!   (
//!       layaut: Named("console"),                 // every screen not named below
//!       screens: {
//!           "edid:DEL-41B2-0123ABCD": Named("cockpit"),  // that Dell, wherever it is plugged in
//!           "eDP-1": Named("panel"),                     // whatever hangs off the laptop's panel
//!       },
//!       main_screen: Named("edid:DEL-41B2-0123ABCD"),
//!   )
//!
//! A screen is named in one of TWO vocabularies, and the difference
//! matters:
//!
//! `edid:MAKER-MODEL-SERIAL` is what the MONITOR says about itself —
//! the description block every screen carries in its own firmware. It
//! travels with the monitor: unplug it, move it to another socket, turn
//! the machine on in another order, and the settings written under this
//! key are still describing that screen.
//!
//! `DP-1`, `HDMI-A-1`, `eDP-1` is the SOCKET, which is a property of
//! the cable and not of the screen. It is what a monitor gets when its
//! firmware says nothing, it is what every file written before
//! 2026-08-18 uses, and it is still perfectly good for a rule that
//! really is about a socket — "whatever is plugged in here".
//!
//! Both are read, the monitor's own name first, so a file of either
//! generation answers. The program prints both the label and the key
//! for every screen at startup, so the line to write is never guesswork.
//! Case is not significant, `Off` means "no layaut of its own" and
//! outranks a system file that gives it one, and a name no layauts/ file
//! answers to costs that screen nothing but a line in the log — it
//! takes the default.
//!
//! A file that has NEVER named a monitor is converted once, at the first
//! start that can see which monitor is on which socket, and the log says
//! so. After that the two vocabularies stand side by side and nothing
//! rewrites either of them: a socket line typed into a file that names a
//! monitor anywhere is a rule about that socket and stays one. Two
//! monitors that give the same name — some models print one serial for
//! every unit — are left keyed by their sockets, which is the only
//! vocabulary that can still tell them apart.
//!
//! A number in the order the screens came up would be no name at all:
//! which monitor is switched on first is not a property of anything.
//!
//! `main_screen:` gives one screen the MAIN SCREEN role, in that same
//! vocabulary. What the role MEANS — four duties, one setting — is
//! written down in `screens::MainScreenDuty` and nowhere else. Absent,
//! the display server's own answer stands; `Off` says to take that
//! answer whatever it is, which is how a user overrules a system file
//! naming a monitor that is not on this desk.

pub mod model;

use crate::widgets::PanelSpec;
use model::{Choice, DesktopConf, Layered};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

// The ranges and the list of names belong to the settings themselves,
// so they live beside the fields they bound. Re-exported because the
// settings window and the grid editor ask this module for them, and
// where a constant is declared is not their business. The gutter's own
// bound is not among them: it is applied where the field is read and
// nothing outside asks for it.
pub use model::{
    color_depths, color_spaces, space_range, SpaceRange, COLOR_SPACES,
    COLOR_SPACE_TABLE, GRID_MAX, GRID_MIN,
};












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
const LINKED: [nacelle::widget::factory::BuiltinWidget; 7] = [
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
                nacelle::widget::factory::WidgetFactory::new(asset_roots()),
                |f, w| f.with_builtin(w),
            )
            .plugins_enabled(!crate::plugins::disabled())
    })
}

/// The toolkit's layaut store over this application's XDG roots.
fn store() -> LayautStore {
    LayautStore::new(asset_roots())
}

pub struct Config {
    pub layout: LayoutDef,
}

pub fn load() -> (Config, Option<String>) {
    // The dead Look=/Style= keys retire on sight — before anything
    // reads the layout or the theme (u3 §6.3).
    //
    // The file named is the one under the family directory, and only
    // that one. A configuration still sitting under the folder's old
    // name is READ and never rewritten, so on such a machine this
    // rewrite simply does not happen: the keys stay in the file,
    // harmlessly, since nothing else reads them. Retiring them would
    // mean editing a file this change promised not to touch.
    migrate_look_style_in(&config_dir().join(CONF_FILE), &asset_roots());
    install_addon_settings();
    // The registry must exist before anything parses a layout: panels
    // are resolved by name against it.
    let roots = asset_roots();
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

/// Tells the toolkit where an ADDON's own settings live, before a
/// single widget exists to ask for any.
///
/// Until this is called every such read answers `Origin::Refused`, and
/// a refusal is deliberately silent — it means a caller asked for a
/// name that is not a name, which is a programming error rather than
/// anything a user did. So leaving it uncalled did not merely disable
/// the addon-settings half, it disabled it WITHOUT A WORD: a user
/// writes `~/.config/nacelle/addons/search.ron`, the addon goes on
/// running on the values baked into it, and nothing anywhere connects
/// the two. That is the exact failure this format was chosen to make
/// impossible, so the call has a name and a test of its own.
///
/// The FAMILY name and only it. The program's own configuration search
/// path carries the folder's old name behind the new one because files
/// were already sitting there; addon settings are new with this
/// change, so there is no such directory in the world to support, and
/// inventing one would be a rung of the search path nothing can ever
/// be found on.
///
/// It also READS, once, everything standing in those directories —
/// [`prime_addon_settings`] carries the whole of why.
fn install_addon_settings() {
    let roots = nacelle::assets::AssetRoots::xdg_config(FAMILY_DIR);
    // Kept before the roots are handed over: the toolkit deliberately
    // answers no path back, so the one moment this side can see the
    // search order is while it is still holding it.
    let dirs = roots.read.clone();
    nacelle::settings::install(roots);
    prime_addon_settings(&dirs);
}

/// The sub-directory addon settings stand in, under the configuration
/// directory. The toolkit's own name for them, written once here so the
/// window and the walk below cannot drift from what is installed.
const ADDON_SETTINGS_SUB: &str = "addons";

/// Where an addon settings file goes on THIS machine.
///
/// The one directory the program would write to, which is also the one
/// the settings window tells the user about — a page reporting that a
/// file is unreadable is no use to somebody who does not know where
/// files go. The toolkit will not answer this (handing out a path is
/// the one thing it does not do), and it does not have to: the embedder
/// chose the directory, so the embedder can name it.
pub fn addon_settings_dir() -> PathBuf {
    config_dir().join(ADDON_SETTINGS_SUB)
}

/// How many settings files one directory is walked for. A machine has
/// sixteen addons; the bound is against a directory somebody has
/// emptied a download into, and reaching it costs a report that stops
/// early rather than a startup that does not finish.
const ADDON_SETTINGS_MAX: usize = 256;

/// Reads every addon settings file that exists, once, at startup.
///
/// Nothing here is needed to make a widget work: an addon reads its own
/// file the first time it draws, and the toolkit caches it, so this walk
/// changes no value anywhere. What it changes is WHEN a file that
/// cannot be used is known about, and that turned out to be the whole
/// difference between a report and a dead channel.
///
/// `nacelle::settings::problems()` fills as a side effect of somebody
/// reading. [`resolve`] is where that list becomes the notice on
/// screen, and it runs at the end of [`load`] — before the first widget
/// is built, so on the one run that matters, the first frame after the
/// user edited the file, the list was always empty. An addon that is on
/// no board never reads at all, and its file stayed unmentioned for as
/// long as it stayed off the boards: the user's edit did nothing, and
/// nothing said why.
///
/// So the host asks for what is THERE rather than waiting to be asked.
/// Both arrangements the format has are walked — `<addon>.ron`, and
/// `<addon>/<file>.ron` for an addon that needs more than one — and
/// only `.ron` is looked at, so the `.ron.bak` the toolkit leaves
/// beside a file it overwrites is not reported as a second broken copy
/// of the same settings.
///
/// A name the toolkit refuses is the one case that gets a line of its
/// own. It is a file the user wrote and NOTHING can ever read — no
/// addon can ask for a name that is not a plain name — so the silence
/// would be permanent, and it is the only failure here that is about
/// the name of the file rather than what is inside it.
///
/// Which is exactly why a name beginning with a dot is passed over
/// without a word. `.#filesystem.ron` is not somebody's settings, it is
/// the lock an editor leaves while that file is OPEN — so the one
/// moment this walk would shout about it is the moment the user is
/// sitting in the file it names. A hidden file could never be read
/// either way, and being quiet about a name nobody chose is cheaper
/// than crying wolf at the one who is mid-edit.
fn prime_addon_settings(dirs: &[PathBuf]) {
    let mut seen = 0usize;
    for dir in dirs {
        let addons = dir.join(ADDON_SETTINGS_SUB);
        let Ok(entries) = std::fs::read_dir(&addons) else {
            continue;
        };
        for entry in entries.flatten() {
            if seen >= ADDON_SETTINGS_MAX {
                return;
            }
            let path = entry.path();
            if path.is_dir() {
                let Some(addon) = stem_of(&path, None) else { continue };
                let Ok(members) = std::fs::read_dir(&path) else { continue };
                for member in members.flatten() {
                    if seen >= ADDON_SETTINGS_MAX {
                        return;
                    }
                    let member = member.path();
                    if let Some(file) = stem_of(&member, Some("ron")) {
                        seen += 1;
                        prime_one(&addon, &file, &member);
                    }
                }
            } else if let Some(addon) = stem_of(&path, Some("ron")) {
                seen += 1;
                prime_one(&addon, "", &path);
            }
        }
    }
}

/// The file's stem, when its extension is the one asked for (or when
/// none is asked for, as for a directory). `None` for anything whose
/// name is not text at all, and for a hidden one — see
/// [`prime_addon_settings`] for the whole of why a dot is passed over.
fn stem_of(path: &Path, ext: Option<&str>) -> Option<String> {
    if path.file_name().and_then(|n| n.to_str())?.starts_with('.') {
        return None;
    }
    if let Some(ext) = ext {
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            return None;
        }
    }
    path.file_stem().and_then(|s| s.to_str()).map(String::from)
}

/// One file, read and thrown away: what is wanted is the toolkit's
/// verdict on it, which lands in `problems()` for the notice and the
/// settings window to find.
///
/// Except for the one verdict the toolkit cannot put there. A name it
/// REFUSES is refused before any file is opened, so nothing is read,
/// nothing is parsed and `problems()` stays empty — and this used to be
/// a line on stderr and no more. That is the argument the toolkit
/// itself raises against a stderr-only report, in the very module that
/// fills `problems()`: a desktop session has no stderr open, and a
/// settings window announcing that every file loads while two of the
/// user's files are being ignored is worse than one that says nothing.
///
/// And here the silence is PERMANENT. A file that does not parse is
/// named the moment some addon asks for it; a file whose NAME no addon
/// can ask for is never asked for by anything, ever, so the day the
/// user renames it is the only day they will find out.
///
/// So it goes onto the toolkit's own list through `settings::report`,
/// which exists for precisely this and which the host is the only side
/// able to call: the module discovers every other problem by somebody
/// ASKING for a file, and this is the one nobody will ever ask for. The
/// stderr line stays, because a headless start has no window.
fn prime_one(addon: &str, file: &str, path: &Path) {
    if nacelle::settings::text(addon, file).1 == nacelle::settings::Origin::Refused {
        let message = "is not a name any addon can ask for \u{2014} a settings file is \
                       <addon>.ron in lower-case letters, digits, `_` and `-`, so this \
                       file will never be read by anything";
        eprintln!("nacelle-desktop: {} {message}", path.display());
        nacelle::settings::report(path.to_path_buf(), message.to_string());
    }
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





















/// The name of the theme the new engine is to load: `theme:` in
/// nacelle-desktop.ron, or the built-in master when nothing is set.
///
/// A theme name is a bare identifier — the engine refuses a path, because a
/// `[meta] base` that could name `../../etc/passwd` would be a file-read
/// primitive.
pub fn current_engine_theme() -> Option<String> {
    conf().theme.name().map(str::to_string)
}

/// Writes `theme:`. An empty name CLEARS the field — the settings
/// window's own way of saying "no choice of mine", which lets the
/// system file answer again.
pub fn set_engine_theme(name: &str) {
    update_conf(|c| c.theme = Choice::named(name));
}

/// The contrast variant to select on top of the theme: `variant:` in
/// nacelle-desktop.ron. `None` — the ordinary case — is the plain theme.
///
/// `hc` is the one the engine's master declares, and every theme resolves it:
/// a theme that declares no `[variant.*]` of its own inherits the master's,
/// so high contrast does not disappear as a side effect of choosing a colour.
///
/// `#[allow(dead_code)]` for the same reason [`set_engine_variant`] carries
/// it: the settings screen's contrast switch is the caller this was written
/// for, and it does not exist yet. [`apply_engine_variant`] used to be that
/// caller, but it now has to tell [`Choice::Off`] apart from
/// [`Choice::Inherit`] — the one distinction this collapses, both being
/// "no name" — so it reads `conf().variant` itself instead. The allow comes
/// off the day either caller is wired in.
#[allow(dead_code)]
pub fn current_engine_variant() -> Option<String> {
    conf().variant.name().map(str::to_string)
}

/// Writes `variant:`. `None` writes [`Choice::Off`] rather than
/// dropping the field, because a contrast switch turned off is an
/// explicit off that outranks a system file naming one — where a
/// missing field would inherit it. Taking the setting back altogether
/// is [`clear_look_and_feel`]'s job, and the difference between the
/// two is the whole reason the third state exists.
///
/// Read here and written elsewhere: the settings screen's contrast switch
/// calls this and then re-applies the configuration, exactly as its theme
/// list already calls [`set_engine_theme`]. Until that switch exists the
/// user writes the field by hand — `allow(dead_code)` says only that, and
/// comes off the day it is called.
#[allow(dead_code)]
pub fn set_engine_variant(name: Option<&str>) {
    update_conf(|c| c.variant = Choice::or_off(name));
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

/// The key one screen's layaut is written under in `screens:`. None
/// when the text names no screen: a key nothing could ever match a
/// screen to is not worth writing.
///
/// The vocabulary itself lives in [`crate::screens::screen_key`], with
/// the two kinds of key and the reason they cannot be confused.
fn screen_layaut_key(key: &str) -> Option<String> {
    crate::screens::screen_key(key)
}

/// What one screen's layaut resolves to, and the one sentence the log
/// gets when the configuration asked for something that is not there.
struct ScreenLayaut {
    name: String,
    note: Option<String>,
}

/// Which layaut one screen takes: the one it is assigned when that
/// layaut is installed, the desktop's default in every other case.
///
/// Pure, and handed everything it judges by — the assignments, the
/// default and the names the store actually holds — because this
/// decision has to be testable on a machine that has no screens, and
/// because it must never be able to name a layaut the store did not
/// list: that is what keeps a hand-written value out of the paths
/// built from it.
///
/// A screen has TWO keys and both are read, the monitor's own name
/// first — `ScreenId::keys` is that order. So a file written before
/// screens were keyed by their monitors goes on answering for exactly
/// as long as it says anything: nothing is lost by the change of key,
/// with or without the migration that rewrites it.
fn choose_layaut(
    id: &crate::screens::ScreenId,
    assigned: &BTreeMap<String, String>,
    default_name: &str,
    installed: &[String],
) -> ScreenLayaut {
    let default = || ScreenLayaut { name: default_name.to_string(), note: None };
    // Case is not significant: RandR says eDP-1 and a user typing
    // edp-1 means that same screen, not a screen the machine lacks.
    let Some((c, want)) = id.keys().into_iter().find_map(|key| {
        assigned
            .iter()
            .find(|(k, _)| k.as_str().eq_ignore_ascii_case(&key))
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
// elsewhere: screen.rs hands each screen the layaut its identity is
// assigned, and the settings screen writes the assignments once it
// exists (until then the user writes the line by hand, which is what
// the format is shaped for). `allow(dead_code)` says exactly that,
// and comes off the day each one is called.

/// Every screen→layaut assignment the configuration carries, the
/// user's file laid over the system ones.
#[allow(dead_code)]
pub fn screen_layauts() -> BTreeMap<String, String> {
    conf().screens()
}

/// The layaut assigned to one screen, if any. Both of a screen's keys
/// are asked, the monitor's own first; case is not significant.
#[allow(dead_code)]
pub fn layaut_for_screen(id: &crate::screens::ScreenId) -> Option<String> {
    let assigned = screen_layauts();
    id.keys().into_iter().find_map(|key| {
        assigned
            .iter()
            .find(|(k, _)| k.as_str().eq_ignore_ascii_case(&key))
            .map(|(_, v)| v.clone())
    })
}

/// Assigns a layaut to one screen, by the key that screen answers to —
/// `edid:DEL-41B2-0123ABCD` for the monitor itself, `DP-1` for the
/// socket. An empty name switches the screen OFF — written as
/// [`Choice::Off`] rather than dropped, so the user's file also
/// overrules an assignment a system file makes. Removing the entry
/// outright is [`clear_screen_layauts`].
#[allow(dead_code)]
pub fn set_layaut_for_screen(key: &str, name: &str) {
    let Some(key) = screen_layaut_key(key) else {
        eprintln!(
            "nacelle-desktop: '{key}' names no screen \u{2014} \
             no screen was assigned a layaut"
        );
        return;
    };
    update_conf(|c| {
        c.screens.insert(key, Choice::or_off(Some(name)));
    });
}

/// The NAME of the layaut a screen takes. A screen nothing names —
/// no monitor description, no connector — takes the default, and so
/// does a screen nothing was written for.
///
/// Reads the configuration files and lists the layaut store, so it is
/// asked when a screen appears or the configuration changes, never
/// per frame. [`screen_layaut`] is the one that reports a fallback;
/// this one answers quietly, so a caller comparing two screens does
/// not fill the log.
#[allow(dead_code)]
pub fn screen_layaut_name(id: &crate::screens::ScreenId) -> String {
    choose_layaut(
        id,
        &screen_layauts(),
        &current_layaut_name().unwrap_or_else(|| "default".into()),
        &list_layauts(),
    )
    .name
}

/// The layaut a screen takes: the name and the layout itself.
///
/// This is the whole of "one screen, one desktop": a screen the
/// configuration names takes that layaut, and every other screen takes
/// the default one. A configuration naming a layaut this machine does
/// not have is a mistake in a file, never a reason not to start — the
/// screen falls back to the default and the log says which screen,
/// which layaut and what it got instead.
pub fn screen_layaut(id: &crate::screens::ScreenId) -> (String, LayoutDef) {
    let chosen = choose_layaut(
        id,
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

/// The key of the screen the configuration gives the MAIN SCREEN role
/// to, if it gives it to any. What the role MEANS is written down in
/// [`crate::screens::MainScreenDuty`] and nowhere else.
///
/// `None` covers both "nothing was said" and an explicit "the display
/// server's answer, whatever it is" — the two differ in the CASCADE
/// and not in the value, which is [`Choice`]'s whole business, and by
/// the time the answer is here the cascade has already happened.
pub fn main_screen_key() -> Option<String> {
    conf().main_screen.name().map(str::to_string)
}

/// Gives the role to one screen. `None` writes [`Choice::Off`] rather
/// than dropping the field — a user who has said "let the display
/// server decide" has to outrank a system file naming a screen that is
/// not on this desk. Taking the setting back altogether — leaving the
/// question to the rest of the cascade, which is a third answer and not
/// the same as either — is [`clear_main_screen`].
///
/// Read here and written elsewhere: the settings window's SCREENS page
/// calls this once it exists, and until then the user writes the field
/// by hand. `allow(dead_code)` says only that, and comes off the day it
/// is called.
#[allow(dead_code)]
pub fn set_main_screen(key: Option<&str>) {
    let key = match key {
        None => {
            update_conf(|c| c.main_screen = Choice::Off);
            return;
        }
        Some(k) => k,
    };
    let Some(key) = screen_layaut_key(key) else {
        eprintln!("nacelle-desktop: '{key}' names no screen \u{2014} the main screen is unchanged");
        return;
    };
    update_conf(|c| c.main_screen = Choice::Named(key));
}

/// Brings the configuration's screen keys up to date with the monitors
/// this machine can actually see — see
/// [`DesktopConf::migrate_screens`], which is the rule and is pure.
///
/// Answers whether anything moved. NOTHING IS WRITTEN WHEN NOTHING
/// MOVED, and that is a promise about more than syscalls: this runs at
/// every start, and the program installs nothing and makes no
/// directory until the user changes something. A machine with no
/// configuration at all must still have none after this.
///
/// The document handed to the migration is the USER's own file, which
/// is what every write through this door gets and for the reason
/// [`update_conf`] gives. A system file keyed by connector is left
/// exactly as its administrator wrote it and goes on being read — both
/// keys resolve, so nothing there is lost either.
pub fn migrate_screen_identities(live: &[crate::screens::ScreenId]) -> bool {
    // Asked of the CASCADE first, and only as a question: is there a
    // socket-keyed entry anywhere that one of today's monitors would
    // claim? Most machines answer no, and on those the door below is
    // never opened at all.
    //
    // WHICH IS NOT ONLY A SAVING OF SYSCALLS. Opening that door READS
    // the user's own file, and a file that cannot be read is copied
    // aside and reported to them in the words "the setting you just
    // changed has REPLACED it" — see [`rescue_unreadable`], which is
    // written for the one moment that sentence is true. Nothing here
    // changes a setting. So a start with nothing to migrate must not
    // reach that door, or every start on a machine with one broken
    // bracket in its file would leave a rescue copy and a notice about
    // a setting nobody touched.
    let seen = conf();
    let worth_it = live.iter().any(|id| {
        let (Some(_), Some(c)) = (&id.edid, &id.connector) else { return false };
        seen.screens.keys().any(|k| k.eq_ignore_ascii_case(c))
            || seen.main_screen.name().map(|m| m.trim().eq_ignore_ascii_case(c)).unwrap_or(false)
    });
    if !worth_it {
        return false;
    }
    let mut moved = false;
    update_conf_when(|c| {
        moved = c.migrate_screens(live);
        moved
    });
    if moved {
        eprintln!(
            "nacelle-desktop: per-screen settings written against a socket now name the \
             monitor on it \u{2014} they follow that monitor to any other socket from here on"
        );
    }
    moved
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
///
/// `Key=Value` throughout, and it stays that way: those two keys only
/// ever existed in that format, so the file this rewrites is by
/// definition an old one. A `.ron` beside it takes the whole directory
/// over anyway (see [`read_conf_dir`]), which is what stops the two
/// from ever contradicting each other.
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
    /// preserving everything else — one line, minus the filesystem.
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
    // BEFORE anything else has a chance to fill this in. A file that
    // did not parse explains every other surprise the user is about to
    // have — the theme, the layaut and the fonts all reverted at once —
    // and a warning about one of the symptoms would send them looking
    // in the wrong place.
    if let Some(bad) = conf_error() {
        warning = Some(bad);
    }
    // An addon whose settings file the host could not use. Second, so
    // it does not stand in front of a broken program file — that one
    // explains more — and ahead of the theme engine's own remarks,
    // because this is a file the user wrote and those are usually about
    // a file they did not.
    if let Some(p) = nacelle::settings::problems().first() {
        warning.get_or_insert_with(|| {
            // An entry with no addon on it belongs to no addon — it is
            // a file whose NAME is not a name — so the clause about an
            // addon running on its defaults would be about nobody.
            let fate = if p.addon.is_empty() {
                ""
            } else {
                " \u{2014} the addon is running on its own defaults"
            };
            format!("{}: {}{fate}", p.path.display(), p.message)
        });
    }
    // Last, so it wins: this one is not a description of a state the
    // user can go and look at, it is the only notice of something that
    // has already happened to their file.
    if let Some(rescued) = take_conf_rescued() {
        warning = Some(rescued);
    }
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

/// Whether the desktop's own accessibility settings ask for high contrast —
/// `org.freedesktop.appearance`'s `contrast`, read by `a11y_portal.rs` at
/// startup and kept live over the portal's `SettingChanged` signal.
///
/// Process-wide rather than carried on a `Config` value, for the same reason
/// libnacelle's `motion::PLATFORM_REDUCE` is: it is a fact about the desktop
/// the user is sitting at, not a fact about any one reload, and the portal's
/// signal can arrive on its own thread at any point in the session — there
/// is no "current config" for it to ride in on. It lives HERE rather than in
/// `a11y_portal.rs` itself because [`apply_engine_variant`] is the one place
/// a `variant:` choice is already turned into an `nacelle::theme::set_variant`
/// call, and folding the platform's answer in anywhere else would be a
/// second place that call can be made from — precisely the "written out
/// twice" failure `apply_config!`'s own doc comment in main.rs warns about,
/// just for a second input instead of a second copy.
static PLATFORM_HIGH_CONTRAST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Tells the configuration layer what the platform's high-contrast
/// preference is, and answers what it was — called by `a11y_portal.rs`
/// alone, both for the portal's first answer and for every
/// `SettingChanged` afterwards.
pub(crate) fn set_platform_high_contrast(on: bool) -> bool {
    PLATFORM_HIGH_CONTRAST.swap(on, std::sync::atomic::Ordering::Relaxed)
}

fn platform_high_contrast() -> bool {
    PLATFORM_HIGH_CONTRAST.load(std::sync::atomic::Ordering::Relaxed)
}

/// The precedence between an explicit `variant:` choice and the platform's
/// own high-contrast signal, reduced to one pure function so the rule is
/// unit-testable without a live D-Bus connection — the same way
/// `motion::set_platform_reduce_motion` decouples the platform's fact from
/// any real portal in libnacelle.
///
/// [`current_engine_variant`] is not what this reads the choice through: it
/// collapses [`Choice::Off`] and [`Choice::Inherit`] to the same `None`,
/// which is exactly the distinction this needs — an explicit `Off` is the
/// user overruling the platform the same way it overrules a system file,
/// while `Inherit` is the one state that has not answered yet and so is the
/// only one the platform gets to answer for it.
fn wanted_variant(platform_high_contrast: bool, choice: &Choice) -> Option<String> {
    match choice {
        Choice::Named(name) => Some(name.clone()),
        Choice::Off => None,
        Choice::Inherit => platform_high_contrast.then(|| "hc".to_string()),
    }
}

/// Selects the wanted variant — the configured `variant:` choice, with the
/// platform's own high-contrast signal deciding for it under
/// [`Choice::Inherit`] (see [`wanted_variant`]) — on the theme just loaded.
///
/// `pub(crate)` rather than private: `a11y_portal.rs` calls this again,
/// standing alone rather than on the far side of a theme load, every time
/// `SettingChanged` reports a new answer — the whole reason `want = None`
/// below sets the plain sibling explicitly instead of trusting a load to
/// have done it already (`nacelle::theme::set_sibling`'s own no-op guard
/// makes the redundant call after a real load free).
///
/// A name this theme has no sibling for is a sentence in the log and nothing
/// more. Falling back to the plain theme costs the user contrast; refusing to
/// start costs them the desktop, and a typo in a text file may not be the
/// reason a machine does not come up.
pub(crate) fn apply_engine_variant() {
    let want = wanted_variant(platform_high_contrast(), &conf().variant);
    let refused = match &want {
        Some(name) if !nacelle::theme::set_variant(Some(name)) => Some(name.clone()),
        Some(_) => None,
        None => {
            nacelle::theme::set_variant(None);
            None
        }
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
/// ones, field by field.
///
/// MEMOISED, and that is not a micro-optimisation — it is the whole
/// difference between a program that reads its settings and one that
/// reads them forty-six times a second. Measured on 2026-08-18 under
/// `strace`: 294 opens of `nacelle-desktop.ron` in 89 seconds, 136 of
/// them a frame apart, because one drawn row asked a question that
/// walks the cascade (`sound_set_note`, below). Every one of those
/// asks parsed 1121 bytes of RON twice and knocked on eight paths that
/// have never existed on this machine.
///
/// What is kept is the ANSWER and the metadata of every file it was
/// built from. A call re-stamps those files — one `statx` each, no
/// open, no parse — and hands back the same document while nothing has
/// moved. The stamp is device, inode, length and modification time,
/// which is what every reader of a file on disk has to go on; two
/// different documents written into the same inode inside one tick of
/// the filesystem's clock would be missed, and that is a hand editing
/// the file twice in the same millisecond. This program's own writes
/// do not depend on it either way — [`update_conf`] puts what it wrote
/// straight into the memo, so a setting is answered by its new value
/// the instant it is changed and not when the bytes reach the disk.
fn conf() -> DesktopConf {
    let dirs = config_dirs();
    let seen = conf_stamps(&dirs);
    if let Some(doc) = conf_memo_hit(&dirs, &seen) {
        return doc;
    }
    // Read afresh. `conf_dirs` may drop a rung — it parses the user's
    // own file to decide — and both halves are covered by the stamps
    // above, which is why they are taken of the FULL search path.
    let doc = cascade_conf(&conf_dirs());
    conf_memo_put(&dirs, seen, &doc);
    doc
}

/// What a file was, as cheaply as a filesystem will say it.
///
/// Not a hash: the point of the memo is to stop reading the bytes, so
/// the test of "still the same file" may not read them either.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
}

impl Stamp {
    /// `None` when nothing stands there — which is an ANSWER and not a
    /// failure, and has to compare equal to itself: eight of the nine
    /// paths a cascade walks on this machine are empty, and a memo that
    /// could not remember an absence would re-read on every call.
    fn of(path: &Path) -> Option<Stamp> {
        use std::os::unix::fs::MetadataExt;
        // Following links, exactly as the read does: a configuration
        // linked in from a dotfiles repository is the file at the end
        // of the chain, and that is the file whose changes matter.
        let m = std::fs::metadata(path).ok()?;
        Some(Stamp { dev: m.dev(), ino: m.ino(), len: m.len(), mtime: (m.mtime(), m.mtime_nsec()) })
    }
}

/// Every file the configuration document can be built from, in the
/// order the cascade would meet them: the carry mark first, then each
/// directory's `.ron` and the `Key=Value` file behind it.
///
/// The list is what the memo watches, so it has to be a SUPERSET of
/// what a read would touch — never a subset. Both formats are named at
/// every rung even where only one can win, because which one wins is
/// itself a thing that changes when a file appears.
///
/// WHAT THIS COSTS, under `strace` on the owner's layout (six
/// directories, so thirteen names): 136 asks of the cascade went from
/// 816 opens of the document, 1088 absent paths knocked on and 272
/// parses of RON, down to six opens, eight absent paths and two parses
/// — and up from 681 stats to 1774. Thirteen stats an ask is the price
/// of not reading, and it is worth paying because a stat neither
/// allocates nor parses; the syscall count of the whole exchange fell
/// by a third. Stamping the DIRECTORIES instead would take it to seven
/// and would stop noticing a directory that came into existence
/// mid-session, which is what installing a package looks like.
///
/// Those are MEASURED numbers and they have to be repeatable to be
/// worth quoting: `the_cascade_asked_a_hundred_and_thirty_six_times`
/// is the probe they were taken from, with the commands in its own
/// comment. The 1088 is the audit's figure for the cascade's ENOENT
/// arrived at independently (8 × 136), which is the check on the probe
/// standing for the real program.
fn conf_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(1 + dirs.len() * 2);
    out.push(config_dir().join(CONF_RON_CARRIED));
    for d in dirs {
        out.push(d.join(CONF_RON));
        out.push(d.join(CONF_FILE));
    }
    out
}

fn conf_stamps(dirs: &[PathBuf]) -> Vec<Option<Stamp>> {
    conf_files(dirs).iter().map(|p| Stamp::of(p)).collect()
}

/// What a WRITE is built from, which is not what a read hands out.
///
/// A save rewrites the user's own rungs and only those: a value that
/// came from `/etc/xdg` has to stay a system value, or the first
/// setting anybody changed would freeze that day's defaults into their
/// home directory. So the two documents are kept apart — [`ConfMemo`]
/// holds both, and each answers the question it is the answer to.
#[derive(Clone)]
struct ConfSeed {
    /// The user's own rungs merged, with nothing of the system's in it.
    mine: DesktopConf,
    /// Whether every one of the user's folders was readable when this
    /// was built — see [`mark_old_folder_carried`]. Remembered rather
    /// than assumed, because a save seeded from here does not re-read
    /// the folder that would have said no.
    carried: bool,
}

/// The last answer [`conf`] gave, and what it was built from.
struct ConfMemo {
    /// The search path it was built for. A test that moves
    /// `XDG_CONFIG_HOME` between two calls is asking a different
    /// question, and must not be answered with the old one.
    dirs: Vec<PathBuf>,
    /// The stamps of [`conf_files`], or `None` while a write of this
    /// program's own is still on its way to the disk: until it lands,
    /// the files cannot answer for something we already know.
    seen: Option<Vec<Option<Stamp>>>,
    doc: DesktopConf,
    /// Present only when this entry was filed by a SAVE. A read cannot
    /// fill it — it merges the whole cascade and never holds the user's
    /// rungs on their own — so a read leaves it empty and the next save
    /// goes to the disk for its seed, which is what it always did.
    seed: Option<ConfSeed>,
    /// Bumped on every store, so the writer thread can tell whether the
    /// document it has just made durable is still the one being held.
    serial: u64,
}

static CONF_MEMO: std::sync::Mutex<Option<ConfMemo>> = std::sync::Mutex::new(None);

/// Whether the memo still stands for this question: the same search
/// path, and either files that have not moved or a write of ours still
/// in the air.
fn conf_memo_stands(m: &ConfMemo, dirs: &[PathBuf], seen: &[Option<Stamp>]) -> bool {
    m.dirs == dirs && m.seen.as_ref().map_or(true, |had| had.as_slice() == seen)
}

/// The memoised answer, if it is still an answer to this question.
fn conf_memo_hit(dirs: &[PathBuf], seen: &[Option<Stamp>]) -> Option<DesktopConf> {
    let memo = CONF_MEMO.lock().ok()?;
    let m = memo.as_ref()?;
    conf_memo_stands(m, dirs, seen).then(|| m.doc.clone())
}

/// What the last SAVE decided, if it still stands — the document the
/// next save is built on top of.
fn conf_memo_seed(dirs: &[PathBuf], seen: &[Option<Stamp>]) -> Option<ConfSeed> {
    let memo = CONF_MEMO.lock().ok()?;
    let m = memo.as_ref()?;
    conf_memo_stands(m, dirs, seen).then(|| m.seed.clone()).flatten()
}

fn conf_memo_put(dirs: &[PathBuf], seen: Vec<Option<Stamp>>, doc: &DesktopConf) {
    let Ok(mut memo) = CONF_MEMO.lock() else { return };
    let serial = memo.as_ref().map_or(0, |m| m.serial).wrapping_add(1);
    // No seed: this entry was built by READING, and the files it was
    // built from have moved since the last save — whatever that save
    // decided is not what stands on the disk any more.
    *memo = Some(ConfMemo {
        dirs: dirs.to_vec(),
        seen: Some(seen),
        doc: doc.clone(),
        seed: None,
        serial,
    });
}

/// Puts a document this program has just DECIDED on into the memo,
/// ahead of the bytes reaching the disk, and answers the serial it was
/// filed under.
///
/// This is what lets the write be durable without the interface waiting
/// for it. The running program's questions are answered from here from
/// the moment the setting changes; the file catches up behind it, and
/// [`conf_memo_settle`] closes the loop when it has.
fn conf_memo_pending(dirs: &[PathBuf], doc: &DesktopConf, seed: ConfSeed) -> u64 {
    let Ok(mut memo) = CONF_MEMO.lock() else { return 0 };
    let serial = memo.as_ref().map_or(0, |m| m.serial).wrapping_add(1);
    *memo = Some(ConfMemo {
        dirs: dirs.to_vec(),
        seen: None,
        doc: doc.clone(),
        seed: Some(seed),
        serial,
    });
    serial
}

/// The write filed under `serial` has landed: stamp the files it wrote
/// so the memo can be checked against them again.
///
/// A serial that has moved on means somebody changed a setting while
/// this write was in flight, and their document is the one being held —
/// so this leaves the memo alone and it stays pending until THEIR write
/// settles. Nothing is lost either way: a memo with no stamps costs one
/// honest re-read.
fn conf_memo_settle(serial: u64) {
    let Ok(mut memo) = CONF_MEMO.lock() else { return };
    let Some(m) = memo.as_mut() else { return };
    if m.serial != serial {
        return;
    }
    let dirs = m.dirs.clone();
    m.seen = Some(conf_files(&dirs).iter().map(|p| Stamp::of(p)).collect());
}

/// The write filed under `serial` never landed: forget what it decided.
///
/// A setting that could not be saved may not go on being answered as
/// though it had been. Dropping the memo puts the disk back in charge,
/// so the value springs back to what the file still says — which is
/// what the user sees beside the sentence [`report_write`] puts up, and
/// what happened before any of this was on a thread of its own.
fn conf_memo_forget(serial: u64) {
    let Ok(mut memo) = CONF_MEMO.lock() else { return };
    if memo.as_ref().is_some_and(|m| m.serial == serial) {
        *memo = None;
    }
}

/// How many times a configuration file has been READ off the disk since
/// the program started — one per file that was found and turned into
/// text, which is what `strace` counts as an `openat` of it.
///
/// The instrument this whole memo was built to move: a claim that the
/// cascade is no longer walked per frame is a number or it is nothing.
/// A TEST BUILD ONLY, because nothing the shipped program does reads it
/// — an `allow(dead_code)` saying so on a `pub fn` was the same fact
/// admitted without acting on it. [`note_conf_file_read`] is what feeds
/// it, and in a shipped build that is an empty function.
#[cfg(test)]
static CONF_FILE_READS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One configuration file turned into text.
#[cfg(test)]
fn note_conf_file_read() {
    CONF_FILE_READS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(test))]
fn note_conf_file_read() {}

/// Read by the tests that count reads, and by nothing the program does
/// — an instrument, kept beside the thing it measures.
#[cfg(test)]
fn conf_file_reads() -> u64 {
    CONF_FILE_READS.load(std::sync::atomic::Ordering::Relaxed)
}

/// The user's OWN configuration directories, most specific first: the
/// family folder and, one rung behind it, the folder's old name. The
/// system end is not here, and that is the point — these are the two
/// places a file of the USER's can be, which is what both the
/// migration and the supersession below need to know.
fn user_conf_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    push_level(&mut out, &config_home());
    out
}

/// The directories the configuration DOCUMENT is read from, which is
/// [`config_dirs`] with one rung possibly taken out.
///
/// A user has ONE configuration. Once `~/.config/nacelle/nacelle-desktop.ron`
/// answers, the same user's old-named folder stops being consulted for
/// it: that file can only have been written by [`update_conf`], which
/// seeds from both folders, so everything the old one said is already
/// in it.
///
/// Leaving it in was a reset that could not work, and the failure was
/// silent. Reset REMOVES the user's fields so the system file can
/// answer again; with a second file of the user's own still standing
/// behind the first, the removal simply handed the question to it, and
/// the theme, the layaut and the sound set came back unchanged with
/// nothing said. Rewriting or deleting that file instead is the one
/// thing this change promised not to do — so it is superseded rather
/// than touched, and it stays on disk exactly as it was.
///
/// The `.ron` must PARSE, not merely exist. A file that does not parse
/// supersedes nothing: its own contents are already unreachable, and
/// taking the older file away as well would turn one broken bracket
/// into the loss of both.
///
/// And the OLD file has to have been readable too, which is the half
/// that was missing. The sentence above — everything the old one said
/// is already in the new one — is a claim about a file [`update_conf`]
/// managed to READ, and it is false exactly when it could not: an old
/// file behind the wrong permissions, or a bracket short, contributes
/// nothing to the document that gets written, and dropping its
/// directory afterwards is what turns "not read this once" into "never
/// read again". The file is still on disk, so the user repairs the
/// typo and waits for their settings to come back; superseded, they
/// never do, and nothing on screen connects the two.
///
/// So an unreadable old file KEEPS its rung. It contributes nothing
/// while it stays broken — `cascade_conf` reads past it and reports it,
/// which is also the sentence that was missing — and everything the
/// moment it is repaired. The reset the supersession exists for is
/// unharmed either way: the old folder is out of the cascade AND out of
/// the seeding, so a field taken out of the new document stays out.
///
/// What the answer may NOT be derived from is the state of the old file
/// now. That was the first repair and it is wrong in the one direction
/// that matters: the file is unreadable when the carry is attempted, so
/// its rung is kept — and then the user repairs the typo, the file
/// parses, and the same rule retires it on the next read without a
/// single byte of it ever having been carried anywhere. The moment the
/// settings would have come back is the moment they are taken away.
/// A condition that heals cannot record an event that did not happen.
///
/// So the event is written down when it happens, and read here. See
/// [`CONF_RON_CARRIED`].
///
/// That costs one parse of a small file per call. `cascade_conf` is
/// about to parse the same file again, and the alternative — passing
/// the answer down through a function whose whole virtue is that it
/// takes its directories and reads no environment — would buy a few
/// microseconds with the one property that makes it testable.
fn conf_dirs() -> Vec<PathBuf> {
    let mut dirs = config_dirs();
    let old = config_home().join(LEGACY_FAMILY_DIR);
    if !dirs.iter().any(|d| *d == old) {
        return dirs;
    }
    // The new file must PARSE as well, and not merely stand there: a
    // file that does not parse is already unreachable, and taking the
    // older one away as well would turn one broken bracket into the
    // loss of both. The mark says the carry happened; this says there
    // is still something to have carried it into.
    if !old_folder_carried() || !matches!(read_conf_dir(&config_dir()), Ok(Some(_))) {
        return dirs;
    }
    warn_once_about_superseded(&old);
    dirs.retain(|d| *d != old);
    dirs
}

/// Whether the user's old-named folder has been carried across.
///
/// A file rather than a comparison of the two documents. The comparison
/// was tried and cannot work: "the new one already says everything the
/// old one says" is exactly what LOOK AND FEEL RESET makes false on
/// purpose — the reset REMOVES fields — so a rule built on it would
/// bring the old folder back the instant somebody reset, and hand the
/// removal straight back to the file it was supposed to get past.
///
/// It is a mark and not a copy: deleting it loses nothing, and what it
/// costs is one directory being read again. That is the safe direction,
/// which is the reason it is a separate file at all — the alternative,
/// a field inside `nacelle-desktop.ron`, is bookkeeping in a document
/// the user edits by hand and would be answering a question about the
/// migration in the middle of their settings.
fn old_folder_carried() -> bool {
    config_dir().join(CONF_RON_CARRIED).is_file()
}

/// Writes that mark, once the carry has actually happened.
///
/// Best effort in both directions: a mark that cannot be written costs
/// one folder being read that need not be, which is the direction to
/// fail in, and it is not worth a sentence to anybody.
fn mark_old_folder_carried() {
    // Nothing to retire, nothing to say: a machine that never had the
    // old folder would otherwise get a mark about a directory it has
    // never seen, which is litter with a sentence on it.
    if !config_home().join(LEGACY_FAMILY_DIR).is_dir() {
        return;
    }
    let mark = config_dir().join(CONF_RON_CARRIED);
    if mark.exists() {
        return;
    }
    let _ = std::fs::write(
        &mark,
        format!(
            "// The settings that were in {} have been carried into {}, which\n\
             // answers for them now. That folder is no longer read for them.\n\
             //\n\
             // Nothing was deleted, and this file holds no settings. Delete it\n\
             // and the old folder is read again, one rung behind this one.\n",
            config_home().join(LEGACY_FAMILY_DIR).display(),
            config_dir().join(CONF_RON).display(),
        ),
    );
}

/// Names the user's old-named folder as no longer answering for the
/// configuration, once.
///
/// Said in its own line rather than folded into
/// [`warn_once_about_legacy`], which is about the FOLDER and stays
/// true: the shell startup file and the data tree are still read from
/// there. This one is about the configuration document alone, and it
/// is the more specific of the two.
static SUPERSEDED_SAID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn warn_once_about_superseded(old: &Path) -> bool {
    use std::sync::atomic::Ordering;
    if SUPERSEDED_SAID.swap(true, Ordering::Relaxed) {
        return false;
    }
    eprintln!(
        "nacelle-desktop: the settings in {} are no longer read \u{2014} they were \
         carried into {} the first time a setting was changed, and that file \
         answers for them now. Nothing has been deleted",
        old.display(),
        config_dir().join(CONF_RON).display()
    );
    true
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
const CONF_RON: &str = "nacelle-desktop.ron";

/// The copy kept of whatever stood there before a write.
///
/// RON is parsed ALL OR NOTHING, where `Key=Value` lost one line per
/// mistake — so the cost of a bad write is the whole file rather than
/// one setting, and a file that cannot be lost is the only honest
/// answer to that. The write itself lands through a temporary name, so
/// a crash half way through leaves the old file whole rather than
/// truncated.
///
/// The copy is taken of a file this program did not write, and only of
/// one — [`ours`] carries why a copy taken on every write is no copy at
/// all.
const CONF_RON_BACKUP: &str = "nacelle-desktop.ron.bak";

/// The middle of a temporary name, not the whole of one: what
/// [`claim_tmp`] builds is `<file>.new.<pid>.<n>`, because a single
/// fixed name is a name two processes share.
const CONF_RON_TMP: &str = ".new.";

/// The mark saying the user's old-named folder has been carried into
/// the new one, and may stop being read for the configuration.
///
/// It records an EVENT — a write that seeded from that folder and
/// succeeded — because the alternative, asking what the two files look
/// like now, cannot tell "everything was carried" from "nothing could
/// be read and nothing was". See [`old_folder_carried`].
const CONF_RON_CARRIED: &str = "nacelle-desktop.ron.carried";

/// The rescue copy of a file that could not be parsed, taken the moment
/// this program decided to replace it.
///
/// [`CONF_RON_BACKUP`] cannot do this job and it is worth saying why,
/// because it looks as though it could. That copy is one generation
/// deep and is retaken on EVERY write, and the writes come in bursts:
/// one arrow key on the volume slider is one write. So a user whose
/// file is a bracket short, who then nudges a slider twice, has a live
/// file holding one setting, a `.bak` holding the same file one nudge
/// earlier, and nothing anywhere holding what they actually wrote. Two
/// keystrokes, and the whole configuration is gone.
///
/// A rescue copy is never written over. Once this name holds a text it
/// keeps it, however many writes follow, and a DIFFERENT text goes to
/// `<name>.2`, `.3` and so on. Both halves are load-bearing and for
/// opposite reasons. Without the first, the burst above simply moves
/// here: the second nudge would replace the user's text with the file
/// the first nudge wrote. Without the second, the copies this program
/// refuses to delete become the thing that destroys the next one — a
/// user who broke their file in June, repaired it and left the copy
/// lying about would have August's text answered with "already
/// rescued", and lose it exactly as if none of this existed.
///
/// Copies are left for the user to delete: one this program cleaned up
/// after itself would be a rescue copy that disappears exactly when
/// somebody is still working out what went wrong.
const CONF_RON_RESCUE: &str = "nacelle-desktop.ron.broken";

/// How many distinct unreadable texts one directory will hold.
///
/// A limit rather than none, because the name is derived from a
/// COMPARISON and a comparison can be wrong about a filesystem that
/// answers oddly; without a stop this walks forever with the user
/// waiting on a keypress. The number is far past anything a person
/// reaches — it is one per typo that was never cleaned up — so the
/// branch that gives up is about a broken disk, not about a user.
const CONF_RESCUE_LIMIT: u32 = 64;

/// The same configuration in the format that came before it.
///
/// Read where no `.ron` stands beside it, never written and never
/// deleted: a machine that had settings before this change keeps
/// exactly the file it had, and gains one the first time a setting is
/// changed.
const CONF_FILE: &str = "nacelle-desktop.conf";

/// The directory the nacelle FAMILY keeps its configuration and its
/// data in, under every XDG root — `~/.config/nacelle`,
/// `~/.local/share/nacelle`, `/etc/xdg/nacelle`, `/usr/share/nacelle`.
///
/// The folder is the family, the file inside it is the program. The
/// themes, the sounds, the layauts and the addons belong to the
/// environment rather than to one binary, and `nacelle-ai` already
/// reads these directories, so a folder named after a single member was
/// an accident that happened to work.
const FAMILY_DIR: &str = "nacelle";

/// What that directory was called before — the desktop's own name.
///
/// Nothing on disk is moved. Every search path carries this name
/// directly BEHIND its new-named counterpart at the same level, so a
/// machine that has `~/.config/nacelle-desktop` or
/// `~/.local/share/nacelle-desktop` keeps reading it: the settings, the
/// sound sets and the layauts installed under the old name go on
/// working, and a user's old file still outranks the system defaults.
/// Only writes have moved, which is what makes this reversible — the
/// whole change can be undone by dropping a branch, because it deleted
/// nothing.
const LEGACY_FAMILY_DIR: &str = "nacelle-desktop";

















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

/// Current `sounds:` value (if a safe, non-empty name).
pub fn current_sounds_name() -> Option<String> {
    conf().sounds.name().and_then(safe_component)
}

pub fn set_sounds_option(name: &str) {
    update_conf(|c| c.sounds = Choice::named(name));
}

pub fn list_layauts() -> Vec<String> {
    store().list()
}

/// Current `layaut:` value (if a safe, non-empty name).
pub fn current_layaut_name() -> Option<String> {
    conf().layaut.name().and_then(safe_component)
}

/// Terminal font preferences: (size scale, family, weight).
pub fn term_font_prefs() -> (f32, Option<String>, Option<String>) {
    font_prefs(&conf().term_font, 0.5, 2.0)
}

/// Interface font preferences: (size scale, family, weight).
pub fn ui_font_prefs() -> (f32, Option<String>, Option<String>) {
    font_prefs(&conf().ui_font, 0.30, 1.25)
}

/// One font section as the renderer wants it. The two sections allow
/// different ranges — the terminal may be made twice as big, the
/// interface may not — so the caller names them.
fn font_prefs(f: &model::FontConf, min: f32, max: f32) -> (f32, Option<String>, Option<String>) {
    (
        f.scale(min, max),
        f.family.name().map(str::to_string),
        f.weight.name().map(str::to_string),
    )
}

pub fn set_term_font_size(percent: u32) {
    update_conf(|c| c.term_font.size = Some(percent as f32));
}

pub fn set_term_font_family(name: &str) {
    update_conf(|c| c.term_font.family = Choice::named(name));
}

pub fn set_term_font_weight(name: &str) {
    update_conf(|c| c.term_font.weight = Choice::named(name));
}

pub fn set_ui_font_size(percent: u32) {
    update_conf(|c| c.ui_font.size = Some(percent as f32));
}

pub fn set_ui_font_family(name: &str) {
    update_conf(|c| c.ui_font.family = Choice::named(name));
}

pub fn set_ui_font_weight(name: &str) {
    update_conf(|c| c.ui_font.weight = Choice::named(name));
}

pub fn set_layaut_option(name: &str) {
    update_conf(|c| c.layaut = Choice::named(name));
}

/// Sound preferences: (master volume 0-100, typing sounds, ambient bed).
/// Everything on by default — a fresh install should be heard.
pub fn sound_prefs() -> (u32, bool, bool) {
    let s = conf().sound;
    (s.volume(), s.typing(), s.ambient())
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

pub fn color_prefs() -> ColorPrefs {
    let c = conf().color;
    let file = |ch: &Choice| ch.name().and_then(safe_component);
    ColorPrefs {
        depth: c.depth(),
        space: c.space(),
        lut: file(&c.lut),
        icc: file(&c.icc),
    }
}

pub fn set_color_depth(bits: u32) {
    update_conf(|c| c.color.depth = Some(bits));
}

pub fn set_color_space(space: &str) {
    update_conf(|c| c.color.space = Choice::named(space));
}

/// Nothing chosen is an explicit OFF here, not a question passed on:
/// a grading LUT switched off in the settings window may not come back
/// because a system file names one.
pub fn set_color_lut(name: Option<&str>) {
    update_conf(|c| c.color.lut = Choice::or_off(name));
}

pub fn set_color_icc(name: Option<&str>) {
    update_conf(|c| c.color.icc = Choice::or_off(name));
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
    let b = conf().blur;
    (b.radius(), b.opacity())
}

pub fn set_blur_radius(percent: u32) {
    update_conf(|c| c.blur.radius = Some(percent.min(model::BlurConf::FULL)));
}

pub fn set_blur_opacity(percent: u32) {
    update_conf(|c| c.blur.opacity = Some(percent.min(model::BlurConf::FULL)));
}

pub fn set_sound_volume(percent: u32) {
    update_conf(|c| c.sound.volume = Some(percent.min(model::SoundConf::VOLUME)));
}

pub fn set_sound_typing(on: bool) {
    update_conf(|c| c.sound.typing = Some(on));
}

pub fn set_sound_ambient(on: bool) {
    update_conf(|c| c.sound.ambient = Some(on));
}

/// `grid: (padding: …)`, if the file carries one.
///
/// The band around a panel is a length like every other, so the theme owns
/// it — `layout.panel_gutter` — and this key is the user's stage-5 override
/// of that one token, the arrangement `Density=` already has with
/// `metric.density`. Held apart from the length itself because only the
/// override comes from a file: re-reading it costs the config cascade,
/// re-reading the theme's costs an array index, and the second happens
/// whenever the engine re-bakes.
///
/// The bound is on the typed number alone. A length the theme wrote is not
/// this program's to cap.
pub fn grid_padding_override() -> Option<u32> {
    conf().grid.padding()
}

/// The band kept clear around every panel on a board, in device px.
///
/// `u` is a function of the window height, so this answer belongs to the
/// screen and the bake that asked it, not to the process.
pub fn panel_gutter(user: Option<u32>) -> f32 {
    if let Some(n) = user {
        return n as f32;
    }
    static GUTTER: std::sync::OnceLock<nacelle::theme::TokenId> = std::sync::OnceLock::new();
    let id = *GUTTER.get_or_init(|| {
        nacelle::theme::id("layout.panel_gutter")
            .unwrap_or(nacelle::theme::TokenId::MISSING)
    });
    nacelle::theme::resolved().px(id)
}

pub fn grid_prefs() -> (bool, u32, u32, u32) {
    let g = conf().grid;
    // Whole pixels because this is the number the settings spinner edits;
    // the layout itself asks `panel_gutter` and gets the theme's own.
    let pad = panel_gutter(g.padding()).round() as u32;
    (g.snap(), g.cols(), g.rows(), pad)
}

pub fn set_grid_snap(on: bool) {
    update_conf(|c| c.grid.snap = Some(on));
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
    update_conf(|c| c.grid.cols = Some(n));
}

pub fn set_grid_rows(n: u32) {
    update_conf(|c| c.grid.rows = Some(n));
}

pub fn set_grid_padding(n: u32) {
    update_conf(|c| c.grid.padding = Some(n));
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

/// Screen diagonal in inches of the monitor with the given display
/// name, out of the picture size the monitor itself reports; 0 =
/// unknown.
///
/// The reading of the block moved to [`crate::screens::edid`] when the
/// same bytes started answering a second question — WHICH monitor this
/// is, not only how big — because two readers of one format is two
/// chances to read it differently.
pub fn monitor_diag_inches(monitor_name: &str) -> u32 {
    // Remembered per display name. A monitor does not change size
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
    // A Wayland compositor may hand winit `DP-1 Dell Inc. U2720Q`; the
    // socket is the first word, which is what names the file.
    let connector = monitor_name
        .split_whitespace()
        .next()
        .unwrap_or(monitor_name);
    crate::screens::read_edid(connector)
        .and_then(|e| e.diagonal_in())
        .map(|d| d.round() as u32)
        .unwrap_or(0)
}

/// Changes the USER's own configuration and writes it back.
///
/// The system files are read-only to the program — what a settings
/// click writes is always the user's own copy, which then outranks
/// them — and the document handed to `f` is the user's file ALONE for
/// the same reason: reading the cascade here would copy a
/// distribution's values into the user's file and freeze them there,
/// so an administrator changing a default would never reach that
/// machine again.
///
/// This is also the moment the configuration directory is created.
/// The program makes no directory at startup and installs nothing:
/// what is not installed is simply not offered, and the home
/// directory stays untouched until the user changes something.
///
/// The user's own file being in the OLD format is what makes this the
/// migration: it is read as it stands, changed, and written out as
/// RON beside it — every setting carried over in one step, and the old
/// file left exactly where it was.
///
/// A file of the user's own that does not parse is REPLACED rather
/// than refused. Refusing would leave the settings window silently
/// powerless on the one machine whose file needs fixing; replacing
/// without a copy would spend somebody's whole configuration on a typo.
/// So two things happen before the replacement, and neither is
/// optional: the text is put beyond reach of every later write, under
/// [`CONF_RON_RESCUE`], and the user is TOLD — at this moment, not at
/// the next start, because by the next start the file parses and there
/// is nothing left to notice.
///
/// Which makes the rule REPLACE WHAT HAS BEEN KEPT, and it decides the
/// case the paragraph above does not reach: a file that could not be
/// COPIED is not replaced at all. Nothing is known about such a file —
/// it was never read, so it may be a perfectly good configuration
/// behind a `chown` that went the wrong way — and replacing it needs
/// no permission on the file itself, only on the directory it sits in,
/// so the rename would land and take the lot. The settings window
/// keeps its power over a file that is merely wrong, which is the case
/// it was given that power for, and stops at one it cannot even see.
fn update_conf(f: impl FnOnce(&mut DesktopConf)) {
    update_conf_when(|c| {
        f(c);
        true
    });
}

/// The same, for a change that may turn out not to be one.
///
/// `f` answers whether it changed anything, and a `false` abandons the
/// write before a byte is serialised: no file, no directory, no memo,
/// nothing. The migration at startup is why this exists — it runs on
/// every machine at every start, and the overwhelming majority of them
/// have nothing to migrate, so a door that always wrote would install a
/// configuration file on a machine whose owner has never changed a
/// setting.
fn update_conf_when(f: impl FnOnce(&mut DesktopConf) -> bool) {
    let dir = config_dir();
    let path = dir.join(CONF_RON);
    // Seeded from the user's OWN configuration wherever it currently
    // lies — the family folder over the folder's old name — and not
    // from the family folder alone.
    //
    // Alone is what this used to read, and it carried nothing across
    // on exactly the machines the migration exists for: their old file
    // sits in ~/.config/nacelle-desktop/, so the family folder
    // answered "nothing", the first write produced a document holding
    // one setting, and the old file went on answering everything else
    // — out of reach of a reset, because nothing ever rewrites it.
    // Reading it here is what makes the write below the whole of the
    // user's configuration, which is in turn what lets `conf_dirs`
    // stop consulting the old folder.
    //
    // And only for as long as `conf_dirs` still consults it, which is
    // why the two are asked the SAME question here. A folder that has
    // been carried across and retired must stop being seeded from as
    // well: it is still on disk, so a reset — which takes fields OUT of
    // the new document — would find them all again on the next write
    // and put them back, and the setting the user cleared in the
    // morning would return the first time they touched a slider.
    //
    // The system end is deliberately absent: a value that came from
    // /etc/xdg must stay a system value, or the first setting anybody
    // changed would freeze that day's defaults into their home
    // directory forever — the exact trap the XDG arrangement exists to
    // avoid.
    let live = conf_dirs();
    let dirs = config_dirs();
    let seen = conf_stamps(&dirs);
    // What the LAST save decided, if nothing on disk has moved since.
    //
    // Not a saving of syscalls but a matter of not losing settings.
    // The save before this one may still be on its way to the disk —
    // that is the whole point of the writer thread — and a loop that
    // read the file now would seed from the document that write is
    // about to replace. Two presses of a slider a millisecond apart
    // would then put the first one's value back, and the file would end
    // up holding whichever write happened to lose the race.
    let held = conf_memo_seed(&dirs, &seen);
    let (mut doc, mut carried) = match &held {
        Some(seed) => (seed.mine.clone(), seed.carried),
        None => (DesktopConf::default(), true),
    };
    let mut keeps_nothing = false;
    for d in user_conf_dirs().iter().rev().filter(|_| held.is_none()) {
        if !live.contains(d) {
            continue;
        }
        match read_conf_dir(d) {
            Ok(Some(found)) => doc = found.over(doc),
            Ok(None) => {}
            // Only the file about to be REPLACED is rescued. A broken
            // one in the old folder is not being replaced by anything,
            // so it needs no copy; `cascade_conf` is what reports it.
            //
            // What it does need is to go on being read, and this is the
            // loop that decides that: the carry is happening HERE, and
            // a folder this loop could not read has not been carried
            // anywhere. Saying so is the whole of the mark below —
            // without it, the document about to be written would stand
            // in for a file none of whose bytes are in it.
            Err(said) => {
                if d == &dir {
                    if !rescue_unreadable(&path, &said) {
                        keeps_nothing = true;
                    }
                } else {
                    carried = false;
                }
            }
        }
    }
    // Before `f`, not after: there is no write to make and no document
    // to make it out of, and running the change would only invite a
    // reader to think one had happened.
    if keeps_nothing {
        return;
    }
    // A change that turned out not to be one leaves no trace at all —
    // see [`update_conf_when`]. Before the text and before the memo,
    // because both of those commit this program to a document.
    if !f(&mut doc) {
        return;
    }
    // THE BYTES BEFORE THE MEMO, and the order is the whole safety of
    // what follows. Filing the memo commits this program to answering
    // from it until a write settles or drops it; a save that dies
    // between those two acts leaves a document no disk has ever held
    // being handed to every reader for the rest of the session, with
    // the file itself no longer consulted — so the user's own editing
    // of it stops existing as well. Making the text HERE is what leaves
    // no order in which that can happen: the one failure that belongs
    // to the caller's thread is taken before there is anything to
    // strand, and past this line every path through [`write_conf_soon`]
    // ends at [`do_write_job`], which either settles the memo or drops
    // it.
    let text = match conf_text(&doc) {
        Ok(t) => t,
        // A document that will not serialise is a bug in this program
        // rather than a slow disk, and the sentence belongs to the
        // press that provoked it.
        Err(e) => return report_write(&path, Err(e)),
    };
    // The setting is IN FORCE from here, and the disk catches up behind
    // it. What the memo is given is not the document about to be
    // written — that one is the user's own rung alone — but that rung
    // laid over the system end, which is the answer [`conf`] gives and
    // therefore the only thing a reader may be handed.
    //
    // Filed before the write rather than after it because the write is
    // the slow half: 0.516 s of `fsync` across seven saves, measured
    // 2026-08-18, with a single save blocking the event loop for 0.35 s.
    // Nothing about the running program has to wait for a disk to
    // confirm what the user has already been told on screen.
    let effective = cascade_conf_over(&live, Some((dir.as_path(), &doc)));
    let seed = ConfSeed { mine: doc.clone(), carried };
    let serial = conf_memo_pending(&dirs, &effective, seed);
    write_conf_soon(&path, text, carried, serial);
}

/// The last write failure said out loud, so a slider held down is one
/// sentence and not one per keypress.
///
/// Cleared by a write that lands, which is what makes the NEXT failure
/// after a good spell worth saying again. `None` is therefore "the last
/// write worked", not "nothing has been written yet" — the two are the
/// same thing to every reader of it.
static WRITE_FAILED_SAID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Puts a write that could not happen where the user will see it.
///
/// A line on stderr was the whole of this, and a desktop session has no
/// stderr open: the user dragged the volume slider, the slider sprang
/// back, and there was nothing anywhere to explain it. Nothing already
/// on disk is lost — this is the branch that loses least, and that is
/// exactly why it was easy to leave silent — but EVERY change from here
/// on goes nowhere, which is a worse thing to find out by accident than
/// a file that was replaced.
///
/// It goes down [`CONF_RESCUED`] rather than [`CONF_ERROR`] because it
/// is an event and not a state: the next write may well work, and a
/// notice re-derived from a condition that has passed is a notice that
/// never clears. Same reason, same channel, same one-shot.
fn report_write(path: &Path, wrote: std::io::Result<()>) {
    let said = match wrote {
        Ok(()) => None,
        Err(e) => Some(format!(
            "the setting you just changed could NOT be saved \u{2014} {}: {e}. \
             Nothing on disk has been touched, and every change made from now \
             on will be lost when the program closes",
            path.display()
        )),
    };
    let Ok(mut last) = WRITE_FAILED_SAID.lock() else { return };
    if *last == said {
        return;
    }
    if let Some(s) = &said {
        remember_rescue(s.clone());
    }
    *last = said;
}

/// Puts an unreadable file out of harm's way and says so, naming the
/// copy that actually holds it.
///
/// What decides is the TEXT, not the presence of a file with the rescue
/// name on it. A name that is there already may be holding this same
/// text — the ordinary case, since a directory the program cannot write
/// to meets the same broken file on every keypress, and one text wants
/// one copy — or it may be holding somebody's configuration from
/// months ago, which is a file to step around rather than through.
/// Asking only whether the name is taken cannot tell those apart, and
/// gets one of them catastrophically wrong.
///
/// Answers whether the file is now safe to replace, which is the same
/// question as whether a copy of it exists — see [`update_conf`], whose
/// write this is the condition of.
fn rescue_unreadable(path: &Path, said: &str) -> bool {
    // Bytes rather than text: what is being kept is the file the user
    // has, and a copy that refuses anything an editor could leave
    // behind is a copy that is missing when it is wanted.
    let kept = match std::fs::read(path) {
        Ok(text) => keep_broken_text(path, &text),
        Err(e) => {
            eprintln!("nacelle-desktop: cannot read {} to keep it: {e}", path.display());
            None
        }
    };
    let notice = match &kept {
        Some(copy) => format!(
            "{said} \u{2014} so the setting you just changed has REPLACED it. \
             What you wrote is kept whole as {}: repair that file and put it \
             back under the old name",
            copy.display()
        ),
        // Says what the user can DO, because this is the one branch
        // where the program has stopped and will go on being stopped
        // until somebody moves. A sentence that only named the failure
        // would leave them clicking a control that does nothing.
        None => format!(
            "{said} \u{2014} and no copy of it could be kept, so the setting you \
             just changed has NOT been applied and nothing has been touched. \
             Make that file readable, or move it aside, and change the setting \
             again"
        ),
    };
    remember_rescue(notice);
    kept.is_some()
}

/// Finds `text` a name of its own beside `path` and answers with it —
/// or with the name that is already holding exactly this text.
///
/// The name is CLAIMED rather than checked and then written: an
/// `exists()` and a `copy()` are two moments, and a second copy of this
/// program — a settings window and a running desktop are two processes
/// — writing between them would have its rescue copy overwritten by
/// this one. `create_new` fails instead, and a failure here means the
/// name is taken, which is the question being asked anyway.
///
/// `None` when nothing could be kept, and the caller must say so rather
/// than name a file: a sentence pointing at a copy that is not there,
/// or holds somebody else's text, is worse than no sentence at all —
/// it is what sends a user to delete the wrong file.
fn keep_broken_text(path: &Path, text: &[u8]) -> Option<PathBuf> {
    use std::io::Write;
    for n in 1..=CONF_RESCUE_LIMIT {
        // The first copy keeps the plain name. A user who has one is
        // the common case and should not have to read a number.
        let name = if n == 1 {
            CONF_RON_RESCUE.to_string()
        } else {
            format!("{CONF_RON_RESCUE}.{n}")
        };
        let candidate = path.with_file_name(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(mut f) => {
                // The copy takes the ORIGINAL's permissions, and takes
                // them before a byte goes in. A new file is born at
                // whatever the umask allows, which is usually wider
                // than a configuration somebody deliberately closed
                // down; a rescue copy that is easier to read than the
                // file it rescues would be this program handing out
                // what it was asked to keep. The descriptor is already
                // open, so tightening it here does not stop the write.
                if let Ok(meta) = std::fs::metadata(path) {
                    let _ = f.set_permissions(meta.permissions());
                }
                // Flushed here rather than left to the system, for the
                // same reason the settings file is: the copy exists to
                // survive the thing that goes wrong next.
                if let Err(e) = f.write_all(text).and_then(|()| f.sync_all()) {
                    eprintln!(
                        "nacelle-desktop: cannot write {}: {e}",
                        candidate.display()
                    );
                    // A half-written copy under the rescue name is the
                    // one outcome worse than none, because the sentence
                    // would send the user to it. It never held anything
                    // else — this branch owns the name it just made.
                    let _ = std::fs::remove_file(&candidate);
                    return None;
                }
                return Some(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Taken. By this same text, in which case it is the
                // answer and nothing needs writing; otherwise by an
                // older one, and the search moves along rather than
                // touching it.
                if std::fs::read(&candidate).map(|kept| kept == text).unwrap_or(false) {
                    return Some(candidate);
                }
            }
            Err(e) => {
                eprintln!("nacelle-desktop: cannot keep {}: {e}", candidate.display());
                return None;
            }
        }
    }
    eprintln!(
        "nacelle-desktop: {} already holds {CONF_RESCUE_LIMIT} unreadable \
         configurations; delete the ones you no longer need",
        path.parent().unwrap_or(Path::new(".")).display()
    );
    None
}

/// The header every written file carries.
///
/// A serialiser cannot keep somebody's comments — they are not part of
/// the value — so the file has to say that itself, next to the name of
/// the copy that still has them. The alternative was to rewrite the
/// file by hand and preserve the text, which is exactly the fragility
/// this format was chosen to leave behind.
///
/// What the second paragraph promises is only true because of `ours`
/// in [`write_conf`]. It used to say "the copy from just before the
/// last write", which is a sentence that stops being true on the
/// SECOND write: two nudges of a slider and the copy is of this
/// program's own previous output. A header sending somebody to a file
/// for comments that are no longer in it is worse than one that says
/// nothing.
///
/// "However many settings you change afterwards" outlives the process
/// as well, and for a while it did not: see [`is_generated`], which is
/// how a file this program wrote LAST WEEK is recognised as its own.
///
/// WHICH MAKES THIS TEXT PART OF A PROMISE AND NOT ONLY PROSE. Editing
/// a word of it means putting the old text into [`CONF_HEADERS`], or
/// every file the previous release wrote stops being recognised and the
/// first save after the upgrade spends the user's copy on one of them.
/// A test refuses to let that be forgotten.
const CONF_HEADER: &str = "\
// nacelle-desktop settings \u{2014} Rusty Object Notation.
//
// The settings window REWRITES this file whenever something changes,
// and comments of your own do not survive that. What you wrote by hand
// is kept in nacelle-desktop.ron.bak: that copy is taken of a file this
// program did not write, and later saves leave it alone, so it stays
// your text however many settings you change afterwards.
//
// A field that is not here is answered by the system file
// (/etc/xdg/nacelle/nacelle-desktop.ron) and then by the program's own
// defaults. `Off` is different: it means \"nothing\", and it outranks a
// system file that names something.
";

/// Writes the document, keeping the previous file and never leaving a
/// half-written one.
///
/// The order is the whole of it: back up what is there, write the new
/// text under a temporary name, flush it to the disk, then rename it
/// into place. Renaming within a directory is atomic, so a machine that
/// loses power during this has either the old file or the new one —
/// never four hundred bytes of a document that no longer parses, which
/// for an all-or-nothing format is the same as having no settings at
/// all.
///
/// The two flushes are what make that sentence true of this code rather
/// than of a filesystem that happens to be kind. Without the first, the
/// rename can reach the disk while the bytes it points at have not, and
/// the file that survives a power cut is the new name over a hole.
/// Without the second, the rename itself is only in memory and the
/// setting is simply lost — which is the milder half, and still not
/// what the paragraph above promises. Both are best effort, for the
/// same reason the copy is: a filesystem that will not sync is not a
/// reason to refuse the setting the user just asked for.
///
/// Three things the first version of that order got wrong, each of
/// which loses a file rather than a setting:
///
/// * the copy was taken on EVERY write, so it was a copy of the
///   previous SAVE and not of the user's document — see [`ours`];
/// * the rename landed on the NAME the caller was given, which for a
///   symbolic link is the link and not the file it points at — see
///   [`follow_link`];
/// * the temporary was one fixed name shared by every process — see
///   [`claim_tmp`].
///
/// It runs on the WRITER THREAD and not on the one that changed the
/// setting, which is the whole of the 2026-08-18 measurement: the two
/// flushes below cost 0.516 s over seven saves, all of it on the event
/// loop, and the worst single save held the interface for 0.35 s (277 ms
/// on the file, 75 ms on the directory, back to back). Nothing about
/// the ORDER changed — see [`write_conf_soon`] for what moved and what
/// happens to a write the process does not live to finish.
fn write_conf(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    // Through the link before anything else, so that every path below —
    // the backup, the temporary, the rename and the directory flush —
    // is about the file the user actually keeps.
    let path = &follow_link(path);
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Read rather than `is_file`, because the question is not whether
    // something stands there but WHOSE it is. Bytes that will not come
    // back as text were certainly not written here, so they are copied
    // aside like any other stranger's rather than falling through the
    // check.
    let stranger = match std::fs::read_to_string(path) {
        Ok(old) => !ours(path, &old),
        Err(_) => path.is_file(),
    };
    if stranger {
        // Best effort: a backup that cannot be made is not a reason to
        // refuse the setting the user just asked for.
        // Beside the file, which after a link has been followed is not
        // the directory the caller named: a copy of somebody's dotfile
        // belongs next to that dotfile, not next to the link.
        if let Err(e) = std::fs::copy(path, path.with_file_name(CONF_RON_BACKUP)) {
            eprintln!("nacelle-desktop: cannot keep a copy of {}: {e}", path.display());
        }
    }
    let (tmp, mut f) = claim_tmp(path)?;
    // A temporary left behind under a name this function may hand out
    // again is a file the next write appends its luck to, so every way
    // out of here from this point takes its own litter with it.
    if let Err(e) = f.write_all(text.as_bytes()) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    let _ = f.sync_all();
    // The instrument, and only in a test build — see [`WriteStep`].
    #[cfg(test)]
    note_write_step(WriteStep::SyncFile);
    drop(f);
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(test)]
    note_write_step(WriteStep::Rename);
    // The directory entry, not the file: what is being made durable
    // here is the rename.
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(test)]
    note_write_step(WriteStep::SyncDir);
    remember_written(path, text);
    Ok(())
}

/// The document as the file carries it: the header this program writes
/// over every save, then the fields.
///

/// One save, waiting its turn on the durable writer.
struct WriteJob {
    path: PathBuf,
    text: String,
    /// Whether this write is the one that carries the user's old-named
    /// folder across — see [`mark_old_folder_carried`]. The mark may
    /// only be put down by a write that landed, so the decision travels
    /// with the job and is acted on where the result is known.
    carry: bool,
    /// Which memo entry these bytes belong to, so that a write landing
    /// after a LATER setting was changed does not stamp the memo with
    /// files that no longer say what it holds — see [`conf_memo_settle`].
    serial: u64,
}

/// The queue, and whether the writer is in the middle of a job.
///
/// `busy` is not derivable from the queue: a job taken out of it is not
/// finished, and [`flush_writes`] promises that when it returns the
/// disk carries everything that was asked for.
#[derive(Default)]
struct WriteDesk {
    queue: std::collections::VecDeque<WriteJob>,
    busy: bool,
    /// Set once the thread exists, so it is spawned by the first save
    /// and not by a program that never changes a setting.
    running: bool,
}

static WRITE_DESK: std::sync::Mutex<Option<WriteDesk>> = std::sync::Mutex::new(None);
static WRITE_BELL: std::sync::Condvar = std::sync::Condvar::new();

/// The desk, whether or not somebody panicked while holding it.
///
/// Poisoning is PERMANENT and process-wide: one panic under this lock
/// and every `lock()` for the rest of the session answers `Err`. Taking
/// that answer at face value cost the user everything they changed
/// afterwards — the save was declined without a word, and the memo
/// already filed for it went on handing that document to every reader
/// while the file itself was never consulted again. Settings stopped
/// being saved and hand-edits stopped being read, and nothing on screen
/// said why.
///
/// Refusing is only right where a panic can leave a half-made thing
/// behind, and nothing here can be half made: the only work done under
/// this lock is a push, a pop, a drain and two flags, with nothing that
/// unwinds standing between any of them. So the contents are as good
/// after somebody else's panic as before it, and this takes them.
fn lock_desk() -> std::sync::MutexGuard<'static, Option<WriteDesk>> {
    WRITE_DESK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sleeps at the desk until somebody rings, for the same reason
/// [`lock_desk`] takes a poisoned lock: a wait that gave up on a panic
/// elsewhere would strand the queue it was waiting on.
fn wait_at_desk(
    desk: std::sync::MutexGuard<'static, Option<WriteDesk>>,
) -> std::sync::MutexGuard<'static, Option<WriteDesk>> {
    WRITE_BELL.wait(desk).unwrap_or_else(|e| e.into_inner())
}

/// Hands a save to the durable writer and returns at once.
///
/// THE ORDER ON DISK IS UNCHANGED — exclusive temporary, write, flush
/// the file, rename, flush the directory — and so is everything that
/// order was for: a machine losing power mid-save has either the old
/// file or the new one. What changed is who waits for it. The 2026-08-18
/// `strace` found all fourteen `fsync` calls of a session on the event
/// loop, 0.516 s of them, one save alone holding the interface for
/// 0.35 s.
///
/// One thread and one queue, so two saves land in the order they were
/// made. A queue behind a lock rather than a channel because the desk
/// also has to answer "is there anything left", which is what
/// [`flush_writes`] is.
///
/// **A program that stops before its writes do.** The event loop calls
/// [`flush_writes`] on the way out, so an ordinary exit finishes every
/// save that was asked for. A process that is KILLED — `SIGTERM` from a
/// session manager at logout is the ordinary way — loses whatever is
/// still queued.
///
/// The WINDOW in which that can happen is the same window as before
/// this thread existed, and it is worth being exact about why: a save
/// is lost to a kill from the moment it is asked for until its rename
/// lands, and the rename waits on the same `fsync` either way. What
/// used to fill that window was a frozen interface; what fills it now
/// is an interface that has already shown the new value. So the change
/// is not a longer exposure but a briefly OPTIMISTIC one — the user is
/// told at 0 ms what used to be true at 350 ms — and a second save
/// arriving inside it queues behind the first rather than replacing it,
/// so a burst of distinct settings is exposed for as long as the burst
/// takes to drain. (Measured 2026-08-18: no control feeds the queue
/// faster than that; a dragged slider saves on release and not per
/// frame.) A handler that flushed on `SIGTERM` would close it, and
/// winit delivers no such event — the honest place for that is the
/// signal work, not here.
///
/// What cannot happen either way is half of one: the bytes reach a
/// temporary of this write's own and the rename is atomic, so the file
/// at the user's path is one whole document or the other, never a
/// mixture, and a temporary that was never renamed is litter rather
/// than a configuration.
///
/// TEXT and not a document, because the one failure a save can have on
/// the caller's own thread — a document that will not serialise — is
/// taken by [`update_conf`] BEFORE it files the memo. Past this point
/// every path ends at [`do_write_job`], which is what resolves the memo
/// either way; a `return` from here that skipped it would pin the memo
/// on a write that never happened.
fn write_conf_soon(path: &Path, text: String, carry: bool, serial: u64) {
    let job = WriteJob { path: path.to_path_buf(), text, carry, serial };
    let mut desk = lock_desk();
    let d = desk.get_or_insert_with(WriteDesk::default);
    d.queue.push_back(job);
    if !d.running {
        d.running = true;
        // Named, because a thread with no name is what the same audit
        // could not identify twice over.
        let spawned = crate::threads::spawn(crate::threads::CONF, write_desk_loop);
        if spawned.is_err() {
            // No thread to be had: the saves still have to happen, and
            // a blocked interface is better than a lost setting.
            d.running = false;
            let jobs: Vec<WriteJob> = d.queue.drain(..).collect();
            drop(desk);
            for job in jobs {
                do_write_job(job);
            }
            return;
        }
    }
    drop(desk);
    WRITE_BELL.notify_all();
}

/// Says the writer is gone, however it went.
///
/// The failure this exists for is a HANG AT QUIT: the event loop waits
/// for the desk to empty on the way out, and a writer that unwound out
/// of its loop would leave `busy` set and nobody to clear it. So the
/// state is put back by a destructor, which runs on the way out of a
/// panic as well — the next save spawns a fresh writer and the queue is
/// drained by that one instead.
struct WriterGone;

impl Drop for WriterGone {
    fn drop(&mut self) {
        {
            let mut desk = lock_desk();
            if let Some(d) = desk.as_mut() {
                d.busy = false;
                d.running = false;
            }
        }
        WRITE_BELL.notify_all();
    }
}

/// The durable writer: one job at a time, in the order they were made.
fn write_desk_loop() {
    let _gone = WriterGone;
    loop {
        let job = {
            let mut desk = lock_desk();
            loop {
                let Some(d) = desk.as_mut() else { return };
                if let Some(job) = d.queue.pop_front() {
                    d.busy = true;
                    break job;
                }
                // Nothing to do: say so — `flush_writes` may be waiting
                // on exactly this — and sleep until somebody rings.
                d.busy = false;
                WRITE_BELL.notify_all();
                desk = wait_at_desk(desk);
            }
        };
        do_write_job(job);
        {
            let mut desk = lock_desk();
            if let Some(d) = desk.as_mut() {
                d.busy = false;
            }
        }
        WRITE_BELL.notify_all();
    }
}

/// One job, start to finish, wherever it is being run.
fn do_write_job(job: WriteJob) {
    let wrote = write_conf(&job.path, &job.text);
    // After the write and only if it landed: a mark saying the old
    // folder is in the new file, put down by the write that actually
    // put it there. A write that failed carried nothing.
    if wrote.is_ok() && job.carry {
        mark_old_folder_carried();
    }
    if wrote.is_ok() {
        // The files now say what the memo already held, so the memo may
        // go back to checking itself against them.
        conf_memo_settle(job.serial);
    } else {
        conf_memo_forget(job.serial);
    }
    report_write(&job.path, wrote);
}

/// How long [`flush_writes`] waits on a writer that is still `running`
/// before giving up on it too — see the sentence beside the timed wait
/// below for what a thread can be stuck on that no panic ever ends.
const CONF_FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Waits until every save asked for so far is on the disk.
///
/// Called on the way out of the event loop, which is what turns "the
/// interface does not wait" into "and nothing is lost by that". A test
/// calls it for the same reason: an assertion about a file is an
/// assertion about a write that has finished.
pub fn flush_writes() {
    let deadline = std::time::Instant::now() + CONF_FLUSH_TIMEOUT;
    let mut desk = lock_desk();
    loop {
        let Some(d) = desk.as_ref() else { return };
        if d.queue.is_empty() && !d.busy {
            return;
        }
        // Waiting on a writer that is not there is a program that will
        // not quit, and quitting matters more than the last save: this
        // gives up out loud rather than hanging the desktop on its way
        // down. Only reachable through [`WriterGone`], which is to say
        // through a panic in the writer.
        if !d.running {
            // The lock goes back BEFORE the sentence, and that is not
            // tidiness. `eprintln!` panics when the write fails, and a
            // closed stderr is the ordinary state of a desktop session;
            // a panic here with the desk held used to poison it, after
            // which every save for the rest of the session was declined
            // in silence. The sentence is worth saying. It is not worth
            // the lock.
            let left = d.queue.len();
            drop(desk);
            eprintln!(
                "nacelle-desktop: {left} settings writes could not be finished \u{2014} \
                 the writer is gone"
            );
            return;
        }
        // A THREAD that answers is not the same as one that finishes:
        // `write_conf`'s `fsync`/rename can block for as long as the
        // filesystem underneath it does, and an unplugged USB disk or a
        // hung network mount under `$HOME` never panics — it just never
        // returns. `running` and `busy` stay true either way, so an
        // unbounded wait here would hang the desktop's own exit on a
        // disk it does not control. Past the deadline this gives up
        // exactly as it does above for a writer that is gone outright.
        let left_time = deadline.saturating_duration_since(std::time::Instant::now());
        if left_time.is_zero() {
            let left = d.queue.len();
            drop(desk);
            eprintln!(
                "nacelle-desktop: {left} settings writes could not be finished \u{2014} \
                 the writer did not answer in time"
            );
            return;
        }
        let (guard, _) = WRITE_BELL
            .wait_timeout(desk, left_time)
            .unwrap_or_else(|e| e.into_inner());
        desk = guard;
    }
}

/// THE INSTRUMENT ON A SAVE — a test build and no other.
///
/// What this module changed on 2026-08-18 is not visible in any value
/// the program computes: the order of the three durable steps and the
/// thread they ran on ARE the fix, and a test that could only read the
/// finished file would pass just as well on the arrangement that held
/// the interface for a third of a second. So the steps are written
/// down as they happen.
///
/// Behind `cfg(test)` rather than behind an `allow(dead_code)`, which
/// is the same statement made honestly: nothing the shipped program
/// does reads any of this, and a mutex, a thread id and a `Vec` push
/// three times per save are not a cost a user should carry for a test.
/// The three lines in [`write_conf`] that feed it carry the same
/// attribute; they compile to nothing at all.
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WriteStep {
    SyncFile,
    Rename,
    SyncDir,
}

/// What the last save did and where. Three enum values and a thread
/// id, rewritten seven times in a session.
#[cfg(test)]
static LAST_WRITE: std::sync::Mutex<Vec<(WriteStep, std::thread::ThreadId)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(test)]
fn note_write_step(step: WriteStep) {
    let Ok(mut log) = LAST_WRITE.lock() else { return };
    if step == WriteStep::SyncFile {
        log.clear();
    }
    log.push((step, std::thread::current().id()));
}

/// The steps the last save took, with the thread that took them.
#[cfg(test)]
fn last_write_steps() -> Vec<(WriteStep, std::thread::ThreadId)> {
    LAST_WRITE.lock().map(|l| l.clone()).unwrap_or_default()
}

/// The file a name finally stands for, links and all.
///
/// A configuration kept in somebody's dotfiles repository and linked
/// into place is an ordinary arrangement, and writing to the LINK ends
/// it: `rename` replaces the link with a plain file, the values survive
/// the day it happens, and every edit the user makes in their
/// repository from then on goes nowhere. Nothing is said, because from
/// this program's side nothing went wrong — which is the exact shape of
/// "loses settings and does not know when".
///
/// `canonicalize` is not what is wanted here: it fails on a link
/// pointing at a file that does not exist yet, which is how a fresh
/// checkout of a dotfiles repository looks before its first write, and
/// falling back to the link itself in that case would replace it. This
/// walks the chain by hand instead, relative targets resolved against
/// the directory the link sits in, and answers the last name in the
/// chain whether or not anything stands there.
///
/// The bound is against a loop — a link to itself is a file a user can
/// make in one command — and reaching it answers the last name looked
/// at, which the write then fails on honestly rather than hanging.
fn follow_link(path: &Path) -> PathBuf {
    let mut at = path.to_path_buf();
    for _ in 0..CONF_LINK_LIMIT {
        let Ok(target) = std::fs::read_link(&at) else { return at };
        at = if target.is_absolute() {
            target
        } else {
            at.parent().unwrap_or(Path::new(".")).join(target)
        };
    }
    at
}

/// How many links deep a configuration file may be. Well past a
/// dotfiles repository behind a `stow`, and short of a loop costing
/// somebody their keypress.
const CONF_LINK_LIMIT: u32 = 16;

/// A temporary file of THIS write's own, created exclusively.
///
/// The name used to be one constant, and `File::create` TRUNCATES: a
/// settings window and a running desktop are two processes — the code
/// says so itself, in [`keep_broken_text`], and uses `create_new` there
/// for exactly this reason — so both could hold the same name, and the
/// rename of whichever finished first would publish a file the other
/// was still writing. An all-or-nothing format turns that into no
/// settings at all.
///
/// The process id makes the name unique among the processes that are
/// ALIVE, which is the whole of the race; the counter is for the
/// leftover of a dead process that happened to have this id, and for a
/// second write from this one before the first is renamed away.
/// `create_new` is what makes it a claim rather than a guess.
fn claim_tmp(path: &Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    let stem = path.file_name().and_then(|n| n.to_str()).unwrap_or(CONF_RON);
    let mut last = None;
    for n in 0..CONF_TMP_TRIES {
        let candidate =
            path.with_file_name(format!("{stem}{CONF_RON_TMP}{}.{n}", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(f) => return Ok((candidate, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no temporary name was free")
    }))
}

/// How many temporary names one write will try before giving up. Each
/// one taken is a leftover of a process that died mid-write, so a
/// handful is already a machine with a story.
const CONF_TMP_TRIES: u32 = 8;

/// One file this program wrote, kept so the backup it takes is never a
/// copy of its own output.
struct WrittenConf {
    path: PathBuf,
    text: String,
}

/// The texts [`write_conf`] last put at each path.
static CONF_WRITTEN: std::sync::Mutex<Vec<WrittenConf>> = std::sync::Mutex::new(Vec::new());

/// How many paths are remembered. One machine writes one file; the
/// bound is against a process that keeps being pointed at new
/// directories.
const CONF_WRITTEN_MAX: usize = 64;

/// Whether what stands at `path` is byte for byte what this program
/// last wrote there.
///
/// The measured reason, which is the same one `nacelle::settings` has
/// written down for addon files and which the program's OWN file never
/// got: a `.bak` refreshed on every write is a backup of the previous
/// SAVE, not of the user's document. A hand-written file with comments
/// in it — and possibly a field a newer build knows and this one drops
/// on the way through serde — is replaced on the first save, so `.bak`
/// holds it; the second save, which is one more press of an arrow key
/// on a slider, replaces `.bak` with the first save's output. Two
/// keypresses and the user's text is nowhere: not in the file, not in
/// the copy, and the header in the file is still pointing at the copy.
///
/// So the rule is not "keep the previous contents" but KEEP WHAT THIS
/// PROGRAM DID NOT WRITE. One file, no generations, no pruning, holding
/// the only version that was ever irreplaceable.
///
/// The table is asked FIRST and is not the whole answer, because it
/// lives in this process and the file does not. A program restarted
/// between two saves used to find an empty table, read its own file
/// from last week as a stranger's, and copy it over the `.bak` that
/// held the only hand-written version there was — the exact loss the
/// paragraph above describes, one restart wide instead of one keypress
/// wide, and the header in the file went on pointing at the copy. So
/// the second question is asked of the BYTES, by [`is_generated`], and
/// the table stays for the case that one cannot cover: a file this
/// process wrote and something else has since changed underneath it.
///
/// The bound above EVICTS rather than refuses for the same reason: the
/// entry a full table would have dropped is the one just written,
/// which is the only one anybody is about to ask about.
fn ours(path: &Path, on_disk: &str) -> bool {
    let remembered = CONF_WRITTEN
        .lock()
        .map(|w| w.iter().any(|e| e.path == path && e.text == on_disk))
        .unwrap_or(false);
    remembered || is_generated(on_disk)
}

/// EVERY HEADER THIS PROGRAM HAS EVER WRITTEN, newest first, and the
/// reason [`is_generated`] does not simply compare against the current
/// one.
///
/// Recognition is a question about a file written by SOME build of this
/// program, not necessarily this one, so anything the recognition rests
/// on is frozen the moment it ships. The prose above is not frozen —
/// it is prose, and the paragraph explaining the `.bak` was rewritten
/// twice while this very defect was being fixed. Compared against the
/// current text alone, the first save after any release that touches a
/// word of it reads last week's file as a stranger's and spends the
/// user's only copy on it: the same loss as the one below, one release
/// wide instead of one restart wide.
///
/// So editing [`CONF_HEADER`] means COPYING ITS OLD TEXT IN HERE, as a
/// new entry after the current one, and files that went out wearing it
/// go on being recognised. The test
/// `the_recognition_is_pinned_to_what_was_shipped` fails the moment the
/// header or the serialiser's output changes without that being done,
/// and says so; it is a tripwire and nothing else.
const CONF_HEADERS: &[&str] = &[CONF_HEADER];

/// Whether `text` is EXACTLY what some build of [`write_conf`] produces
/// from the document inside it: read back, written out again, compared
/// byte for byte under each header in [`CONF_HEADERS`].
///
/// The question this has to answer is not "does it look like ours" but
/// "is there anything of the USER'S in it", and the round trip answers
/// that one precisely, because everything a person can add is
/// something the serialiser drops: a comment, a field a newer build
/// knows and this one has never heard of, a number written `40` where
/// the writer writes it differently, one blank line more than the
/// writer leaves. Any of them and the two texts differ, the file is
/// treated as a stranger's, and the copy is taken — which is the side
/// to be wrong on.
///
/// Matching the HEADER alone would have been shorter and wrong in the
/// one case that matters: a person who opens the file the program
/// wrote and adds their own lines under that header would have had
/// them silently replaced with no copy kept at all. Ignoring the header
/// and comparing only what follows it would have been wrong in the same
/// case for the same reason — the line they added is likeliest to be a
/// comment, and a comment goes at the top.
fn is_generated(text: &str) -> bool {
    is_generated_under(text, CONF_HEADERS)
}

/// [`is_generated`] with the list of known headers handed in, so that
/// what the list DOES can be stated by a test — the const holds exactly
/// one entry today, and the whole point of it is what happens on the
/// day it holds two.
fn is_generated_under(text: &str, headers: &[&str]) -> bool {
    let Ok(doc) = ron_options().from_str::<DesktopConf>(text) else { return false };
    let Ok(body) = conf_body(&doc) else { return false };
    headers
        .iter()
        .any(|header| text.strip_prefix(*header) == Some(body.as_str()))
}

/// The bytes of a written configuration: the header, then [`conf_body`].
///
/// Called on the CALLER's thread, before the write is handed on, because
/// it is the one part of a save that can fail there — a document that
/// will not serialise is a bug in this program, not a slow disk — and
/// because the writer is handed TEXT: a `DesktopConf` crossing a thread
/// boundary would be cloned for no reason, and the bytes are what is
/// being made durable.
///
/// One function because two callers must agree forever — [`write_conf`]
/// produces this and [`is_generated`] recognises it — and a difference
/// of a single byte between them would turn every save into a backup
/// of the previous save, which is the defect this whole page is about.
fn conf_text(doc: &DesktopConf) -> std::io::Result<String> {
    Ok(format!("{CONF_HEADER}{}", conf_body(doc)?))
}

/// The document and the newline that ends the file — everything a
/// written configuration is APART from its header.
///
/// Split off because the header is the part that changes between
/// releases and this part is the part that carries the meaning:
/// [`is_generated`] compares this against what follows each header it
/// knows, so a file from an older build is recognised by the bytes that
/// were never prose.
fn conf_body(doc: &DesktopConf) -> std::io::Result<String> {
    let body = ron_options()
        .to_string_pretty(doc, ron_pretty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(format!("{body}\n"))
}

/// Remembers the text [`write_conf`] just wrote, for [`ours`].
fn remember_written(path: &Path, text: &str) {
    let Ok(mut w) = CONF_WRITTEN.lock() else { return };
    if let Some(e) = w.iter_mut().find(|e| e.path == path) {
        e.text = text.to_string();
        return;
    }
    if w.len() >= CONF_WRITTEN_MAX {
        w.remove(0);
    }
    w.push(WrittenConf { path: path.to_path_buf(), text: text.to_string() });
}

/// How RON is read and written here.
///
/// `implicit_some` is on for BOTH, and on by default rather than by
/// the `#![enable(…)]` line a file may carry: it lets a person write
/// `volume: 80` where the type says `Option<u32>`, and `Some(80)` goes
/// on parsing as well. A configuration file is written by hand at
/// least as often as by the program, and `Some(…)` around every number
/// is a Rust detail leaking into somebody's evening.
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

fn ron_pretty() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        // The type's name is this program's business, not the file's.
        .struct_names(false)
        .indentor("    ")
        .extensions(ron::extensions::Extensions::IMPLICIT_SOME)
}

// ---------------------------------------------------------------- clearing
//
// Writing an EMPTY value and REMOVING a field are two different acts,
// and the difference only shows on a machine that has a system file.
// An empty value wins the cascade and pins the setting off; a removed
// field lets the system file answer. The old format could only do the
// first, so LOOK AND FEEL RESET wrote empties and was correct by
// accident — there was no system file for them to beat. Installing one
// is what would have made it silently wrong, which is why the removing
// half lands in the same change as the format.

/// Everything to do with look and feel, taken back to what this
/// machine looks like with no configuration of the user's at all.
///
/// One write rather than a dozen: the fields go out TOGETHER, so no
/// intermediate state of the file is ever on disk, and a crash in the
/// middle cannot leave half a reset behind. Every field is REMOVED —
/// `variant:` and the two font sizes included, which is precisely what
/// the old format could not express: an empty variant is a documented
/// explicit off, and a size cannot be written empty at all.
pub fn clear_look_and_feel() {
    update_conf(|c| {
        c.theme = Choice::Inherit;
        c.variant = Choice::Inherit;
        // The default arrangement AND every screen given one of its
        // own: clearing the first alone would leave a second monitor
        // pinned to whatever its entry says, which is precisely the
        // setting the user cannot see from that page.
        c.layaut = Choice::Inherit;
        c.screens.clear();
        c.sounds = Choice::Inherit;
        c.term_font = model::FontConf::default();
        c.ui_font = model::FontConf::default();
        // The band around every panel. It is typed on the GRID page
        // rather than this one, which is why it was missed, but the
        // page a control sits on does not decide what it IS: this
        // field is an override of the theme's `layout.panel_gutter`
        // and of nothing else, so a number left standing here would
        // leave the user's own spacing around a reset look, from a
        // theme token they never chose to overrule.
        c.grid.padding = None;
    });
}

/// Takes back every per-screen assignment, leaving the default
/// arrangement alone — and leaving the MAIN SCREEN role alone with it,
/// the role being a different setting about the same screens and not a
/// layaut. [`clear_main_screen`] is that one.
#[allow(dead_code)]
pub fn clear_screen_layauts() {
    update_conf(|c| c.screens.clear());
}

/// Takes the MAIN SCREEN setting out of the file altogether, which is
/// the ONE answer [`set_main_screen`] cannot write.
///
/// Three states, three ways in: a name and an explicit "the display
/// server's, whatever it is" are both [`set_main_screen`]'s, and this
/// is the third — the question handed back to the rest of the cascade,
/// so that a system file naming a screen is heard again. Removing the
/// field is what says that, because an empty value would be the `Off`
/// above and would go on outranking it.
#[allow(dead_code)]
pub fn clear_main_screen() {
    update_conf(|c| c.main_screen = Choice::Inherit);
}

/// One level of a search path: `<base>/nacelle` and, directly behind
/// it, `<base>/nacelle-desktop`.
///
/// The pair is kept together at EVERY level rather than all the legacy
/// directories being appended at the end, and that is the whole
/// correctness of the fallback. Both trees merge by precedence — the
/// configuration key by key, the data file by file — so the user's own
/// old-named directory has to keep outranking the system defaults.
/// Appending would quietly reverse that and let a distribution's
/// `/etc/xdg` answer a key the user had set years ago.
///
/// This is the SEARCH PATH. The configuration document drops one rung
/// of it once that rung has been carried across — see [`conf_dirs`],
/// which is the only place that happens and the only thing it happens
/// to.
fn push_level(out: &mut Vec<PathBuf>, base: &Path) {
    for name in [FAMILY_DIR, LEGACY_FAMILY_DIR] {
        let dir = base.join(name);
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
}

/// Where the user's own configuration lives: `$XDG_CONFIG_HOME`, or
/// `~/.config`. The BASE, without the family directory — the search
/// path needs it to build both names.
fn config_home() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config")
}

/// The one configuration directory anything is ever WRITTEN to:
/// `$XDG_CONFIG_HOME/nacelle`, or `~/.config/nacelle`.
///
/// The new name only. The old one is read and never written, so a
/// machine keeps exactly the files it had and gains one directory the
/// first time a setting is changed.
fn config_dir() -> PathBuf {
    config_home().join(FAMILY_DIR)
}

/// Every directory the configuration is READ from, most specific
/// first: the user's own, then the system ones from `XDG_CONFIG_DIRS`
/// (or `/etc/xdg` when it is unset), each of them under the family name
/// and then under the old one.
///
/// The counterpart of [`data_dirs`] for configuration, and the reason
/// a package can ship defaults: they are read where they are
/// installed, never copied to the user.
fn config_dirs() -> Vec<PathBuf> {
    let dirs = config_search_path(
        &config_home(),
        std::env::var("XDG_CONFIG_DIRS").ok().as_deref(),
    );
    warn_once_about_legacy("configuration", &dirs, &LEGACY_CONFIG_SAID);
    dirs
}

/// [`config_dirs`] without the environment: the user's base first, then
/// `system` split on ':', every one of them contributing the pair of
/// directories [`push_level`] builds, duplicates dropped. An unset or
/// empty value means the standard `/etc/xdg`, as the XDG base directory
/// specification says.
fn config_search_path(user: &Path, system: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    push_level(&mut out, user);
    let system = system.filter(|v| !v.is_empty()).unwrap_or("/etc/xdg");
    for base in system.split(':').filter(|b| !b.is_empty()) {
        push_level(&mut out, Path::new(base));
    }
    out
}

/// Where the user's own data lives: `$XDG_DATA_HOME`, or
/// `~/.local/share`. The BASE, as [`config_home`] is for configuration.
fn data_home() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local").join("share")
}

/// Data directory: `~/.local/share/nacelle`. Holds everything a theme is
/// made of (layauts/, sounds/, addons/) — those are data, not
/// configuration, so they belong under XDG_DATA_HOME while
/// nacelle-desktop.ron stays in the config directory. The one data
/// directory written to, and the new name only.
fn data_dir() -> PathBuf {
    data_home().join(FAMILY_DIR)
}

/// Every directory assets are READ from, most specific first: the
/// user's own, then the system ones from XDG_DATA_DIRS (or the two
/// standard prefixes when it is unset), each under the family name and
/// then under the old one.
///
/// This is what makes `sudo make install` and a distribution package
/// mean something, while a user install still shadows both: the first
/// directory holding a given name wins, and nothing has to be copied
/// anywhere for that to work. A sound set or a layaut installed under
/// the old name is found by the same rule, one place further down.
fn data_dirs() -> Vec<PathBuf> {
    let dirs = data_search_path(&data_home(), std::env::var("XDG_DATA_DIRS").ok().as_deref());
    warn_once_about_legacy("data", &dirs, &LEGACY_DATA_SAID);
    dirs
}

/// [`data_dirs`] without the environment, so a test can read it.
fn data_search_path(user: &Path, system: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    push_level(&mut out, user);
    let system = system
        .filter(|v| !v.is_empty())
        .unwrap_or("/usr/local/share:/usr/share");
    for base in system.split(':').filter(|b| !b.is_empty()) {
        push_level(&mut out, Path::new(base));
    }
    out
}

/// The data tree as the toolkit sees it: every directory it may READ —
/// both names, user before system — and the single new-named directory
/// it WRITES to.
///
/// `AssetRoots::xdg` cannot serve here any more. It builds a search
/// path from ONE name, and the point of this arrangement is that a
/// machine which installed its sounds and its layauts under the old
/// name keeps them without moving a file.
fn asset_roots() -> AssetRoots {
    AssetRoots::new(data_dirs(), data_dir())
}

/// Whether a search-path directory is one under the folder's old name.
fn is_legacy_dir(dir: &Path) -> bool {
    dir.file_name().and_then(|n| n.to_str()) == Some(LEGACY_FAMILY_DIR)
}

/// Said once per tree, and that is the requirement rather than a
/// nicety: [`config_dirs`] and [`data_dirs`] are called on every read,
/// so a line printed unguarded would be a line per settings click and
/// per frame that asks for a sound.
static LEGACY_CONFIG_SAID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LEGACY_DATA_SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Names the first old-named directory on `path` that actually exists
/// and says where it belongs from now on — once.
///
/// The flag is claimed BEFORE the filesystem is touched, so the whole
/// question is asked once per process and every later call is one
/// atomic read: `find_asset` is on the path of a settings page that
/// redraws every frame, and a stat per root per frame would be a cost
/// paid for a sentence already printed. What that trades away is
/// noticing an old directory CREATED mid-session, which cannot happen
/// to a directory that only exists from before the rename.
///
/// Returns whether it printed, which is what a test can count: stderr
/// is not capturable from here, and the failure worth catching is not
/// the wording but a warning that comes back every frame.
fn warn_once_about_legacy(
    tree: &str,
    path: &[PathBuf],
    said: &std::sync::atomic::AtomicBool,
) -> bool {
    use std::sync::atomic::Ordering;
    if said.swap(true, Ordering::Relaxed) {
        return false;
    }
    let Some(old) = path.iter().find(|d| is_legacy_dir(d) && d.is_dir()) else {
        return false;
    };
    eprintln!(
        "nacelle-desktop: reading {tree} from {} \u{2014} the folder's old name. \
         Nothing has been moved and nothing has to be: it goes on being read \
         there. Its place from now on is {}, one folder for the whole nacelle \
         family",
        old.display(),
        old.with_file_name(FAMILY_DIR).display()
    );
    true
}

/// Merges the configuration directories given MOST SPECIFIC FIRST: an
/// earlier document wins field by field, and a field only a later one
/// carries is inherited. A directory with no configuration in it
/// contributes nothing, which is the normal case on both ends — a
/// machine with no system defaults and a user who has never changed a
/// setting are both perfectly ordinary.
///
/// `Off` is an answer, not an absence: a LUT switched off in the
/// user's file has to beat a system file that names one.
///
/// Takes its directories rather than reading the environment, so a
/// test hands it two temporary ones and no process-wide state is
/// touched.
fn cascade_conf(dirs: &[PathBuf]) -> DesktopConf {
    cascade_conf_over(dirs, None)
}

/// [`cascade_conf`], with one rung's document HELD rather than read.
///
/// The write path is the only caller with something to hold: it has
/// just decided what the user's own file is going to say, and the disk
/// does not carry those bytes yet — the durable write is on its way to
/// another thread. Every other rung is read exactly as it always was,
/// and everything a full read reports is still reported, which is the
/// reason this is the same function and not a second copy of it: a
/// broken file at the system end has to be named whether or not the
/// user has just changed a setting.
fn cascade_conf_over(dirs: &[PathBuf], held: Option<(&Path, &DesktopConf)>) -> DesktopConf {
    let mut out = DesktopConf::default();
    let mut bad: Option<String> = None;
    for dir in dirs.iter().rev() {
        if let Some((at, doc)) = held {
            if dir == at {
                out = doc.clone().over(out);
                continue;
            }
        }
        match read_conf_dir(dir) {
            Ok(Some(doc)) => out = doc.over(out),
            Ok(None) => {}
            // Least specific first, so the last one assigned is the
            // most specific broken file — the user's own, when it is
            // theirs, which is the one they can do something about.
            //
            // The consequence is spelled out here because it is this
            // caller's: reading goes ON past the file, so every
            // setting in it counts as unset and the rest of the
            // cascade answers in its place.
            Err(said) => {
                bad = Some(format!(
                    "{said} \u{2014} it is being ignored WHOLE, so every setting \
                     in it counts as unset"
                ))
            }
        }
    }
    // Set on EVERY read, so a file that has been repaired stops being
    // complained about without the program having to be restarted.
    remember_conf_error(bad);
    out
}

/// One directory's configuration: its `.ron` file, or — where there is
/// none — the `Key=Value` file that came before it.
///
/// Per DIRECTORY, so the two formats can stand on different rungs of
/// the same cascade: a distribution still shipping the old format is
/// answered by a user who has already moved on, and neither has to
/// wait for the other. Nothing here writes, renames or deletes: the
/// old file goes on being read exactly where it lies until a `.ron`
/// appears beside it.
///
/// `Ok(None)` when the directory says nothing at all — which means no
/// `.ron` THERE, not a `.ron` that would not come open. `Err` carries
/// the sentence the user should see whenever a file exists and could
/// not be turned into a document, whether the obstacle was the syntax,
/// the permissions or the bytes.
///
/// That sentence names the file and the place in it and STOPS there,
/// without saying what follows: the two callers do different things
/// with the same broken file — one reads past it, the other replaces
/// it — and each states its own consequence. One sentence covering
/// both would have to be true of neither.
fn read_conf_dir(dir: &Path) -> Result<Option<DesktopConf>, String> {
    let ron = dir.join(CONF_RON);
    match std::fs::read_to_string(&ron) {
        Ok(text) => {
            note_conf_file_read();
            return match ron_options().from_str::<DesktopConf>(&text) {
                Ok(doc) => {
                    warn_once_about_dead_conf(dir);
                    Ok(Some(doc))
                }
                Err(e) => Err(format!(
                    "{} could not be read at line {}, column {}: {}",
                    ron.display(),
                    e.span.start.line,
                    e.span.start.col,
                    e.code
                )),
            };
        }
        // NOT THERE is the only reading of a failure that means this
        // directory says nothing — and it is the ordinary one, since a
        // machine with no system defaults and a user who has never
        // changed a setting both land here. Everything else is a file
        // that EXISTS and could not be had: the wrong permissions on
        // it, an I/O error, or bytes that are not text at all, which is
        // what a filesystem hands back after replaying a journal over a
        // write it never finished.
        //
        // Reading those as silence is the quiet half of the loss this
        // whole arrangement is about: the caller that WRITES would take
        // the directory for empty and rename a one-field document over
        // somebody's settings, with no copy kept, because nothing ever
        // told it there was a file there. An `Err` costs one popup on a
        // machine where something is genuinely wrong and saves the file
        // on the one where it is.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{} could not be read: {e}", ron.display())),
    }
    let legacy = dir.join(CONF_FILE);
    let Ok(text) = std::fs::read_to_string(&legacy) else { return Ok(None) };
    note_conf_file_read();
    warn_once_about_conf_format(&legacy);
    Ok(Some(DesktopConf::from_legacy(&parse_kv(&text))))
}

/// The last thing found wrong with a configuration file, in the words
/// the user gets on screen.
///
/// A parse error may NOT degrade quietly to the defaults. RON is
/// parsed all or nothing, so ONE misplaced bracket costs the whole
/// file where the old format lost a single line — and a program that
/// answered that with its factory appearance and no sentence would
/// leave the user nothing to connect it to. So it is said in the log
/// and carried to [`resolve`], which puts it on the screen.
static CONF_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Remembers what the last read found, and says it once per distinct
/// message: the configuration is read whenever the grid editor draws,
/// and a sentence per frame is a sentence nobody reads.
fn remember_conf_error(said: Option<String>) {
    let Ok(mut last) = CONF_ERROR.lock() else { return };
    if *last == said {
        return;
    }
    if let Some(s) = &said {
        eprintln!("nacelle-desktop: {s}");
    }
    *last = said;
}

/// What the last read had to say about a broken file, if anything.
fn conf_error() -> Option<String> {
    CONF_ERROR.lock().ok().and_then(|e| e.clone())
}

/// A file that was replaced because it could not be read, waiting to be
/// put in front of the user.
///
/// A channel of its own rather than [`CONF_ERROR`], because the two
/// have opposite lifetimes. The error describes a file that is still
/// broken, so it is re-derived on every read and clears itself the
/// moment the file is repaired. This describes something that has
/// ALREADY happened and is already undone — one write later the file
/// parses, the error is gone, and the only trace is a rescue copy the
/// user has no reason to look for. So it is held until it has been
/// shown, and it is taken rather than read: an event is reported once,
/// and a popup that came back on every settings click would be a popup
/// people learn to dismiss unread.
static CONF_RESCUED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn remember_rescue(notice: String) {
    let Ok(mut pending) = CONF_RESCUED.lock() else { return };
    // The log gets it immediately and unconditionally: a headless start
    // has no popup, and this is the one line explaining why a machine
    // came up looking factory-fresh.
    eprintln!("nacelle-desktop: {notice}");
    // First one wins. A second unreadable file cannot happen without
    // the user having repaired the first, and the pending sentence
    // names the rescue copy that is actually on disk.
    if pending.is_none() {
        *pending = Some(notice);
    }
}

/// The pending rescue notice, if there is one, and it is gone once
/// taken. Both the startup path and every configuration re-apply ask,
/// so whichever comes first after the write shows it.
pub fn take_conf_rescued() -> Option<String> {
    CONF_RESCUED.lock().ok().and_then(|mut p| p.take())
}

/// Said once per directory: a `Key=Value` file lying beside a `.ron`
/// one is not merged with it, it is not read AT ALL.
///
/// The installer prints this for `/etc/xdg` at the moment it writes the
/// new file, which covers a site's own defaults. It cannot cover the
/// user's own directory, because no installer ever writes there — the
/// old file gets there by having been there, and the `.ron` beside it
/// appears the first time a setting is changed. So the program says it
/// too, from the one place that can see both files: whoever put site
/// defaults in the old file has to move them across by hand, and until
/// they do the file is furniture.
///
/// Keyed by DIRECTORY rather than said once overall, because the two
/// ends of the cascade are two different people's problem.
///
/// Returns whether it printed, which is what a test can count:
/// `read_conf_dir` is on the path of a settings page that redraws every
/// frame, and the failure worth catching is not the wording but a line
/// that comes back forever.
static DEAD_CONF_SAID: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

fn warn_once_about_dead_conf(dir: &Path) -> bool {
    let legacy = dir.join(CONF_FILE);
    // BOTH files, and the pair is the whole condition. An old file
    // standing alone is being read, and `warn_once_about_conf_format`
    // is what has something to say about that one; it only goes dead
    // the moment a `.ron` appears in the same directory.
    if !legacy.is_file() || !dir.join(CONF_RON).is_file() {
        return false;
    }
    {
        let Ok(mut said) = DEAD_CONF_SAID.lock() else { return false };
        if said.iter().any(|d| d == dir) {
            return false;
        }
        said.push(dir.to_path_buf());
    }
    eprintln!(
        "nacelle-desktop: {} is no longer read \u{2014} within one directory the \
         {CONF_RON} beside it answers WHOLE, and the two formats are never \
         merged. Nothing has been deleted; anything still wanted out of that \
         file has to be moved across by hand",
        legacy.display()
    );
    true
}

/// Said once: the configuration is read on every settings click and on
/// every frame the grid editor draws.
static LEGACY_CONF_SAID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Names the old-format file being read and where its contents are
/// headed. Nothing is converted here — the file is read as it is, and
/// the day a setting changes the new one is written beside it.
fn warn_once_about_conf_format(path: &Path) {
    use std::sync::atomic::Ordering;
    if LEGACY_CONF_SAID.swap(true, Ordering::Relaxed) {
        return;
    }
    eprintln!(
        "nacelle-desktop: reading {} \u{2014} the format that came before \
         {CONF_RON}. Nothing has been converted and nothing has to be: it goes \
         on being read there, and the first setting you change writes {} \
         beside it, which then answers first",
        path.display(),
        path.with_file_name(CONF_RON).display()
    );
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




/// Serialises every test that writes an XDG variable — IN THIS CRATE,
/// not in this module.
///
/// `std::env::set_var` is PROCESS-wide while `cargo test` runs its
/// tests on many threads: one test pointing XDG_CONFIG_HOME at its own
/// directory silently redirected another test's `resolve()` half way
/// through, and the theme-switch test read somebody else's
/// configuration and saw the wrong accent. Nothing about that was
/// visible in the failure — the colour was simply not the one the theme
/// names. Per-PID directories are not enough; the variable itself is
/// the shared thing.
///
/// It lives out here rather than inside `mod tests` because the
/// variable it guards is not this module's either. A settings-window
/// test that opens a page reaches [`active_sounds_dir`] and so reads
/// the same environment, and a second lock would guard nothing: one
/// process, one variable, one lock.
#[cfg(test)]
pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    // A poisoned lock only means some other test panicked while holding
    // it; the variable it set is being overwritten anyway.
    L.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    // A widget KIND: only the tests still name one directly — the
    // interface below speaks in instance identities.
    use crate::widgets::{LayoutMode, Panel};
    // What a screen is called. The rules below are about screens, and a
    // test binary has no sockets to plug a monitor into, so every one of
    // them builds the identity it is talking about.
    use crate::screens::ScreenId;

    fn test_store(dir: &std::path::Path) -> nacelle::layout::LayautStore {
        nacelle::layout::LayautStore::new(nacelle::assets::AssetRoots::new(
            vec![dir.to_path_buf()],
            dir.to_path_buf(),
        ))
    }

    use super::*;

    // EVERY SETTER THIS MODULE DRIVES WAITS FOR THE DISK.
    //
    // A save is made durable on a thread of its own — see
    // [`super::write_conf_soon`] — so the interface never waits for an
    // `fsync`. That is a promise about the INTERFACE and about nothing
    // else: a test that changes a setting and then reads the file is
    // reading a write that may still be in the air, and the ones that
    // pass anyway pass by winning a race, which is worse than failing.
    //
    // So the writers are shadowed here rather than a `flush_writes` line
    // being sprinkled through fifty assertions. Each wrapper calls the
    // real one and then waits; the behaviour under test is the shipped
    // behaviour, with the one thing a test cannot see — "the bytes have
    // landed" — made visible. A test that wants the OTHER half, that the
    // running program answers before the disk does, calls `super::` by
    // name and says so: `a_save_is_made_durable_off_the_thread_that_-
    // asked_for_it`, `a_second_save_is_built_on_the_first_and_not_on_-
    // the_disk` and `a_panic_at_the_desk_does_not_cost_the_next_save`.
    // Named in prose rather than in a doc link because this is a `//`
    // block on an item that has none, and a link nothing resolves is a
    // promise of a test that need not exist.
    //
    // Shadowing works because a glob import loses to an item declared in
    // the module, which is what makes this one block instead of an edit
    // at every call site.
    fn set_engine_theme(name: &str) {
        super::set_engine_theme(name);
        flush_writes();
    }
    fn set_engine_variant(name: Option<&str>) {
        super::set_engine_variant(name);
        flush_writes();
    }
    fn set_layaut_for_screen(key: &str, name: &str) {
        super::set_layaut_for_screen(key, name);
        flush_writes();
    }
    fn set_main_screen(key: Option<&str>) {
        super::set_main_screen(key);
        flush_writes();
    }
    fn set_layaut_option(name: &str) {
        super::set_layaut_option(name);
        flush_writes();
    }
    fn set_sounds_option(name: &str) {
        super::set_sounds_option(name);
        flush_writes();
    }
    fn set_sound_volume(percent: u32) {
        super::set_sound_volume(percent);
        flush_writes();
    }
    fn set_blur_radius(percent: u32) {
        super::set_blur_radius(percent);
        flush_writes();
    }
    fn set_blur_opacity(percent: u32) {
        super::set_blur_opacity(percent);
        flush_writes();
    }
    fn set_grid_padding(n: u32) {
        super::set_grid_padding(n);
        flush_writes();
    }
    fn set_term_font_size(percent: u32) {
        super::set_term_font_size(percent);
        flush_writes();
    }
    fn set_term_font_family(name: &str) {
        super::set_term_font_family(name);
        flush_writes();
    }
    fn clear_look_and_feel() {
        super::clear_look_and_feel();
        flush_writes();
    }

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
        // The shipped variants left with 2026-08-16 — `default` is the one
        // look compiled in, every other theme is a person's file. So the
        // test WRITES the themes it is about to select, which is also the
        // road every theme takes from now on: out of the editor, into a
        // file, found by the loader's walk.
        let themes = dir.join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::env::set_var("NACELLE_THEME_DIR", &themes);
        for (name, accent) in [
            ("proba-red", "#E03A3A"),
            ("proba-blue", "#2A6BE0"),
            ("proba-green", "#2AB05A"),
        ] {
            std::fs::write(
                themes.join(format!("{name}.theme")),
                format!("[palette]\naccent = {accent}\n"),
            )
            .unwrap();
        }

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

        let red = colour_of("proba-red");
        assert!(
            dir.join(FAMILY_DIR).join(CONF_RON).is_file(),
            "the first settings change must create the user's configuration file"
        );
        let blue = colour_of("proba-blue");
        let green = colour_of("proba-green");

        // Each is its own hue — the hue its file just declared.
        assert!(red.r > red.b + 0.2, "the red theme's accent is not red: {red:?}");
        assert!(blue.b > blue.r + 0.2, "the blue theme's accent is not blue: {blue:?}");
        assert!(green.g > green.r + 0.2, "the green theme's accent is not green: {green:?}");
        // And switching really moves the value, rather than returning a cached
        // theme from the first load.
        assert!(red.r != blue.r, "the accent did not change at all");

        std::env::remove_var("NACELLE_THEME_DIR");
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

    /// The precedence between the platform's high-contrast signal and an
    /// explicit `variant:` choice, tested against [`wanted_variant`] alone —
    /// no D-Bus connection, no theme engine, no filesystem: the same reason
    /// libnacelle's reduced-motion contract is tested against a plain
    /// settable `bool` rather than a real portal.
    ///
    /// `Named` and `Off` are the user's own word on the matter, and both
    /// outrank the platform WHICHEVER WAY it points — an explicit choice is
    /// not merely a default the platform can talk over. `Inherit` is the one
    /// state with no opinion of its own, and there alone the platform's
    /// answer stands.
    #[test]
    fn the_platform_signal_only_answers_for_an_inherited_choice() {
        assert_eq!(
            wanted_variant(true, &Choice::Inherit),
            Some("hc".to_string()),
            "nobody has an opinion here but the platform"
        );
        assert_eq!(
            wanted_variant(false, &Choice::Inherit),
            None,
            "the platform has nothing to say either"
        );
        assert_eq!(
            wanted_variant(true, &Choice::Off),
            None,
            "an explicit off must refuse the platform exactly as it refuses a system file"
        );
        assert_eq!(
            wanted_variant(false, &Choice::Off),
            None,
            "off stays off with no platform signal to refuse"
        );
        assert_eq!(
            wanted_variant(true, &Choice::named("crimson")),
            Some("crimson".to_string()),
            "a named variant is the user's own word, not the platform's"
        );
        assert_eq!(
            wanted_variant(false, &Choice::named("crimson")),
            Some("crimson".to_string()),
            "a named variant does not need the platform to agree"
        );
    }

    /// [`apply_engine_variant`] is what `a11y_portal.rs` calls on every
    /// `SettingChanged`, standing alone rather than on the far side of a
    /// theme load — this is the test that a `None` want clears a variant a
    /// PREVIOUS call left selected, rather than relying on a load that is
    /// not happening here to have done it.
    #[test]
    fn the_platform_signal_is_undone_when_the_desktop_stops_asking() {
        let _theme = crate::widgets::theme_test_lock();
        fixture_registry();
        let _env = env_lock();
        let dir = variant_conf_dir("platform-undo");
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        // Inherit: nothing of the user's own stands in the platform's way.
        set_engine_theme("default");
        let (_cfg, _) = resolve();
        assert_eq!(current_engine_variant(), None, "Inherit is the default with a fresh file");

        assert_eq!(set_platform_high_contrast(true), false);
        apply_engine_variant();
        assert_eq!(
            nacelle::theme::current_variant().as_deref(),
            Some("hc"),
            "Inherit defers to the platform's own answer"
        );

        assert_eq!(set_platform_high_contrast(false), true);
        apply_engine_variant();
        assert_eq!(
            nacelle::theme::current_variant(),
            None,
            "the platform withdrawing its answer must not leave hc selected behind it"
        );

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
    /// wins FIELD by field, the system file answers everything it does
    /// not mention, and a missing file on either end is ordinary rather
    /// than an error. Nothing is copied for this to work — which is
    /// the whole point of reading a search path instead of seeding a
    /// home directory at install time.
    ///
    /// The third state is what a two-state format could not carry:
    /// `Off` in the user's file has to BEAT the system file, while an
    /// absent field lets it answer. Both readings are taken here,
    /// because a reset that confuses them turns the system defaults off
    /// instead of letting them back in.
    ///
    /// Hermetic: explicit paths, no process environment.
    #[test]
    fn the_user_file_wins_and_the_system_file_fills_the_gaps() {
        fixture_registry();
        // Reading a cascade SETS the process-wide sentence about broken
        // files ([`remember_conf_error`]), so this test may not run
        // beside one that reads a configuration of its own — the reason
        // spelled out on `a_file_that_does_not_parse_is_said_out_loud_-
        // and_never_swallowed`, which is where the rule was written and
        // this one was the only reader of a cascade that had not taken
        // it. It sets no environment variable, and takes the lock
        // anyway: what the lock really guards is the process, and a
        // reader that clears the sentence under another test's feet
        // fails that test and not its own — which is how it survived
        // this long, showing up as whichever test happened to be
        // asserting at the time.
        let _env = env_lock();
        let base =
            std::env::temp_dir().join(format!("nacelle-conf-cascade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let system = base.join("etc/xdg").join(FAMILY_DIR);
        let user = base.join("config").join(FAMILY_DIR);
        std::fs::create_dir_all(&system).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(
            system.join(CONF_RON),
            "// the distribution's defaults\n(\n    theme: Named(\"azure\"),\n    \
             layaut: Named(\"console\"),\n    color: (lut: Named(\"studio\")),\n)\n",
        )
        .unwrap();
        std::fs::write(
            user.join(CONF_RON),
            "(theme: Named(\"crimson\"), color: (lut: Off))\n",
        )
        .unwrap();

        let dirs = vec![user.clone(), system.clone()];
        let c = cascade_conf(&dirs);
        assert_eq!(c.theme.name(), Some("crimson"), "the user's own value must win");
        assert_eq!(
            c.layaut.name(),
            Some("console"),
            "a field the user never set comes from the system file"
        );
        assert_eq!(
            c.color.lut,
            Choice::Off,
            "an explicit off is not an absence: it must beat the system file"
        );
        assert_eq!(c.color.lut.name(), None, "and no LUT is loaded");
        // A group the user's file mentions AT ALL must not swallow the
        // system file's other fields in that group: the merge is per
        // leaf, not per section.
        assert_eq!(
            c.color.depth(),
            model::ColorConf::DEPTH,
            "nobody set a depth, so the model's own default stands"
        );

        // A user who has never changed a setting has no file at all,
        // and the system defaults stand on their own.
        std::fs::remove_file(user.join(CONF_RON)).unwrap();
        let c = cascade_conf(&dirs);
        assert_eq!(c.theme.name(), Some("azure"));
        assert_eq!(c.color.lut.name(), Some("studio"));

        // And with nothing installed anywhere the program is left with
        // what is built into it, rather than with an error.
        assert_eq!(cascade_conf(&[base.join("nowhere")]), DesktopConf::default());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A file in the format that came before, on a machine that has
    /// never written the new one. Every setting in it goes on working,
    /// and — the part a migration usually gets wrong — an EMPTY value
    /// keeps meaning what it meant: an explicit off, not an absence.
    #[test]
    fn the_old_format_is_still_read_and_still_means_what_it_said() {
        fixture_registry();
        let dir = scratch("legacy-format");
        std::fs::write(
            dir.join(CONF_FILE),
            "# somebody's own file\n\
             Theme=crimson\n\
             Variant=hc\n\
             Layaut=console\n\
             Layaut[DP-1]=cockpit\n\
             Sounds=classic\n\
             TermFontSize=112\n\
             UIFontWeight=bold\n\
             SoundVolume=40\n\
             SoundTyping=0\n\
             GridSnap=true\n\
             GridCols=32\n\
             GridPadding=9\n\
             ColorDepth=10\n\
             ColorSpace=bt2020 pq\n\
             ColorLut=\n\
             BlurRadius=60\n",
        )
        .unwrap();

        let c = read_conf_dir(&dir).unwrap().expect("the old file must be read");
        assert_eq!(c.theme.name(), Some("crimson"));
        assert_eq!(c.variant.name(), Some("hc"));
        assert_eq!(c.layaut.name(), Some("console"));
        assert_eq!(c.screens().get("DP-1").map(String::as_str), Some("cockpit"));
        assert_eq!(c.sounds.name(), Some("classic"));
        assert_eq!(c.term_font.scale(0.5, 2.0), 1.12);
        assert_eq!(c.ui_font.weight.name(), Some("bold"));
        assert_eq!(c.sound.volume(), 40);
        assert!(!c.sound.typing(), "SoundTyping=0 was off and stays off");
        assert!(c.sound.ambient(), "a key the file never had takes the default");
        assert!(c.grid.snap());
        assert_eq!(c.grid.cols(), 32);
        assert_eq!(c.grid.padding(), Some(9));
        assert_eq!(c.color.depth(), 10);
        assert_eq!(c.color.space(), "bt2020 pq");
        assert_eq!(
            c.color.lut,
            Choice::Off,
            "an empty value outranked the system file in that format too"
        );
        assert_eq!(c.blur.radius(), 60);
        assert_eq!(c.blur.opacity(), model::BlurConf::FULL, "and the rest is default");

        // A `.ron` beside it takes over WHOLE — the two formats are not
        // merged inside one directory, or a setting deleted from the
        // new file would be answered by a stale line in the old one.
        std::fs::write(dir.join(CONF_RON), "(theme: Named(\"azure\"))\n").unwrap();
        let c = read_conf_dir(&dir).unwrap().unwrap();
        assert_eq!(c.theme.name(), Some("azure"));
        assert_eq!(c.sounds, Choice::Inherit, "the old file no longer answers here");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The depth and the space are TWO LINES OF ONE STATEMENT, and the
    /// reading rules on the pair.
    ///
    /// Either field alone passes: eight is a depth this program can ask
    /// for, `bt2020 pq` is a space it knows. Together they ask for a
    /// picture that bands, and nothing used to look at them together —
    /// so a file could say what the settings window forbids, and the
    /// swapchain, which reads this and never asks the window anything,
    /// was handed it. This is the one place both fields are in reach.
    ///
    /// The floor is read off the space here exactly as the COLOR page
    /// reads it off its switch (`color_depths`), which is what keeps
    /// the page and the swapchain from being told different numbers.
    #[test]
    fn a_depth_the_named_space_cannot_show_is_raised_by_the_reading() {
        let high = |bits: Option<u32>| model::ColorConf {
            depth: bits,
            space: Choice::named("bt2020 pq"),
            ..Default::default()
        };
        let offered = color_depths(true);
        assert!(
            !offered.contains(&8),
            "the high-range offer holds eight bits: this test is measuring \
             nothing"
        );
        assert!(
            offered.contains(&high(Some(8)).depth()),
            "a file saying depth 8 and space 'bt2020 pq' resolved to {} bits, \
             which the high range is not shown in",
            high(Some(8)).depth()
        );
        // A file that names the space and no depth is the same pair with
        // the model's own default written in it, and the default is the
        // eight this cannot use.
        assert_eq!(model::ColorConf::DEPTH, 8, "the default depth moved");
        assert!(
            offered.contains(&high(None).depth()),
            "a high-range space with no depth beside it resolved to {} bits",
            high(None).depth()
        );
        // It raises and never lowers: what the user wrote above the
        // floor is theirs.
        assert_eq!(high(Some(16)).depth(), 16, "the reading took a depth away");

        // And it touches nothing else. A standard-range space keeps the
        // eight — the pair is only contradictory in one direction.
        for name in ["auto", "srgb", "display p3", "adobe rgb"] {
            let c = model::ColorConf {
                depth: Some(8),
                space: Choice::named(name),
                ..Default::default()
            };
            assert_eq!(
                c.depth(),
                8,
                "'{name}' is a standard-range space and eight bits shows it"
            );
        }
    }

    /// A file with a syntax error costs the WHOLE file — that is what
    /// this format is — so the one thing that may not happen is a quiet
    /// fall back to the defaults. The rest of the cascade still
    /// answers, the program still starts, and the user is told which
    /// file and where in it.
    #[test]
    fn a_file_that_does_not_parse_is_said_out_loud_and_never_swallowed() {
        fixture_registry();
        // The sentence is remembered process-wide, so this test may not
        // run beside one that reads a configuration of its own.
        let _env = env_lock();
        let base = scratch("broken-conf");
        let user = base.join("user");
        let system = base.join("system");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(system.join(CONF_RON), "(theme: Named(\"azure\"))\n").unwrap();
        // One bracket short, which is all it takes.
        std::fs::write(
            user.join(CONF_RON),
            "(\n    theme: Named(\"crimson\"\n    variant: Off,\n)\n",
        )
        .unwrap();

        let dirs = vec![user.clone(), system.clone()];
        let c = cascade_conf(&dirs);
        assert_eq!(
            c.theme.name(),
            Some("azure"),
            "the file that does parse must still answer"
        );
        let said = conf_error().expect("a broken file must leave a sentence");
        assert!(said.contains(&user.join(CONF_RON).display().to_string()), "{said}");
        assert!(said.contains("line 2"), "the sentence must point AT it: {said}");

        // And it stops being complained about the moment it is fixed,
        // without the program being restarted.
        std::fs::write(user.join(CONF_RON), "(theme: Named(\"crimson\"))\n").unwrap();
        assert_eq!(cascade_conf(&dirs).theme.name(), Some("crimson"));
        assert_eq!(conf_error(), None, "a repaired file must clear the warning");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The document is the schema: what the program writes, the program
    /// reads back unchanged — and a file that is merely INCOMPLETE, or
    /// carries a field this build has never heard of, is ordinary
    /// rather than a failure. That is what keeps an older binary able
    /// to open a newer machine's file.
    #[test]
    fn the_written_document_reads_back_and_a_partial_one_still_parses() {
        let doc = DesktopConf {
            theme: Choice::Named("crimson".into()),
            variant: Choice::Off,
            screens: [
                ("DP-1".to_string(), Choice::Named("cockpit".into())),
                ("eDP-1".to_string(), Choice::Off),
            ]
            .into_iter()
            .collect(),
            term_font: model::FontConf { size: Some(112.5), ..Default::default() },
            grid: model::GridConf { cols: Some(32), ..Default::default() },
            color: model::ColorConf {
                space: Choice::Named("bt2020 pq".into()),
                ..Default::default()
            },
            ..Default::default()
        };

        let text = ron_options().to_string_pretty(&doc, ron_pretty()).unwrap();
        assert_eq!(ron_options().from_str::<DesktopConf>(&text).unwrap(), doc);
        // Nothing that was not set is written down, which is what makes
        // a cleared setting indistinguishable from one never made.
        assert!(!text.contains("sounds"), "an unset field must not be written: {text}");
        assert!(!text.contains("blur"), "nor an untouched group: {text}");
        assert_eq!(
            ron_options()
                .to_string_pretty(&DesktopConf::default(), ron_pretty())
                .unwrap()
                .trim(),
            "()",
            "and a document with nothing in it is empty rather than a list of defaults"
        );

        // Half a document, a field from the future, and a number
        // written the way a person writes one.
        let hand = "// mine\n(\n    theme: Named(\"azure\"),\n    \
                    warp_drive: Enabled,\n    sound: (volume: 40),\n)\n";
        let c = ron_options().from_str::<DesktopConf>(hand).expect("must parse");
        assert_eq!(c.theme.name(), Some("azure"));
        assert_eq!(c.sound.volume(), 40, "`40` and `Some(40)` are the same number");
        assert_eq!(c.layaut, Choice::Inherit, "everything absent is simply absent");
    }

    /// The search path itself: the user's directory first — it is also
    /// the only one written to — then XDG_CONFIG_DIRS in its own
    /// order, `/etc/xdg` when it is unset or empty, and no directory
    /// twice. Every level carries the folder's old name directly behind
    /// its new one, so nothing installed under `nacelle-desktop` falls
    /// off the path.
    #[test]
    fn the_configuration_search_path_follows_xdg() {
        fixture_registry();
        let home = PathBuf::from("/home/somebody/.config");
        let user = home.join(FAMILY_DIR);
        let user_old = home.join(LEGACY_FAMILY_DIR);
        let etc = PathBuf::from("/etc/xdg").join(FAMILY_DIR);
        let etc_old = PathBuf::from("/etc/xdg").join(LEGACY_FAMILY_DIR);
        for unset in [None, Some(""), Some("/etc/xdg")] {
            assert_eq!(
                config_search_path(&home, unset),
                vec![user.clone(), user_old.clone(), etc.clone(), etc_old.clone()],
                "{unset:?} must resolve to the standard /etc/xdg"
            );
        }
        assert_eq!(
            config_search_path(&home, Some("/opt/site/etc:/etc/xdg:/opt/site/etc")),
            vec![
                user.clone(),
                user_old.clone(),
                PathBuf::from("/opt/site/etc").join(FAMILY_DIR),
                PathBuf::from("/opt/site/etc").join(LEGACY_FAMILY_DIR),
                etc.clone(),
                etc_old,
            ],
            "the order of the variable is kept and duplicates drop"
        );
        assert_eq!(
            config_search_path(&home, Some("/opt/site/etc"))[0],
            user,
            "the write target is always the head of the read path"
        );
        // The user's OLD directory outranks the system's new one: the
        // cascade merges key by key, and a setting somebody made years
        // ago may not be overruled by a distribution default just
        // because the folder has been renamed since.
        let path = config_search_path(&home, Some("/etc/xdg"));
        let at = |p: &PathBuf| path.iter().position(|d| d == p).expect("on the path");
        assert!(at(&user_old) < at(&etc), "{path:?}");
    }

    /// The data path is the configuration path's twin, down to the two
    /// standard prefixes XDG names when the variable is unset — so a
    /// sound set or a layaut installed under either name is found, and
    /// a user install still shadows a system one.
    #[test]
    fn the_data_search_path_carries_both_names_at_every_level() {
        fixture_registry();
        let home = PathBuf::from("/home/somebody/.local/share");
        for unset in [None, Some("")] {
            assert_eq!(
                data_search_path(&home, unset),
                vec![
                    home.join(FAMILY_DIR),
                    home.join(LEGACY_FAMILY_DIR),
                    PathBuf::from("/usr/local/share").join(FAMILY_DIR),
                    PathBuf::from("/usr/local/share").join(LEGACY_FAMILY_DIR),
                    PathBuf::from("/usr/share").join(FAMILY_DIR),
                    PathBuf::from("/usr/share").join(LEGACY_FAMILY_DIR),
                ],
                "{unset:?} must resolve to the two standard prefixes"
            );
        }
        assert_eq!(
            data_search_path(&home, Some("/usr/share:/usr/share")),
            vec![
                home.join(FAMILY_DIR),
                home.join(LEGACY_FAMILY_DIR),
                PathBuf::from("/usr/share").join(FAMILY_DIR),
                PathBuf::from("/usr/share").join(LEGACY_FAMILY_DIR),
            ],
            "duplicates drop on the data side too"
        );
    }

    /// The ordinary machine's shape: no `XDG_*` variables at all, just
    /// `HOME`. That is the case the owner's own machine is in, and the
    /// one every path here is really built from.
    #[test]
    fn with_only_home_set_both_folder_names_stand_under_the_dot_directories() {
        fixture_registry();
        let _env = env_lock();
        let home_was = std::env::var("HOME").ok();
        let root = scratch("home-only");
        std::env::set_var("HOME", &root);
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        std::env::remove_var("XDG_DATA_HOME");
        std::env::remove_var("XDG_DATA_DIRS");

        assert_eq!(config_dir(), root.join(".config").join(FAMILY_DIR));
        assert_eq!(data_dir(), root.join(".local/share").join(FAMILY_DIR));
        assert_eq!(
            config_dirs()[..2],
            [
                root.join(".config").join(FAMILY_DIR),
                root.join(".config").join(LEGACY_FAMILY_DIR),
            ],
            "the user's two folders, new name first"
        );
        assert_eq!(
            data_dirs()[..2],
            [
                root.join(".local/share").join(FAMILY_DIR),
                root.join(".local/share").join(LEGACY_FAMILY_DIR),
            ],
            "and the same pair on the data side"
        );
        // The toolkit is handed exactly that path, and writes to the
        // head of it and nowhere else.
        let roots = asset_roots();
        assert_eq!(roots.read, data_dirs());
        assert_eq!(roots.write, data_dir());

        match home_was {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// How many frames a claim about "not once a frame" is worth
    /// making. Sixty is a second of the storm the 2026-08-18 `strace`
    /// caught: 136 walks of the cascade in 2.2 s, one per frame, each
    /// of them 1121 bytes of RON parsed twice.
    const FRAMES: u32 = 60;

    /// A configuration directory with a file in it, and the environment
    /// pointed at it. Answers the directory.
    fn conf_root(tag: &str, body: &str) -> PathBuf {
        let root = scratch(tag);
        std::env::set_var("XDG_CONFIG_HOME", &root);
        // A system end that exists as a NAME and not as a directory,
        // which is the ordinary machine: the eight paths the cascade
        // knocked on sixty times a second were all of them absent.
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).expect("the scratch tree must be writable");
        std::fs::write(dir.join(CONF_RON), body).expect("the fixture must be writable");
        dir
    }

    fn conf_root_done(dir: &Path) {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap_or(dir));
    }

    /// The whole of nuisance number 2 from the 2026-08-18 audit: a
    /// desktop that reads its settings file every frame.
    ///
    /// Measured there: 294 opens of `nacelle-desktop.ron` in 89 seconds,
    /// 136 of them 16-17 ms apart. The question came from a row of the
    /// settings window and the answer walked the cascade — parse, eight
    /// absent paths, parse again — with nothing on the disk having moved
    /// between one frame and the next.
    ///
    /// So the assertion is a COUNT and not a description: the first ask
    /// reaches the disk, and sixty more do not reach it at all.
    #[test]
    fn the_configuration_is_read_once_and_not_once_a_frame() {
        let _env = env_lock();
        let dir = conf_root("conf-per-frame", "(theme: Named(\"crimson\"))\n");

        let base = conf_file_reads();
        assert_eq!(conf().theme.name(), Some("crimson"), "the fixture must be readable");
        let first = conf_file_reads() - base;
        assert!(first > 0, "the first ask has to reach the disk, and read {first} files");

        let settled = conf_file_reads();
        for _ in 0..FRAMES {
            assert_eq!(
                conf().theme.name(),
                Some("crimson"),
                "the answer changed with nothing on the disk changing"
            );
        }
        assert_eq!(
            conf_file_reads(),
            settled,
            "{FRAMES} frames cost {} more reads of a file nothing had touched",
            conf_file_reads() - settled
        );

        conf_root_done(&dir);
    }

    /// And the memo is not a one-way door: a file edited by hand, by a
    /// second copy of this program or by anything else answers on the
    /// next ask.
    ///
    /// This is the half that makes the count above safe to want. A cache
    /// with no invalidation is the settings window's Apply button doing
    /// nothing, which the toolkit has written down as the worse failure
    /// of the two (`HostApi::settings_epoch`).
    #[test]
    fn a_file_changed_behind_the_programs_back_is_read_again() {
        let _env = env_lock();
        let dir = conf_root("conf-changed", "(theme: Named(\"crimson\"))\n");
        assert_eq!(conf().theme.name(), Some("crimson"));

        std::fs::write(dir.join(CONF_RON), "(theme: Named(\"azure\"))\n").unwrap();
        assert_eq!(
            conf().theme.name(),
            Some("azure"),
            "the file was rewritten and the answer stayed on the old one"
        );

        // Including a file that goes AWAY: absence is an answer too, and
        // a memo that only watched the files it found would go on
        // reciting a document the user deleted.
        std::fs::remove_file(dir.join(CONF_RON)).unwrap();
        assert_eq!(
            conf().theme.name(),
            None,
            "the file was deleted and the answer stayed on what it said"
        );

        conf_root_done(&dir);
    }

    /// Nuisance number 4: the two `fsync` calls of a save ran on the
    /// event loop — 0.516 s of them over seven saves, one save alone
    /// holding the interface for 0.35 s.
    ///
    /// What may NOT change is the order, which is the whole of the
    /// atomicity: the bytes are flushed before the rename publishes
    /// them, and the directory after it. So the test asserts both — the
    /// order, and that no step of it happened on the thread that asked.
    #[test]
    fn a_save_is_made_durable_off_the_thread_that_asked_for_it() {
        let _env = env_lock();
        let dir = conf_root("conf-fsync-thread", "(theme: Named(\"crimson\"))\n");

        // `super::`, because the wrapper at the head of this module is
        // exactly what this test is about: the wait is done by hand.
        super::set_engine_theme("azure");
        flush_writes();

        let steps = last_write_steps();
        assert_eq!(
            steps.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![WriteStep::SyncFile, WriteStep::Rename, WriteStep::SyncDir],
            "the order that makes a save atomic is not the order it was taken in"
        );
        let asked_on = std::thread::current().id();
        for (step, ran_on) in &steps {
            assert_ne!(
                *ran_on, asked_on,
                "{step:?} ran on the thread that changed the setting"
            );
        }
        assert!(
            std::fs::read_to_string(dir.join(CONF_RON)).unwrap().contains("azure"),
            "the setting has to be IN the file once the wait is over"
        );

        conf_root_done(&dir);
    }

    /// And the setting is in force before its bytes are — which is what
    /// makes the write above safe to hand to another thread.
    ///
    /// The trap this closes is not a slow interface but a LOST SETTING.
    /// A save seeds from the user's own file; with the write still in
    /// the air, a second save reading that file would seed from the
    /// document the first one is about to replace, and whichever write
    /// finished last would decide what the user ends up with. Two
    /// presses of a slider are a millisecond apart.
    #[test]
    fn a_second_save_is_built_on_the_first_and_not_on_the_disk() {
        let _env = env_lock();
        let dir = conf_root("conf-two-saves", "(theme: Named(\"crimson\"))\n");

        super::set_engine_theme("azure");
        // In force at once: no reader of the running program waits for
        // a disk to confirm what the window already shows.
        assert_eq!(
            conf().theme.name(),
            Some("azure"),
            "the setting was not in force until the bytes landed"
        );
        super::set_sound_volume(42);
        flush_writes();

        let text = std::fs::read_to_string(dir.join(CONF_RON)).unwrap();
        assert!(
            text.contains("azure"),
            "the second save was built on the file the first had not written yet, \
             and the theme went with it: {text}"
        );
        assert!(text.contains("42"), "the second save is in the file too: {text}");

        conf_root_done(&dir);
    }

    /// A panic ANYWHERE near the desk used to cost the user every
    /// setting they changed after it.
    ///
    /// Poisoning is permanent and process-wide, and the two places that
    /// reached for the desk read `Err` as "nothing to do here": the job
    /// was dropped without a word, and the memo already filed for it
    /// went on handing that document to every reader while the file
    /// itself was never opened again. Settings stopped being saved and
    /// hand-edits stopped being read, from that moment until the
    /// desktop was restarted, with nothing on screen to connect it to.
    ///
    /// The panic is not hypothetical: `flush_writes` says its one
    /// sentence with `eprintln!`, which panics when stderr will not
    /// take it, and it used to say it holding the desk.
    ///
    /// Two assertions, because the loss had two halves — the file that
    /// was not written, and the reader that was answered anyway.
    #[test]
    fn a_panic_at_the_desk_does_not_cost_the_next_save() {
        let _env = env_lock();
        let dir = conf_root("conf-desk-poison", "(theme: Named(\"crimson\"))\n");
        // Nothing of anybody else's still in the air: the fixture below
        // strands whatever is queued at the moment it fires, and that
        // is meant to be this test's business alone.
        flush_writes();

        // Exactly what a panic inside `flush_writes`'s own sentence
        // did. The panic message this prints belongs to the fixture.
        let _ = std::thread::spawn(|| { // thread-guard: fixture
            let _held = lock_desk();
            panic!("a fixture panicking with the desk held");
        })
        .join();
        assert!(WRITE_DESK.is_poisoned(), "the fixture poisoned nothing, so this proves nothing");

        super::set_engine_theme("azure");
        flush_writes();
        assert!(
            std::fs::read_to_string(dir.join(CONF_RON)).unwrap().contains("azure"),
            "the save was dropped on the floor because somebody else had panicked"
        );

        // And the memo is not left standing for a write that never
        // happened: the disk is still in charge of the answer.
        std::fs::write(dir.join(CONF_RON), "(theme: Named(\"ochre\"))\n").unwrap();
        assert_eq!(
            conf().theme.name(),
            Some("ochre"),
            "the memo went on answering for a document the disk had never carried"
        );

        // Put the flag down again so the rest of the suite runs against
        // the ordinary state rather than the cured one.
        WRITE_DESK.clear_poison();
        conf_root_done(&dir);
    }

    /// THE OTHER HALF OF THE MEASUREMENT — an instrument, not an
    /// assertion about a value.
    ///
    /// The figures quoted in [`conf_files`] have two provenances, and
    /// only one of them was ever in this repository. The counter half
    /// (`the_settings_file_is_read_once_and_not_once_a_frame`) is here
    /// and repeatable. The `strace` half — 816 opens of the document
    /// and 1088 absent paths knocked on before, six and eight after,
    /// over 136 asks — came from a probe that was never committed, so
    /// a reader had to take it on trust. This is that probe.
    ///
    /// Ignored by default because it measures rather than asserts, and
    /// because it reads the ambient XDG layout on purpose: the numbers
    /// are of a MACHINE, and the owner's has six configuration
    /// directories and therefore thirteen names to stamp. To repeat it:
    ///
    /// ```text
    /// cargo test --offline --no-run
    /// strace -f -e trace=openat,statx -o /tmp/cfg.log \
    ///     target/debug/deps/nacelle_desktop-<hash> \
    ///     --ignored --test-threads=1 --exact \
    ///     config::tests::the_cascade_asked_a_hundred_and_thirty_six_times
    /// grep -c '"[^"]*nacelle-desktop.ron", O_RDONLY' /tmp/cfg.log   # opens
    /// grep -c 'ENOENT' /tmp/cfg.log                                 # absent
    /// ```
    ///
    /// The BEFORE column is the same run with the memo taken out of
    /// [`conf`] — the one line `if let Some(doc) = conf_memo_hit(...)`
    /// — which is the change being measured and nothing else.
    ///
    /// It does assert one thing, and it is the property the numbers are
    /// about rather than any particular number: asking a second time
    /// costs nothing, so the count does not grow with the asking.
    #[test]
    #[ignore = "an instrument: run it alone, under a syscall tracer"]
    fn the_cascade_asked_a_hundred_and_thirty_six_times() {
        let _env = env_lock();
        let base = conf_file_reads();
        for _ in 0..136 {
            let _ = conf();
        }
        let first = conf_file_reads() - base;
        for _ in 0..136 {
            let _ = conf();
        }
        let second = conf_file_reads() - base - first;
        println!("136 asks: {first} reads off the disk; 136 more: {second}");
        assert_eq!(second, 0, "the cascade is still being read once per ask");
    }

    /// A directory of this test's own, emptied first.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nacelle-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch tree must be writable");
        dir
    }

    /// The four states a machine can be in the day the folder changes
    /// name, on the CONFIGURATION side.
    ///
    /// The one that matters is OLD ONLY: that is every machine that ran
    /// this program before the rename, and its settings have to go on
    /// being read where they lie. Nothing here moves, renames or
    /// deletes a file.
    ///
    /// BOTH is the state after the first setting is changed, and it is
    /// reached by changing one rather than by planting a file: on a
    /// real machine the new file can only have been written by
    /// [`update_conf`], which is what carries the old one across and
    /// what earns the right to answer in its place.
    #[test]
    fn the_new_configuration_folder_wins_and_the_old_one_is_still_read() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("config-fallback");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        // A system end of the cascade that exists and is empty, so this
        // test says nothing about the machine it runs on.
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));

        // NEITHER: an ordinary first run. Not an error, not a directory
        // created — the program simply uses what is built into it.
        assert_eq!(conf(), DesktopConf::default(), "an empty home carries no settings");
        assert!(!root.join(FAMILY_DIR).exists(), "reading creates nothing");

        // OLD ONLY: the file is read under the folder's old name, in
        // the format it was written in.
        let old = root.join(LEGACY_FAMILY_DIR);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(CONF_FILE), "Theme=crimson\nLayaut=console\n").unwrap();
        assert_eq!(
            conf().theme.name(),
            Some("crimson"),
            "a machine that never moved anything must keep its settings"
        );

        // BOTH, reached the only way a machine reaches it: by changing
        // a setting. The write carries the old file across WHOLE and
        // then answers for it — a user has one configuration, not two,
        // and the second one would be a copy no reset could reach.
        // Nothing is moved or deleted; the old file is still there.
        let new = root.join(FAMILY_DIR);
        set_engine_theme("azure");
        let c = conf();
        assert_eq!(c.theme.name(), Some("azure"), "the setting just made");
        assert_eq!(
            c.layaut.name(),
            Some("console"),
            "and every field only the old file had, carried across rather than lost"
        );
        let text = std::fs::read_to_string(new.join(CONF_RON)).unwrap();
        assert!(text.contains("console"), "carried into the file, not merely answered: {text}");
        assert!(old.join(CONF_FILE).is_file(), "and the old file is still where it was");

        // Which is what makes taking a field OUT mean something: with
        // the old file still in the cascade this would answer
        // "crimson" again and nothing would say why.
        clear_look_and_feel();
        assert_eq!(conf().theme, Choice::Inherit, "a cleared field stays cleared");
        assert_eq!(conf().layaut, Choice::Inherit);

        // NEW ONLY: what a machine looks like once the user has moved
        // the folder themselves.
        std::fs::remove_dir_all(&old).unwrap();
        set_engine_theme("azure");
        let c = conf();
        assert_eq!(c.theme.name(), Some("azure"));
        assert_eq!(c.layaut, Choice::Inherit);

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The measurement that says a configuration cannot be LOST.
    ///
    /// The copy `write_conf` keeps is one generation deep and is
    /// retaken on every write, and the writes come in bursts: one arrow
    /// key on a slider is one write, and `nudge` calls the setter on
    /// every press. So a user whose file is one bracket short, who then
    /// presses the key twice, used to end up with a live file holding
    /// one setting, a `.bak` holding that same file one press earlier,
    /// and their own text nowhere at all.
    ///
    /// Three writes, which is two more than it used to survive.
    #[test]
    fn what_could_not_be_parsed_outlives_a_burst_of_writes_and_is_said_out_loud() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("rescue-broken");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        // Somebody's whole configuration, one bracket short.
        let mine = "(\n    theme: Named(\"crimson\",\n    layaut: Named(\"console\"),\n    \
                    sounds: Named(\"classic\"),\n    grid: (cols: 40, rows: 30),\n)\n";
        std::fs::write(dir.join(CONF_RON), mine).unwrap();

        // Exactly what holding an arrow key on the volume slider does.
        set_sound_volume(70);
        set_sound_volume(71);
        set_sound_volume(72);

        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON_RESCUE)).unwrap(),
            mine,
            "the rescue copy must still be the text the USER wrote, not a \
             write from halfway through the burst"
        );
        assert_eq!(conf().sound.volume(), 72, "and the setting asked for still took");

        // The user is told, at the moment it happens rather than at the
        // next start — by then the file parses and there is nothing
        // left for a startup check to notice.
        let said = take_conf_rescued().expect("a replaced file must leave a sentence");
        assert!(said.contains(&dir.join(CONF_RON).display().to_string()), "{said}");
        assert!(
            said.contains(&dir.join(CONF_RON_RESCUE).display().to_string()),
            "the sentence must name where the text went: {said}"
        );
        assert_eq!(take_conf_rescued(), None, "and it is said once, not per frame");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The SECOND typo, on a machine that still carries the first one's
    /// rescue copy.
    ///
    /// This program never deletes a rescue copy — that is deliberate,
    /// and it is exactly what makes this state ordinary rather than
    /// exotic. A user breaks their file in June, gets a copy, repairs
    /// the live file and leaves the copy lying there because nobody
    /// told them to sweep it up. In August they hand-edit again and
    /// miss another bracket. If the presence of June's copy is taken as
    /// "already rescued", August's text is copied nowhere, the live
    /// file is replaced, and two more nudges push it out of the `.bak`
    /// as well — the same total loss the rescue copy exists to prevent,
    /// arrived at by having been careful once before.
    ///
    /// So the question the copy answers is not "is there a file with
    /// that name" but "is this text already kept". June's copy is not
    /// touched, because a copy that is overwritten is not a copy.
    #[test]
    fn a_second_broken_file_is_kept_beside_the_first_rescue_rather_than_instead_of_it() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("rescue-twice");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        // June: broken, rescued, repaired, and the copy left lying
        // about — written directly, because that is the state on disk
        // however it was reached.
        let june = "(\n    theme: Named(\"crimson\",\n)\n";
        std::fs::write(dir.join(CONF_RON_RESCUE), june).unwrap();

        // August: a different whole configuration, one bracket short,
        // and the same burst of arrow keys on the volume slider.
        let august = "(\n    theme: Named(\"azure\",\n    layaut: Named(\"console\"),\n    \
                      sounds: Named(\"classic\"),\n    grid: (cols: 40, rows: 30),\n)\n";
        std::fs::write(dir.join(CONF_RON), august).unwrap();
        set_sound_volume(70);
        set_sound_volume(71);
        set_sound_volume(72);

        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON_RESCUE)).unwrap(),
            june,
            "the copy already on disk is somebody's configuration too and may \
             not be written over"
        );
        let kept = rescue_copies(&dir);
        assert!(
            kept.iter().any(|t| t == august),
            "August's text has to be SOMEWHERE on disk; what was found: {kept:?}"
        );
        assert_eq!(conf().sound.volume(), 72, "and the setting asked for still took");

        // Named, so the sentence sends the user to the file that holds
        // their August text rather than to June's.
        let said = take_conf_rescued().expect("a replaced file must leave a sentence");
        let named = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| said.contains(&p.display().to_string()) && p != &dir.join(CONF_RON))
            .expect("the sentence must name a file that exists");
        assert_eq!(
            std::fs::read_to_string(&named).unwrap(),
            august,
            "the sentence must name the copy holding what was just replaced: {said}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The file that is not TEXT — the shape a machine that lost power
    /// mid-write leaves behind, and the one an editor set to the wrong
    /// encoding leaves behind on purpose.
    ///
    /// A filesystem that recovers a journal can hand back a file of the
    /// right length whose blocks were never written: bytes that are not
    /// UTF-8 at all. Reading that with a `read_to_string` and taking the
    /// failure to mean "this directory says nothing" is the quiet
    /// version of the whole problem — a present file, holding
    /// somebody's settings under whatever went wrong, replaced by a
    /// document with one field in it and not a word said.
    ///
    /// A file that is THERE is never nothing. Only its absence is.
    #[test]
    fn a_configuration_that_is_not_even_text_is_kept_and_said_out_loud_too() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("rescue-not-text");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        // Half a configuration and then the hole the journal left,
        // under permissions the user closed down by hand.
        use std::os::unix::fs::PermissionsExt;
        let mut mine: Vec<u8> = b"(\n    theme: Named(\"crimson\"),\n".to_vec();
        mine.extend_from_slice(&[0xff, 0xfe, 0x00, 0x00, 0x9c]);
        std::fs::write(dir.join(CONF_RON), &mine).unwrap();
        std::fs::set_permissions(dir.join(CONF_RON), std::fs::Permissions::from_mode(0o600))
            .unwrap();

        set_sound_volume(70);
        set_sound_volume(71);
        set_sound_volume(72);

        assert_eq!(
            std::fs::read(dir.join(CONF_RON_RESCUE)).unwrap(),
            mine,
            "bytes that are not text are still the user's file"
        );
        assert_eq!(
            std::fs::metadata(dir.join(CONF_RON_RESCUE)).unwrap().permissions().mode() & 0o777,
            0o600,
            "and a copy easier to read than the file it copies is a copy that \
             gave something away"
        );
        assert_eq!(conf().sound.volume(), 72, "and the setting asked for still took");
        let said = take_conf_rescued().expect("a replaced file must leave a sentence");
        assert!(
            said.contains(&dir.join(CONF_RON_RESCUE).display().to_string()),
            "the sentence must name where the bytes went: {said}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The file this program cannot get at, at all.
    ///
    /// A configuration owned by root, or one on a mount that has gone
    /// sour, cannot be READ — and a file that cannot be read cannot be
    /// judged. It may be somebody's whole configuration, perfectly
    /// well-formed, behind a `chown` that went the wrong way; the one
    /// thing certain about it is that no copy of it can be taken. Yet
    /// replacing it needs no permission on the FILE at all, only on the
    /// directory: the rename lands, and what could not be read is gone
    /// with nothing kept.
    ///
    /// So the rule is what may be replaced, not what may be refused:
    /// this program replaces what it has safely kept. A file that does
    /// not PARSE is kept whole and then replaced, which is what leaves
    /// the settings window able to fix a machine. A file that could not
    /// be kept is left exactly where it is, and the sentence says what
    /// to do about it — the one case where the window is honestly
    /// powerless, because the shell it is running in is too.
    #[test]
    fn a_file_that_could_not_be_copied_anywhere_is_not_replaced_either() {
        // Meaningless as root, who can read every file there is, so
        // there is nothing for the rescue copy to fail at.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        fixture_registry();
        let _env = env_lock();
        let root = scratch("rescue-unreadable");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        use std::os::unix::fs::PermissionsExt;
        let mine = "(\n    theme: Named(\"crimson\"),\n    grid: (cols: 40),\n)\n";
        let path = dir.join(CONF_RON);
        std::fs::write(&path, mine).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

        set_sound_volume(70);
        set_sound_volume(71);
        set_sound_volume(72);

        // The directory is the user's own, so nothing stopped the
        // rename except the decision not to make it.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            mine,
            "a file that could not be copied may not be written over"
        );
        let said = take_conf_rescued().expect("and the user has to be told why");
        assert!(
            said.contains(&path.display().to_string()),
            "the sentence must name the file standing in the way: {said}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A directory with no `.ron` in it says NOTHING, and that has to
    /// stay a different answer from a `.ron` that cannot be read.
    ///
    /// The distinction is the whole of the change above: an absent file
    /// hands the question to the next rung of the cascade in silence,
    /// which is the ordinary state of every machine with no system
    /// defaults and of every user who has never changed a setting. If
    /// absence started reporting itself, the popup would be on screen
    /// for everybody, and the one it exists for would be lost in it.
    #[test]
    fn a_directory_without_a_configuration_is_still_silent() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("no-conf-at-all");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let _ = take_conf_rescued();

        assert_eq!(read_conf_dir(&root.join(FAMILY_DIR)), Ok(None), "a directory that is not there");
        std::fs::create_dir_all(root.join(FAMILY_DIR)).unwrap();
        assert_eq!(read_conf_dir(&root.join(FAMILY_DIR)), Ok(None), "and one that is empty");

        let _ = conf();
        assert_eq!(conf_error(), None, "nothing to complain about");
        assert_eq!(take_conf_rescued(), None, "and nothing was replaced");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A write that keeps failing must not turn one broken file into a
    /// directory full of identical copies of it.
    ///
    /// The read-only configuration directory is the case: every nudge
    /// finds the same unreadable file, fails to replace it, and comes
    /// back for the next keypress. Holding a slider down is hundreds of
    /// those. What decides is the TEXT — the same text is already kept,
    /// so there is nothing to keep — which is also why the burst in the
    /// test above leaves exactly one copy.
    #[test]
    fn the_same_broken_text_is_kept_once_however_many_writes_come() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("rescue-repeats");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        let mine = "(\n    theme: Named(\"crimson\",\n    grid: (cols: 40),\n)\n";
        for _ in 0..5 {
            // Put it back each time, which is what a directory the
            // program cannot write to amounts to: the replacement never
            // lands, so the next keypress meets the same file again.
            std::fs::write(dir.join(CONF_RON), mine).unwrap();
            set_sound_volume(70);
        }

        assert_eq!(
            rescue_copies(&dir),
            vec![mine.to_string()],
            "one text, one copy — a copy per keypress is a directory nobody \
             can read"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Every rescue copy in a directory, read back as text.
    fn rescue_copies(dir: &Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .expect("the configuration directory must exist")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(CONF_RON_RESCUE))
                    .unwrap_or(false)
            })
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect();
        out.sort();
        out
    }

    /// The abandoned template, which is the state of every machine that
    /// installed any release up to `be64867`.
    ///
    /// That installer wrote `~/.config/nacelle-desktop/nacelle-desktop.conf`
    /// with every key present and BLANK, under a comment of its own
    /// saying that empty means the defaults built into the program.
    /// Reading those blanks as explicit offs pinned the system file off
    /// — permanently, since that directory stands ahead of `/etc/xdg`
    /// and is deliberately never rewritten — and LOOK AND FEEL RESET
    /// then had nothing left to clear and no way to say so.
    #[test]
    fn the_blank_template_under_the_old_folder_name_does_not_pin_the_system_file_off() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("abandoned-template");
        let etc = root.join("etc");
        std::fs::create_dir_all(etc.join(FAMILY_DIR)).unwrap();
        std::fs::create_dir_all(root.join(LEGACY_FAMILY_DIR)).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", &etc);
        std::fs::write(
            etc.join(FAMILY_DIR).join(CONF_RON),
            "(\n    theme: Named(\"corporate\"),\n    layaut: Named(\"console\"),\n    \
             sounds: Named(\"classic\"),\n    term_font: (family: Named(\"Iosevka\")),\n)\n",
        )
        .unwrap();
        // Verbatim from that release, blanks and all.
        std::fs::write(
            root.join(LEGACY_FAMILY_DIR).join(CONF_FILE),
            "# Empty values or missing options = defaults built into the program.\n\
             Theme=\nLayaut=\nSounds=\nTermFontSize=\nTermFontFamily=\n\
             TermFontWeight=\nUIFontSize=\nUIFontFamily=\nUIFontWeight=\n",
        )
        .unwrap();

        let c = conf();
        assert_eq!(
            c.theme.name(),
            Some("corporate"),
            "a template nobody chose may not outrank the machine's own file"
        );
        assert_eq!(c.sounds.name(), Some("classic"));
        assert_eq!(c.layaut.name(), Some("console"));
        assert_eq!(c.term_font.family.name(), Some("Iosevka"));

        // And the reset still reaches the system file THROUGH it: the
        // old file is never rewritten, so this is the state it leaves
        // behind for good if the blanks are read as offs.
        set_engine_theme("crimson");
        assert_eq!(conf().theme.name(), Some("crimson"));
        clear_look_and_feel();
        assert_eq!(
            conf().theme.name(),
            Some("corporate"),
            "the reset must land on the system value, not on a blank"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same blank, on the keys that are SWITCHES rather than names.
    ///
    /// The template shipped none of these, but the file it shipped
    /// documented itself as one to edit — "Empty values or missing
    /// options = defaults built into the program" — so `SoundTyping=`
    /// with nothing after it is a line somebody typed and left, and the
    /// old reader answered it with the built-in default, exactly as it
    /// answered an absent one. Nothing on that machine could tell the
    /// two apart. Reading the blank as a value now would hand the user
    /// a switch they never flipped and let it outrank the system file
    /// for good, which is the whole of the template's failure one rung
    /// down.
    #[test]
    fn a_blank_switch_in_an_old_file_is_not_a_switch_anybody_flipped() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("blank-switches");
        let etc = root.join("etc");
        std::fs::create_dir_all(etc.join(FAMILY_DIR)).unwrap();
        std::fs::create_dir_all(root.join(LEGACY_FAMILY_DIR)).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", &etc);
        // A site that has decided about all three, against the
        // program's own defaults — which is the only arrangement under
        // which the difference is visible at all.
        std::fs::write(
            etc.join(FAMILY_DIR).join(CONF_RON),
            "(sound: (typing: false, ambient: false), grid: (snap: true))\n",
        )
        .unwrap();
        std::fs::write(
            root.join(LEGACY_FAMILY_DIR).join(CONF_FILE),
            "SoundTyping=\nSoundAmbient=\nGridSnap=\nSoundVolume=\n",
        )
        .unwrap();

        let c = conf();
        assert!(!c.sound.typing(), "a blank switch may not outrank the machine's own file");
        assert!(!c.sound.ambient());
        assert!(c.grid.snap(), "and the same blank read the other way round");
        // The numbers next to them have always been read this way: a
        // blank is not a number, so nothing is said. The switches now
        // agree with them.
        assert_eq!(c.sound.volume(), model::SoundConf::VOLUME);

        // A switch that was actually written still wins, blank
        // neighbours or not — this is not a rule about old files, it is
        // a rule about blanks.
        std::fs::write(
            root.join(LEGACY_FAMILY_DIR).join(CONF_FILE),
            "SoundTyping=1\nSoundAmbient=\nGridSnap=0\n",
        )
        .unwrap();
        let c = conf();
        assert!(c.sound.typing(), "1 is an answer and beats the system file");
        assert!(!c.grid.snap(), "so is 0");
        assert!(!c.sound.ambient(), "and the blank beside them still says nothing");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The other half of the same failure, and the one the blank
    /// template hid: a user who had CHOSEN things before the folder was
    /// renamed, so their old file holds real names rather than blanks.
    ///
    /// Measured before this was closed: the reset wrote a document
    /// holding `()` and nothing else, the old file in
    /// `~/.config/nacelle-desktop/` went on answering theme, sound set
    /// and volume, and the screen did not change. Not a blank
    /// outranking a system file this time — a second file of the
    /// user's own standing behind the first, which no amount of
    /// removing fields from the first can reach.
    #[test]
    fn a_users_old_folder_is_carried_across_by_the_first_write_and_then_answers_no_more() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("old-folder-carried");
        let etc = root.join("etc");
        std::fs::create_dir_all(etc.join(FAMILY_DIR)).unwrap();
        std::fs::create_dir_all(root.join(LEGACY_FAMILY_DIR)).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", &etc);
        std::fs::write(
            etc.join(FAMILY_DIR).join(CONF_RON),
            "(theme: Named(\"corporate\"), sounds: Named(\"classic\"))\n",
        )
        .unwrap();
        std::fs::write(
            root.join(LEGACY_FAMILY_DIR).join(CONF_FILE),
            "Theme=crimson\nSounds=quiet\nSoundVolume=40\nBlurRadius=55\n",
        )
        .unwrap();

        // Before anything is written the old file is the answer, which
        // is the promise the rename made and still keeps.
        let c = conf();
        assert_eq!(c.theme.name(), Some("crimson"));
        assert_eq!(c.sound.volume(), 40);

        // One setting changed — the whole of that file comes across,
        // including the parts the setting had nothing to do with.
        set_blur_opacity(80);
        let text = std::fs::read_to_string(root.join(FAMILY_DIR).join(CONF_RON)).unwrap();
        for carried in ["crimson", "quiet", "40", "55"] {
            assert!(text.contains(carried), "'{carried}' was left behind: {text}");
        }
        let c = conf();
        assert_eq!(c.sound.volume(), 40, "and nothing changed on screen by moving");
        assert_eq!(c.blur.radius(), 55);

        // Which is what lets the reset mean what it says.
        clear_look_and_feel();
        let c = conf();
        assert_eq!(
            c.theme.name(),
            Some("corporate"),
            "the system file must answer again, not the user's old folder"
        );
        assert_eq!(c.sounds.name(), Some("classic"));
        // What the reset does not cover is still the user's, from the
        // file it was carried into.
        assert_eq!(c.sound.volume(), 40, "a reset of the LOOK takes no sound with it");

        // And the old file is exactly where it was, untouched.
        assert_eq!(
            std::fs::read_to_string(root.join(LEGACY_FAMILY_DIR).join(CONF_FILE)).unwrap(),
            "Theme=crimson\nSounds=quiet\nSoundVolume=40\nBlurRadius=55\n",
            "superseded is not the same as rewritten"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A `Key=Value` file lying in the SAME directory as a `.ron` one
    /// is not merged with it — it is not read at all — and that is
    /// worth one line, because nothing else on the machine says so.
    ///
    /// The installer prints it for `/etc/xdg` at the moment it writes
    /// the new file. It cannot print it for the user's own directory,
    /// where no installer ever writes: the old file is there by having
    /// been there, and the `.ron` appears beside it the first time a
    /// setting is changed. So the program says it, from the one place
    /// that can see both files.
    #[test]
    fn a_dead_key_value_file_beside_the_ron_one_is_named_once_per_directory() {
        let root = scratch("dead-conf");
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(CONF_FILE), "Theme=crimson\n").unwrap();

        // Nothing to say while the old file is the only one there: it
        // is being READ, and `warn_once_about_conf_format` covers that.
        assert!(!warn_once_about_dead_conf(&dir));

        std::fs::write(dir.join(CONF_RON), "(theme: Named(\"azure\"))\n").unwrap();
        assert_eq!(
            read_conf_dir(&dir).unwrap().unwrap().theme.name(),
            Some("azure"),
            "the .ron answers whole"
        );
        // Said by that read, so this second ask is the guard's own test:
        // `read_conf_dir` is on the path of a page that redraws every
        // frame, and a line per frame is a line nobody reads.
        assert!(!warn_once_about_dead_conf(&dir), "once per directory, not once per read");

        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join(CONF_FILE), "Theme=crimson\n").unwrap();
        std::fs::write(other.join(CONF_RON), "()\n").unwrap();
        assert!(
            warn_once_about_dead_conf(&other),
            "the two ends of the cascade are two different people's problem"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The addon settings directories are actually installed, which is
    /// the difference between the whole half working and it being dead
    /// AND silent: an uninstalled read answers `Origin::Refused`, and a
    /// refusal says nothing to anybody because it means a programming
    /// error rather than a user's mistake.
    ///
    /// Measured through the toolkit rather than by asserting that a
    /// call happened, so the PATH SHAPE is under test too — `addons/`
    /// beside the program's own file, one file per addon.
    #[test]
    fn an_addon_settings_file_written_by_the_user_is_actually_found() {
        let _env = env_lock();
        let root = scratch("addon-settings");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let addons = root.join(FAMILY_DIR).join("addons");
        std::fs::create_dir_all(&addons).unwrap();
        std::fs::write(addons.join("shell.ron"), "(rows: 40)\n").unwrap();

        install_addon_settings();
        let (text, origin) = nacelle::settings::text("shell", "");
        assert_eq!(
            origin,
            nacelle::settings::Origin::File,
            "the file the user wrote must be the one the addon is handed"
        );
        assert_eq!(text.trim(), "(rows: 40)");

        // And an addon nobody wrote a file for is ordinary, not refused
        // — the difference the settings window reports on.
        assert_eq!(nacelle::settings::text("clock", "").1, nacelle::settings::Origin::Absent);

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A settings file the program cannot use is named AT STARTUP, and
    /// not one addon later.
    ///
    /// The toolkit fills `problems()` when somebody READS, so a host
    /// that only installs the directories has an empty report for as
    /// long as it takes a widget to be built — and `resolve()`, which
    /// is where the report is turned into the notice on screen, runs
    /// before any widget exists at all. The whole channel was therefore
    /// dead on the one run that matters: the first frame after the user
    /// edited the file. It also stays dead for an addon that is on no
    /// board, whose file is exactly as broken and never asked for.
    ///
    /// So the host reads what is THERE, once, on the way in.
    #[test]
    fn a_settings_file_the_host_cannot_use_is_named_before_any_widget_exists() {
        let _env = env_lock();
        let root = scratch("addon-settings-report");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let addons = root.join(FAMILY_DIR).join("addons");
        std::fs::create_dir_all(addons.join("search")).unwrap();
        // One file per addon, and one member of an addon's directory:
        // both arrangements the format has, both unreadable.
        std::fs::write(addons.join("filesystem.ron"), "(hidden: false\n").unwrap();
        std::fs::write(addons.join("search/engines.ron"), "(\n").unwrap();
        // ...and one that is perfectly good, which must not be named.
        std::fs::write(addons.join("shell.ron"), "(rows: 40)\n").unwrap();
        // Nor may anything be said about what is not a settings file:
        // the backup the toolkit takes stands in the same directory.
        std::fs::write(addons.join("filesystem.ron.bak"), "(hidden: true)\n").unwrap();
        // A name no addon can ask for is a bad NAME and not a bad file
        // — and it is the one the window has to say MOST about, because
        // nothing will ever ask for it and so nothing else will ever
        // mention it. Reported without an addon on it, since the whole
        // trouble is that the name is not an addon's.
        std::fs::write(addons.join("Weird.ron"), "(hidden: false\n").unwrap();

        install_addon_settings();

        let problems = nacelle::settings::problems();
        let named = |addon: &str, file: &str| {
            problems.iter().any(|p| p.addon == addon && p.file == file)
        };
        assert!(
            named("filesystem", ""),
            "a broken settings file is not in the report the notice reads: {problems:?}"
        );
        assert!(
            named("search", "engines"),
            "an addon's directory is not walked, so half the format is unreported"
        );
        assert!(
            !named("shell", ""),
            "a file that loads was reported as a problem"
        );
        // The whole of the fourth path: a settings window saying every
        // file loads while this one never will is the same half-truth
        // the toolkit refuses to tell about a file that does not parse,
        // except that this one can never repair itself.
        let refused = addons.join("Weird.ron");
        assert!(
            problems.iter().any(|p| p.path == refused && p.addon.is_empty()),
            "a name nothing can ever ask for is the one silence that never \
             ends, and it is not in the report: {problems:?}"
        );
        assert_eq!(problems.len(), 3, "something else was named too: {problems:?}");

        // What the walk picks up, stated where it can be read without a
        // directory: the settings file, never the copy the toolkit
        // leaves beside one it overwrites, and never the lock an editor
        // drops while the user is sitting IN the file.
        assert_eq!(
            stem_of(Path::new("/a/filesystem.ron"), Some("ron")).as_deref(),
            Some("filesystem")
        );
        assert_eq!(stem_of(Path::new("/a/filesystem.ron.bak"), Some("ron")), None);
        assert_eq!(stem_of(Path::new("/a/.#filesystem.ron"), Some("ron")), None);
        assert_eq!(stem_of(Path::new("/a/search"), None).as_deref(), Some("search"));

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same four states on the DATA side: a sound set installed
    /// under the old name goes on playing, and one under the new name
    /// shadows it exactly as a user install shadows a system one.
    #[test]
    fn a_sound_set_installed_under_the_old_name_is_still_found() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("data-fallback");
        std::env::set_var("XDG_DATA_HOME", &root);
        // Keeps the real /usr/share off this test's search path.
        std::env::set_var("XDG_DATA_DIRS", root.join("usr/share"));

        // NEITHER.
        assert!(find_asset("sounds", "classic").is_none());
        assert!(asset_dirs("sounds").is_empty());

        // OLD ONLY.
        let old = root.join(LEGACY_FAMILY_DIR).join("sounds").join("classic");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("meta"), "click=click.wav\n").unwrap();
        assert_eq!(
            find_asset("sounds", "classic").as_ref(),
            Some(&old),
            "a set installed under the old name must still be found"
        );
        assert!(list_sound_themes().iter().any(|n| n == "classic"));

        // BOTH: the new folder shadows the old one, and the set is
        // listed once rather than twice.
        let new = root.join(FAMILY_DIR).join("sounds").join("classic");
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("meta"), "click=click.wav\n").unwrap();
        assert_eq!(
            find_asset("sounds", "classic").as_ref(),
            Some(&new),
            "the new folder wins when both hold the same name"
        );
        assert_eq!(
            list_sound_themes().iter().filter(|n| *n == "classic").count(),
            1,
            "one set, listed once"
        );
        assert_eq!(asset_dirs("sounds").len(), 2, "both folders take part");

        // NEW ONLY.
        std::fs::remove_dir_all(root.join(LEGACY_FAMILY_DIR)).unwrap();
        assert_eq!(find_asset("sounds", "classic").as_ref(), Some(&new));

        std::env::remove_var("XDG_DATA_HOME");
        std::env::remove_var("XDG_DATA_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Writing goes to the new folder, in the new format, and ONLY
    /// there: the old file is left byte for byte as it was found.
    ///
    /// This is what makes the change reversible — a mistake costs the
    /// user nothing, because nothing of theirs was touched. It is also
    /// the whole of the migration: there is no conversion step that
    /// could go wrong halfway, only a new file that begins to answer
    /// first while the old one goes on answering for the rest.
    #[test]
    fn a_setting_is_written_to_the_new_folder_and_the_old_file_is_untouched() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("write-target");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));

        let old = root.join(LEGACY_FAMILY_DIR);
        std::fs::create_dir_all(&old).unwrap();
        let before = "# somebody's own file\nTheme=crimson\nSounds=classic\n";
        std::fs::write(old.join(CONF_FILE), before).unwrap();

        set_engine_theme("azure");

        let new = root.join(FAMILY_DIR).join(CONF_RON);
        assert!(new.is_file(), "the write must land in the new folder");
        let text = std::fs::read_to_string(&new).unwrap();
        assert!(
            text.contains("theme: Named(\"azure\")"),
            "the value must be in the file that was written: {text}"
        );
        assert!(
            text.starts_with("// nacelle-desktop settings"),
            "a file people edit must say what it is: {text}"
        );
        assert_eq!(
            std::fs::read_to_string(old.join(CONF_FILE)).unwrap(),
            before,
            "the user's old file may not be touched, moved or rewritten"
        );
        // And the program now reads the new value while the rest of the
        // old file still answers for everything it alone carries.
        let c = conf();
        assert_eq!(c.theme.name(), Some("azure"));
        assert_eq!(c.sounds.name(), Some("classic"));

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The write keeps a copy of what the USER wrote, and lands whole.
    ///
    /// A format parsed all or nothing raises the price of one bad
    /// write from a single setting to the entire file, and a file the
    /// user cannot get back is the difference between a format change
    /// and a bug with a version number.
    ///
    /// Which is why the copy is of a file this program did NOT write.
    /// A copy refreshed on every write is a copy of the previous save:
    /// the first nudge of a slider puts the user's document in `.bak`,
    /// the second replaces it with the first nudge's output, and after
    /// two keypresses the hand-written text is in neither file. This
    /// test spends most of its length on that second keypress, because
    /// that is where it used to go.
    #[test]
    fn every_write_keeps_the_copy_it_replaced_and_leaves_nothing_half_written() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("write-backup");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);

        set_engine_theme("crimson");
        assert!(
            !dir.join(CONF_RON_BACKUP).exists(),
            "the first write replaced nothing, so there is nothing to keep"
        );

        set_engine_theme("azure");
        assert!(
            !dir.join(CONF_RON_BACKUP).exists(),
            "a copy of this program's own output is not a backup of anything"
        );
        assert!(leftover_tmp(&dir).is_empty(), "the temporary name may not survive");
        assert_eq!(conf().theme.name(), Some("azure"));

        // A file of the user's own that cannot be parsed is replaced
        // rather than left in the way — and the copy is what makes that
        // defensible, so it has to hold the broken text verbatim.
        //
        // Comments and a field this build has never heard of, both of
        // which the serialiser drops on the way through: the copy is
        // the only place they can still be, so what it holds has to be
        // the bytes and not the document.
        let mine = "// mine, and nobody else's\n(theme: Named(\"crimson\", wallpaper: \"sea\")\n";
        std::fs::write(dir.join(CONF_RON), mine).unwrap();
        set_engine_theme("pure");
        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON_BACKUP)).unwrap(),
            mine,
            "what could not be parsed must still be recoverable"
        );
        assert_eq!(conf().theme.name(), Some("pure"));

        // The keypress that used to destroy it. Two more, in fact: a
        // slider does not stop at one.
        set_engine_theme("azure");
        set_engine_theme("crimson");
        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON_BACKUP)).unwrap(),
            mine,
            "the copy holds what the user wrote, however many saves follow"
        );
        assert!(leftover_tmp(&dir).is_empty(), "and no write left a temporary behind");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A RESTART IS NOT A REASON TO SPEND THE USER'S COPY.
    ///
    /// The rule was kept in a table in memory, so the first save after
    /// every start read this program's own file — written the day
    /// before, header and all — as a stranger's and copied it over the
    /// `.bak`. Whatever the user had written by hand, and the header in
    /// the file went on naming that copy as the place it was kept.
    ///
    /// The half this leaves standing is the one the header promises:
    /// a file the program did not write is copied aside, once, however
    /// many saves follow — across restarts as well now, which is what
    /// the second half of this test is about.
    #[test]
    fn a_restart_does_not_turn_the_programs_own_file_into_the_backup() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("restart-backup");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();

        // Session one: a document of the user's own — it parses, so
        // nothing here rests on a file being broken — and one save.
        let mine = "// mine, and nobody else's\n(\n    theme: Named(\"crimson\"),\n)\n";
        std::fs::write(dir.join(CONF_RON), mine).unwrap();
        set_engine_theme("azure");
        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON_BACKUP)).unwrap(),
            mine,
            "the copy is of what the user wrote"
        );

        // The restart, which is exactly this: a process that did not
        // write the file standing on the disk.
        CONF_WRITTEN.lock().unwrap().clear();

        set_engine_theme("pure");
        let kept = std::fs::read_to_string(dir.join(CONF_RON_BACKUP)).unwrap();
        assert_eq!(kept, mine, "a new process spent the user's only copy");
        assert!(
            !kept.contains("REWRITES this file"),
            "the copy holds this program's own output: {kept}"
        );
        assert_eq!(conf().theme.name(), Some("pure"), "and the setting still landed");
        assert!(leftover_tmp(&dir).is_empty(), "no write left a temporary behind");

        // What the recognition rests on, said directly. A file wearing
        // the header is not thereby the program's: a person who opens
        // what was written and adds a line of their own still gets
        // their copy, because the round trip does not reproduce it.
        let doctored = format!(
            "{CONF_HEADER}// and a note of my own\n(\n    theme: Named(\"pure\"),\n)\n"
        );
        assert!(!is_generated(&doctored), "a header is not a signature");
        assert!(
            is_generated(&conf_text(&DesktopConf::default()).unwrap()),
            "and what the writer produces must be recognised as its own"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A RELEASE IS THE OTHER WAY THIS COPY CAN BE SPENT, AND WHAT THE
    /// LIST OF HEADERS IS FOR.
    ///
    /// The restart was one half. The other half is the upgrade:
    /// recognition compares the file against a header this program
    /// WROTE, and if that meant the header the current build writes,
    /// then the first save after any release that edited a word of that
    /// prose would read the previous release's file as a stranger's and
    /// copy it over the `.bak` — the same loss as the restart's, moved
    /// from every start to every upgrade that touches a comment.
    ///
    /// So the recogniser asks a LIST, and this test is about the day
    /// that list has two entries on it. Both directions are stated: the
    /// old release's file is ours once its header is on the list, and it
    /// is NOT ours while the list has never heard of it — which is
    /// exactly the loss, and exactly why editing [`CONF_HEADER`] without
    /// putting the old text into [`CONF_HEADERS`] is a thing a test has
    /// to refuse. That test is
    /// `the_recognition_is_pinned_to_what_was_shipped`.
    #[test]
    fn a_file_from_a_release_with_other_prose_in_its_header_is_still_ours() {
        fixture_registry();
        let doc = DesktopConf::default();
        // Last release's output: this build's body under a header whose
        // prose was since edited — one word, which is all it takes.
        let shipped = CONF_HEADER.replacen("REWRITES", "rewrites", 1);
        let last_release = format!("{shipped}{}", conf_body(&doc).unwrap());
        assert_ne!(last_release, conf_text(&doc).unwrap(), "the drift is real");

        assert!(
            !is_generated_under(&last_release, &[CONF_HEADER]),
            "this is the loss: one word of prose and last week's file is a stranger"
        );
        assert!(
            is_generated_under(&last_release, &[CONF_HEADER, &shipped]),
            "a header this program shipped is a header it recognises"
        );

        // And the list does not become a way in. What makes a file the
        // user's is the same under two headers as under one: their text
        // is in the body, and the body has to round-trip whichever
        // header stands above it.
        let doctored = format!("{shipped}// and a note of my own\n(\n    theme: Named(\"pure\"),\n)\n");
        assert!(
            !is_generated_under(&doctored, &[CONF_HEADER, &shipped]),
            "a known header is not a signature either"
        );
        // Nor is a header the right prefix of somebody else's document:
        // the bytes after it are compared, all of them.
        let truncated = format!("{shipped}(\n)\n");
        assert!(!is_generated_under(&truncated, &[CONF_HEADER, &shipped]));
    }

    /// THE TRIPWIRE. It has no other job.
    ///
    /// [`is_generated`] recognises a file by the bytes some build of
    /// this program wrote, so two things it does not own are frozen the
    /// moment a release ships: the header's prose, and the exact shape
    /// [`ron_pretty`] serialises in. Change either and every file
    /// already on disk stops being recognised — the first save after the
    /// upgrade takes it for a stranger's document and copies it over the
    /// `.bak` that held the user's real one.
    ///
    /// Neither of those is a thing anybody would think of as a promise
    /// while editing it, which is why this test states it instead of the
    /// comment. What to do when it fails is in the message.
    #[test]
    fn the_recognition_is_pinned_to_what_was_shipped() {
        // FNV-1a, chosen for being four lines rather than for being
        // strong: this compares against a number written down, it does
        // not defend against anybody.
        fn fingerprint(s: &str) -> u64 {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in s.as_bytes() {
                h = (h ^ *b as u64).wrapping_mul(0x100_0000_01b3);
            }
            h
        }

        assert_eq!(
            fingerprint(CONF_HEADER),
            0x6c08_0c76_913a_f370,
            "CONF_HEADER changed. Every file the previous release wrote wears \
             the OLD text, and is now a stranger's document whose first save \
             overwrites the user's .bak. Put the old header — `git show \
             HEAD:src/config.rs` — into CONF_HEADERS as a second entry, then \
             pin the new fingerprint here."
        );
        // A document with something IN it, and nested: the empty one
        // serialises to two characters, so an indentor or a separator
        // could change under it without moving a byte.
        let doc = DesktopConf {
            theme: Choice::Named("crimson".into()),
            variant: Choice::Off,
            screens: [("DP-1".to_string(), Choice::Named("cockpit".into()))]
                .into_iter()
                .collect(),
            term_font: model::FontConf { size: Some(112.5), ..Default::default() },
            color: model::ColorConf {
                space: Choice::Named("bt2020 pq".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            fingerprint(&conf_body(&doc).unwrap()),
            0x9b8c_4760_84d8_3f4a,
            "the serialised shape changed — ron_pretty, a renamed field, a new \
             one with a default. Files already on disk carry the old shape and \
             no longer round-trip, so they read as strangers' and their saves \
             spend the .bak. If the change is deliberate, this number is the \
             thing to update, and CONF_HEADERS cannot help: only a reader of \
             the old shape could."
        );
        // And the list is a list of DISTINCT headers, the current one at
        // its head — an entry equal to another buys nothing and an
        // entry that is not the current one being first would mean the
        // writer and the recogniser had come apart.
        assert_eq!(CONF_HEADERS.first(), Some(&CONF_HEADER));
        for (i, a) in CONF_HEADERS.iter().enumerate() {
            for b in &CONF_HEADERS[i + 1..] {
                assert_ne!(a, b, "the same header twice in CONF_HEADERS");
            }
        }
    }

    /// Every temporary file left standing in a directory. Matched on
    /// the infix rather than on a whole name, because the whole name is
    /// the point: a temporary carries the writing process's id, so
    /// there is no single name a test could ask about.
    fn leftover_tmp(dir: &Path) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
        rd.flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|n| n.contains(CONF_RON_TMP))
            .collect()
    }

    /// The user's old folder is only superseded by a file that actually
    /// CARRIES it.
    ///
    /// The state is ordinary and it is the worst one there is: an old
    /// file with a bracket missing, in the folder this program used to
    /// write to. The first setting anybody changes writes a document in
    /// the NEW folder holding that one setting and nothing else —
    /// nothing could be carried across, because nothing could be read —
    /// and if the new file's mere existence retires the old folder, the
    /// whole configuration is gone: theme, layaut, sounds, all answered
    /// by the defaults, with a file sitting on the disk that says
    /// otherwise and is no longer read by anything.
    ///
    /// What the user does next is repair the typo. That is the moment
    /// this test is about: their settings have to come back.
    #[test]
    fn an_old_file_that_was_never_carried_across_goes_on_being_read() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("supersede-unread");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let old = root.join(LEGACY_FAMILY_DIR);
        std::fs::create_dir_all(&old).unwrap();
        let _ = take_conf_rescued();

        // Everything they have, one bracket short of parsing.
        let mine = "(\n    theme: Named(\"crimson\"\n    sounds: Named(\"classic\"),\n)\n";
        std::fs::write(old.join(CONF_RON), mine).unwrap();

        // One nudge of the volume slider, which is what writes the new
        // folder's file for the first time.
        set_sound_volume(72);
        assert_eq!(conf().sound.volume(), 72, "the setting asked for took");
        assert!(
            conf_error().is_some(),
            "a file of the user's that could not be read is not a file to say \
             nothing about"
        );
        assert_eq!(
            std::fs::read_to_string(old.join(CONF_RON)).unwrap(),
            mine,
            "and nothing was rewritten, moved or deleted"
        );

        // The typo repaired — one character, in an editor, in the file
        // that was there all along.
        let repaired = "(\n    theme: Named(\"crimson\"),\n    sounds: Named(\"classic\"),\n)\n";
        std::fs::write(old.join(CONF_RON), repaired).unwrap();

        let c = conf();
        assert_eq!(
            c.theme.name(),
            Some("crimson"),
            "a file nothing was taken out of may not be cut out of the cascade"
        );
        assert_eq!(c.sounds.name(), Some("classic"));
        assert_eq!(c.sound.volume(), 72, "and the new file still answers for what it holds");
        assert_eq!(conf_error(), None, "with nothing left to complain about");

        // Now the carry can happen, and one nudge is what does it: the
        // old folder retires only once its bytes are in the new file.
        set_sound_volume(73);
        assert!(
            std::fs::read_to_string(dir_of(&root).join(CONF_RON)).unwrap().contains("crimson"),
            "the write that retires the old folder is the one that carries it"
        );
        assert_eq!(conf().theme.name(), Some("crimson"));

        // And from here the reset works, which is what the retirement
        // was for — including on the write AFTER it, which used to find
        // every cleared field still sitting in the old folder and put
        // them all back.
        clear_look_and_feel();
        set_sound_volume(74);
        assert_eq!(
            conf().theme.name(),
            None,
            "a folder that has been carried across may not be seeded from either"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The configuration directory under a scratch root.
    fn dir_of(root: &Path) -> PathBuf {
        root.join(FAMILY_DIR)
    }

    /// A configuration kept in a dotfiles repository and linked into
    /// place stays LINKED to it.
    ///
    /// The loss here is quieter than any other on this page, which is
    /// what makes it worth a test of its own: every value survives the
    /// day it happens. `rename` replaces the link with a plain file,
    /// the settings are all still there, and from that moment the
    /// user's repository — the thing they think of as their
    /// configuration — is a file nothing reads. They edit it, restart,
    /// and the edit did not take; there is no message, because from the
    /// program's side nothing went wrong.
    #[test]
    fn a_configuration_linked_in_from_elsewhere_is_written_through_the_link() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("conf-symlink");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();

        let store = root.join("dotfiles");
        std::fs::create_dir_all(&store).unwrap();
        let real = store.join("nacelle-desktop.ron");
        std::fs::write(&real, "// mine\n(\n    theme: Named(\"crimson\"),\n)\n").unwrap();
        std::os::unix::fs::symlink(&real, dir.join(CONF_RON)).unwrap();

        set_sound_volume(72);

        assert!(
            std::fs::symlink_metadata(dir.join(CONF_RON)).unwrap().file_type().is_symlink(),
            "the link the user made is theirs, and a write may not eat it"
        );
        let written = std::fs::read_to_string(&real).unwrap();
        assert!(
            written.contains("volume: 72"),
            "the setting has to land in the file the link points at: {written}"
        );
        assert!(
            std::fs::read_to_string(store.join(CONF_RON_BACKUP)).unwrap().contains("// mine"),
            "and the copy of what was replaced belongs beside that file, not \
             beside the link"
        );

        // The point of the whole arrangement: the repository is still
        // the source of truth, so an edit made there is what the
        // program reads.
        std::fs::write(&real, "(\n    theme: Named(\"azure\"),\n)\n").unwrap();
        assert_eq!(conf().theme.name(), Some("azure"));

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A write that cannot happen is on the SCREEN.
    ///
    /// This is the branch that loses least — the file on disk is whole,
    /// and it is left whole — which is exactly why it was easy to leave
    /// silent. But the slider springs back and nothing explains it, and
    /// every change made from here on goes nowhere: a permanent, quiet
    /// loss of everything the user does next, which is worse to find
    /// out by accident than a file that was replaced.
    #[test]
    fn a_setting_that_could_not_be_saved_is_not_only_said_to_stderr() {
        // Meaningless as root, who may write a directory whatever its
        // permissions say.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        fixture_registry();
        let _env = env_lock();
        let root = scratch("write-refused");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let mine = "(\n    theme: Named(\"crimson\"),\n)\n";
        std::fs::write(dir.join(CONF_RON), mine).unwrap();
        let _ = take_conf_rescued();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        set_sound_volume(72);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON)).unwrap(),
            mine,
            "nothing on disk may be touched by a write that could not happen"
        );
        let said = take_conf_rescued()
            .expect("a setting that went nowhere may not go there quietly");
        assert!(
            said.contains(&dir.join(CONF_RON).display().to_string()),
            "and the sentence has to name the file it could not write: {said}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Two processes writing at the same moment do not share a
    /// temporary name.
    ///
    /// The settings window and the running desktop are two processes —
    /// this file says so itself, in `keep_broken_text`, and claims its
    /// rescue names exclusively for that reason. The temporary the
    /// configuration is written through had one fixed name and was
    /// opened with a `File::create`, which TRUNCATES: both processes
    /// hold the same file, and the rename of whichever finishes first
    /// publishes what the other is still writing. For a format parsed
    /// all or nothing that is not a lost setting, it is a lost file.
    ///
    /// Measured on the leftover, because the race itself is not
    /// reproducible on demand: what another process is in the middle of
    /// writing must still be there afterwards, untouched.
    #[test]
    fn a_second_writer_does_not_write_through_this_ones_temporary() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("tmp-shared");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();

        // Half a document, under the one name every process used to
        // take. Whoever is writing it has not finished.
        let theirs = dir.join("nacelle-desktop.ron.new");
        let half = "(\n    theme: Named(\"crim";
        std::fs::write(&theirs, half).unwrap();

        set_sound_volume(72);

        assert_eq!(
            std::fs::read_to_string(&theirs).unwrap(),
            half,
            "a name this write did not claim is somebody else's file"
        );
        assert_eq!(conf().sound.volume(), 72, "and this write still landed");

        // The claim itself: two of them running at once cannot come
        // back with the same name, and the name says which process it
        // belongs to.
        let path = dir.join(CONF_RON);
        let (a, _held) = claim_tmp(&path).unwrap();
        let (b, _also) = claim_tmp(&path).unwrap();
        assert_ne!(a, b, "a name that is merely checked is a name two writers get");
        let name = a.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            name.contains(&std::process::id().to_string()),
            "the id is what makes the name unique among the processes that are \
             alive: {name}"
        );
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// LOOK AND FEEL RESET, and the reason it needed the format
    /// changed under it: it has to REMOVE the user's fields, not write
    /// them empty. An empty value wins the cascade, so a reset made of
    /// empties would pin the system defaults OFF — on a machine that
    /// has a system file, which is exactly what this change installs.
    ///
    /// The measurement is the one that failed before: a system file
    /// naming a theme, a layaut, a sound set and a variant, a user who
    /// had chosen otherwise, and the system's answers standing again
    /// afterwards.
    #[test]
    fn the_reset_takes_the_users_fields_out_and_lets_the_system_file_answer() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("reset-clears");
        let etc = root.join("etc");
        std::fs::create_dir_all(etc.join(FAMILY_DIR)).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", &etc);
        std::fs::write(
            etc.join(FAMILY_DIR).join(CONF_RON),
            "(\n    theme: Named(\"azure\"),\n    variant: Named(\"hc\"),\n    \
             layaut: Named(\"console\"),\n    sounds: Named(\"classic\"),\n    \
             screens: {\"DP-1\": Named(\"cockpit\")},\n    \
             term_font: (size: 130, family: Named(\"Iosevka\")),\n    \
             blur: (radius: 40),\n)\n",
        )
        .unwrap();

        // The user disagrees with all of it, including by switching
        // two things OFF — the state an empty value used to leave
        // behind, and the one a reset must also undo.
        set_engine_theme("crimson");
        set_engine_variant(None);
        set_layaut_option("hangar");
        set_layaut_for_screen("DP-1", "");
        set_sounds_option("quiet");
        set_term_font_size(80);
        set_term_font_family("Fira Code");
        set_grid_padding(24);
        set_blur_radius(90);
        assert_eq!(conf().theme.name(), Some("crimson"));
        assert_eq!(conf().variant, Choice::Off, "off is a value of its own");
        assert!(conf().screens().is_empty(), "the screen was switched off");

        clear_look_and_feel();

        let c = conf();
        assert_eq!(c.theme.name(), Some("azure"), "the system theme must answer again");
        assert_eq!(
            c.variant.name(),
            Some("hc"),
            "an explicit off that was never removed would have blocked this"
        );
        assert_eq!(c.layaut.name(), Some("console"));
        assert_eq!(c.sounds.name(), Some("classic"));
        assert_eq!(
            c.screens().get("DP-1").map(String::as_str),
            Some("cockpit"),
            "the per-screen assignments go too, or a second monitor stays pinned"
        );
        assert_eq!(c.term_font.scale(0.5, 2.0), 1.3, "and both font sections, whole");
        assert_eq!(c.term_font.family.name(), Some("Iosevka"));
        // The band around every panel. It is typed on the GRID page,
        // which is why it was missed, but it overrides the theme's
        // `layout.panel_gutter` and nothing else — so a number left
        // standing here would be the user's own spacing around a look
        // they have just taken back.
        assert_eq!(
            c.grid.padding(),
            None,
            "the panel gutter is an override of a theme token, so it goes too"
        );

        // What the reset does NOT touch stays the user's: this is a
        // LOOK AND FEEL reset, not a factory reset.
        assert_eq!(c.blur.radius(), 90, "the glass is not part of look and feel here");

        // And the user's file no longer carries a word about any of it.
        let text = std::fs::read_to_string(root.join(FAMILY_DIR).join(CONF_RON)).unwrap();
        for gone in ["theme", "variant", "layaut", "sounds", "screens", "term_font"] {
            assert!(!text.contains(gone), "'{gone}' must be REMOVED, not emptied: {text}");
        }

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The warning is said ONCE. `config_dirs` and `data_dirs` are
    /// called on every read, so the failure this guards against is not
    /// a wrong sentence but a right one printed per frame.
    ///
    /// Hermetic: its own flag and its own directory, no process-wide
    /// state and no environment.
    #[test]
    fn the_warning_about_the_old_folder_is_said_once_and_not_every_frame() {
        use std::sync::atomic::AtomicBool;
        let root = scratch("legacy-warning");
        let path = vec![root.join(FAMILY_DIR), root.join(LEGACY_FAMILY_DIR)];
        std::fs::create_dir_all(root.join(FAMILY_DIR)).unwrap();
        std::fs::create_dir_all(root.join(LEGACY_FAMILY_DIR)).unwrap();

        // The first read that can land in the old folder says so.
        let said = AtomicBool::new(false);
        assert!(warn_once_about_legacy("data", &path, &said), "the first read must say it");

        // Every read after it is silent — this is the whole assertion.
        for _ in 0..1000 {
            assert!(
                !warn_once_about_legacy("data", &path, &said),
                "the warning came back a second time"
            );
        }

        // A machine with no old folder on its path is told nothing at
        // all, however often it is asked.
        let quiet = AtomicBool::new(false);
        assert!(!warn_once_about_legacy("configuration", &path[..1], &quiet));
        assert!(!warn_once_about_legacy("configuration", &path[..1], &quiet));

        let _ = std::fs::remove_dir_all(&root);
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

        // A full-base rewrite keeps the boards but DROPS the per-screen
        // sections: SAVE writes one arrangement every screen shares, so a
        // second monitor can no longer diverge into a section of its own.
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
        assert!(
            !after.contains("[1280x720@7]"),
            "a full save drops per-screen sections: one arrangement for all screens"
        );

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
        let ldir = dir.join(FAMILY_DIR).join("layauts");
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
    ///
    /// Written in the OLD format on purpose: the bracket family it
    /// invented is the thing the map replaces, and every one of those
    /// files still has to read the way it always did.
    #[test]
    fn every_screen_takes_the_layaut_its_connector_is_assigned() {
        fixture_registry();
        let doc = DesktopConf::from_legacy(&parse_kv(
            "# the desktop\n\
             Layaut=console\n\
             Layaut[DP-1]=cockpit\n\
             Layaut [eDP-1] = panel\n\
             Layaut[HDMI-A-1]=\n\
             Layaut[Dell Inc.]=nonsense\n\
             Theme=default\n",
        ));
        let assigned = doc.screens();
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
            doc.layaut.name(),
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
            let id = match connector {
                Some(c) => ScreenId::of_connector(c),
                None => ScreenId::default(),
            };
            let got = choose_layaut(&id, &assigned, "console", &installed);
            assert_eq!(got.name, want, "screen {connector:?} takes '{want}'");
            assert!(got.note.is_none(), "nothing to report for {connector:?}");
        }
        // The user typed the connector in another case than RandR says
        // it; it is the same socket and the same screen.
        assert_eq!(
            choose_layaut(&ScreenId::of_connector("edp-1"), &assigned, "console", &installed).name,
            "panel"
        );
    }

    /// Writing an assignment, and taking it back. What matters as much
    /// as the value is that everything else in the file comes out
    /// untouched: one screen's arrangement is not the others', and it
    /// is certainly not the theme.
    #[test]
    fn an_assignment_is_written_beside_the_rest_of_the_file() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("assign-screen");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        set_engine_theme("crimson");
        set_layaut_option("console");

        set_layaut_for_screen("DP-1", "cockpit");
        let c = conf();
        assert_eq!(
            c.screens().get("DP-1").map(String::as_str),
            Some("cockpit"),
            "what was written must read back"
        );
        assert_eq!(
            c.layaut.name(),
            Some("console"),
            "the default layaut is a different field and must not be touched"
        );
        assert_eq!(c.theme.name(), Some("crimson"));

        // Assigning again replaces the entry rather than adding a second.
        set_layaut_for_screen("DP-1", "hangar");
        let text = std::fs::read_to_string(root.join(FAMILY_DIR).join(CONF_RON)).unwrap();
        assert_eq!(text.matches("\"DP-1\"").count(), 1, "one entry per screen: {text}");
        assert_eq!(conf().screens().get("DP-1").map(String::as_str), Some("hangar"));

        // Clearing writes an explicit off: the assignment is gone, and
        // the entry stays to overrule a system file that makes one.
        set_layaut_for_screen("DP-1", "");
        assert_eq!(conf().screens.get("DP-1"), Some(&Choice::Off), "an off, not an absence");
        assert!(conf().screens().is_empty(), "and no screen is assigned anything");

        // A key nothing could match a screen to is never written.
        assert_eq!(screen_layaut_key("HDMI-A-1").as_deref(), Some("HDMI-A-1"));
        for bad in ["", "Dell Inc. U2720Q", "DP-1]", "screen 2"] {
            assert!(screen_layaut_key(bad).is_none(), "'{bad}' must not become a key");
            set_layaut_for_screen(bad, "cockpit");
        }
        assert_eq!(conf().screens.len(), 1, "and nothing of the sort reached the file");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A screen assigned a layaut this machine does not have. The
    /// desktop starts, that screen shows the default, and the log gets
    /// one sentence naming the screen, the layaut and what it took
    /// instead — a mistake in a file is not a reason not to start.
    #[test]
    fn an_assignment_to_a_layaut_that_is_not_installed_falls_back_to_the_default() {
        fixture_registry();
        let assigned = DesktopConf::from_legacy(&parse_kv("Layaut[DP-1]=cockpit\n")).screens();
        let installed = ["default".to_string(), "console".to_string()];
        let got = choose_layaut(&ScreenId::of_connector("DP-1"), &assigned, "console", &installed);
        assert_eq!(got.name, "console", "the screen falls back to the default layaut");
        let note = got.note.expect("a fallback must say so");
        assert!(note.contains("DP-1"), "the sentence names the screen: {note}");
        assert!(note.contains("cockpit"), "and the layaut that is missing: {note}");
        assert!(note.contains("console"), "and what it took instead: {note}");

        // The same rule keeps a hand-written value out of the paths
        // built from it: only a name the store listed is ever chosen.
        let evil =
            DesktopConf::from_legacy(&parse_kv("Layaut[DP-1]=../../etc/passwd\n")).screens();
        assert_eq!(
            choose_layaut(&ScreenId::of_connector("DP-1"), &evil, "console", &installed).name,
            "console"
        );
    }

    /// The point of keying screens by connector: which monitor comes
    /// up first is not a property of anything. The same two screens
    /// surveyed in either order take the same two layauts, and a
    /// position in the list would have swapped them.
    #[test]
    fn an_assignment_survives_the_screens_coming_up_in_another_order() {
        fixture_registry();
        let assigned = DesktopConf::from_legacy(&parse_kv(
            "Layaut[DP-1]=cockpit\nLayaut[HDMI-A-1]=hangar\n",
        ))
        .screens();
        let installed = [
            "default".to_string(),
            "console".to_string(),
            "cockpit".to_string(),
            "hangar".to_string(),
        ];
        let survey = |order: [&str; 2]| -> Vec<String> {
            order
                .iter()
                .map(|c| {
                    choose_layaut(&ScreenId::of_connector(c), &assigned, "console", &installed).name
                })
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

    /// A monitor now, not a socket. The same screen moved from one
    /// socket to another keeps its layaut, which is the thing a key
    /// naming the socket could never do — and the whole reason the key
    /// changed.
    #[test]
    fn a_layaut_follows_the_monitor_and_not_the_cable() {
        fixture_registry();
        let installed =
            ["default".into(), "console".into(), "cockpit".into(), "panel".to_string()];
        let assigned = DesktopConf {
            screens: [
                ("edid:DEL-41B2-0123ABCD".to_string(), Choice::Named("cockpit".into())),
                ("eDP-1".to_string(), Choice::Named("panel".into())),
            ]
            .into_iter()
            .collect(),
            ..DesktopConf::default()
        }
        .screens();

        let dell = |socket: &str| ScreenId {
            edid: Some("DEL-41B2-0123ABCD".into()),
            connector: Some(socket.to_string()),
        };
        for socket in ["DP-1", "HDMI-A-1", "DP-4"] {
            assert_eq!(
                choose_layaut(&dell(socket), &assigned, "console", &installed).name,
                "cockpit",
                "the Dell keeps its desktop plugged into {socket}"
            );
        }
        // A key naming a SOCKET goes on meaning the socket: whatever is
        // plugged into the laptop's own panel takes that layaut.
        assert_eq!(
            choose_layaut(&ScreenId::of_connector("eDP-1"), &assigned, "console", &installed).name,
            "panel"
        );
        // Another monitor on the socket the Dell used to be on takes
        // nothing of the Dell's.
        let other = ScreenId { edid: Some("GSM-5B0F".into()), connector: Some("DP-1".into()) };
        assert_eq!(choose_layaut(&other, &assigned, "console", &installed).name, "console");
    }

    /// NOTHING IS LOST BY THE CHANGE OF KEY, before any migration has
    /// run and whether or not one ever does: a file keyed by connector
    /// goes on answering for a monitor that now has a name of its own,
    /// and a monitor's own key is read first when the file holds both.
    #[test]
    fn a_file_written_against_a_socket_still_answers_for_the_monitor_on_it() {
        fixture_registry();
        let installed =
            ["default".into(), "console".into(), "cockpit".into(), "hangar".to_string()];
        let old = DesktopConf::from_legacy(&parse_kv("Layaut[DP-1]=cockpit\n")).screens();
        let dell = ScreenId {
            edid: Some("DEL-41B2-0123ABCD".into()),
            connector: Some("DP-1".into()),
        };
        assert_eq!(
            choose_layaut(&dell, &old, "console", &installed).name,
            "cockpit",
            "the socket key answers for the screen on that socket"
        );

        let both = DesktopConf {
            screens: [
                ("DP-1".to_string(), Choice::Named("hangar".into())),
                ("edid:DEL-41B2-0123ABCD".to_string(), Choice::Named("cockpit".into())),
            ]
            .into_iter()
            .collect(),
            ..DesktopConf::default()
        }
        .screens();
        assert_eq!(
            choose_layaut(&dell, &both, "console", &installed).name,
            "cockpit",
            "a rule about THIS MONITOR beats a rule about the socket it is on"
        );
    }

    /// THE MIGRATION, and what it is for: every per-screen assignment
    /// written before screens were keyed by their monitors has to
    /// survive, or a user's arrangement quietly becomes nobody's.
    ///
    /// Pure, so it says what the rule is rather than what this machine's
    /// screens happen to be.
    #[test]
    fn the_old_screen_map_survives_being_keyed_by_the_monitor() {
        let mut doc = DesktopConf {
            screens: [
                ("DP-1".to_string(), Choice::Named("cockpit".into())),
                ("eDP-1".to_string(), Choice::Named("panel".into())),
                ("HDMI-A-1".to_string(), Choice::Off),
            ]
            .into_iter()
            .collect(),
            main_screen: Choice::Named("DP-1".into()),
            layaut: Choice::Named("console".into()),
            ..DesktopConf::default()
        };
        // What the machine can see: the Dell on DP-1 and a nameless
        // panel on HDMI-A-1 both say who they are; the laptop's own
        // screen says nothing, and there is nothing on eDP-1 today.
        let live = [
            ScreenId {
                edid: Some("DEL-41B2-0123ABCD".into()),
                connector: Some("DP-1".into()),
            },
            ScreenId { edid: Some("GSM-5B0F".into()), connector: Some("HDMI-A-1".into()) },
            ScreenId::of_connector("DP-9"),
        ];

        assert!(doc.migrate_screens(&live), "there was something to move");
        assert_eq!(
            doc.screens.get("edid:DEL-41B2-0123ABCD"),
            Some(&Choice::Named("cockpit".into())),
            "the assignment moved to the monitor that was on that socket"
        );
        assert!(!doc.screens.contains_key("DP-1"), "and it moved rather than being copied");
        assert_eq!(
            doc.screens.get("edid:GSM-5B0F"),
            Some(&Choice::Off),
            "an explicit off is a setting too and moves whole"
        );
        assert_eq!(
            doc.screens.get("eDP-1"),
            Some(&Choice::Named("panel".into())),
            "a socket with nothing plugged in is left exactly as it was"
        );
        assert_eq!(
            doc.main_screen,
            Choice::Named("edid:DEL-41B2-0123ABCD".into()),
            "the main screen role travels the same road"
        );
        assert_eq!(doc.layaut.name(), Some("console"), "and nothing else in the file is touched");

        // Run twice and nothing happens the second time: a machine with
        // nothing to move must not be written to.
        assert!(!doc.migrate_screens(&live), "the same file migrated twice is the same file");

        // AND THE POINT OF ALL OF IT: the Dell keeps its desktop after
        // somebody moves its cable, which is what the old key could not
        // survive.
        let installed = ["default".into(), "console".into(), "cockpit".to_string()];
        let moved =
            ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-3".into()) };
        assert_eq!(choose_layaut(&moved, &doc.screens(), "console", &installed).name, "cockpit");
    }

    /// TWO OF THE SAME MONITOR, which is the case that costs somebody a
    /// desktop if the migration is not asked whether a name names one
    /// screen or two.
    ///
    /// The AORUS FO32U2P prints 0x01010101 in the serial field of every
    /// unit sold, so a pair of them is one identity — and the identity
    /// is looked up before the socket. Migrating the first one's socket
    /// entry to that shared name would hand its layaut to BOTH screens,
    /// and the second one's entry would still be in the file, still
    /// correct, and never read again.
    #[test]
    fn two_of_the_same_monitor_keep_the_desktops_their_sockets_name() {
        fixture_registry();
        let mut doc = DesktopConf {
            screens: [
                ("DP-1".to_string(), Choice::Named("cockpit".into())),
                ("DP-2".to_string(), Choice::Named("hangar".into())),
            ]
            .into_iter()
            .collect(),
            ..DesktopConf::default()
        };
        let twin = |socket: &str| ScreenId {
            edid: Some("GBT-3215-01010101".into()),
            connector: Some(socket.to_string()),
        };
        let live = [twin("DP-1"), twin("DP-2")];

        assert!(
            !doc.migrate_screens(&live),
            "a name both screens answer to is a name neither of them may be written under"
        );
        assert_eq!(doc.screens.get("DP-1"), Some(&Choice::Named("cockpit".into())));
        assert_eq!(doc.screens.get("DP-2"), Some(&Choice::Named("hangar".into())));
        assert!(
            !doc.screens.keys().any(|k| k.starts_with(crate::screens::EDID_PREFIX)),
            "and no shared name was written: {:?}",
            doc.screens
        );

        // Which is the whole point: both keep the desktop they had.
        let installed = ["default".into(), "console".into(), "cockpit".into(), "hangar".to_string()];
        let assigned = doc.screens();
        assert_eq!(choose_layaut(&live[0], &assigned, "console", &installed).name, "cockpit");
        assert_eq!(choose_layaut(&live[1], &assigned, "console", &installed).name, "hangar");

        // The role is the same question asked once: "the main screen"
        // cannot be a name two screens answer to either.
        let mut role = DesktopConf {
            main_screen: Choice::Named("DP-2".into()),
            ..DesktopConf::default()
        };
        assert!(!role.migrate_screens(&live));
        assert_eq!(
            role.main_screen,
            Choice::Named("DP-2".into()),
            "the socket is the only vocabulary that still tells the two apart"
        );

        // One of them alone is a perfectly ordinary monitor, and this
        // is what says the refusal above is about the AMBIGUITY and not
        // about the model.
        let mut alone = DesktopConf {
            screens: [("DP-1".to_string(), Choice::Named("cockpit".into()))].into_iter().collect(),
            ..DesktopConf::default()
        };
        assert!(alone.migrate_screens(&[twin("DP-1")]));
        assert_eq!(
            alone.screens.get("edid:GBT-3215-01010101"),
            Some(&Choice::Named("cockpit".into()))
        );
    }

    /// A RULE ABOUT A SOCKET, written on purpose, into a file that has
    /// already been migrated. The program must not argue with it.
    ///
    /// The socket vocabulary is documented as being good for "whatever
    /// is plugged in here", and a promise that only holds until the
    /// next start is not one. So the migration is bounded by the one
    /// thing that says WHEN a document was written: a document naming a
    /// monitor anywhere is a document written since monitors could be
    /// named, and its socket keys are somebody's decision rather than
    /// the old vocabulary.
    #[test]
    fn a_socket_rule_in_a_file_that_names_a_monitor_is_left_alone() {
        fixture_registry();
        // Last month's migration gave the LG its own name. Today the
        // user writes a rule about the socket the Dell hangs off.
        let mut doc = DesktopConf {
            screens: [
                ("edid:GSM-5B0F".to_string(), Choice::Named("hangar".into())),
                ("DP-1".to_string(), Choice::Named("cockpit".into())),
            ]
            .into_iter()
            .collect(),
            ..DesktopConf::default()
        };
        let live = [
            ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-1".into()) },
            ScreenId { edid: Some("GSM-5B0F".into()), connector: Some("HDMI-A-1".into()) },
        ];

        assert!(!doc.migrate_screens(&live), "this file has nothing left to migrate");
        assert_eq!(
            doc.screens.get("DP-1"),
            Some(&Choice::Named("cockpit".into())),
            "the rule about the socket stands: rewriting it would be an argument, not a migration"
        );
        assert!(
            !doc.screens.contains_key("edid:DEL-41B2-0123ABCD"),
            "and no rule about the Dell was invented from it"
        );

        // The same bound reached through the ROLE: naming the main
        // screen by its monitor is naming a monitor.
        let mut role = DesktopConf {
            screens: [("DP-1".to_string(), Choice::Named("cockpit".into()))].into_iter().collect(),
            main_screen: Choice::Named("edid:GSM-5B0F".into()),
            ..DesktopConf::default()
        };
        assert!(!role.migrate_screens(&live));
        assert_eq!(role.screens.get("DP-1"), Some(&Choice::Named("cockpit".into())));

        // And a document that has never named one is still converted,
        // which is what the bound is a bound ON.
        let mut old = DesktopConf {
            screens: [("DP-1".to_string(), Choice::Named("cockpit".into()))].into_iter().collect(),
            ..DesktopConf::default()
        };
        assert!(old.migrate_screens(&live));
        assert_eq!(
            old.screens.get("edid:DEL-41B2-0123ABCD"),
            Some(&Choice::Named("cockpit".into()))
        );
        assert!(
            !old.migrate_screens(&live),
            "and having named one, it is never converted again"
        );
    }

    /// What the migration refuses to do. Each of these is a way of
    /// losing a setting, and none of them is worth the tidiness.
    #[test]
    fn the_migration_never_overwrites_and_never_drops() {
        // An entry already naming the monitor is the answer, and the
        // socket entry beside it stays where it is — a file that names
        // a monitor is past migrating, whichever of the two lines the
        // reader's eye falls on first.
        let mut doc = DesktopConf {
            screens: [
                ("DP-1".to_string(), Choice::Named("hangar".into())),
                ("edid:DEL-41B2-0123ABCD".to_string(), Choice::Named("cockpit".into())),
            ]
            .into_iter()
            .collect(),
            ..DesktopConf::default()
        };
        let live =
            [ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-1".into()) }];
        assert!(!doc.migrate_screens(&live), "there is nothing to move");
        assert_eq!(doc.screens.get("edid:DEL-41B2-0123ABCD"), Some(&Choice::Named("cockpit".into())));
        assert_eq!(
            doc.screens.get("DP-1"),
            Some(&Choice::Named("hangar".into())),
            "the socket entry is left where it is: it may be about the socket"
        );

        // A monitor that gives no name of its own has nothing to move
        // to, and its socket entry stays the only thing naming it.
        let mut silent = DesktopConf {
            screens: [("eDP-1".to_string(), Choice::Named("panel".into()))].into_iter().collect(),
            ..DesktopConf::default()
        };
        assert!(!silent.migrate_screens(&[ScreenId::of_connector("eDP-1")]));
        assert_eq!(silent.screens.get("eDP-1"), Some(&Choice::Named("panel".into())));

        // The user typed the connector in another case than the display
        // server says it. It is the same socket and the same setting.
        let mut typed = DesktopConf {
            screens: [("edp-1".to_string(), Choice::Named("panel".into()))].into_iter().collect(),
            ..DesktopConf::default()
        };
        assert!(typed.migrate_screens(&[ScreenId {
            edid: Some("AUO-1234".into()),
            connector: Some("eDP-1".into()),
        }]));
        assert_eq!(typed.screens.get("edid:AUO-1234"), Some(&Choice::Named("panel".into())));
        assert!(typed.screens.is_empty() || !typed.screens.contains_key("edp-1"));
    }

    /// The migration through the door that writes, and the promise it
    /// has to keep on the machines that have nothing to migrate: a
    /// program that installs nothing must go on having installed
    /// nothing after running it.
    #[test]
    fn a_machine_with_nothing_to_migrate_is_not_written_to() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("migrate-screens");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let path = root.join(FAMILY_DIR).join(CONF_RON);

        // A machine nobody has configured, with two monitors on it.
        let live = [
            ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-1".into()) },
            ScreenId::of_connector("eDP-1"),
        ];
        assert!(!migrate_screen_identities(&live), "nothing was written under a socket");
        flush_writes();
        assert!(!path.exists(), "and so no configuration file was made: {}", path.display());

        // Now the user assigns a layaut the old way, by socket.
        set_layaut_for_screen("DP-1", "cockpit");
        assert_eq!(conf().screens().get("DP-1").map(String::as_str), Some("cockpit"));
        set_main_screen(Some("DP-1"));

        assert!(migrate_screen_identities(&live), "that one has somewhere to go");
        flush_writes();
        let after = conf();
        assert_eq!(
            after.screens().get("edid:DEL-41B2-0123ABCD").map(String::as_str),
            Some("cockpit"),
            "the assignment is now the monitor's"
        );
        assert!(!after.screens().contains_key("DP-1"));
        assert_eq!(after.main_screen.name(), Some("edid:DEL-41B2-0123ABCD"));
        let text = std::fs::read_to_string(&path).expect("the file the migration wrote");
        assert!(text.contains("edid:DEL-41B2-0123ABCD"), "and it landed on disk: {text}");

        // A second start has nothing left to do.
        assert!(!migrate_screen_identities(&live), "run twice, moved once");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// THE DOOR ITSELF: a change that turns out not to be one leaves no
    /// trace at all.
    ///
    /// `update_conf_when` is on the way out of every write this program
    /// makes — `update_conf` is one line of it — so what it does when
    /// the change answers "nothing happened" is worth pinning on its
    /// own rather than through whichever caller happens to ask today.
    /// No file, no directory, and no memo either: filing the memo would
    /// hand every later reader a document no disk has ever held.
    #[test]
    fn a_change_that_turns_out_not_to_be_one_leaves_no_trace() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("update-when");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        let path = dir.join(CONF_RON);

        update_conf_when(|c| {
            c.theme = Choice::Named("crimson".into());
            false
        });
        flush_writes();
        assert!(!path.exists(), "no file: {}", path.display());
        assert!(!dir.exists(), "and not even the directory: {}", dir.display());
        assert_eq!(
            conf().theme.name(),
            None,
            "and nothing was remembered either, or the next reader would be \
             answered from a document that was never written"
        );

        // The same door with something to write goes all the way to the
        // disk, which is what says the check above is about the answer
        // and not about the door being shut.
        update_conf_when(|c| {
            c.theme = Choice::Named("crimson".into());
            true
        });
        flush_writes();
        assert_eq!(conf().theme.name(), Some("crimson"));
        let text = std::fs::read_to_string(&path).expect("the file the change wrote");
        assert!(text.contains("crimson"), "{text}");

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// THE PRECHECK, and why it is not merely a saving of syscalls.
    ///
    /// Opening the write door READS the user's file, and a file that
    /// cannot be read is copied aside and reported to them in the words
    /// "the setting you just changed has REPLACED it". A start is not a
    /// setting. So a machine with one bracket missing from its
    /// configuration must be able to start every day without collecting
    /// a rescue copy per start and a notice about a change nobody made.
    #[test]
    fn a_start_with_nothing_to_migrate_does_not_rescue_a_broken_file() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("migrate-broken");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        let dir = root.join(FAMILY_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = take_conf_rescued();

        // Somebody's whole configuration, one bracket short — and not a
        // word in it about any screen.
        let mine = "(\n    theme: Named(\"crimson\",\n    layaut: Named(\"console\"),\n)\n";
        std::fs::write(dir.join(CONF_RON), mine).unwrap();

        let live =
            [ScreenId { edid: Some("DEL-41B2-0123ABCD".into()), connector: Some("DP-1".into()) }];
        assert!(!migrate_screen_identities(&live), "there is nothing to migrate");
        flush_writes();

        assert_eq!(
            rescue_copies(&dir),
            Vec::<String>::new(),
            "a start makes no rescue copy: nothing was being replaced"
        );
        assert_eq!(
            take_conf_rescued(),
            None,
            "and the user is told nothing, there being nothing to tell them"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(CONF_RON)).unwrap(),
            mine,
            "their own text is exactly where they left it"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The MAIN SCREEN role written down and taken back. What it means
    /// is `screens::MainScreenDuty`'s business; this is about the
    /// setting carrying it.
    #[test]
    fn the_main_screen_is_a_setting_and_not_only_the_display_servers_opinion() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("main-screen");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", root.join("etc"));
        set_layaut_option("console");

        assert_eq!(main_screen_key(), None, "with nothing said, the display server answers");

        set_main_screen(Some("edid:DEL-41B2-0123ABCD"));
        assert_eq!(main_screen_key().as_deref(), Some("edid:DEL-41B2-0123ABCD"));
        assert_eq!(
            conf().layaut.name(),
            Some("console"),
            "the role is its own field and touches nothing else"
        );

        // A socket names a screen too — a monitor that gives no name of
        // its own can still be the main one.
        set_main_screen(Some("HDMI-A-1"));
        assert_eq!(main_screen_key().as_deref(), Some("HDMI-A-1"));

        // Taking it back is an explicit "whatever the display server
        // says", which has to beat a system file naming a screen that
        // is not on this desk — so it is written, not dropped.
        set_main_screen(None);
        assert_eq!(conf().main_screen, Choice::Off, "an off, not an absence");
        assert_eq!(main_screen_key(), None);

        // A key nothing could match a screen to never reaches the file.
        for bad in ["", "Dell Inc. U2720Q", "edid:", "screen 2"] {
            set_main_screen(Some(bad));
            assert_eq!(conf().main_screen, Choice::Off, "'{bad}' must not become the main screen");
        }

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// THE THIRD ANSWER, which is the one a setter offering a name and
    /// an off cannot write: the question handed back to the cascade.
    ///
    /// An explicit "whatever the display server says" and "nothing was
    /// said here" look the same on this machine and are opposite
    /// answers on a machine whose administrator named a screen — the
    /// first outranks that file, the second lets it through. So taking
    /// the setting back has to REMOVE the field, and there has to be
    /// something that does.
    #[test]
    fn the_main_screen_setting_can_be_taken_out_of_the_file_altogether() {
        fixture_registry();
        let _env = env_lock();
        let root = scratch("main-screen-clear");
        let etc = root.join("etc");
        std::fs::create_dir_all(etc.join(FAMILY_DIR)).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::set_var("XDG_CONFIG_DIRS", &etc);
        // The machine's own file names a screen this desk may not have.
        std::fs::write(
            etc.join(FAMILY_DIR).join(CONF_RON),
            "(main_screen: Named(\"edid:GSM-5B0F\"))\n",
        )
        .unwrap();

        assert_eq!(main_screen_key().as_deref(), Some("edid:GSM-5B0F"), "the system file answers");

        set_main_screen(Some("edid:DEL-41B2-0123ABCD"));
        assert_eq!(main_screen_key().as_deref(), Some("edid:DEL-41B2-0123ABCD"));

        set_main_screen(None);
        assert_eq!(conf().main_screen, Choice::Off);
        assert_eq!(main_screen_key(), None, "an off silences the system file too");

        clear_main_screen();
        flush_writes();
        assert_eq!(
            main_screen_key().as_deref(),
            Some("edid:GSM-5B0F"),
            "the rest of the cascade is heard again, which an off could not have allowed"
        );
        let text = std::fs::read_to_string(root.join(FAMILY_DIR).join(CONF_RON)).unwrap();
        assert!(
            !text.contains("main_screen"),
            "and the field is GONE from the user's file rather than emptied: {text}"
        );

        // The role is its own setting: clearing the assignments leaves
        // it alone, and clearing it leaves them alone.
        set_layaut_for_screen("edid:DEL-41B2-0123ABCD", "cockpit");
        set_main_screen(Some("edid:DEL-41B2-0123ABCD"));
        clear_screen_layauts();
        flush_writes();
        assert!(conf().screens().is_empty(), "the assignments are gone");
        assert_eq!(
            main_screen_key().as_deref(),
            Some("edid:DEL-41B2-0123ABCD"),
            "and the role is not an assignment"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_DIRS");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A configuration cascade with nothing in it, so `GridPadding=` is
    /// genuinely absent — the state a fresh install is in, and the only
    /// state in which the theme's answer is the one that shows.
    fn empty_conf(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("nacelle-gutter-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the fixture tree must be writable");
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_CONFIG_DIRS", &dir);
        dir
    }

    /// The band around every panel on every board came out of the code as
    /// a flat eight device pixels, at every resolution and every density,
    /// which no theme could reach. It is a length now, and this is what
    /// says so: the same program, two themes, two gutters.
    #[test]
    fn the_theme_sets_the_band_around_every_panel() {
        let _theme = crate::widgets::theme_test_lock();
        let _env = env_lock();
        let dir = empty_conf("theme");
        assert!(
            grid_padding_override().is_none(),
            "the fixture must carry no GridPadding, or this proves nothing"
        );

        let shipped = panel_gutter(None);
        assert_eq!(
            shipped,
            crate::widgets::token_px("layout.panel_gutter"),
            "the gutter must BE the token, not a number that happens to match it"
        );
        // Far from the shipped 1.5u, and not a round eight either, so
        // neither the old literal nor a rounding could produce it.
        let _wide = crate::widgets::Themed::new(
            "gutter",
            "[layout]\npanel_gutter = 9u\n",
        );
        let wide = panel_gutter(None);
        assert!(
            wide > shipped * 3.0,
            "a theme asking for 9u must widen the gutter: {shipped} -> {wide}"
        );

        // And the content boxes the widgets are actually drawn in follow
        // it: `padded` is the call every board makes on every frame, and
        // this is the whole distance between a token and a pixel.
        let id = nacelle::layout::InstanceId::new(1);
        let mut board = nacelle::base::Layout::empty(400.0, 300.0);
        board.place(
            id,
            nacelle::base::Panel(0),
            nacelle::base::Rect::new(0.0, 0.0, 400.0, 300.0),
        );
        let shipped_box = board.padded(shipped).of(id);
        let wide_box = board.padded(wide).of(id);
        assert!(
            wide_box.w < shipped_box.w && wide_box.h < shipped_box.h,
            "the content box must shrink when the theme widens the gutter: \
             {shipped_box:?} -> {wide_box:?}"
        );
        assert!(wide_box.x > shipped_box.x, "and move inwards on both axes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `GridPadding=` is the user's stage-5 override of that one token,
    /// the arrangement `Density=` already has with `metric.density`: the
    /// theme is what the interface looks like until the user has said
    /// otherwise, and after that the user wins.
    #[test]
    fn the_users_grid_padding_still_overrides_the_theme() {
        let _theme = crate::widgets::theme_test_lock();
        let _env = env_lock();
        let dir = empty_conf("user");
        set_grid_padding(31);
        assert_eq!(grid_padding_override(), Some(31));
        assert_eq!(panel_gutter(grid_padding_override()), 31.0);
        // Even under a theme that wants something else entirely.
        let _wide =
            crate::widgets::Themed::new("gutter-user", "[layout]\npanel_gutter = 9u\n");
        assert_eq!(panel_gutter(grid_padding_override()), 31.0);
        // And a number no spinner could have produced is still bounded.
        set_grid_padding(model::GRID_PAD_MAX + 1000);
        assert_eq!(grid_padding_override(), Some(model::GRID_PAD_MAX));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two monitors of two heights are two gutters — and the engine's
    /// epoch names neither of them.
    ///
    /// Every screen sets the viewport as it draws, so on a mixed-height
    /// desktop the published bake alternates between the two. Because a
    /// bake is cached under `(sibling, u)` and carries the epoch it was
    /// born with, the epoch alternates too: it does not run forward, it
    /// flips. Anything that re-read a u-derived length whenever the epoch
    /// moved would therefore fire on EVERY frame, and would hand both
    /// screens whichever height drew last — the neighbour's. That is why
    /// the gutter is taken per screen, under the viewport that screen has
    /// just set, and why it is the one thing the lens guard leaves alone.
    #[test]
    fn two_screen_heights_are_two_gutters_and_the_epoch_names_neither() {
        let _theme = crate::widgets::theme_test_lock();
        let _env = env_lock();
        let dir = empty_conf("viewport");
        // Far from the shipped gutter, so the two heights cannot round
        // together into one number.
        let _wide =
            crate::widgets::Themed::new("gutter-vp", "[layout]\npanel_gutter = 9u\n");

        nacelle::theme::set_viewport(1080.0, 1.0);
        let short = panel_gutter(None);
        let short_epoch = nacelle::theme::epoch();

        nacelle::theme::set_viewport(2160.0, 1.0);
        let tall = panel_gutter(None);
        let tall_epoch = nacelle::theme::epoch();

        assert!(
            tall > short,
            "a taller screen must get a wider gutter, or the gutter is not a u: \
             {short} -> {tall}"
        );
        assert_ne!(
            short_epoch, tall_epoch,
            "the two heights must publish two bakes, or there is nothing to guard"
        );

        // The turn of the screw: back to the first height, the epoch does
        // not advance past the second — it returns to the value the first
        // screen had. A guard comparing against a remembered epoch never
        // reaches a resting state on this desktop.
        nacelle::theme::set_viewport(1080.0, 1.0);
        assert_eq!(
            panel_gutter(None),
            short,
            "the same height must answer the same gutter"
        );
        assert_eq!(
            nacelle::theme::epoch(),
            short_epoch,
            "the epoch must FLIP BACK, not advance: this is exactly why it cannot \
             be used to decide when a per-screen length is stale"
        );

        // The user's override is not a u and is the same on both screens,
        // which is what lets it be carried on the frame instead.
        assert_eq!(panel_gutter(Some(31)), 31.0);
        nacelle::theme::set_viewport(2160.0, 1.0);
        assert_eq!(panel_gutter(Some(31)), 31.0);

        nacelle::theme::set_viewport(1080.0, 1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `grid_prefs` answers the gutter ROUNDED, because whole pixels are
    /// what the settings spinner edits. The board is solved with the
    /// unrounded length. So the two readings are not interchangeable, and
    /// anything drawn OVER the board — the grid editor — has to be handed
    /// the board's own or its cells sit off the panels they describe.
    ///
    /// This is the reading `apply_config!` used to take. It is kept here
    /// as a live measurement rather than a remark, so that the day the
    /// gutter is made whole-pixel by construction, this test says so
    /// instead of quietly passing.
    #[test]
    fn the_spinners_gutter_is_rounded_and_the_boards_is_not() {
        let _theme = crate::widgets::theme_test_lock();
        let _env = env_lock();
        let dir = empty_conf("rounding");
        assert!(
            grid_padding_override().is_none(),
            "the fixture must carry no GridPadding, or the theme is never asked"
        );
        let _wide =
            crate::widgets::Themed::new("gutter-round", "[layout]\npanel_gutter = 9u\n");

        // Some height at which the theme's gutter is not a whole number of
        // device pixels. A u is a fraction of the window, so most heights
        // are such a height.
        let split = (600..=2400).step_by(3).find_map(|h| {
            nacelle::theme::set_viewport(h as f32, 1.0);
            let exact = panel_gutter(None);
            (exact.fract() > 0.01 && exact.fract() < 0.99).then_some((h, exact))
        });
        let Some((h, exact)) = split else {
            panic!("no height in 600..2400 gave a fractional gutter — if the gutter \
                    is now whole by construction, the editor no longer needs its own \
                    reading and this test should be retired deliberately");
        };

        let spinner = grid_prefs().3;
        assert_eq!(
            spinner,
            exact.round() as u32,
            "the spinner must show the gutter rounded, not some other number"
        );
        assert_ne!(
            spinner as f32, exact,
            "at {h} lines the two readings differ ({exact} vs {spinner}); the editor \
             must take the board's, which is what `sc.pad` carries"
        );

        nacelle::theme::set_viewport(1080.0, 1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

