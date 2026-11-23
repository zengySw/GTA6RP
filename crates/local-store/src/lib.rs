//! Local persistent state for the launcher.
//!
//! This crate owns the SQLite database used for settings, the selected install
//! root, the active manifest id, and download-task checkpoints. It never stores
//! credentials or other secrets; those belong in the platform secure store.
//!
//! The database runs against a local file so state survives restarts, and
//! migrations are embedded and applied on open.

pub mod error;
pub mod model;
pub mod store;

pub use error::StoreError;
pub use gta6rp_domain::settings::{MAX_PARALLEL_DOWNLOADS, Settings};
pub use model::{DownloadState, DownloadTask};
pub use store::LocalStore;
