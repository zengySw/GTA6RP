//! Download engine: single-file downloads with resume, verification,
//! and atomic placement.
//!
//! MVP 5.1 covers downloading one HTTPS file into a managed installation root
//! with streaming writes, HTTP status/size checks, progress reporting,
//! SHA-256 verification, and atomic move into place. Later MVPs add the queue,
//! concurrency limits, pause/resume/cancel, retries, and SQLite persistence.

use core::fmt;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use gta6rp_domain::path::RelativePath;
use gta6rp_installation::{SafeRoot, ensure_free_space};
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const READ_BUFFER_SIZE: usize = 8 * 1024;
const TMP_PREFIX: &str = ".gta6rp-download-";

pub mod queue;

pub use queue::{DownloadQueue, QueueConfig, QueueEntry, QueueError};

/// Lifecycle of a download.
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
}

/// Progress snapshot for a single download.
#[derive(Clone, Debug)]
pub struct DownloadProgress {
    pub task_id: String,
    pub state: DownloadState,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: u64,
    pub last_error: Option<String>,
}

/// A single download task.
#[derive(Clone, Debug)]
pub struct DownloadTask {
    pub id: String,
    pub url: Url,
    pub relative_path: RelativePath,
    pub expected_size: u64,
    pub expected_sha256: String,
    pub state: DownloadState,
    pub downloaded_bytes: u64,
    pub attempts: u32,
    pub last_error: Option<String>,
}

/// Errors produced by the download engine.
#[derive(Debug)]
pub enum DownloadError {
    InvalidUrl(String),
    PathValidation(gta6rp_domain::path::PathError),
    Install(gta6rp_installation::InstallError),
    Http(reqwest::Error),
    Status(reqwest::StatusCode),
    SizeMismatch { expected: u64, actual: u64 },
    ChecksumMismatch { expected: String, actual: String },
    Io(std::io::Error),
    Cancelled,
    InsufficientSpace { available: u64, required: u64 },
}

