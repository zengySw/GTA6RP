//! Validation for relative paths that originate from manifests.
//!
//! Every path that the launcher writes into a controlled installation
//! directory must first be validated here. The rules reject absolute paths,
//! traversal, Windows-specific path forms, and names that Windows silently
//! rewrites, so a manifest cannot steer a write outside the managed root.

use core::fmt;
use std::path::{Path, PathBuf};

/// Maximum accepted length of a manifest path, in bytes.
pub const MAX_PATH_LENGTH: usize = 4096;
/// Maximum accepted length of a single path component, in bytes.
pub const MAX_COMPONENT_LENGTH: usize = 255;

/// A path that is guaranteed to be relative, normalised, and safe to join to a
/// trusted root directory.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RelativePath(String);

impl RelativePath {
    /// Validate and wrap a manifest path.
    ///
    /// # Errors
    /// Returns a [`PathError`] describing the first rule the input violates.
    pub fn parse(input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        if input.len() > MAX_PATH_LENGTH {
            return Err(PathError::TooLong);
        }
        if input.starts_with('/') {
            return Err(PathError::Absolute);
        }
        for character in input.chars() {
            if character == '\\' {
                return Err(PathError::Backslash);
            }
            if character == ':' {
                return Err(PathError::Colon);
            }
            if character.is_control() {
                return Err(PathError::ControlCharacter);
            }
        }

        for component in input.split('/') {
            validate_component(component)?;
        }

        Ok(Self(input.to_owned()))
    }

    /// The validated path with forward-slash separators.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The individual path components, in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for RelativePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn validate_component(component: &str) -> Result<(), PathError> {
    if component.is_empty() {
        return Err(PathError::EmptyComponent);
    }
    if component == "." {
        return Err(PathError::CurrentDirectory);
    }
    if component == ".." {
        return Err(PathError::ParentTraversal);
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace);
    }
    if component.len() > MAX_COMPONENT_LENGTH {
        return Err(PathError::ComponentTooLong);
    }
    if is_reserved_device_name(component) {
        return Err(PathError::ReservedDeviceName);
    }
    Ok(())
}

fn is_reserved_device_name(component: &str) -> bool {
    const EXACT: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    // Windows treats these as devices regardless of extension, so compare the
    // base name before the first dot, case-insensitively.
    let base = component.split('.').next().unwrap_or(component);
    let base = base.to_ascii_uppercase();
    if EXACT.contains(&base.as_str()) {
        return true;
    }
    let Some(number) = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"))
    else {
        return false;
    };
    matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
}

/// Join a validated relative path to a trusted root, defensively confirming the
/// result stays under that root.
///
/// # Errors
/// Returns [`JoinError::EscapesRoot`] if the joined path is not lexically under
/// `root`. This should be unreachable for a [`RelativePath`] and exists as
/// defence in depth.
pub fn join_under(root: &Path, relative: &RelativePath) -> Result<PathBuf, JoinError> {
    let candidate = root.join(relative.as_str());
    if candidate.starts_with(root) {
        Ok(candidate)
    } else {
        Err(JoinError::EscapesRoot(candidate))
    }
}

/// Reasons a validated path could not be joined under a root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JoinError {
    EscapesRoot(PathBuf),
}

