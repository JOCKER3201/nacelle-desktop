//! Panel FILESYSTEM — siatka ikon jak w eDEX-UI, śledzi katalog roboczy
//! powłoki (z /proc/<pid>/cwd); klik w katalog nawiguje ręcznie,
//! klik w ścieżkę w nagłówku wraca do śledzenia terminala.

use super::{Ctx, Rect};
use crate::font::FONT_UI;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub is_link: bool,
    pub size: u64,
}

/// Zdarzenie kliknięcia w panelu plików.
pub enum FsEvent {
    /// Wejście do katalogu — aktywna karta terminala ma zrobić `cd`.
    OpenDir(PathBuf),
    /// Otwarcie pliku aplikacją skojarzoną w systemie (xdg-open).
    OpenFile(PathBuf),
}

pub struct Filesystem {
    pub cwd: PathBuf,
    entries: Vec<Entry>,
    pub scroll: f32,
    /// Prostokąty kafelków z ostatniej klatki.
    hits: Vec<(Rect, usize)>,
    last_refresh: Instant,
    error: Option<String>,
}

impl Filesystem {
    pub fn new(start: PathBuf) -> Self {
        let mut fs = Filesystem {
            cwd: start,
            entries: Vec::new(),
            scroll: 0.0,
            hits: Vec::new(),
            last_refresh: Instant::now() - std::time::Duration::from_secs(60),
            error: None,
        };
        fs.refresh();
        fs
    }

    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        self.entries.clear();
        self.error = None;
        if self.cwd.parent().is_some() {
            self.entries.push(Entry {
                name: "..".into(),
                is_dir: true,
                is_link: false,
                size: 0,
            });
        }
        match std::fs::read_dir(&self.cwd) {
            Ok(rd) => {
                let mut list: Vec<Entry> = rd
                    .flatten()
                    .filter_map(|e| {
                        let ft = e.file_type().ok()?;
                        let is_link = ft.is_symlink();
                        // Podążamy za dowiązaniami (symbolicznymi i innymi):
                        // o traktowaniu jako katalog decyduje typ celu.
                        let target = std::fs::metadata(e.path()).ok();
                        let is_dir =
                            target.as_ref().map(|m| m.is_dir()).unwrap_or(ft.is_dir());
                        Some(Entry {
                            name: e.file_name().to_string_lossy().into_owned(),
                            is_dir,
                            is_link,
                            size: target.map(|m| m.len()).unwrap_or(0),
                        })
                    })
                    .collect();
                list.sort_by(|a, b| {
                    b.is_dir
                        .cmp(&a.is_dir)
                        .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                self.entries.extend(list);
            }
            Err(e) => self.error = Some(format!("I/O ERROR: {e}")),
        }
    }

    /// Podążanie za katalogiem powłoki.
    pub fn follow(&mut self, shell_cwd: Option<PathBuf>) {
        if let Some(cwd) = shell_cwd {
            if cwd != self.cwd {
                self.cwd = cwd;
                self.scroll = 0.0;
                self.refresh();
            }
        }
        if self.last_refresh.elapsed().as_secs() >= 2 {
            self.refresh();
        }
    }

    pub fn wheel(&mut self, delta: f32) {
        self.scroll = (self.scroll - delta).max(0.0);
    }

    /// Klik; zwraca zdarzenie do obsłużenia przez główną pętlę.
    pub fn click(&mut self, x: f32, y: f32) -> Option<FsEvent> {
        let idx = self
            .hits
            .iter()
            .find(|(r, _)| r.contains(x, y))
            .map(|&(_, i)| i)?;
        let entry = self.entries.get(idx)?.clone();
        if entry.is_dir {
            let target = if entry.name == ".." {
                self.cwd.parent()?.to_path_buf()
            } else {
                self.cwd.join(&entry.name)
            };
            self.scroll = 0.0;
            Some(FsEvent::OpenDir(target))
        } else {
            Some(FsEvent::OpenFile(self.cwd.join(&entry.name)))
        }
    }

    pub fn draw(&mut self, ctx: &mut Ctx, r: Rect) {
        self.hits.clear();
        let base = ctx.theme.base;
        let title_px = ctx.font_px(1.02);
        // Ścieżka przycinana od lewej, żeby zmieściła się w wąskim panelu.
        let left_w = ctx.fonts.measure(FONT_UI, title_px, "FILESYSTEM", title_px * 0.06);
        let avail = (r.w - left_w - title_px * 3.0).max(title_px * 4.0);
        let full = format!("{}", self.cwd.display());
        let path_text = fit_tail(ctx, title_px, &full, avail);
        ctx.dl.module_title(
            ctx.fonts,
            r.x,
            r.y,
            r.w,
            title_px,
            "FILESYSTEM",
            &path_text,
            base,
        );

        if let Some(err) = &self.error {
            let err = err.clone();
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                ctx.font_px(1.4),
                r.cx(),
                r.y + r.h * 0.4,
                &err,
                base,
                1.0,
            );
            return;
        }

        // Stała siatka 3 kolumn; kolejne wiersze dostępne przez scroll.
        // Dół obszaru pokrywa się z dolną krawędzią panelu (i klawiatury).
        let area = Rect::new(r.x, r.y + title_px * 2.8, r.w, r.h - title_px * 2.8);
        let gap = ctx.vh(1.0);
        let cols = 3usize;
        let tile = ((area.w - gap * (cols as f32 - 1.0)) / cols as f32)
            .min((area.h - 2.0 * gap) / 3.0)
            .max(20.0);
        let name_px = ctx.font_px(0.85);

        // Przewijanie skokowe o pełne wiersze — rysujemy tylko wiersze
        // mieszczące się w całości, nic nie wystaje poza panel.
        let row_h = tile + gap;
        let total_rows = self.entries.len().div_ceil(cols);
        let nvis = (((area.h + gap) / row_h).floor() as usize).max(1);
        let max_off = total_rows.saturating_sub(nvis);
        self.scroll = self.scroll.clamp(0.0, max_off as f32 * row_h);
        let row_off = ((self.scroll / row_h).round() as usize).min(max_off);
        // Gdy lista wymaga przewijania, rozciągamy odstępy tak, żeby ostatni
        // widoczny wiersz kończył się dokładnie na dolnej krawędzi panelu
        // (na równi z klawiaturą).
        let step = if total_rows > nvis && nvis > 1 {
            (area.h - tile) / (nvis as f32 - 1.0)
        } else {
            row_h
        };

        for (i, entry) in self.entries.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            if row < row_off || row >= row_off + nvis {
                continue;
            }
            let x = area.x + col as f32 * (tile + gap);
            let y = area.y + (row - row_off) as f32 * step;
            let trect = Rect::new(x, y, tile, tile);
            let hover = trect.contains(ctx.mouse.0, ctx.mouse.1);
            if hover {
                ctx.dl.rect(x, y, tile, tile, base.alpha(0.1));
            }

            // Ikona rysowana wektorowo.
            let icon = Rect::new(
                x + tile * 0.22,
                y + tile * 0.12,
                tile * 0.56,
                tile * 0.5,
            );
            let color = if entry.is_dir { base } else { base.alpha(0.75) };
            if entry.is_dir {
                draw_folder_icon(ctx, icon, color);
            } else {
                draw_file_icon(ctx, icon, color);
            }
            if entry.is_link {
                ctx.dl.line(
                    icon.x,
                    icon.bottom(),
                    icon.x + icon.w * 0.3,
                    icon.bottom() - icon.h * 0.3,
                    1.5,
                    base,
                );
            }

            // Nazwa pod ikoną.
            let name = if entry.name.chars().count() > 12 {
                let cut: String = entry.name.chars().take(11).collect();
                format!("{cut}\u{2026}")
            } else {
                entry.name.clone()
            };
            ctx.dl.text_center(
                ctx.fonts,
                FONT_UI,
                name_px,
                trect.cx(),
                y + tile * 0.7,
                &name,
                if entry.is_dir { base } else { base.alpha(0.85) },
                0.0,
            );

            self.hits.push((trect, i));
        }
    }
}

