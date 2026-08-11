//! What the machine's screens are: winit's monitors joined with the
//! physical millimetres the display server knows about them, because
//! two decisions here depend on GEOMETRY rather than pixels — a chassis
//! screen is "10 inches or less" whatever its resolution, and a desktop
//! spanning several monitors needs to know which one is which.
//!
//! The millimetres come from RandR (x11rb), which covers both X11
//! sessions and XWayland — gamescope included. A pure Wayland session
//! without an X socket simply reports no diagonal: every decision that
//! reads one degrades to "not found", never to a guess.

use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;

pub struct Screen {
    pub monitor: MonitorHandle,
    /// Physical diagonal in inches, when the display server says.
    pub diagonal_in: Option<f32>,
    pub primary: bool,
}

/// Every monitor the event loop can see, joined with RandR's physical
/// sizes by CRTC geometry (position + pixel size), primary first.
///
/// Which one is PRIMARY is answered three ways, in order of knowledge:
/// winit's own answer (X11 has one), then RandR's primary output
/// matched by geometry (native Wayland winit answers None, but the
/// session's XWayland still knows), then the monitor at the origin —
/// somebody must be the main screen, and (0,0) is where every desktop
/// puts it by default.
pub fn survey(el: &EventLoop<()>) -> Vec<Screen> {
    let winit_primary = el.primary_monitor();
    let (physical, randr_primary) = randr_info().unwrap_or((Vec::new(), None));
    let mut out: Vec<Screen> = el
        .available_monitors()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            let phys = physical
                .iter()
                .find(|p| {
                    p.x == pos.x && p.y == pos.y && p.px_w == size.width && p.px_h == size.height
                })
                .or_else(|| {
                    // A second chance by pixel size alone: a compositor
                    // may shift logical positions while CRTCs stay put.
                    physical
                        .iter()
                        .find(|p| p.px_w == size.width && p.px_h == size.height)
                });
            let diagonal_in = phys.and_then(|p| diagonal(p.mm_w, p.mm_h));
            let primary = winit_primary
                .as_ref()
                .map(|p| *p == m)
                .unwrap_or_else(|| {
                    phys.map(|p| Some(p.output) == randr_primary).unwrap_or(false)
                });
            Screen { monitor: m, diagonal_in, primary }
        })
        .collect();
    if !out.iter().any(|s| s.primary) {
        // Nobody answered: the origin monitor is the main screen.
        if let Some(i) = out
            .iter()
            .position(|s| s.monitor.position() == winit::dpi::PhysicalPosition::new(0, 0))
            .or(if out.is_empty() { None } else { Some(0) })
        {
            out[i].primary = true;
        }
    }
    // Primary first, so "the main screen" is always index 0.
    out.sort_by_key(|s| !s.primary);
    for s in &out {
        eprintln!(
            "nacelle-desktop: screen '{}' {}x{}{}{}",
            s.monitor.name().unwrap_or_else(|| "?".into()),
            s.monitor.size().width,
            s.monitor.size().height,
            s.diagonal_in.map(|d| format!(" {d:.1}\"")).unwrap_or_default(),
            if s.primary { " — primary" } else { "" },
        );
    }
    out
}

/// The smallest screen at or under the chassis threshold — the little
/// panel in a computer case this program makes a fine face for.
pub fn chassis(screens: &[Screen]) -> Option<&Screen> {
    screens
        .iter()
        .filter(|s| s.diagonal_in.map(|d| d <= 10.0).unwrap_or(false))
        .min_by(|a, b| {
            a.diagonal_in
                .partial_cmp(&b.diagonal_in)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn diagonal(mm_w: u32, mm_h: u32) -> Option<f32> {
    // RandR reports 0x0 for projectors, headless outputs and anything
    // whose EDID keeps quiet — no measurement, not a tiny screen.
    if mm_w == 0 || mm_h == 0 {
        return None;
    }
    let (w, h) = (mm_w as f32, mm_h as f32);
    Some((w * w + h * h).sqrt() / 25.4)
}

struct PhysOutput {
    output: u32,
    x: i32,
    y: i32,
    px_w: u32,
    px_h: u32,
    mm_w: u32,
    mm_h: u32,
}

/// Connected RandR outputs with their CRTC geometry and physical size,
/// plus the server's primary output. Any failure — no X socket, no
/// RandR, a racing disconnect — is an empty answer.
fn randr_info() -> Option<(Vec<PhysOutput>, Option<u32>)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let res = conn.randr_get_screen_resources_current(root).ok()?.reply().ok()?;
    let primary = conn
        .randr_get_output_primary(root)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.output)
        .filter(|&o| o != x11rb::NONE);
    let mut out = Vec::new();
    for &o in &res.outputs {
        let Ok(info) = conn.randr_get_output_info(o, res.config_timestamp) else { continue };
        let Ok(info) = info.reply() else { continue };
        if info.crtc == x11rb::NONE {
            continue; // not driving anything
        }
        let Ok(crtc) = conn.randr_get_crtc_info(info.crtc, res.config_timestamp) else { continue };
        let Ok(crtc) = crtc.reply() else { continue };
        out.push(PhysOutput {
            output: o,
            x: crtc.x as i32,
            y: crtc.y as i32,
            px_w: crtc.width as u32,
            px_h: crtc.height as u32,
            mm_w: info.mm_width,
            mm_h: info.mm_height,
        });
    }
    Some((out, primary))
}

#[cfg(test)]
mod tests {
    use super::diagonal;

    /// The arithmetic the chassis rule hangs on: a 217x136 mm panel is
    /// the classic 10.1", a 0x0 EDID is no measurement at all.
    #[test]
    fn a_diagonal_is_inches_or_nothing() {
        let d = diagonal(217, 136).unwrap();
        assert!((d - 10.08).abs() < 0.05, "217x136 mm is a 10.1\" panel, got {d}");
        let d = diagonal(598, 336).unwrap();
        assert!((d - 27.0).abs() < 0.2, "598x336 mm is a 27\" monitor, got {d}");
        assert!(diagonal(0, 336).is_none(), "a silent EDID measures nothing");
    }
}
