//! Metadata JSON read/write with atomic tmp→rename (R2).
//!
//! This is the central authority for Work state.
//! DB is a read model; metadata.json is the source of truth.

use std::path::Path;

use tracing::{debug, warn};
use uuid::Uuid;

use crate::domain::metadata::MetadataJson;
use crate::domain::work::{EnrichmentState, LibraryStatus, Work};
use crate::scanner::watcher::RecentWrites;

/// Read metadata.json from a game folder.
///
/// Returns None if file doesn't exist or is unparseable.
pub fn read_metadata(folder: &Path) -> Option<MetadataJson> {
    let path = folder.join("metadata.json");
    let content = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&content) {
        Ok(meta) => Some(meta),
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                "Failed to parse metadata.json, treating as new"
            );
            None
        }
    }
}

/// Write metadata.json atomically: tmp → rename (R2).
///
/// Also:
/// - Sets write_nonce and last_written_by for watcher suppression (R20)
/// - Records the write in RecentWrites so the watcher can suppress the event
pub fn write_metadata(
    folder: &Path,
    metadata: &mut MetadataJson,
    recent_writes: Option<&RecentWrites>,
) -> std::io::Result<()> {
    // R20: Set tracking fields
    metadata.write_nonce = Some(Uuid::now_v7().to_string());
    metadata.last_written_by = Some("galroon".to_string());

    let target = folder.join("metadata.json");
    let tmp = folder.join(".metadata.json.tmp");

    let content = serde_json::to_string_pretty(metadata)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Write to temp file first
    std::fs::write(&tmp, &content)?;

    // Atomic rename (R2)
    std::fs::rename(&tmp, &target)?;

    // R20: Record this write so the watcher suppresses the event
    if let Some(rw) = recent_writes {
        rw.record(target.clone());
    }

    debug!(path = %target.display(), "metadata.json written atomically (R2, R20)");
    Ok(())
}

pub fn sync_metadata_from_work(
    work: &Work,
    recent_writes: Option<&RecentWrites>,
) -> std::io::Result<()> {
    let mut metadata = read_metadata(&work.folder_path).unwrap_or_default();
    apply_work_to_metadata(&mut metadata, work);
    write_metadata(&work.folder_path, &mut metadata, recent_writes)
}

