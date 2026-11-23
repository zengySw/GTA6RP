//! SQLite-backed local store.

use std::{path::Path, time::Duration};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use gta6rp_application::use_cases::LocalStatePort;

use crate::{
    Settings,
    error::StoreError,
    model::{DownloadState, DownloadTask, is_lowercase_sha256, now_millis},
};

/// Handle to the local SQLite database.
#[derive(Clone, Debug)]
pub struct LocalStore {
    pool: SqlitePool,
}

impl LocalStore {
    /// Open (creating if needed) a local database file and apply migrations.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the directory cannot be created, the
    /// database cannot be opened, or migrations fail.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Open an ephemeral in-memory database, primarily for tests.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the database cannot be opened or migrations
    /// fail.
    pub async fn open_in_memory() -> Result<Self, StoreError> {
        // One connection keeps every query on the same in-memory database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Close the connection pool and wait for connections to be released.
    pub async fn close(self) {
        self.pool.close().await;
    }

    /// Read the current settings.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the row is missing, the query fails, or the
    /// stored values are out of range.
    pub async fn settings(&self) -> Result<Settings, StoreError> {
        let row = sqlx::query_as::<_, SettingsRow>(
            "SELECT language, install_root, auto_start, close_to_tray, \
             notifications_enabled, max_parallel_downloads, \
             download_limit_bytes_per_second \
             FROM local_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        row.try_into()
    }

    /// Persist settings after validating them.
    ///
    /// # Errors
    /// Returns [`StoreError::InvalidInput`] when validation fails, or
    /// [`StoreError`] when the update fails.
    pub async fn save_settings(&self, settings: Settings) -> Result<(), StoreError> {
        settings.validate()?;
        let install_root = match &settings.install_root {
            Some(root) => Some(
                root.to_str()
                    .ok_or_else(|| StoreError::invalid("install root must be valid UTF-8"))?
                    .to_owned(),
            ),
            None => None,
        };
        let max_parallel = i64::from(settings.max_parallel_downloads);

        sqlx::query(
            "UPDATE local_settings SET language = $1, install_root = $2, auto_start = $3, \
             close_to_tray = $4, notifications_enabled = $5, max_parallel_downloads = $6, \
             download_limit_bytes_per_second = $7, updated_at_ms = $8 WHERE id = 1",
        )
        .bind(&settings.language)
        .bind(install_root)
        .bind(i64::from(settings.auto_start))
        .bind(i64::from(settings.close_to_tray))
        .bind(i64::from(settings.notifications_enabled))
        .bind(max_parallel)
        .bind(optional_i64(settings.download_limit_bytes_per_second)?)
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read the active manifest id, if any.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the row is missing or the query fails.
    pub async fn active_manifest_id(&self) -> Result<Option<String>, StoreError> {
        let row = sqlx::query_as::<_, StateRow>(
            "SELECT active_manifest_id FROM installation_state WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.active_manifest_id)
    }

    /// Set or clear the active manifest id.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the update fails.
    pub async fn set_active_manifest_id(
        &self,
        manifest_id: Option<&str>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE installation_state SET active_manifest_id = $1, updated_at_ms = $2 \
             WHERE id = 1",
        )
        .bind(manifest_id)
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a download task or update its progress and state.
    ///
    /// Immutable fields (`id`, `manifest_id`, `relative_path`, `url`,
    /// `expected_size`, `sha256`, `created_at_ms`) are preserved on conflict.
    ///
    /// # Errors
    /// Returns [`StoreError::InvalidInput`] when the task fails validation, or
    /// [`StoreError`] when the write fails.
    pub async fn upsert_task(&self, task: &DownloadTask) -> Result<(), StoreError> {
        task.validate()?;
        let expected_size = to_i64(task.expected_size, "expected_size")?;
        let downloaded = to_i64(task.downloaded_bytes, "downloaded_bytes")?;
        let attempts = i64::from(task.attempts);

        sqlx::query(
            "INSERT INTO download_tasks \
             (id, manifest_id, relative_path, url, expected_size, downloaded_bytes, sha256, \
              state, attempts, last_error, created_at_ms, updated_at_ms) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             ON CONFLICT(id) DO UPDATE SET \
              downloaded_bytes = excluded.downloaded_bytes, \
              state = excluded.state, \
              attempts = excluded.attempts, \
              last_error = excluded.last_error, \
              updated_at_ms = excluded.updated_at_ms",
        )
        .bind(&task.id)
        .bind(&task.manifest_id)
        .bind(&task.relative_path)
        .bind(&task.url)
        .bind(expected_size)
        .bind(downloaded)
        .bind(&task.sha256)
        .bind(task.state.as_str())
        .bind(attempts)
        .bind(&task.last_error)
        .bind(task.created_at_ms)
        .bind(task.updated_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load every download task ordered by creation time.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the query fails or a row is corrupt.
    pub async fn tasks(&self) -> Result<Vec<DownloadTask>, StoreError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, manifest_id, relative_path, url, expected_size, downloaded_bytes, \
             sha256, state, attempts, last_error, created_at_ms, updated_at_ms \
             FROM download_tasks ORDER BY created_at_ms, rowid",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(DownloadTask::try_from).collect()
    }

    /// Move tasks that were mid-flight during an unclean shutdown back to a
    /// resumable state.
    ///
    /// Returns the number of tasks that were changed.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the update fails.
    pub async fn recover_interrupted(&self) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "UPDATE download_tasks \
             SET state = 'paused', \
                 last_error = COALESCE(last_error, 'interrupted by restart'), \
                 updated_at_ms = $1 \
             WHERE state IN ('downloading', 'verifying')",
        )
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

impl LocalStatePort for LocalStore {
    type Error = StoreError;

    async fn load_settings(&self) -> Result<Settings, StoreError> {
        self.settings().await
    }

    async fn save_settings(&self, settings: Settings) -> Result<(), StoreError> {
        LocalStore::save_settings(self, settings).await
    }

    async fn active_manifest_id(&self) -> Result<Option<String>, StoreError> {
        self.active_manifest_id().await
    }

    async fn recover_interrupted(&self) -> Result<u64, StoreError> {
        self.recover_interrupted().await
    }
}

#[derive(sqlx::FromRow)]
struct SettingsRow {
    language: String,
    install_root: Option<String>,
    auto_start: bool,
    close_to_tray: bool,
    notifications_enabled: bool,
    max_parallel_downloads: i64,
    download_limit_bytes_per_second: Option<i64>,
}

impl TryFrom<SettingsRow> for Settings {
    type Error = StoreError;

    fn try_from(row: SettingsRow) -> Result<Self, Self::Error> {
        let max_parallel_downloads = u8::try_from(row.max_parallel_downloads)
            .map_err(|_| StoreError::corrupt("max_parallel_downloads is out of range"))?;
        let download_limit_bytes_per_second = row
            .download_limit_bytes_per_second
            .map(|limit| {
                u64::try_from(limit).map_err(|_| StoreError::corrupt("download limit is negative"))
            })
            .transpose()?;

        Ok(Self {
            language: row.language,
            install_root: row.install_root.map(std::path::PathBuf::from),
            auto_start: row.auto_start,
            close_to_tray: row.close_to_tray,
            notifications_enabled: row.notifications_enabled,
            max_parallel_downloads,
            download_limit_bytes_per_second,
        })
    }
}

#[derive(sqlx::FromRow)]
struct StateRow {
    active_manifest_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    manifest_id: String,
    relative_path: String,
    url: String,
    expected_size: i64,
    downloaded_bytes: i64,
    sha256: String,
    state: String,
    attempts: i64,
    last_error: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl TryFrom<TaskRow> for DownloadTask {
    type Error = StoreError;

    fn try_from(row: TaskRow) -> Result<Self, Self::Error> {
        let expected_size = u64::try_from(row.expected_size)
            .map_err(|_| StoreError::corrupt("expected_size is negative"))?;
        let downloaded_bytes = u64::try_from(row.downloaded_bytes)
            .map_err(|_| StoreError::corrupt("downloaded_bytes is negative"))?;
        let attempts = u32::try_from(row.attempts)
            .map_err(|_| StoreError::corrupt("attempts is out of range"))?;
        let state = DownloadState::parse(&row.state)
            .ok_or_else(|| StoreError::corrupt(format!("unknown download state {}", row.state)))?;
        if !is_lowercase_sha256(&row.sha256) {
            return Err(StoreError::corrupt("stored sha256 is not lowercase hex"));
        }

        Ok(Self {
            id: row.id,
            manifest_id: row.manifest_id,
            relative_path: row.relative_path,
            url: row.url,
            expected_size,
            downloaded_bytes,
            sha256: row.sha256,
            state,
            attempts,
            last_error: row.last_error,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
        })
    }
}

fn to_i64(value: u64, field: &str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::invalid(format!("{field} is too large")))
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>, StoreError> {
    value
        .map(|value| to_i64(value, "download limit"))
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::LocalStore;
    use crate::{
        Settings,
        error::StoreError,
        model::{DownloadState, DownloadTask},
    };
    use gta6rp_application::use_cases::{LocalStatePort, prepare_startup};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(tag: &str) -> Self {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "gta6rp-store-{tag}-{}-{counter}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sample_task(id: &str, relative_path: &str, state: DownloadState) -> DownloadTask {
        let mut task = DownloadTask::queued(
            id,
            "fixture-001",
            relative_path,
            "https://cdn.example.invalid/objects/example",
            1_048_576,
            "a".repeat(64),
        );
        task.state = state;
        task
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let settings = Settings {
            language: "uk".to_owned(),
            install_root: Some(PathBuf::from("C:/Games/GTA6RP")),
            auto_start: true,
            max_parallel_downloads: 4,
            download_limit_bytes_per_second: Some(2_000_000),
            ..Settings::default()
        };

        store.save_settings(settings.clone()).await.expect("save");
        assert_eq!(store.settings().await.expect("read"), settings);
    }

    #[tokio::test]
    async fn settings_validation_is_enforced() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let settings = Settings {
            max_parallel_downloads: 0,
            ..Settings::default()
        };
        assert!(matches!(
            store.save_settings(settings).await,
            Err(StoreError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn active_manifest_round_trip() {
        let store = LocalStore::open_in_memory().await.expect("open");
        assert_eq!(store.active_manifest_id().await.expect("read"), None);
        store
            .set_active_manifest_id(Some("fixture-win64-002"))
            .await
            .expect("set");
        assert_eq!(
            store.active_manifest_id().await.expect("read"),
            Some("fixture-win64-002".to_owned())
        );
        store.set_active_manifest_id(None).await.expect("clear");
        assert_eq!(store.active_manifest_id().await.expect("read"), None);
    }

    #[tokio::test]
    async fn inserts_and_lists_tasks() {
        let store = LocalStore::open_in_memory().await.expect("open");
        store
            .upsert_task(&sample_task("a", "data/a.bin", DownloadState::Queued))
            .await
            .expect("insert a");
        store
            .upsert_task(&sample_task("b", "data/b.bin", DownloadState::Completed))
            .await
            .expect("insert b");

        let tasks = store.tasks().await.expect("list");
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().any(|task| task.id == "a"));
        assert!(tasks.iter().any(|task| task.id == "b"));
    }

    #[tokio::test]
    async fn upsert_updates_progress() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let mut task = sample_task("a", "data/a.bin", DownloadState::Queued);
        store.upsert_task(&task).await.expect("insert");

        task.downloaded_bytes = 512;
        task.state = DownloadState::Downloading;
        task.attempts = 1;
        store.upsert_task(&task).await.expect("update");

        let tasks = store.tasks().await.expect("list");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].downloaded_bytes, 512);
        assert_eq!(tasks[0].state, DownloadState::Downloading);
        assert_eq!(tasks[0].attempts, 1);
    }

    #[tokio::test]
    async fn rejects_invalid_task() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let mut task = sample_task("a", "../escape.bin", DownloadState::Queued);
        task.relative_path = "../escape.bin".to_owned();
        assert!(matches!(
            store.upsert_task(&task).await,
            Err(StoreError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn recovers_interrupted_tasks_only() {
        let store = LocalStore::open_in_memory().await.expect("open");
        for (id, state) in [
            ("down", DownloadState::Downloading),
            ("verify", DownloadState::Verifying),
            ("done", DownloadState::Completed),
            ("queued", DownloadState::Queued),
        ] {
            let path = format!("data/{id}.bin");
            store
                .upsert_task(&sample_task(id, &path, state))
                .await
                .expect("insert");
        }

        assert_eq!(store.recover_interrupted().await.expect("recover"), 2);
        let tasks = store.tasks().await.expect("list");
        for task in &tasks {
            match task.id.as_str() {
                "down" | "verify" => {
                    assert_eq!(task.state, DownloadState::Paused);
                    assert!(
                        task.last_error
                            .as_deref()
                            .is_some_and(|error| error.contains("interrupted"))
                    );
                }
                "done" => assert_eq!(task.state, DownloadState::Completed),
                "queued" => assert_eq!(task.state, DownloadState::Queued),
                other => panic!("unexpected task {other}"),
            }
        }

        // Recovery is idempotent: nothing else is transitional.
        assert_eq!(store.recover_interrupted().await.expect("recover"), 0);
    }

    #[tokio::test]
    async fn state_survives_reopen() {
        let directory = TestDir::new("reopen");
        let path = directory.path.join("launcher.db");

        let store = LocalStore::open(&path).await.expect("open");
        let settings = Settings {
            language: "uk".to_owned(),
            ..Settings::default()
        };
        store.save_settings(settings.clone()).await.expect("save");
        store
            .set_active_manifest_id(Some("fixture-win64-001"))
            .await
            .expect("set manifest");
        store
            .upsert_task(&sample_task("a", "data/a.bin", DownloadState::Paused))
            .await
            .expect("insert");
        store.close().await;

        let reopened = LocalStore::open(&path).await.expect("reopen");
        assert_eq!(reopened.settings().await.expect("settings"), settings);
        assert_eq!(
            reopened.active_manifest_id().await.expect("manifest"),
            Some("fixture-win64-001".to_owned())
        );
        assert_eq!(reopened.tasks().await.expect("tasks").len(), 1);
        reopened.close().await;
    }

    #[tokio::test]
    async fn local_state_port_prepare_startup_on_fresh_store() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let report = prepare_startup(&store).await.expect("startup");
        assert_eq!(report.settings, Settings::default());
        assert_eq!(report.active_manifest_id, None);
        assert_eq!(report.recovered_tasks, 0);
    }

    #[tokio::test]
    async fn local_state_port_settings_persistence_through_trait() {
        let store = LocalStore::open_in_memory().await.expect("open");
        let settings = Settings {
            language: "de".to_owned(),
            max_parallel_downloads: 5,
            ..Settings::default()
        };

        LocalStatePort::save_settings(&store, settings.clone())
            .await
            .expect("save");
        let loaded = LocalStatePort::load_settings(&store).await.expect("load");
        assert_eq!(loaded, settings);
    }

    #[tokio::test]
    async fn local_state_port_recover_interrupted_through_trait() {
        let store = LocalStore::open_in_memory().await.expect("open");
        store
            .upsert_task(&sample_task("a", "data/a.bin", DownloadState::Downloading))
            .await
            .expect("insert");
        store
            .upsert_task(&sample_task("b", "data/b.bin", DownloadState::Completed))
            .await
            .expect("insert");

        let recovered = LocalStatePort::recover_interrupted(&store)
            .await
            .expect("recover");
        assert_eq!(recovered, 1);

        let tasks = store.tasks().await.expect("tasks");
        let a = tasks.iter().find(|t| t.id == "a").expect("task a");
        assert_eq!(a.state, DownloadState::Paused);
        let b = tasks.iter().find(|t| t.id == "b").expect("task b");
        assert_eq!(b.state, DownloadState::Completed);
    }
}
