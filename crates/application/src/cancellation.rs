//! Cooperative cancellation built on a `watch` channel.
//!
//! A token is cheaply cloneable. Clones observe the same signal, so one owner
//! can cancel work that was handed to several background tasks. Waiting on a
//! cancelled token returns immediately, which makes cancellation checks safe to
//! place before expensive or blocking steps.

use tokio::sync::watch::{self, Sender};

/// A cloneable signal that asks background work to stop.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    sender: Sender<bool>,
}

impl CancellationToken {
    /// Create a token that has not been cancelled yet.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _receiver) = watch::channel(false);
        Self { sender }
    }

    /// Request cancellation. Repeated calls are harmless.
    pub fn cancel(&self) {
        // The previous value is intentionally unused: whether this call or an
        // earlier one performed the transition does not matter.
        let _previous = self.sender.send_replace(true);
    }

    /// Whether cancellation was already requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.sender.borrow()
    }

    /// Resolve once cancellation is requested.
    ///
    /// The future resolves immediately for an already-cancelled token. The
    /// returned clone keeps the token alive, so the underlying channel cannot
    /// close while this future is pending.
    pub async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        if *receiver.borrow_and_update() {
            return;
        }
        while receiver.changed().await.is_ok() {
            if *receiver.borrow_and_update() {
                return;
            }
        }
        // Unreachable while `self` is alive: `self.sender` keeps the channel
        // open. If it ever closes, treat it as "stop waiting".
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::CancellationToken;

    #[tokio::test]
    async fn starts_running_and_cancels() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn waiting_resolves_after_cancel_from_another_task() {
        let token = CancellationToken::new();
        let waiter = token.clone();
        let handle = tokio::spawn(async move {
            waiter.cancelled().await;
        });

        // The waiter must not resolve before cancellation.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), handle)
                .await
                .is_err()
        );

        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            token_clone.cancelled().await;
        });
        token.cancel();
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("cancellation should resolve")
            .expect("task should join");
    }

    #[tokio::test]
    async fn already_cancelled_token_resolves_immediately() {
        let token = CancellationToken::new();
        token.cancel();
        tokio::time::timeout(Duration::from_millis(50), token.cancelled())
            .await
            .expect("pre-cancelled token should resolve immediately");
    }

    #[tokio::test]
    async fn clones_share_one_signal() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());
    }
}