pub fn apply_work_to_metadata(metadata: &mut MetadataJson, work: &Work) {
    metadata.work_id = Some(work.id.to_string());
    metadata.title = Some(work.title.clone());
    metadata.title_original = work.title_original.clone();
    metadata.title_aliases = work.title_aliases.clone();
    metadata.developer = work.developer.clone();
    metadata.publisher = work.publisher.clone();
    metadata.release_date = work.release_date;
    metadata.description = work.description.clone();
    metadata.tags = work.tags.clone();
    metadata.user_tags = work.user_tags.clone();
    metadata.field_sources = work.field_sources.clone();
    metadata.field_preferences = work.field_preferences.clone();
    metadata.user_overrides = work.user_overrides.clone();
    metadata.library_status = Some(enum_label_library_status(&work.library_status).to_string());
    metadata.vndb_id = work.vndb_id.clone();
    metadata.bangumi_id = work.bangumi_id.clone();
    metadata.dlsite_id = work.dlsite_id.clone();
    metadata.rating = work.rating;
    metadata.vote_count = work.vote_count;
    metadata.enrichment_state =
        Some(enum_label_enrichment_state(&work.enrichment_state).to_string());
    metadata.content_signature = work.content_signature.clone();

    metadata.cover = None;
    metadata.cover_url = None;

    if let Some(cover_path) = work
        .cover_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if cover_path.starts_with("http://") || cover_path.starts_with("https://") {
            metadata.cover_url = Some(cover_path.to_string());
        } else {
            let cover_path = std::path::Path::new(cover_path);
            if cover_path.is_absolute() {
                if let Ok(relative) = cover_path.strip_prefix(&work.folder_path) {
                    metadata.cover = Some(relative.to_string_lossy().replace('\\', "/"));
                } else {
                    metadata.cover = Some(cover_path.to_string_lossy().replace('\\', "/"));
                }
            } else {
                metadata.cover = Some(cover_path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

fn enum_label_library_status(status: &LibraryStatus) -> &'static str {
    match status {
        LibraryStatus::Unplayed => "unplayed",
        LibraryStatus::Playing => "playing",
        LibraryStatus::Completed => "completed",
        LibraryStatus::OnHold => "on_hold",
        LibraryStatus::Dropped => "dropped",
        LibraryStatus::Wishlist => "wishlist",
    }
}

fn enum_label_enrichment_state(state: &EnrichmentState) -> &'static str {
    match state {
        EnrichmentState::Unmatched => "unmatched",
        EnrichmentState::PendingReview => "pending_review",
        EnrichmentState::Matched => "matched",
        EnrichmentState::Rejected => "rejected",
    }
}

/// Compute a hash of metadata.json for sanity checking (R2).
///
/// Uses FNV-1a: fast, good enough for change detection.
pub fn compute_metadata_hash(folder: &Path) -> String {
    let path = folder.join("metadata.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut hash: u64 = 14695981039346656037;
            for byte in &bytes {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(1099511628211);
            }
            format!("{:016x}", hash)
        }
        Err(_) => "no_file".to_string(),
    }
}

/// Startup sanity check (R2): compare metadata.json mtimes/hashes with DB.
///
/// Returns list of folder paths that need to be re-ingested because
/// their metadata.json has changed since the DB last saw it.
pub fn find_stale_entries(
    db_checks: &[(String, f64, Option<String>)], // (path, mtime, hash)
) -> Vec<String> {
    let mut stale = Vec::new();

    for (path, db_mtime, db_hash) in db_checks {
        let folder = std::path::Path::new(path);
        let meta_path = folder.join("metadata.json");

        // Check if file still exists
        let file_mtime = match std::fs::metadata(&meta_path) {
            Ok(m) => m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
            Err(_) => continue, // File gone, skip (will be handled by scanner diff)
        };

        // Fast path: if mtime hasn't changed, skip hash check
        if (file_mtime - db_mtime).abs() < 0.001 {
            continue;
        }

        // Mtime changed — check hash
        let current_hash = compute_metadata_hash(folder);
        let hashes_match = db_hash
            .as_ref()
            .map(|h| h == &current_hash)
            .unwrap_or(false);

        if !hashes_match {
            debug!(
                path = %path,
                db_mtime = %db_mtime,
                file_mtime = %file_mtime,
                "Stale metadata detected (R2)"
            );
            stale.push(path.clone());
        }
    }

    if !stale.is_empty() {
        tracing::info!(
            count = stale.len(),
            "Found stale metadata entries needing re-ingest (R2)"
        );
    }

    stale
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::domain::ids::WorkId;
    use crate::domain::work::{EnrichmentState, FieldSource, LibraryStatus, Work};

    use super::*;

    fn sample_work(folder_path: PathBuf) -> Work {
        Work {
            id: WorkId::new(),
            folder_path,
            title: "Sample".to_string(),
            title_original: Some("サンプル".to_string()),
            title_aliases: vec!["Alias".to_string()],
            developer: Some("Brand".to_string()),
            publisher: Some("Pub".to_string()),
            release_date: None,
            rating: Some(7.8),
            vote_count: Some(42),
            description: Some("Desc".to_string()),
            cover_path: None,
            tags: vec!["tag".to_string()],
            user_tags: vec!["mine".to_string()],
            field_sources: HashMap::from([("title".to_string(), "vndb".to_string())]),
            field_preferences: HashMap::new(),
            user_overrides: HashMap::new(),
            library_status: LibraryStatus::Playing,
            vndb_id: Some("v1".to_string()),
            bangumi_id: Some("2".to_string()),
            dlsite_id: Some("RJ123456".to_string()),
            enrichment_state: EnrichmentState::Matched,
            title_source: FieldSource::Vndb,
            folder_mtime: 0.0,
            metadata_mtime: 0.0,
            metadata_hash: None,
            content_signature: Some("abc".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn apply_work_to_metadata_persists_remote_cover_url() {
        let mut metadata = MetadataJson::default();
        let mut work = sample_work(PathBuf::from("C:/tmp/work"));
        work.cover_path = Some("https://example.com/cover.jpg".to_string());

        apply_work_to_metadata(&mut metadata, &work);

        assert_eq!(
            metadata.cover_url.as_deref(),
            Some("https://example.com/cover.jpg")
        );
        assert!(metadata.cover.is_none());
    }

    #[test]
    fn apply_work_to_metadata_relativizes_local_cover() {
        let mut metadata = MetadataJson::default();
        let mut work = sample_work(PathBuf::from("C:/tmp/work"));
        work.cover_path = Some("C:/tmp/work/covers/poster.webp".to_string());

        apply_work_to_metadata(&mut metadata, &work);

        assert_eq!(metadata.cover.as_deref(), Some("covers/poster.webp"));
        assert!(metadata.cover_url.is_none());
    }
}
