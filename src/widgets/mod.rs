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
