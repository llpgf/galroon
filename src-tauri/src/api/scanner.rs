//! Scanner API — Tauri IPC commands for scan control.

use serde::Serialize;
use tauri::State;

use crate::config::SharedConfig;
use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::ids::WorkId;
use crate::domain::work::{EnrichmentState, FieldSource, LibraryStatus, Work};
use crate::fs::metadata_io;
use crate::scanner::classifier;
use crate::scanner::discover;
use crate::scanner::ingest;

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub job_id: Option<i64>,
    pub state: String,
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
    pub total: u64,
}

#[tauri::command]
pub async fn trigger_scan(
    db: State<'_, Database>,
) -> Result<ScanResult, AppError> {
    let job_id = queries::app_jobs::enqueue_job(
        db.read_pool(),
        "scan_library",
        "Scan library roots",
        None,
        Some("scan:library"),
        true,
        true,
        true,
    )
    .await?;

    Ok(ScanResult {
        job_id: Some(job_id),
        state: "queued".to_string(),
        added: 0,
        removed: 0,
        modified: 0,
        moved: 0,
        total: 0,
    })
}

#[tauri::command]
pub async fn get_scan_status(db: State<'_, Database>) -> Result<serde_json::Value, AppError> {
    let latest = queries::app_jobs::list_jobs(db.read_pool(), 20)
        .await?
        .into_iter()
        .find(|job| job.kind == "scan_library");

    if let Some(job) = latest {
        return Ok(serde_json::json!({
            "is_scanning": matches!(job.state.as_str(), "queued" | "running" | "paused"),
            "stage": job.current_step,
            "job_id": job.id,
            "state": job.state,
            "progress_pct": job.progress_pct,
            "last_error": job.last_error,
            "result": job.result_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok()),
        }));
    }

    Ok(serde_json::json!({
        "is_scanning": false,
        "stage": "idle",
        "job_id": null,
        "state": "idle",
        "progress_pct": 0.0,
        "last_error": null,
        "result": null,
    }))
}

