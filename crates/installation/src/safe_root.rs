//! Filesystem operations confined to a canonicalised managed root.

use core::fmt;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gta6rp_domain::path::{RelativePath, join_under};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A directory that the launcher owns. Every path resolved through it stays
/// inside the directory unless a filesystem link would escape it.
#[derive(Clone, Debug)]
pub struct SafeRoot {
    root: PathBuf,
}

impl SafeRoot {
    /// Canonicalise and verify a user-selected installation directory.
    ///
    /// # Errors
    /// Returns [`InstallError::RootNotFound`] when the path does not exist,
    /// [`InstallError::RootNotDirectory`] when it is not a directory, and
    /// [`InstallError::PermissionDenied`] or [`InstallError::Io`] for other
    /// filesystem failures.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, InstallError> {
        let requested = path.as_ref();
        let root = match fs::canonicalize(requested) {
            Ok(root) => root,
            Err(error) => {
                return Err(match error {
                    error if error.kind() == io::ErrorKind::NotFound => {
                        InstallError::RootNotFound(requested.to_path_buf())
                    }
                    error => io_error(requested, error),
                });
            }
        };
        if !root.is_dir() {
            return Err(InstallError::RootNotDirectory(requested.to_path_buf()));
        }
        Ok(Self { root })
    }

    /// The canonical root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a validated relative path to an absolute path inside the root.
    ///
    /// # Errors
    /// Returns [`InstallError::EscapesRoot`] if the resolved path leaves the
    /// root, [`InstallError::SymlinkDetected`] if any existing component is a
    /// symbolic link, or an I/O error when a component cannot be inspected.
    pub fn resolve(&self, relative: &RelativePath) -> Result<PathBuf, InstallError> {
        let target = join_under(&self.root, relative)
            .map_err(|_| InstallError::EscapesRoot(self.root.join(relative.as_str())))?;
        self.reject_symlink_components(&target)?;
        Ok(target)
    }

    /// Create a directory (and missing parents) inside the root.
    ///
    /// # Errors
    /// Propagates resolution failures and directory-creation I/O errors.
    pub fn create_dir_all(&self, relative: &RelativePath) -> Result<PathBuf, InstallError> {
        let target = self.resolve(relative)?;
        fs::create_dir_all(&target).map_err(|error| io_error(&target, error))?;
        Ok(target)
    }

    /// Read a file inside the root.
    ///
    /// # Errors
    /// Returns [`InstallError::NotFound`] when the file is missing, and
    /// propagates resolution and read errors otherwise.
    pub fn read(&self, relative: &RelativePath) -> Result<Vec<u8>, InstallError> {
        let target = self.resolve(relative)?;
        fs::read(&target).map_err(|error| io_error(&target, error))
    }

    /// Whether a regular file already exists at the resolved path.
    ///
    /// # Errors
    /// Propagates resolution failures.
    pub fn exists(&self, relative: &RelativePath) -> Result<bool, InstallError> {
        let target = self.resolve(relative)?;
        Ok(target.is_file())
    }

    /// Write a complete file atomically.
    ///
    /// The bytes are written to a temporary file in the destination directory,
    /// flushed to disk, then renamed over the final name. The temporary file is
    /// removed if any step fails.
    ///
    /// # Errors
    /// Returns [`InstallError::SymlinkDetected`] for a linked target or
    /// ancestor, and propagates resolution and I/O errors.
    pub fn write_atomic(
        &self,
        relative: &RelativePath,
        data: &[u8],
    ) -> Result<PathBuf, InstallError> {
        let target = self.resolve(relative)?;
        let Some(parent) = target.parent() else {
            return Err(InstallError::EscapesRoot(target));
        };
        fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;

        let temporary = temporary_path(parent);
        let guard = TempGuard::new(temporary.clone());

        let mut file = File::create(&temporary).map_err(|error| io_error(&temporary, error))?;
        file.write_all(data)
            .map_err(|error| io_error(&temporary, error))?;
        file.sync_all()
            .map_err(|error| io_error(&temporary, error))?;
        drop(file);

        replace_file(&temporary, &target)?;
        guard.disarm();
        Ok(target)
    }

    fn reject_symlink_components(&self, target: &Path) -> Result<(), InstallError> {
        let relative = target
            .strip_prefix(&self.root)
            .map_err(|_| InstallError::EscapesRoot(target.to_path_buf()))?;

        let mut cursor = self.root.clone();
        for component in relative.components() {
            cursor.push(component);
            match fs::symlink_metadata(&cursor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(InstallError::SymlinkDetected(cursor));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) => return Err(io_error(&cursor, error)),
            }
        }
        Ok(())
    }
}

