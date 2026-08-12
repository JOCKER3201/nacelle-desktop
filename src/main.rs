//! nacelle-desktop — an independent sci-fi terminal inspired by eDEX-UI, in Rust + Vulkan.
//! Left column with telemetry, central terminal, right column with network
//! and files, on-screen keyboard and control panel at the bottom.

mod audio;
mod clipboard;
mod config;
mod plugins;
mod pty;
mod sala;
mod screens;
mod system;
mod fullscreen;
mod hashframe;
mod widgets;
mod wl_color;

// The platform-independent base (drawing, fonts, themes, layout engine,
// terminal emulation) lives in the libnacelle crate; re-export its
// modules under crate:: so the rest of the code refers to them without
// naming the crate every time. This tree keeps only the Linux parts.
pub use nacelle::{draw, flex, font, term, theme};

use nacelle::theme::{ThemeColor, TokenId};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::OnceLock;
use std::time::Instant;

use winit::event::{ElementState, Event, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::wayland::EventLoopBuilderExtWayland;
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{CursorIcon, Fullscreen, WindowBuilder};

use crate::pty::PtyEvent;

// ---- theme token access -------------------------------------------------
// Each name resolves once per process. A token the master does not declare
// degrades through the engine's per-kind fallback — grey, zero, false —
// never through a value that used to be the design.
fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

/// The engine's colour, as the draw list's `Color`.
fn tcol(c: ThemeColor) -> theme::Color {
    theme::Color { r: c.r, g: c.g, b: c.b, a: c.a }
}





/// One terminal session (tab): PTY + emulation + parser.
struct Session {
    term: term::Term,
    pty: pty::Pty,
    rx: Receiver<PtyEvent>,
    parser: vte::Parser,
    /// Where the shell stands, and whether that answer is still good.
    /// Asking means a readlink through /proc, and the file panel wants
    /// the answer every frame; the shell, however, only moves when it
    /// is told to, and being told leaves a trace — a new prompt. So the
    /// question is asked when the shell has spoken since the last
    /// answer, and once in a long while besides, for the rare program
    /// that changes directory without saying anything.
    cwd: Option<PathBuf>,
    cwd_asked: Option<Instant>,
    spoke: bool,
    /// A paste still on its way to the PTY. Fed in [`PASTE_CHUNK`]
    /// slices from `pump`, one per tick like the PTY reads themselves:
    /// a 5 MB paste must not spin the event loop, and the kernel's PTY
    /// buffer is small.
    paste_buf: Vec<u8>,
    paste_off: usize,
}

/// How much of a pending paste one tick hands the PTY.
const PASTE_CHUNK: usize = 4096;

/// How many shell sessions this application keeps.
///
/// The application's own number, not a widget's: the sessions are the
/// program's — their PTYs, their scrollback, their lifetime — and a
/// widget that draws a strip of them is told how many there are through
/// `Host::tabs`. Nothing here knows whether any widget draws them at
/// all.
const SESSIONS: usize = 5;

impl Session {
    fn spawn(cols: usize, rows: usize, cwd: &Path) -> std::io::Result<Session> {
        let (pty, rx) = pty::Pty::spawn(cols as u16, rows as u16, Some(cwd))?;
        Ok(Session {
            term: term::Term::new(cols, rows),
            pty,
            rx,
            parser: vte::Parser::new(),
            cwd: None,
            cwd_asked: None,
            spoke: true,
            paste_buf: Vec::new(),
            paste_off: 0,
        })
    }

    /// Pastes text into this session: sanitised and bracketed by the
    /// EMULATION (it owns mode 2004), then queued for `pump` to feed.
    fn paste(&mut self, text: &str) {
        let bytes = self.term.paste_bytes(text);
        if bytes.is_empty() {
            return;
        }
        self.paste_buf.extend_from_slice(&bytes);
        self.term.view_offset = 0;
    }

    /// Processes PTY data; returns true if the shell has exited.
    fn pump(&mut self) -> bool {
        let mut exited = false;
        for ev in self.rx.try_iter() {
            match ev {
                PtyEvent::Data(data) => {
                    self.spoke = true;
                    let mut performer = term::Performer { term: &mut self.term };
                    for byte in data {
                        self.parser.advance(&mut performer, byte);
                    }
                }
                PtyEvent::Exited => exited = true,
            }
        }
        if !self.term.responses.is_empty() {
            let resp = std::mem::take(&mut self.term.responses);
            self.pty.write(&resp);
        }
        // One slice of any pending paste per tick.
        if self.paste_off < self.paste_buf.len() {
            let end = (self.paste_off + PASTE_CHUNK).min(self.paste_buf.len());
            self.pty.write(&self.paste_buf[self.paste_off..end]);
            self.paste_off = end;
            if self.paste_off == self.paste_buf.len() {
                self.paste_buf.clear();
                self.paste_off = 0;
            }
        }
        exited
    }

    /// The shell's working directory, re-read only when it can have
    /// changed.
    fn cwd(&mut self) -> Option<PathBuf> {
        let stale = self
            .cwd_asked
            .map(|t| t.elapsed() >= std::time::Duration::from_secs(2))
            .unwrap_or(true);
        if self.spoke || stale {
            self.cwd = self.pty.child_cwd();
            self.cwd_asked = Some(Instant::now());
            self.spoke = false;
        }
        self.cwd.clone()
    }
}

fn main() {
    // The two layout aids run and leave before any window exists —
    // they are for the user whose layout is broken enough that the
    // settings window may not be reachable (u1 §5.3).
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Desktop mode: this process IS the desktop (nacelle-session
    // starts it so), which claims the primary screen for HOME and
    // raises a hall on every other one. Without the flag the program
    // is a guest on somebody's desktop and takes one window.
    let desktop_mode = args.iter().any(|a| a == "--desktop");
    if args.iter().any(|a| a == "--print-layaut") {
        // The effective layout — built-in or file — in .layaut syntax,
        // so what the user starts from is inspectable, not folklore.
        let (cfg, _) = config::load();
        print!("{}", config::print_layaut(&cfg.layout));
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--reset-screen-layaut") {
        // Deletes the pinned [WxH@D] section of the selected layout for
        // one screen: the one named after the flag ("1920x1080@27"), or
        // the primary monitor's when none is.
        let _ = config::load();
        let key = args
            .get(i + 1)
            .and_then(|s| config::parse_screen_key(s))
            .or_else(|| {
                EventLoopBuilder::new().build().ok().and_then(|el| {
                    el.primary_monitor()
                        .or_else(|| el.available_monitors().next())
                        .map(|m| {
                            let s = m.size();
                            let diag = m
                                .name()
                                .map(|n| config::monitor_diag_inches(&n))
                                .unwrap_or(0);
                            (s.width, s.height, diag)
                        })
                })
            });
        let Some(key) = key else {
            eprintln!(
                "nacelle-desktop: cannot determine the screen \u{2014} pass it \
                 explicitly: --reset-screen-layaut 1920x1080@27"
            );
            return;
        };
        let name = config::current_layaut_name()
            .unwrap_or_else(|| "default".to_string());
        match config::clear_screen_section(&name, key) {
            Ok(()) => eprintln!(
                "nacelle-desktop: cleared the {}x{}@{} section of layaut '{}'",
                key.0, key.1, key.2, name
            ),
            Err(e) => eprintln!("nacelle-desktop: cannot reset this screen: {e}"),
        }
        return;
    }

    // Configuration: the XDG cascade — the user's own file over the
    // system ones. Nothing is created here; the directory appears the
    // first time the user changes a setting.
    let (cfg, startup_warning) = config::load();
    let mut layout_spec = cfg.layout;
    let mut fonts = font::FontSystem::new();
    // Font preferences (size scales + family/weight, terminal and UI).
    let (mut font_scale, tfam, twgt) = config::term_font_prefs();
    let (mut ui_font_scale, ufam, uwgt) = config::ui_font_prefs();
    // Widget padding: content inset from the outer panel edge (GRID view).
    let mut ui_padding = config::grid_prefs().3 as f32;
    let mut last_term_key = (tfam.clone().unwrap_or_default(), twgt.clone().unwrap_or_default());
    let mut last_ui_key = (ufam.clone().unwrap_or_default(), uwgt.clone().unwrap_or_default());
    if tfam.is_some() || twgt.is_some() {
        if let Some(f) = font::load_variant_for(tfam.as_deref(), twgt.as_deref(), false) {
            fonts.set_mono(f);
        }
    }
    if ufam.is_some() || uwgt.is_some() {
        if let Some(f) = font::load_variant_for(ufam.as_deref(), uwgt.as_deref(), true) {
            fonts.set_ui(f);
        }
    }

    // Window backend selection: Wayland natively, but an X11 session or
    // gamescope (a gaming compositor exposing XWayland) forces X11.
    let wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let x11 = std::env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false);
    let gamescope = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("gamescope"))
            .unwrap_or(false)
        || std::env::var("XDG_SESSION_DESKTOP")
            .map(|d| d.to_lowercase().contains("gamescope"))
            .unwrap_or(false);

    let event_loop = {
        let mut builder = EventLoopBuilder::new();
        if gamescope && x11 {
            eprintln!("nacelle-desktop: gamescope detected — X11 backend");
            builder.with_x11();
        } else if wayland && !gamescope {
            eprintln!("nacelle-desktop: Wayland backend (native)");
            builder.with_wayland();
        } else if x11 {
            eprintln!("nacelle-desktop: X11 backend");
            builder.with_x11();
        }
        builder.build().expect("cannot create event loop")
    };

    // Monitor resolution check (orientation-agnostic: a rotated 720x1280
    // panel is fine). Below the minimum the program does NOT start — only
    // a small dialog window with the message is shown.
    let monitor_size = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map(|m| m.size());
    if let Some(s) = monitor_size {
        let (long, short) = (s.width.max(s.height), s.width.min(s.height));
        if long < 1280 || short < 720 {
            eprintln!(
                "nacelle-desktop: monitor resolution {}x{} is below the 1280x720 minimum",
                s.width, s.height
            );
            run_resolution_dialog(event_loop, fonts, s.width, s.height);
            return;
        }
    }

    // Which screen the main window opens on. Desktop mode claims the
    // primary and raises a hall on every other screen below; without
    // the flag the one special case is a CHASSIS screen — a panel of
    // ten inches or less built into a computer case, which this
    // program makes a fine face for. None keeps winit's own choice,
    // the current monitor — exactly the old behaviour.
    let screens = screens::survey(&event_loop);
    let main_monitor = if desktop_mode {
        screens.first().filter(|s| s.primary).map(|s| s.monitor.clone())
    } else {
        screens::chassis(&screens).map(|s| {
            eprintln!(
                "nacelle-desktop: chassis screen '{}' ({:.1}\") \u{2014} opening there",
                s.monitor.name().unwrap_or_else(|| "?".into()),
                s.diagonal_in.unwrap_or(0.0)
            );
            s.monitor.clone()
        })
    };

    let window = WindowBuilder::new()
        .with_title("nacelle-desktop")
        .with_decorations(false)
        .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0))
        // Start fullscreen right away, like eDEX-UI.
        .with_fullscreen(Some(Fullscreen::Borderless(main_monitor)))
        .build(&event_loop)
        .expect("cannot create window");
    // Minimum window size in landscape orientation.
    window.set_min_inner_size(Some(winit::dpi::PhysicalSize::new(1280u32, 720u32)));

    // The screen the window is on, as the layout sees it. Read when it
    // can actually change — at startup and on a resize or a scale
    // change — because asking costs a round trip to the display server.
    let mut screen = screen_key(&window);
    // The theme engine bakes every u-derived length from the window
    // height (u = clamp(h × metric.unit_pct_h, …) — §2.2). config::load
    // ran before a window existed, so the engine is still on its
    // 1080-line default; on a 800-line window that is a 35 % oversize on
    // every metric token, which is how the control buttons left their
    // panel. Told here and on every resize, never per frame. The second
    // argument is `UIFontSize=`/100 the day it moves into u; today the
    // font path applies it itself, so the bake takes 1.0.
    nacelle::theme::set_viewport(window.inner_size().height as f32, 1.0);
    // Per-screen layout override matching the current monitor
    // (resolution + diagonal), refreshed on resize and config changes.
    let mut active_ov = layout_spec.pick(screen).cloned();

    let mut gfx = nacelle_renderer::Gfx::new(&window, window.inner_size().width, window.inner_size().height);

    // The colour pipeline: only a native Wayland session has a
    // compositor to discuss colour with. Everywhere else this stays
    // None, the COLOR settings are greyed out and their stored values
    // are never read.
    let mut color_mgr = {
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
        let dh = window.display_handle().ok().map(|h| h.as_raw());
        let wh = window.window_handle().ok().map(|h| h.as_raw());
        // The clipboard backend follows whatever winit actually
        // connected to — the same handle, read once, never an env var.
        clipboard::install(dh);
        match (dh, wh) {
            (
                Some(RawDisplayHandle::Wayland(d)),
                Some(RawWindowHandle::Wayland(w)),
            ) => wl_color::ColorMgr::start(d.display.as_ptr(), w.surface.as_ptr()),
            _ => None,
        }
    };
    // Applied on start and after every change in the COLOR view.
    macro_rules! apply_color {
        () => {{
            if let Some(mgr) = color_mgr.as_mut() {
                let prefs = config::color_prefs();
                gfx.set_color_depth(prefs.depth);
                let lut = prefs
                    .lut
                    .as_deref()
                    .and_then(|name| config::color_file_path("lut", name))
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .and_then(|text| nacelle_renderer::parse_cube(&text));
                if prefs.lut.is_some() && lut.is_none() {
                    eprintln!("nacelle-desktop: the chosen .cube did not parse — no grading");
                }
                gfx.set_lut(lut);
                let icc = prefs
                    .icc
                    .as_deref()
                    .and_then(|name| config::color_file_path("icc", name));
                mgr.apply(&prefs.space, icc.as_deref());
            }
        }};
    }
    apply_color!();

    // Under gamescope every launched program takes the whole screen —
    // that is the compositor's own model, and leaning into it replaced
    // a window manager's worth of frame machinery. Elsewhere a real
    // window manager owns the windows and this stays out of the way.
    let mut fullscreen =
        if gamescope { fullscreen::Fullscreen::start(&window) } else { None };
    if fullscreen.is_some() {
        eprintln!("nacelle-desktop: gamescope clients go fullscreen");
    }

    // Desktop mode: every screen beyond the primary gets a hall — the
    // board environment with, for now, nothing but empty boards on it.
    // Which hall the settings window is being drawn on, and where the
    // pointer is there. The window follows the hand from screen to
    // screen; None means the main window has it, as it always did.
    let mut settings_host: Option<usize> = None;
    let mut sala_mouse = (0.0f32, 0.0f32);
    // Whether the main window is still showing the boot log. Halls
    // clone the main screen, and during the boot the main screen is not
    // a board — so a hall stays at its plates until the first frame
    // that draws one. True until the main window says otherwise: a hall
    // may well be asked to redraw before the main window has drawn at
    // all.
    let mut booting = true;
    let mut salas: Vec<sala::Sala> = if desktop_mode && screens.len() > 1 {
        screens
            .iter()
            .filter(|s| !s.primary)
            .filter_map(|s| sala::Sala::new(&event_loop, s.monitor.clone()))
            .collect()
    } else {
        Vec::new()
    };

    // Sound. Optional by design: without a device the program simply
    // runs silent. The theme's meta file is what maps events to files.
    let mut audio = audio::Audio::new();
    if audio.is_none() {
        eprintln!("nacelle-desktop: no audio output available — running silent");
    }
    if let (Some(a), Some(dir)) = (audio.as_mut(), config::active_sounds_dir()) {
        a.load_theme(&dir);
        let (vol, typing, ambient) = config::sound_prefs();
        a.set_volume(vol as f32 / 100.0);
        a.set_typing_enabled(typing);
        a.set_ambient_enabled(ambient);
        eprintln!(
            "nacelle-desktop: audio {} Hz, sound theme '{}' ({} events)",
            a.rate(),
            dir.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
            a.event_count()
        );
    }
    let mut panel_scale: Vec<f32> = vec![1.0; widgets::panel_count()];
    // The content box the host's container left each panel on its last
    // draw (u2 §4.1): the rect a widget drew in is the ONE rect its
    // clicks and wheel turns are answered in, so it is stored rather
    // than recomputed — input arrives with no frame in flight.
    let mut panel_content: Vec<Option<widgets::Rect>> = vec![None; widgets::panel_count()];

    // ---- widget boards ------------------------------------------------
    // Extra spaces for widgets around the central one, Android-style:
    // hold the left button and drag sideways to turn to a neighbour.
    // Only the boards turn — the settings window, the popup and the
    // editor are the application's own and stay where they are.
    // Boards live on a row centred on home — (x, 0) to the sides — plus
    // the permanent top and bottom boards, which sit above and below
    // EVERY board on the row (y grows downwards like the screen). The
    // position remembers where on the row the hand came from, so the
    // slide back down returns there; which board a position shows is
    // config::board_key's answer.
    #[allow(unused_assignments)]
    // The board world — identity, topology and presence — lives in the
    // toolkit now (u3 L5/D3): one object instead of three locals and
    // five macro bodies. The macros below survive as thin shims so the
    // 2000-line loop's call sites did not have to move in the same
    // commit as the logic.
    let mut world = nacelle::stage::BoardWorld::new(nacelle::layout::LayoutDef::default());
    // The program always starts on the central board.
    let mut cur_board: config::BoardId = (0, 0);
    // A held click, not delivered yet: it becomes a board drag if the
    // pointer travels, and the widget's click on release if it does
    // not. Delivering on release is what lets one gesture be both.
    let mut press_at: Option<(f32, f32)> = None;
    // A drag in progress, locked to the axis it started on. Sideways
    // the world turns like a cube and the number is degrees; up and
    // down it slides flat and the number is the fraction of the window
    // already travelled. Positive goes right, or down.
    let mut pan: Option<(bool, f32)> = None;
    /// The move finishing (or undoing) itself after release.
    struct Cube {
        horizontal: bool,
        a0: f32,
        a1: f32,
        t0: Instant,
        /// Board the move lands on when it completes.
        to: config::BoardId,
        /// Board shown coming in while it moves.
        face_b: config::BoardId,
    }
    let mut cube: Option<Cube> = None;
    // Steps still to walk after the current move lands, last first.
    // Going to a distant board from the BOARDS view is a chain of
    // single-neighbour moves, so the animation passes through whatever
    // lies between.
    let mut go_queue: Vec<config::BoardId> = Vec::new();
    let mut sfx: Vec<nacelle::sound::Event> = Vec::new();
    nacelle::sound::emit(nacelle::sound::Event::Boot);

    // System telemetry in the background.
    let sys = system::start();

    // Home directory — default start directory for the terminal and file panel.
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));

    // Terminal sessions (tabs). Slot 0 starts immediately.
    let mut grid = (80usize, 24usize);
    let mut sessions: Vec<Option<Session>> = (0..SESSIONS).map(|_| None).collect();
    match Session::spawn(grid.0, grid.1, &home) {
        Ok(s) => sessions[0] = Some(s),
        Err(e) => {
            // No PTY = the terminal cannot run; exit cleanly with a
            // message instead of a panic backtrace.
            eprintln!("nacelle-desktop: cannot start the shell (PTY): {e}");
            return;
        }
    }
    let mut active: usize = 0;

    // Widget instances, one slot per registry entry — all empty until
    // a board actually places the widget (sync_widgets!). A clock costs
    // nothing to hold, but the registry is open-ended and a future
    // widget may be a whole browser engine: what is not on any board
    // must not be running, and that has to hold from the start.
    let mut widget_inst: Vec<Option<Box<dyn widgets::Widget>>> =
        (0..widgets::panel_count()).map(|_| None).collect();

    let mut settings = widgets::settings::Settings::new();
    settings.color_enabled = color_mgr.is_some();
    // Frosted-glass preferences: the radius goes to the renderer, the
    // opacity into the tint of every glass quad drawn this frame.
    let (blur_radius, blur_opacity) = config::blur_prefs();
    gfx.set_blur_radius(blur_radius);
    // The theme's lens: glyph-coverage exponent and the blur pyramid's clear.
    // Re-applied on every configuration change beside the rest of the theme.
    macro_rules! apply_lens {
        () => {{
            let t = nacelle::theme::resolved();
            if let Some(id) = nacelle::theme::id("render.text_gamma") {
                gfx.set_text_gamma(t.px(id));
            }
        }};
    }
    apply_lens!();
    // The glass itself is always fully frosted — RADIUS is the only
    // say over how blurred it is. OPACITY is the user's scale over the
    // alpha of the theme's own wash (elev.fixture.glass.wash): nothing
    // at 0 %, the wash exactly as the theme wrote it at 100 %.
    let mut frost_wash = blur_opacity as f32 / 100.0;
    let mut editor = widgets::editor::Editor::new();
    let mut popup = widgets::popup::Popup::new();
    if let Some(w) = startup_warning {
        nacelle::sound::emit(nacelle::sound::Event::Alert);
        popup.show(w);
    }
    // A pinned screen section that predates the current default
    // arrangement wins over everything the layout engine computes, so
    // the user of this screen sees no part of a changed default — and
    // panels the section does not name land wherever the NEW base puts
    // them, on top of whatever was pinned. Nothing is migrated or
    // rewritten behind the user's back (u1 §5.3): it is told, once,
    // with the way out named. The same line goes to stderr so it
    // survives a headless start.
    if let Some((pinned, placed)) = config::stale_screen_section(&layout_spec, screen) {
        let lname = config::current_layaut_name()
            .unwrap_or_else(|| "default".to_string());
        let msg = format!(
            "Layaut '{lname}' pins {pinned} of {placed} panels for {}x{}@{}. \
             The default arrangement changed; this screen still shows the saved \
             one. Settings \u{2192} Themes \u{2192} Layauts \u{2192} RESET THIS SCREEN.",
            screen.0, screen.1, screen.2
        );
        eprintln!("nacelle-desktop: {msg}");
        nacelle::sound::emit(nacelle::sound::Event::Alert);
        popup.show(msg);
    }

    // The panel holding the terminal view, refreshed from every frame:
    // it is whichever widget reports a CHARACTER GRID, a capability the
    // widget interface declares and nothing else answers. The
    // application resizes the PTY to it, pastes the primary selection
    // into it and opens the terminal menu over it — all without a
    // widget's name, so an installation with a different terminal
    // addon, or none, needs no line of this program changed.
    let mut term_panel: Option<widgets::Panel> = None;

    let mut dl = draw::DrawList::new();

    // ---- the decoration plates (theme::plate, r1 §8 / DECISION M10) ----
    // The theme's static decoration, CPU-baked into TWO screen-sized
    // RGBA images: the BACKDROP plate (traces, grid, starfield, bottom
    // vignette) drawn as one quad under everything else, inside the
    // glass snapshot, and the OVERLAY plate (scanlines, grain, top
    // vignette) drawn as one quad over everything themed — z 70.
    // Registered through the renderer's ordinary image path; rebaked on
    // a WORKER thread when the theme epoch or the surface size changes
    // (measured 5.2 ms at 2560x1440 with aurora's traces, release),
    // never per frame. `None` — the theme turned every layer of that
    // plate off, and the raw run draws no quad for it at all.
    let mut plate_tex: Option<(draw::ImageId, u32, u32)> = None; // backdrop
    let mut overlay_tex: Option<(draw::ImageId, u32, u32)> = None; // overlay
    let mut plate_key: Option<(u32, u32, u32)> = None; // (epoch, w, h) last kicked
    type PlatePair = (Option<nacelle::theme::Plate>, Option<nacelle::theme::Plate>);
    let mut plate_rx: Option<Receiver<PlatePair>> = None;

    let start = Instant::now();
    let mut mods = ModifiersState::empty();
    let mut mouse = (0.0f32, 0.0f32);
    // IME state (F1 §3.2): allowed strictly on TEXT-focus edges, and
    // the only text control wired in this slice is the SAVE AS field.
    // It starts OFF and the terminal KEEPS it off — the §3.2 red-team
    // gate: engaging text-input-v3 (KWin) or XIM (XWayland, gamescope's
    // usual socket) while the shell owns the keyboard at boot can
    // reroute dead-key/compose delivery and break the byte-identical
    // PTY stream the boot session promises. Terminal IME (commit-only,
    // ImePurpose::Terminal) is a later slice, gated on verifying key
    // delivery unchanged under KWin-Wayland, plain X11 AND gamescope's
    // XWayland; until then a terminal composition simply never starts
    // and typing degrades to plain KeyboardInput — which is what
    // gamescope offers anyway (it usually runs no IME at all).
    let mut ime_allowed = false;
    // The last IME cursor area sent, so the anchor is re-sent on
    // change, not sixty times a second.
    let mut ime_area: Option<(i32, i32, i32, i32)> = None;
    // The single pointer-capture path (F1 §5.1): a left press the shell
    // widget's drag(Begin) ACCEPTED routes every CursorMoved to
    // drag(Move) and the release to drag(End). Declined presses fall
    // through to the click/board machinery untouched.
    let mut drag_capture: Option<widgets::Panel> = None;
    // Double/triple click, tracked HERE because a widget cannot see
    // click counts: the count picks the selection kind on Begin.
    let mut click_streak: u32 = 0;
    let mut click_last: Option<(Instant, f32, f32)> = None;

    // The F1 §1 command registry. OVER_GREEDY: the terminal is a greedy
    // control and eats plain chords as bytes — Ctrl+Shift+C/V and the
    // menu key are exactly the escape hatches that must work over it
    // (the §1.4 red-team names Shift+F10/Menu explicitly).
    const CMD_COPY: u32 = 1;
    const CMD_PASTE: u32 = 2;
    const CMD_PASTE_PRIMARY: u32 = 3;
    const CMD_CLEAR_SCROLLBACK: u32 = 4;
    const CMD_NEW_TAB: u32 = 5;
    const CMD_EDIT_LAYOUT: u32 = 6;
    const CMD_OPEN_SETTINGS: u32 = 7;
    const CMD_BOARD_LEFT: u32 = 8;
    const CMD_BOARD_RIGHT: u32 = 9;
    const CMD_INPUT_CUT: u32 = 10;
    const CMD_INPUT_COPY: u32 = 11;
    const CMD_INPUT_PASTE: u32 = 12;
    const CMD_INPUT_SELECT_ALL: u32 = 13;
    const CMD_OPEN_MENU: u32 = 14;
    let shortcuts = {
        use nacelle::focus::{Scope, ShortcutFlags, ShortcutMap};
        let mut m = ShortcutMap::new();
        m.bind(Scope::Global, "ctrl+shift+c", CMD_COPY, ShortcutFlags::OVER_GREEDY);
        m.bind(Scope::Global, "ctrl+shift+v", CMD_PASTE, ShortcutFlags::OVER_GREEDY);
        m.bind(Scope::Global, "shift+f10", CMD_OPEN_MENU, ShortcutFlags::OVER_GREEDY);
        m.bind(Scope::Global, "menu", CMD_OPEN_MENU, ShortcutFlags::OVER_GREEDY);
        // The field chords live in `text_input::key_msg`; they are
        // REGISTERED here as Scope::Focused so the input menu's hints
        // come from the one registry (§4.6: hints from ShortcutMap,
        // never hand-written) — the Global lookups never see them, so
        // the terminal still gets its literal Ctrl+C.
        m.bind(Scope::Focused, "ctrl+x", CMD_INPUT_CUT, ShortcutFlags::NONE);
        m.bind(Scope::Focused, "ctrl+c", CMD_INPUT_COPY, ShortcutFlags::NONE);
        m.bind(Scope::Focused, "ctrl+v", CMD_INPUT_PASTE, ShortcutFlags::NONE);
        m.bind(Scope::Focused, "ctrl+a", CMD_INPUT_SELECT_ALL, ShortcutFlags::NONE);
        m
    };
    // The open context menu — the router's TOP layer (F1 §4.3,
    // LayerId::Menu above everything): while one is up it sees keys
    // and clicks first, and its draw runs last. None = no layer.
    let mut menu: Option<nacelle::object::menu::MenuState> = None;
    // The desktop's one tooltip manager (F2 §8.1). Controls file their
    // requests while drawing — a trimmed table heading, a tab whose
    // label did not fit — and this shows the one the pointer has
    // actually settled on, after it has settled long enough.
    let mut tips = nacelle::object::tooltip::Tooltips::new();
    // The desktop's one focus chain (F1 §1.2), rebuilt every frame
    // from whatever registers while drawing. This slice wires the
    // settings window's controls — the modal layer, so Tab cannot be
    // stolen from the shell: while settings is closed nothing
    // registers, the chain is empty, and every key keeps today's
    // route to the terminal. The focus-visible bit inside keeps the
    // boot frame pixel-identical (no ring until keyboard navigation).
    let mut focus_ctl = nacelle::focus::FocusCtl::new();

    // Re-reads the configuration and applies everything it can change:
    // the theme, the layout with its panel sizes, the sound clips and
    // the two fonts. Every path that alters a setting ends here, so none
    // of them can quietly skip a step — which is exactly what happened
    // while this was written out twice: only one copy reloaded the sound
    // theme, so choosing a new one did nothing until the next restart.
    //
    // A macro rather than a function because all of these are locals of
    // the event loop, and threading eleven &mut through a call would be
    // longer than the body.
    // The boards are the selected layout's: whenever layout_spec is
    // read anew, this derives the in-memory board definitions from it.
    // Boards share the layout's size table — one layout, one table.
    // Widgets live where the boards put them. This walks every board
    // of the selected layout and matches instances to placement:
    // placed but not built — build; built but no longer placed — drop
    // (a plugin's Drop crosses the ABI and frees its instance). This
    // is what makes "not on any board" mean "not running", which a
    // clock never cares about and a future browser widget will.
    macro_rules! sync_widgets {
        () => {{
            let size = window.inner_size();
            let (sw, sh) = (size.width as f32, size.height as f32);
            let present =
                world.present(sw, sh, ui_padding, screen, &nacelle::base::size_table());
            for p in widgets::Panel::all() {
                let i = p.idx();
                if present[i] && widget_inst[i].is_none() {
                    widget_inst[i] = make_widget(p);
                } else if !present[i] && widget_inst[i].is_some() {
                    widget_inst[i] = None;
                }
            }
        }};
    }
    macro_rules! refresh_boards {
        () => {{
            go_queue.clear();
            // The world rebuilds from the layout wholesale: the row's
            // extents, the per-board size tables, the two fixtures that
            // exist whether or not the file names them.
            world.rebuild(layout_spec.clone());
            // A layout with fewer boards than where the user stood:
            // home is the one place that always exists.
            if !(has_board!(cur_board)) {
                cur_board = (0, 0);
            }
            world.set_current(cur_board);
            sync_widgets!();
        }};
    }
    // The definition a position shows — the vertical positions all
    // share the top or bottom board's.
    macro_rules! def_of {
        ($k:expr) => {
            world.def($k)
        };
    }
    macro_rules! cur_def {
        () => {
            def_of!(cur_board)
        };
    }
    macro_rules! has_board {
        ($k:expr) => {
            world.has_board($k)
        };
    }
    // Starts the one-neighbour move to `$t` — the cube sideways, the
    // flat slide up and down.
    macro_rules! step_to {
        ($t:expr) => {{
            let t: config::BoardId = $t;
            let horizontal = t.0 != cur_board.0;
            let sign: i32 = if horizontal {
                if t.0 > cur_board.0 { 1 } else { -1 }
            } else if t.1 > cur_board.1 {
                1
            } else {
                -1
            };
            let full: f32 = if horizontal { 90.0 } else { 1.0 };
            nacelle::sound::emit(nacelle::sound::Event::Snap);
            cube = Some(Cube {
                horizontal,
                a0: 0.0,
                a1: full * sign as f32,
                t0: Instant::now(),
                to: t,
                face_b: t,
            });
        }};
    }
    // Every board that exists, home first.
    macro_rules! all_boards {
        () => {
            world.ids()
        };
    }
    // Every board as it would look at the given size, the current one
    // marked — what the BOARDS view draws. Taken at the size of the
    // screen the settings window is being drawn on, which is why this
    // is a macro over (w, h) rather than the main window's numbers.
    macro_rules! board_thumbs {
        ($w:expr, $h:expr) => {{
            let mut thumbs = Vec::new();
            for k in all_boards!() {
                let def = def_of!(k);
                let lay = outer_layout(def, def.pick(screen), $w, $h, ui_padding);
                let panels = widgets::Panel::all()
                    .into_iter()
                    .filter_map(|pnl| {
                        let r = lay.p(pnl);
                        (r.x < $w).then_some(widgets::PanelSpec {
                            x: r.x / $w * 100.0,
                            y: r.y / $h * 100.0,
                            w: r.w / $w * 100.0,
                            h: r.h / $h * 100.0,
                        })
                    })
                    .collect();
                thumbs.push(widgets::settings::BoardThumb {
                    id: k,
                    current: k == config::board_key(cur_board),
                    panels,
                });
            }
            thumbs
        }};
    }
    // SAVE while standing on a board: the board's panels go into the
    // selected layout's file, and the world is re-read from it.
    macro_rules! save_board_cur {
        () => {{
            let name = config::current_layaut_name()
                .unwrap_or_else(|| "default".to_string());
            match config::set_board_in_layaut(
                &name,
                config::board_key(cur_board),
                &editor.spec(),
            ) {
                Ok(()) => {
                    nacelle::sound::emit(nacelle::sound::Event::Save);
                    apply_config!(
                        layout_spec, active_ov, popup, audio, fonts,
                        font_scale, ui_font_scale, last_term_key, last_ui_key,
                        window
                    );
                    // A finished save leaves the editor, on every board
                    // — the same ending HOME's own save has. Only the
                    // board differed, and the difference was invisible
                    // and therefore wrong: the user pressed SAVE and
                    // stayed in a mode they thought they had left.
                    editor.stop();
                }
                Err(e) => {
                    // A save that failed keeps the editor open: the
                    // arrangement is still the user's to retry, and
                    // dropping them out of the mode would throw it away.
                    nacelle::sound::emit(nacelle::sound::Event::Error);
                    popup.show(format!("Cannot save the board: {e}"));
                }
            }
        }};
    }
    macro_rules! apply_config {
        ($layout_spec:ident, $active_ov:ident, $popup:ident,
         $audio:ident, $fonts:ident, $font_scale:ident, $ui_font_scale:ident,
         $last_term_key:ident, $last_ui_key:ident, $window:ident) => {{
            let (new_cfg, warn) = config::resolve();
            // Sizes travel with the layout, so a new layout brings its own.
            nacelle::base::set_panel_sizes(&new_cfg.layout.sizes);
            $layout_spec = new_cfg.layout;
            $active_ov = $layout_spec.pick(screen).cloned();
            // A new look or sound set means new clips.
            if let (Some(a), Some(dir)) = ($audio.as_mut(), config::active_sounds_dir()) {
                a.load_theme(&dir);
            }
            if let Some(w) = warn {
                $popup.show(w);
            }
            let (tscale, tfam, twgt) = config::term_font_prefs();
            let (uscale, ufam, uwgt) = config::ui_font_prefs();
            $font_scale = tscale;
            $ui_font_scale = uscale;
            let tkey = (
                tfam.clone().unwrap_or_default(),
                twgt.clone().unwrap_or_default(),
            );
            if tkey != $last_term_key {
                $last_term_key = tkey;
                if tfam.is_none() && twgt.is_none() {
                    $fonts.set_mono(font::load_default_mono());
                } else if let Some(f) =
                    font::load_variant_for(tfam.as_deref(), twgt.as_deref(), false)
                {
                    $fonts.set_mono(f);
                }
            }
            let ukey = (
                ufam.clone().unwrap_or_default(),
                uwgt.clone().unwrap_or_default(),
            );
            if ukey != $last_ui_key {
                $last_ui_key = ukey;
                if ufam.is_none() && uwgt.is_none() {
                    $fonts.set_ui(font::load_default_ui());
                } else if let Some(f) =
                    font::load_variant_for(ufam.as_deref(), uwgt.as_deref(), true)
                {
                    $fonts.set_ui(f);
                }
            }
            apply_lens!();
            refresh_boards!();
        }};
    }

    // Routes one drag phase to a panel's widget, in the content box its
    // last draw used — the same rect discipline as click and wheel.
    macro_rules! widget_drag {
        ($panel:expr, $phase:expr, $x:expr, $y:expr) => {{
            let panel = $panel;
            let size = window.inner_size();
            let layout = outer_layout(
                cur_def!(),
                cur_def!().pick(screen),
                size.width as f32,
                size.height as f32,
                ui_padding,
            )
            .padded(ui_padding);
            let r = panel_content
                .get(panel.idx())
                .copied()
                .flatten()
                .unwrap_or(layout.p(panel));
            let occupied: Vec<bool> =
                (0..SESSIONS).map(|i| sessions[i].is_some()).collect();
            let snap = sys.lock().unwrap().clone();
            let host = widgets::Host {
                snap: &snap,
                term: sessions[active].as_ref().map(|s| &s.term),
                tabs: &occupied,
                tab_active: active,
                shell_cwd: None,
                t: start.elapsed().as_secs_f64(),
                window: (size.width as f32, size.height as f32),
            };
            widget_inst
                .get_mut(panel.idx())
                .and_then(|w| w.as_mut())
                .map(|w| w.drag($phase, $x, $y, r, &host))
                .unwrap_or(widgets::Action::None)
        }};
    }
    // Applies a widget's TermSelect to the active session. The host
    // resolves `base + row` — the view the widget DREW, echoed through
    // term_view, never the live view_offset — and owns the click-count
    // kinds, because a widget cannot see double clicks. Copy-on-select
    // stores PRIMARY on End only, never per motion: a per-move store
    // would spam the compositor with ownership changes every frame.
    macro_rules! apply_term_select {
        ($op:expr, $col:expr, $row:expr, $base:expr) => {{
            if let Some(s) = sessions[active].as_mut() {
                let line: u64 = $base + $row as u64;
                match $op {
                    widgets::SelectOp::Begin(kind) => {
                        let kind = if kind == term::SelKind::Cells {
                            match click_streak {
                                2 => term::SelKind::Words,
                                n if n >= 3 => term::SelKind::Lines,
                                _ => term::SelKind::Cells,
                            }
                        } else {
                            kind
                        };
                        s.term.selection_begin(line, $col, kind);
                    }
                    widgets::SelectOp::Extend => s.term.selection_extend(line, $col),
                    widgets::SelectOp::End => {
                        s.term.selection_extend(line, $col);
                        // A click that never moved is a click, not a
                        // one-cell copy: it clears the old selection.
                        let trivial = s.term.selection.map_or(true, |sel| {
                            sel.kind == term::SelKind::Cells && sel.anchor == sel.head
                        });
                        if trivial {
                            s.term.selection_clear();
                        } else if let Some(text) = s.term.selection_text() {
                            if !text.is_empty() {
                                nacelle::clipboard::store(
                                    nacelle::clipboard::Board::Primary,
                                    &text,
                                );
                            }
                        }
                    }
                }
            }
        }};
    }
    // Pastes into the active session — the one paste path for every
    // gesture (Ctrl+Shift+V, middle click, a widget's PastePrimary).
    macro_rules! paste_into_active {
        ($board:expr) => {{
            if let Some(text) = nacelle::clipboard::load($board) {
                if let Some(s) = sessions[active].as_mut() {
                    s.paste(&text);
                }
            }
        }};
    }
    // Opens a context menu at a point — the one gate every opener runs
    // through: mid-ride the boards are nowhere in particular, so a menu
    // over moving rects must be impossible (F1 §4.6's cube rule).
    macro_rules! open_menu_at {
        ($entries:expr, $x:expr, $y:expr) => {{
            if cube.is_none() {
                menu = Some(nacelle::object::menu::MenuState::open_at(
                    $entries,
                    $x,
                    $y,
                    start.elapsed().as_secs_f64(),
                ));
            }
        }};
    }
    // The three F1 §4.2 menus. Entries are built fresh at open, so a
    // disabled row reflects that moment (a menu is a snapshot); hints
    // come from the shortcut registry and nowhere else.
    macro_rules! terminal_menu_entries {
        () => {{
            use nacelle::object::menu::{MenuEntry, MenuItem};
            let has_sel = sessions[active]
                .as_ref()
                .and_then(|s| s.term.selection_text())
                .map_or(false, |t| !t.is_empty());
            let free_tab = sessions.iter().any(|s| s.is_none());
            vec![
                MenuEntry::Item(
                    MenuItem::new("COPY", CMD_COPY)
                        .with_hint(shortcuts.hint(CMD_COPY))
                        .with_disabled(!has_sel),
                ),
                MenuEntry::Item(
                    MenuItem::new("PASTE", CMD_PASTE).with_hint(shortcuts.hint(CMD_PASTE)),
                ),
                MenuEntry::Item(
                    MenuItem::new("PASTE SELECTION", CMD_PASTE_PRIMARY)
                        .with_hint(shortcuts.hint(CMD_PASTE_PRIMARY)),
                ),
                MenuEntry::Rule,
                MenuEntry::Item(
                    MenuItem::new("CLEAR SCROLLBACK", CMD_CLEAR_SCROLLBACK)
                        .with_hint(shortcuts.hint(CMD_CLEAR_SCROLLBACK)),
                ),
                MenuEntry::Rule,
                MenuEntry::Item(
                    MenuItem::new("NEW TAB", CMD_NEW_TAB)
                        .with_hint(shortcuts.hint(CMD_NEW_TAB))
                        .with_disabled(!free_tab),
                ),
            ]
        }};
    }
    macro_rules! panel_menu_entries {
        () => {{
            use nacelle::object::menu::{MenuEntry, MenuItem};
            // Sideways only along the row — exactly the pan gesture's
            // reach; the vertical fixtures are not "left or right".
            let on_arm = cur_board.1 == 0;
            let left_ok = on_arm && has_board!((cur_board.0 - 1, cur_board.1));
            let right_ok = on_arm && has_board!((cur_board.0 + 1, cur_board.1));
            vec![
                MenuEntry::Item(
                    MenuItem::new("EDIT LAYOUT", CMD_EDIT_LAYOUT)
                        .with_hint(shortcuts.hint(CMD_EDIT_LAYOUT)),
                ),
                MenuEntry::Item(
                    MenuItem::new("SETTINGS", CMD_OPEN_SETTINGS)
                        .with_hint(shortcuts.hint(CMD_OPEN_SETTINGS)),
                ),
                MenuEntry::Rule,
                MenuEntry::Item(
                    MenuItem::new("BOARD LEFT", CMD_BOARD_LEFT)
                        .with_hint(shortcuts.hint(CMD_BOARD_LEFT))
                        .with_disabled(!left_ok),
                ),
                MenuEntry::Item(
                    MenuItem::new("BOARD RIGHT", CMD_BOARD_RIGHT)
                        .with_hint(shortcuts.hint(CMD_BOARD_RIGHT))
                        .with_disabled(!right_ok),
                ),
            ]
        }};
    }
    macro_rules! input_menu_entries {
        () => {{
            use nacelle::object::menu::{MenuEntry, MenuItem};
            let (has_sel, has_text) = editor
                .naming
                .as_ref()
                .map_or((false, false), |m| {
                    (m.selection().is_some(), !m.value().is_empty())
                });
            vec![
                MenuEntry::Item(
                    MenuItem::new("CUT", CMD_INPUT_CUT)
                        .with_hint(shortcuts.hint(CMD_INPUT_CUT))
                        .with_disabled(!has_sel),
                ),
                MenuEntry::Item(
                    MenuItem::new("COPY", CMD_INPUT_COPY)
                        .with_hint(shortcuts.hint(CMD_INPUT_COPY))
                        .with_disabled(!has_sel),
                ),
                MenuEntry::Item(
                    MenuItem::new("PASTE", CMD_INPUT_PASTE)
                        .with_hint(shortcuts.hint(CMD_INPUT_PASTE)),
                ),
                MenuEntry::Rule,
                MenuEntry::Item(
                    MenuItem::new("SELECT ALL", CMD_INPUT_SELECT_ALL)
                        .with_hint(shortcuts.hint(CMD_INPUT_SELECT_ALL))
                        .with_disabled(!has_text),
                ),
            ]
        }};
    }
    // Enters the layout editor with the current panel rectangles — the
    // EDIT GRID button's body, shared with the panel context menu so
    // the two ways in cannot drift apart.
    macro_rules! enter_editor {
        () => {{
            let size = window.inner_size();
            let (snap, cols, rows, pad) = config::grid_prefs();
            // The editor edits the OUTER panel rects.
            let outer = outer_layout(
                cur_def!(),
                cur_def!().pick(screen),
                size.width as f32,
                size.height as f32,
                ui_padding,
            );
            // Widgets living on other boards are not offered here: a
            // widget exists once, somewhere. Neither are widgets of
            // another category — the board being edited decides which
            // kind it takes (ordinary boards, APPGRID, or SEARCH AND
            // AI).
            let mut blocked = vec![false; widgets::panel_count()];
            let here = match config::board_key(cur_board) {
                (0, y) if y < 0 => nacelle::base::WidgetCategory::SearchAi,
                (0, y) if y > 0 => nacelle::base::WidgetCategory::Appgrid,
                _ => nacelle::base::WidgetCategory::Board,
            };
            for pnl in widgets::Panel::all() {
                if pnl.category() != here {
                    blocked[pnl.idx()] = true;
                }
            }
            for k in all_boards!() {
                if k == config::board_key(cur_board) {
                    continue;
                }
                let def = def_of!(k);
                let lay = outer_layout(
                    def,
                    def.pick(screen),
                    size.width as f32,
                    size.height as f32,
                    ui_padding,
                );
                for pnl in widgets::Panel::all() {
                    if lay.p(pnl).x < size.width as f32 {
                        blocked[pnl.idx()] = true;
                    }
                }
            }
            editor.start(
                &outer,
                size.width as f32,
                size.height as f32,
                snap,
                cols,
                rows,
                pad as f32,
                blocked,
            );
        }};
    }
    // What a picked row runs — the same commands the shortcut registry
    // names, so a chord and a menu row cannot drift apart.
    macro_rules! run_menu_cmd {
        ($cmd:expr) => {{
            match $cmd {
                CMD_COPY => {
                    let text = sessions[active]
                        .as_ref()
                        .and_then(|s| s.term.selection_text())
                        .unwrap_or_default();
                    if !text.is_empty() {
                        nacelle::clipboard::store(nacelle::clipboard::Board::Clipboard, &text);
                    }
                }
                CMD_PASTE => paste_into_active!(nacelle::clipboard::Board::Clipboard),
                CMD_PASTE_PRIMARY => {
                    paste_into_active!(nacelle::clipboard::Board::Primary)
                }
                CMD_CLEAR_SCROLLBACK => {
                    if let Some(s) = sessions[active].as_mut() {
                        // The emulator's own ESC[3J semantics: the
                        // scrollback goes, the view snaps to live, and
                        // the selection drops (its line ids may name
                        // exactly the lines being forgotten).
                        s.term.scrollback.clear();
                        s.term.view_offset = 0;
                        s.term.selection_clear();
                    }
                }
                CMD_NEW_TAB => {
                    // The first free slot, started where the active
                    // shell is — the tab widget's own click behaviour.
                    if let Some(i) = (0..sessions.len()).find(|i| sessions[*i].is_none()) {
                        let start_dir = sessions[active]
                            .as_mut()
                            .and_then(|s| s.cwd())
                            .unwrap_or_else(|| home.clone());
                        match Session::spawn(grid.0, grid.1, &start_dir) {
                            Ok(s) => {
                                sessions[i] = Some(s);
                                active = i;
                            }
                            Err(e) => {
                                eprintln!("nacelle-desktop: cannot open PTY: {e}")
                            }
                        }
                    }
                }
                CMD_EDIT_LAYOUT => {
                    if !editor.active {
                        enter_editor!();
                    }
                }
                CMD_OPEN_SETTINGS => settings.show(),
                CMD_BOARD_LEFT | CMD_BOARD_RIGHT => {
                    let target = if $cmd == CMD_BOARD_LEFT {
                        (cur_board.0 - 1, cur_board.1)
                    } else {
                        (cur_board.0 + 1, cur_board.1)
                    };
                    if cur_board.1 == 0 && cube.is_none() && has_board!(target) {
                        step_to!(target);
                    }
                }
                CMD_INPUT_CUT | CMD_INPUT_COPY | CMD_INPUT_PASTE | CMD_INPUT_SELECT_ALL => {
                    use nacelle::object::text_input::{InputEdited, InputMsg};
                    let msg = match $cmd {
                        CMD_INPUT_CUT => InputMsg::Cut,
                        CMD_INPUT_COPY => InputMsg::Copy,
                        CMD_INPUT_PASTE => InputMsg::Paste,
                        _ => InputMsg::SelectAll,
                    };
                    let out = editor
                        .naming
                        .as_mut()
                        .map(|m| m.apply(msg))
                        .unwrap_or(InputEdited::None);
                    match out {
                        // The model answers with INTENTS; resolving
                        // them here mirrors the keyboard path exactly.
                        InputEdited::CopyRequest { text, .. } => {
                            nacelle::clipboard::store(
                                nacelle::clipboard::Board::Clipboard,
                                &text,
                            );
                        }
                        InputEdited::PasteRequest => {
                            if let Some(text) = nacelle::clipboard::load(
                                nacelle::clipboard::Board::Clipboard,
                            ) {
                                let text: String = text
                                    .to_lowercase()
                                    .chars()
                                    .filter(|&c| {
                                        widgets::editor::Editor::layaut_name_char(c)
                                    })
                                    .collect();
                                if let Some(m) = editor.naming.as_mut() {
                                    m.apply(InputMsg::Insert(text));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }};
    }
    // Everything a settings interaction may have ASKED the application
    // for — one body behind both ways of pressing a control (mouse
    // click and the F1 §1.5 Enter/Space), so the keyboard cannot reach
    // a control whose consequences only the mouse path knows.
    macro_rules! settings_after {
        () => {{
            // RESET THIS SCREEN (the LAYAUTS view): the window cannot
            // clear the section itself — only the application knows
            // which screen this is — so it asks, like the boards do.
            if settings.reset_screen {
                settings.reset_screen = false;
                let name = config::current_layaut_name()
                    .unwrap_or_else(|| "default".to_string());
                match config::clear_screen_section(&name, screen) {
                    Ok(()) => {
                        nacelle::sound::emit(nacelle::sound::Event::Save);
                        popup.show(format!(
                            "Cleared the {}x{}@{} section of layaut '{}'",
                            screen.0, screen.1, screen.2, name
                        ));
                    }
                    Err(e) => {
                        nacelle::sound::emit(nacelle::sound::Event::Error);
                        popup.show(format!("Cannot reset this screen: {e}"));
                    }
                }
                apply_config!(
                    layout_spec, active_ov, popup, audio, fonts,
                    font_scale, ui_font_scale, last_term_key, last_ui_key,
                    window
                );
            }
            // The BOARDS view asks; the boards are the
            // application's, so the answers live here.
            if let Some(act) = settings.board_action.take() {
                use widgets::settings::BoardAction;
                match act {
                    BoardAction::Go(k) => {
                        // The whole point of going from
                        // here: the window stays open, so
                        // no board needs its own control
                        // panel to come back.
                        if !editor.active
                            && cube.is_none()
                            && k != config::board_key(cur_board)
                            && has_board!(k)
                        {
                            // A distant board is walked one
                            // neighbour at a time, so the
                            // move animates through every
                            // board between — the cube
                            // sideways, the slide up and
                            // down.
                            let mut steps: Vec<config::BoardId> = Vec::new();
                            let mut at = cur_board;
                            if k.1 != 0 {
                                // Top or bottom: y is the
                                // whole journey, through
                                // the row if coming from
                                // the other one.
                                if at.1 == -k.1 {
                                    at = (at.0, 0);
                                    steps.push(at);
                                }
                                steps.push((at.0, k.1));
                            } else {
                                if at.1 != 0 {
                                    at = (at.0, 0);
                                    steps.push(at);
                                }
                                while at.0 != k.0 {
                                    at.0 += if k.0 > at.0 { 1 } else { -1 };
                                    steps.push(at);
                                }
                            }
                            steps.reverse();
                            if let Some(first) = steps.pop() {
                                step_to!(first);
                                go_queue = steps;
                            }
                        }
                    }
                    BoardAction::Add(side) => {
                        let name = config::current_layaut_name()
                            .unwrap_or_else(|| "default".to_string());
                        if let Err(e) = config::add_board_in_layaut(&name, side) {
                            popup.show(format!("Cannot add a board: {e}"));
                        }
                        apply_config!(
                            layout_spec, active_ov, popup,
                            audio, fonts, font_scale, ui_font_scale,
                            last_term_key, last_ui_key, window
                        );
                    }
                    BoardAction::Del(k) => {
                        // Only the row shrinks; home and
                        // the top and bottom boards are
                        // fixtures.
                        if k != (0, 0) && k.1 == 0 && cube.is_none() {
                            let name = config::current_layaut_name()
                                .unwrap_or_else(|| "default".to_string());
                            if let Err(e) = config::remove_board_in_layaut(&name, k) {
                                popup.show(format!("Cannot remove the board: {e}"));
                            }
                            // Whoever stood on a moved board
                            // follows it; whoever stood on
                            // the removed one — or came up
                            // or down from it — lands over
                            // home.
                            if cur_board.0 == k.0 {
                                cur_board.0 = 0;
                            } else if k.0 > 0 && cur_board.0 > k.0 {
                                cur_board.0 -= 1;
                            } else if k.0 < 0 && cur_board.0 < k.0 {
                                cur_board.0 += 1;
                            }
                            apply_config!(
                                layout_spec, active_ov,
                                popup, audio, fonts, font_scale,
                                ui_font_scale, last_term_key,
                                last_ui_key, window
                            );
                        }
                    }
                }
            }
            // EDIT GRID: hide settings, enter the editor
            // with the current panel rectangles — the
            // body is shared with the panel context
            // menu's EDIT LAYOUT row.
            if settings.edit_requested {
                settings.edit_requested = false;
                if !editor.active {
                    enter_editor!();
                }
                // With the editor already running the window
                // simply hides — back to the grid.
            }
            if editor.active {
                let size = window.inner_size();
                let (snap, cols, rows, pad) = config::grid_prefs();
                editor.sync_prefs(
                    snap,
                    cols,
                    rows,
                    pad as f32,
                    size.width as f32,
                    size.height as f32,
                );
            }
        }};
    }

    refresh_boards!();
    eprintln!(
        "nacelle-desktop: {} of {} registered widgets placed on boards",
        widget_inst.iter().filter(|w| w.is_some()).count(),
        widgets::panel_count()
    );

    // How often the world is redrawn. Nothing here is a game: the
    // clock ticks once a second, telemetry once a second, the terminal
    // as fast as a person types. Drawing at whatever the display can
    // take — 240 Hz on this machine — spent most of a core rebuilding
    // an image that had not changed. Sixty is smooth for the board
    // animations (a transition is eighteen frames) and costs a quarter
    // of that.
    const FRAME: std::time::Duration = std::time::Duration::from_nanos(1_000_000_000 / 60);
    let mut next_frame = Instant::now();
    // When the last frame actually went out. The pace cannot be kept by
    // only asking politely: the display server asks for a redraw of its
    // own whenever it exposes the window, and dragging a framed window
    // over the desktop exposes it hundreds of times a second. Each of
    // those was rebuilding the entire world — the loop ran at eight
    // times its cap while a window was being waved about.
    let mut last_render = Instant::now() - FRAME;

    event_loop
        .run(move |event, elwt| {
            match event {
                // A hall's window first: its whole event surface is a
                // resize, a redraw and a close it politely declines —
                // the main window's machinery below never sees it.
                // The KEYS are the exception, and deliberately so: a
                // click in a hall gives that window the compositor's
                // keyboard focus, so every keystroke would arrive here
                // and be swallowed — the settings window could be
                // clicked but not typed at or walked with Tab. Keyboard
                // events fall through to the main window's arm below
                // and are handled exactly as they always were, because
                // the state they act on is one and the same wherever it
                // is drawn.
                Event::WindowEvent { window_id, event }
                    if window_id != window.id()
                        && !matches!(
                            event,
                            WindowEvent::KeyboardInput { .. }
                                | WindowEvent::ModifiersChanged(_)
                                | WindowEvent::Ime(_)
                        ) =>
                {
                    if let Some(i) =
                        salas.iter().position(|s| s.window.id() == window_id)
                    {
                        match event {
                            WindowEvent::Resized(_)
                            | WindowEvent::ScaleFactorChanged { .. } => {
                                salas[i].resize()
                            }
                            // The pointer arriving here is what moves the
                            // settings window to this screen: it follows
                            // the hand, and only the hand — the keyboard
                            // keeps talking to the main window, because
                            // the window's STATE is one and the same
                            // wherever it is drawn.
                            //
                            // The layout editor is the deliberate
                            // exception, and this is NOT an oversight:
                            // a hall SHOWS edit mode (the grid and the
                            // rectangles, drawn for its own geometry)
                            // but never receives it. Dragging a panel is
                            // done on the main screen; a hall is a
                            // preview of the desktop, and routing the
                            // pointer into the editor from a second
                            // screen would let one editor be dragged
                            // from two places at once against two
                            // different pixel geometries.
                            WindowEvent::CursorMoved { position, .. } => {
                                sala_mouse = (position.x as f32, position.y as f32);
                                settings_host = Some(i);
                                if settings.open {
                                    settings.drag(sala_mouse.0);
                                    let pointer =
                                        settings.hover(sala_mouse.0, sala_mouse.1);
                                    salas[i].window.set_cursor_icon(if pointer {
                                        CursorIcon::Pointer
                                    } else {
                                        CursorIcon::Default
                                    });
                                }
                            }
                            WindowEvent::MouseInput {
                                state: ElementState::Pressed,
                                button: MouseButton::Left,
                                ..
                            } if settings.open => {
                                let size = salas[i].window.inner_size();
                                if settings.click(
                                    sala_mouse.0,
                                    sala_mouse.1,
                                    size.width as f32,
                                    size.height as f32,
                                    Some(&mut focus_ctl),
                                ) {
                                    apply_config!(
                                        layout_spec, active_ov, popup, audio, fonts,
                                        font_scale, ui_font_scale, last_term_key,
                                        last_ui_key, window
                                    );
                                }
                            }
                            WindowEvent::MouseInput {
                                state: ElementState::Released,
                                button: MouseButton::Left,
                                ..
                            } if settings.open => {
                                if settings.release() {
                                    apply_config!(
                                        layout_spec, active_ov, popup, audio, fonts,
                                        font_scale, ui_font_scale, last_term_key,
                                        last_ui_key, window
                                    );
                                }
                            }
                            WindowEvent::RedrawRequested => {
                                let size = salas[i].window.inner_size();
                                let (sw, sh) =
                                    (size.width as f32, size.height as f32);
                                // A hall is a CLONE of the main screen,
                                // empty: the board being stood on, its
                                // rectangles solved for the HALL's size
                                // and the hall's own screen key, and an
                                // empty container where the main screen
                                // has a widget. Same source as the main
                                // frame — the editor's rectangles while
                                // it is open, the board's otherwise —
                                // so the two never disagree. A
                                // content-sized panel is solved against
                                // the intrinsic sizes the MAIN screen
                                // measured, which is the only measure
                                // there is: the widgets that answer for
                                // them run there.
                                let hall = salas[i].screen;
                                let layout = (!booting).then(|| {
                                    if editor.active {
                                        editor.layout(sw, sh)
                                    } else {
                                        let def = cur_def!();
                                        outer_layout(
                                            def,
                                            def.pick(hall),
                                            sw,
                                            sh,
                                            ui_padding,
                                        )
                                    }
                                    .padded(ui_padding)
                                });
                                let hosting =
                                    settings_host == Some(i) && settings.open;
                                if hosting {
                                    settings.boards = board_thumbs!(sw, sh);
                                }
                                let t = start.elapsed().as_secs_f64();
                                let (m, tfs, ufs) =
                                    (sala_mouse, font_scale, ui_font_scale);
                                // Read-only here: a hall never edits.
                                let ed = editor.active.then_some(&editor);
                                let (settings_ref, focus_ref) =
                                    (&mut settings, &mut focus_ctl);
                                salas[i].draw_hosted(&mut fonts, |dl, w, h, fonts| {
                                    let mut ctx = widgets::Ctx {
                                        dl,
                                        fonts,
                                        w,
                                        h,
                                        t,
                                        mouse: m,
                                        term_font_scale: tfs,
                                        ui_font_scale: ufs,
                                        panel_scale: 1.0,
                                        // Only the hosted settings
                                        // window has controls to focus;
                                        // the empty frames and the
                                        // editor's preview register
                                        // nothing, so a hall without it
                                        // leaves the chain alone.
                                        focus: hosting.then_some(focus_ref),
                                        // A hall draws its own list, and
                                        // one manager cannot answer to
                                        // two windows at once: nothing
                                        // drawn here files requests.
                                        tips: None,
                                    };
                                    if let Some(layout) = &layout {
                                        sala::draw_empty_board(&mut ctx, layout);
                                        // Edit mode is visible on every
                                        // screen; it is operated on one.
                                        if let Some(ed) = ed {
                                            ed.draw_preview(&mut ctx);
                                        }
                                    }
                                    if hosting {
                                        settings_ref.draw(&mut ctx);
                                    }
                                });
                            }
                            // A hall is part of the desktop: closing it
                            // closes nothing.
                            WindowEvent::CloseRequested => eprintln!(
                                "nacelle-desktop: sala close request ignored"
                            ),
                            _ => {}
                        }
                    }
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        eprintln!("nacelle-desktop: compositor requested window close");
                        elwt.exit();
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        gfx.resize();
                        screen = screen_key(&window);
                        active_ov = layout_spec.pick(screen).cloned();
                        // Re-bake the u-derived lengths for the new
                        // height (§2.2 step 4: on resize, never per
                        // frame). A height that lands on the same u is
                        // deduplicated inside.
                        nacelle::theme::set_viewport(
                            window.inner_size().height as f32,
                            1.0,
                        );
                    }
                    WindowEvent::ModifiersChanged(m) => mods = m.state(),
                    WindowEvent::CursorMoved { position, .. } => {
                        mouse = (position.x as f32, position.y as f32);
                        // The hand came back to the main screen, and the
                        // settings window comes with it.
                        settings_host = None;
                        // An accepted drag capture owns the pointer:
                        // every motion goes to the widget as drag(Move)
                        // and nothing below (board pan, editor hover)
                        // sees it — the single capture path.
                        if let Some(panel) = drag_capture {
                            if let widgets::Action::TermSelect { op, col, row, base } =
                                widget_drag!(panel, widgets::DragPhase::Move, mouse.0, mouse.1)
                            {
                                apply_term_select!(op, col, row, base);
                            }
                            return;
                        }
                        // A held button that travels sideways becomes a
                        // board drag; the click it started as is then
                        // never delivered.
                        if let Some((px0, py0)) = press_at {
                            if pan.is_none() && cube.is_none() {
                                let size = window.inner_size();
                                let (dx, dy) = (mouse.0 - px0, mouse.1 - py0);
                                let th = (size.width as f32 * 0.02).max(20.0);
                                // The axis is decided once, by whichever
                                // way the hand went first, and the drag
                                // stays on it.
                                if dx.abs() > th && dx.abs() > dy.abs() {
                                    pan = Some((true, 0.0));
                                } else if dy.abs() > th && dy.abs() > dx.abs() {
                                    pan = Some((false, 0.0));
                                }
                            }
                            if let Some((horizontal, _)) = pan {
                                let size = window.inner_size();
                                let (w, h) =
                                    (size.width as f32, size.height as f32);
                                // How far the hand travels for a full move,
                                // and how rubbery the row's ends answer it
                                // — material properties, so the theme's.
                                static GESTURE: OnceLock<TokenId> = OnceLock::new();
                                static R_GAIN: OnceLock<TokenId> = OnceLock::new();
                                static R_MAX: OnceLock<TokenId> = OnceLock::new();
                                let thm = nacelle::theme::resolved();
                                let gest = thm
                                    .px(tok(&GESTURE, "motion.board_ride.gesture_frac"));
                                let gain =
                                    thm.px(tok(&R_GAIN, "motion.board_ride.rubber_gain"));
                                let rmax =
                                    thm.px(tok(&R_MAX, "motion.board_ride.rubber_max"));
                                if horizontal {
                                    // Dragging left turns to the right-hand
                                    // neighbour; gesture_frac of the window
                                    // is a full quarter turn. Sideways only
                                    // from the horizontal arm.
                                    let raw = -(mouse.0 - px0) / (w * gest) * 90.0;
                                    let target = (
                                        cur_board.0 + if raw > 0.0 { 1 } else { -1 },
                                        cur_board.1,
                                    );
                                    let a = if cur_board.1 == 0 && has_board!(target)
                                    {
                                        raw.clamp(-90.0, 90.0)
                                    } else {
                                        // No board that way: a short
                                        // rubbery give, so the edge
                                        // answers the hand without
                                        // pretending to go anywhere.
                                        (raw * gain).clamp(-90.0 * rmax, 90.0 * rmax)
                                    };
                                    pan = Some((true, a));
                                } else {
                                    // Up and down the world slides flat;
                                    // dragging up goes to the board below.
                                    // The top and bottom boards sit above
                                    // and below every board on the row,
                                    // so this works from anywhere.
                                    let raw = -(mouse.1 - py0) / (h * gest);
                                    let target = (
                                        cur_board.0,
                                        cur_board.1 + if raw > 0.0 { 1 } else { -1 },
                                    );
                                    let f = if has_board!(target) {
                                        raw.clamp(-1.0, 1.0)
                                    } else {
                                        (raw * gain).clamp(-rmax, rmax)
                                    };
                                    pan = Some((false, f));
                                }
                                return;
                            }
                        }
                        if editor.active && !settings.open {
                            let size = window.inner_size();
                            let (fw, fh) = (size.width as f32, size.height as f32);
                            editor.mouse_move(mouse.0, mouse.1, fw, fh);
                            // Move/resize cursors over the panels.
                            use widgets::editor::CursorKind;
                            window.set_cursor_icon(
                                match editor.cursor_at(mouse.0, mouse.1, fw, fh) {
                                    CursorKind::Move => CursorIcon::Grab,
                                    CursorKind::Ew => CursorIcon::EwResize,
                                    CursorKind::Ns => CursorIcon::NsResize,
                                    CursorKind::Nwse => CursorIcon::NwseResize,
                                    CursorKind::Nesw => CursorIcon::NeswResize,
                                    CursorKind::Normal => CursorIcon::Default,
                                },
                            );
                            return;
                        }
                        if settings.open {
                            settings.drag(mouse.0);
                        }
                        // Pointer cursor over a widget's own controls.
                        let size = window.inner_size();
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        let pointer = if settings.open {
                            settings.hover(mouse.0, mouse.1)
                        } else {
                            // The widget under the pointer is the only
                            // one that knows where its controls are, so
                            // it is asked — in the same content box it
                            // drew in and will be clicked in. The
                            // application holds no copy of anybody's
                            // geometry and needs no widget's name.
                            let win = (size.width as f32, size.height as f32);
                            widgets::Panel::all()
                                .into_iter()
                                .find(|p| layout.p(*p).contains(mouse.0, mouse.1))
                                .is_some_and(|p| {
                                    let r = panel_content
                                        .get(p.idx())
                                        .copied()
                                        .flatten()
                                        .unwrap_or(layout.p(p));
                                    widget_inst
                                        .get_mut(p.idx())
                                        .and_then(|w| w.as_mut())
                                        .is_some_and(|w| {
                                            w.pointer(mouse.0, mouse.1, r, win)
                                        })
                                })
                        };
                        window.set_cursor_icon(if pointer {
                            CursorIcon::Pointer
                        } else {
                            CursorIcon::Default
                        });
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        // An open menu is a grab: the wheel must not
                        // scroll whatever sits under it.
                        if menu.is_some() {
                            return;
                        }
                        if editor.active {
                            return;
                        }
                        let dy = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.0,
                        };
                        let size = window.inner_size();
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        let hit = widgets::Panel::all()
                            .into_iter()
                            .find(|p| layout.p(*p).contains(mouse.0, mouse.1));
                        if let Some(panel) = hit {
                            // The rect the widget is answered in is the
                            // CONTENT BOX its last draw used (u2 §4.1),
                            // never the panel rect — the container's
                            // band and padding are the host's, not the
                            // widget's.
                            let r = panel_content
                                .get(panel.idx())
                                .copied()
                                .flatten()
                                .unwrap_or(layout.p(panel));
                            let occupied: Vec<bool> =
                                (0..SESSIONS).map(|i| sessions[i].is_some()).collect();
                            let action = {
                                let host = widgets::Host {
                                    snap: &sys.lock().unwrap().clone(),
                                    term: sessions[active].as_ref().map(|s| &s.term),
                                    tabs: &occupied,
                                    tab_active: active,
                                    shell_cwd: None,
                                    t: start.elapsed().as_secs_f64(),
                                    window: (size.width as f32, size.height as f32),
                                };
                                widget_inst
                                    .get_mut(panel.idx())
                                    .and_then(|w| w.as_mut())
                                    .map(|w| w.wheel(dy, r, &host))
                                    .unwrap_or(widgets::Action::None)
                            };
                            if let widgets::Action::ScrollTerminal(n) = action {
                                if let Some(s) = sessions[active].as_mut() {
                                    s.term.scroll_view(n);
                                }
                            }
                        }
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                        ..
                    } => {
                        // A captured drag ends here, and the release is
                        // the widget's drag(End) — copy-on-select fires
                        // in the TermSelect handler, on End only. The
                        // capture never coexists with the editor or the
                        // settings window: it only ever starts when
                        // neither had the press.
                        if let Some(panel) = drag_capture.take() {
                            if let widgets::Action::TermSelect { op, col, row, base } =
                                widget_drag!(panel, widgets::DragPhase::End, mouse.0, mouse.1)
                            {
                                apply_term_select!(op, col, row, base);
                            }
                            return;
                        }
                        if editor.active && !settings.open {
                            editor.mouse_up();
                            return;
                        }
                        if editor.active && settings.open {
                            settings.release();
                            let (snap, cols, rows, pad) = config::grid_prefs();
                            let size = window.inner_size();
                            editor.sync_prefs(
                                snap,
                                cols,
                                rows,
                                pad as f32,
                                size.width as f32,
                                size.height as f32,
                            );
                            ui_padding = pad as f32;
                            return;
                        }
                        if settings.open && settings.release() {
                                apply_config!(
                                    layout_spec, active_ov, popup, audio, fonts,
                                    font_scale, ui_font_scale, last_term_key, last_ui_key,
                                    window
                                );
                            }
                        // A drag ends: past the point of no return the
                        // turn completes to the neighbour, short of it
                        // the world settles back.
                        if let Some((horizontal, a)) = pan.take() {
                            press_at = None;
                            let sign: i32 = if a > 0.0 { 1 } else { -1 };
                            let target = if horizontal {
                                (cur_board.0 + sign, cur_board.1)
                            } else {
                                (cur_board.0, cur_board.1 + sign)
                            };
                            // Sideways only along the row; up and down
                            // reach the top and bottom from anywhere.
                            let on_arm = if horizontal { cur_board.1 == 0 } else { true };
                            let full: f32 = if horizontal { 90.0 } else { 1.0 };
                            let past = a.abs() >= full / 3.0;
                            if past && on_arm && has_board!(target) {
                                nacelle::sound::emit(nacelle::sound::Event::Snap);
                                cube = Some(Cube {
                                    horizontal,
                                    a0: a,
                                    a1: full * sign as f32,
                                    t0: Instant::now(),
                                    to: target,
                                    face_b: target,
                                });
                            } else if a.abs() > full * 0.001 {
                                cube = Some(Cube {
                                    horizontal,
                                    a0: a,
                                    a1: 0.0,
                                    t0: Instant::now(),
                                    to: cur_board,
                                    face_b: target,
                                });
                            }
                            return;
                        }
                        let Some((cx, cy)) = press_at.take() else { return };
                        // A click held in place: delivered now, to the
                        // widget it went down on. One route for every
                        // widget — the application does not know which
                        // one it is talking to.
                        let size = window.inner_size();
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        let hit = widgets::Panel::all()
                            .into_iter()
                            .find(|p| layout.p(*p).contains(cx, cy));
                        let Some(panel) = hit else { return };
                        // The content box the widget drew in — the same
                        // rect its draw received (u2 §4.1).
                        let r = panel_content
                            .get(panel.idx())
                            .copied()
                            .flatten()
                            .unwrap_or(layout.p(panel));
                        let occupied: Vec<bool> =
                            (0..SESSIONS).map(|i| sessions[i].is_some()).collect();
                        // Asked before the widget borrows the session:
                        // the answer is cached, and taking it here keeps
                        // that cache the only place that reads /proc.
                        let clicked_cwd = sessions[active].as_mut().and_then(|s| s.cwd());
                        let action = {
                            let snap = sys.lock().unwrap().clone();
                            let host = widgets::Host {
                                snap: &snap,
                                term: sessions[active].as_ref().map(|s| &s.term),
                                tabs: &occupied,
                                tab_active: active,
                                shell_cwd: clicked_cwd,
                                t: start.elapsed().as_secs_f64(),
                                window: (size.width as f32, size.height as f32),
                            };
                            widget_inst
                                .get_mut(panel.idx())
                                .and_then(|w| w.as_mut())
                                .map(|w| w.click(cx, cy, r, &host))
                                .unwrap_or(widgets::Action::None)
                        };
                        // The on-screen keyboard sounds its own keys, so a
                        // click on it must not also click. A selection
                        // step is quiet by nature.
                        if !matches!(
                            action,
                            widgets::Action::None
                                | widgets::Action::Bytes(_)
                                | widgets::Action::TermSelect { .. }
                                | widgets::Action::Capture
                        ) {
                            nacelle::sound::emit(nacelle::sound::Event::Click);
                        }
                        match action {
                            widgets::Action::Bytes(bytes) => {
                                if let Some(s) = sessions[active].as_mut() {
                                    s.pty.write(&bytes);
                                    s.term.view_offset = 0;
                                }
                            }
                            widgets::Action::OpenDir(dir) => {
                                // Entering a directory = cd in the active
                                // tab (a leading space skips bash history).
                                if let Some(s) = sessions[active].as_mut() {
                                    let quoted =
                                        dir.display().to_string().replace('\'', "'\\''");
                                    s.pty.write(format!(" cd '{quoted}'\r").as_bytes());
                                    s.term.view_offset = 0;
                                }
                            }
                            widgets::Action::OpenFile(file) => {
                                // Application associated with the extension.
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(&file)
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn();
                            }
                            // A tab number comes from a widget, and a
                            // widget is a file someone can replace: the
                            // number is checked here rather than trusted,
                            // because indexing past the end would take the
                            // whole desktop down with it.
                            widgets::Action::SelectTab(i) if i >= sessions.len() => {
                                eprintln!(
                                    "nacelle-desktop: a widget asked for session {i}; there are {}",
                                    sessions.len()
                                );
                            }
                            widgets::Action::SelectTab(i) => {
                                if sessions[i].is_some() {
                                    active = i;
                                } else {
                                    // A new tab starts where the active
                                    // shell is, which is what the file
                                    // panel is showing.
                                    let start = sessions[active]
                                        .as_mut()
                                        .and_then(|s| s.cwd())
                                        .unwrap_or_else(|| home.clone());
                                    match Session::spawn(grid.0, grid.1, &start) {
                                        Ok(s) => {
                                            sessions[i] = Some(s);
                                            active = i;
                                        }
                                        Err(e) => {
                                            eprintln!("nacelle-desktop: cannot open PTY: {e}")
                                        }
                                    }
                                }
                            }
                            widgets::Action::Exit => {
                                eprintln!("nacelle-desktop: closed from the control panel");
                                elwt.exit();
                            }
                            widgets::Action::OpenSettings => settings.show(),
                            widgets::Action::ScrollTerminal(n) => {
                                if let Some(s) = sessions[active].as_mut() {
                                    s.term.scroll_view(n);
                                }
                            }
                            // A widget may also answer these from a
                            // click; the handlers are the drag path's.
                            widgets::Action::TermSelect { op, col, row, base } => {
                                apply_term_select!(op, col, row, base)
                            }
                            widgets::Action::PastePrimary => {
                                paste_into_active!(nacelle::clipboard::Board::Primary)
                            }
                            // Nothing to do by construction: it is the
                            // press path's "the gesture is mine", and a
                            // click is not a gesture to capture.
                            widgets::Action::Capture => {}
                            widgets::Action::None => {}
                        }
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => {
                        // Any pointer press hides the focus ring (the
                        // focus-visible rule, F1 §1.2) — focus itself
                        // stays wherever it was.
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        // Mid-turn the boards are nowhere in particular;
                        // the press waits for the world to settle.
                        if cube.is_some() {
                            return;
                        }
                        // The open context menu is the top layer: the
                        // press is its first, BEFORE panel dispatch —
                        // and a press outside every level closes the
                        // menu AND is consumed (no click-through, the
                        // §4.6 rule the popup already follows).
                        if let Some(m) = menu.as_mut() {
                            use nacelle::object::menu::MenuOut;
                            match m.click(mouse.0, mouse.1) {
                                MenuOut::Close => menu = None,
                                MenuOut::Pick(cmd) => {
                                    menu = None;
                                    nacelle::sound::emit(nacelle::sound::Event::Click);
                                    run_menu_cmd!(cmd);
                                }
                                MenuOut::None => {}
                            }
                            return;
                        }
                        let size = window.inner_size();
                        // A click on the warning popup dismisses it. The
                        // toaster answers against the box it drew, so it
                        // needs the point and nothing else.
                        if popup.click(mouse.0, mouse.1) {
                            return;
                        }
                        // The layout editor captures all clicks while active
                        // (unless the settings window is open over it).
                        if editor.active && !settings.open {
                            match editor.mouse_down(
                                mouse.0,
                                mouse.1,
                                size.width as f32,
                                size.height as f32,
                            ) {
                                widgets::editor::EditorHit::Save => {
                                    if cur_board != (0, 0) {
                                        save_board_cur!();
                                        return;
                                    }
                                    // Overwrite the currently selected layout —
                                    // only the changes, for this screen.
                                    let name = config::current_layaut_name()
                                        .unwrap_or_else(|| "default".to_string());
                                    editor_save(
                                        &mut editor,
                                        &name,
                                        false,
                                        &mut layout_spec,
                                        &mut active_ov,
                                        &mut popup,
                                        screen,
                                    );
                                    refresh_boards!();
                                }
                                widgets::editor::EditorHit::SaveAs => {
                                    // A board is a place, not a style: it
                                    // has no name to save as, so the same
                                    // save answers both buttons there.
                                    if cur_board != (0, 0) {
                                        save_board_cur!();
                                        return;
                                    }
                                    editor.begin_naming(&mut focus_ctl);
                                }
                                widgets::editor::EditorHit::Exit => {
                                    // Back to the settings window, GRID view.
                                    editor.stop();
                                    settings.show_grid();
                                }
                                widgets::editor::EditorHit::Settings => {
                                    settings.show_grid();
                                }
                                widgets::editor::EditorHit::Handled => {}
                            }
                            return;
                        }
                        // An open settings window captures all clicks —
                        // except the editor buttons, which share its plane.
                        if settings.open {
                            if editor.active {
                                if let Some(hit) = editor.buttons_hit(
                                    mouse.0,
                                    mouse.1,
                                    size.width as f32,
                                    size.height as f32,
                                ) {
                                    match hit {
                                        widgets::editor::EditorHit::Settings => {
                                            // Toggle: hide the window.
                                            settings.close();
                                        }
                                        widgets::editor::EditorHit::Save => {
                                            if cur_board != (0, 0) {
                                                save_board_cur!();
                                                return;
                                            }
                                            let name = config::current_layaut_name()
                                                .unwrap_or_else(|| "default".to_string());
                                            editor_save(
                                                &mut editor,
                                                &name,
                                                false,
                                                &mut layout_spec,
                                                &mut active_ov,
                                                &mut popup,
                                                screen,
                                            );
                                        }
                                        widgets::editor::EditorHit::SaveAs => {
                                            if cur_board != (0, 0) {
                                                save_board_cur!();
                                                return;
                                            }
                                            settings.close();
                                            editor.begin_naming(&mut focus_ctl);
                                        }
                                        widgets::editor::EditorHit::Exit => {
                                            editor.stop();
                                            settings.show_grid();
                                        }
                                        widgets::editor::EditorHit::Handled => {}
                                    }
                                    return;
                                }
                            }
                            if settings.click(
                                mouse.0,
                                mouse.1,
                                size.width as f32,
                                size.height as f32,
                                Some(&mut focus_ctl),
                            ) {
                                apply_config!(
                                    layout_spec, active_ov, popup, audio, fonts,
                                    font_scale, ui_font_scale, last_term_key, last_ui_key,
                                    window
                                );
                            }
                            // Whatever the press asked the application
                            // for — the body shared with the keyboard
                            // path (F1 §1.5).
                            settings_after!();
                            return;
                        }
                        // The press is offered to the widget under it as
                        // drag(Begin) first — the single capture path.
                        // Today only the shell view accepts (a press on
                        // its cell grid starts a selection); a declined
                        // Begin answers None and the press falls through
                        // to the machinery below, so tab clicks and
                        // board drags are exactly what they were.
                        click_streak = match click_last {
                            Some((t, x, y))
                                if t.elapsed() < std::time::Duration::from_millis(400)
                                    && (mouse.0 - x).abs() < 6.0
                                    && (mouse.1 - y).abs() < 6.0 =>
                            {
                                click_streak + 1
                            }
                            _ => 1,
                        };
                        click_last = Some((Instant::now(), mouse.0, mouse.1));
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        let hit = widgets::Panel::all()
                            .into_iter()
                            .find(|p| layout.p(*p).contains(mouse.0, mouse.1));
                        if let Some(panel) = hit {
                            // The widget's own answer decides who owns
                            // the hand: anything but None takes the
                            // capture (the contract `Widget::drag`
                            // states), and the board never sees the
                            // gesture. A selection asks for something
                            // while it captures; a scroll thumb asks
                            // for nothing and says so with Capture.
                            match widget_drag!(
                                panel,
                                widgets::DragPhase::Begin,
                                mouse.0,
                                mouse.1
                            ) {
                                widgets::Action::None => {}
                                widgets::Action::TermSelect { op, col, row, base } => {
                                    apply_term_select!(op, col, row, base);
                                    drag_capture = Some(panel);
                                    return;
                                }
                                _ => {
                                    drag_capture = Some(panel);
                                    return;
                                }
                            }
                        }
                        // The click is not delivered yet. Held and
                        // moved, it becomes a board drag; released where
                        // it went down, the widget under it gets it then.
                        press_at = Some((mouse.0, mouse.1));
                        let _ = size;
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Middle,
                        ..
                    } => {
                        // Middle click over the shell view pastes the
                        // PRIMARY selection — the terminal convention,
                        // the same action a widget would ask for with
                        // Action::PastePrimary. Primary may simply not
                        // exist (gamescope): then this is a quiet no-op.
                        // A pointer press hides the focus ring (F1 §1.2).
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        // An open menu is a grab: the press only closes
                        // it, exactly like a left press outside.
                        if menu.is_some() {
                            menu = None;
                            return;
                        }
                        if cube.is_some() || editor.active || settings.open {
                            return;
                        }
                        let size = window.inner_size();
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        if term_panel
                            .is_some_and(|p| layout.p(p).contains(mouse.0, mouse.1))
                        {
                            paste_into_active!(nacelle::clipboard::Board::Primary);
                        }
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Right,
                        ..
                    } => {
                        // The F1 §4.2 right-click menus. A press with
                        // one already open dismisses it first, then
                        // opens whatever the new point earns — the
                        // toolkit behaviour for a moved right-click.
                        // Like every pointer press it hides the focus
                        // ring first (F1 §1.2).
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        menu = None;
                        if cube.is_some() {
                            return;
                        }
                        // The SAVE AS field: the input object's menu.
                        // The editor is otherwise pointer-first and
                        // claims no other right-click in F1.
                        if editor.active && !settings.open {
                            if editor.naming.is_some()
                                && editor
                                    .naming_field
                                    .map_or(false, |r| r.contains(mouse.0, mouse.1))
                            {
                                open_menu_at!(input_menu_entries!(), mouse.0, mouse.1);
                            }
                            return;
                        }
                        // The settings window claims its plane whole;
                        // no F1 menu opens over it (§4.2 names none).
                        if settings.open {
                            return;
                        }
                        let size = window.inner_size();
                        let layout = outer_layout(
                            cur_def!(),
                            cur_def!().pick(screen),
                            size.width as f32,
                            size.height as f32,
                            ui_padding,
                        )
                        .padded(ui_padding);
                        let hit = widgets::Panel::all()
                            .into_iter()
                            .find(|p| layout.p(*p).contains(mouse.0, mouse.1));
                        let Some(panel) = hit else { return };
                        // The content box the widget drew in (u2 §4.1);
                        // everything above it is the host's chrome —
                        // the title band and its padding.
                        let content = panel_content
                            .get(panel.idx())
                            .copied()
                            .flatten()
                            .unwrap_or(layout.p(panel));
                        if term_panel == Some(panel)
                            && content.contains(mouse.0, mouse.1)
                        {
                            // Terminal panel: copy/paste/scrollback/tabs.
                            open_menu_at!(terminal_menu_entries!(), mouse.0, mouse.1);
                        } else if mouse.1 < content.y {
                            // The title band: the existing Actions,
                            // menu-shaped (edit layout, settings, the
                            // neighbour boards).
                            open_menu_at!(panel_menu_entries!(), mouse.0, mouse.1);
                        }
                        // A right-click no desktop rule claims does
                        // what it does today: nothing (widgets and
                        // plugins join in F2 — F1 lays no wrong rails).
                    }
                    // IME (F1 §3.2). These arrive only while
                    // set_ime_allowed is on — today, only for the SAVE
                    // AS field. Backend reality, and the degradation
                    // ladder this code accepts: KWin-Wayland speaks
                    // text-input-v3 and delivers real Preedit/Commit;
                    // X11/XWayland is XIM — fragile preedit, commit
                    // mostly; gamescope typically runs no IME at all,
                    // so everything falls back to plain KeyboardInput,
                    // which the field already handles as Insert. No
                    // path below is load-bearing: with no IME the
                    // prompt still types.
                    WindowEvent::Ime(ime) => {
                        use nacelle::object::text_input::InputMsg;
                        if let Some(model) = editor.naming.as_mut() {
                            match ime {
                                Ime::Preedit(text, range) => {
                                    model.apply(InputMsg::Preedit(text, range));
                                }
                                Ime::Commit(text) => {
                                    // The composition's own end: most
                                    // backends send Preedit("") first,
                                    // XIM sometimes does not — ending
                                    // it here is the belt to that
                                    // suspender (§3.7). Commit is the
                                    // ONE text source while composing;
                                    // canonicalised like typed text.
                                    model.apply(InputMsg::PreeditEnd);
                                    model.apply(InputMsg::Insert(text.to_lowercase()));
                                }
                                Ime::Enabled => {}
                                Ime::Disabled => {
                                    // A dying IME must not leave a
                                    // phantom composition on screen.
                                    model.apply(InputMsg::PreeditEnd);
                                }
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if key_event.state != ElementState::Pressed {
                            return;
                        }
                        // The open context menu is the TOP layer (F1
                        // §4.3): it sees every key first, and an open
                        // menu is a grab — a key it does not understand
                        // is consumed, not passed under it.
                        if let Some(m) = menu.as_mut() {
                            use nacelle::object::menu::MenuOut;
                            if let Some(kev) =
                                focus_key_ev(&key_event.logical_key, mods)
                            {
                                match m.key(&kev) {
                                    MenuOut::Close => menu = None,
                                    MenuOut::Pick(cmd) => {
                                        menu = None;
                                        nacelle::sound::emit(
                                            nacelle::sound::Event::Click,
                                        );
                                        run_menu_cmd!(cmd);
                                    }
                                    MenuOut::None => {}
                                }
                            }
                            return;
                        }
                        // Layout editor: the SAVE AS prompt takes typing;
                        // otherwise ESC exits without saving. Nothing
                        // reaches the terminal.
                        if editor.active && !settings.open {
                            if editor.naming.is_some() {
                                use nacelle::object::text_input::{
                                    self, InputEdited, InputMsg,
                                };
                                // The neutral key event, WITH the text
                                // this press produced (dead keys and
                                // compose already applied by winit) —
                                // that text is what a field inserts.
                                let Some(mut kev) =
                                    focus_key_ev(&key_event.logical_key, mods)
                                else {
                                    return;
                                };
                                // Shift+F10 / Menu over the focused
                                // field opens ITS menu, anchored under
                                // the field's box (F1 §4.2: the focus
                                // chain's rect; the prompt is the one
                                // focused control of this world).
                                {
                                    use nacelle::focus::Scope;
                                    if shortcuts
                                        .lookup_over_greedy(&[Scope::Global], &kev)
                                        == Some(CMD_OPEN_MENU)
                                    {
                                        if let Some(r) = editor.naming_field {
                                            open_menu_at!(
                                                input_menu_entries!(),
                                                r.x,
                                                r.bottom()
                                            );
                                        }
                                        return;
                                    }
                                }
                                kev.text =
                                    key_event.text.as_ref().map(|s| s.to_string());
                                let Some(msg) = text_input::key_msg(&kev) else {
                                    return;
                                };
                                // One text source at a time (F1 §3.2
                                // red-team): while the IME composes,
                                // committed text arrives as Ime::Commit
                                // — a KeyboardInput insert alongside it
                                // would double characters, so it drops.
                                let composing = editor
                                    .naming
                                    .as_ref()
                                    .map_or(false, |m| m.has_preedit());
                                if composing && matches!(msg, InputMsg::Insert(_)) {
                                    return;
                                }
                                // Layout names are lowercase by rule;
                                // canonicalise BEFORE the validator
                                // judges (the old type_char behaviour).
                                let msg = match msg {
                                    InputMsg::Insert(s) => {
                                        InputMsg::Insert(s.to_lowercase())
                                    }
                                    m => m,
                                };
                                let out = editor
                                    .naming
                                    .as_mut()
                                    .map(|m| m.apply(msg))
                                    .unwrap_or(InputEdited::None);
                                match out {
                                    InputEdited::Submit => {
                                        let name = editor
                                            .naming
                                            .as_ref()
                                            .map(|m| m.value().to_string())
                                            .unwrap_or_default();
                                        if !name.is_empty() {
                                            editor_save(
                                                &mut editor,
                                                &name,
                                                true,
                                                &mut layout_spec,
                                                &mut active_ov,
                                                &mut popup,
                                                screen,
                                            );
                                        }
                                    }
                                    InputEdited::Cancel => editor.close_naming(),
                                    // The model answers with INTENTS;
                                    // the clipboard seam is the app's.
                                    InputEdited::CopyRequest { text, .. } => {
                                        nacelle::clipboard::store(
                                            nacelle::clipboard::Board::Clipboard,
                                            &text,
                                        );
                                    }
                                    InputEdited::PasteRequest => {
                                        // A paste is filtered to the
                                        // name alphabet rather than
                                        // rejected whole — the old
                                        // type_char behaviour, kept.
                                        if let Some(text) = nacelle::clipboard::load(
                                            nacelle::clipboard::Board::Clipboard,
                                        ) {
                                            let text: String = text
                                                .to_lowercase()
                                                .chars()
                                                .filter(|&c| {
                                                    widgets::editor::Editor::layaut_name_char(c)
                                                })
                                                .collect();
                                            if let Some(m) = editor.naming.as_mut() {
                                                m.apply(InputMsg::Insert(text));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            } else if let Key::Named(NamedKey::Escape) =
                                key_event.logical_key
                            {
                                editor.stop();
                            }
                            return;
                        }
                        // Open settings window: a modal layer, so no
                        // key reaches the terminal. The window walks
                        // its own chain (F1 §1.5: Tab in draw order,
                        // bare arrows spatially, Enter/Space through
                        // the click body, sliders eating Left/Right);
                        // whatever it does not claim falls back to the
                        // layer's own Escape = close, exactly as
                        // before focus existed.
                        if settings.open {
                            if let Some(kev) =
                                focus_key_ev(&key_event.logical_key, mods)
                            {
                                use widgets::settings::KeyOut;
                                match settings.key(&kev, &mut focus_ctl) {
                                    KeyOut::Changed => {
                                        apply_config!(
                                            layout_spec, active_ov, popup, audio,
                                            fonts, font_scale, ui_font_scale,
                                            last_term_key, last_ui_key, window
                                        );
                                        settings_after!();
                                    }
                                    KeyOut::Consumed => settings_after!(),
                                    KeyOut::Ignored => {
                                        if kev.key == nacelle::focus::Key::Escape {
                                            settings.close();
                                        }
                                    }
                                }
                            }
                            return;
                        }
                        // Application shortcuts.
                        if let Key::Named(NamedKey::F11) = key_event.logical_key {
                            let fs = window.fullscreen();
                            window.set_fullscreen(if fs.is_some() {
                                None
                            } else {
                                Some(Fullscreen::Borderless(None))
                            });
                            return;
                        }
                        if mods.control_key() && mods.shift_key() {
                            if let Key::Character(s) = &key_event.logical_key {
                                if s.eq_ignore_ascii_case("q") {
                                    elwt.exit();
                                    return;
                                }
                            }
                        }
                        // The F1 §1 registry, restricted to OVER_GREEDY:
                        // the terminal is a greedy control and every
                        // plain chord stays PTY bytes — copy and paste
                        // are exactly the escape hatches that must work
                        // over it, which is why the emulator convention
                        // adds Shift. A matched chord is CONSUMED even
                        // when it then does nothing (no selection, an
                        // empty clipboard): Ctrl+Shift+C must never
                        // fall through and become a ^C.
                        if let Some(kev) = focus_key_ev(&key_event.logical_key, mods) {
                            use nacelle::clipboard::Board;
                            use nacelle::focus::Scope;
                            match shortcuts.lookup_over_greedy(&[Scope::Global], &kev) {
                                Some(CMD_COPY) => {
                                    let text = sessions[active]
                                        .as_ref()
                                        .and_then(|s| s.term.selection_text())
                                        .unwrap_or_default();
                                    if !text.is_empty() {
                                        nacelle::clipboard::store(Board::Clipboard, &text);
                                    }
                                    return;
                                }
                                Some(CMD_PASTE) => {
                                    paste_into_active!(Board::Clipboard);
                                    return;
                                }
                                Some(CMD_OPEN_MENU) => {
                                    // The focused control's menu at its
                                    // rect corner (F1 §4.2). No focus
                                    // chain is wired in the desktop yet
                                    // and the terminal owns the keyboard
                                    // at boot, so the focused control
                                    // IS the terminal view; its content
                                    // box stands in for `rect_of` until
                                    // the §1 desktop router lands.
                                    // Fallback to the pointer when no
                                    // widget on this board holds one.
                                    let r = term_panel
                                        .and_then(|p| panel_content.get(p.idx()).copied())
                                        .flatten();
                                    let (ax, ay) =
                                        r.map(|r| (r.x, r.y)).unwrap_or(mouse);
                                    open_menu_at!(terminal_menu_entries!(), ax, ay);
                                    return;
                                }
                                _ => {}
                            }
                        }
                        let app_cursor = sessions[active]
                            .as_ref()
                            .map(|s| s.term.app_cursor)
                            .unwrap_or(false);
                        if let Some(bytes) =
                            key_to_bytes(&key_event.logical_key, mods, app_cursor)
                        {
                            // Highlight the key on the on-screen keyboard.
                            let (ch, label) = match &key_event.logical_key {
                                Key::Character(s) => (s.chars().next(), None),
                                Key::Named(NamedKey::Enter) => (None, Some("ENTER")),
                                Key::Named(NamedKey::Backspace) => (None, Some("BACK")),
                                Key::Named(NamedKey::Space) => (None, Some("SPACE")),
                                Key::Named(NamedKey::Tab) => (None, Some("TAB")),
                                Key::Named(NamedKey::Escape) => (None, Some("ESC")),
                                _ => (None, None),
                            };
                            // Announced to every widget: which of them
                            // draws an on-screen keyboard is not this
                            // program's business, and the ones that
                            // draw none ignore it (the interface's
                            // default is to do nothing).
                            for wg in widget_inst.iter_mut().flatten() {
                                wg.key_feedback(ch, label);
                            }
                            // Typing: Enter and Backspace have their own
                            // sounds, every other key shares the rotating
                            // Key variants.
                            nacelle::sound::emit(match &key_event.logical_key {
                                Key::Named(NamedKey::Enter) => {
                                    nacelle::sound::Event::KeyReturn
                                }
                                Key::Named(NamedKey::Backspace) => {
                                    nacelle::sound::Event::KeyErase
                                }
                                _ => nacelle::sound::Event::Key,
                            });
                            if let Some(s) = sessions[active].as_mut() {
                                s.pty.write(&bytes);
                                s.term.view_offset = 0;
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        if last_render.elapsed() < FRAME {
                            return;
                        }
                        last_render = Instant::now();
                        if let Some(f) = fullscreen.as_mut() {
                            f.poll();
                        }
                        // Live preview of the size sliders while dragging.
                        if let Some((tscale, uscale)) = settings.live_scales() {
                            font_scale = tscale;
                            ui_font_scale = uscale;
                        }
                        // Live widget padding while the GRID view is open.
                        if let Some(p) = settings.live_padding() {
                            ui_padding = p as f32;
                        }
                        // The layout is recomputed from the window size every
                        // frame (nacelle::flex), so moving the window to another
                        // monitor or resizing it reflows the interface live.
                        // 1. PTY data for all sessions; exited sessions free their slot.
                        for slot in sessions.iter_mut() {
                            let exited = slot.as_mut().map(|s| s.pump()).unwrap_or(false);
                            if exited {
                                *slot = None;
                            }
                        }
                        if sessions[active].is_none() {
                            // Active session died — switch to the first live one.
                            match sessions.iter().position(|s| s.is_some()) {
                                Some(i) => active = i,
                                None => {
                                    eprintln!("nacelle-desktop: all shells have exited");
                                    elwt.exit();
                                    return;
                                }
                            }
                        }

                        // 2. Session state the widgets are given this frame.
                        let occupied: Vec<bool> =
                            (0..SESSIONS).map(|i| sessions[i].is_some()).collect();
                        let shell_cwd = sessions[active].as_mut().and_then(|s| s.cwd());

                        // 3. Build the draw list.
                        let size = window.inner_size();
                        let (w, h) = (size.width as f32, size.height as f32);
                        if w < 8.0 || h < 8.0 {
                            return;
                        }
                        // The decoration plate follows the theme and the
                        // surface, never the frame: kick a rebake when
                        // either changes, collect it whenever it lands.
                        let plate_want = (theme::epoch(), size.width, size.height);
                        if plate_key != Some(plate_want) {
                            plate_key = Some(plate_want);
                            let (pw, ph) = (size.width, size.height);
                            let (tx, rx) = std::sync::mpsc::channel();
                            plate_rx = Some(rx);
                            // The bake reads the resolved theme once at
                            // entry; a swap mid-bake re-kicks on the next
                            // frame's epoch check. A stale worker's send
                            // fails into a dropped receiver, silently.
                            std::thread::spawn(move || {
                                let _ = tx.send((
                                    theme::plate::bake_backdrop(pw, ph),
                                    theme::plate::bake_overlay(pw, ph),
                                ));
                            });
                        }
                        if let Some((back, over)) =
                            plate_rx.as_ref().and_then(|rx| rx.try_recv().ok())
                        {
                            plate_rx = None;
                            let mut install =
                                |tex: &mut Option<(draw::ImageId, u32, u32)>,
                                 baked: Option<nacelle::theme::Plate>,
                                 which: &str| {
                                    match baked {
                                        Some(p) => {
                                            let stale = match *tex {
                                                Some((_, tw, th)) => (tw, th) != (p.w, p.h),
                                                None => true,
                                            };
                                            if stale {
                                                // destroy_texture waits for
                                                // the device — theme swap and
                                                // resize only, never a steady
                                                // frame.
                                                if let Some((old, _, _)) = tex.take() {
                                                    gfx.destroy_texture(old);
                                                }
                                                *tex = Some((
                                                    gfx.create_texture(p.w, p.h),
                                                    p.w,
                                                    p.h,
                                                ));
                                            }
                                            if let Some((id, _, _)) = *tex {
                                                gfx.update_texture(id, &p.rgba);
                                            }
                                            eprintln!(
                                                "nacelle-desktop: {which} plate {}x{} baked in {:.1} ms",
                                                p.w, p.h, p.bake_ms
                                            );
                                        }
                                        // Every layer off: no plate, no quad.
                                        None => {
                                            if let Some((old, _, _)) = tex.take() {
                                                gfx.destroy_texture(old);
                                            }
                                        }
                                    }
                                };
                            install(&mut plate_tex, back, "backdrop");
                            install(&mut overlay_tex, over, "overlay");
                        }
                        // Perform any deferred glyph-atlas reset at the frame
                        // boundary, never mid-frame (see font.rs).
                        fonts.begin_frame();
                        dl.clear();
                        // The backdrop plate is the first thing in the
                        // list — z 0, under every panel, inside the glass
                        // snapshot. White tint is the multiplicative
                        // identity: the plate's pixels ARE the theme's
                        // baked colours. Mid-resize it stretches for the
                        // frame or two until the fresh bake lands.
                        if let Some((id, _, _)) = plate_tex {
                            dl.image(
                                0.0,
                                0.0,
                                w,
                                h,
                                id,
                                theme::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
                            );
                        }
                        // Read in place, not copied. The snapshot carries
                        // the process table and every string in it, and
                        // cloning that for each frame was pure waste: the
                        // collector rewrites it once a second and can wait
                        // the few milliseconds a frame takes. Nothing
                        // below locks it again — that would deadlock.
                        let snap_held = sys.lock().unwrap();
                        let snap: &nacelle::telemetry::Snapshot = &snap_held;
                        // Rings are withheld while board rects are
                        // mid-flight (the cube ride) — set before any
                        // registration is answered this frame.
                        focus_ctl.set_ring_suppressed(cube.is_some());
                        let mut ctx = widgets::Ctx {
                            dl: &mut dl,
                            fonts: &mut fonts,
                            w,
                            h,
                            t: hashframe::clock(start.elapsed().as_secs_f64()),
                            mouse,
                            term_font_scale: font_scale,
                            ui_font_scale,
                            panel_scale: 1.0,
                            // The desktop's chain (F1 §1.2). Only the
                            // settings window registers in this slice;
                            // with it closed the chain stays empty and
                            // the boot frame keeps its pixels (the ring
                            // needs keyboard navigation to exist).
                            focus: Some(&mut focus_ctl),
                            // Requests are collected all through the
                            // frame and answered at the end of it, below
                            // — a tooltip is drawn over what it explains,
                            // so it cannot be drawn while that is still
                            // being drawn.
                            tips: Some(&mut tips),
                        };

                        booting = widgets::boot::draw(&mut ctx);
                        if !booting {
                            // A finished turn lands first, so this frame
                            // measures and draws the board it arrived at —
                            // or starts the next leg of a longer journey.
                            let ride_s = nacelle::deco::ride_secs();
                            if let Some(c) = &cube {
                                if c.t0.elapsed().as_secs_f32() >= ride_s {
                                    let landed = c.a1 != 0.0;
                                    if landed {
                                        cur_board = c.to;
                                        // The world's notion of "here"
                                        // must land with us, or the next
                                        // rebuild would send us home.
                                        world.set_current(cur_board);
                                        nacelle::base::set_panel_sizes(&cur_def!().sizes);
                                    }
                                    cube = None;
                                    if landed {
                                        if let Some(next) = go_queue.pop() {
                                            step_to!(next);
                                        }
                                    } else {
                                        go_queue.clear();
                                    }
                                }
                            }
                            let trans: Option<(bool, f32)> = pan.or_else(|| {
                                cube.as_ref().map(|c| {
                                    let t = if ride_s <= 0.0 {
                                        1.0
                                    } else {
                                        (c.t0.elapsed().as_secs_f32() / ride_s)
                                            .clamp(0.0, 1.0)
                                    };
                                    let e = nacelle::deco::ride_ease(t);
                                    (c.horizontal, c.a0 + (c.a1 - c.a0) * e)
                                })
                            });
                            let host = widgets::Host {
                                snap: &snap,
                                term: sessions[active].as_ref().map(|s| &s.term),
                                tabs: &occupied,
                                tab_active: active,
                                shell_cwd: shell_cwd.clone(),
                                // The SAME clock the draw context above
                                // was given: a widget animating off
                                // `host.t` while the context ran on a
                                // virtual clock would put the machine's
                                // speed back into a frame the pixel
                                // guard is trying to compare.
                                t: hashframe::clock(start.elapsed().as_secs_f64()),
                                window: (w, h),
                            };
                            // Two passes, because a widget's height comes
                            // from its content and its content is sized by
                            // the width it is given. The first pass settles
                            // the columns — their widths do not depend on
                            // any of this — the widgets measure themselves
                            // against those widths, and the second pass
                            // hands each the height it asked for. A widget
                            // that grows into whatever it gets measures as
                            // None and shares what the others left.
                            {
                                // The probe must not see last frame's
                                // answers, or each measurement would feed
                                // the next and the panels would creep
                                // frame by frame.
                                nacelle::base::set_panel_intrinsic(&[]);
                                let probe = if editor.active {
                                    editor.layout(w, h)
                                } else {
                                    outer_layout(
                                        cur_def!(),
                                        cur_def!().pick(screen),
                                        w,
                                        h,
                                        ui_padding,
                                    )
                                }
                                .padded(ui_padding);
                                // Standing on a fixture, the main-row
                                // board rides along underneath, sharp and
                                // in place, showing through the glass —
                                // but intrinsic sizing is a GLOBAL table
                                // keyed by panel, and without this its
                                // widgets would only ever be measured
                                // while home itself is the current board.
                                // A widget lives on one board only, so
                                // the two probes' panels never collide.
                                let under_probe = if !editor.active && cur_board.1 != 0 {
                                    let udef = def_of!((cur_board.0, 0));
                                    Some(
                                        outer_layout(udef, udef.pick(screen), w, h, ui_padding)
                                            .padded(ui_padding),
                                    )
                                } else {
                                    None
                                };
                                let mut wants: Vec<Option<f32>> = Vec::new();
                                // What the container adds around each
                                // panel, published beside the wants: the
                                // layout engine adds it to the content
                                // minimums, so a panel held at its
                                // minimum keeps its last content row
                                // BELOW the title band instead of losing
                                // a band's worth of content to it.
                                let mut chrome_px: Vec<f32> = Vec::new();
                                for p in widgets::Panel::all() {
                                    let here = probe.p(p);
                                    let r = if here.x < w {
                                        Some(here)
                                    } else {
                                        under_probe.as_ref().map(|u| u.p(p)).filter(|r| r.x < w)
                                    };
                                    // A panel no board placed sits at the
                                    // off-screen rectangle; measuring it
                                    // would run absent widgets for a
                                    // height nobody will use.
                                    let Some(r) = r else {
                                        panel_scale[p.idx()] = 1.0;
                                        wants.push(None);
                                        chrome_px.push(0.0);
                                        continue;
                                    };
                                    // Measured at scale 1, so the answer is
                                    // the content's own size and not a
                                    // reflection of the box it happens to
                                    // be in this frame.
                                    ctx.panel_scale = 1.0;
                                    let (sizing, titled, scales) = widget_inst
                                        .get_mut(p.idx())
                                        .and_then(|w| w.as_mut())
                                        .map(|wg| {
                                            let s = wg.sizing(&mut ctx, &host);
                                            let c = wg.chrome(&mut ctx, &host);
                                            (
                                                s,
                                                c.title.is_some() || c.right.is_some(),
                                                wg.scales_with_panel(),
                                            )
                                        })
                                        .unwrap_or((widgets::Sizing::Reference, false, true));
                                    // What the container will draw around
                                    // the content: border, padding, and
                                    // the title band when the widget
                                    // declares one. A Content panel is
                                    // made tall enough for BOTH, and the
                                    // widget's scale is computed against
                                    // the room the chrome leaves (u2 §4.2).
                                    let chrome_extra =
                                        nacelle::object::panel::chrome_extra(titled);
                                    chrome_px.push(chrome_extra);
                                    // Which edge of the panel changes the
                                    // size of what is inside is the
                                    // widget's answer, not one rule for
                                    // all of them: a table of rows must
                                    // not magnify when it is stretched
                                    // downwards, and a clock must keep its
                                    // proportions whichever way it is
                                    // pulled.
                                    let ws = {
                                        static REF: OnceLock<TokenId> = OnceLock::new();
                                        static LO: OnceLock<TokenId> = OnceLock::new();
                                        static HI: OnceLock<TokenId> = OnceLock::new();
                                        let t = nacelle::theme::resolved();
                                        let rf = t.px(tok(&REF, "responsive.panel_ref_frac")).max(0.01);
                                        let lo = t.px(tok(&LO, "responsive.scale_min")).max(0.05);
                                        let hi = t.px(tok(&HI, "responsive.scale_max")).max(lo);
                                        (r.w / (h * rf)).clamp(lo, hi)
                                    };
                                    let scale = match sizing {
                                        widgets::Sizing::Rows => ws,
                                        // A widget that does not follow
                                        // panel_scale draws its measured
                                        // content at scale 1 in any box, so
                                        // its want must be published whole:
                                        // shrinking the want without
                                        // shrinking the drawing is how the
                                        // control buttons left their panel
                                        // at 1280x800.
                                        widgets::Sizing::Content(_) if !scales => 1.0,
                                        widgets::Sizing::Content(natural) => {
                                            {
                                            static LO2: OnceLock<TokenId> = OnceLock::new();
                                            static HI2: OnceLock<TokenId> = OnceLock::new();
                                            let t = nacelle::theme::resolved();
                                            let lo = t.px(tok(&LO2, "responsive.scale_min")).max(0.05);
                                            let hi = t.px(tok(&HI2, "responsive.scale_max")).max(lo);
                                            ws.min(
                                                (r.h - chrome_extra).max(1.0)
                                                    / natural.max(1.0),
                                            )
                                            .clamp(lo, hi)
                                        }
                                        }
                                        widgets::Sizing::Reference => {
                                            ctx.panel_font_scale(&r, p)
                                        }
                                    };
                                    panel_scale[p.idx()] = scale;
                                    // The height a measured widget is given
                                    // is its content at the scale it will
                                    // be drawn with, PLUS the container
                                    // around it — so the box hugs content
                                    // and chrome together, and the band
                                    // never overlaps the first row.
                                    wants.push(match sizing {
                                        widgets::Sizing::Content(natural) => {
                                            Some(natural * scale + chrome_extra)
                                        }
                                        _ => None,
                                    });
                                    ctx.panel_scale = 1.0;
                                }
                                if !editor.active {
                                    nacelle::base::set_panel_intrinsic(&wants);
                                    nacelle::base::set_panel_chrome(&chrome_px);
                                }
                            }
                            // The editor shows its edited rectangles (WYSIWYG).
                            // Widgets draw inside the padded (content) rects;
                            // the editor overlay shows the outer edges.
                            let layout = if editor.active {
                                editor.layout(w, h)
                            } else {
                                outer_layout(
                                    cur_def!(),
                                    cur_def!().pick(screen),
                                    w,
                                    h,
                                    ui_padding,
                                )
                            }
                            .padded(ui_padding);
                            // What the terminal reported this frame, read
                            // before the editor redraws it as a thumbnail.
                            let grid_now: Option<(usize, usize)>;
                            // Every widget drawn through the one contract:
                            // the application no longer knows which is
                            // which, only what the registry lists.
                            {
                                // Below this the ride is not drawn at all.
                                // The guard is authored in the cube's
                                // degrees; the flat slide, whose full
                                // travel is 1.0 rather than 90, scales it
                                // down by the same ratio.
                                static EPSILON: OnceLock<TokenId> = OnceLock::new();
                                let eps = nacelle::theme::resolved()
                                    .px(tok(&EPSILON, "motion.board_ride.epsilon"));
                                let active_trans = trans.filter(|(hz, a)| {
                                    a.abs() > if *hz { eps } else { eps / 90.0 }
                                });
                                if let Some((horizontal, a)) = active_trans {
                                    // Two boards in motion. Sideways they
                                    // are the faces of a cube — a yaw and
                                    // a perspective divide applied to the
                                    // vertices the widgets have already
                                    // emitted; up and down they slide
                                    // flat. No widget knows either way.
                                    let sign: i32 = if a > 0.0 { 1 } else { -1 };
                                    let face_b =
                                        cube.as_ref().map(|c| c.face_b).unwrap_or(
                                            if horizontal {
                                                (cur_board.0 + sign, cur_board.1)
                                            } else {
                                                (cur_board.0, cur_board.1 + sign)
                                            },
                                        );
                                    // Per-face motion parameter: yaw for
                                    // the cube, y-offset for the ride-in.
                                    // Up and down nothing drags HOME along
                                    // any more: the ordinary board (y == 0)
                                    // holds perfectly still, and APPGRID or
                                    // SEARCH AND AI rides in over it — the
                                    // same picture their overlay layer will
                                    // give under the project's own
                                    // compositor.
                                    let (ma, mb) = if horizontal {
                                        (-a, 90.0 * sign as f32 - a)
                                    } else {
                                        (
                                            if cur_board.1 == 0 { 0.0 } else { -a * h },
                                            if face_b.1 == 0 {
                                                0.0
                                            } else {
                                                (sign as f32 - a) * h
                                            },
                                        )
                                    };
                                    let mut faces = [(cur_board, ma), (face_b, mb)];
                                    if horizontal && faces[0].1.abs() < faces[1].1.abs()
                                    {
                                        faces.swap(0, 1);
                                    }
                                    // The rider draws over the still board,
                                    // so leaving a fixture the still board
                                    // must be painted first.
                                    if !horizontal && cur_board.1 != 0 {
                                        faces.swap(0, 1);
                                    }
                                    // The space the cube turns in, one
                                    // flat colour under the whole turn.
                                    // It is emitted BEFORE the first face
                                    // and therefore before any face's
                                    // `start`, so no yaw ever touches it:
                                    // the walls move, the void does not.
                                    if horizontal {
                                        let void = nacelle::deco::ride_void();
                                        ctx.dl.rect(0.0, 0.0, w, h, void);
                                    }
                                    for (b, m) in faces {
                                        if !has_board!(b) {
                                            continue;
                                        }
                                        let start = ctx.dl.verts.len();
                                        // Sideways each face is a WALL of a
                                        // solid and carries its own ground
                                        // — what the theme puts behind a
                                        // board, the board's own field and
                                        // the decoration plate on it —
                                        // emitted before the panels so the
                                        // yaw and the perspective divide
                                        // below take ground and panels
                                        // together.
                                        // Standing still, and riding up or
                                        // down, a board paints no ground at
                                        // all: the frame's own clear and
                                        // plate are already there and must
                                        // stay visible under the fixture
                                        // that rides in over them.
                                        // Fixtures carry a face material on
                                        // top — frosted glass: whatever is
                                        // beneath shows through it blurred.
                                        // The glass is sampled by screen
                                        // position, so the ride may carry
                                        // the quad and the frost stays put.
                                        if horizontal {
                                            nacelle::deco::board_ground(
                                                ctx.dl,
                                                w,
                                                h,
                                                plate_tex.map(|(id, _, _)| id),
                                            );
                                        }
                                        let thm = nacelle::theme::resolved();
                                        if b.1 != 0 {
                                            nacelle::deco::fixture_glass(
                                                ctx.dl, w, h, frost_wash,
                                            );
                                        }
                                        let bdef = def_of!(b);
                                        let blay = outer_layout(
                                            bdef,
                                            bdef.pick(screen),
                                            w,
                                            h,
                                            ui_padding,
                                        )
                                        .padded(ui_padding);
                                        for panel in widgets::Panel::all() {
                                            let r = blay.p(panel);
                                            if r.x >= w {
                                                continue;
                                            }
                                            ctx.panel_scale = if b == cur_board {
                                                panel_scale[panel.idx()]
                                            } else {
                                                ctx.panel_font_scale(&r, panel)
                                            };
                                            if let Some(wg) = widget_inst
                                                .get_mut(panel.idx())
                                                .and_then(|w| w.as_mut())
                                            {
                                                let content = draw_panel(
                                                    &mut ctx,
                                                    wg.as_mut(),
                                                    r,
                                                    &host,
                                                    panel,
                                                );
                                                // Input belongs to the
                                                // board being stood on,
                                                // not the one riding by.
                                                if b == cur_board {
                                                    panel_content[panel.idx()] =
                                                        Some(content);
                                                }
                                            }
                                            ctx.panel_scale = 1.0;
                                        }
                                        if horizontal {
                                            static PERSP: OnceLock<TokenId> =
                                                OnceLock::new();
                                            static SHADE_MIN: OnceLock<TokenId> =
                                                OnceLock::new();
                                            let rad = m.to_radians();
                                            let (sinp, cosp) = rad.sin_cos();
                                            let rr = w / 2.0;
                                            let fl = w * thm.px(tok(
                                                &PERSP,
                                                "motion.board_ride.perspective",
                                            ));
                                            let smin = thm.px(tok(
                                                &SHADE_MIN,
                                                "boardswitch.shade_min",
                                            ));
                                            let shade =
                                                smin + (1.0 - smin) * cosp.max(0.0);
                                            // The turned-away wall settles
                                            // toward the very colour painted
                                            // behind the cube, not toward
                                            // #000000: edge-on it melts into
                                            // the space it turns in, and a
                                            // light theme rides through its
                                            // own dark, never through grey.
                                            let void = nacelle::deco::ride_void();
                                            for v in &mut ctx.dl.verts[start..] {
                                                let u = v.pos[0] - rr;
                                                let x3 = u * cosp + rr * sinp;
                                                let depth =
                                                    rr - (rr * cosp - u * sinp);
                                                let sc = fl / (fl + depth);
                                                v.pos[0] = rr + x3 * sc;
                                                v.pos[1] =
                                                    h / 2.0 + (v.pos[1] - h / 2.0) * sc;
                                                v.color[0] = void.r
                                                    + (v.color[0] - void.r) * shade;
                                                v.color[1] = void.g
                                                    + (v.color[1] - void.g) * shade;
                                                v.color[2] = void.b
                                                    + (v.color[2] - void.b) * shade;
                                            }
                                        } else {
                                            for v in &mut ctx.dl.verts[start..] {
                                                v.pos[1] += m;
                                            }
                                        }
                                    }
                                } else {
                                    if cur_board.1 != 0 {
                                        // Standing on APPGRID or SEARCH
                                        // AND AI: the main-row board
                                        // stays exactly where it was,
                                        // showing through the frosted
                                        // glass the fixture's panels
                                        // sit on.
                                        let under = (cur_board.0, 0);
                                        let udef = def_of!(under);
                                        let ulay = outer_layout(
                                            udef,
                                            udef.pick(screen),
                                            w,
                                            h,
                                            ui_padding,
                                        )
                                        .padded(ui_padding);
                                        for panel in widgets::Panel::all() {
                                            let r = ulay.p(panel);
                                            if r.x >= w {
                                                continue;
                                            }
                                            ctx.panel_scale = panel_scale[panel.idx()];
                                            if let Some(wg) = widget_inst
                                                .get_mut(panel.idx())
                                                .and_then(|w| w.as_mut())
                                            {
                                                // A widget lives on one
                                                // board only, so the ride-
                                                // under board's boxes and
                                                // the fixture's never
                                                // collide in this table.
                                                panel_content[panel.idx()] =
                                                    Some(draw_panel(
                                                        &mut ctx,
                                                        wg.as_mut(),
                                                        r,
                                                        &host,
                                                        panel,
                                                    ));
                                            }
                                            ctx.panel_scale = 1.0;
                                        }
                                        static GLASS_TINT: OnceLock<TokenId> =
                                            OnceLock::new();
                                        static GLASS_WASH: OnceLock<TokenId> =
                                            OnceLock::new();
                                        let thm = nacelle::theme::resolved();
                                        ctx.dl.blur(
                                            0.0,
                                            0.0,
                                            w,
                                            h,
                                            tcol(thm.color(tok(
                                                &GLASS_TINT,
                                                "elev.fixture.glass.tint",
                                            ))),
                                        );
                                        // The theme's own wash; the user's
                                        // BlurOpacity scales its alpha.
                                        let wash = thm.color(tok(
                                            &GLASS_WASH,
                                            "elev.fixture.glass.wash",
                                        ));
                                        if wash.a * frost_wash > 0.0 {
                                            ctx.dl.rect(
                                                0.0,
                                                0.0,
                                                w,
                                                h,
                                                tcol(wash).alpha(wash.a * frost_wash),
                                            );
                                        }
                                    }
                                    for panel in widgets::Panel::all() {
                                        let r = layout.p(panel);
                                        // Hidden here or living on another
                                        // board: not drawn, so a terminal
                                        // elsewhere keeps its PTY size.
                                        if r.x >= w {
                                            continue;
                                        }
                                        ctx.panel_scale = panel_scale[panel.idx()];
                                        if let Some(wg) = widget_inst
                                            .get_mut(panel.idx())
                                            .and_then(|w| w.as_mut())
                                        {
                                            panel_content[panel.idx()] =
                                                Some(draw_panel(
                                                    &mut ctx,
                                                    wg.as_mut(),
                                                    r,
                                                    &host,
                                                    panel,
                                                ));
                                        }
                                        ctx.panel_scale = 1.0;
                                    }
                                }
                                // Read the grid BEFORE the editor runs the
                                // same widgets again at miniature rects:
                                // the terminal reports whatever it drew
                                // last, and opening ADD WIDGET would
                                // otherwise resize it to a thumbnail.
                                // The widget that reports one IS the
                                // terminal view — the capability is the
                                // whole of what this program knows
                                // about it.
                                let held = widget_inst.iter().enumerate().find_map(
                                    |(i, w)| {
                                        w.as_ref()
                                            .and_then(|w| w.grid())
                                            .map(|g| (widgets::Panel(i as u16), g))
                                    },
                                );
                                term_panel = held.map(|(p, _)| p);
                                grid_now = held.map(|(_, g)| g);
                                // Grid overlay + editor controls on top of
                                // the live panels; the closure draws live
                                // miniatures in the ADD WIDGET window.
                                if editor.active {
                                    editor.draw(&mut ctx, |ctx, panel, r| {
                                        let p = widgets::Panel(panel as u16);
                                        // ADD WIDGET previews widgets from
                                        // outside the boards; they are
                                        // built here on first sight and
                                        // dropped again at the next sync
                                        // unless they get placed.
                                        if widget_inst[p.idx()].is_none() {
                                            widget_inst[p.idx()] = make_widget(p);
                                        }
                                        ctx.panel_scale = ctx.panel_font_scale(&r, p);
                                        if let Some(wg) =
                                            widget_inst.get_mut(p.idx()).and_then(|w| w.as_mut())
                                        {
                                            // A live miniature with its
                                            // container, exactly as it
                                            // will look placed; the
                                            // input table is NOT touched
                                            // — these rects are the ADD
                                            // WIDGET window's.
                                            draw_panel(ctx, wg.as_mut(), r, &host, p);
                                        }
                                        ctx.panel_scale = 1.0;
                                    });
                                }
                            }
                            let (cols, rows) = grid_now.unwrap_or(grid);
                            // The BOARDS view draws whatever this hands
                            // it: every board as it would look here and
                            // now, the current one marked.
                            // Drawn HERE only while no hall is hosting it
                            // — the window is one, and it is drawn once.
                            if settings.open && settings_host.is_none() {
                                settings.boards = board_thumbs!(w, h);
                            }
                            if settings_host.is_none() {
                                settings.draw(&mut ctx);
                            }
                            // With the settings window open over the editor
                            // its buttons share the window's plane.
                            if editor.active && settings.open {
                                editor.draw_buttons(&mut ctx);
                            }
                            // Warning popup on the very top.
                            popup.draw(&mut ctx);
                            // The open context menu draws after
                            // EVERYTHING interactive (F1 §4.3): the
                            // draw list is immediate, draw order is
                            // z-order, and the menu is the top layer —
                            // anything drawn later would sit on it.
                            // Only the theme's overlay plate follows:
                            // it covers panels, popovers and content
                            // alike by design.
                            if let Some(m) = menu.as_mut() {
                                m.draw(&mut ctx);
                            }
                            // Then the tooltip, over the menu as over
                            // everything else — taken OUT of the context
                            // first, because the manager cannot be lent
                            // to the frame and drawn into the same frame
                            // at once, and because nothing drawn after
                            // this point may file a request it would be
                            // too late to answer.
                            if let Some(t) = ctx.tips.take() {
                                // A menu covers what is under it, and
                                // explaining something the user cannot
                                // see is noise: requests filed under an
                                // open menu go down with it (F2 §8.1).
                                if menu.is_some() {
                                    t.clear();
                                }
                                t.draw(&mut ctx);
                            }
                            // The overlay plate is the LAST themed thing
                            // in the list — z 70, one quad over panels,
                            // popovers and content alike: scanlines,
                            // grain, the top vignette. White tint, same
                            // as the backdrop: the plate's pixels ARE
                            // the theme's baked colours.
                            if let Some((id, _, _)) = overlay_tex {
                                ctx.dl.image(
                                    0.0,
                                    0.0,
                                    w,
                                    h,
                                    id,
                                    theme::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
                                );
                            }

                            // Fit all session grids to the panel size.
                            if (cols, rows) != grid {
                                grid = (cols, rows);
                                for s in sessions.iter_mut().flatten() {
                                    s.term.resize(cols, rows);
                                    s.pty.resize(cols as u16, rows as u16);
                                }
                            }
                        }

                        // The focus frame boundary (F1 §1.2): the chain
                        // this frame's draws registered becomes the one
                        // navigation walks — after the world has drawn,
                        // before the next frame's events, so Tab never
                        // sees a half-built chain.
                        focus_ctl.begin_frame();

                        // 4. Sound preferences changed in the SOUND view
                        // apply immediately, so dragging the volume
                        // slider is audible while dragging.
                        if settings.color_dirty {
                            settings.color_dirty = false;
                            apply_color!();
                        }
                        if settings.blur_dirty {
                            settings.blur_dirty = false;
                            let (radius, opacity) = settings.blur_settings();
                            gfx.set_blur_radius(radius);
                            frost_wash = opacity as f32 / 100.0;
                        }
                        if settings.sound_dirty {
                            settings.sound_dirty = false;
                            let (vol, typing, ambient) = settings.sound_settings();
                            if let Some(a) = audio.as_mut() {
                                a.set_volume(vol);
                                a.set_typing_enabled(typing);
                                a.set_ambient_enabled(ambient);
                            }
                        }

                        // 4b. IME follows the frame (F1 §3.2): allowed
                        // strictly while a TEXT control owns the
                        // keyboard — the SAVE AS field is the only one
                        // wired in this slice (the terminal's stays
                        // off; see the ime_allowed declaration). The
                        // caret box the field just drew anchors the
                        // candidate window; winit's call wants LOGICAL
                        // coordinates and converts a Physical value
                        // itself by the window's scale factor (§3.7's
                        // logical-px trap, handled by the type).
                        {
                            let want_ime = editor.naming.is_some();
                            if want_ime != ime_allowed {
                                ime_allowed = want_ime;
                                window.set_ime_allowed(want_ime);
                                if want_ime {
                                    window.set_ime_purpose(
                                        winit::window::ImePurpose::Normal,
                                    );
                                } else {
                                    ime_area = None;
                                }
                            }
                            if want_ime {
                                if let Some(cr) = editor.naming_caret {
                                    let area = (
                                        cr.x as i32,
                                        cr.y as i32,
                                        cr.w.max(1.0) as i32,
                                        cr.h.max(1.0) as i32,
                                    );
                                    if ime_area != Some(area) {
                                        ime_area = Some(area);
                                        window.set_ime_cursor_area(
                                            winit::dpi::PhysicalPosition::new(
                                                area.0, area.1,
                                            ),
                                            winit::dpi::PhysicalSize::new(
                                                area.2, area.3,
                                            ),
                                        );
                                    }
                                }
                            }
                        }

                        // 5. Play whatever this frame reported. The theme
                        // decides which file each event maps to; an event
                        // it says nothing about is silently skipped.
                        nacelle::sound::drain(&mut sfx);
                        if let Some(a) = audio.as_mut() {
                            for e in sfx.iter() {
                                a.play(*e);
                            }
                        }

                        // 6. Render.
                        // Only the touched rows travel — a glyph-churn frame
                        // re-uploads a shelf, not the whole four megabytes.
                        let atlas_rows = fonts.take_dirty_rows();
                        // A hall's renderer has its own copy of the glyph
                        // atlas; whatever this frame drained belongs to it
                        // too, or the settings window would arrive there
                        // with holes where the main window took the rows.
                        if let Some((y0, rows)) = atlas_rows {
                            for s in salas.iter_mut() {
                                s.note_atlas_rows(y0, rows);
                            }
                        }
                        let fsize = window.inner_size();
                        // The swapchain clear is the absolute bed — one
                        // rung below the board's own fill; the master
                        // forces its alpha to 1.0.
                        static CLEAR: OnceLock<TokenId> = OnceLock::new();
                        let clear = nacelle::theme::resolved()
                            .bed(tok(&CLEAR, "surface.void"));
                        // The pixel guard, before a triangle leaves for the
                        // GPU: unarmed it is one atomic load.
                        hashframe::observe(&dl);
                        gfx.render(
                            fsize.width,
                            fsize.height,
                            &dl.verts,
                            &dl.runs,
                            atlas_rows.map(|(y0, rows)| (fonts.atlas.as_slice(), y0, rows)),
                            [clear.r, clear.g, clear.b, 1.0],
                        );
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    // The loop sleeps between frames instead of spinning:
                    // asking for a redraw the moment the last one landed
                    // is what made an idle desktop cost a whole core.
                    let now = Instant::now();
                    if now >= next_frame {
                        // Catching up frame by frame after a stall would
                        // burn through the backlog at full speed; the
                        // cadence simply restarts from now.
                        next_frame = now + FRAME;
                        window.request_redraw();
                        // The halls ride the same cadence; their frame
                        // is two textured quads, so this costs nothing.
                        for s in &salas {
                            s.window.request_redraw();
                        }
                    }
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_frame));
                }
                // One exit hook for every way out — the close button,
                // Ctrl+Shift+Q, the compositor, the last shell dying.
                // Blocking briefly is the point: the process would
                // otherwise cut the sound off as it goes.
                Event::LoopExiting => {
                    if let Some(a) = audio.as_mut() {
                        a.play_blocking(nacelle::sound::Event::Shutdown, 1400);
                    }
                }
                _ => {}
            }
        })
        .expect("event loop ended with an error");
}

/// Draws one panel through the one contract (u2 §4.1): asks the widget
/// for its chrome, has the host's container drawn — material, ring,
/// title band — and hands the widget the CONTENT BOX the container
/// left. Answers that box, because it is also the rect the panel's
/// `click` and `wheel` must later receive: hit-testing against any
/// other rectangle would land clicks on chrome the widget cannot see.
fn draw_panel(
    ctx: &mut nacelle::Ctx,
    wg: &mut dyn widgets::Widget,
    r: widgets::Rect,
    host: &widgets::Host,
    panel: widgets::Panel,
) -> widgets::Rect {
    let chrome = wg.chrome(ctx, host);
    let content = nacelle::object::panel::draw(ctx, r, &chrome, panel.idx());
    wg.draw(ctx, content, host);
    content
}

/// Builds one widget instance by name. A widget is its file: the
/// compiled library wins where there is one — it exists precisely
/// because a script could not do that job — and the script otherwise.
/// None when the file fails to load: the panel takes part in the
/// layout and draws nothing.
fn make_widget(p: widgets::Panel) -> Option<Box<dyn widgets::Widget>> {
    // The toolkit's factory: linked-in first (disk is never asked
    // about the four core widgets, so deleting files cannot take them
    // away), then a compiled plugin, then the script.
    config::widget_factory().make(p.name())
}

/// The current screen key: monitor resolution + diagonal in inches.
///
/// Asked once per screen change, never per frame: it queries the
/// display server for the monitor list and then the monitor's physical
/// size, and the layout code wants it in a dozen places. Doing that
/// inside the frame put a round trip and a sysfs scan between the
/// widgets and the picture.
fn screen_key(window: &winit::window::Window) -> (u32, u32, u32) {
    match window.current_monitor().or_else(|| window.primary_monitor()) {
        Some(m) => {
            let s = m.size();
            let diag = m
                .name()
                .map(|n| config::monitor_diag_inches(&n))
                .unwrap_or(0);
            (s.width, s.height, diag)
        }
        None => (0, 0, 0),
    }
}

/// Outer layout for the current frame: the flex engine result plus the
/// per-screen override panels (before padding).
fn outer_layout(
    def: &config::LayoutDef,
    active: Option<&config::ResOverride>,
    w: f32,
    h: f32,
    pad: f32,
) -> widgets::Layout {
    let mut l = flex::compute(w, h, &def.base, pad);
    if let Some(ov) = active {
        for (p, ps) in &ov.panels {
            l.set(
                *p,
                widgets::Rect::new(
                    ps.x / 100.0 * w,
                    ps.y / 100.0 * h,
                    ps.w / 100.0 * w,
                    ps.h / 100.0 * h,
                ),
            );
        }
    }
    l
}

/// Saves the layout edited in the grid editor and applies it live.
/// `select` = also make it the selected layout (SAVE AS); a plain SAVE
/// keeps the current selection. Only the CHANGED panels are written,
/// into the section of the current screen (resolution + diagonal).
#[allow(clippy::too_many_arguments)]
fn editor_save(
    editor: &mut widgets::editor::Editor,
    name: &str,
    select: bool,
    layout_spec: &mut config::LayoutDef,
    active_ov: &mut Option<config::ResOverride>,
    popup: &mut widgets::popup::Popup,
    key: (u32, u32, u32),
) {
    if name.is_empty() {
        return;
    }
    // SAVE AS writes ALL panels as the base of the (new) file; SAVE
    // rewrites the base on its own screen or stores only the changes in
    // the section of the current screen.
    let result = if select {
        config::save_layaut_full(name, &editor.spec(), key)
    } else {
        config::save_layaut_overrides(
            name,
            key,
            &editor.changes_since_start(),
            &editor.spec(),
        )
    };
    if let Err(e) = result {
        nacelle::sound::emit(nacelle::sound::Event::Error);
        popup.show(format!("Cannot save layout '{name}': {e}"));
        return;
    }
    if select || config::current_layaut_name().is_none() {
        config::select_layaut(name);
    }
    let (new_cfg, warn) = config::resolve();
    // Sizes travel with the layout, so a new layout brings its own.
    nacelle::base::set_panel_sizes(&new_cfg.layout.sizes);
    *layout_spec = new_cfg.layout;
    *active_ov = layout_spec.pick(key).cloned();
    if let Some(wmsg) = warn {
        nacelle::sound::emit(nacelle::sound::Event::Alert);
        popup.show(wmsg);
    } else {
        nacelle::sound::emit(nacelle::sound::Event::Save);
    }
    editor.stop();
}

/// A small dialog window shown INSTEAD of the program when the monitor
/// resolution is below the 1280x720 minimum. OK, Enter/Escape or closing
/// the window quits.
fn run_resolution_dialog(
    event_loop: winit::event_loop::EventLoop<()>,
    mut fonts: font::FontSystem,
    mw: u32,
    mh: u32,
) {
    let window = WindowBuilder::new()
        .with_title("nacelle-desktop")
        .with_inner_size(winit::dpi::LogicalSize::new(640.0, 200.0))
        .with_resizable(false)
        .build(&event_loop)
        .expect("cannot create window");
    let mut gfx = nacelle_renderer::Gfx::new(&window, window.inner_size().width, window.inner_size().height);
    let mut dl = draw::DrawList::new();
    let mut mouse = (0.0f32, 0.0f32);

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        gfx.resize();
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        mouse = (position.x as f32, position.y as f32);
                        window.request_redraw();
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let size = window.inner_size();
                        let ok = widgets::popup::resolution_dialog_ok_rect(
                            size.width as f32,
                            size.height as f32,
                        );
                        if ok.contains(mouse.0, mouse.1) {
                            elwt.exit();
                        }
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if key_event.state == ElementState::Pressed
                            && matches!(
                                key_event.logical_key,
                                Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Enter)
                            )
                        {
                            elwt.exit();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let size = window.inner_size();
                        let (w, h) = (size.width as f32, size.height as f32);
                        fonts.begin_frame();
                        dl.clear();
                        let mut ctx = widgets::Ctx {
                            dl: &mut dl,
                            fonts: &mut fonts,
                            w,
                            h,
                            t: 0.0,
                            mouse,
                            term_font_scale: 1.0,
                            ui_font_scale: 1.0,
                            panel_scale: 1.0,
                            // The resolution dialog never gains focus
                            // code (F1 §1.5): two keys, one button.
                            focus: None,
                            // Nor a tooltip: nothing on it is trimmed.
                            tips: None,
                        };
                        widgets::popup::draw_resolution_dialog(&mut ctx, mw, mh);
                        // Only the touched rows travel — a glyph-churn frame
                        // re-uploads a shelf, not the whole four megabytes.
                        let atlas_rows = fonts.take_dirty_rows();
                        let fsize = window.inner_size();
                        // Same bed as the main loop's clear (surface.void,
                        // alpha forced to 1.0 by the master).
                        static CLEAR: OnceLock<TokenId> = OnceLock::new();
                        let clear = nacelle::theme::resolved()
                            .bed(tok(&CLEAR, "surface.void"));
                        gfx.render(
                            fsize.width,
                            fsize.height,
                            &dl.verts,
                            &dl.runs,
                            atlas_rows.map(|(y0, rows)| (fonts.atlas.as_slice(), y0, rows)),
                            [clear.r, clear.g, clear.b, 1.0],
                        );
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("event loop ended with an error");
}

/// Mapping of physical keys to terminal sequences.
/// The winit event, translated to the neutral key set the F1 shortcut
/// registry matches on ([`nacelle::focus::KeyEv`]). Only chords route
/// through it, so the map covers what a chord can spell; a key with no
/// neutral name simply matches no binding.
fn focus_key_ev(key: &Key, mods: ModifiersState) -> Option<nacelle::focus::KeyEv> {
    use nacelle::focus::{Key as FKey, KeyEv, Mods};
    let mut m = Mods::NONE;
    if mods.control_key() {
        m = m | Mods::CTRL;
    }
    if mods.shift_key() {
        m = m | Mods::SHIFT;
    }
    if mods.alt_key() {
        m = m | Mods::ALT;
    }
    if mods.super_key() {
        m = m | Mods::SUPER;
    }
    let k = match key {
        Key::Character(s) => FKey::Char(s.chars().next()?),
        Key::Named(n) => match n {
            NamedKey::Enter => FKey::Enter,
            NamedKey::Escape => FKey::Escape,
            NamedKey::Tab => FKey::Tab,
            NamedKey::Backspace => FKey::Backspace,
            NamedKey::Delete => FKey::Delete,
            NamedKey::Space => FKey::Space,
            NamedKey::ArrowLeft => FKey::Left,
            NamedKey::ArrowRight => FKey::Right,
            NamedKey::ArrowUp => FKey::Up,
            NamedKey::ArrowDown => FKey::Down,
            NamedKey::Home => FKey::Home,
            NamedKey::End => FKey::End,
            NamedKey::PageUp => FKey::PageUp,
            NamedKey::PageDown => FKey::PageDown,
            NamedKey::Insert => FKey::Insert,
            NamedKey::ContextMenu => FKey::Menu,
            NamedKey::F1 => FKey::F(1),
            NamedKey::F2 => FKey::F(2),
            NamedKey::F3 => FKey::F(3),
            NamedKey::F4 => FKey::F(4),
            NamedKey::F5 => FKey::F(5),
            NamedKey::F6 => FKey::F(6),
            NamedKey::F7 => FKey::F(7),
            NamedKey::F8 => FKey::F(8),
            NamedKey::F9 => FKey::F(9),
            NamedKey::F10 => FKey::F(10),
            NamedKey::F11 => FKey::F(11),
            NamedKey::F12 => FKey::F(12),
            _ => return None,
        },
        _ => return None,
    };
    Some(KeyEv { key: k, mods: m, repeat: false, text: None })
}

fn key_to_bytes(key: &Key, mods: ModifiersState, app_cursor: bool) -> Option<Vec<u8>> {
    let esc: u8 = 0x1b;
    match key {
        Key::Character(s) => {
            let text = s.as_str();
            if mods.control_key() {
                if let Some(c) = text.chars().next() {
                    let lc = c.to_ascii_lowercase();
                    if lc.is_ascii_alphabetic() || "[\\]^_@".contains(lc) {
                        let mut out = Vec::new();
                        if mods.alt_key() {
                            out.push(esc);
                        }
                        out.push((lc as u8) & 0x1f);
                        return Some(out);
                    }
                }
                return None;
            }
            let mut out = Vec::new();
            if mods.alt_key() {
                out.push(esc);
            }
            out.extend_from_slice(text.as_bytes());
            Some(out)
        }
        Key::Named(n) => {
            let arrows = |ch: u8| -> Vec<u8> {
                if app_cursor {
                    vec![esc, b'O', ch]
                } else {
                    vec![esc, b'[', ch]
                }
            };
            let seq: Vec<u8> = match n {
                NamedKey::Enter => vec![b'\r'],
                NamedKey::Backspace => vec![0x7f],
                NamedKey::Tab => vec![b'\t'],
                NamedKey::Escape => vec![esc],
                NamedKey::Space => vec![b' '],
                NamedKey::ArrowUp => arrows(b'A'),
                NamedKey::ArrowDown => arrows(b'B'),
                NamedKey::ArrowRight => arrows(b'C'),
                NamedKey::ArrowLeft => arrows(b'D'),
                NamedKey::Home => arrows(b'H'),
                NamedKey::End => arrows(b'F'),
                NamedKey::PageUp => b"\x1b[5~".to_vec(),
                NamedKey::PageDown => b"\x1b[6~".to_vec(),
                NamedKey::Insert => b"\x1b[2~".to_vec(),
                NamedKey::Delete => b"\x1b[3~".to_vec(),
                NamedKey::F1 => b"\x1bOP".to_vec(),
                NamedKey::F2 => b"\x1bOQ".to_vec(),
                NamedKey::F3 => b"\x1bOR".to_vec(),
                NamedKey::F4 => b"\x1bOS".to_vec(),
                NamedKey::F5 => b"\x1b[15~".to_vec(),
                NamedKey::F6 => b"\x1b[17~".to_vec(),
                NamedKey::F7 => b"\x1b[18~".to_vec(),
                NamedKey::F8 => b"\x1b[19~".to_vec(),
                NamedKey::F9 => b"\x1b[20~".to_vec(),
                NamedKey::F10 => b"\x1b[21~".to_vec(),
                NamedKey::F11 => return None,
                NamedKey::F12 => b"\x1b[24~".to_vec(),
                _ => return None,
            };
            let mut out = Vec::new();
            if mods.alt_key() && *n != NamedKey::Escape {
                out.push(esc);
            }
            out.extend_from_slice(&seq);
            Some(out)
        }
        _ => None,
    }
}


