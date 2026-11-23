//! Graceful shutdown for a group of background tasks.
//!
//! Tasks are expected to observe the supervisor's [`CancellationToken`] and
//! return promptly. The supervisor cancels every task, waits for them to finish
//! within a grace period, and reports panics or an overrun instead of hanging
//! the process.

use std::{future::Future, time::Duration};

use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::{cancellation::CancellationToken, error::ApplicationError};

/// Owns a set of tasks and coordinates their cancellation and join.
pub struct ShutdownSupervisor {
    token: CancellationToken,
    tasks: JoinSet<()>,
    grace: Duration,
}

impl ShutdownSupervisor {
    /// Create a supervisor with a grace period for task completion.
    #[must_use]
    pub fn new(grace: Duration) -> Self {
        Self {
            token: CancellationToken::new(),
            tasks: JoinSet::new(),
            grace,
        }
    }

    /// A clone of the cancellation token that spawned tasks must observe.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Spawn a task that is tracked and cancelled on shutdown.
    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(task);
    }

    /// Number of tasks that have not finished yet.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Whether no tracked tasks remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Cancel all tasks and wait for them to finish.
    ///
    /// # Errors
    /// Returns [`ApplicationError::TaskFailed`] if a task panicked, or
    /// [`ApplicationError::ShutdownTimeout`] if tasks did not finish within the
    /// grace period. On timeout the remaining tasks are aborted.
    pub async fn shutdown(mut self) -> Result<(), ApplicationError> {
        self.token.cancel();

        let mut failed = false;
        let draining = async {
            while let Some(result) = self.tasks.join_next().await {
                match result {
                    Ok(()) => {}
                    Err(error) if error.is_cancelled() => {}
                    Err(_) => failed = true,
                }
            }
        };

        match timeout(self.grace, draining).await {
            Ok(()) if failed => Err(ApplicationError::TaskFailed),
            Ok(()) => Ok(()),
            Err(_) => {
                self.tasks.abort_all();
                Err(ApplicationError::ShutdownTimeout)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ShutdownSupervisor;
    use crate::error::ApplicationError;

    #[tokio::test]
    async fn cancels_and_joins_cooperative_tasks() {
        let mut supervisor = ShutdownSupervisor::new(Duration::from_secs(1));
        let token = supervisor.cancellation_token();
        supervisor.spawn(async move {
            token.cancelled().await;
        });
        assert_eq!(supervisor.len(), 1);
        assert!(supervisor.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn reports_timeout_when_a_task_ignores_cancellation() {
        let mut supervisor = ShutdownSupervisor::new(Duration::from_millis(50));
        supervisor.spawn(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        assert_eq!(
            supervisor.shutdown().await,
            Err(ApplicationError::ShutdownTimeout)
        );
    }

    #[tokio::test]
    async fn reports_task_failure_on_panic() {
        let mut supervisor = ShutdownSupervisor::new(Duration::from_secs(1));
        supervisor.spawn(async {
            panic!("background task failed");
        });
        assert_eq!(
            supervisor.shutdown().await,
            Err(ApplicationError::TaskFailed)
        );
    }

    #[tokio::test]
    async fn empty_supervisor_shuts_down_cleanly() {
        let supervisor = ShutdownSupervisor::new(Duration::from_millis(50));
        assert!(supervisor.is_empty());
        assert!(supervisor.shutdown().await.is_ok());
    }
}