/// Przycina tekst od lewej (z wielokropkiem na początku), aby zmieścił się
/// w zadanej szerokości.
fn fit_tail(ctx: &mut Ctx, px: f32, text: &str, max_w: f32) -> String {
    if ctx.fonts.measure(FONT_UI, px, text, px * 0.06) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut start = 1;
    while start < chars.len() - 1 {
        let candidate: String =
            std::iter::once('\u{2026}').chain(chars[start..].iter().copied()).collect();
        if ctx.fonts.measure(FONT_UI, px, &candidate, px * 0.06) <= max_w {
            return candidate;
        }
        start += 1;
    }
    "\u{2026}".to_string()
}

fn draw_folder_icon(ctx: &mut Ctx, r: Rect, c: crate::theme::Color) {
    // Teczka: zakładka + korpus.
    let tab_w = r.w * 0.4;
    let tab_h = r.h * 0.18;
    let pts = [
        [r.x, r.y + tab_h],
        [r.x, r.y],
        [r.x + tab_w, r.y],
        [r.x + tab_w + tab_h, r.y + tab_h],
        [r.right(), r.y + tab_h],
        [r.right(), r.bottom()],
        [r.x, r.bottom()],
    ];
    ctx.dl.polyline(&pts, 1.5, c, true);
}

fn draw_file_icon(ctx: &mut Ctx, r: Rect, c: crate::theme::Color) {
    // Kartka z zagiętym rogiem.
    let fold = r.w * 0.3;
    let x = r.x + r.w * 0.15;
    let w = r.w * 0.7;
    let pts = [
        [x, r.y],
        [x + w - fold, r.y],
        [x + w, r.y + fold],
        [x + w, r.bottom()],
        [x, r.bottom()],
    ];
    ctx.dl.polyline(&pts, 1.5, c, true);
    ctx.dl.line(
        x + w - fold,
        r.y,
        x + w - fold,
        r.y + fold,
        1.0,
        c.alpha(0.7),
    );
    ctx.dl.line(
        x + w - fold,
        r.y + fold,
        x + w,
        r.y + fold,
        1.0,
        c.alpha(0.7),
    );
}
