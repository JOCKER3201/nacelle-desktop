//! The application's own interface — everything on screen that is not a
//! widget: the settings window, the layout editor, the warning popup and
//! the boot animation.
//!
//! The widgets themselves are files on disk, installed from the addons
//! repository and loaded by name; nothing here knows any of them
//! individually. Where a widget's clickable controls are is the
//! widget's own business too: the application asks it (`Widget::
//! pointer`) rather than keeping a copy of its geometry.
pub mod boot;
pub mod editor;
pub mod popup;
pub mod settings;

// The widget contract the application drives every widget through.
pub use nacelle::{Action, DragPhase, Host, SelectOp, Sizing, Widget};

// Geometry, the panel/layout model and text fitting come from the base.
pub use nacelle::base::*;

/// Where one line of `px`-tall type sits when it is centred in a box
/// `box_h` tall whose top edge is at `y`.
///
/// The toolkit's `nacelle::view::paint::center_line_y` says this for
/// everything drawn through a `Surface`; the application's own chrome
/// draws through a `DrawList` instead, and so needs the same two
/// tokens said once rather than copied per label. Both halves matter:
/// the box is shared out over the LINE (`px * leading`), not over the
/// glyph, and the master centres `optical`ly, which nudges the line by
/// `rhythm.cap_center_bias` — a nudge every open-coded copy of this
/// arithmetic in this crate silently dropped.
pub(crate) fn center_line_y(y: f32, box_h: f32, px: f32, leading: f32) -> f32 {
    use nacelle::theme::{self, TokenId};
    use std::sync::OnceLock;
    static MODE: OnceLock<TokenId> = OnceLock::new();
    static OPTICAL: OnceLock<Option<u16>> = OnceLock::new();
    static BIAS: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    let mode = *MODE.get_or_init(|| theme::id("rhythm.center_mode").unwrap_or(TokenId::MISSING));
    let mut ty = y + (box_h - px * leading) / 2.0;
    // The word is compared through the enum's declared index, so the
    // comparison survives a theme that lists the words in its own order.
    if *OPTICAL.get_or_init(|| theme::enum_index(mode, "optical")) == Some(t.enum_of(mode)) {
        let bias =
            *BIAS.get_or_init(|| theme::id("rhythm.cap_center_bias").unwrap_or(TokenId::MISSING));
        ty += px * t.px(bias);
    }
    ty
}

/// Serialises the tests that SELECT a theme against the ones that READ
/// one.
///
/// The theme engine is process-wide and publishes lazily: the first
/// reader that finds nothing loaded installs the master itself. Two
/// tests running at once can therefore interleave so that the master
/// lands on top of the theme the other one had just selected, and the
/// colours it then asserts on are not the ones it asked for. The
/// program never sees this — `config::resolve()` loads the theme before
/// anything draws — so the lock belongs to the test binary and not to
/// the interface.
///
/// A poisoned lock is taken anyway: the theme is a global either way,
/// and one failing test must not turn into a cascade of failures that
/// hide it.
#[cfg(test)]
pub(crate) fn theme_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    L.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// A theme of one test's own — `body` cascaded over the master and made
/// the running theme, which is how a test asks "and if the theme said
/// something else?".
///
/// Dropping it hands the engine back the plain master, so whatever runs
/// next reads the theme the program ships — including when the test in
/// between fails. Take [`theme_test_lock`] first: this SELECTS in a
/// process-wide engine.
#[cfg(test)]
pub(crate) struct Themed {
    dir: std::path::PathBuf,
}

