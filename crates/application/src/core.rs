//! Off-UI-thread state reducer and command loop.
//!
//! The UI (or any caller) sends [`ScreenAction`] commands through a bounded
//! queue. A background task applies the domain transition, stores the new state
//! in a `watch` channel, and publishes an [`AppEvent`]. Keeping the reducer in a
//! task means UI callbacks never run domain logic or I/O themselves, and a slow
//! consumer cannot block state updates.

use std::time::Duration;

use gta6rp_domain::screen::{ScreenAction, ScreenState};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::{
    cancellation::CancellationToken,
    error::ApplicationError,
    events::{AppEvent, EventBus},
};

const COMMAND_QUEUE_CAPACITY: usize = 64;

enum Command {
    Apply(ScreenAction),
    Shutdown,
}

/// Spawns and owns the application core task.
pub struct ApplicationCore;

impl ApplicationCore {
    /// Start the command loop on the current Tokio runtime.
    ///
    /// The task stops when the handle is shut down, when the handle is dropped
    /// (closing the command channel), or when cancellation is requested.
    ///
    /// # Panics
    /// Panics if called outside a Tokio runtime, because it must spawn the
    /// reducer task.
    #[must_use]
    pub fn spawn(
        initial: ScreenState,
        event_capacity: usize,
        shutdown_timeout: Duration,
    ) -> CoreHandle {
        let (commands, command_receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let (state_sender, state_receiver) = watch::channel(initial);
        let events = EventBus::new(event_capacity);
        let cancellation = CancellationToken::new();

        let task_events = events.clone();
        let task_cancellation = cancellation.clone();
        let join = tokio::spawn(run(
            command_receiver,
            state_sender,
            task_events,
            task_cancellation,
        ));

        CoreHandle {
            control: CoreControl {
                commands,
                events,
                state: state_receiver,
                cancellation,
            },
            join: Some(join),
            shutdown_timeout,
        }
    }
}

/// Cloneable control surface for a running application core.
///
/// A clone only carries senders and receivers, so it can be handed to the UI
/// thread or to any background task without keeping the core task alive.
#[derive(Clone)]
pub struct CoreControl {
    commands: mpsc::Sender<Command>,
    events: EventBus,
    state: watch::Receiver<ScreenState>,
    cancellation: CancellationToken,
}

impl CoreControl {
    /// The event bus, for subscribing a UI bridge.
    #[must_use]
    pub fn events(&self) -> EventBus {
        self.events.clone()
    }

    /// A receiver for the latest state. The value is always current, even if
    /// intermediate events were dropped.
    #[must_use]
    pub fn state(&self) -> watch::Receiver<ScreenState> {
        self.state.clone()
    }

    /// The latest state, without waiting.
    #[must_use]
    pub fn current_state(&self) -> ScreenState {
        *self.state.borrow()
    }

    /// Enqueue a screen action for the reducer.
    ///
    /// The method only queues the command; it returns before the transition is
    /// applied. Subscribe to [`Self::state`] or [`Self::events`] to observe the
    /// result.
    ///
    /// # Errors
    /// Returns [`ApplicationError::CommandQueueFull`] when the bounded queue is
    /// full, or [`ApplicationError::CommandClosed`] when the core has stopped.
    pub fn apply(&self, action: ScreenAction) -> Result<(), ApplicationError> {
        self.commands
            .try_send(Command::Apply(action))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => ApplicationError::CommandQueueFull,
                mpsc::error::TrySendError::Closed(_) => ApplicationError::CommandClosed,
            })
    }

    /// Request cancellation of the core loop.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

/// Owner-side handle for a running application core.
pub struct CoreHandle {
    control: CoreControl,
    join: Option<JoinHandle<()>>,
    shutdown_timeout: Duration,
}

impl CoreHandle {
    /// A cloneable control surface, safe to hand to the UI or other tasks.
    #[must_use]
    pub fn control(&self) -> CoreControl {
        self.control.clone()
    }

    /// The event bus, for subscribing a UI bridge.
    #[must_use]
    pub fn events(&self) -> EventBus {
        self.control.events()
    }

    /// A receiver for the latest state.
    #[must_use]
    pub fn state(&self) -> watch::Receiver<ScreenState> {
        self.control.state()
    }

    /// The latest state, without waiting.
    #[must_use]
    pub fn current_state(&self) -> ScreenState {
        self.control.current_state()
    }

