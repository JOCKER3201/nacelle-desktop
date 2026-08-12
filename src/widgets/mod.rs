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
