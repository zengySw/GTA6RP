//! Application core shared by the launcher and its background tasks.
//!
//! The crate owns the pieces that sit between the pure domain and the UI or
//! infrastructure layers: a typed error set, a bounded event channel, a
//! cancellation token, a graceful-shutdown supervisor, and a small
//! state-reducer that applies domain transitions off the UI thread.
//!
//! It deliberately contains no filesystem, HTTP, database, or UI code.

pub mod cancellation;
pub mod core;
pub mod error;
pub mod events;
pub mod shutdown;
pub mod use_cases;

pub use cancellation::CancellationToken;
pub use core::{ApplicationCore, CoreControl, CoreHandle};
pub use error::ApplicationError;
pub use events::{AppEvent, EventBus};
pub use shutdown::ShutdownSupervisor;
pub use use_cases::{
    LocalStatePort, StartupReport, UseCaseError, load_settings, prepare_startup, update_settings,
};
