//! Typed errors for local state operations.

use core::fmt;

/// A failure while reading or writing local state.
#[derive(Debug)]
pub enum StoreError {
    /// The database returned an error.
    Database(sqlx::Error),
    /// Embedded migrations could not be applied.
    Migration(sqlx::migrate::MigrateError),
    /// The database file or its directory could not be prepared.
    Io(std::io::Error),
    /// The caller supplied values that violate the store contract.
    InvalidInput(String),
    /// Stored data does not match the expected shape.
    Corrupt(String),
}

impl StoreError {
    /// Build a corruption error with an explanatory message.
    #[must_use]
    pub fn corrupt(message: impl Into<String>) -> Self {
        Self::Corrupt(message.into())
    }

    /// Build an invalid-input error with an explanatory message.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "local database error: {error}"),
            Self::Migration(error) => write!(formatter, "local migration error: {error}"),
            Self::Io(error) => write!(formatter, "local storage I/O error: {error}"),
            Self::InvalidInput(message) => {
                write!(formatter, "invalid local state input: {message}")
            }
            Self::Corrupt(message) => write!(formatter, "corrupt local state: {message}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Migration(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InvalidInput(_) | Self::Corrupt(_) => None,
        }
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<gta6rp_domain::settings::SettingsError> for StoreError {
    fn from(error: gta6rp_domain::settings::SettingsError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

impl From<sqlx::migrate::MigrateError> for StoreError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Migration(error)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
