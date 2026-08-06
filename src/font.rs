//! Ładowanie fontów i atlas glifów (jednokanałowy, R8).
//!
//! eDEX-UI używa "United Sans" (UI) oraz "Fira Mono" (terminal). Pliki .woff2
//! z repozytorium eDEX można przekonwertować na .ttf (patrz README) i wrzucić
//! do katalogu ./fonts — zostaną wykryte automatycznie. W przeciwnym razie
//! szukamy podobnych fontów systemowych.

use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ATLAS_W: usize = 1024;
pub const ATLAS_H: usize = 1024;

pub const FONT_UI: u8 = 0;
pub const FONT_MONO: u8 = 1;

#[derive(Clone, Copy)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: f32,
    pub h: f32,
    /// Przesunięcie lewej krawędzi bitmapy względem pióra.
    pub xmin: f32,
    /// Przesunięcie dolnej krawędzi bitmapy względem linii bazowej (oś Y w górę).
    pub ymin: f32,
    pub advance: f32,
}

pub struct FontSystem {
    fonts: [Font; 2],
    pub atlas: Vec<u8>,
    pub atlas_dirty: bool,
    cache: HashMap<(u8, u32, char), Option<Glyph>>,
    // prosty pakowacz półkowy
    cur_x: usize,
    cur_y: usize,
    row_h: usize,
}

impl FontSystem {
    pub fn new() -> Self {
        let (ui, mono) = load_fonts();
        let mut fs = FontSystem {
            fonts: [ui, mono],
            atlas: vec![0u8; ATLAS_W * ATLAS_H],
            atlas_dirty: true,
            cache: HashMap::new(),
            cur_x: 2,
            cur_y: 2,
            row_h: 0,
        };
        // Biały piksel (0,0..2x2) dla jednolitych wypełnień.
        for y in 0..2 {
            for x in 0..2 {
                fs.atlas[y * ATLAS_W + x] = 255;
            }
        }
        fs
    }

    /// UV białego piksela — używane przez figury jednolite.
    pub fn white_uv() -> (f32, f32) {
        (0.5 / ATLAS_W as f32, 0.5 / ATLAS_H as f32)
    }

    /// Czyści atlas i cache (np. gdy się zapełni po wielu zmianach rozmiaru).
    fn reset_atlas(&mut self) {
        self.atlas.iter_mut().for_each(|p| *p = 0);
        for y in 0..2 {
            for x in 0..2 {
                self.atlas[y * ATLAS_W + x] = 255;
            }
        }
        self.cache.clear();
        self.cur_x = 2;
        self.cur_y = 2;
        self.row_h = 0;
        self.atlas_dirty = true;
    }

    pub fn glyph(&mut self, font: u8, px: f32, ch: char) -> Option<Glyph> {
        let key = (font, (px * 4.0).round() as u32, ch);
        if let Some(g) = self.cache.get(&key) {
            return *g;
        }
        let f = &self.fonts[font as usize];
        let (metrics, bitmap) = f.rasterize(ch, px);
        if metrics.width == 0 || metrics.height == 0 {
            let g = Some(Glyph {
                u0: 0.0,
                v0: 0.0,
                u1: 0.0,
                v1: 0.0,
                w: 0.0,
                h: 0.0,
                xmin: 0.0,
                ymin: 0.0,
                advance: metrics.advance_width,
            });
            self.cache.insert(key, g);
            return g;
        }
        let (w, h) = (metrics.width, metrics.height);
        if self.cur_x + w + 2 > ATLAS_W {
            self.cur_x = 2;
            self.cur_y += self.row_h + 2;
            self.row_h = 0;
        }
        if self.cur_y + h + 2 > ATLAS_H {
            // Atlas pełny — zaczynamy od nowa (rzadkie, po wielu resize'ach).
            self.reset_atlas();
            if self.cur_y + h + 2 > ATLAS_H {
                return None;
            }
        }
        let (ax, ay) = (self.cur_x, self.cur_y);
        for row in 0..h {
            let dst = (ay + row) * ATLAS_W + ax;
            self.atlas[dst..dst + w].copy_from_slice(&bitmap[row * w..row * w + w]);
        }
        self.cur_x += w + 2;
        self.row_h = self.row_h.max(h);
        self.atlas_dirty = true;

        let g = Some(Glyph {
            u0: ax as f32 / ATLAS_W as f32,
            v0: ay as f32 / ATLAS_H as f32,
            u1: (ax + w) as f32 / ATLAS_W as f32,
            v1: (ay + h) as f32 / ATLAS_H as f32,
            w: w as f32,
            h: h as f32,
            xmin: metrics.xmin as f32,
            ymin: metrics.ymin as f32,
            advance: metrics.advance_width,
        });
        self.cache.insert(key, g);
        g
    }