    /// Enqueue a screen action for the reducer.
    ///
    /// # Errors
    /// Returns [`ApplicationError::CommandQueueFull`] or
    /// [`ApplicationError::CommandClosed`].
    pub fn apply(&self, action: ScreenAction) -> Result<(), ApplicationError> {
        self.control.apply(action)
    }

    /// Ask the core to stop and wait for its task to finish.
    ///
    /// # Errors
    /// Returns [`ApplicationError::ShutdownTimeout`] if the task does not stop
    /// within the configured grace period, or [`ApplicationError::TaskFailed`]
    /// if the task panicked.
    pub async fn shutdown(mut self) -> Result<(), ApplicationError> {
        self.control.cancel();
        if self.control.commands.send(Command::Shutdown).await.is_err() {
            // The task stopped on its own; the join below still reports it.
        }
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        match tokio::time::timeout(self.shutdown_timeout, join).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.is_cancelled() => Ok(()),
            Ok(Err(_)) => Err(ApplicationError::TaskFailed),
            Err(_) => Err(ApplicationError::ShutdownTimeout),
        }
    }
}

async fn run(
    mut commands: mpsc::Receiver<Command>,
    state: watch::Sender<ScreenState>,
    events: EventBus,
    cancellation: CancellationToken,
) {
    loop {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => break,
            command = commands.recv() => {
                match command {
                    Some(Command::Apply(action)) => apply(&state, &events, action),
                    Some(Command::Shutdown) | None => break,
                }
            }
        }
    }
    events.publish(AppEvent::Shutdown);
}

fn apply(state: &watch::Sender<ScreenState>, events: &EventBus, action: ScreenAction) {
    let current = *state.borrow();
    match current.apply(action) {
        Ok(next) => {
            if state.send(next).is_err() {
                // No observers are connected; `watch` still retains the value.
            }
            events.publish(AppEvent::ScreenChanged(next));
        }
        Err(error) => events.publish(AppEvent::ActionRejected {
            state: error.state,
            action: error.action,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gta6rp_domain::screen::{ScreenAction, ScreenState};
    use tokio::time::timeout;

    use super::ApplicationCore;
    use crate::events::AppEvent;

    async fn next_event(receiver: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> AppEvent {
        timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("event should arrive")
            .expect("event channel should stay open")
    }

    #[tokio::test]
    async fn applies_valid_transition_and_emits_event() {
        let handle = ApplicationCore::spawn(ScreenState::NotInstalled, 16, Duration::from_secs(1));
        let mut events = handle.events().subscribe();
        let mut state = handle.state();

        handle
            .apply(ScreenAction::BeginSetup)
            .expect("command should enqueue");

        state.changed().await.expect("state should change");
        assert_eq!(*state.borrow(), ScreenState::Detecting);
        assert_eq!(
            next_event(&mut events).await,
            AppEvent::ScreenChanged(ScreenState::Detecting)
        );

        handle.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn rejects_invalid_action_without_changing_state() {
        let handle = ApplicationCore::spawn(ScreenState::NotInstalled, 16, Duration::from_secs(1));
        let mut events = handle.events().subscribe();

        handle
            .apply(ScreenAction::LaunchStarted)
            .expect("command should enqueue");

        assert_eq!(
            next_event(&mut events).await,
            AppEvent::ActionRejected {
                state: ScreenState::NotInstalled,
                action: ScreenAction::LaunchStarted,
            }
        );
        assert_eq!(handle.current_state(), ScreenState::NotInstalled);

        handle.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn preserves_command_order() {
        let handle = ApplicationCore::spawn(ScreenState::NotInstalled, 16, Duration::from_secs(1));
        let mut state = handle.state();

        handle
            .apply(ScreenAction::BeginSetup)
            .expect("first command");
        state.changed().await.expect("first transition");
        assert_eq!(*state.borrow(), ScreenState::Detecting);

        handle
            .apply(ScreenAction::DetectionFinished)
            .expect("second command");
        state.changed().await.expect("second transition");
        assert_eq!(*state.borrow(), ScreenState::Verifying);

        handle.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn shutdown_stops_the_reducer() {
        let handle = ApplicationCore::spawn(ScreenState::Ready, 4, Duration::from_millis(500));
        handle.shutdown().await.expect("clean shutdown");
    }
}