fn temporary_path(parent: &Path) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    parent.join(format!(".gta6rp-tmp-{}-{nanos}-{counter}", process::id()))
}

fn replace_file(source: &Path, target: &Path) -> Result<(), InstallError> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        // On Windows a rename cannot replace an existing file. Fall back to an
        // explicit removal, which is not atomic but still ends with a complete
        // file under the final name.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
            ) && target.is_file() =>
        {
            fs::remove_file(target).map_err(|error| io_error(target, error))?;
            fs::rename(source, target).map_err(|error| io_error(target, error))
        }
        Err(error) => Err(io_error(target, error)),
    }
}

/// Removes the temporary file on drop unless disarmed after a successful
/// rename.
struct TempGuard {
    path: Option<PathBuf>,
}

impl TempGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn io_error(path: impl AsRef<Path>, error: io::Error) -> InstallError {
    let path = path.as_ref().to_path_buf();
    match error.kind() {
        io::ErrorKind::NotFound => InstallError::NotFound(path),
        io::ErrorKind::PermissionDenied => InstallError::PermissionDenied(path),
        _ => InstallError::Io {
            path,
            source: error,
        },
    }
}

/// Information about available disk space on a filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub struct FreeSpaceInfo {
    /// Total space on the filesystem, in bytes.
    pub total_bytes: u64,
    /// Space available to the current user, in bytes.
    pub available_bytes: u64,
}

#[cfg(unix)]
#[allow(unsafe_code)]
#[allow(dead_code)]
fn query_free_space(path: &Path) -> Result<(u64, u64), io::Error> {
    use libc::statvfs;

    let mut vfs: statvfs = unsafe { std::mem::zeroed() };
    let c_path = std::ffi::CString::new(path.to_string_lossy().as_ref())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid path"))?;
    let result = unsafe { statvfs(c_path.as_ptr(), &raw mut vfs) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // f_bsize * f_bavail = available bytes for unprivileged user
    // f_bsize * f_blocks = total bytes
    let block_size = vfs.f_bsize as u64;
    let total_bytes = vfs
        .f_blocks
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("total bytes overflow"))?;
    let available_bytes = vfs
        .f_bavail
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("available bytes overflow"))?;
    Ok((total_bytes, available_bytes))
}

#[cfg(windows)]
#[allow(unsafe_code)]
#[allow(dead_code)]
fn query_free_space(path: &Path) -> Result<(u64, u64), io::Error> {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide_path: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes_available: u64 = 0;
    let mut total_number_of_bytes: u64 = 0;
    let mut _total_number_of_free_bytes: u64 = 0;

    let result: BOOL = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut free_bytes_available,
            &mut total_number_of_bytes,
            &mut _total_number_of_free_bytes,
        )
    };

    if result.as_bool() {
        Ok((total_number_of_bytes, free_bytes_available))
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
fn query_free_space(_path: &Path) -> Result<(u64, u64), io::Error> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "platform not supported for free space query",
    ))
}

