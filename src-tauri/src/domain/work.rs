//! Work entity — the core domain object representing a game.
//!
//! Replaces v0.4.0's `Dict[str, Any]` with strongly-typed fields.
//! Every field has a clear type, optionality, and documentation.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::ids::WorkId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSource {
    Filesystem,
    Vndb,
    Bangumi,
    Dlsite,
    UserOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LibraryStatus {
    #[default]
    Unplayed,
    Playing,
    Completed,
    OnHold,
    Dropped,
    Wishlist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentState {
    #[default]
    Unmatched,
    PendingReview,
    Matched,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: WorkId,
    pub folder_path: PathBuf,
    pub title: String,
    pub title_original: Option<String>,
    pub title_aliases: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<NaiveDate>,
    pub rating: Option<f64>,
    pub vote_count: Option<u32>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub tags: Vec<String>,
    pub user_tags: Vec<String>,
    pub field_sources: HashMap<String, String>,
    pub field_preferences: HashMap<String, String>,
    pub user_overrides: HashMap<String, serde_json::Value>,
    pub library_status: LibraryStatus,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub enrichment_state: EnrichmentState,
    pub title_source: FieldSource,
    pub folder_mtime: f64,
    pub metadata_mtime: f64,
    pub metadata_hash: Option<String>,
    pub content_signature: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Work {
    pub fn from_discovery(folder_path: PathBuf, title: String, folder_mtime: f64) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: WorkId::new(),
            folder_path,
            title,
            title_original: None,
            title_aliases: Vec::new(),
            developer: None,
            publisher: None,
            release_date: None,
            rating: None,
            vote_count: None,
            description: None,
            cover_path: None,
            tags: Vec::new(),
            user_tags: Vec::new(),
            field_sources: HashMap::new(),
            field_preferences: HashMap::new(),
            user_overrides: HashMap::new(),
            library_status: LibraryStatus::default(),
            vndb_id: None,
            bangumi_id: None,
            dlsite_id: None,
            enrichment_state: EnrichmentState::default(),
            title_source: FieldSource::Filesystem,
            folder_mtime,
            metadata_mtime: 0.0,
            metadata_hash: None,
            content_signature: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSummary {
    pub id: WorkId,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub library_status: LibraryStatus,
    pub enrichment_state: EnrichmentState,
    pub tags: Vec<String>,
    pub release_date: Option<NaiveDate>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub variant_count: u32,
    pub asset_count: u32,
    pub asset_types: Vec<String>,
    pub primary_asset_type: Option<String>,
}
