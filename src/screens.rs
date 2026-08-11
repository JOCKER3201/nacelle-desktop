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
pub fn survey(el: &EventLoop<()>) -> Vec<Screen> {
    let primary = el.primary_monitor();
    let physical = randr_mm().unwrap_or_default();
    let mut out: Vec<Screen> = el
        .available_monitors()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            let diagonal_in = physical
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
                })
                .and_then(|p| diagonal(p.mm_w, p.mm_h));
            let primary = primary.as_ref().map(|p| *p == m).unwrap_or(false);
            Screen { monitor: m, diagonal_in, primary }
        })
        .collect();
    // Primary first, so "the main screen" is always index 0 when one
    // is declared; the rest keep the server's order.
    out.sort_by_key(|s| !s.primary);
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
    x: i32,
    y: i32,
    px_w: u32,
    px_h: u32,
    mm_w: u32,
    mm_h: u32,
}

/// Connected RandR outputs with their CRTC geometry and physical size.
/// Any failure — no X socket, no RandR, a racing disconnect — is an
/// empty answer.
fn randr_mm() -> Option<Vec<PhysOutput>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let res = conn.randr_get_screen_resources_current(root).ok()?.reply().ok()?;
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
            x: crtc.x as i32,
            y: crtc.y as i32,
            px_w: crtc.width as u32,
            px_h: crtc.height as u32,
            mm_w: info.mm_width,
            mm_h: info.mm_height,
        });
    }
    Some(out)
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