impl fmt::Display for JoinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EscapesRoot(path) => {
                write!(
                    formatter,
                    "path {} escapes the managed root",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for JoinError {}

/// Reasons a manifest path was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathError {
    Empty,
    TooLong,
    Absolute,
    Backslash,
    Colon,
    ControlCharacter,
    EmptyComponent,
    CurrentDirectory,
    ParentTraversal,
    TrailingDotOrSpace,
    ComponentTooLong,
    ReservedDeviceName,
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "path must not be empty",
            Self::TooLong => "path is too long",
            Self::Absolute => "path must be relative",
            Self::Backslash => "path must use '/' separators",
            Self::Colon => "path must not contain ':'",
            Self::ControlCharacter => "path must not contain control characters",
            Self::EmptyComponent => "path must not contain empty components",
            Self::CurrentDirectory => "path must not contain '.' components",
            Self::ParentTraversal => "path must not contain '..' components",
            Self::TrailingDotOrSpace => "path components must not end with '.' or a space",
            Self::ComponentTooLong => "path component is too long",
            Self::ReservedDeviceName => "path contains a reserved Windows device name",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PathError {}

#[cfg(test)]
mod tests {
    use super::{PathError, RelativePath, join_under};
    use std::path::Path;

    #[test]
    fn accepts_ordinary_relative_paths() {
        for path in [
            "data/example.bin",
            "content/textures/ground.dds",
            "a",
            "nested/deeper/file.pak",
            "unicode/пример.txt",
        ] {
            let parsed = RelativePath::parse(path).expect("path should be valid");
            assert_eq!(parsed.as_str(), path);
        }
    }

    #[test]
    fn rejects_absolute_and_unc_like_paths() {
        assert_eq!(RelativePath::parse("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(
            RelativePath::parse("\\server\\share"),
            Err(PathError::Backslash)
        );
        assert_eq!(
            RelativePath::parse("//server/share"),
            Err(PathError::Absolute)
        );
        assert_eq!(RelativePath::parse("C:/Windows"), Err(PathError::Colon));
    }

    #[test]
    fn rejects_traversal_and_current_directory() {
        assert_eq!(RelativePath::parse(".."), Err(PathError::ParentTraversal));
        assert_eq!(
            RelativePath::parse("data/../../secret"),
            Err(PathError::ParentTraversal)
        );
        assert_eq!(RelativePath::parse("."), Err(PathError::CurrentDirectory));
        assert_eq!(
            RelativePath::parse("data/./file"),
            Err(PathError::CurrentDirectory)
        );
    }

    #[test]
    fn rejects_empty_and_control_input() {
        assert_eq!(RelativePath::parse(""), Err(PathError::Empty));
        assert_eq!(RelativePath::parse("a//b"), Err(PathError::EmptyComponent));
        assert_eq!(RelativePath::parse("a/"), Err(PathError::EmptyComponent));
        assert_eq!(
            RelativePath::parse("a\tb"),
            Err(PathError::ControlCharacter)
        );
        assert_eq!(
            RelativePath::parse("a\0b"),
            Err(PathError::ControlCharacter)
        );
    }

    #[test]
    fn rejects_windows_rewritten_names() {
        assert_eq!(
            RelativePath::parse("data/file."),
            Err(PathError::TrailingDotOrSpace)
        );
        assert_eq!(
            RelativePath::parse("data/file "),
            Err(PathError::TrailingDotOrSpace)
        );
        assert_eq!(
            RelativePath::parse("data/NUL"),
            Err(PathError::ReservedDeviceName)
        );
        assert_eq!(
            RelativePath::parse("data/con.txt"),
            Err(PathError::ReservedDeviceName)
        );
        assert_eq!(
            RelativePath::parse("data/Com9.bin"),
            Err(PathError::ReservedDeviceName)
        );
        assert_eq!(
            RelativePath::parse("data/LPT1"),
            Err(PathError::ReservedDeviceName)
        );
    }

    #[test]
    fn reserved_names_do_not_catch_similar_names() {
        for path in [
            "data/CONSOLE.txt",
            "data/COM10.bin",
            "data/NULL",
            "data/auxiliary",
        ] {
            assert!(RelativePath::parse(path).is_ok(), "{path} should be valid");
        }
    }

    #[test]
    fn enforces_length_limits() {
        let long_component = "a".repeat(super::MAX_COMPONENT_LENGTH + 1);
        assert_eq!(
            RelativePath::parse(&long_component),
            Err(PathError::ComponentTooLong)
        );

        let long_path = "a/".repeat(super::MAX_PATH_LENGTH / 2 + 1);
        assert_eq!(RelativePath::parse(&long_path), Err(PathError::TooLong));
    }

    #[test]
    fn components_round_trip() {
        let parsed = RelativePath::parse("a/b/c.bin").expect("valid");
        let parts: Vec<_> = parsed.components().collect();
        assert_eq!(parts, ["a", "b", "c.bin"]);
    }

    #[test]
    fn join_stays_under_root() {
        let root = Path::new("/managed/root");
        let parsed = RelativePath::parse("data/example.bin").expect("valid");
        let joined = join_under(root, &parsed).expect("join");
        assert_eq!(joined, Path::new("/managed/root/data/example.bin"));
    }

    #[test]
    fn join_always_produces_a_path_under_root() {
        let root = Path::new("/managed/root");
        for raw in ["a", "data/example.bin", "deep/nested/file.pak"] {
            let parsed = RelativePath::parse(raw).expect("valid");
            let joined = join_under(root, &parsed).expect("join");
            assert!(joined.starts_with(root), "{raw} escaped {root:?}");
        }
    }
}
