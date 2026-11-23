//! Shared request and response types for the HTTP API.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PAGE_SIZE: u32 = 25;
pub const MAX_PAGE_SIZE: u32 = 100;

/// Offset pagination parameters with explicit bounds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct Pagination {
    pub limit: u32,
    pub offset: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: DEFAULT_PAGE_SIZE,
            offset: 0,
        }
    }
}

impl Pagination {
    /// Validate requested pagination parameters and apply the documented defaults.
    ///
    /// # Errors
    /// Returns an error when `limit` is zero or exceeds [`MAX_PAGE_SIZE`].
    pub fn validate(self) -> Result<Self, PaginationError> {
        if !(1..=MAX_PAGE_SIZE).contains(&self.limit) {
            return Err(PaginationError);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaginationError;

impl std::fmt::Display for PaginationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "limit must be between 1 and {MAX_PAGE_SIZE}")
    }
}

impl std::error::Error for PaginationError {}

/// A page of results with enough metadata to request the following page.
#[derive(Clone, Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub limit: u32,
    pub offset: u64,
    pub has_more: bool,
}

impl<T> Page<T> {
    #[must_use]
    pub fn new(items: Vec<T>, pagination: Pagination, has_more: bool) -> Self {
        Self {
            items,
            limit: pagination.limit,
            offset: pagination.offset,
            has_more,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ErrorEnvelope {
    pub error: ErrorDetails,
}

#[derive(Serialize)]
pub(crate) struct ErrorDetails {
    pub code: &'static str,
    pub message: &'static str,
    pub request_id: Option<String>,
}