impl fmt::Display for DownloadError {
    fn fmt(self: &DownloadError, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(message) => write!(formatter, "invalid URL: {message}"),
            Self::PathValidation(error) => write!(formatter, "path validation: {error}"),
            Self::Install(error) => write!(formatter, "installation: {error}"),
            Self::Http(error) => write!(formatter, "HTTP error: {error}"),
            Self::Status(status) => write!(formatter, "unexpected HTTP status: {status}"),
            Self::SizeMismatch { expected, actual } => write!(
                formatter,
                "size mismatch: expected {expected} bytes, received {actual} bytes"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "checksum mismatch: expected {expected}, computed {actual}"
            ),
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Cancelled => write!(formatter, "download was cancelled"),
            Self::InsufficientSpace {
                available,
                required,
            } => write!(
                formatter,
                "insufficient free space: {available} bytes available, {required} bytes required"
            ),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<gta6rp_domain::path::PathError> for DownloadError {
    fn from(error: gta6rp_domain::path::PathError) -> Self {
        Self::PathValidation(error)
    }
}

impl From<gta6rp_installation::InstallError> for DownloadError {
    fn from(error: gta6rp_installation::InstallError) -> Self {
        Self::Install(error)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Result of a completed download.
#[derive(Clone, Debug)]
pub struct DownloadResult {
    pub task_id: String,
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
}

/// Configuration for a downloader.
#[derive(Clone, Debug, Default)]
pub struct DownloaderConfig {
    /// Required free space before starting, in bytes. Zero means no check.
    pub required_free_space: u64,
}

impl DownloaderConfig {
    /// Create a config with the given required free space.
    #[must_use]
    pub const fn with_required_free_space(mut self, required_free_space: u64) -> Self {
        self.required_free_space = required_free_space;
        self
    }
}

/// Downloads a single file into a managed root.
///
/// The downloader streams the response body to a temporary file inside the
/// destination directory, verifies size and SHA-256, then atomically renames
/// it into place. It never loads the whole file into RAM.
#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    config: DownloaderConfig,
}

impl Downloader {
    /// Create a downloader with a default client.
    ///
    /// # Errors
    /// Returns [`DownloadError::Http`] if the HTTP client cannot be built.
    pub fn new() -> Result<Self, DownloadError> {
        Ok(Self {
            client: build_client()?,
            config: DownloaderConfig::default(),
        })
    }

    /// Create a downloader with custom configuration.
    ///
    /// # Errors
    /// Returns [`DownloadError::Http`] if the HTTP client cannot be built.
    pub fn with_config(config: DownloaderConfig) -> Result<Self, DownloadError> {
        Ok(Self {
            client: build_client()?,
            config,
        })
    }

    /// Download `task` into `root`, placing the verified file at
    /// `root.join(task.relative_path)`.
    ///
    /// Progress is reported through `on_progress` after every chunk. Returning
    /// `false` from `on_progress` cancels the download.
    ///
    /// # Errors
    /// Returns [`DownloadError`] for validation, HTTP, size, checksum, I/O, or
    /// space failures.
    pub async fn download<F>(
        &self,
        root: &SafeRoot,
        task: &DownloadTask,
        mut on_progress: F,
    ) -> Result<DownloadResult, DownloadError>
    where
        F: FnMut(&DownloadProgress) -> bool,
    {
        let destination = root.resolve(&task.relative_path)?;

        if self.config.required_free_space > 0 {
            let info = ensure_free_space(root.root(), self.config.required_free_space)
                .map_err(DownloadError::Install)?;
            if info.available_bytes < self.config.required_free_space {
                return Err(DownloadError::InsufficientSpace {
                    available: info.available_bytes,
                    required: self.config.required_free_space,
                });
            }
        }

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(DownloadError::Io)?;
        }

        let response = self.send_request(task).await?;
        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(DownloadError::Status(status));
        }

        // When resuming, the server must honor Range with 206 Partial Content.
        // If it returns 200 instead, the server does not support Range; restart
        // the download from scratch rather than failing.
        let resume = task.downloaded_bytes > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;

        let total_bytes = response.content_length();
        let temporary = temporary_path(destination.parent().unwrap_or(root.root()));

        let downloaded = if resume {
            self.resume_download(response, &temporary, task, total_bytes, &mut on_progress)
                .await?
        } else {
            self.fresh_download(response, &temporary, task, total_bytes, &mut on_progress)
                .await?
        };

        self.verify_and_place(
            &task.id,
            &temporary,
            destination,
            downloaded,
            task.expected_size,
            &task.expected_sha256,
        )
        .await
    }

    /// Send the HTTP GET request, applying a Range header when resuming.
    async fn send_request(&self, task: &DownloadTask) -> Result<reqwest::Response, DownloadError> {
        let mut request = self.client.get(task.url.clone());
        if task.downloaded_bytes > 0 {
            request = request.header(
                reqwest::header::RANGE,
                format!("bytes={}-", task.downloaded_bytes),
            );
        }
        request.send().await.map_err(DownloadError::Http)
    }

    /// Stream a fresh download into the temporary file.
    async fn fresh_download<F>(
        &self,
        response: reqwest::Response,
        temporary: &Path,
        task: &DownloadTask,
        total_bytes: Option<u64>,
        on_progress: &mut F,
    ) -> Result<u64, DownloadError>
    where
        F: FnMut(&DownloadProgress) -> bool,
    {
        let file = tokio::fs::File::create(temporary)
            .await
            .map_err(DownloadError::Io)?;
        match self
            .stream_chunks(response, file, task, 0, total_bytes, on_progress)
            .await
        {
            Ok(downloaded) => Ok(downloaded),
            Err(DownloadError::Cancelled) => {
                let _ = tokio::fs::remove_file(temporary).await;
                Err(DownloadError::Cancelled)
            }
            Err(err) => Err(err),
        }
    }

    /// Stream a resumed download, appending to an existing partial file.
    async fn resume_download<F>(
        &self,
        response: reqwest::Response,
        temporary: &Path,
        task: &DownloadTask,
        total_bytes: Option<u64>,
        on_progress: &mut F,
    ) -> Result<u64, DownloadError>
    where
        F: FnMut(&DownloadProgress) -> bool,
    {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(temporary)
            .await
            .map_err(DownloadError::Io)?;
        match self
            .stream_chunks(
                response,
                file,
                task,
                task.downloaded_bytes,
                total_bytes,
                on_progress,
            )
            .await
        {
            Ok(downloaded) => Ok(downloaded),
            Err(DownloadError::Cancelled) => {
                let _ = tokio::fs::remove_file(temporary).await;
                Err(DownloadError::Cancelled)
            }
            Err(err) => Err(err),
        }
    }

    /// Stream response body chunks into a file while hashing and reporting
    /// progress. Returns the total number of bytes on disk after streaming.
    ///
    /// On cancellation the file is closed and the caller is responsible for
    /// removing the temporary file.
    async fn stream_chunks<F>(
        &self,
        response: reqwest::Response,
        mut file: tokio::fs::File,
        task: &DownloadTask,
        mut downloaded: u64,
        total_bytes: Option<u64>,
        on_progress: &mut F,
    ) -> Result<u64, DownloadError>
    where
        F: FnMut(&DownloadProgress) -> bool,
    {
        let mut stream = response.bytes_stream();
        let mut hasher = Sha256::new();
        let start = Instant::now();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(DownloadError::Http)?;
            file.write_all(&chunk).await.map_err(DownloadError::Io)?;
            hasher.update(&chunk);
            downloaded += u64::try_from(chunk.len()).expect("chunk fits in u64");

            let progress = DownloadProgress {
                task_id: task.id.clone(),
                state: DownloadState::Downloading,
                downloaded_bytes: downloaded,
                total_bytes: total_bytes.map(|total| total.saturating_add(task.downloaded_bytes)),
                bytes_per_second: speed(downloaded, start.elapsed()),
                last_error: None,
            };
            if !on_progress(&progress) {
                drop(file);
                return Err(DownloadError::Cancelled);
            }
        }

        file.flush().await.map_err(DownloadError::Io)?;
        Ok(downloaded)
    }

