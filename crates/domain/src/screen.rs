//! Main screen state machine.
//!
//! The launcher preview and the future application core must agree on which
//! transitions are legal. Making the transitions explicit means an out-of-order
//! or duplicated asynchronous event cannot silently move the screen into an
//! impossible state.

use core::fmt;

/// Distinct states shown on the launcher's main screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScreenState {
    NotInstalled,
    Detecting,
    Verifying,
    UpdateAvailable,
    Downloading,
    Paused,
    Ready,
    Launching,
    Running,
    RepairRequired,
    Error,
}

/// A single input that may advance the main screen state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScreenAction {
    BeginSetup,
    DetectionFinished,
    VerificationFinished,
    DownloadStarted,
    DownloadCompleted,
    PauseRequested,
    ResumeRequested,
    LaunchStarted,
    LaunchConfirmed,
    ResetPreview,
    Fail,
    RequireRepair,
}

impl ScreenAction {
    /// Every action that is meaningful and can therefore appear in an event.
    pub const ALL: [Self; 12] = [
        Self::BeginSetup,
        Self::DetectionFinished,
        Self::VerificationFinished,
        Self::DownloadStarted,
        Self::DownloadCompleted,
        Self::PauseRequested,
        Self::ResumeRequested,
        Self::LaunchStarted,
        Self::LaunchConfirmed,
        Self::ResetPreview,
        Self::Fail,
        Self::RequireRepair,
    ];
}

impl ScreenState {
    /// Whether the screen is doing background work that should block the
    /// primary action.
    #[must_use]
    pub const fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Detecting | Self::Verifying | Self::Downloading | Self::Launching
        )
    }

    /// Whether the primary action is available in this state.
    #[must_use]
    pub const fn primary_action_available(self) -> bool {
        matches!(
            self,
            Self::NotInstalled | Self::RepairRequired | Self::Error | Self::Ready | Self::Running
        )
    }

    /// Apply an action, returning the next state or an error when the action is
    /// not valid in the current state.
    ///
    /// # Errors
    /// Returns [`InvalidTransition`] when `action` is not legal while in
    /// `self`.
    pub const fn apply(self, action: ScreenAction) -> Result<Self, InvalidTransition> {
        use ScreenAction as A;
        use ScreenState as S;

        let next = match action {
            A::BeginSetup => match self {
                S::NotInstalled | S::RepairRequired | S::Error => S::Detecting,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::DetectionFinished => match self {
                S::Detecting => S::Verifying,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::VerificationFinished => match self {
                S::Verifying => S::UpdateAvailable,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::DownloadStarted => match self {
                S::UpdateAvailable => S::Downloading,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::DownloadCompleted => match self {
                S::Downloading => S::Ready,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::PauseRequested => match self {
                S::Downloading => S::Paused,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::ResumeRequested => match self {
                S::Paused => S::Downloading,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::LaunchStarted => match self {
                S::Ready => S::Launching,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::LaunchConfirmed => match self {
                S::Launching => S::Running,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            A::ResetPreview => match self {
                S::Running => S::NotInstalled,
                _ => return Err(InvalidTransition::new(self, action)),
            },
            // A failure or a repair requirement may be reported from any state,
            // including when it is already active, so both are idempotent.
            A::Fail => S::Error,
            A::RequireRepair => S::RepairRequired,
        };

        Ok(next)
    }
}

/// Error returned when an action is not valid for the current state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidTransition {
    pub state: ScreenState,
    pub action: ScreenAction,
}

impl InvalidTransition {
    #[must_use]
    pub const fn new(state: ScreenState, action: ScreenAction) -> Self {
        Self { state, action }
    }
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "action {:?} is not valid while in state {:?}",
            self.action, self.state
        )
    }
}

impl std::error::Error for InvalidTransition {}

#[cfg(test)]
mod tests {
    use super::{InvalidTransition, ScreenAction, ScreenState};
    use ScreenAction as A;
    use ScreenState as S;

    #[test]
    fn detection_and_install_chain() {
        let mut state = S::NotInstalled;
        for (action, expected) in [
            (A::BeginSetup, S::Detecting),
            (A::DetectionFinished, S::Verifying),
            (A::VerificationFinished, S::UpdateAvailable),
            (A::DownloadStarted, S::Downloading),
            (A::DownloadCompleted, S::Ready),
        ] {
            state = state.apply(action).expect("valid transition");
            assert_eq!(state, expected);
        }
    }

    #[test]
    fn launch_and_reset_chain() {
        assert_eq!(S::Ready.apply(A::LaunchStarted), Ok(S::Launching));
        assert_eq!(S::Launching.apply(A::LaunchConfirmed), Ok(S::Running));
        assert_eq!(S::Running.apply(A::ResetPreview), Ok(S::NotInstalled));
    }

    #[test]
    fn pause_and_resume_round_trip() {
        assert_eq!(S::Downloading.apply(A::PauseRequested), Ok(S::Paused));
        assert_eq!(S::Paused.apply(A::ResumeRequested), Ok(S::Downloading));
    }

    #[test]
    fn failure_and_repair_are_available_from_any_state() {
        for state in [
            S::NotInstalled,
            S::Detecting,
            S::Verifying,
            S::UpdateAvailable,
            S::Downloading,
            S::Paused,
            S::Ready,
            S::Launching,
            S::Running,
            S::RepairRequired,
            S::Error,
        ] {
            assert_eq!(state.apply(A::Fail), Ok(S::Error));
            assert_eq!(state.apply(A::RequireRepair), Ok(S::RepairRequired));
        }
    }

    #[test]
    fn setup_can_restart_after_failure_or_repair() {
        assert_eq!(S::Error.apply(A::BeginSetup), Ok(S::Detecting));
        assert_eq!(S::RepairRequired.apply(A::BeginSetup), Ok(S::Detecting));
    }

    #[test]
    fn out_of_order_actions_are_rejected() {
        let cases = [
            (S::Ready, A::DetectionFinished),
            (S::Downloading, A::VerificationFinished),
            (S::Paused, A::DownloadStarted),
            (S::NotInstalled, A::LaunchStarted),
            (S::Ready, A::LaunchConfirmed),
            (S::Running, A::BeginSetup),
            (S::Paused, A::ResetPreview),
        ];
        for (state, action) in cases {
            assert_eq!(
                state.apply(action),
                Err(InvalidTransition::new(state, action)),
                "{state:?} should reject {action:?}"
            );
        }
    }

    #[test]
    fn every_action_resolves_for_at_least_one_state() {
        for action in A::ALL {
            assert!(
                [
                    S::NotInstalled,
                    S::Detecting,
                    S::Verifying,
                    S::UpdateAvailable,
                    S::Downloading,
                    S::Paused,
                    S::Ready,
                    S::Launching,
                    S::Running,
                    S::RepairRequired,
                    S::Error,
                ]
                .into_iter()
                .any(|state| state.apply(action).is_ok()),
                "{action:?} is not accepted from any state"
            );
        }
    }

    #[test]
    fn busy_and_primary_action_flags_match_states() {
        assert!(S::Detecting.is_busy());
        assert!(S::Downloading.is_busy());
        assert!(!S::Ready.is_busy());
        assert!(!S::Paused.is_busy());

        assert!(S::Ready.primary_action_available());
        assert!(S::Error.primary_action_available());
        assert!(!S::Detecting.primary_action_available());
        assert!(!S::Downloading.primary_action_available());
    }
}
