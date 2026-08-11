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
        Some(Self { window, gfx, backdrop: None, overlay: None, baked: None })
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
        let mut dl = DrawList::new();
        let full = Rect::new(0.0, 0.0, w, h);
        if let Some((id, _, _)) = self.backdrop {
            dl.image(full.x, full.y, full.w, full.h, id, Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
        }
        if let Some((id, _, _)) = self.overlay {
            dl.image(full.x, full.y, full.w, full.h, id, Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
        }
        let clear = nacelle::deco::clear_color();
        self.gfx.render(
            size.width,
            size.height,
            &dl.verts,
            &dl.runs,
            None,
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