    /// Metryki linii: (ascent, wysokość linii).
    pub fn line_metrics(&self, font: u8, px: f32) -> (f32, f32) {
        if let Some(m) = self.fonts[font as usize].horizontal_line_metrics(px) {
            (m.ascent, m.ascent - m.descent + m.line_gap)
        } else {
            (px * 0.8, px * 1.2)
        }
    }

    /// Szerokość komórki dla fontu monospace.
    pub fn mono_advance(&mut self, px: f32) -> f32 {
        self.glyph(FONT_MONO, px, 'M').map(|g| g.advance).unwrap_or(px * 0.6)
    }

    pub fn measure(&mut self, font: u8, px: f32, text: &str, letter_spacing: f32) -> f32 {
        let mut w = 0.0;
        for ch in text.chars() {
            if let Some(g) = self.glyph(font, px, ch) {
                w += g.advance + letter_spacing;
            }
        }
        w
    }
}

fn try_load(path: &Path) -> Option<Font> {
    let data = std::fs::read(path).ok()?;
    Font::from_bytes(data, FontSettings::default()).ok()
}

/// Rekurencyjne szukanie pliku fontu, którego nazwa (bez rozróżniania
/// wielkości liter, bez separatorów) zawiera jeden z wzorców.
fn find_font(dirs: &[PathBuf], patterns: &[&str]) -> Option<PathBuf> {
    fn walk(dir: &Path, patterns: &[&str], depth: u32, out: &mut Option<PathBuf>) {
        if depth > 4 || out.is_some() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            if out.is_some() {
                return;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(&p, patterns, depth + 1, out);
            } else {
                let name: String = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect();
                if !(name.ends_with("ttf") || name.ends_with("otf")) {
                    continue;
                }
                // Preferuj odmiany Regular/Medium, unikaj Italic/Bold.
                if name.contains("italic") || name.contains("oblique") || name.contains("bold") {
                    continue;
                }
                for pat in patterns {
                    if name.contains(pat) {
                        *out = Some(p.clone());
                        break;
                    }
                }
            }
        }
    }
    for &pat in patterns {
        let mut found = None;
        for d in dirs {
            walk(d, &[pat], 0, &mut found);
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("fonts")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!("{home}/.local/share/fonts")));
        dirs.push(PathBuf::from(format!("{home}/.fonts")));
    }
    dirs.push(PathBuf::from("/usr/share/fonts"));
    dirs.push(PathBuf::from("/usr/local/share/fonts"));
    dirs
}

fn load_fonts() -> (Font, Font) {
    let dirs = font_dirs();

    // Font terminala: Fira Mono jak w eDEX, potem sensowne zamienniki.
    let mono_path = std::env::var("NGTERM_FONT_MONO").ok().map(PathBuf::from).or_else(|| {
        find_font(
            &dirs,
            &[
                "firamonoregular", "firamono", "firacoderegular", "firacode",
                "jetbrainsmonoregular", "jetbrainsmono", "dejavusansmono",
                "liberationmonoregular", "liberationmono", "notosansmono",
            ],
        )
    });

    // Font UI: United Sans jak w eDEX, potem podobne "techniczne" kroje.
    let ui_path = std::env::var("NGTERM_FONT_UI").ok().map(PathBuf::from).or_else(|| {
        find_font(
            &dirs,
            &[
                "unitedsansmedium", "unitedsans", "oxanium", "rajdhani",
                "exo2", "orbitron", "sairacondensed", "saira",
            ],
        )
    });

    let mono = mono_path
        .as_deref()
        .and_then(try_load)
        .unwrap_or_else(|| panic!(
            "ng-term: nie znaleziono żadnego fontu monospace (.ttf/.otf).\n\
             Wskaż go zmienną NGTERM_FONT_MONO lub wrzuć do katalogu ./fonts"
        ));
    let ui = ui_path.as_deref().and_then(try_load).unwrap_or_else(|| {
        eprintln!("ng-term: brak fontu UI (United Sans) — używam fontu monospace");
        mono.clone()
    });
    (ui, mono)
}
