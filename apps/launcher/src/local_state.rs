//! Local state integration for the launcher.
//!
//! Manages the SQLite-backed `LocalStore` lifecycle: opening the database,
//! running startup preparation (settings, active manifest, recovery), and
//! providing settings persistence through the `LocalStatePort` interface.
//! All operations run on the Tokio thread to avoid blocking the Slint UI.

use std::path::PathBuf;

use gta6rp_application::use_cases::{LocalStatePort, StartupReport, prepare_startup};
use gta6rp_domain::settings::Settings;
use gta6rp_installation::{FreeSpaceInfo, ensure_free_space};
use gta6rp_local_store::LocalStore;

/// Error type for local state operations.
#[derive(Debug)]
pub enum LocalStateError {
    /// The database directory or file could not be prepared.
    Io(std::io::Error),
    /// The database could not be opened or migrations failed.
    Store(gta6rp_local_store::error::StoreError),
    /// A use-case operation failed.
    UseCase(gta6rp_application::use_cases::UseCaseError<gta6rp_local_store::error::StoreError>),
    /// Free space check failed.
    Installation(gta6rp_installation::InstallError),
}

impl std::fmt::Display for LocalStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "local state I/O error: {error}"),
            Self::Store(error) => write!(formatter, "local store error: {error}"),
            Self::UseCase(error) => write!(formatter, "local state use case error: {error}"),
            Self::Installation(error) => write!(formatter, "installation error: {error}"),
        }
    }
}

impl std::error::Error for LocalStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::UseCase(error) => Some(error),
            Self::Installation(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for LocalStateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<gta6rp_local_store::error::StoreError> for LocalStateError {
    fn from(error: gta6rp_local_store::error::StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<gta6rp_application::use_cases::UseCaseError<gta6rp_local_store::error::StoreError>>
    for LocalStateError
{
    fn from(
        error: gta6rp_application::use_cases::UseCaseError<gta6rp_local_store::error::StoreError>,
    ) -> Self {
        Self::UseCase(error)
    }
}

impl From<gta6rp_installation::InstallError> for LocalStateError {
    fn from(error: gta6rp_installation::InstallError) -> Self {
        Self::Installation(error)
    }
}

/// Determines the local database file path.
///
/// Prefers the system configuration directory (e.g. `~/.config/gta6rp/` on
/// Linux, `%APPDATA%\GTA6RP\` on Windows). Falls back to a file alongside the
/// executable if the config directory cannot be determined.
pub fn database_path() -> Result<PathBuf, std::io::Error> {
    // Try system config directory first.
    #[cfg(target_os = "linux")]
    fn config_dir() -> Option<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .map(|dir| dir.join("gta6rp"))
    }

    #[cfg(target_os = "windows")]
    fn config_dir() -> Option<PathBuf> {
        std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)
            .map(|dir| dir.join("GTA6RP"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn config_dir() -> Option<PathBuf> {
        None
    }

    if let Some(app_dir) = config_dir() {
        std::fs::create_dir_all(&app_dir)?;
        return Ok(app_dir.join("launcher.db"));
    }

    // Fallback: alongside the executable.
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        return Ok(exe_dir.join("launcher.db"));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "cannot determine local state directory",
    ))
}

#[derive(Clone)]
/// Handle to the local state subsystem.
///
/// Holds the `LocalStore` and provides methods for reading and saving
/// settings. All methods must be called on the Tokio thread.
pub struct LocalStateHandle {
    store: LocalStore,
}

impl LocalStateHandle {
    /// Open the local database and run startup preparation.
    ///
    /// Returns the [`StartupReport`] with settings, active manifest id, and
    /// recovered task count.
    pub async fn open() -> Result<(Self, StartupReport), LocalStateError> {
        let path = database_path()?;
        let store = LocalStore::open(&path).await?;
        let report = prepare_startup(&store).await?;
        Ok((Self { store }, report))
    }

    /// Read the current settings.
    pub async fn settings(&self) -> Result<Settings, LocalStateError> {
        Ok(self.store.settings().await?)
    }

    /// Save settings after validation.
    ///
    /// Validation happens before the write, so invalid settings never reach
    /// the database.
    pub async fn save_settings(&self, settings: Settings) -> Result<(), LocalStateError> {
        self.store.save_settings(settings).await?;
        Ok(())
    }

    /// Read the active manifest id, if any.
    pub async fn active_manifest_id(&self) -> Result<Option<String>, LocalStateError> {
        Ok(self.store.active_manifest_id().await?)
    }

    /// Close the database connection pool.
    pub async fn close(self) {
        self.store.close().await;
    }

    /// Check whether there is enough free space at `path` for `required_bytes`.
    ///
    /// All operations run on the Tokio thread to avoid blocking the Slint UI.
    pub fn check_free_space(
        path: &std::path::Path,
        required_bytes: u64,
    ) -> Result<FreeSpaceInfo, LocalStateError> {
        ensure_free_space(path, required_bytes).map_err(Into::into)
    }
}

impl LocalStatePort for LocalStateHandle {
    type Error = LocalStateError;

    async fn load_settings(&self) -> Result<Settings, Self::Error> {
        self.settings().await
    }

    async fn save_settings(&self, settings: Settings) -> Result<(), Self::Error> {
        self.save_settings(settings).await
    }

    async fn active_manifest_id(&self) -> Result<Option<String>, Self::Error> {
        self.active_manifest_id().await
    }

    async fn recover_interrupted(&self) -> Result<u64, Self::Error> {
        Ok(self.store.recover_interrupted().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_path_returns_valid_path() {
        let path = database_path().expect("database path should be resolvable");
        assert!(path.is_absolute());
        assert_eq!(path.file_name().unwrap(), "launcher.db");
    }
}
