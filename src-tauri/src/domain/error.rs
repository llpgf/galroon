//! Unified error type for the entire application.
//!
//! Every error is an enum variant — no stringly-typed errors,
//! no `Box<dyn Error>` escaping into business logic.

use thiserror::Error;

/// Top-level application error.
#[derive(Debug, Error)]
pub enum AppError {
    // --- Database ---
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Database writer channel closed")]
    DbWriterClosed,

    // --- Filesystem ---
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path is outside allowed scope: {0}")]
    PathOutOfScope(String),

    #[error("Path contains invalid characters: {0}")]
    InvalidPath(String),

    // --- Config ---
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    // --- Serialization ---
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // --- Scanner ---
    #[error("Scan already in progress")]
    ScanAlreadyRunning,

    #[error("Scanner error: {0}")]
    Scanner(String),

    // --- Enrichment ---
    #[error("VNDB API error: {0}")]
    VndbApi(String),

    #[error("Bangumi API error: {0}")]
    BangumiApi(String),

    #[error("Matching failed: {0}")]
    MatchingFailed(String),

    #[error("Rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    // --- Domain ---
    #[error("Work not found: {0}")]
    WorkNotFound(String),

    #[error("Invalid work ID: {0}")]
    InvalidWorkId(String),

    // --- Generic ---
    #[error("{0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Network error: {0}")]
    Network(String),
}

/// Convenience Result type for the application.
pub type AppResult<T> = Result<T, AppError>;

// Make AppError serializable for Tauri IPC error responses.
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
