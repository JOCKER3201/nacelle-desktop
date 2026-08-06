//! Konfiguracja użytkownika: ~/.config/ng-term
//!
//! Struktura:
//!   ~/.config/ng-term/ng-term.conf     — główny plik konfiguracyjny (Klucz=Wartość)
//!   ~/.config/ng-term/themes/<motyw>/  — katalogi motywów, każdy zawiera:
//!       meta        — metaplik z polem Name= (nazwa używana w ng-term.conf)
//!       *.css       — styl (kolory)
//!       *.layaut    — układ paneli (jednostki vw/vh)
//!
//! W ng-term.conf opcja Themes=<nazwa> wybiera motyw po polu Name= z metapliku.
//! Pusta wartość lub brak opcji = motyw domyślny zaszyty w kodzie.

use crate::theme::{Color, Theme};
use crate::widgets::{LayoutSpec, PanelSpec};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Config {
    pub theme: Theme,
    pub layout: LayoutSpec,
}

/// Motyw znaleziony w ~/.config/ng-term/themes (nazwa z metapliku).
#[derive(Clone)]
pub struct ThemeInfo {
    pub name: String,
    #[allow(dead_code)]
    pub dir: PathBuf,
}

/// Skanuje katalog motywów; zwraca wpisy z poprawnym metaplikiem (Name=).
pub fn list_themes() -> Vec<ThemeInfo> {
    let dir = config_dir().join("themes");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            if let Some(meta) = read_meta(&p) {
                if let Some(name) = parse_kv(&meta).get("Name") {
                    if !name.is_empty() {
                        out.push(ThemeInfo { name: name.clone(), dir: p });
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn load() -> Config {
    let dir = config_dir();
    init_tree(&dir);

    let conf_text = std::fs::read_to_string(dir.join("ng-term.conf")).unwrap_or_default();
    let kv = parse_kv(&conf_text);
    let theme_name = kv
        .get("Themes")
        .or_else(|| kv.get("Theme"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if !theme_name.is_empty() {
        match load_theme(&dir.join("themes"), &theme_name) {
            Some(cfg) => return cfg,
            None => eprintln!(
                "ng-term: motyw '{theme_name}' nie znaleziony w {} — używam domyślnego",
                dir.join("themes").display()
            ),
        }
    }
    // Motyw domyślny (hardkodowany; Theme::load honoruje jeszcze NGTERM_THEME).
    Config {
        theme: Theme::load(),
        layout: LayoutSpec::default(),
    }
}

/// Ścieżka pliku startowego basha generowanego przez ng-term.
pub fn shellrc_path() -> PathBuf {
    config_dir().join("shellrc")
}

/// Wczytuje motyw o podanej nazwie (pole Name= metapliku).
pub fn load_theme_by_name(name: &str) -> Option<Config> {
    load_theme(&config_dir().join("themes"), name)
}

/// Bieżąca wartość Themes= z ng-term.conf (jeśli niepusta).
pub fn current_theme_name() -> Option<String> {
    let text = std::fs::read_to_string(config_dir().join("ng-term.conf")).ok()?;
    let kv = parse_kv(&text);
    kv.get("Themes")
        .or_else(|| kv.get("Theme"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Zapisuje wybór motywu do ng-term.conf, zachowując resztę pliku.
pub fn set_theme_option(name: &str) {
    set_conf_kv("Themes", name);
}

/// Ustawia Klucz=Wartość w ng-term.conf, zachowując resztę pliku.
fn set_conf_kv(key: &str, value: &str) {
    let path = config_dir().join("ng-term.conf");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut replaced = false;
    let alt = format!("{}=", key.trim_end_matches('s'));
    for line in lines.iter_mut() {
        let t = line.trim_start();
        if t.starts_with(&format!("{key}=")) || (key == "Themes" && t.starts_with(&alt)) {
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
        eprintln!("ng-term: nie można zapisać {}: {e}", path.display());
    }
}

fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("ng-term");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("ng-term")
}

/// Tworzy katalog konfiguracji, plik ng-term.conf, katalog themes
/// oraz (przy pierwszym uruchomieniu) przykładowy motyw "tron".
fn init_tree(dir: &Path) {
    let themes = dir.join("themes");
    let themes_existed = themes.is_dir();
    if let Err(e) = std::fs::create_dir_all(&themes) {
        eprintln!("ng-term: nie można utworzyć {}: {e}", themes.display());
        return;
    }

    let conf = dir.join("ng-term.conf");
    if !conf.exists() {
        let _ = std::fs::write(
            &conf,
            "# Konfiguracja ng-term\n\
             #\n\
             # Themes=<nazwa>  — wybiera motyw po polu Name= z metapliku\n\
             #                   ~/.config/ng-term/themes/<katalog>/meta\n\
             # Pusta wartość lub brak opcji = motyw domyślny wbudowany w program.\n\
             Themes=\n",
        );
    }

    // Plik startowy powłoki: source ~/.bashrc + otwieranie plików
    // skojarzoną aplikacją po wpisaniu samej nazwy pliku.
    let rc = dir.join("shellrc");
    if !rc.exists() {
        let _ = std::fs::write(
            &rc,
            "# ng-term: plik startowy basha (generowany raz; mozna edytowac)\n\
             [ -f \"$HOME/.bashrc\" ] && source \"$HOME/.bashrc\"\n\
             \n\
             # Wpisanie nazwy istniejacego pliku otwiera go aplikacja\n\
             # skojarzona z rozszerzeniem (xdg-open).\n\
             command_not_found_handle() {\n\
             \x20   if [ -e \"$1\" ] && [ ! -d \"$1\" ]; then\n\
             \x20       (xdg-open \"$1\" >/dev/null 2>&1 &)\n\
             \x20       return 0\n\
             \x20   fi\n\
             \x20   printf 'bash: %s: command not found\\n' \"$1\" >&2\n\
             \x20   return 127\n\
             }\n",
        );
    }

    // Przykładowy motyw tworzymy tylko przy pierwszym utworzeniu themes/.
    if !themes_existed {
        let tron = themes.join("tron");
        if std::fs::create_dir_all(&tron).is_ok() {
            let _ = std::fs::write(
                tron.join("meta"),
                "Name=tron\nDescription=Domyslny motyw ng-term (eDEX-UI tron)\n",
            );
            let _ = std::fs::write(
                tron.join("tron.css"),
                "/* Styl motywu ng-term — kolory w formacie #rrggbb */\n\
                 :root {\n\
                 \x20   --color-r: 170;\n\
                 \x20   --color-g: 207;\n\
                 \x20   --color-b: 209;\n\
                 \x20   --background: #05080d;\n\
                 \x20   --grey: #262828;\n\
                 }\n\
                 terminal {\n\
                 \x20   foreground: #aacfd1;\n\
                 \x20   background: #05080d;\n\
                 \x20   cursor: #aacfd1;\n\
                 }\n",
            );
            let _ = std::fs::write(
                tron.join("tron.layaut"),
                "# Uklad paneli ng-term: panel = x y szerokosc wysokosc\n\
                 # Jednostki: vw (procent szerokosci okna), vh (procent wysokosci).\n\
                 left_col   = 0.6vw  2.5vh  16.4vw 59.5vh\n\
                 shell      = 17.5vw 2.5vh  65.0vw 60.3vh\n\
                 right_col  = 83.0vw 2.5vh  16.4vw 59.5vh\n\
                 filesystem = 83.0vw 17.4vh 16.4vw 79.6vh\n\
                 keyboard   = 17.5vw 64.5vh 65.0vw 32.5vh\n\
                 control    = 0.6vw  64.5vh 16.4vw 32.5vh\n",
            );
        }
    }
}

/// Parser plików Klucz=Wartość (komentarze # i ;).
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

/// Szuka w themes/ katalogu, którego metaplik ma Name=<name>,
/// i wczytuje z niego styl (.css) oraz układ (.layaut).
fn load_theme(themes_dir: &Path, name: &str) -> Option<Config> {
    let rd = std::fs::read_dir(themes_dir).ok()?;
    for entry in rd.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(meta_text) = read_meta(&dir) else { continue };
        let meta = parse_kv(&meta_text);
        if meta.get("Name").map(|n| n.as_str()) != Some(name) {
            continue;
        }
        // Znaleziony motyw: styl + układ (braki uzupełniamy domyślnymi).
        let theme = find_file(&dir, "css")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|css| parse_css(&css))
            .unwrap_or_else(Theme::tron);
        let layout = find_file(&dir, "layaut")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|l| parse_layaut(&l))
            .unwrap_or_default();
        return Some(Config { theme, layout });
    }
    None
}

/// Metaplik: plik o nazwie "meta" albo z rozszerzeniem ".meta".
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

/// Uproszczony parser CSS: bloki `selektor { klucz: wartość; }`.
fn parse_css(src: &str) -> Theme {
    let src = strip_css_comments(src);
    let mut blocks: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut rest = src.as_str();
    while let Some(ob) = rest.find('{') {
        let sel = rest[..ob].trim().to_lowercase();
        let Some(cb_rel) = rest[ob + 1..].find('}') else { break };
        let body = &rest[ob + 1..ob + 1 + cb_rel];
        let mut props = HashMap::new();
        for decl in body.split(';') {
            if let Some((k, v)) = decl.split_once(':') {
                props.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        blocks.insert(sel, props);
        rest = &rest[ob + 1 + cb_rel + 1..];
    }

    let mut theme = Theme::tron();
    if let Some(root) = blocks.get(":root").or_else(|| blocks.get("colors")) {
        let num = |key: &str| -> Option<u8> { root.get(key)?.parse().ok() };
        if let (Some(r), Some(g), Some(b)) =
            (num("--color-r"), num("--color-g"), num("--color-b"))
        {
            let base = Color::rgb8(r, g, b);
            theme.base = base;
            theme.term_fg = base;
            theme.cursor = base;
        }
        if let Some(c) = root
            .get("--background")
            .or_else(|| root.get("--light-black"))
            .and_then(|v| Color::from_hex(v))
        {
            theme.bg = c;
            theme.term_bg = c;
        }
        if let Some(c) = root.get("--grey").and_then(|v| Color::from_hex(v)) {
            theme.grey = c;
        }
    }
    if let Some(term) = blocks.get("terminal") {
        if let Some(c) = term.get("foreground").and_then(|v| Color::from_hex(v)) {
            theme.term_fg = c;
        }
        if let Some(c) = term.get("background").and_then(|v| Color::from_hex(v)) {
            theme.term_bg = c;
        }
        if let Some(c) = term.get("cursor").and_then(|v| Color::from_hex(v)) {
            theme.cursor = c;
        }
    }
    // Opcjonalna pełna paleta ANSI: blok `palette { color0..color15 }`.
    if let Some(pal) = blocks.get("palette") {
        for i in 0..16 {
            if let Some(c) = pal
                .get(&format!("color{i}"))
                .and_then(|v| Color::from_hex(v))
            {
                theme.ansi[i] = c;
            }
        }
    }
    theme
}

fn strip_css_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start..].find("*/") {
            Some(end) => rest = &rest[start + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Parser plików .layaut: `panel = x y szerokość wysokość` (jednostki vw/vh).
fn parse_layaut(src: &str) -> LayoutSpec {
    let mut spec = LayoutSpec::default();
    for line in src.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let nums: Vec<f32> = v
            .split_whitespace()
            .filter_map(|t| {
                t.trim_end_matches("vw")
                    .trim_end_matches("vh")
                    .parse::<f32>()
                    .ok()
            })
            .collect();
        if nums.len() != 4 {
            continue;
        }
        let p = PanelSpec {
            x: nums[0],
            y: nums[1],
            w: nums[2],
            h: nums[3],
        };
        match k.trim() {
            "left_col" => spec.left_col = p,
            "shell" => spec.shell = p,
            "right_col" => spec.right_col = p,
            "filesystem" => spec.filesystem = p,
            "keyboard" => spec.keyboard = p,
            "control" => spec.control = p,
            other => eprintln!("ng-term: nieznany panel w .layaut: {other}"),
        }
    }
    spec
}
