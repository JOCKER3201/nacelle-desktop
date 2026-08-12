//! nacelle-desktop — an independent sci-fi terminal inspired by eDEX-UI, in Rust + Vulkan.
//! Left column with telemetry, central terminal, right column with network
//! and files, on-screen keyboard and control panel at the bottom.

mod audio;
mod clipboard;
mod config;
mod plugins;
mod pty;
mod screen;
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

use crate::screen::{draw_panel, Cube, Screen};
use nacelle::layout::{BoardId, InstanceId};
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

/// The user's say over the picture, gathered so one frame can be handed
/// all of it. Per PROCESS and not per screen: a font scale is a
/// preference about reading, not about a monitor.
#[derive(Clone, Copy)]
struct Prefs {
    term_font_scale: f32,
    ui_font_scale: f32,
    /// The user's scale over the alpha of the theme's own glass wash.
    frost_wash: f32,
    /// The clock this frame is told it is (virtual while the pixel
    /// guard is armed).
    t: f64,
}

fn main() {
    // The pixel guard, before anything is built: `cmds` needs the
    // toolkit's command register switched on, and a draw list made
    // while it was off records nothing. Off, this reads one env var.
    hashframe::arm();
    // The two layout aids run and leave before any window exists —
    // they are for the user whose layout is broken enough that the
    // settings window may not be reachable (u1 §5.3).
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Desktop mode: this process IS the desktop (nacelle-session
    // starts it so), which puts a full desktop on EVERY screen —
    // each with the layaut its connector is assigned. Without the flag
    // the program is a guest on somebody's desktop and takes one
    // window.
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
    let (_cfg, startup_warning) = config::load();
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

    // ---- the screens ---------------------------------------------------
    // ONE MONITOR IS ONE ROOM. Desktop mode gives every screen a window
    // of its own and a full desktop behind it: its own layaut (chosen by
    // the connector it hangs off), its own boards, its own widgets. The
    // main screen is no longer a special case — it is simply the first
    // element of this list, and everything below indexes into it.
    //
    // Without the flag the program is a guest and takes one window. The
    // one special case there is a CHASSIS screen — a panel of ten inches
    // or less built into a computer case, which this program makes a
    // fine face for; None keeps winit's own choice, the current monitor.
    let survey = screens::survey(&event_loop);
    let mut screens: Vec<Screen> = if desktop_mode {
        survey
            .iter()
            .filter_map(|s| {
                Screen::new(
                    &event_loop,
                    Some(s.monitor.clone()),
                    s.connector.clone(),
                    s.primary,
                )
            })
            .collect()
    } else {
        let chassis = screens::chassis(&survey).map(|s| {
            eprintln!(
                "nacelle-desktop: chassis screen '{}' ({:.1}\") \u{2014} opening there",
                s.monitor.name().unwrap_or_else(|| "?".into()),
                s.diagonal_in.unwrap_or(0.0)
            );
            s
        });
        let (monitor, connector) = match chassis {
            Some(s) => (Some(s.monitor.clone()), s.connector.clone()),
            None => (None, None),
        };
        Screen::new(&event_loop, monitor, connector, true)
            .into_iter()
            .collect()
    };
    if screens.is_empty() {
        eprintln!("nacelle-desktop: no screen came up — nothing to draw on");
        return;
    }
    for sc in screens.iter_mut() {
        sc.pad = ui_padding;
    }
    for sc in screens.iter() {
        let (w, h) = sc.size();
        eprintln!(
            "nacelle-desktop: screen '{}' {}x{} on layaut '{}', {} placements on its \
             boards, {} of them running ({} widgets registered)",
            sc.connector.as_deref().unwrap_or("?"),
            w as u32,
            h as u32,
            sc.layaut,
            sc.widgets.len(),
            sc.widgets.running(),
            widgets::panel_count()
        );
    }

    // The theme engine bakes every u-derived length from the window
    // height (u = clamp(h × metric.unit_pct_h, …) — §2.2). config::load
    // ran before a window existed, so the engine is still on its
    // 1080-line default; on a 800-line window that is a 35 % oversize on
    // every metric token, which is how the control buttons left their
    // panel. Told at the top of every frame, for the screen about to be
    // drawn, and deduplicated inside on a height that lands on the same
    // u — which is what lets two screens of two heights each get their
    // own bake without either paying for the other's.
    nacelle::theme::set_viewport(screens[0].size().1, 1.0);

    // The colour pipeline: only a native Wayland session has a
    // compositor to discuss colour with. Everywhere else this stays
    // None, the COLOR settings are greyed out and their stored values
    // are never read. Discussed through the FIRST screen's surface:
    // colour management is a property of the session, not of a monitor.
    let mut color_mgr = {
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
        let dh = screens[0].window.display_handle().ok().map(|h| h.as_raw());
        let wh = screens[0].window.window_handle().ok().map(|h| h.as_raw());
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
    // Applied on start and after every change in the COLOR view. Every
    // screen shows the same picture, so every screen's renderer is told.
    macro_rules! apply_color {
        () => {{
            if let Some(mgr) = color_mgr.as_mut() {
                let prefs = config::color_prefs();
                let lut = prefs
                    .lut
                    .as_deref()
                    .and_then(|name| config::color_file_path("lut", name))
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .and_then(|text| nacelle_renderer::parse_cube(&text));
                if prefs.lut.is_some() && lut.is_none() {
                    eprintln!("nacelle-desktop: the chosen .cube did not parse — no grading");
                }
                for sc in screens.iter_mut() {
                    sc.set_color_depth(prefs.depth);
                    sc.set_lut(lut.clone());
                }
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
    let mut fullscreen = if gamescope {
        fullscreen::Fullscreen::start(&screens[0].window)
    } else {
        None
    };
    if fullscreen.is_some() {
        eprintln!("nacelle-desktop: gamescope clients go fullscreen");
    }

    // Which screen the application's own interface is on — the settings
    // window, the popup, the context menu, the focus chain. It follows
    // the hand from screen to screen; with one screen it is always 0,
    // which is what it always was.
    let mut ui_screen: usize = 0;

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

    let mut settings = widgets::settings::Settings::new();
    settings.color_enabled = color_mgr.is_some();
    // Frosted-glass preferences: the radius goes to the renderer, the
    // opacity into the tint of every glass quad drawn this frame.
    let (blur_radius, blur_opacity) = config::blur_prefs();
    for sc in screens.iter_mut() {
        sc.set_blur_radius(blur_radius);
    }
    // The theme's lens: glyph-coverage exponent and the blur pyramid's clear.
    // Re-applied on every configuration change beside the rest of the theme.
    macro_rules! apply_lens {
        () => {{
            let t = nacelle::theme::resolved();
            if let Some(id) = nacelle::theme::id("render.text_gamma") {
                let g = t.px(id);
                for sc in screens.iter_mut() {
                    sc.set_text_gamma(g);
                }
            }
        }};
    }
    apply_lens!();
    // The glass itself is always fully frosted — RADIUS is the only
    // say over how blurred it is. OPACITY is the user's scale over the
    // alpha of the theme's own wash (elev.fixture.glass.wash): nothing
    // at 0 %, the wash exactly as the theme wrote it at 100 %.
    let mut frost_wash = blur_opacity as f32 / 100.0;
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
    // rewritten behind the user's back (u1 §5.3).
    //
    // Said to the LOG and nowhere else. It used to raise a window and
    // an alert sound at every start, which was right when a stale
    // section was rare and wrong the moment it stopped being: installing
    // an addon changes what the default names, so anybody with a saved
    // arrangement met the same window every morning and learned to
    // dismiss it without reading. A notice nobody reads is worse than
    // silence, because it also trains the next one to be ignored. The
    // line survives for a headless start and for anybody wondering why
    // a new widget did not appear.
    if let Some((pinned, placed)) =
        config::stale_screen_section(screens[0].spec(), screens[0].key)
    {
        let key = screens[0].key;
        let lname = screens[0].layaut.clone();
        eprintln!(
            "nacelle-desktop: layaut '{lname}' pins {pinned} of {placed} panels for \
             {}x{}@{}; this screen keeps the saved arrangement. \
             Settings \u{2192} Themes \u{2192} Layauts \u{2192} RESET THIS SCREEN undoes it.",
            key.0, key.1, key.2
        );
    }

    let mut mods = ModifiersState::empty();
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
    // from whatever registers while drawing — on the screen hosting the
    // interface, which is the only screen anything registers on.
    let mut focus_ctl = nacelle::focus::FocusCtl::new();

    let start = Instant::now();

    // Every screen re-reads the layaut its connector is assigned and
    // brings its widgets into line with it. One call, so no path that
    // changes a layout can quietly forget a screen.
    macro_rules! reload_layauts {
        () => {{
            for sc in screens.iter_mut() {
                sc.pad = ui_padding;
                sc.reload_layaut();
            }
        }};
    }

    // Re-reads the configuration and applies everything it can change:
    // the theme, the layouts with their panel sizes, the sound clips
    // and the two fonts. Every path that alters a setting ends here, so
    // none of them can quietly skip a step — which is exactly what
    // happened while this was written out twice: only one copy reloaded
    // the sound theme, so choosing a new one did nothing until the next
    // restart.
    //
    // A macro rather than a function because all of these are locals of
    // the event loop, and threading a dozen &mut through a call would
    // be longer than the body.
    macro_rules! apply_config {
        () => {{
            let (new_cfg, warn) = config::resolve();
            // Sizes travel with the layout, so a new layout brings its own.
            nacelle::base::set_panel_sizes(&new_cfg.layout.sizes);
            // A new look or sound set means new clips.
            if let (Some(a), Some(dir)) = (audio.as_mut(), config::active_sounds_dir()) {
                a.load_theme(&dir);
            }
            if let Some(w) = warn {
                popup.show(w);
            }
            let (tscale, tfam, twgt) = config::term_font_prefs();
            let (uscale, ufam, uwgt) = config::ui_font_prefs();
            font_scale = tscale;
            ui_font_scale = uscale;
            let tkey = (
                tfam.clone().unwrap_or_default(),
                twgt.clone().unwrap_or_default(),
            );
            if tkey != last_term_key {
                last_term_key = tkey;
                if tfam.is_none() && twgt.is_none() {
                    fonts.set_mono(font::load_default_mono());
                } else if let Some(f) =
                    font::load_variant_for(tfam.as_deref(), twgt.as_deref(), false)
                {
                    fonts.set_mono(f);
                }
            }
            let ukey = (
                ufam.clone().unwrap_or_default(),
                uwgt.clone().unwrap_or_default(),
            );
            if ukey != last_ui_key {
                last_ui_key = ukey;
                if ufam.is_none() && uwgt.is_none() {
                    fonts.set_ui(font::load_default_ui());
                } else if let Some(f) =
                    font::load_variant_for(ufam.as_deref(), uwgt.as_deref(), true)
                {
                    fonts.set_ui(f);
                }
            }
            apply_lens!();
            reload_layauts!();
        }};
    }

    // Routes one drag phase to a placement's widget, in the content box
    // its last draw used — the same rect discipline as click and wheel.
    macro_rules! widget_drag {
        ($si:expr, $id:expr, $phase:expr, $x:expr, $y:expr) => {{
            let si: usize = $si;
            let id: InstanceId = $id;
            let sc = &mut screens[si];
            let (w, h) = sc.size();
            let fallback = sc.content_layout().of(id);
            let r = sc.widgets.content(id).unwrap_or(fallback);
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
                window: (w, h),
            };
            sc.widgets
                .get_mut(id)
                .map(|wg| wg.drag($phase, $x, $y, r, &host))
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
        ($si:expr, $entries:expr, $x:expr, $y:expr) => {{
            if screens[$si].cube.is_none() {
                ui_screen = $si;
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
        ($si:expr) => {{
            use nacelle::object::menu::{MenuEntry, MenuItem};
            let sc = &screens[$si];
            // Sideways only along the row — exactly the pan gesture's
            // reach; the vertical fixtures are not "left or right".
            let on_arm = sc.board.1 == 0;
            let left_ok = on_arm && sc.has_board((sc.board.0 - 1, sc.board.1));
            let right_ok = on_arm && sc.has_board((sc.board.0 + 1, sc.board.1));
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
        ($si:expr) => {{
            use nacelle::object::menu::{MenuEntry, MenuItem};
            let (has_sel, has_text) = screens[$si]
                .editor
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
    // SAVE while standing on a board: the board's panels go into that
    // screen's layaut file, and every screen showing it is re-read.
    macro_rules! save_board_on {
        ($si:expr) => {{
            let si: usize = $si;
            let name = screens[si].layaut.clone();
            let board = screens[si].editor.board();
            // The store takes the board's placements by identity and
            // drops whatever is no longer among them: a widget dragged
            // off the board leaves the file with it.
            let rects = screens[si].editor.rects();
            match config::set_board_in_layaut(&name, board, &rects) {
                Ok(()) => {
                    nacelle::sound::emit(nacelle::sound::Event::Save);
                    apply_config!();
                    // A finished save leaves the editor, on every board
                    // — the same ending HOME's own save has. Only the
                    // board differed, and the difference was invisible
                    // and therefore wrong: the user pressed SAVE and
                    // stayed in a mode they thought they had left.
                    screens[si].stop_editor();
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
    // What a picked row runs — the same commands the shortcut registry
    // names, so a chord and a menu row cannot drift apart. `$si` is the
    // screen the gesture came from: BOARD LEFT turns the monitor the
    // hand is on, not the first one in the list.
    macro_rules! run_menu_cmd {
        ($si:expr, $cmd:expr) => {{
            let si: usize = $si;
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
                    if !screens[si].editor.active {
                        screens[si].enter_editor();
                    }
                }
                CMD_OPEN_SETTINGS => settings.show(),
                CMD_BOARD_LEFT | CMD_BOARD_RIGHT => {
                    let here = screens[si].board;
                    let target = if $cmd == CMD_BOARD_LEFT {
                        (here.0 - 1, here.1)
                    } else {
                        (here.0 + 1, here.1)
                    };
                    if here.1 == 0
                        && screens[si].cube.is_none()
                        && screens[si].has_board(target)
                    {
                        screens[si].step_to(target);
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
                    let out = screens[si]
                        .editor
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
                                if let Some(m) = screens[si].editor.naming.as_mut() {
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
    // a control whose consequences only the mouse path knows. It acts
    // on the screen the window is being shown on: RESET THIS SCREEN
    // means THIS one, and going to a board turns the monitor in front
    // of the user.
    macro_rules! settings_after {
        () => {{
            let si = ui_screen.min(screens.len() - 1);
            // RESET THIS SCREEN (the LAYAUTS view): the window cannot
            // clear the section itself — only the application knows
            // which screen this is — so it asks, like the boards do.
            if settings.reset_screen {
                settings.reset_screen = false;
                let name = screens[si].layaut.clone();
                let key = screens[si].key;
                match config::clear_screen_section(&name, key) {
                    Ok(()) => {
                        nacelle::sound::emit(nacelle::sound::Event::Save);
                        popup.show(format!(
                            "Cleared the {}x{}@{} section of layaut '{}'",
                            key.0, key.1, key.2, name
                        ));
                    }
                    Err(e) => {
                        nacelle::sound::emit(nacelle::sound::Event::Error);
                        popup.show(format!("Cannot reset this screen: {e}"));
                    }
                }
                apply_config!();
            }
            // The BOARDS view asks; the boards are the screen's, so the
            // answers live here.
            if let Some(act) = settings.board_action.take() {
                use widgets::settings::BoardAction;
                match act {
                    BoardAction::Go(k) => {
                        // The whole point of going from here: the
                        // window stays open, so no board needs its own
                        // control panel to come back.
                        if !screens[si].editor.active
                            && screens[si].cube.is_none()
                            && k != nacelle::layout::board_key(screens[si].board)
                            && screens[si].has_board(k)
                        {
                            // A distant board is walked one neighbour
                            // at a time, so the move animates through
                            // every board between — the cube sideways,
                            // the slide up and down.
                            let mut steps: Vec<BoardId> = Vec::new();
                            let mut at = screens[si].board;
                            if k.1 != 0 {
                                // Top or bottom: y is the whole
                                // journey, through the row if coming
                                // from the other one.
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
                                screens[si].step_to(first);
                                screens[si].go_queue = steps;
                            }
                        }
                    }
                    BoardAction::Add(side) => {
                        let name = screens[si].layaut.clone();
                        if let Err(e) = config::add_board_in_layaut(&name, side) {
                            popup.show(format!("Cannot add a board: {e}"));
                        }
                        apply_config!();
                    }
                    BoardAction::Del(k) => {
                        // Only the row shrinks; home and the top and
                        // bottom boards are fixtures.
                        if k != (0, 0) && k.1 == 0 && screens[si].cube.is_none() {
                            let name = screens[si].layaut.clone();
                            if let Err(e) = config::remove_board_in_layaut(&name, k) {
                                popup.show(format!("Cannot remove the board: {e}"));
                            }
                            // Whoever stood on a moved board follows it;
                            // whoever stood on the removed one — or came
                            // up or down from it — lands over home. Every
                            // screen showing this layaut is walked, not
                            // just the one that asked.
                            for sc in screens.iter_mut() {
                                if sc.layaut != name {
                                    continue;
                                }
                                if sc.board.0 == k.0 {
                                    sc.board.0 = 0;
                                } else if k.0 > 0 && sc.board.0 > k.0 {
                                    sc.board.0 -= 1;
                                } else if k.0 < 0 && sc.board.0 < k.0 {
                                    sc.board.0 += 1;
                                }
                            }
                            apply_config!();
                        }
                    }
                }
            }
            // EDIT GRID: hide settings, enter the editor with the
            // current panel rectangles — the body is shared with the
            // panel context menu's EDIT LAYOUT row.
            if settings.edit_requested {
                settings.edit_requested = false;
                if !screens[si].editor.active {
                    screens[si].enter_editor();
                }
                // With the editor already running the window simply
                // hides — back to the grid.
            }
            let (snap, cols, rows, pad) = config::grid_prefs();
            for sc in screens.iter_mut() {
                if sc.editor.active {
                    let (w, h) = sc.size();
                    sc.editor.sync_prefs(snap, cols, rows, pad as f32, w, h);
                }
            }
        }};
    }

    // How often the world is redrawn. Nothing here is a game: the
    // clock ticks once a second, telemetry once a second, the terminal
    // as fast as a person types. Drawing at whatever the display can
    // take — 240 Hz on this machine — spent most of a core rebuilding
    // an image that had not changed. Sixty is smooth for the board
    // animations (a transition is eighteen frames) and costs a quarter
    // of that.
    const FRAME: std::time::Duration = std::time::Duration::from_nanos(1_000_000_000 / 60);
    let mut next_frame = Instant::now();

    event_loop
        .run(move |event, elwt| {
            match event {
                // Every window event is answered by the SCREEN whose
                // window received it. That is the whole of "input goes
                // where it was aimed": a drag on the second monitor
                // turns the second monitor's boards, and the layout
                // editor is operated against the pixels it is drawn on.
                Event::WindowEvent { window_id, event } => {
                    let Some(si) = screens.iter().position(|s| s.window.id() == window_id)
                    else {
                        return;
                    };
                    match event {
                    WindowEvent::CloseRequested => {
                        // A screen of the desktop is not a document
                        // window: closing one closes nothing. The
                        // FIRST screen is the program, and the
                        // compositor asking it to go is obeyed.
                        if si == 0 {
                            eprintln!("nacelle-desktop: compositor requested window close");
                            elwt.exit();
                        } else {
                            eprintln!("nacelle-desktop: close request on a second screen ignored");
                        }
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        screens[si].resized();
                    }
                    WindowEvent::ModifiersChanged(m) => mods = m.state(),
                    WindowEvent::CursorMoved { position, .. } => {
                        screens[si].mouse = (position.x as f32, position.y as f32);
                        let mouse = screens[si].mouse;
                        // The application's own interface follows the
                        // hand from screen to screen: the settings
                        // window, the popup and the menu are drawn
                        // wherever the pointer is. The KEYBOARD does
                        // not follow — the state they act on is one and
                        // the same wherever it is drawn.
                        ui_screen = si;
                        // An accepted drag capture owns the pointer:
                        // every motion goes to the widget as drag(Move)
                        // and nothing below (board pan, editor hover)
                        // sees it — the single capture path.
                        if let Some(id) = screens[si].drag_capture {
                            if let widgets::Action::TermSelect { op, col, row, base } =
                                widget_drag!(si, id, widgets::DragPhase::Move, mouse.0, mouse.1)
                            {
                                apply_term_select!(op, col, row, base);
                            }
                            return;
                        }
                        // A held button that travels sideways becomes a
                        // board drag; the click it started as is then
                        // never delivered.
                        if let Some((px0, py0)) = screens[si].press_at {
                            if screens[si].pan.is_none() && screens[si].cube.is_none() {
                                let (w, _) = screens[si].size();
                                let (dx, dy) = (mouse.0 - px0, mouse.1 - py0);
                                let th = (w * 0.02).max(20.0);
                                // The axis is decided once, by whichever
                                // way the hand went first, and the drag
                                // stays on it.
                                if dx.abs() > th && dx.abs() > dy.abs() {
                                    screens[si].pan = Some((true, 0.0));
                                } else if dy.abs() > th && dy.abs() > dx.abs() {
                                    screens[si].pan = Some((false, 0.0));
                                }
                            }
                            if let Some((horizontal, _)) = screens[si].pan {
                                let (w, h) = screens[si].size();
                                let here = screens[si].board;
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
                                    let target =
                                        (here.0 + if raw > 0.0 { 1 } else { -1 }, here.1);
                                    let a = if here.1 == 0 && screens[si].has_board(target)
                                    {
                                        raw.clamp(-90.0, 90.0)
                                    } else {
                                        // No board that way: a short
                                        // rubbery give, so the edge
                                        // answers the hand without
                                        // pretending to go anywhere.
                                        (raw * gain).clamp(-90.0 * rmax, 90.0 * rmax)
                                    };
                                    screens[si].pan = Some((true, a));
                                } else {
                                    // Up and down the world slides flat;
                                    // dragging up goes to the board below.
                                    // The top and bottom boards sit above
                                    // and below every board on the row,
                                    // so this works from anywhere.
                                    let raw = -(mouse.1 - py0) / (h * gest);
                                    let target =
                                        (here.0, here.1 + if raw > 0.0 { 1 } else { -1 });
                                    let f = if screens[si].has_board(target) {
                                        raw.clamp(-1.0, 1.0)
                                    } else {
                                        (raw * gain).clamp(-rmax, rmax)
                                    };
                                    screens[si].pan = Some((false, f));
                                }
                                return;
                            }
                        }
                        if screens[si].editor.active && !settings.open {
                            let (fw, fh) = screens[si].size();
                            screens[si].editor.mouse_move(mouse.0, mouse.1, fw, fh);
                            // Move/resize cursors over the panels.
                            use widgets::editor::CursorKind;
                            let kind = screens[si].editor.cursor_at(mouse.0, mouse.1, fw, fh);
                            screens[si].window.set_cursor_icon(match kind {
                                CursorKind::Move => CursorIcon::Grab,
                                CursorKind::Ew => CursorIcon::EwResize,
                                CursorKind::Ns => CursorIcon::NsResize,
                                CursorKind::Nwse => CursorIcon::NwseResize,
                                CursorKind::Nesw => CursorIcon::NeswResize,
                                CursorKind::Normal => CursorIcon::Default,
                            });
                            return;
                        }
                        if settings.open {
                            settings.drag(mouse.0);
                        }
                        // Pointer cursor over a widget's own controls.
                        let (w, h) = screens[si].size();
                        let layout = screens[si].content_layout();
                        let pointer = if settings.open {
                            settings.hover(mouse.0, mouse.1)
                        } else {
                            // The widget under the pointer is the only
                            // one that knows where its controls are, so
                            // it is asked — in the same content box it
                            // drew in and will be clicked in. The
                            // application holds no copy of anybody's
                            // geometry and needs no widget's name.
                            match layout.hit(mouse.0, mouse.1) {
                                Some(pl) => {
                                    let r = screens[si]
                                        .widgets
                                        .content(pl.id)
                                        .unwrap_or(pl.rect);
                                    screens[si]
                                        .widgets
                                        .get_mut(pl.id)
                                        .is_some_and(|wg| {
                                            wg.pointer(mouse.0, mouse.1, r, (w, h))
                                        })
                                }
                                None => false,
                            }
                        };
                        screens[si].window.set_cursor_icon(if pointer {
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
                        if screens[si].editor.active {
                            return;
                        }
                        let dy = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.0,
                        };
                        let mouse = screens[si].mouse;
                        let (w, h) = screens[si].size();
                        let layout = screens[si].content_layout();
                        let Some(pl) = layout.hit(mouse.0, mouse.1) else { return };
                        // The rect the widget is answered in is the
                        // CONTENT BOX its last draw used (u2 §4.1),
                        // never the panel rect — the container's
                        // band and padding are the host's, not the
                        // widget's.
                        let r = screens[si].widgets.content(pl.id).unwrap_or(pl.rect);
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
                                window: (w, h),
                            };
                            screens[si]
                                .widgets
                                .get_mut(pl.id)
                                .map(|wg| wg.wheel(dy, r, &host))
                                .unwrap_or(widgets::Action::None)
                        };
                        if let widgets::Action::ScrollTerminal(n) = action {
                            if let Some(s) = sessions[active].as_mut() {
                                s.term.scroll_view(n);
                            }
                        }
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let mouse = screens[si].mouse;
                        // A captured drag ends here, and the release is
                        // the widget's drag(End) — copy-on-select fires
                        // in the TermSelect handler, on End only. The
                        // capture never coexists with the editor or the
                        // settings window: it only ever starts when
                        // neither had the press.
                        if let Some(id) = screens[si].drag_capture.take() {
                            if let widgets::Action::TermSelect { op, col, row, base } =
                                widget_drag!(si, id, widgets::DragPhase::End, mouse.0, mouse.1)
                            {
                                apply_term_select!(op, col, row, base);
                            }
                            return;
                        }
                        if screens[si].editor.active && !settings.open {
                            screens[si].editor.mouse_up();
                            return;
                        }
                        if screens[si].editor.active && settings.open {
                            settings.release();
                            let (snap, cols, rows, pad) = config::grid_prefs();
                            let (w, h) = screens[si].size();
                            screens[si]
                                .editor
                                .sync_prefs(snap, cols, rows, pad as f32, w, h);
                            ui_padding = pad as f32;
                            for sc in screens.iter_mut() {
                                sc.pad = ui_padding;
                            }
                            return;
                        }
                        if settings.open && settings.release() {
                            apply_config!();
                        }
                        // A drag ends: past the point of no return the
                        // turn completes to the neighbour, short of it
                        // the world settles back.
                        if let Some((horizontal, a)) = screens[si].pan.take() {
                            screens[si].press_at = None;
                            let here = screens[si].board;
                            let sign: i32 = if a > 0.0 { 1 } else { -1 };
                            let target = if horizontal {
                                (here.0 + sign, here.1)
                            } else {
                                (here.0, here.1 + sign)
                            };
                            // Sideways only along the row; up and down
                            // reach the top and bottom from anywhere.
                            let on_arm = if horizontal { here.1 == 0 } else { true };
                            let full: f32 = if horizontal { 90.0 } else { 1.0 };
                            let past = a.abs() >= full / 3.0;
                            if past && on_arm && screens[si].has_board(target) {
                                nacelle::sound::emit(nacelle::sound::Event::Snap);
                                screens[si].cube = Some(Cube {
                                    horizontal,
                                    a0: a,
                                    a1: full * sign as f32,
                                    t0: Instant::now(),
                                    to: target,
                                    face_b: target,
                                });
                            } else if a.abs() > full * 0.001 {
                                screens[si].cube = Some(Cube {
                                    horizontal,
                                    a0: a,
                                    a1: 0.0,
                                    t0: Instant::now(),
                                    to: here,
                                    face_b: target,
                                });
                            }
                            return;
                        }
                        let Some((cx, cy)) = screens[si].press_at.take() else { return };
                        // A click held in place: delivered now, to the
                        // widget it went down on. One route for every
                        // widget — the application does not know which
                        // one it is talking to.
                        let (w, h) = screens[si].size();
                        let layout = screens[si].content_layout();
                        let Some(pl) = layout.hit(cx, cy) else { return };
                        // The content box the widget drew in — the same
                        // rect its draw received (u2 §4.1).
                        let r = screens[si].widgets.content(pl.id).unwrap_or(pl.rect);
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
                                window: (w, h),
                            };
                            screens[si]
                                .widgets
                                .get_mut(pl.id)
                                .map(|wg| wg.click(cx, cy, r, &host))
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
                        let mouse = screens[si].mouse;
                        ui_screen = si;
                        // Any pointer press hides the focus ring (the
                        // focus-visible rule, F1 §1.2) — focus itself
                        // stays wherever it was.
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        // Mid-turn the boards are nowhere in particular;
                        // the press waits for the world to settle.
                        if screens[si].cube.is_some() {
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
                                    run_menu_cmd!(si, cmd);
                                }
                                MenuOut::None => {}
                            }
                            return;
                        }
                        let (w, h) = screens[si].size();
                        // A click on the warning popup dismisses it. The
                        // toaster answers against the box it drew, so it
                        // needs the point and nothing else.
                        if popup.click(mouse.0, mouse.1) {
                            return;
                        }
                        // The layout editor captures all clicks while active
                        // (unless the settings window is open over it).
                        if screens[si].editor.active && !settings.open {
                            match screens[si].editor.mouse_down(mouse.0, mouse.1, w, h) {
                                widgets::editor::EditorHit::Save => {
                                    if screens[si].board != (0, 0) {
                                        save_board_on!(si);
                                        return;
                                    }
                                    // Overwrite this screen's layaut —
                                    // only the changes, for this screen.
                                    let name = screens[si].layaut.clone();
                                    editor_save(&mut screens[si], &name, false, &mut popup);
                                    reload_layauts!();
                                }
                                widgets::editor::EditorHit::SaveAs => {
                                    // A board is a place, not a style: it
                                    // has no name to save as, so the same
                                    // save answers both buttons there.
                                    if screens[si].board != (0, 0) {
                                        save_board_on!(si);
                                        return;
                                    }
                                    screens[si].editor.begin_naming(&mut focus_ctl);
                                }
                                widgets::editor::EditorHit::Exit => {
                                    // Back to the settings window, GRID view.
                                    screens[si].stop_editor();
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
                            if screens[si].editor.active {
                                let hit =
                                    screens[si].editor.buttons_hit(mouse.0, mouse.1, w, h);
                                if let Some(hit) = hit {
                                    match hit {
                                        widgets::editor::EditorHit::Settings => {
                                            // Toggle: hide the window.
                                            settings.close();
                                        }
                                        widgets::editor::EditorHit::Save => {
                                            if screens[si].board != (0, 0) {
                                                save_board_on!(si);
                                                return;
                                            }
                                            let name = screens[si].layaut.clone();
                                            editor_save(
                                                &mut screens[si],
                                                &name,
                                                false,
                                                &mut popup,
                                            );
                                            reload_layauts!();
                                        }
                                        widgets::editor::EditorHit::SaveAs => {
                                            if screens[si].board != (0, 0) {
                                                save_board_on!(si);
                                                return;
                                            }
                                            settings.close();
                                            screens[si]
                                                .editor
                                                .begin_naming(&mut focus_ctl);
                                        }
                                        widgets::editor::EditorHit::Exit => {
                                            screens[si].stop_editor();
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
                                w,
                                h,
                                Some(&mut focus_ctl),
                            ) {
                                apply_config!();
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
                        let layout = screens[si].content_layout();
                        if let Some(pl) = layout.hit(mouse.0, mouse.1) {
                            // The widget's own answer decides who owns
                            // the hand: anything but None takes the
                            // capture (the contract `Widget::drag`
                            // states), and the board never sees the
                            // gesture. A selection asks for something
                            // while it captures; a scroll thumb asks
                            // for nothing and says so with Capture.
                            match widget_drag!(
                                si,
                                pl.id,
                                widgets::DragPhase::Begin,
                                mouse.0,
                                mouse.1
                            ) {
                                widgets::Action::None => {}
                                widgets::Action::TermSelect { op, col, row, base } => {
                                    apply_term_select!(op, col, row, base);
                                    screens[si].drag_capture = Some(pl.id);
                                    return;
                                }
                                _ => {
                                    screens[si].drag_capture = Some(pl.id);
                                    return;
                                }
                            }
                        }
                        // The click is not delivered yet. Held and
                        // moved, it becomes a board drag; released where
                        // it went down, the widget under it gets it then.
                        screens[si].press_at = Some((mouse.0, mouse.1));
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
                        let mouse = screens[si].mouse;
                        ui_screen = si;
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        // An open menu is a grab: the press only closes
                        // it, exactly like a left press outside.
                        if menu.is_some() {
                            menu = None;
                            return;
                        }
                        if screens[si].cube.is_some()
                            || screens[si].editor.active
                            || settings.open
                        {
                            return;
                        }
                        let layout = screens[si].content_layout();
                        if screens[si]
                            .term_inst
                            .is_some_and(|id| layout.of(id).contains(mouse.0, mouse.1))
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
                        let mouse = screens[si].mouse;
                        ui_screen = si;
                        let f = focus_ctl.focused();
                        focus_ctl.focus(f);
                        menu = None;
                        if screens[si].cube.is_some() {
                            return;
                        }
                        // The SAVE AS field: the input object's menu.
                        // The editor is otherwise pointer-first and
                        // claims no other right-click in F1.
                        if screens[si].editor.active && !settings.open {
                            let over_field = screens[si].editor.naming.is_some()
                                && screens[si]
                                    .editor
                                    .naming_field
                                    .map_or(false, |r| r.contains(mouse.0, mouse.1));
                            if over_field {
                                open_menu_at!(si, input_menu_entries!(si), mouse.0, mouse.1);
                            }
                            return;
                        }
                        // The settings window claims its plane whole;
                        // no F1 menu opens over it (§4.2 names none).
                        if settings.open {
                            return;
                        }
                        let layout = screens[si].content_layout();
                        let Some(pl) = layout.hit(mouse.0, mouse.1) else { return };
                        // The content box the widget drew in (u2 §4.1);
                        // everything above it is the host's chrome —
                        // the title band and its padding.
                        let content =
                            screens[si].widgets.content(pl.id).unwrap_or(pl.rect);
                        if screens[si].term_inst == Some(pl.id)
                            && content.contains(mouse.0, mouse.1)
                        {
                            // Terminal panel: copy/paste/scrollback/tabs.
                            open_menu_at!(si, terminal_menu_entries!(), mouse.0, mouse.1);
                        } else if mouse.1 < content.y {
                            // The title band: the existing Actions,
                            // menu-shaped (edit layout, settings, the
                            // neighbour boards).
                            open_menu_at!(si, panel_menu_entries!(si), mouse.0, mouse.1);
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
                        if let Some(model) = screens[si].editor.naming.as_mut() {
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
                                        run_menu_cmd!(si, cmd);
                                    }
                                    MenuOut::None => {}
                                }
                            }
                            return;
                        }
                        // Layout editor: the SAVE AS prompt takes typing;
                        // otherwise ESC exits without saving. Nothing
                        // reaches the terminal.
                        if screens[si].editor.active && !settings.open {
                            if screens[si].editor.naming.is_some() {
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
                                        if let Some(r) = screens[si].editor.naming_field {
                                            open_menu_at!(
                                                si,
                                                input_menu_entries!(si),
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
                                let composing = screens[si]
                                    .editor
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
                                let out = screens[si]
                                    .editor
                                    .naming
                                    .as_mut()
                                    .map(|m| m.apply(msg))
                                    .unwrap_or(InputEdited::None);
                                match out {
                                    InputEdited::Submit => {
                                        let name = screens[si]
                                            .editor
                                            .naming
                                            .as_ref()
                                            .map(|m| m.value().to_string())
                                            .unwrap_or_default();
                                        if !name.is_empty() {
                                            editor_save(
                                                &mut screens[si],
                                                &name,
                                                true,
                                                &mut popup,
                                            );
                                            reload_layauts!();
                                        }
                                    }
                                    InputEdited::Cancel => {
                                        screens[si].editor.close_naming()
                                    }
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
                                            if let Some(m) =
                                                screens[si].editor.naming.as_mut()
                                            {
                                                m.apply(InputMsg::Insert(text));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            } else if let Key::Named(NamedKey::Escape) =
                                key_event.logical_key
                            {
                                screens[si].stop_editor();
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
                                        apply_config!();
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
                            let fs = screens[si].window.fullscreen();
                            screens[si].window.set_fullscreen(if fs.is_some() {
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
                                    // chain is wired into the boards yet
                                    // and the terminal owns the keyboard
                                    // at boot, so the focused control
                                    // IS the terminal view; its content
                                    // box stands in for `rect_of` until
                                    // the §1 desktop router lands.
                                    // Fallback to the pointer when no
                                    // widget on this board holds one.
                                    let r = screens[si]
                                        .term_inst
                                        .and_then(|id| screens[si].widgets.content(id));
                                    let (ax, ay) = r
                                        .map(|r| (r.x, r.y))
                                        .unwrap_or(screens[si].mouse);
                                    open_menu_at!(
                                        si,
                                        terminal_menu_entries!(),
                                        ax,
                                        ay
                                    );
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
                            // Announced to every widget of every screen:
                            // which of them draws an on-screen keyboard
                            // is not this program's business, and the
                            // ones that draw none ignore it (the
                            // interface's default is to do nothing).
                            for sc in screens.iter_mut() {
                                for wg in sc.widgets.each_mut() {
                                    wg.key_feedback(ch, label);
                                }
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
                        if screens[si].last_render.elapsed() < FRAME {
                            return;
                        }
                        screens[si].last_render = Instant::now();
                        if si == 0 {
                            if let Some(f) = fullscreen.as_mut() {
                                f.poll();
                            }
                        }
                        // Live preview of the size sliders while dragging.
                        if let Some((tscale, uscale)) = settings.live_scales() {
                            font_scale = tscale;
                            ui_font_scale = uscale;
                        }
                        // Live widget padding while the GRID view is open.
                        if let Some(p) = settings.live_padding() {
                            ui_padding = p as f32;
                            for sc in screens.iter_mut() {
                                sc.pad = ui_padding;
                            }
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

                        let (w, h) = screens[si].size();
                        if w < 8.0 || h < 8.0 {
                            return;
                        }
                        // Read in place, not copied. The snapshot carries
                        // the process table and every string in it, and
                        // cloning that for each frame was pure waste: the
                        // collector rewrites it once a second and can wait
                        // the few milliseconds a frame takes. Nothing
                        // below locks it again — that would deadlock.
                        let snap_held = sys.lock().unwrap();
                        let clock = hashframe::clock(start.elapsed().as_secs_f64());
                        let host = widgets::Host {
                            snap: &snap_held,
                            term: sessions[active].as_ref().map(|s| &s.term),
                            tabs: &occupied,
                            tab_active: active,
                            shell_cwd: shell_cwd.clone(),
                            // The SAME clock the draw context is given: a
                            // widget animating off `host.t` while the
                            // context ran on a virtual clock would put the
                            // machine's speed back into a frame the pixel
                            // guard is trying to compare.
                            t: clock,
                            window: (w, h),
                        };
                        let prefs = Prefs {
                            term_font_scale: font_scale,
                            ui_font_scale,
                            frost_wash,
                            t: clock,
                        };
                        // The application's own interface is drawn on
                        // ONE screen — the one the hand is on.
                        let hosts_ui = si == ui_screen;
                        let (grid_now, drained) = draw_screen(
                            &mut screens[si],
                            &mut fonts,
                            &host,
                            &mut settings,
                            &mut popup,
                            &mut menu,
                            &mut tips,
                            &mut focus_ctl,
                            prefs,
                            hosts_ui,
                            // The pixel guard watches the first screen:
                            // an armed run is a measurement of one
                            // picture, and the first screen is the one
                            // that always exists.
                            si == 0,
                        );
                        drop(snap_held);
                        // Every other screen's renderer holds its own
                        // copy of the glyph atlas; whatever this frame
                        // drained belongs to them too, or the next
                        // window to draw text would arrive with holes
                        // where this one took the rows.
                        if let Some((y0, rows)) = drained {
                            for (j, sc) in screens.iter_mut().enumerate() {
                                if j != si {
                                    sc.note_atlas_rows(y0, rows);
                                }
                            }
                        }
                        // The focus frame boundary (F1 §1.2): the chain
                        // this frame's draws registered becomes the one
                        // navigation walks — after the world has drawn,
                        // before the next frame's events, so Tab never
                        // sees a half-built chain. Only the screen that
                        // hosts the interface registers anything.
                        if hosts_ui {
                            focus_ctl.begin_frame();
                        }

                        // Fit all session grids to the panel size.
                        if let Some((cols, rows)) = grid_now {
                            if (cols, rows) != grid {
                                grid = (cols, rows);
                                for s in sessions.iter_mut().flatten() {
                                    s.term.resize(cols, rows);
                                    s.pty.resize(cols as u16, rows as u16);
                                }
                            }
                        }

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
                            for sc in screens.iter_mut() {
                                sc.set_blur_radius(radius);
                            }
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
                            let want_ime = screens[si].editor.naming.is_some();
                            if want_ime != ime_allowed {
                                ime_allowed = want_ime;
                                screens[si].window.set_ime_allowed(want_ime);
                                if want_ime {
                                    screens[si].window.set_ime_purpose(
                                        winit::window::ImePurpose::Normal,
                                    );
                                } else {
                                    ime_area = None;
                                }
                            }
                            if want_ime {
                                if let Some(cr) = screens[si].editor.naming_caret {
                                    let area = (
                                        cr.x as i32,
                                        cr.y as i32,
                                        cr.w.max(1.0) as i32,
                                        cr.h.max(1.0) as i32,
                                    );
                                    if ime_area != Some(area) {
                                        ime_area = Some(area);
                                        screens[si].window.set_ime_cursor_area(
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
                    }
                    _ => {}
                    }
                }
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
                        for sc in screens.iter() {
                            sc.window.request_redraw();
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

/// ONE SCREEN'S FRAME.
///
/// The whole picture a monitor shows: its board (or the two boards of a
/// ride), the widgets standing on them, the layout editor over them,
/// and — on the one screen hosting it — the application's own interface
/// above everything. Every screen runs this; the first one is not a
/// special case, it is simply the one the pixel guard watches.
///
/// Answers what the terminal view reported (so the PTYs can be resized)
/// and which glyph-atlas rows this frame drained (so the other screens'
/// renderers can catch their own copies up).
#[allow(clippy::too_many_arguments)]
fn draw_screen(
    sc: &mut Screen,
    fonts: &mut font::FontSystem,
    host: &widgets::Host,
    settings: &mut widgets::settings::Settings,
    popup: &mut widgets::popup::Popup,
    menu: &mut Option<nacelle::object::menu::MenuState>,
    tips: &mut nacelle::object::tooltip::Tooltips,
    focus_ctl: &mut nacelle::focus::FocusCtl,
    prefs: Prefs,
    hosts_ui: bool,
    observe: bool,
) -> (Option<(usize, usize)>, Option<(u32, u32)>) {
    let (w, h) = sc.size();
    // The theme engine bakes every u-derived length from the window
    // height (§2.2). Told here, for the screen about to draw, because
    // two monitors are two heights and one global bake cannot be both;
    // a height that lands on the same u is deduplicated inside, so a
    // one-screen desktop pays nothing for the call.
    nacelle::theme::set_viewport(h, 1.0);
    // The decoration plate follows the theme and the surface, never the
    // frame: a rebake is kicked when either changes and collected
    // whenever it lands.
    sc.poll_plates();
    // Perform any deferred glyph-atlas reset at the frame boundary,
    // never mid-frame (see font.rs).
    fonts.begin_frame();
    // The list is taken OUT of the screen for the frame, so the drawing
    // below may hold it and the screen's own state at the same time; it
    // goes back with its capacity at the end, and a steady frame
    // allocates nothing.
    let mut dl = std::mem::replace(&mut sc.dl, draw::DrawList::new());
    dl.clear();
    let white = theme::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    // The backdrop plate is the first thing in the list — z 0, under
    // every panel, inside the glass snapshot. White tint is the
    // multiplicative identity: the plate's pixels ARE the theme's baked
    // colours. Mid-resize it stretches for the frame or two until the
    // fresh bake lands.
    let backdrop = sc.backdrop_id();
    if let Some(id) = backdrop {
        dl.image(0.0, 0.0, w, h, id, white);
    }
    // Rings are withheld while board rects are mid-flight (the cube
    // ride) — set before any registration is answered this frame.
    focus_ctl.set_ring_suppressed(sc.cube.is_some());
    let mut ctx = widgets::Ctx {
        dl: &mut dl,
        fonts,
        w,
        h,
        t: prefs.t,
        mouse: sc.mouse,
        term_font_scale: prefs.term_font_scale,
        ui_font_scale: prefs.ui_font_scale,
        panel_scale: 1.0,
        // The desktop's one chain (F1 §1.2) belongs to the screen the
        // interface is on; the others register nothing, so a second
        // monitor cannot put a second copy of every control into the
        // Tab order.
        focus: hosts_ui.then_some(&mut *focus_ctl),
        // Requests are collected all through the frame and answered at
        // the end of it, below — a tooltip is drawn over what it
        // explains, so it cannot be drawn while that is still being
        // drawn. One manager cannot answer to two windows at once.
        tips: hosts_ui.then_some(&mut *tips),
    };

    let mut grid_now: Option<(usize, usize)> = None;
    sc.booting = widgets::boot::draw(&mut ctx);
    if !sc.booting {
        // A finished turn lands first, so this frame measures and draws
        // the board it arrived at — or starts the next leg of a longer
        // journey.
        let ride_s = nacelle::deco::ride_secs();
        let landing = sc
            .cube
            .as_ref()
            .filter(|c| c.t0.elapsed().as_secs_f32() >= ride_s)
            .map(|c| (c.a1 != 0.0, c.to));
        if let Some((landed, to)) = landing {
            sc.cube = None;
            if landed {
                sc.board = to;
                // The world's notion of "here" must land with us, or
                // the next rebuild would send us home.
                sc.world.set_current(to);
                let sizes = sc.cur_def().sizes.clone();
                nacelle::base::set_panel_sizes(&sizes);
                if let Some(next) = sc.go_queue.pop() {
                    sc.step_to(next);
                }
            } else {
                sc.go_queue.clear();
            }
        }
        let trans: Option<(bool, f32)> = sc.pan.or_else(|| {
            sc.cube.as_ref().map(|c| {
                let t = if ride_s <= 0.0 {
                    1.0
                } else {
                    (c.t0.elapsed().as_secs_f32() / ride_s).clamp(0.0, 1.0)
                };
                let e = nacelle::deco::ride_ease(t);
                (c.horizontal, c.a0 + (c.a1 - c.a0) * e)
            })
        });
        // Two passes, because a widget's height comes from its content
        // and its content is sized by the width it is given. The first
        // pass settles the columns — their widths do not depend on any
        // of this — the widgets measure themselves against those
        // widths, and the second pass hands each the height it asked
        // for. A widget that grows into whatever it gets measures as
        // None and shares what the others left.
        {
            // The probe must not see last frame's answers, or each
            // measurement would feed the next and the panels would
            // creep frame by frame.
            nacelle::base::set_panel_intrinsic(&[]);
            let probe = sc.content_layout();
            // Standing on a fixture, the main-row board rides along
            // underneath, sharp and in place, showing through the glass
            // — but intrinsic sizing is a GLOBAL table keyed by panel,
            // and without this its widgets would only ever be measured
            // while home itself is the current board. A widget lives on
            // one board only, so the two probes' panels never collide.
            let under_probe = if !sc.editor.active && sc.board.1 != 0 {
                Some(sc.solve((sc.board.0, 0)).padded(sc.pad))
            } else {
                None
            };
            let mut wants: Vec<Option<f32>> = Vec::new();
            // What the container adds around each panel, published
            // beside the wants: the layout engine adds it to the
            // content minimums, so a panel held at its minimum keeps
            // its last content row BELOW the title band instead of
            // losing a band's worth of content to it.
            let mut chrome_px: Vec<f32> = Vec::new();
            for p in widgets::Panel::all() {
                // Every placement of this widget that is actually on
                // screen — here first, and on the board riding along
                // underneath otherwise.
                let here: Vec<_> = probe
                    .instances_of(p)
                    .into_iter()
                    .filter(|pl| pl.rect.x < w)
                    .collect();
                let placed = if here.is_empty() {
                    under_probe
                        .as_ref()
                        .map(|u| {
                            u.instances_of(p)
                                .into_iter()
                                .filter(|pl| pl.rect.x < w)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    here
                };
                // A panel no board placed sits at the off-screen
                // rectangle; measuring it would run absent widgets for
                // a height nobody will use.
                let Some(first) = placed.first().copied() else {
                    wants.push(None);
                    chrome_px.push(0.0);
                    continue;
                };
                // Measured at scale 1, so the answer is the content's
                // own size and not a reflection of the box it happens
                // to be in this frame.
                ctx.panel_scale = 1.0;
                let (sizing, titled, scales) = sc
                    .widgets
                    .get_mut(first.id)
                    .map(|wg| {
                        let s = wg.sizing(&mut ctx, host);
                        let c = wg.chrome(&mut ctx, host);
                        (s, c.title.is_some() || c.right.is_some(), wg.scales_with_panel())
                    })
                    .unwrap_or((widgets::Sizing::Reference, false, true));
                // What the container will draw around the content:
                // border, padding, and the title band when the widget
                // declares one. A Content panel is made tall enough for
                // BOTH, and the widget's scale is computed against the
                // room the chrome leaves (u2 §4.2).
                let chrome_extra = nacelle::object::panel::chrome_extra(titled);
                chrome_px.push(chrome_extra);
                // Which edge of the panel changes the size of what is
                // inside is the widget's answer, not one rule for all
                // of them: a table of rows must not magnify when it is
                // stretched downwards, and a clock must keep its
                // proportions whichever way it is pulled.
                let scale_of = |rect: &widgets::Rect, ctx: &mut nacelle::Ctx| -> f32 {
                    static REF: OnceLock<TokenId> = OnceLock::new();
                    static LO: OnceLock<TokenId> = OnceLock::new();
                    static HI: OnceLock<TokenId> = OnceLock::new();
                    let t = nacelle::theme::resolved();
                    let rf = t.px(tok(&REF, "responsive.panel_ref_frac")).max(0.01);
                    let lo = t.px(tok(&LO, "responsive.scale_min")).max(0.05);
                    let hi = t.px(tok(&HI, "responsive.scale_max")).max(lo);
                    let ws = (rect.w / (h * rf)).clamp(lo, hi);
                    match sizing {
                        widgets::Sizing::Rows => ws,
                        // A widget that does not follow panel_scale
                        // draws its measured content at scale 1 in any
                        // box, so its want must be published whole:
                        // shrinking the want without shrinking the
                        // drawing is how the control buttons left their
                        // panel at 1280x800.
                        widgets::Sizing::Content(_) if !scales => 1.0,
                        widgets::Sizing::Content(natural) => ws
                            .min((rect.h - chrome_extra).max(1.0) / natural.max(1.0))
                            .clamp(lo, hi),
                        widgets::Sizing::Reference => ctx.panel_font_scale(rect, p),
                    }
                };
                // Every placement gets its own scale: two terminals in
                // two boxes are two sizes of text.
                for pl in placed.iter() {
                    let s = scale_of(&pl.rect, &mut ctx);
                    sc.widgets.set_scale(pl.id, s);
                }
                let scale = sc.widgets.scale(first.id);
                // The height a measured widget is given is its content
                // at the scale it will be drawn with, PLUS the
                // container around it — so the box hugs content and
                // chrome together, and the band never overlaps the
                // first row.
                wants.push(match sizing {
                    widgets::Sizing::Content(natural) => {
                        Some(natural * scale + chrome_extra)
                    }
                    _ => None,
                });
                ctx.panel_scale = 1.0;
            }
            if !sc.editor.active {
                nacelle::base::set_panel_intrinsic(&wants);
                nacelle::base::set_panel_chrome(&chrome_px);
            }
        }
        // The editor shows its edited rectangles (WYSIWYG). Widgets
        // draw inside the padded (content) rects; the editor overlay
        // shows the outer edges.
        let layout = sc.content_layout();
        // Every widget drawn through the one contract: the application
        // no longer knows which is which, only what the boards place.
        {
            // Below this the ride is not drawn at all. The guard is
            // authored in the cube's degrees; the flat slide, whose
            // full travel is 1.0 rather than 90, scales it down by the
            // same ratio.
            static EPSILON: OnceLock<TokenId> = OnceLock::new();
            let eps =
                nacelle::theme::resolved().px(tok(&EPSILON, "motion.board_ride.epsilon"));
            let active_trans =
                trans.filter(|(hz, a)| a.abs() > if *hz { eps } else { eps / 90.0 });
            if let Some((horizontal, a)) = active_trans {
                // Two boards in motion. Sideways they are the faces of
                // a cube — a yaw and a perspective divide applied to
                // the vertices the widgets have already emitted; up and
                // down they slide flat. No widget knows either way.
                let cur = sc.board;
                let sign: i32 = if a > 0.0 { 1 } else { -1 };
                let face_b = sc.cube.as_ref().map(|c| c.face_b).unwrap_or(if horizontal {
                    (cur.0 + sign, cur.1)
                } else {
                    (cur.0, cur.1 + sign)
                });
                // Per-face motion parameter: yaw for the cube, y-offset
                // for the ride-in. Up and down nothing drags HOME along
                // any more: the ordinary board (y == 0) holds perfectly
                // still, and APPGRID or SEARCH AND AI rides in over it
                // — the same picture their overlay layer will give
                // under the project's own compositor.
                let (ma, mb) = if horizontal {
                    (-a, 90.0 * sign as f32 - a)
                } else {
                    (
                        if cur.1 == 0 { 0.0 } else { -a * h },
                        if face_b.1 == 0 { 0.0 } else { (sign as f32 - a) * h },
                    )
                };
                let mut faces = [(cur, ma), (face_b, mb)];
                if horizontal && faces[0].1.abs() < faces[1].1.abs() {
                    faces.swap(0, 1);
                }
                // The rider draws over the still board, so leaving a
                // fixture the still board must be painted first.
                if !horizontal && cur.1 != 0 {
                    faces.swap(0, 1);
                }
                // The space the cube turns in, one flat colour under
                // the whole turn. It is emitted BEFORE the first face
                // and therefore before any face's `start`, so no yaw
                // ever touches it: the walls move, the void does not.
                if horizontal {
                    let void = nacelle::deco::ride_void();
                    ctx.dl.rect(0.0, 0.0, w, h, void);
                }
                for (b, m) in faces {
                    if !sc.has_board(b) {
                        continue;
                    }
                    let start = ctx.dl.verts.len();
                    // Sideways each face is a WALL of a solid and
                    // carries its own ground — what the theme puts
                    // behind a board, the board's own field and the
                    // decoration plate on it — emitted before the
                    // panels so the yaw and the perspective divide
                    // below take ground and panels together.
                    // Standing still, and riding up or down, a board
                    // paints no ground at all: the frame's own clear
                    // and plate are already there and must stay visible
                    // under the fixture that rides in over them.
                    // Fixtures carry a face material on top — frosted
                    // glass: whatever is beneath shows through it
                    // blurred. The glass is sampled by screen position,
                    // so the ride may carry the quad and the frost
                    // stays put.
                    if horizontal {
                        nacelle::deco::board_ground(ctx.dl, w, h, backdrop);
                    }
                    let thm = nacelle::theme::resolved();
                    if b.1 != 0 {
                        nacelle::deco::fixture_glass(ctx.dl, w, h, prefs.frost_wash);
                    }
                    let blay = sc.solve(b).padded(sc.pad);
                    for panel in widgets::Panel::all() {
                        for pl in blay.instances_of(panel) {
                            if pl.rect.x >= w {
                                continue;
                            }
                            ctx.panel_scale = if b == cur {
                                sc.widgets.scale(pl.id)
                            } else {
                                ctx.panel_font_scale(&pl.rect, panel)
                            };
                            if let Some(wg) = sc.widgets.get_mut(pl.id) {
                                let content =
                                    draw_panel(&mut ctx, wg, pl.rect, host, panel);
                                // Input belongs to the board being
                                // stood on, not the one riding by.
                                if b == cur {
                                    sc.widgets.set_content(pl.id, content);
                                }
                            }
                            ctx.panel_scale = 1.0;
                        }
                    }
                    if horizontal {
                        static PERSP: OnceLock<TokenId> = OnceLock::new();
                        static SHADE_MIN: OnceLock<TokenId> = OnceLock::new();
                        let rad = m.to_radians();
                        let (sinp, cosp) = rad.sin_cos();
                        let rr = w / 2.0;
                        let fl =
                            w * thm.px(tok(&PERSP, "motion.board_ride.perspective"));
                        let smin = thm.px(tok(&SHADE_MIN, "boardswitch.shade_min"));
                        let shade = smin + (1.0 - smin) * cosp.max(0.0);
                        // The turned-away wall settles toward the very
                        // colour painted behind the cube, not toward
                        // #000000: edge-on it melts into the space it
                        // turns in, and a light theme rides through its
                        // own dark, never through grey.
                        let void = nacelle::deco::ride_void();
                        for v in &mut ctx.dl.verts[start..] {
                            let u = v.pos[0] - rr;
                            let x3 = u * cosp + rr * sinp;
                            let depth = rr - (rr * cosp - u * sinp);
                            let sc_p = fl / (fl + depth);
                            v.pos[0] = rr + x3 * sc_p;
                            v.pos[1] = h / 2.0 + (v.pos[1] - h / 2.0) * sc_p;
                            v.color[0] = void.r + (v.color[0] - void.r) * shade;
                            v.color[1] = void.g + (v.color[1] - void.g) * shade;
                            v.color[2] = void.b + (v.color[2] - void.b) * shade;
                        }
                    } else {
                        for v in &mut ctx.dl.verts[start..] {
                            v.pos[1] += m;
                        }
                    }
                }
            } else {
                if sc.board.1 != 0 {
                    // Standing on APPGRID or SEARCH AND AI: the
                    // main-row board stays exactly where it was,
                    // showing through the frosted glass the fixture's
                    // panels sit on.
                    let ulay = sc.solve((sc.board.0, 0)).padded(sc.pad);
                    for panel in widgets::Panel::all() {
                        for pl in ulay.instances_of(panel) {
                            if pl.rect.x >= w {
                                continue;
                            }
                            ctx.panel_scale = sc.widgets.scale(pl.id);
                            if let Some(wg) = sc.widgets.get_mut(pl.id) {
                                // A widget lives on one board only, so
                                // the ride-under board's boxes and the
                                // fixture's never collide.
                                let content =
                                    draw_panel(&mut ctx, wg, pl.rect, host, panel);
                                sc.widgets.set_content(pl.id, content);
                            }
                            ctx.panel_scale = 1.0;
                        }
                    }
                    static GLASS_TINT: OnceLock<TokenId> = OnceLock::new();
                    static GLASS_WASH: OnceLock<TokenId> = OnceLock::new();
                    let thm = nacelle::theme::resolved();
                    ctx.dl.blur(
                        0.0,
                        0.0,
                        w,
                        h,
                        tcol(thm.color(tok(&GLASS_TINT, "elev.fixture.glass.tint"))),
                    );
                    // The theme's own wash; the user's BlurOpacity
                    // scales its alpha.
                    let wash = thm.color(tok(&GLASS_WASH, "elev.fixture.glass.wash"));
                    if wash.a * prefs.frost_wash > 0.0 {
                        ctx.dl.rect(
                            0.0,
                            0.0,
                            w,
                            h,
                            tcol(wash).alpha(wash.a * prefs.frost_wash),
                        );
                    }
                }
                for panel in widgets::Panel::all() {
                    for pl in layout.instances_of(panel) {
                        // Hidden here or living on another board: not
                        // drawn, so a terminal elsewhere keeps its PTY
                        // size.
                        if pl.rect.x >= w {
                            continue;
                        }
                        ctx.panel_scale = sc.widgets.scale(pl.id);
                        if let Some(wg) = sc.widgets.get_mut(pl.id) {
                            let content = draw_panel(&mut ctx, wg, pl.rect, host, panel);
                            sc.widgets.set_content(pl.id, content);
                        }
                        ctx.panel_scale = 1.0;
                    }
                }
            }
            // What the terminal reported this frame. The widget that
            // reports a CHARACTER GRID IS the terminal view — the
            // capability is the whole of what this program knows about
            // it. Read before the editor draws, which is where it had
            // to be while the ADD WIDGET miniatures WERE the running
            // widgets; they are their own now, and cannot resize a
            // shell to a thumbnail whatever the order.
            let held = sc.widgets.grid_holder();
            sc.term_inst = held.map(|(id, _)| id);
            grid_now = held.map(|(_, g)| g);
            // Grid overlay + editor controls on top of the live panels;
            // the closure draws live miniatures in the ADD WIDGET
            // window.
            if sc.editor.active {
                sc.draw_editor(&mut ctx, host);
            }
        }
        // The BOARDS view draws whatever this hands it: every board of
        // THIS screen as it would look here and now, the current one
        // marked. Only the screen showing the window computes them —
        // the window is one, and it is drawn once.
        if hosts_ui {
            if settings.open {
                settings.boards = sc.board_thumbs(w, h);
            }
            settings.draw(&mut ctx);
            // With the settings window open over the editor its buttons
            // share the window's plane.
            if sc.editor.active && settings.open {
                sc.editor.draw_buttons(&mut ctx);
            }
            // Warning popup on the very top.
            popup.draw(&mut ctx);
            // The open context menu draws after EVERYTHING interactive
            // (F1 §4.3): the draw list is immediate, draw order is
            // z-order, and the menu is the top layer — anything drawn
            // later would sit on it. Only the theme's overlay plate
            // follows: it covers panels, popovers and content alike by
            // design.
            if let Some(m) = menu.as_mut() {
                m.draw(&mut ctx);
            }
            // Then the tooltip, over the menu as over everything else —
            // taken OUT of the context first, because the manager
            // cannot be lent to the frame and drawn into the same frame
            // at once, and because nothing drawn after this point may
            // file a request it would be too late to answer.
            if let Some(t) = ctx.tips.take() {
                // A menu covers what is under it, and explaining
                // something the user cannot see is noise: requests
                // filed under an open menu go down with it (F2 §8.1).
                if menu.is_some() {
                    t.clear();
                }
                t.draw(&mut ctx);
            }
        }
        // The overlay plate is the LAST themed thing in the list — z
        // 70, one quad over panels, popovers and content alike:
        // scanlines, grain, the top vignette. White tint, same as the
        // backdrop: the plate's pixels ARE the theme's baked colours.
        if let Some(id) = sc.overlay_id() {
            ctx.dl.image(0.0, 0.0, w, h, id, white);
        }
    }
    drop(ctx);

    // The pixel guard, before a triangle leaves for the GPU: unarmed it
    // is one atomic load.
    if observe {
        hashframe::observe(&dl);
    }
    // Only the touched rows travel — a glyph-churn frame re-uploads a
    // shelf, not the whole four megabytes.
    let drained = fonts.take_dirty_rows();
    sc.dl = dl;
    sc.present_frame(fonts, drained);
    (grid_now, drained)
}

/// Saves the layout edited in the grid editor and applies it live.
/// `select` = also make it the selected layout (SAVE AS); a plain SAVE
/// keeps the current selection. Only the CHANGED panels are written,
/// into the section of THIS screen (resolution + diagonal) — a second
/// monitor arranges itself against its own pixels, and says so in its
/// own section.
fn editor_save(
    sc: &mut Screen,
    name: &str,
    select: bool,
    popup: &mut widgets::popup::Popup,
) {
    if name.is_empty() {
        return;
    }
    let key = sc.key;
    // The layaut with the edited board folded back in — placements
    // added, moved and dropped — which is what both save paths write.
    let mut def = sc.edited_spec();
    // SAVE AS writes ALL placements as the base of the (new) file; SAVE
    // rewrites the base on its own screen or stores only the changes in
    // the section of the current screen.
    let result = if select {
        config::save_layaut_full(name, &mut def, key)
    } else {
        config::save_layaut_overrides(
            name,
            key,
            &sc.editor.changes_since_start(),
            &mut def,
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
    if let Some(wmsg) = warn {
        nacelle::sound::emit(nacelle::sound::Event::Alert);
        popup.show(wmsg);
    } else {
        nacelle::sound::emit(nacelle::sound::Event::Save);
    }
    sc.stop_editor();
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
                        // Same bed as a screen's clear (surface.void,
                        // alpha forced to 1.0 by the master).
                        let clear = nacelle::deco::clear_color();
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
