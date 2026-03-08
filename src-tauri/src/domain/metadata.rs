//! Metadata models — the portable format that travels with game folders.
//!
//! metadata.json is the AUTHORITY (R2). DB is a read model.
//! This module defines the on-disk JSON schema.
//!
//! R19: Contains a stable `work_id` (UUID) that survives folder rename/move.
//! R20: Contains `write_nonce` so the watcher can suppress self-triggered events.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// On-disk metadata.json schema.
///
/// This is what gets written to each game folder.
/// It must be backward-compatible and self-describing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataJson {
    /// Schema version for future migration support (R16)
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// R19: Stable Work identity — generated on first ingest, never changes.
    /// Survives folder rename/move. The scanner uses this to detect moves
    /// instead of treating renamed folders as new works.
    pub work_id: Option<String>,

    /// R20: Nonce written by the app on each save. The watcher checks this
    /// to suppress self-triggered events (app-write → watcher → re-scan loop).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_nonce: Option<String>,

    /// R20: Identifies the last writer. "galroon" = app wrote this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_written_by: Option<String>,

    /// Display title
    pub title: Option<String>,

    /// Original title (Japanese/Chinese)
    pub title_original: Option<String>,

    /// Alternative titles for search
    #[serde(default)]
    pub title_aliases: Vec<String>,

    /// Developer / brand
    pub developer: Option<String>,

    /// Publisher
    pub publisher: Option<String>,

    /// Release date (YYYY-MM-DD)
    pub release_date: Option<NaiveDate>,

    /// Description / synopsis
    pub description: Option<String>,

    /// Cover image filename (relative to game folder)
    pub cover: Option<String>,

    /// Remote cover URL when the poster is sourced from metadata providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,

    /// Stable content signature derived from top-level assets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_signature: Option<String>,

    /// Tags from external sources
    #[serde(default)]
    pub tags: Vec<String>,

    /// User-defined tags
    #[serde(default)]
    pub user_tags: Vec<String>,

    /// Library status
    pub library_status: Option<String>,

    /// External IDs
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,

    /// VNDB rating
    pub rating: Option<f64>,

    /// Vote count
    pub vote_count: Option<u32>,

    /// User overrides: field_name → value
    /// When set, these take priority over all sources.
    #[serde(default)]
    pub user_overrides: HashMap<String, serde_json::Value>,

    /// Enrichment state tracking
    pub enrichment_state: Option<String>,

    /// Source tracking: which source provided which value
    #[serde(default)]
    pub field_sources: HashMap<String, String>,

    /// Source preference overrides: field_name -> preferred provider
    #[serde(default)]
    pub field_preferences: HashMap<String, String>,
}

fn default_schema_version() -> u32 {
    1
}

impl Default for MetadataJson {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            work_id: None,
            write_nonce: None,
            last_written_by: None,
            title: None,
            title_original: None,
            title_aliases: Vec::new(),
            developer: None,
            publisher: None,
            release_date: None,
            description: None,
            cover: None,
            cover_url: None,
            content_signature: None,
            tags: Vec::new(),
            user_tags: Vec::new(),
            library_status: None,
            vndb_id: None,
            bangumi_id: None,
            dlsite_id: None,
            rating: None,
            vote_count: None,
            user_overrides: HashMap::new(),
            enrichment_state: None,
            field_sources: HashMap::new(),
            field_preferences: HashMap::new(),
        }
    }
}
