//! Boot screen — a quick "boot log" and logo, like the eDEX-UI startup sequence.
//!
//! Every length, duration, size, tracking and colour on this screen comes
//! from `[boot]` and the three type roles that section binds. The master
//! wrote those nine keys for this file and quotes its line numbers in
//! their comments; until now not one of them was read, so the first thing
//! the user ever sees was the one screen a theme could not touch.

use super::Ctx;
use nacelle::theme::{self, TokenId};
use nacelle::ui;
use std::sync::OnceLock;

fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

const BOOT_LOG: &[&str] = &[
    "nacelle-desktop kernel interface initialized",
    "vulkan: loading ICDs and enumerating physical devices",
    "vulkan: swapchain acquired, FIFO present mode",
    "gpu pipeline compiled (naga -> SPIR-V)",
    "glyph atlas online (1024x1024 R8)",
    "mounting /proc and /sys data sources",
    "reading DMI tables",
    "cpu governor: performance metrics attached",
    "memory watcher armed",
    "spawning pty master/slave pair",
    "exec user shell",
    "network probe scheduled",
    "loading world map projection",
    "keyboard matrix mapped (en-US)",
    "filesystem tracker linked to shell cwd",
    "theme loaded: tron",
    "audio subsystem: skipped (headless fx)",
    "compositor bypass: direct-to-swapchain",
    "all modules nominal",
    "initiating boot sequence...",
];

/// Draws the boot screen. Returns true while the sequence lasts.
pub fn draw(ctx: &mut Ctx) -> bool {
    static DURATION: OnceLock<TokenId> = OnceLock::new();
    static LOG_DURATION: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let ms = ctx.t * 1000.0;
    // Strictly less, so `duration_ms = 0ms` is a theme saying "no boot
    // screen" and gets none — at `>` it would still be shown one frame.
    if ms >= th.px(tok(&DURATION, "boot.duration_ms")) as f64 {
        return false;
    }
    // The log owns the window it scrolls in and the logo owns the rest:
    // one token draws the whole split, where the code used to hold a
    // third, undeclared number that left the log standing still for its
    // last 0.2 s.
    let log_ms = th.px(tok(&LOG_DURATION, "boot.log_duration_ms")) as f64;
    if ms < log_ms {
        draw_log(ctx, ms / log_ms);
    } else {
        draw_logo(ctx);
    }
    true
}

/// The scrolling log, `at` of the way through its window.
fn draw_log(ctx: &mut Ctx, at: f64) {
    static LINE_ROLE: OnceLock<TokenId> = OnceLock::new();
    static PAD_TOP: OnceLock<TokenId> = OnceLock::new();
    static PAD_X: OnceLock<TokenId> = OnceLock::new();
    static LOG_DURATION: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let role = ui::bound_role(&LINE_ROLE, "boot.line_role");
    // No runtime factor: UIFontSize= is `metric.ui_scale` and the bake
    // carries it into every role's size. Passing `ctx.ui_font_scale` here
    // — which this line did while the viewport was told a literal 1.0 —
    // now applies the user's scale a second time, and 125 % draws at
    // 156 %. The shrink argument is for a stack that is squeezing its own
    // text, and the boot screen squeezes nothing.
    let px = role.px(ctx, 1.0);
    let ink = role.color();
    let track = role.tracking_px(px);
    let step = px * role.leading();
    let x = th.px(tok(&PAD_X, "boot.pad_x"));
    let mut y = th.px(tok(&PAD_TOP, "boot.pad_top"));
    // The stamps a fictional kernel prints are paced by the window the
    // theme gives the log, so a theme that widens it does not leave the
    // clock in the text claiming something else.
    let per_line = th.px(tok(&LOG_DURATION, "boot.log_duration_ms")) as f64
        / 1000.0
        / BOOT_LOG.len() as f64;
    let shown = (at * BOOT_LOG.len() as f64) as usize;
    // The role's OWN face. This used to be `FONT_MONO` written here,
    // under a note saying the face was the one part of a binding that
    // could not be routed — so the screen was monospace while
    // `boot.line_role` named a role whose face is `ui`, and the two
    // could not be told apart from either end. `Role::font` routes it,
    // and the master now binds the log to `data`, which IS the mono
    // face: the key says what the screen shows.
    let face = role.font();
    for (i, line) in BOOT_LOG.iter().take(shown + 1).enumerate() {
        ctx.dl.text(
            ctx.fonts,
            face,
            px,
            x,
            y,
            &format!("[{:>8.4}] {}", i as f64 * per_line, line),
            ink,
            track,
        );
        y += step;
    }
}

