//! Application use cases for the launcher's local state.
//!
//! These functions encode the order and validation rules that the UI must not
//! own. They depend only on [`LocalStatePort`]; the concrete SQLite adapter is
//! supplied by `gta6rp-local-store`, so the application layer stays independent
//! of the storage engine.

use core::fmt;
use std::future::Future;

use gta6rp_domain::settings::{Settings, SettingsError};

/// Storage operations required by the local-state use cases.
///
/// Implementations are expected to be cheap to reference and safe to call from
/// a background task. Each method returns a future so the caller can await it
/// without blocking the runtime.
pub trait LocalStatePort {
    /// Error returned by the concrete store.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Read the persisted settings.
    fn load_settings(&self) -> impl Future<Output = Result<Settings, Self::Error>> + Send;

    /// Persist validated settings.
    fn save_settings(
        &self,
        settings: Settings,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Read the active manifest id, if any.
    fn active_manifest_id(
        &self,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;

    /// Move interrupted download tasks back to a resumable state.
    fn recover_interrupted(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;
}

/// A failure while running a local-state use case.
#[derive(Debug)]
pub enum UseCaseError<E> {
    /// The requested settings are invalid and were not persisted.
    InvalidSettings(SettingsError),
    /// The backing store failed.
    Storage(E),
}

impl<E: fmt::Display> fmt::Display for UseCaseError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSettings(error) => write!(formatter, "invalid settings: {error}"),
            Self::Storage(error) => write!(formatter, "local state unavailable: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for UseCaseError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSettings(error) => Some(error),
            Self::Storage(error) => Some(error),
        }
    }
}

/// Snapshot produced by [`prepare_startup`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupReport {
    pub settings: Settings,
    pub active_manifest_id: Option<String>,
    /// Number of download tasks recovered from an unclean shutdown.
    pub recovered_tasks: u64,
}

/// Load the persisted settings.
///
/// # Errors
/// Returns [`UseCaseError::Storage`] when the store cannot be read.
pub async fn load_settings<P>(port: &P) -> Result<Settings, UseCaseError<P::Error>>
where
    P: LocalStatePort + Sync,
{
    port.load_settings().await.map_err(UseCaseError::Storage)
}

/// Validate and persist settings.
///
/// Validation happens before the write, so invalid settings never reach the
/// database. The validated value is returned for the caller to apply.
///
/// # Errors
/// Returns [`UseCaseError::InvalidSettings`] when validation fails, or
/// [`UseCaseError::Storage`] when the write fails.
pub async fn update_settings<P>(
    port: &P,
    settings: Settings,
) -> Result<Settings, UseCaseError<P::Error>>
where
    P: LocalStatePort + Sync,
{
    settings.validate().map_err(UseCaseError::InvalidSettings)?;
    port.save_settings(settings.clone())
        .await
        .map_err(UseCaseError::Storage)?;
    Ok(settings)
}

/// Prepare local state at startup: load settings, report the active manifest,
/// and recover tasks interrupted by a previous unclean shutdown.
///
/// # Errors
/// Returns [`UseCaseError::Storage`] when any read or recovery step fails.
pub async fn prepare_startup<P>(port: &P) -> Result<StartupReport, UseCaseError<P::Error>>
where
    P: LocalStatePort + Sync,
{
    let settings = port.load_settings().await.map_err(UseCaseError::Storage)?;
    let active_manifest_id = port
        .active_manifest_id()
        .await
        .map_err(UseCaseError::Storage)?;
    let recovered_tasks = port
        .recover_interrupted()
        .await
        .map_err(UseCaseError::Storage)?;
    Ok(StartupReport {
        settings,
        active_manifest_id,
        recovered_tasks,
    })
}

#[cfg(test)]
mod tests {
    use std::{future::Future, sync::Mutex};

    use gta6rp_domain::settings::{Settings, SettingsError};

    use super::{LocalStatePort, UseCaseError, load_settings, prepare_startup, update_settings};