    /// Verify checksum and size, then atomically place the file at its
    /// destination.
    async fn verify_and_place(
        &self,
        task_id: &str,
        temporary: &Path,
        destination: PathBuf,
        downloaded: u64,
        expected_size: u64,
        expected_sha256: &str,
    ) -> Result<DownloadResult, DownloadError> {
        // Recompute the hash from the final file on disk so a resumed download
        // hashes the complete file, not only the newly appended bytes.
        let actual = self.file_sha256(temporary).await?;
        if actual != expected_sha256 {
            let _ = tokio::fs::remove_file(temporary).await;
            return Err(DownloadError::ChecksumMismatch {
                expected: expected_sha256.to_owned(),
                actual,
            });
        }

        if downloaded != expected_size {
            let _ = tokio::fs::remove_file(temporary).await;
            return Err(DownloadError::SizeMismatch {
                expected: expected_size,
                actual: downloaded,
            });
        }

        tokio::fs::rename(temporary, &destination)
            .await
            .map_err(DownloadError::Io)?;

        Ok(DownloadResult {
            task_id: task_id.to_owned(),
            path: destination,
            size: downloaded,
            sha256: actual,
        })
    }

    /// SHA-256 digest of a file on disk, read in bounded chunks.
    async fn file_sha256(&self, path: &Path) -> Result<String, DownloadError> {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(DownloadError::Io)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; READ_BUFFER_SIZE];
        loop {
            let n = file.read(&mut buffer).await.map_err(DownloadError::Io)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }
}

fn speed(bytes: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs().max(1);
    bytes / secs
}

