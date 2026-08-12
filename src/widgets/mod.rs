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