    #[derive(Debug, PartialEq, Eq)]
    struct FakeError(&'static str);

    impl core::fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl std::error::Error for FakeError {}

    struct FakePort {
        settings: Mutex<Settings>,
        manifest: Mutex<Option<String>>,
        recovered: Mutex<u64>,
        fail_load: bool,
    }

    impl FakePort {
        fn new(settings: Settings) -> Self {
            Self {
                settings: Mutex::new(settings),
                manifest: Mutex::new(None),
                recovered: Mutex::new(0),
                fail_load: false,
            }
        }

        fn failing() -> Self {
            Self {
                fail_load: true,
                ..Self::new(Settings::default())
            }
        }
    }

    impl LocalStatePort for FakePort {
        type Error = FakeError;

        fn load_settings(&self) -> impl Future<Output = Result<Settings, Self::Error>> + Send {
            let result = if self.fail_load {
                Err(FakeError("unavailable"))
            } else {
                self.settings
                    .lock()
                    .map(|settings| settings.clone())
                    .map_err(|_| FakeError("poisoned"))
            };
            std::future::ready(result)
        }

        fn save_settings(
            &self,
            settings: Settings,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            let result = self
                .settings
                .lock()
                .map(|mut stored| {
                    *stored = settings;
                })
                .map_err(|_| FakeError("poisoned"));
            std::future::ready(result)
        }

        fn active_manifest_id(
            &self,
        ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send {
            let result = self
                .manifest
                .lock()
                .map(|manifest| manifest.clone())
                .map_err(|_| FakeError("poisoned"));
            std::future::ready(result)
        }

        fn recover_interrupted(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send {
            let result = self
                .recovered
                .lock()
                .map(|mut recovered| {
                    let count = *recovered;
                    *recovered = 0;
                    count
                })
                .map_err(|_| FakeError("poisoned"));
            std::future::ready(result)
        }
    }

    #[tokio::test]
    async fn loads_settings() {
        let port = FakePort::new(Settings {
            language: "uk".to_owned(),
            ..Settings::default()
        });
        let settings = load_settings(&port).await.expect("load");
        assert_eq!(settings.language, "uk");
    }

    #[tokio::test]
    async fn update_settings_rejects_invalid_values_without_writing() {
        let port = FakePort::new(Settings::default());
        let invalid = Settings {
            max_parallel_downloads: 0,
            ..Settings::default()
        };

        let result = update_settings(&port, invalid).await;
        assert!(matches!(
            result,
            Err(UseCaseError::InvalidSettings(
                SettingsError::ParallelDownloadsOutOfRange
            ))
        ));
        assert_eq!(
            port.settings
                .lock()
                .expect("settings")
                .max_parallel_downloads,
            3
        );
    }

    #[tokio::test]
    async fn update_settings_persists_valid_values() {
        let port = FakePort::new(Settings::default());
        let updated = Settings {
            language: "de".to_owned(),
            max_parallel_downloads: 5,
            ..Settings::default()
        };

        assert_eq!(
            update_settings(&port, updated.clone())
                .await
                .expect("update"),
            updated
        );
        assert_eq!(port.settings.lock().expect("settings").language, "de");
    }

    #[tokio::test]
    async fn startup_reports_state_and_recovery() {
        let port = FakePort::new(Settings::default());
        *port.manifest.lock().expect("manifest") = Some("fixture-001".to_owned());
        *port.recovered.lock().expect("recovered") = 3;

        let report = prepare_startup(&port).await.expect("startup");
        assert_eq!(report.settings, Settings::default());
        assert_eq!(report.active_manifest_id.as_deref(), Some("fixture-001"));
        assert_eq!(report.recovered_tasks, 3);
        // Recovery is reported once and cleared in the fake.
        assert_eq!(port.recovered.lock().expect("recovered").to_owned(), 0);
    }

    #[tokio::test]
    async fn startup_surfaces_storage_failure() {
        let port = FakePort::failing();
        assert!(matches!(
            prepare_startup(&port).await,
            Err(UseCaseError::Storage(FakeError("unavailable")))
        ));
    }
}