/// The logo and its sub-line, once the log has had its window.
fn draw_logo(ctx: &mut Ctx) {
    static LOGO_ROLE: OnceLock<TokenId> = OnceLock::new();
    static SUB_ROLE: OnceLock<TokenId> = OnceLock::new();
    static LOGO_Y: OnceLock<TokenId> = OnceLock::new();
    static SUB_GAP: OnceLock<TokenId> = OnceLock::new();
    let th = theme::resolved();
    let logo = ui::bound_role(&LOGO_ROLE, "boot.logo_role");
    // 1.0 for the same reason as the log above: the scale is in the bake.
    let big = logo.px(ctx, 1.0);
    let y = ctx.h * th.px(tok(&LOGO_Y, "boot.logo_y_frac"));
    ctx.dl.text_center(
        ctx.fonts,
        logo.font(),
        big,
        ctx.w / 2.0,
        y,
        "NACELLE",
        logo.color(),
        logo.tracking_px(big),
    );
    let sub = ui::bound_role(&SUB_ROLE, "boot.sub_role");
    let px = sub.px(ctx, 1.0);
    // The sub-line blinked on a period, a duty and a floor written here;
    // the theme's [motion.*_blink] family owns all three, and has no
    // entry for this screen, so the line stands still and the effect is
    // reported once rather than being invented a fourth time.
    let blink = ui::blink_factor("boot_sub_blink", ctx.t);
    ctx.dl.text_center(
        ctx.fonts,
        sub.font(),
        px,
        ctx.w / 2.0,
        y + th.px(tok(&SUB_GAP, "boot.sub_gap")),
        "INITIATING BOOT SEQUENCE",
        sub.color().fade(blink),
        sub.tracking_px(px),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use nacelle::draw::{DrawCmd, DrawList};
    use nacelle::font::FontSystem;
    use nacelle::theme::{Color, LoadRequest};

    const W: f32 = 1920.0;
    const H: f32 = 1080.0;

    /// One text command as the register saw it — what the screen ASKED
    /// for, which is the thing a token is supposed to move.
    #[derive(Clone, Debug)]
    struct Text {
        x: f32,
        y: f32,
        font: u8,
        px: f32,
        tracking: f32,
        color: Color,
        text: String,
    }

    /// One boot frame at `t` seconds: whether the sequence is still on,
    /// and every string it asked for.
    fn frame(fonts: &mut FontSystem, t: f64) -> (bool, Vec<Text>) {
        let mut dl = DrawList::recording();
        let live = {
            let mut ctx = Ctx {
                dl: &mut dl,
                fonts,
                w: W,
                h: H,
                t,
                mouse: nacelle::pointer::Pointer::new(0.0, 0.0),
                term_font_scale: 1.0,
                ui_font_scale: 1.0,
                panel_scale: 1.0,
                focus: None,
                tips: None,
            };
            draw(&mut ctx)
        };
        let texts = dl
            .cmds()
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text { at, font, px, tracking, color, text, .. } => Some(Text {
                    x: at[0],
                    y: at[1],
                    font: *font,
                    px: *px,
                    tracking: *tracking,
                    color: *color,
                    text: text.clone(),
                }),
                _ => None,
            })
            .collect();
        (live, texts)
    }

    /// The px behind a token name, now.
    fn px(name: &str) -> f32 {
        theme::resolved().px(theme::id(name).unwrap_or_else(|| panic!("the master declares {name}")))
    }

    /// Installs a theme that says nothing but the keys given, over the
    /// master. The file is data, so this is the same door a user's own
    /// theme comes in by.
    fn install(keys: &str) {
        let path = std::env::temp_dir()
            .join(format!("nacelle-boot-fixture-{}.theme", std::process::id()));
        std::fs::write(
            &path,
            format!(
                "[meta]\nschema = 1\nname = \"boot fixture\"\nbase = \"default\"\n\n{keys}\n"
            ),
        )
        .expect("the fixture theme must be writable");
        let _ = theme::load_with(LoadRequest { path: Some(path), ..Default::default() });
        theme::set_viewport(H, 1.0);
    }

    /// Back to the theme every other test in this binary expects.
    fn master() {
        let _ = theme::load_with(LoadRequest::default());
        theme::set_viewport(H, 1.0);
    }

    /// A time inside the log's window, and one inside the logo's.
    fn during_log() -> f64 {
        px("boot.log_duration_ms") as f64 / 2.0 / 1000.0
    }
    fn during_logo() -> f64 {
        (px("boot.log_duration_ms") as f64 + px("boot.duration_ms") as f64) / 2.0 / 1000.0
    }

    /// `boot.duration_ms` is the whole screen's lease: a theme that sets
    /// it to nothing gets no boot screen at all, at frame zero.
    #[test]
    fn duration_ends_the_screen_and_zero_never_starts_it() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let dur = px("boot.duration_ms") as f64 / 1000.0;
        let inside = frame(&mut fonts, dur - 0.01).0;
        let past = frame(&mut fonts, dur + 0.01).0;

        install("[boot]\nduration_ms = 0ms");
        let silenced = frame(&mut fonts, 0.0);
        master();

        assert!(inside, "the screen ended before the theme's duration");
        assert!(!past, "the screen outlived the theme's duration");
        assert!(!silenced.0, "duration_ms = 0ms still showed a boot screen");
        assert!(silenced.1.is_empty(), "duration_ms = 0ms still drew {} strings", silenced.1.len());
    }

    /// `boot.log_duration_ms` is the split: shorten it and the logo is
    /// already up at a moment the master still spends on the log.
    #[test]
    fn log_duration_moves_the_split_between_log_and_logo() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let t = during_log();
        let by_master = frame(&mut fonts, t).1;

        install("[boot]\nlog_duration_ms = 10ms");
        let shortened = frame(&mut fonts, t).1;
        master();

        assert!(
            by_master.iter().all(|c| c.text.starts_with('[')),
            "the master's log window drew something other than log lines"
        );
        assert!(
            shortened.iter().any(|c| c.text == "NACELLE"),
            "a 10 ms log window was still scrolling the log"
        );
    }

    /// The log's pace and its length: how many lines stand on screen is
    /// the theme's window, not a number here.
    #[test]
    fn the_log_fills_its_window_and_no_faster() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let window = px("boot.log_duration_ms") as f64 / 1000.0;
        let first = frame(&mut fonts, 0.0).1.len();
        let half = frame(&mut fonts, window / 2.0).1.len();
        let last = frame(&mut fonts, window * 0.999).1.len();
        master();

        assert_eq!(first, 1, "the log did not start on its first line");
        assert_eq!(half, BOOT_LOG.len() / 2 + 1, "the log was not half done at half its window");
        assert_eq!(last, BOOT_LOG.len(), "the log did not reach its last line inside its window");
    }

    /// `boot.pad_top` and `boot.pad_x` place the log, and the bound
    /// role's leading paces it — the three numbers that used to be
    /// vh(3.0), vw(2.0) and a copy of `type.body.leading`.
    #[test]
    fn padding_and_leading_come_from_the_theme() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let t = during_log();
        let by_master = frame(&mut fonts, t).1;
        let (pad_x, pad_top, leading) =
            (px("boot.pad_x"), px("boot.pad_top"), px("type.data.leading"));

        install("[boot]\npad_top = 20u\npad_x = 30u\n\n[type]\ndata.leading = 1.90");
        let moved = frame(&mut fonts, t).1;
        let (pad_x2, pad_top2, leading2) =
            (px("boot.pad_x"), px("boot.pad_top"), px("type.data.leading"));
        master();

        assert_eq!((by_master[0].x, by_master[0].y), (pad_x, pad_top));
        assert!(
            (by_master[1].y - by_master[0].y - by_master[0].px * leading).abs() < 0.01,
            "the master's line pitch is not its role's leading"
        );
        assert_ne!(pad_x, pad_x2, "the fixture failed to move boot.pad_x");
        assert_eq!((moved[0].x, moved[0].y), (pad_x2, pad_top2));
        assert!(
            (moved[1].y - moved[0].y - moved[0].px * leading2).abs() < 0.01,
            "the line pitch ignored the role's new leading"
        );
    }

    /// The three `*_role` bindings: size, tracking and ink of all three
    /// texts follow whichever role the theme names.
    #[test]
    fn the_role_bindings_size_track_and_colour_every_text() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let line = frame(&mut fonts, during_log()).1.remove(0);
        let logo_frame = frame(&mut fonts, during_logo()).1;
        let (logo, sub) = (logo_frame[0].clone(), logo_frame[1].clone());
        let hero_track = px("type.display.hero.tracking");
        let data_px = px("type.data.size").max(px("type.data.min_px"));
        let body_px = px("type.body.size").max(px("type.body.min_px"));
        let hero_px = px("type.display.hero.size").max(px("type.display.hero.min_px"));
        let caption_px = px("type.caption.size").max(px("type.caption.min_px"));
        let spare0_px = px("type.spare0.size").max(px("type.spare0.min_px"));

        install("[boot]\nline_role = display.hero\nlogo_role = caption\nsub_role = body");
        let rebound_line = frame(&mut fonts, during_log()).1.remove(0);
        let rebound_logo = frame(&mut fonts, during_logo()).1;
        master();

        assert_eq!(line.px, data_px, "the log line is not its bound role's size");
        assert_eq!(logo.px, hero_px, "the logo is not its bound role's size");
        assert_eq!(sub.px, spare0_px, "the sub-line is not its bound role's size");
        assert!(
            (logo.tracking - logo.px * hero_track).abs() < 0.01,
            "the logo's tracking is not display.hero's 0.040em"
        );
        assert!(
            (sub.tracking - sub.px * px("type.spare0.tracking")).abs() < 0.01,
            "the sub-line's tracking is not its own role's"
        );
        assert_eq!(rebound_line.px, hero_px, "boot.line_role did not move the log's size");
        assert_eq!(rebound_logo[0].px, caption_px, "boot.logo_role did not move the logo's size");
        assert_eq!(rebound_logo[1].px, body_px, "boot.sub_role did not move the sub-line's size");
    }

    /// One accent used to be painted over all three texts; each now
    /// draws in its own role's `fg`, and the theme can tell them apart.
    #[test]
    fn each_text_draws_in_its_own_roles_ink() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let line = frame(&mut fonts, during_log()).1.remove(0);
        let logo_frame = frame(&mut fonts, during_logo()).1;
        let th = theme::resolved();
        let data = th.color(theme::id("type.data.fg").expect("the master declares it"));
        let hero = th.color(theme::id("type.display.hero.fg").expect("the master declares it"));
        let caption = th.color(theme::id("type.spare0.fg").expect("the master declares it"));

        install("[type]\nspare0.fg = @severity.critical.text");
        let repainted = frame(&mut fonts, during_logo()).1;
        let wanted =
            theme::resolved().color(theme::id("type.spare0.fg").expect("the master declares it"));
        master();

        assert_eq!(line.color, data);
        assert_eq!(logo_frame[0].color, hero);
        assert_eq!(logo_frame[1].color, caption);
        assert_ne!(hero, caption, "two of the three texts share one ink");
        assert_eq!(repainted[1].color, wanted, "type.spare0.fg did not repaint the sub-line");
    }

    /// `boot.logo_y_frac` puts the logo down the screen and
    /// `boot.sub_gap` hangs the sub-line under it.
    #[test]
    fn the_logo_sits_where_the_theme_puts_it() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let t = during_logo();
        let by_master = frame(&mut fonts, t).1;
        let (frac, gap) = (px("boot.logo_y_frac"), px("boot.sub_gap"));

        install("[boot]\nlogo_y_frac = 10%\nsub_gap = 1.5x @type.display.hero.size");
        let moved = frame(&mut fonts, t).1;
        let (frac2, gap2) = (px("boot.logo_y_frac"), px("boot.sub_gap"));
        master();

        assert_eq!(by_master[0].y, H * frac);
        assert!((by_master[1].y - by_master[0].y - gap).abs() < 0.01);
        assert_ne!(frac, frac2, "the fixture failed to move boot.logo_y_frac");
        assert_ne!(gap, gap2, "the fixture failed to move boot.sub_gap");
        assert_eq!(moved[0].y, H * frac2, "the logo ignored boot.logo_y_frac");
        assert!(
            (moved[1].y - moved[0].y - gap2).abs() < 0.01,
            "the sub-line ignored boot.sub_gap"
        );
    }

    /// The two texts do not touch — measured in the face they are
    /// actually drawn in, not in the size the theme names.
    ///
    /// `y` in a draw command is the TOP of the line box, and the
    /// baseline sits one ascent below it, so a gap written as a
    /// fraction of the logo's size says nothing about whether the ink
    /// clears. For a long time `boot.sub_gap` was 0.4x of a 31 px logo:
    /// the sub-line's baseline landed ABOVE the logo's, and the owner
    /// saw one word printed through the other. The rule is stated here
    /// in the only terms that can settle it — the logo's line box must
    /// end before the sub-line's begins.
    #[test]
    fn the_sub_line_clears_the_logo_it_hangs_under() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let texts = frame(&mut fonts, during_logo()).1;
        let (logo, sub) = (&texts[0], &texts[1]);
        let (_, logo_line_h) = fonts.line_metrics(logo.font, logo.px);

        assert!(
            sub.y >= logo.y + logo_line_h,
            "the sub-line starts at {} and the logo's line box ends at {} — they overlap",
            sub.y,
            logo.y + logo_line_h
        );
        assert!(
            logo.px > sub.px,
            "the logo ({} px) is not larger than the line under it ({} px)",
            logo.px,
            sub.px
        );
    }

    /// The PAIR sits in the middle of the screen — the owner's call,
    /// against centring the logo alone and letting the sub-line hang
    /// below the middle. Nothing in the theme centres anything: the
    /// pair lands there because `logo_y_frac`, `sub_gap` and the two
    /// sizes agree, so this test is what keeps that agreement true
    /// when any one of the four moves.
    #[test]
    fn the_pair_is_centred_on_the_screen() {
        let _lock = crate::widgets::theme_test_lock();
        master();
        let mut fonts = FontSystem::new();
        let texts = frame(&mut fonts, during_logo()).1;
        let (logo, sub) = (&texts[0], &texts[1]);
        let (_, sub_line_h) = fonts.line_metrics(sub.font, sub.px);

        // Top of the logo's line box to the bottom of the sub-line's.
        let middle = (logo.y + sub.y + sub_line_h) / 2.0;

        assert!(
            (middle - H / 2.0).abs() < H * 0.02,
            "the pair's middle is at {middle} and the screen's at {}",
            H / 2.0
        );
    }
}
