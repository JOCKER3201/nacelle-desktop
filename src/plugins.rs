//! The plugin kill-switch. Loading itself — dlopen, the attach
//! handshake, the linked-in four — moved into the toolkit
//! (nacelle::widget::{loader, factory}); what stays is the one policy
//! decision that belongs to THIS application: whether a run honours
//! NACELLE_SAFE.


/// Whether plugins are switched off for this run.
///
/// A plugin that crashes during startup would otherwise lock the user
/// out of the very settings they need to disable it, so there has to be
/// a way in that loads none of them.
pub fn disabled() -> bool {
    std::env::var("NACELLE_SAFE").is_ok_and(|v| v != "0")
}
