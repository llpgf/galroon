//! Database row models — FromRow structs for SQLx.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::domain::ids::WorkId;
use crate::domain::work::{FieldSource, Work, WorkSummary};

#[derive(Debug, Clone, FromRow)]
pub struct WorkRow {
    pub id: String,
    pub folder_path: String,
    pub title: String,
    pub title_original: Option<String>,
    pub title_aliases: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    pub rating: Option<f64>,
    pub vote_count: Option<i64>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub tags: Option<String>,
    pub user_tags: Option<String>,
    pub field_sources: Option<String>,
    pub field_preferences: Option<String>,
    pub user_overrides: Option<String>,
    pub library_status: String,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub enrichment_state: String,
    pub title_source: String,
    pub folder_mtime: f64,
    pub metadata_mtime: f64,
    pub metadata_hash: Option<String>,
    pub content_signature: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkRow {
    pub fn into_work(self) -> Work {
        let parse_json_vec = |s: Option<String>| -> Vec<String> {
            s.and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default()
        };

        Work {
            id: WorkId::parse(&self.id).unwrap_or_default(),
            folder_path: self.folder_path.into(),
            title: self.title,
            title_original: self.title_original,
            title_aliases: parse_json_vec(self.title_aliases),
            developer: self.developer,
            publisher: self.publisher,
            release_date: self
                .release_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            rating: self.rating,
            vote_count: self.vote_count.map(|v| v as u32),
            description: self.description,
            cover_path: self.cover_path,
            tags: parse_json_vec(self.tags),
            user_tags: parse_json_vec(self.user_tags),
            field_sources: self
                .field_sources
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default(),
            field_preferences: self
                .field_preferences
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default(),
            user_overrides: self
                .user_overrides
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default(),
            library_status: serde_json::from_str(&format!("\"{}\"", self.library_status))
                .unwrap_or_default(),
            vndb_id: self.vndb_id,
            bangumi_id: self.bangumi_id,
            dlsite_id: self.dlsite_id,
            enrichment_state: serde_json::from_str(&format!("\"{}\"", self.enrichment_state))
                .unwrap_or_default(),
            title_source: serde_json::from_str(&format!("\"{}\"", self.title_source))
                .unwrap_or(FieldSource::Filesystem),
            folder_mtime: self.folder_mtime,
            metadata_mtime: self.metadata_mtime,
            metadata_hash: self.metadata_hash,
            content_signature: self.content_signature,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

#[derive(Debug, FromRow)]
pub struct WorkSummaryRow {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub library_status: String,
    pub enrichment_state: String,
    pub tags: Option<String>,
    pub release_date: Option<String>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub variant_count: Option<i64>,
    pub asset_count: Option<i64>,
    pub asset_types: Option<String>,
    pub primary_asset_type: Option<String>,
}

impl WorkSummaryRow {
    pub fn into_summary(self) -> WorkSummary {
        WorkSummary {
            id: WorkId::parse(&self.id).unwrap_or_default(),
            title: self.title,
            cover_path: self.cover_path,
            developer: self.developer,
            rating: self.rating,
            library_status: serde_json::from_str(&format!("\"{}\"", self.library_status))
                .unwrap_or_default(),
            enrichment_state: serde_json::from_str(&format!("\"{}\"", self.enrichment_state))
                .unwrap_or_default(),
            tags: self
                .tags
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default(),
            release_date: self
                .release_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            vndb_id: self.vndb_id,
            bangumi_id: self.bangumi_id,
            dlsite_id: self.dlsite_id,
            variant_count: self.variant_count.unwrap_or(1).max(1) as u32,
            asset_count: self.asset_count.unwrap_or(0).max(0) as u32,
            asset_types: self
                .asset_types
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default(),
            primary_asset_type: self.primary_asset_type,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct FolderMtimeRow {
    pub folder_path: String,
    pub folder_mtime: f64,
}

#[derive(Debug, FromRow)]
pub struct MetadataCheckRow {
    pub folder_path: String,
    pub metadata_mtime: f64,
    pub metadata_hash: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct EnrichmentMappingRow {
    pub normalized_title: String,
    pub source: String,
    pub external_id: String,
    pub resolved_title: String,
    pub title_original: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct JobRow {
    pub id: i64,
    pub work_id: String,
    pub job_type: String,
    pub state: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub next_run_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AppJobRow {
    pub id: i64,
    pub kind: String,
    pub state: String,
    pub title: String,
    pub progress_pct: f64,
    pub current_step: Option<String>,
    pub checkpoint_json: Option<String>,
    pub payload: Option<String>,
    pub result_json: Option<String>,
    pub last_error: Option<String>,
    pub can_pause: i64,
    pub can_resume: i64,
    pub can_cancel: i64,
    pub dedup_key: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
