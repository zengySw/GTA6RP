//! Typed errors for application-core operations.

use core::fmt;

/// A failure while driving the application core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    /// The command channel was closed because the core task stopped.
    CommandClosed,
    /// The bounded command queue is full; the caller should retry later.
    CommandQueueFull,
    /// The core task did not stop within the configured grace period.
    ShutdownTimeout,
    /// A background task panicked or was cancelled unexpectedly.
    TaskFailed,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::CommandClosed => "the application core is not running",
            Self::CommandQueueFull => "the application core command queue is full",
            Self::ShutdownTimeout => "the application core did not stop in time",
            Self::TaskFailed => "a background task failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ApplicationError {}