/// Query available free space on the filesystem containing `path`.
///
/// # Errors
/// Returns [`InstallError::Io`] when the filesystem space cannot be queried.
#[allow(dead_code)]
pub fn free_space_info(path: &Path) -> Result<FreeSpaceInfo, InstallError> {
    let (total_bytes, available_bytes) =
        query_free_space(path).map_err(|error| io_error(path, error))?;
    Ok(FreeSpaceInfo {
        total_bytes,
        available_bytes,
    })
}

/// Check whether there is enough free space for `required_bytes`.
///
/// # Errors
/// Returns [`InstallError::Io`] when the filesystem space cannot be queried,
/// or [`InstallError::InsufficientSpace`] when there is not enough free space.
#[allow(dead_code)]
pub fn ensure_free_space(path: &Path, required_bytes: u64) -> Result<FreeSpaceInfo, InstallError> {
    let info = free_space_info(path)?;
    if info.available_bytes < required_bytes {
        return Err(InstallError::InsufficientSpace {
            available: info.available_bytes,
            required: required_bytes,
        });
    }
    Ok(info)
}

/// A failure while resolving or writing inside a [`SafeRoot`].
#[derive(Debug)]
pub enum InstallError {
    RootNotFound(PathBuf),
    RootNotDirectory(PathBuf),
    EscapesRoot(PathBuf),
    SymlinkDetected(PathBuf),
    NotFound(PathBuf),
    PermissionDenied(PathBuf),
    Io { path: PathBuf, source: io::Error },
    InsufficientSpace { available: u64, required: u64 },
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotFound(path) => {
                write!(
                    formatter,
                    "installation root {} does not exist",
                    path.display()
                )
            }
            Self::RootNotDirectory(path) => {
                write!(
                    formatter,
                    "installation root {} is not a directory",
                    path.display()
                )
            }
            Self::EscapesRoot(path) => {
                write!(
                    formatter,
                    "path {} escapes the installation root",
                    path.display()
                )
            }
            Self::SymlinkDetected(path) => {
                write!(formatter, "path {} reaches a symbolic link", path.display())
            }
            Self::NotFound(path) => write!(formatter, "file {} was not found", path.display()),
            Self::PermissionDenied(path) => {
                write!(formatter, "permission denied for {}", path.display())
            }
            Self::Io { path, source } => {
                write!(formatter, "I/O error for {}: {source}", path.display())
            }
            Self::InsufficientSpace {
                available,
                required,
            } => {
                write!(
                    formatter,
                    "insufficient free space: {available} bytes available, {required} bytes required"
                )
            }
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallError, SafeRoot, ensure_free_space, free_space_info};
    use gta6rp_domain::path::RelativePath;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(tag: &str) -> Self {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "gta6rp-test-{tag}-{}-{counter}",
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

    fn relative(value: &str) -> RelativePath {
        RelativePath::parse(value).expect("valid relative path")
    }

    #[test]
    fn open_accepts_existing_directory() {
        let directory = TestDir::new("open");
        let root = SafeRoot::open(&directory.path).expect("open root");
        assert_eq!(
            root.root(),
            fs::canonicalize(&directory.path).expect("canonical")
        );
    }

    #[test]
    fn open_rejects_missing_directory() {
        let directory = TestDir::new("missing");
        let missing = directory.path.join("absent");
        assert!(matches!(
            SafeRoot::open(&missing),
            Err(InstallError::RootNotFound(_))
        ));
    }

    #[test]
    fn open_rejects_file() {
        let directory = TestDir::new("file-root");
        let file = directory.path.join("file.bin");
        fs::write(&file, b"data").expect("write file");
        assert!(matches!(
            SafeRoot::open(&file),
            Err(InstallError::RootNotDirectory(_))
        ));
    }

    #[test]
    fn writes_creates_directories_and_content() {
        let directory = TestDir::new("write");
        let root = SafeRoot::open(&directory.path).expect("open root");
        let path = relative("data/nested/example.bin");

        let written = root.write_atomic(&path, b"hello").expect("write");
        assert!(written.starts_with(root.root()));
        assert_eq!(root.read(&path).expect("read"), b"hello");
        assert!(root.exists(&path).expect("exists"));
    }

    #[test]
    fn overwrites_existing_file_completely() {
        let directory = TestDir::new("overwrite");
        let root = SafeRoot::open(&directory.path).expect("open root");
        let path = relative("data/file.bin");

        root.write_atomic(&path, b"first version")
            .expect("first write");
        root.write_atomic(&path, b"second").expect("second write");
        assert_eq!(root.read(&path).expect("read"), b"second");
    }

    #[test]
    fn write_leaves_no_temporary_files() {
        let directory = TestDir::new("temp-cleanup");
        let root = SafeRoot::open(&directory.path).expect("open root");
        root.write_atomic(&relative("data/file.bin"), b"payload")
            .expect("write");

        let data_dir = directory.path.join("data");
        let entries: Vec<String> = fs::read_dir(&data_dir)
            .expect("read data dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(entries, vec!["file.bin".to_string()]);
    }

    #[test]
    fn read_missing_file_reports_not_found() {
        let directory = TestDir::new("read-missing");
        let root = SafeRoot::open(&directory.path).expect("open root");
        assert!(matches!(
            root.read(&relative("data/absent.bin")),
            Err(InstallError::NotFound(_))
        ));
        assert!(!root.exists(&relative("data/absent.bin")).expect("exists"));
    }

    #[test]
    fn create_dir_all_creates_nested_directories() {
        let directory = TestDir::new("mkdir");
        let root = SafeRoot::open(&directory.path).expect("open root");
        let created = root
            .create_dir_all(&relative("content/textures"))
            .expect("create dirs");
        assert!(created.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new("symlink");
        let outside = TestDir::new("symlink-outside");
        fs::write(outside.path.join("secret.bin"), b"secret").expect("write secret");
        symlink(&outside.path, directory.path.join("link")).expect("create symlink");

        let root = SafeRoot::open(&directory.path).expect("open root");
        assert!(matches!(
            root.resolve(&relative("link/secret.bin")),
            Err(InstallError::SymlinkDetected(_))
        ));
        assert!(matches!(
            root.write_atomic(&relative("link/secret.bin"), b"overwritten"),
            Err(InstallError::SymlinkDetected(_))
        ));
        assert_eq!(
            fs::read(outside.path.join("secret.bin")).expect("read secret"),
            b"secret"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_target() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new("symlink-target");
        let outside = TestDir::new("symlink-target-outside");
        fs::write(outside.path.join("file.bin"), b"outside").expect("write outside");
        symlink(
            outside.path.join("file.bin"),
            directory.path.join("file.bin"),
        )
        .expect("create symlink");

        let root = SafeRoot::open(&directory.path).expect("open root");
        assert!(matches!(
            root.write_atomic(&relative("file.bin"), b"overwritten"),
            Err(InstallError::SymlinkDetected(_))
        ));
        assert_eq!(
            fs::read(outside.path.join("file.bin")).expect("read outside"),
            b"outside"
        );
    }

    #[test]
    fn free_space_info_returns_available_and_total() {
        let directory = TestDir::new("freespace");
        let info = free_space_info(&directory.path).expect("free space info");
        assert!(info.total_bytes > 0);
        assert!(info.available_bytes > 0);
        assert!(info.available_bytes <= info.total_bytes);
    }

    #[test]
    fn ensure_free_space_succeeds_when_enough_space() {
        let directory = TestDir::new("freespace-enough");
        let info = ensure_free_space(&directory.path, 1).expect("enough space");
        assert!(info.available_bytes >= 1);
    }

    #[test]
    fn ensure_free_space_fails_when_not_enough_space() {
        let directory = TestDir::new("freespace-not-enough");
        let result = ensure_free_space(&directory.path, u64::MAX);
        assert!(matches!(
            result,
            Err(InstallError::InsufficientSpace { .. })
        ));
    }
}