fn temporary_path(directory: &Path) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    directory.join(format!("{TMP_PREFIX}{nanos}-{counter}"))
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_client() -> Result<reqwest::Client, DownloadError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(DownloadError::Http)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_file(port: u16, content: &[u8]) {
        let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let content = content.to_vec();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Consume the request headers so the response stays in sync.
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = match stream.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = buf;
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: close\r\n\
                 \r\n",
                content.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.write_all(&content).await;
            let _ = stream.shutdown().await;
        });
    }

    #[tokio::test]
    async fn downloads_file_verifies_size_and_checksum() {
        let content: Vec<u8> = (0usize..10_000)
            .map(|i| u8::try_from(i % 251).expect("fits in a byte"))
            .collect();
        let expected_hex = hex::encode(sha2::Sha256::digest(&content));

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        serve_file(port, &content).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let temp_dir =
            std::env::temp_dir().join(format!("gta6rp-download-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let root = SafeRoot::open(&temp_dir).unwrap();
        let url = Url::parse(&format!("http://127.0.0.1:{port}/file.bin")).unwrap();
        let task = DownloadTask {
            id: "task-1".to_owned(),
            url,
            relative_path: RelativePath::parse("data/file.bin").unwrap(),
            expected_size: content.len() as u64,
            expected_sha256: expected_hex.clone(),
            state: DownloadState::Queued,
            downloaded_bytes: 0,
            attempts: 0,
            last_error: None,
        };

        let downloader = Downloader::new().unwrap();
        let mut progress_count = 0u64;
        let result = downloader
            .download(&root, &task, |progress| {
                progress_count += 1;
                assert_eq!(progress.state, DownloadState::Downloading);
                true
            })
            .await
            .expect("download should succeed");

        assert_eq!(result.size, content.len() as u64);
        assert_eq!(result.sha256, expected_hex);
        assert!(progress_count > 0);

        let written = root
            .read(&RelativePath::parse("data/file.bin").unwrap())
            .unwrap();
        assert_eq!(written, content);

        // No temporary file should be left behind.
        let entries: Vec<_> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(!entries.iter().any(|e| e.starts_with(TMP_PREFIX)));
    }

    #[tokio::test]
    async fn rejects_checksum_mismatch() {
        let temp_dir =
            std::env::temp_dir().join(format!("gta6rp-download-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let root = SafeRoot::open(&temp_dir).unwrap();

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        serve_file(port, b"some content").await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let url = Url::parse(&format!("http://127.0.0.1:{port}/file.bin")).unwrap();
        let task = DownloadTask {
            id: "task-2".to_owned(),
            url,
            relative_path: RelativePath::parse("data/file.bin").unwrap(),
            expected_size: 12,
            expected_sha256: "0".repeat(64),
            state: DownloadState::Queued,
            downloaded_bytes: 0,
            attempts: 0,
            last_error: None,
        };

        let downloader = Downloader::new().unwrap();
        let err = downloader
            .download(&root, &task, |_| true)
            .await
            .expect_err("checksum should mismatch");
        assert!(matches!(err, DownloadError::ChecksumMismatch { .. }));
    }

    #[tokio::test]
    async fn resumes_from_scratch_when_server_ignores_range() {
        let content: Vec<u8> = (0usize..5_000)
            .map(|i| u8::try_from(i % 251).expect("fits in a byte"))
            .collect();
        let expected_hex = hex::encode(sha2::Sha256::digest(&content));

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        // The test server always answers 200, ignoring Range headers, to
        // simulate a server without resume support.
        serve_file(port, &content).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let temp_dir =
            std::env::temp_dir().join(format!("gta6rp-download-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let root = SafeRoot::open(&temp_dir).unwrap();

        let url = Url::parse(&format!("http://127.0.0.1:{port}/file.bin")).unwrap();
        // Pretend we already downloaded half the file; the server will ignore
        // Range and return the full body, so the downloader must restart.
        let task = DownloadTask {
            id: "task-3".to_owned(),
            url,
            relative_path: RelativePath::parse("data/file.bin").unwrap(),
            expected_size: content.len() as u64,
            expected_sha256: expected_hex.clone(),
            state: DownloadState::Downloading,
            downloaded_bytes: content.len() as u64 / 2,
            attempts: 0,
            last_error: None,
        };

        let downloader = Downloader::new().unwrap();
        let result = downloader
            .download(&root, &task, |_| true)
            .await
            .expect("download should succeed after Range restart");

        assert_eq!(result.size, content.len() as u64);
        assert_eq!(result.sha256, expected_hex);

        let written = root
            .read(&RelativePath::parse("data/file.bin").unwrap())
            .unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn state_as_str_round_trip() {
        assert_eq!(DownloadState::Queued.as_str(), "queued");
        assert_eq!(DownloadState::Downloading.as_str(), "downloading");
        assert_eq!(DownloadState::Completed.as_str(), "completed");
    }
}
