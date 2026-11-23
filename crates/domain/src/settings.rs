//! Non-secret launcher preferences.
//!
//! Settings are a domain concept: they describe desired launcher behaviour
//! without depending on the storage mechanism. The local store persists them,
//! and the API exposes a separate per-account copy for server-side settings.

use std::{fmt, path::PathBuf};

/// Maximum number of parallel downloads the launcher supports.
pub const MAX_PARALLEL_DOWNLOADS: u8 = 8;

/// Non-secret launcher preferences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub language: String,
    pub install_root: Option<PathBuf>,
    pub auto_start: bool,
    pub close_to_tray: bool,
    pub notifications_enabled: bool,
    pub max_parallel_downloads: u8,
    pub download_limit_bytes_per_second: Option<u64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".to_owned(),
            install_root: None,
            auto_start: false,
            close_to_tray: false,
            notifications_enabled: true,
            max_parallel_downloads: 3,
            download_limit_bytes_per_second: None,
        }
    }
}

impl Settings {
    /// Validate settings before they are applied or persisted.
    ///
    /// # Errors
    /// Returns [`SettingsError`] describing the first invalid field.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.language.trim().is_empty() || self.language.len() > 16 {
            return Err(SettingsError::InvalidLanguage);
        }
        if !(1..=MAX_PARALLEL_DOWNLOADS).contains(&self.max_parallel_downloads) {
            return Err(SettingsError::ParallelDownloadsOutOfRange);
        }
        if self
            .download_limit_bytes_per_second
            .is_some_and(|limit| limit == 0)
        {
            return Err(SettingsError::ZeroDownloadLimit);
        }
        if let Some(root) = &self.install_root
            && root.to_str().is_none()
        {
            return Err(SettingsError::NonUtf8InstallRoot);
        }
        Ok(())
    }
}

/// Reasons a [`Settings`] value was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsError {
    InvalidLanguage,
    ParallelDownloadsOutOfRange,
    ZeroDownloadLimit,
    NonUtf8InstallRoot,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidLanguage => "language must be non-empty and at most 16 bytes",
            Self::ParallelDownloadsOutOfRange => "max_parallel_downloads must be 1..=8",
            Self::ZeroDownloadLimit => "download limit must be greater than zero",
            Self::NonUtf8InstallRoot => "install root must be a valid UTF-8 path",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::{Settings, SettingsError};

    #[test]
    fn default_settings_are_valid() {
        let settings = Settings::default();
        assert_eq!(settings.language, "en");
        assert_eq!(settings.max_parallel_downloads, 3);
        assert!(settings.install_root.is_none());
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_language() {
        let settings = Settings {
            language: "  ".to_owned(),
            ..Settings::default()
        };
        assert_eq!(settings.validate(), Err(SettingsError::InvalidLanguage));

        let settings = Settings {
            language: "x".repeat(17),
            ..Settings::default()
        };
        assert_eq!(settings.validate(), Err(SettingsError::InvalidLanguage));
    }

    #[test]
    fn rejects_out_of_range_parallel_downloads() {
        for value in [0, 9] {
            let settings = Settings {
                max_parallel_downloads: value,
                ..Settings::default()
            };
            assert_eq!(
                settings.validate(),
                Err(SettingsError::ParallelDownloadsOutOfRange)
            );
        }
        let settings = Settings {
            max_parallel_downloads: 8,
            ..Settings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_zero_download_limit() {
        let settings = Settings {
            download_limit_bytes_per_second: Some(0),
            ..Settings::default()
        };
        assert_eq!(settings.validate(), Err(SettingsError::ZeroDownloadLimit));
    }

    #[test]
    fn accepts_valid_install_root() {
        let settings = Settings {
            install_root: Some(std::path::PathBuf::from("C:/Games/GTA6RP")),
            ..Settings::default()
        };
        assert!(settings.validate().is_ok());
    }
}