pub async fn run_scan_job(config: &SharedConfig, db: &Database, job_id: i64) -> Result<ScanResult, AppError> {
    queries::app_jobs::update_progress(
        db.read_pool(),
        job_id,
        2.0,
        Some("Walking library roots"),
        None,
    )
    .await?;

    let cfg = config.read().await;
    let roots = cfg.library_roots.clone();
    drop(cfg);

    if roots.is_empty() {
        let empty = ScanResult {
            job_id: Some(job_id),
            state: "completed".to_string(),
            added: 0,
            removed: 0,
            modified: 0,
            moved: 0,
            total: 0,
        };
        return Ok(empty);
    }

    let fs_folders = discover::walk_library_roots(&roots);
    check_job_control(db.read_pool(), job_id).await?;
    queries::app_jobs::update_progress(
        db.read_pool(),
        job_id,
        15.0,
        Some("Computing diff"),
        Some(&serde_json::json!({ "discovered": fs_folders.len() })),
    )
    .await?;

    let db_rows = queries::works::get_all_folder_mtimes(db.read_pool()).await?;
    let mut entries = std::collections::HashMap::new();
    for r in db_rows {
        entries.insert(r.folder_path, (r.folder_mtime, None));
    }
    let db_state = discover::DbState { entries };
    let diff = discover::compute_diff(fs_folders, &db_state);

    let total_units = (diff.added.len() + diff.modified.len() + diff.moved.len() + diff.removed.len())
        .max(1) as f64;
    let mut completed_units = 0.0;

    let mut added_count: u64 = 0;
    let removed_count = diff.removed.len() as u64;
    let mut modified_count: u64 = 0;
    let mut moved_count = diff.moved.len() as u64;
    let mut affected_work_ids: Vec<String> = Vec::new();
    let removed_lookup = load_removed_signature_matches(&diff.removed, db.read_pool()).await?;
    let removed_path_set = diff
        .removed
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<std::collections::HashSet<_>>();
    let mut signature_moved_old_paths = std::collections::HashSet::new();
    let mut signature_moved_new_paths = std::collections::HashSet::new();

    for info in &diff.added {
        if let Some(mut work) = ingest::ingest_folder(&info.path, info.mtime) {
            let Some(signature) = work.content_signature.clone() else {
                continue;
            };
            let Some(existing) = removed_lookup.get(&signature) else {
                continue;
            };
            if existing.len() != 1 {
                continue;
            }
            let old = &existing[0];
            let old_path = old.folder_path.clone();
            if !signature_moved_old_paths.insert(old_path.clone()) {
                continue;
            }
            signature_moved_new_paths.insert(info.path.to_string_lossy().to_string());

            let existing_work = old.clone().into_work();
            inherit_work_identity(&existing_work, &mut work);
            persist_move_metadata(&work)?;
            let assets = classifier::classify_folder(&info.path);
            queries::works::move_work_and_refresh(db.read_pool(), &work, &old_path).await?;
            queries::assets::replace_assets_for_work(db.read_pool(), &work.id.to_string(), &assets)
                .await?;
            affected_work_ids.push(work.id.to_string());
            moved_count += 1;
        }
    }

    for info in &diff.added {
        if signature_moved_new_paths.contains(&info.path.to_string_lossy().to_string()) {
            continue;
        }
        if let Some(work) = ingest::ingest_folder(&info.path, info.mtime) {
            match persist_scanned_work(db.read_pool(), work, &info.path, &removed_path_set).await? {
                ScanPersistOutcome::Added(work_id) | ScanPersistOutcome::Cloned(work_id) => {
                    affected_work_ids.push(work_id);
                    added_count += 1;
                }
                ScanPersistOutcome::Moved(work_id) => {
                    affected_work_ids.push(work_id);
                    moved_count += 1;
                }
                ScanPersistOutcome::Refreshed(work_id) => {
                    affected_work_ids.push(work_id);
                    modified_count += 1;
                }
            }
        }
        completed_units += 1.0;
        report_scan_progress(db.read_pool(), job_id, 15.0, 65.0, completed_units / total_units, "Ingesting new folders").await?;
        check_job_control(db.read_pool(), job_id).await?;
    }

    for info in &diff.modified {
        if let Some(work) = ingest::ingest_folder(&info.path, info.mtime) {
            let outcome =
                persist_scanned_work(db.read_pool(), work, &info.path, &removed_path_set).await?;
            let work_id = match outcome {
                ScanPersistOutcome::Added(work_id)
                | ScanPersistOutcome::Cloned(work_id)
                | ScanPersistOutcome::Moved(work_id)
                | ScanPersistOutcome::Refreshed(work_id) => work_id,
            };
            affected_work_ids.push(work_id);
            modified_count += 1;
        }
        completed_units += 1.0;
        report_scan_progress(db.read_pool(), job_id, 15.0, 65.0, completed_units / total_units, "Refreshing modified folders").await?;
        check_job_control(db.read_pool(), job_id).await?;
    }

    for (old_path, new_info) in &diff.moved {
        let old_path_str = old_path.to_string_lossy().to_string();
        if let Some(old_row) =
            queries::works::get_work_by_path(db.read_pool(), &old_path_str).await?
        {
            let existing = old_row.into_work();
            affected_work_ids.push(existing.id.to_string());

            if let Some(mut work) = ingest::ingest_folder(&new_info.path, new_info.mtime) {
                inherit_work_identity(&existing, &mut work);
                persist_move_metadata(&work)?;
                let assets = classifier::classify_folder(&new_info.path);
                queries::works::move_work_and_refresh(db.read_pool(), &work, &old_path_str).await?;
                queries::assets::replace_assets_for_work(
                    db.read_pool(),
                    &work.id.to_string(),
                    &assets,
                )
                .await?;
                affected_work_ids.push(work.id.to_string());
            }
        } else if let Some(work) = ingest::ingest_folder(&new_info.path, new_info.mtime) {
            let outcome =
                persist_scanned_work(db.read_pool(), work, &new_info.path, &removed_path_set)
                    .await?;
            let work_id = match outcome {
                ScanPersistOutcome::Added(work_id)
                | ScanPersistOutcome::Cloned(work_id)
                | ScanPersistOutcome::Moved(work_id)
                | ScanPersistOutcome::Refreshed(work_id) => work_id,
            };
            affected_work_ids.push(work_id);
        }
        completed_units += 1.0;
        report_scan_progress(db.read_pool(), job_id, 15.0, 65.0, completed_units / total_units, "Reconciling moved folders").await?;
        check_job_control(db.read_pool(), job_id).await?;
    }

    for path in &diff.removed {
        if signature_moved_old_paths.contains(&path.to_string_lossy().to_string()) {
            continue;
        }
        if let Some(old_row) =
            queries::works::get_work_by_path(db.read_pool(), &path.to_string_lossy()).await?
        {
            affected_work_ids.push(old_row.id);
        }
        queries::works::delete_work_by_path(db.read_pool(), &path.to_string_lossy()).await?;
        completed_units += 1.0;
        report_scan_progress(db.read_pool(), job_id, 15.0, 65.0, completed_units / total_units, "Removing missing folders").await?;
        check_job_control(db.read_pool(), job_id).await?;
    }

    queries::app_jobs::update_progress(
        db.read_pool(),
        job_id,
        82.0,
        Some("Queueing metadata refresh"),
        Some(&serde_json::json!({ "affected": affected_work_ids.len() })),
    )
    .await?;

    let mut queued_work_ids = std::collections::HashSet::new();
    for work_id in &affected_work_ids {
        if queued_work_ids.insert(work_id.clone()) {
            let dedup_key = format!("refresh:{work_id}");
            let _ = queries::jobs::enqueue_job(
                db.read_pool(),
                work_id,
                "metadata_refresh",
                Some(&dedup_key),
                None,
            )
            .await;
        }
    }

    queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    let total_rows =
        queries::canonical::list_canonical_works(db.read_pool(), "title", false, None).await?;
    let result = ScanResult {
        job_id: Some(job_id),
        state: "completed".to_string(),
        added: added_count,
        removed: removed_count,
        modified: modified_count,
        moved: moved_count,
        total: total_rows.len() as u64,
    };

    queries::app_jobs::update_progress(
        db.read_pool(),
        job_id,
        97.0,
        Some("Finalizing canonical poster view"),
        Some(&serde_json::json!({
            "added": added_count,
            "removed": removed_count,
            "modified": modified_count,
            "moved": moved_count,
            "total": result.total,
        })),
    )
    .await?;

    tracing::info!(?result, "Scan complete");
    Ok(result)
}

