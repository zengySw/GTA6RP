//! Value types persisted in the local database.

use std::fmt;

use crate::error::StoreError;
use gta6rp_domain::path::RelativePath;

/// Lifecycle of a single download task, persisted across restarts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadState {
    /// The stable string stored in the database.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse a stored state string.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "downloading" => Some(Self::Downloading),
            "paused" => Some(Self::Paused),
            "verifying" => Some(Self::Verifying),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Whether the state only exists while a process is actively working and
    /// therefore must be recovered after an unclean restart.
    #[must_use]
    pub const fn is_transitional(self) -> bool {
        matches!(self, Self::Downloading | Self::Verifying)
    }
}

impl fmt::Display for DownloadState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A persisted download checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadTask {
    pub id: String,
    pub manifest_id: String,
    pub relative_path: String,
    pub url: String,
    pub expected_size: u64,
    pub downloaded_bytes: u64,
    pub sha256: String,
    pub state: DownloadState,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl DownloadTask {
    /// Create a queued task with no progress yet.
    #[must_use]
    pub fn queued(
        id: impl Into<String>,
        manifest_id: impl Into<String>,
        relative_path: impl Into<String>,
        url: impl Into<String>,
        expected_size: u64,
        sha256: impl Into<String>,
    ) -> Self {
        let now = now_millis();
        Self {
            id: id.into(),
            manifest_id: manifest_id.into(),
            relative_path: relative_path.into(),
            url: url.into(),
            expected_size,
            downloaded_bytes: 0,
            sha256: sha256.into(),
            state: DownloadState::Queued,
            attempts: 0,
            last_error: None,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    /// Validate the task before it is persisted.
    ///
    /// # Errors
    /// Returns [`StoreError::InvalidInput`] for an empty identifier, an unsafe
    /// relative path, an out-of-range digest, or inconsistent progress.
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.id.trim().is_empty() {
            return Err(StoreError::invalid("task id must not be empty"));
        }
        if self.manifest_id.trim().is_empty() {
            return Err(StoreError::invalid("manifest id must not be empty"));
        }
        RelativePath::parse(&self.relative_path)
            .map_err(|error| StoreError::invalid(format!("relative path: {error}")))?;
        if self.url.trim().is_empty() {
            return Err(StoreError::invalid("download url must not be empty"));
        }
        if self.downloaded_bytes > self.expected_size {
            return Err(StoreError::invalid(
                "downloaded bytes must not exceed the expected size",
            ));
        }
        if !is_lowercase_sha256(&self.sha256) {
            return Err(StoreError::invalid(
                "sha256 must be 64 lowercase hexadecimal characters",
            ));
        }
        Ok(())
    }
}

pub(crate) fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn now_millis() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    i64::try_from(millis).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{DownloadState, DownloadTask, now_millis};

    fn valid_task() -> DownloadTask {
        DownloadTask::queued(
            "task-1",
            "fixture-001",
            "data/example.bin",
            "https://cdn.example.invalid/objects/example",
            1024,
            "a".repeat(64),
        )
    }

    #[test]
    fn state_strings_round_trip() {
        for state in [
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Verifying,
            DownloadState::Completed,
            DownloadState::Failed,
            DownloadState::Cancelled,
        ] {
            assert_eq!(DownloadState::parse(state.as_str()), Some(state));
        }
        assert_eq!(DownloadState::parse("unknown"), None);
    }

    #[test]
    fn transitions_are_flagged() {
        assert!(DownloadState::Downloading.is_transitional());
        assert!(DownloadState::Verifying.is_transitional());
        assert!(!DownloadState::Paused.is_transitional());
        assert!(!DownloadState::Completed.is_transitional());
    }

    #[test]
    fn valid_task_passes_validation() {
        assert!(valid_task().validate().is_ok());
    }

    #[test]
    fn rejects_unsafe_or_inconsistent_task() {
        let mut task = valid_task();
        task.relative_path = "../escape.bin".to_owned();
        assert!(task.validate().is_err());

        let mut task = valid_task();
        task.sha256 = "A".repeat(64);
        assert!(task.validate().is_err());

        let mut task = valid_task();
        task.downloaded_bytes = task.expected_size + 1;
        assert!(task.validate().is_err());

        let mut task = valid_task();
        task.url.clear();
        assert!(task.validate().is_err());
    }

    #[test]
    fn queued_task_starts_with_zero_progress() {
        let task = valid_task();
        assert_eq!(task.state, DownloadState::Queued);
        assert_eq!(task.downloaded_bytes, 0);
        assert_eq!(task.attempts, 0);
        assert_eq!(task.created_at_ms, task.updated_at_ms);
        assert!(now_millis() >= task.created_at_ms);
    }
}
