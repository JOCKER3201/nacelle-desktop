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
//!
//! RandR answers one more thing here: what each screen is CALLED. A
//! screen has to have a name the configuration can refer to — the
//! connector it hangs off — because a desktop that gives every monitor
//! its own layout has to know which monitor is which across reboots.

use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;

pub struct Screen {
    pub monitor: MonitorHandle,
    /// The socket this screen hangs off — `DP-1`, `HDMI-A-1`, `eDP-1`.
    ///
    /// A screen's IDENTITY, and the one thing about it the user can
    /// write down: unplugging a monitor and plugging it back in, or
    /// switching the monitors on in another order, leave it exactly
    /// what it was. A position in this list survives neither, so
    /// nothing that has to name a screen may be keyed to one.
    ///
    /// None when nothing names the connector — a display server that
    /// reports only a marketing string, a headless output. Such a
    /// screen has no identity to configure and simply takes whatever
    /// every unnamed screen takes.
    pub connector: Option<String>,
    /// Physical diagonal in inches, when the display server says.
    pub diagonal_in: Option<f32>,
    pub primary: bool,
}

/// The connector name a display name carries, or None.
///
/// Both display servers are asked the same question and answer it
/// differently: RandR names the output outright (`DP-1`), while a
/// Wayland compositor may hand winit the connector, `DP-1 Dell Inc.
/// U2720Q`, or nothing but the make and model. So the first
/// whitespace-separated word is taken — the convention the EDID
/// lookup in config.rs already reads names by — and kept only if it
/// LOOKS like a connector: letters and digits in dash-separated
/// parts, `HDMI-A-1` as much as `eDP-1`.
///
/// The shape test is what keeps a make and model out: `Dell` is a
/// perfectly good word and a hopeless screen name, because the second
/// Dell on the desk answers to it too.
pub fn connector_of(name: &str) -> Option<String> {
    let token = name.split_whitespace().next()?;
    let parts: Vec<&str> = token.split('-').collect();
    let shaped = parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()));
    shaped.then(|| token.to_string())
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
            // RandR names the output for what it is, so it is asked
            // first; winit's own name is the answer on a session with
            // no X socket, where it is all there is.
            let connector = phys
                .and_then(|p| connector_of(&p.name))
                .or_else(|| m.name().as_deref().and_then(connector_of));
            let primary = winit_primary
                .as_ref()
                .map(|p| *p == m)
                .unwrap_or_else(|| {
                    phys.map(|p| Some(p.output) == randr_primary).unwrap_or(false)
                });
            Screen { monitor: m, connector, diagonal_in, primary }
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
        let name = s.monitor.name().unwrap_or_else(|| "?".into());
        // The connector is said out loud whenever it is not already
        // the whole name: it is what the user has to type to give this
        // screen a layaut of its own, and guessing it from a monitor's
        // model is not something anybody should have to do.
        let on = match &s.connector {
            Some(c) if *c != name => format!(" on {c}"),
            _ => String::new(),
        };
        eprintln!(
            "nacelle-desktop: screen '{}'{} {}x{}{}{}",
            name,
            on,
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
    /// The connector RandR calls this output, verbatim.
    name: String,
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
            name: String::from_utf8_lossy(&info.name).into_owned(),
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
    use super::{connector_of, diagonal};

    /// The name a screen is configured by. Every form the two display
    /// servers hand over resolves to the same connector, and anything
    /// that would not identify a screen tomorrow resolves to nothing —
    /// a screen with no name takes the default layaut rather than
    /// borrowing another screen's.
    #[test]
    fn a_screen_is_named_by_its_connector_or_not_at_all() {
        for name in ["DP-1", "HDMI-A-1", "eDP-1", "DVI-I-1", "Virtual-1"] {
            assert_eq!(connector_of(name).as_deref(), Some(name), "{name} is a connector");
        }
        // Wayland hands over the connector with the model behind it.
        assert_eq!(connector_of("DP-1 Dell Inc. U2720Q").as_deref(), Some("DP-1"));
        // A make and model alone names no socket: two of them answer
        // to the same word, so it may not become a key.
        for name in ["", "?", "Dell", "Dell Inc. U2720Q", "-1", "DP-"] {
            assert!(connector_of(name).is_none(), "'{name}' must not name a screen");
        }
    }

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
