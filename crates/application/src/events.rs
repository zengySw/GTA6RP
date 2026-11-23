//! Bounded event channel from the application core to the UI.
//!
//! Events are delivered through a `broadcast` channel with a fixed capacity.
//! That keeps memory bounded and lets a slow UI lag instead of blocking the
//! core. Lagging is reported by the receiver as `RecvError::Lagged`; discrete
//! screen changes are still available through the state `watch` channel, so a
//! dropped intermediate event cannot corrupt the visible state.

use gta6rp_domain::screen::{ScreenAction, ScreenState};
use tokio::sync::broadcast::{self, Receiver, Sender};

use crate::use_cases::StartupReport;

/// Something the core wants the UI to observe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEvent {
    /// The main screen entered a new state.
    ScreenChanged(ScreenState),
    /// An action was ignored because it is not valid in the current state.
    ActionRejected {
        state: ScreenState,
        action: ScreenAction,
    },
    /// Local state was loaded and interrupted work was recovered.
    StartupCompleted(StartupReport),
    /// Local state could not be prepared. The UI must show a generic message;
    /// details are deliberately not part of the event.
    StartupFailed(StartupFailure),
    /// The core has stopped processing commands.
    Shutdown,
}

/// Coarse startup failure categories safe to surface in the UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupFailure {
    /// The local database could not be opened or queried.
    StorageUnavailable,
    /// Persisted settings do not satisfy the domain rules.
    InvalidSettings,
}

/// Cloneable publisher used by the core and background tasks.
#[derive(Clone)]
pub struct EventBus {
    sender: Sender<AppEvent>,
}

impl EventBus {
    /// Create a bus with the given buffer capacity.
    ///
    /// A capacity of zero is raised to one because `broadcast` requires a
    /// non-zero buffer.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    /// Subscribe a new consumer.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<AppEvent> {
        self.sender.subscribe()
    }

    /// Publish an event.
    ///
    /// Publishing is best-effort: with no live subscribers the event is
    /// discarded, which is expected during startup and shutdown.
    pub fn publish(&self, event: AppEvent) {
        if self.sender.send(event).is_err() {
            // No subscribers are connected; nothing to deliver.
        }
    }

    /// Number of currently connected subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppEvent, EventBus};
    use gta6rp_domain::screen::{ScreenAction, ScreenState};

    #[test]
    fn delivers_events_to_subscribers() {
        let bus = EventBus::new(4);
        let mut receiver = bus.subscribe();
        bus.publish(AppEvent::ScreenChanged(ScreenState::Detecting));
        bus.publish(AppEvent::ActionRejected {
            state: ScreenState::Ready,
            action: ScreenAction::BeginSetup,
        });

        assert_eq!(
            receiver.try_recv(),
            Ok(AppEvent::ScreenChanged(ScreenState::Detecting))
        );
        assert_eq!(
            receiver.try_recv(),
            Ok(AppEvent::ActionRejected {
                state: ScreenState::Ready,
                action: ScreenAction::BeginSetup,
            })
        );
    }

    #[test]
    fn publishes_without_subscribers() {
        let bus = EventBus::new(1);
        bus.publish(AppEvent::Shutdown);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn zero_capacity_is_raised_to_one() {
        let bus = EventBus::new(0);
        let mut receiver = bus.subscribe();
        bus.publish(AppEvent::Shutdown);
        assert_eq!(receiver.try_recv(), Ok(AppEvent::Shutdown));
    }

    #[test]
    fn slow_subscriber_observes_lag_instead_of_blocking() {
        let bus = EventBus::new(2);
        let mut receiver = bus.subscribe();
        for _ in 0..5 {
            bus.publish(AppEvent::ScreenChanged(ScreenState::Downloading));
        }
        // A slow receiver reports lag but the publisher never blocks.
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn each_subscriber_receives_independently() {
        let bus = EventBus::new(4);
        let mut first = bus.subscribe();
        let mut second = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
        bus.publish(AppEvent::ScreenChanged(ScreenState::Ready));
        assert_eq!(
            first.try_recv(),
            Ok(AppEvent::ScreenChanged(ScreenState::Ready))
        );
        assert_eq!(
            second.try_recv(),
            Ok(AppEvent::ScreenChanged(ScreenState::Ready))
        );
    }
}
