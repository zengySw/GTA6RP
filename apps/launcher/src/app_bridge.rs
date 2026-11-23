//! Bridge between `ApplicationCore` `EventBus` and Slint UI.
//!
//! Runs in the Tokio thread and marshals `AppEvent` updates onto the Slint event
//! loop using `slint::invoke_from_event_loop`. UI objects are never handed to
//! the Tokio thread; only `Weak<MainWindow>` is used for safe upgrade.

use slint::Weak;

use crate::MainWindow;

use gta6rp_application::events::{AppEvent, EventBus};
use gta6rp_domain::screen::ScreenState;

/// Subscribes to the event bus and applies screen-state changes to the Slint window.
///
/// The function runs until the event bus signals `Shutdown` or the channel
/// closes. All Slint updates are marshaled through `invoke_from_event_loop`,
/// so the Slint event loop is never blocked.
pub async fn run_event_loop(events: EventBus, window: Weak<MainWindow>) {
    let mut receiver = events.subscribe();

    while let Ok(event) = receiver.recv().await {
        match event {
            AppEvent::ScreenChanged(state) => {
                let window = window.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = window.upgrade() {
                        apply_ui_state(&w, state);
                    }
                });
            }
            AppEvent::ActionRejected { state, action } => {
                let window = window.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = window.upgrade() {
                        w.set_notice(
                            format!("Действие {action:?} недопустимо в состоянии {state:?}").into(),
                        );
                    }
                });
            }
            AppEvent::Shutdown => break,
            AppEvent::StartupCompleted(_) | AppEvent::StartupFailed(_) => {
                // Handled separately during startup; ignored here.
            }
        }
    }
}

/// Apply a `ScreenState` to the Slint window properties.
pub fn apply_ui_state(window: &MainWindow, state: ScreenState) {
    window.set_fixture_state(format!("{state:?}").into());
    window.set_fixture_state_title(crate::state::title(state).into());
    window.set_fixture_state_detail(crate::state::detail(state).into());
    window.set_fixture_state_badge(crate::state::badge(state).into());
    window.set_fixture_action_label(crate::state::action_label(state).into());
    window.set_notice(crate::state::notice(state).into());
}