async fn report_scan_progress(
    pool: &sqlx::SqlitePool,
    job_id: i64,
    base: f64,
    span: f64,
    ratio: f64,
    step: &str,
) -> Result<(), AppError> {
    let pct = (base + span * ratio.clamp(0.0, 1.0)).min(95.0);
    queries::app_jobs::update_progress(pool, job_id, pct, Some(step), None).await?;
    Ok(())
}

async fn check_job_control(pool: &sqlx::SqlitePool, job_id: i64) -> Result<(), AppError> {
    if let Some(job) = queries::app_jobs::get_job(pool, job_id).await? {
        if job.state == "cancelled" {
            return Err(AppError::Internal("job_cancelled".to_string()));
        }
        if job.state == "paused" {
            return Err(AppError::Internal("job_paused".to_string()));
        }
    }
    Ok(())
}

fn inherit_work_identity(existing: &Work, incoming: &mut Work) {
    incoming.id = existing.id.clone();
    incoming.created_at = existing.created_at;

    let metadata_missing = incoming.metadata_hash.as_deref() == Some("no_file");
    if metadata_missing
        || (matches!(incoming.title_source, FieldSource::Filesystem)
            && !matches!(existing.title_source, FieldSource::Filesystem))
    {
        incoming.title = existing.title.clone();
        incoming.title_original = existing.title_original.clone();
        incoming.title_aliases = if incoming.title_aliases.is_empty() {
            existing.title_aliases.clone()
        } else {
            incoming.title_aliases.clone()
        };
        incoming.title_source = existing.title_source.clone();
    } else if incoming.title_aliases.is_empty() {
        incoming.title_aliases = existing.title_aliases.clone();
    }

    if incoming.developer.is_none() {
        incoming.developer = existing.developer.clone();
    }
    if incoming.publisher.is_none() {
        incoming.publisher = existing.publisher.clone();
    }
    if incoming.release_date.is_none() {
        incoming.release_date = existing.release_date;
    }
    if incoming.rating.is_none() {
        incoming.rating = existing.rating;
    }
    if incoming.vote_count.is_none() {
        incoming.vote_count = existing.vote_count;
    }
    if incoming.description.is_none() {
        incoming.description = existing.description.clone();
    }
    if incoming.cover_path.is_none() {
        incoming.cover_path = existing.cover_path.clone();
    }
    if incoming.tags.is_empty() {
        incoming.tags = existing.tags.clone();
    }
    if incoming.user_tags.is_empty() {
        incoming.user_tags = existing.user_tags.clone();
    }
    if incoming.vndb_id.is_none() {
        incoming.vndb_id = existing.vndb_id.clone();
    }
    if incoming.bangumi_id.is_none() {
        incoming.bangumi_id = existing.bangumi_id.clone();
    }
    if incoming.dlsite_id.is_none() {
        incoming.dlsite_id = existing.dlsite_id.clone();
    }
    if matches!(incoming.enrichment_state, EnrichmentState::Unmatched)
        && !matches!(existing.enrichment_state, EnrichmentState::Unmatched)
    {
        incoming.enrichment_state = existing.enrichment_state.clone();
    }
    if matches!(incoming.library_status, LibraryStatus::Unplayed)
        && !matches!(existing.library_status, LibraryStatus::Unplayed)
    {
        incoming.library_status = existing.library_status.clone();
    }
}

