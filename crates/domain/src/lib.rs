//! Pure domain types shared by the launcher core.
//!
//! This crate has no filesystem, network, database, or UI dependencies. It
//! holds the rules that must stay identical between the preview UI and the
//! future application core: the main screen state machine and the validation
//! rules for relative paths that come from manifests.

pub mod path;
pub mod screen;
pub mod settings;
