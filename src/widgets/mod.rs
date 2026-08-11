//! The application's own interface — everything on screen that is not a
//! widget: the settings window, the layout editor, the warning popup and
//! the boot animation.
//!
//! The widgets themselves are files on disk, installed from the widgets
//! repository and loaded by name; nothing here knows any of them
//! individually. What the application must agree on with the two that
//! own clickable controls — where those controls are — comes from the
//! toolkit, re-exported below.
pub mod boot;
pub mod editor;
pub mod popup;
pub mod settings;

pub use nacelle::geometry::{control, shell};

// The widget contract the application drives every widget through.
pub use nacelle::{Action, DragPhase, Host, SelectOp, Sizing, Widget};

// Geometry, the panel/layout model and text fitting come from the base.
pub use nacelle::base::*;