fn persist_move_metadata(work: &Work) -> Result<(), AppError> {
    metadata_io::sync_metadata_from_work(work, None).map_err(AppError::Io)
}

enum ScanPersistOutcome {
    Added(String),
    Refreshed(String),
    Moved(String),
    Cloned(String),
}

async fn persist_scanned_work(
    pool: &sqlx::SqlitePool,
    mut work: Work,
    folder_path: &std::path::Path,
    removed_paths: &std::collections::HashSet<String>,
) -> Result<ScanPersistOutcome, AppError> {
    let assets = classifier::classify_folder(folder_path);
    let incoming_path = folder_path.to_string_lossy().to_string();
    if let Some(existing_row) = queries::works::get_work_by_id(pool, &work.id.to_string()).await? {
        let existing = existing_row.into_work();
        let existing_path = existing.folder_path.to_string_lossy().to_string();
        if existing_path != incoming_path {
            if removed_paths.contains(&existing_path) || !existing.folder_path.exists() {
                inherit_work_identity(&existing, &mut work);
                persist_move_metadata(&work)?;
                queries::works::move_work_and_refresh(pool, &work, &existing_path).await?;
                queries::assets::replace_assets_for_work(pool, &work.id.to_string(), &assets)
                    .await?;
                return Ok(ScanPersistOutcome::Moved(work.id.to_string()));
            }

            reseed_cloned_work_identity(&existing, &mut work);
            persist_move_metadata(&work)?;
            queries::works::upsert_work(pool, &work).await?;
            queries::assets::replace_assets_for_work(pool, &work.id.to_string(), &assets).await?;
            clone_review_state(pool, &existing.id.to_string(), &work.id.to_string()).await?;
            return Ok(ScanPersistOutcome::Cloned(work.id.to_string()));
        }
    }

    let exists_by_path = queries::works::get_work_by_path(pool, &incoming_path)
        .await?
        .is_some();
    queries::works::upsert_work(pool, &work).await?;
    queries::assets::replace_assets_for_work(pool, &work.id.to_string(), &assets).await?;
    Ok(if exists_by_path {
        ScanPersistOutcome::Refreshed(work.id.to_string())
    } else {
        ScanPersistOutcome::Added(work.id.to_string())
    })
}

