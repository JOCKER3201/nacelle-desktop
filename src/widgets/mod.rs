//! Układ interfejsu — wierna replika rozmieszczenia paneli eDEX-UI:
//! lewa kolumna (17%), centralny terminal (65% x 60%), prawa kolumna (17%),
//! dolny pas: przeglądarka plików + klawiatura ekranowa.

pub mod boot;
pub mod control;
pub mod filesystem;
pub mod keyboard;
pub mod left;
pub mod right;
pub mod settings;
pub mod shell;

use crate::draw::DrawList;
use crate::font::FontSystem;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
}

/// Pozycja i rozmiar panelu w jednostkach vw/vh (procent okna).
#[derive(Clone, Copy)]
pub struct PanelSpec {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Układ paneli — domyślny lub wczytany z pliku .layaut motywu.
#[derive(Clone)]
pub struct LayoutSpec {
    pub left_col: PanelSpec,
    pub shell: PanelSpec,
    pub right_col: PanelSpec,
    pub filesystem: PanelSpec,
    pub keyboard: PanelSpec,
    pub control: PanelSpec,
}

impl Default for LayoutSpec {
    fn default() -> Self {
        LayoutSpec {
            left_col: PanelSpec { x: 0.6, y: 2.5, w: 16.4, h: 59.5 },
            shell: PanelSpec { x: 17.5, y: 2.5, w: 65.0, h: 60.3 },
            right_col: PanelSpec { x: 83.0, y: 2.5, w: 16.4, h: 59.5 },
            // Pliki w prawej kolumnie pod NETWORK STATUS, do dołu ekranu.
            filesystem: PanelSpec { x: 83.0, y: 17.4, w: 16.4, h: 79.6 },
            // Klawiatura bezpośrednio pod terminalem, o jego szerokości.
            keyboard: PanelSpec { x: 17.5, y: 64.5, w: 65.0, h: 32.5 },
            // Panel sterowania programem w lewym dolnym rogu.
            control: PanelSpec { x: 0.6, y: 64.5, w: 16.4, h: 32.5 },
        }
    }
}

/// Wyliczone prostokąty paneli (w pikselach fizycznych).
pub struct Layout {
    pub left_col: Rect,
    pub shell: Rect,
    pub right_col: Rect,
    pub filesystem: Rect,
    pub keyboard: Rect,
    pub control: Rect,
}

impl Layout {
    pub fn compute(w: f32, h: f32, spec: &LayoutSpec) -> Self {
        let vw = w / 100.0;
        let vh = h / 100.0;
        let r = |p: &PanelSpec| Rect::new(p.x * vw, p.y * vh, p.w * vw, p.h * vh);
        Layout {
            left_col: r(&spec.left_col),
            shell: r(&spec.shell),
            right_col: r(&spec.right_col),
            filesystem: r(&spec.filesystem),
            keyboard: r(&spec.keyboard),
            control: r(&spec.control),
        }
    }
}

/// Kontekst rysowania przekazywany do paneli.
pub struct Ctx<'a> {
    pub dl: &'a mut DrawList,
    pub fonts: &'a mut FontSystem,
    pub theme: &'a Theme,
    /// Szerokość/wysokość okna w px.
    pub w: f32,
    pub h: f32,
    /// Czas od startu aplikacji w sekundach.
    pub t: f64,
    /// Pozycja kursora myszy.
    pub mouse: (f32, f32),
}

impl<'a> Ctx<'a> {
    pub fn vh(&self, v: f32) -> f32 {
        self.h / 100.0 * v
    }
    pub fn vw(&self, v: f32) -> f32 {
        self.w / 100.0 * v
    }
    /// Rozmiar fontu nie mniejszy niż 8 px (czytelność na małych oknach).
    pub fn font_px(&self, v: f32) -> f32 {
        self.vh(v).max(8.0)
    }
}
