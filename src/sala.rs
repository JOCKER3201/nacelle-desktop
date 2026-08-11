//! A hall (sala): one more screen carrying boards of its own. The
//! desktop mode puts HOME on the primary monitor and a hall on every
//! other one; for now every board in a hall is EMPTY, so what a hall
//! shows is the theme itself — the clear colour and the two decoration
//! plates — waiting for its own layauts. That is deliberate: the hall
//! machinery (a window, a renderer, a redraw path per screen) lands
//! first, and boards move in when per-hall layauts exist.
//!
//! Everything a hall draws comes from the theme; with the default
//! theme (every decoration off) a hall is the clear colour and nothing
//! else, exactly like an empty board on the main screen.

use nacelle::base::Rect;
use nacelle::draw::{DrawList, ImageId};
use nacelle::theme::Color;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowBuilder};

pub struct Sala {
    pub window: Window,
    gfx: nacelle_renderer::Gfx,
    backdrop: Option<(ImageId, u32, u32)>,
    overlay: Option<(ImageId, u32, u32)>,
    /// (theme epoch, w, h) the plates were baked for.
    baked: Option<(u32, u32, u32)>,
    /// Glyph-atlas rows this hall's GPU copy is missing, as a row span
    /// (lo, hi exclusive). The main window drains the font system's
    /// dirty rows for its own renderer; whatever it takes is noted
    /// here too, so a hall that starts drawing TEXT (hosting the
    /// settings window) can catch its atlas up.
    atlas_behind: Option<(u32, u32)>,
    /// False until this hall has uploaded the WHOLE atlas once —
    /// glyphs rasterised before the hall existed were never dirty
    /// while it listened, so the first hosted frame syncs everything.
    atlas_synced: bool,
}

impl Sala {
    /// A borderless fullscreen window on the given monitor, with its
    /// own renderer. A hall that cannot come up is reported and
    /// skipped — the desktop keeps its other screens.
    pub fn new(
        el: &winit::event_loop::EventLoop<()>,
        monitor: MonitorHandle,
    ) -> Option<Self> {
        let name = monitor.name().unwrap_or_else(|| "?".into());
        let window = WindowBuilder::new()
            .with_title("nacelle-desktop — sala")
            .with_decorations(false)
            .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
            .build(el)
            .map_err(|e| eprintln!("nacelle-desktop: sala on '{name}' failed: {e}"))
            .ok()?;
        let size = window.inner_size();
        let gfx = nacelle_renderer::Gfx::new(&window, size.width, size.height);
        eprintln!("nacelle-desktop: sala on '{name}' ({}x{})", size.width, size.height);
        Some(Self {
            window,
            gfx,
            backdrop: None,
            overlay: None,
            baked: None,
            atlas_behind: None,
            atlas_synced: false,
        })
    }

    /// The main window drained these rows for its own renderer; note
    /// them so a hosted frame here can upload them too.
    pub fn note_atlas_rows(&mut self, y0: u32, rows: u32) {
        let (lo, hi) = (y0, y0 + rows);
        self.atlas_behind = Some(match self.atlas_behind {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }

    pub fn resize(&mut self) {
        self.gfx.resize();
    }

    /// One hall frame: the theme's clear colour under the backdrop
    /// plate, the overlay plate above — an empty board, by the book.
    /// Plates rebake when the theme epoch or the size changes, never
    /// per frame; the bake is synchronous because both events are
    /// user-visible moments (a theme switch, a mode change) where a
    /// few milliseconds cost nothing a frame would notice.
    pub fn draw(&mut self) {
        self.frame(None, |_, _, _, _| {});
    }

    /// A hall frame HOSTING interface: `overlay` draws whatever the
    /// hall is asked to carry (the settings window follows the screen
    /// the pointer is on) between the two plates, with text. The hall
    /// uploads the glyph rows it fell behind on — the whole atlas the
    /// first time — so the text arrives whole.
    pub fn draw_hosted(
        &mut self,
        fonts: &mut nacelle::font::FontSystem,
        overlay: impl FnOnce(&mut DrawList, f32, f32, &mut nacelle::font::FontSystem),
    ) {
        // Split borrows: frame() needs &mut self, fonts travel beside.
        self.frame(Some(fonts), overlay);
    }

    fn frame(
        &mut self,
        mut fonts: Option<&mut nacelle::font::FontSystem>,
        overlay: impl FnOnce(&mut DrawList, f32, f32, &mut nacelle::font::FontSystem),
    ) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let key = (nacelle::theme::epoch(), size.width, size.height);
        if self.baked != Some(key) {
            self.rebake(size.width, size.height);
            self.baked = Some(key);
        }
        let (w, h) = (size.width as f32, size.height as f32);
        let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut dl = DrawList::new();
        let full = Rect::new(0.0, 0.0, w, h);
        if let Some((id, _, _)) = self.backdrop {
            dl.image(full.x, full.y, full.w, full.h, id, white);
        }
        if let Some(f) = fonts.as_deref_mut() {
            overlay(&mut dl, w, h, f);
        }
        if let Some((id, _, _)) = self.overlay {
            dl.image(full.x, full.y, full.w, full.h, id, white);
        }
        // The atlas this hall owes its renderer: everything it noted
        // while the main window drained, plus whatever the overlay
        // just rasterised, plus the WHOLE atlas the first hosted time.
        let atlas = fonts.map(|f| {
            if let Some((y0, rows)) = f.take_dirty_rows() {
                self.note_atlas_rows(y0, rows);
            }
            if !self.atlas_synced {
                self.atlas_synced = true;
                self.atlas_behind = None;
                (f.atlas.as_slice(), 0u32, nacelle::font::ATLAS_H as u32)
            } else {
                let (lo, hi) = self.atlas_behind.take().unwrap_or((0, 0));
                (f.atlas.as_slice(), lo, hi - lo)
            }
        });
        let atlas = atlas.filter(|(_, _, rows)| *rows > 0);
        let clear = nacelle::deco::clear_color();
        self.gfx.render(
            size.width,
            size.height,
            &dl.verts,
            &dl.runs,
            atlas,
            [clear.r, clear.g, clear.b, 1.0],
        );
    }

    fn rebake(&mut self, w: u32, h: u32) {
        let install = |tex: &mut Option<(ImageId, u32, u32)>,
                           baked: Option<nacelle::theme::Plate>,
                           gfx: &mut nacelle_renderer::Gfx| {
            match baked {
                Some(p) => {
                    let stale = match *tex {
                        Some((_, tw, th)) => (tw, th) != (p.w, p.h),
                        None => true,
                    };
                    if stale {
                        if let Some((old, _, _)) = tex.take() {
                            gfx.destroy_texture(old);
                        }
                        *tex = Some((gfx.create_texture(p.w, p.h), p.w, p.h));
                    }
                    if let Some((id, _, _)) = *tex {
                        gfx.update_texture(id, &p.rgba);
                    }
                }
                // Every layer off: no plate, no quad.
                None => {
                    if let Some((old, _, _)) = tex.take() {
                        gfx.destroy_texture(old);
                    }
                }
            }
        };
        let back = nacelle::theme::plate::bake_backdrop(w, h);
        let over = nacelle::theme::plate::bake_overlay(w, h);
        install(&mut self.backdrop, back, &mut self.gfx);
        install(&mut self.overlay, over, &mut self.gfx);
    }
}