fn reseed_cloned_work_identity(existing: &Work, incoming: &mut Work) {
    inherit_work_identity(existing, incoming);
    incoming.id = WorkId::new();
    incoming.created_at = chrono::Utc::now();
}

async fn clone_review_state(
    pool: &sqlx::SqlitePool,
    source_work_id: &str,
    target_work_id: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO canonical_variant_overrides (work_id, manual_group_key, make_representative, created_at, updated_at)
         SELECT ?1, manual_group_key, 0, datetime('now'), datetime('now')
         FROM canonical_variant_overrides
         WHERE work_id = ?2
         ON CONFLICT(work_id) DO UPDATE SET
         manual_group_key = excluded.manual_group_key,
         updated_at = datetime('now')",
    )
    .bind(target_work_id)
    .bind(source_work_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO workshop_ignored_diagnostics (work_id, category)
         SELECT ?1, category
         FROM workshop_ignored_diagnostics
         WHERE work_id = ?2
         ON CONFLICT(work_id, category) DO NOTHING",
    )
    .bind(target_work_id)
    .bind(source_work_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn load_removed_signature_matches(
    removed_paths: &[std::path::PathBuf],
    pool: &sqlx::SqlitePool,
) -> Result<std::collections::HashMap<String, Vec<crate::db::models::WorkRow>>, AppError> {
    let mut by_signature =
        std::collections::HashMap::<String, Vec<crate::db::models::WorkRow>>::new();

    for path in removed_paths {
        if let Some(row) = queries::works::get_work_by_path(pool, &path.to_string_lossy()).await? {
            if let Some(signature) = row
                .content_signature
                .clone()
                .filter(|value| !value.is_empty())
            {
                by_signature.entry(signature).or_default().push(row);
            }
        }
    }

    Ok(by_signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn inherit_work_identity_preserves_enriched_state_when_metadata_missing() {
        let mut existing = Work::from_discovery("C:/old".into(), "Existing".to_string(), 1.0);
        existing.developer = Some("Brand".to_string());
        existing.vndb_id = Some("v1".to_string());
        existing.enrichment_state = EnrichmentState::Matched;
        existing.title_source = FieldSource::Vndb;

        let mut incoming = Work::from_discovery("C:/new".into(), "Temp".to_string(), 2.0);
        incoming.metadata_hash = Some("no_file".to_string());

        inherit_work_identity(&existing, &mut incoming);

        assert_eq!(incoming.id, existing.id);
        assert_eq!(incoming.title, "Existing");
        assert_eq!(incoming.developer.as_deref(), Some("Brand"));
        assert_eq!(incoming.vndb_id.as_deref(), Some("v1"));
        assert!(matches!(
            incoming.enrichment_state,
            EnrichmentState::Matched
        ));
        assert!(matches!(incoming.title_source, FieldSource::Vndb));
    }

    #[test]
    fn diagnostics_placeholder_pattern_is_detectable() {
        let code_like = Regex::new(r"(?i)^[a-z]{0,2}\d{5,10}$").expect("regex");
        assert!(code_like.is_match("VJ01004242"));
        assert!(code_like.is_match("1261651"));
    }
}