#[cfg(test)]
impl Themed {
    pub(crate) fn new(tag: &str, body: &str) -> Self {
        let dir = std::env::temp_dir()
            .join(format!("nacelle-themed-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the fixture tree must be writable");
        let path = dir.join("fixture.theme");
        std::fs::write(&path, format!("[meta]\nschema = 1\nname = \"{tag}\"\n\n{body}"))
            .expect("the fixture theme must be writable");
        nacelle::theme::load_with(nacelle::theme::LoadRequest {
            path: Some(path),
            ..Default::default()
        });
        Themed { dir }
    }
}

#[cfg(test)]
impl Drop for Themed {
    fn drop(&mut self) {
        nacelle::theme::load_with(nacelle::theme::LoadRequest::default());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One number out of the running theme, by name.
#[cfg(test)]
pub(crate) fn token_px(name: &str) -> f32 {
    nacelle::theme::resolved()
        .px(nacelle::theme::id(name).unwrap_or_else(|| panic!("the master declares {name}")))
}

/// Every string a piece of chrome DREW, with the size it was drawn at, on
/// a window `h` tall whose user has set `UIFontSize=` to `uscale × 100`.
///
/// The measuring instrument for "the interface scale is applied exactly
/// once". Two mechanisms could carry that setting — the theme's
/// `metric.ui_scale`, which multiplies u and therefore every length the
/// master writes, and a factor a drawer applies by hand — and a caller
/// that uses both squares it. Nothing in the arithmetic says which is
/// happening: at 125 % the wrong answer is 156 % and both are "bigger".
/// So the measurement is taken off the drawing itself, through the
/// command register, and the ratio between two scales is the whole test.
///
/// `Ctx::ui_font_scale` is set to the same `uscale` the viewport is told,
/// because that is what a frame does — a drawer that reaches for it is
/// exactly what has to be caught.
#[cfg(test)]
pub(crate) fn drawn_text(
    h: f32,
    t: f64,
    uscale: f32,
    draw: impl FnOnce(&mut Ctx),
) -> Vec<(String, f32)> {
    // The engine publishes lazily and `set_viewport` is a no-op until it
    // exists — a first caller that skips this measures the 1080-line
    // default and reports it as this height's answer.
    nacelle::theme::resolved();
    nacelle::theme::set_viewport(h, uscale);
    let mut fonts = crate::font::FontSystem::new();
    let mut dl = nacelle::draw::DrawList::recording();
    {
        let mut ctx = Ctx {
            dl: &mut dl,
            fonts: &mut fonts,
            w: h * 16.0 / 9.0,
            h,
            t,
            // Off the window, so nothing is drawn in its hover look: a
            // hovered control may swap the role it draws in, and this
            // measurement compares two runs string for string.
            mouse: nacelle::pointer::Pointer::new(-1.0, -1.0),
            term_font_scale: 1.0,
            ui_font_scale: uscale,
            panel_scale: 1.0,
            focus: None,
            tips: None,
        };
        draw(&mut ctx);
    }
    dl.cmds()
        .iter()
        .filter_map(|c| match c {
            nacelle::draw::DrawCmd::Text { text, px, .. } => Some((text.clone(), *px)),
            // A module title is one call that draws two strings at one
            // size; the pair is enough to tell the two runs apart.
            nacelle::draw::DrawCmd::ModuleTitle { left, right, px, .. } => {
                Some((format!("{left}\u{1f}{right}"), *px))
            }
            _ => None,
        })
        .collect()
}

/// Asserts that every string `draw` puts on screen at both 100 % and
/// 125 % is exactly 25 % bigger in the second run.
///
/// Matched by the string itself rather than by position, because a bigger
/// interface is a shorter list: a page holds itself to its content box,
/// so the last row of a full one falls off the bottom at 125 % and the
/// two runs are legitimately not the same picture. What must not differ
/// is the size of a line BOTH runs drew.
///
/// The floor is the other exception: `type.<role>.min_px` is a device
/// length and does not follow u, so a role already sitting on its floor
/// at 100 % stays there. Such a line is skipped. A run in which nothing
/// at all could be measured fails — a measurement that measured nothing
/// must not read as a pass.
#[cfg(test)]
pub(crate) fn assert_scales_once(what: &str, h: f32, t: f64, draw: impl Fn(&mut Ctx)) {
    const SCALE: f32 = 1.25;
    let floor = token_px("type.min_px");
    let plain = drawn_text(h, t, 1.0, &draw);
    let mut big = drawn_text(h, t, SCALE, &draw);
    assert!(!plain.is_empty(), "{what} at {h}: drew no text at all — nothing was measured");
    let mut measured = 0;
    for (s, a) in plain.iter() {
        let Some(i) = big.iter().position(|(s2, _)| s2 == s) else { continue };
        // Taken out, so two identical strings are two measurements and
        // not the same one twice.
        let (_, b) = big.remove(i);
        // On the floor at 100 %: a device px, deliberately outside u.
        if *a <= floor * 1.001 {
            continue;
        }
        measured += 1;
        let got = b / a;
        assert!(
            (got - SCALE).abs() < 0.005,
            "{what} at {h}: \"{s}\" is {a} px at 100 % and {b} px at 125 % — \
             a ratio of {got:.4} where 1.25 is the setting. \
             1.5625 is the setting applied twice (the theme's u AND a \
             factor in the drawer); 1.0 is a drawer that reads neither."
        );
    }
    assert!(
        measured > 0,
        "{what} at {h}: no line could be measured — every one of them either \
         sat on type.min_px or was drawn in only one of the two runs"
    );
}
