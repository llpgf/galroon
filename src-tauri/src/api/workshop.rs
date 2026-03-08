//! Metadata Workshop — bulk edit, merge works, poster review, re-match, year-in-review.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use tauri::State;

use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::EnrichmentState;
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::people;
use crate::enrichment::provider::{self, MetadataSource, ProviderLinkState};
use crate::enrichment::resolver;
use crate::enrichment::vndb::VndbClient;
use crate::fs::metadata_io;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkshopDiagnosticInput {
    pub work_id: String,
    pub category: String,
    pub preferred_field: Option<String>,
    #[serde(default)]
    pub linked_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchWorkshopResult {
    pub updated: u64,
    pub skipped: u64,
}

// ── Bulk Edit ──

#[tauri::command]
pub async fn bulk_update_field(
    db: State<'_, Database>,
    work_ids: Vec<String>,
    field: String,
    value: String,
) -> Result<u64, AppError> {
    match field.as_str() {
        "library_status" | "developer" | "publisher" => {}
        _ => {
            return Err(AppError::Validation(format!(
                "Field '{}' not allowed for bulk edit",
                field
            )))
        }
    };

    let mut affected: u64 = 0;
    let mut affected_work_ids = Vec::new();
    for work_id in work_ids {
        let preferred_id =
            crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
                .await?
                .unwrap_or(work_id);
        let row = crate::db::queries::works::get_work_by_id(db.read_pool(), &preferred_id)
            .await?
            .ok_or_else(|| AppError::WorkNotFound(preferred_id.clone()))?;
        let mut work = row.into_work();

        match field.as_str() {
            "library_status" => {
                work.library_status = serde_json::from_str(&format!("\"{}\"", value))
                    .map_err(|_| AppError::Validation("Invalid library_status".to_string()))?;
                work.user_overrides.insert(
                    "library_status".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            "developer" => {
                work.developer = Some(value.clone());
                work.user_overrides.insert(
                    "developer".to_string(),
                    serde_json::Value::String(value.clone()),
                );
                work.field_sources
                    .insert("developer".to_string(), "user_override".to_string());
            }
            "publisher" => {
                work.publisher = Some(value.clone());
                work.user_overrides.insert(
                    "publisher".to_string(),
                    serde_json::Value::String(value.clone()),
                );
                work.field_sources
                    .insert("publisher".to_string(), "user_override".to_string());
            }
            _ => {}
        }

        crate::db::queries::works::upsert_work(db.read_pool(), &work).await?;
        metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        affected_work_ids.push(preferred_id);
        affected += 1;
    }

    crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;

    Ok(affected)
}

#[tauri::command]
pub async fn set_canonical_representative(
    db: State<'_, Database>,
    work_id: String,
) -> Result<(), AppError> {
    ensure_work_exists(&db, &work_id).await?;
    let affected_work_ids =
        crate::db::queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;
    let manual_group_key = canonical_group_key(&work_id);

    let pool = db.read_pool();
    let mut tx = pool.begin().await?;
    for variant_id in &affected_work_ids {
        upsert_variant_override(&mut tx, variant_id, &manual_group_key, false).await?;
    }
    upsert_variant_override(&mut tx, &work_id, &manual_group_key, true).await?;
    tx.commit().await?;

    crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    Ok(())
}

#[tauri::command]
pub async fn split_work_variant(db: State<'_, Database>, work_id: String) -> Result<(), AppError> {
    ensure_work_exists(&db, &work_id).await?;
    let mut affected_work_ids =
        crate::db::queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;
    affected_work_ids.push(work_id.clone());
    let affected_work_ids = unique_work_ids(affected_work_ids);
    let manual_group_key = split_group_key(&work_id);

    let pool = db.read_pool();
    let mut tx = pool.begin().await?;
    upsert_variant_override(&mut tx, &work_id, &manual_group_key, true).await?;
    tx.commit().await?;

    crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    Ok(())
}

#[tauri::command]
pub async fn clear_work_variant_override(
    db: State<'_, Database>,
    work_id: String,
) -> Result<(), AppError> {
    ensure_work_exists(&db, &work_id).await?;
    let mut affected_work_ids =
        crate::db::queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;
    affected_work_ids.push(work_id.clone());
    let affected_work_ids = unique_work_ids(affected_work_ids);

    sqlx::query("DELETE FROM canonical_variant_overrides WHERE work_id = ?")
        .bind(&work_id)
        .execute(db.read_pool())
        .await?;

    crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    Ok(())
}

#[tauri::command]
pub async fn merge_poster_groups(
    db: State<'_, Database>,
    target_id: String,
    source_id: String,
) -> Result<(), AppError> {
    if target_id == source_id {
        return Err(AppError::Validation(
            "Cannot merge a poster into itself".to_string(),
        ));
    }

    ensure_work_exists(&db, &target_id).await?;
    ensure_work_exists(&db, &source_id).await?;

    let target_key = crate::db::queries::canonical::get_canonical_key(db.read_pool(), &target_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(target_id.clone()))?;
    let source_key = crate::db::queries::canonical::get_canonical_key(db.read_pool(), &source_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(source_id.clone()))?;
    if target_key == source_key {
        return Err(AppError::Validation(
            "Works already belong to the same poster".to_string(),
        ));
    }

    let mut affected_work_ids =
        crate::db::queries::canonical::list_variant_ids(db.read_pool(), &target_id).await?;
    affected_work_ids
        .extend(crate::db::queries::canonical::list_variant_ids(db.read_pool(), &source_id).await?);
    let affected_work_ids = unique_work_ids(affected_work_ids);
    let manual_group_key = canonical_group_key(&target_id);

    let pool = db.read_pool();
    let mut tx = pool.begin().await?;
    for variant_id in &affected_work_ids {
        upsert_variant_override(&mut tx, variant_id, &manual_group_key, false).await?;
    }
    upsert_variant_override(&mut tx, &target_id, &manual_group_key, true).await?;
    tx.commit().await?;

    crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    Ok(())
}

/// Merge two works into one — keeps the target, deletes the source.
#[tauri::command]
pub async fn merge_works(
    db: State<'_, Database>,
    target_id: String,
    source_id: String,
) -> Result<(), AppError> {
    if target_id == source_id {
        return Err(AppError::Validation(
            "Cannot merge a work into itself".to_string(),
        ));
    }

    let pool = db.read_pool();
    let mut tx = pool.begin().await?;

    let target_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM works WHERE id = ?")
        .bind(&target_id)
        .fetch_optional(&mut *tx)
        .await?;
    if target_exists.is_none() {
        return Err(AppError::WorkNotFound(target_id.clone()));
    }

    let source_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM works WHERE id = ?")
        .bind(&source_id)
        .fetch_optional(&mut *tx)
        .await?;
    if source_exists.is_none() {
        return Err(AppError::WorkNotFound(source_id.clone()));
    }

    let target_completion: Option<CompletionRow> = sqlx::query_as(
        "SELECT work_id, status, progress_pct, playtime_min, started_at, completed_at, notes \
         FROM completion_tracking WHERE work_id = ?",
    )
    .bind(&target_id)
    .fetch_optional(&mut *tx)
    .await?;
    let source_completion: Option<CompletionRow> = sqlx::query_as(
        "SELECT work_id, status, progress_pct, playtime_min, started_at, completed_at, notes \
         FROM completion_tracking WHERE work_id = ?",
    )
    .bind(&source_id)
    .fetch_optional(&mut *tx)
    .await?;

    for sql in [
        "UPDATE OR IGNORE assets SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE collection_items SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE work_auto_tags SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE work_user_tags SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE work_characters SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE work_credits SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE OR IGNORE work_texts SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE jobs SET work_id = ?1 WHERE work_id = ?2",
        "UPDATE import_queue SET target_work = ?1 WHERE target_work = ?2",
    ] {
        sqlx::query(sql)
            .bind(&target_id)
            .bind(&source_id)
            .execute(&mut *tx)
            .await?;
    }

    if let Some(merged) = merge_completion_rows(target_completion, source_completion, &target_id) {
        sqlx::query(
            "INSERT INTO completion_tracking (work_id, status, progress_pct, playtime_min, started_at, completed_at, notes, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now')) \
             ON CONFLICT(work_id) DO UPDATE SET \
             status = excluded.status, progress_pct = excluded.progress_pct, playtime_min = excluded.playtime_min, \
             started_at = excluded.started_at, completed_at = excluded.completed_at, notes = excluded.notes, updated_at = datetime('now')",
        )
        .bind(&merged.work_id)
        .bind(&merged.status)
        .bind(merged.progress_pct)
        .bind(merged.playtime_min)
        .bind(&merged.started_at)
        .bind(&merged.completed_at)
        .bind(&merged.notes)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM completion_tracking WHERE work_id = ?")
        .bind(&source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM works WHERE id = ?")
        .bind(&source_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    crate::db::queries::canonical::sync_work_ids(
        db.read_pool(),
        &[target_id.clone(), source_id.clone()],
    )
    .await?;

    Ok(())
}

#[derive(Debug, Clone, FromRow)]
struct CompletionRow {
    work_id: String,
    status: String,
    progress_pct: i32,
    playtime_min: i32,
    started_at: Option<String>,
    completed_at: Option<String>,
    notes: String,
}

fn merge_completion_rows(
    target: Option<CompletionRow>,
    source: Option<CompletionRow>,
    target_id: &str,
) -> Option<CompletionRow> {
    match (target, source) {
        (None, None) => None,
        (Some(mut target), None) => {
            target.work_id = target_id.to_string();
            Some(target)
        }
        (None, Some(mut source)) => {
            source.work_id = target_id.to_string();
            Some(source)
        }
        (Some(mut target), Some(source)) => {
            target.work_id = target_id.to_string();
            if completion_status_rank(&source.status) > completion_status_rank(&target.status) {
                target.status = source.status;
            }
            target.progress_pct = target.progress_pct.max(source.progress_pct);
            target.playtime_min = target.playtime_min.max(source.playtime_min);
            if target.started_at.is_none() {
                target.started_at = source.started_at;
            }
            if target.completed_at.is_none() {
                target.completed_at = source.completed_at;
            }
            if target.notes.trim().is_empty() && !source.notes.trim().is_empty() {
                target.notes = source.notes;
            }
            Some(target)
        }
    }
}

fn completion_status_rank(status: &str) -> i32 {
    match status {
        "completed" => 5,
        "in_progress" => 4,
        "on_hold" => 3,
        "dropped" => 2,
        "not_started" => 1,
        _ => 0,
    }
}

/// Reset enrichment state for a work (re-match).
#[tauri::command]
pub async fn reset_enrichment(db: State<'_, Database>, work_id: String) -> Result<(), AppError> {
    let preferred_id =
        crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
            .await?
            .unwrap_or(work_id);
    let row = crate::db::queries::works::get_work_by_id(db.read_pool(), &preferred_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(preferred_id.clone()))?;
    let mut work = row.into_work();
    work.enrichment_state = EnrichmentState::Unmatched;
    work.vndb_id = None;
    work.bangumi_id = None;
    work.dlsite_id = None;
    crate::db::queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;

    let dedup_key = format!("refresh:{preferred_id}");
    crate::db::queries::jobs::enqueue_job(
        db.read_pool(),
        &preferred_id,
        "metadata_refresh",
        Some(&dedup_key),
        None,
    )
    .await?;
    crate::db::queries::canonical::sync_work_ids(
        db.read_pool(),
        std::slice::from_ref(&preferred_id),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn refresh_work_provider_link(
    db: State<'_, Database>,
    work_id: String,
    source: String,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<(), AppError> {
    let preferred_id =
        crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
            .await?
            .unwrap_or(work_id);
    let source_kind = MetadataSource::from_str(&source)
        .ok_or_else(|| AppError::Validation(format!("Unknown source '{source}'")))?;
    let provider_defaults =
        crate::db::queries::provider_rules::list_field_defaults(db.read_pool()).await?;
    refresh_provider_link_for_work(
        &db,
        &preferred_id,
        source_kind,
        &vndb,
        &bangumi,
        &dlsite,
        &provider_defaults,
    )
    .await?;
    crate::db::queries::canonical::sync_work_ids(
        db.read_pool(),
        std::slice::from_ref(&preferred_id),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn ignore_workshop_diagnostic(
    db: State<'_, Database>,
    work_id: String,
    category: String,
) -> Result<(), AppError> {
    let preferred_id =
        crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
            .await?
            .unwrap_or(work_id);
    sqlx::query(
        "INSERT INTO workshop_ignored_diagnostics (work_id, category) VALUES (?, ?)
         ON CONFLICT(work_id, category) DO NOTHING",
    )
    .bind(preferred_id)
    .bind(category)
    .execute(db.read_pool())
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn restore_workshop_diagnostic(
    db: State<'_, Database>,
    work_id: String,
    category: String,
) -> Result<(), AppError> {
    let preferred_id =
        crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
            .await?
            .unwrap_or(work_id);
    sqlx::query("DELETE FROM workshop_ignored_diagnostics WHERE work_id = ? AND category = ?")
        .bind(preferred_id)
        .bind(category)
        .execute(db.read_pool())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn batch_ignore_workshop_diagnostics(
    db: State<'_, Database>,
    items: Vec<WorkshopDiagnosticInput>,
) -> Result<BatchWorkshopResult, AppError> {
    let mut updated = 0_u64;
    let mut skipped = 0_u64;

    for item in dedupe_diagnostics(items) {
        let preferred_id =
            crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &item.work_id)
                .await?
                .unwrap_or(item.work_id);
        let result = sqlx::query(
            "INSERT INTO workshop_ignored_diagnostics (work_id, category) VALUES (?, ?)
             ON CONFLICT(work_id, category) DO NOTHING",
        )
        .bind(preferred_id)
        .bind(item.category)
        .execute(db.read_pool())
        .await?;
        if result.rows_affected() > 0 {
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(BatchWorkshopResult { updated, skipped })
}

#[tauri::command]
pub async fn batch_restore_workshop_diagnostics(
    db: State<'_, Database>,
    items: Vec<WorkshopDiagnosticInput>,
) -> Result<BatchWorkshopResult, AppError> {
    let mut updated = 0_u64;
    let mut skipped = 0_u64;

    for item in dedupe_diagnostics(items) {
        let preferred_id =
            crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &item.work_id)
                .await?
                .unwrap_or(item.work_id);
        let result = sqlx::query(
            "DELETE FROM workshop_ignored_diagnostics WHERE work_id = ? AND category = ?",
        )
        .bind(preferred_id)
        .bind(item.category)
        .execute(db.read_pool())
        .await?;
        if result.rows_affected() > 0 {
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(BatchWorkshopResult { updated, skipped })
}

#[tauri::command]
pub async fn batch_apply_diagnostic_preferences(
    db: State<'_, Database>,
    items: Vec<WorkshopDiagnosticInput>,
    source: Option<String>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<BatchWorkshopResult, AppError> {
    let normalized_source = normalize_source_choice(source.as_deref())?;
    let provider_defaults =
        crate::db::queries::provider_rules::list_field_defaults(db.read_pool()).await?;
    let mut updated = 0_u64;
    let mut skipped = 0_u64;
    let mut affected_work_ids = Vec::new();

    for item in dedupe_diagnostics(items) {
        let Some(field) = item
            .preferred_field
            .as_deref()
            .and_then(normalize_preference_field)
        else {
            skipped += 1;
            continue;
        };

        if let Some(source_name) = normalized_source.as_deref() {
            if !item.linked_sources.iter().any(|value| value == source_name) {
                skipped += 1;
                continue;
            }
        }

        let preferred_id =
            crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &item.work_id)
                .await?
                .unwrap_or(item.work_id);
        let row = crate::db::queries::works::get_work_by_id(db.read_pool(), &preferred_id)
            .await?
            .ok_or_else(|| AppError::WorkNotFound(preferred_id.clone()))?;
        let mut work = row.into_work();

        match normalized_source.as_deref() {
            Some(source_name) => {
                work.field_preferences
                    .insert(field.to_string(), source_name.to_string());
            }
            None => {
                work.field_preferences.remove(field);
            }
        }

        re_resolve_work(
            db.read_pool(),
            &mut work,
            &vndb,
            &bangumi,
            &dlsite,
            &provider_defaults,
        )
        .await?;
        metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        affected_work_ids.push(preferred_id);
        updated += 1;
    }

    let affected_work_ids = unique_work_ids(affected_work_ids);
    if !affected_work_ids.is_empty() {
        crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    }

    Ok(BatchWorkshopResult { updated, skipped })
}

#[tauri::command]
pub async fn batch_refresh_work_provider_links(
    db: State<'_, Database>,
    work_ids: Vec<String>,
    source: String,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<BatchWorkshopResult, AppError> {
    let source_kind = MetadataSource::from_str(&source)
        .ok_or_else(|| AppError::Validation(format!("Unknown source '{source}'")))?;
    let provider_defaults =
        crate::db::queries::provider_rules::list_field_defaults(db.read_pool()).await?;
    let mut updated = 0_u64;
    let mut skipped = 0_u64;
    let mut affected_work_ids = Vec::new();

    for work_id in unique_work_ids(work_ids) {
        let preferred_id =
            crate::db::queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
                .await?
                .unwrap_or(work_id);
        let did_refresh = refresh_provider_link_for_work(
            &db,
            &preferred_id,
            source_kind,
            &vndb,
            &bangumi,
            &dlsite,
            &provider_defaults,
        )
        .await?;
        if did_refresh {
            affected_work_ids.push(preferred_id);
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    let affected_work_ids = unique_work_ids(affected_work_ids);
    if !affected_work_ids.is_empty() {
        crate::db::queries::canonical::sync_work_ids(db.read_pool(), &affected_work_ids).await?;
    }

    Ok(BatchWorkshopResult { updated, skipped })
}

#[tauri::command]
pub async fn list_provider_field_defaults(
    db: State<'_, Database>,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    crate::db::queries::provider_rules::list_field_defaults(db.read_pool()).await
}

#[tauri::command]
pub async fn set_provider_field_default(
    db: State<'_, Database>,
    field: String,
    source: Option<String>,
) -> Result<(), AppError> {
    let normalized_field = normalize_preference_field(&field)
        .ok_or_else(|| AppError::Validation(format!("Unsupported field '{field}'")))?;
    match normalize_source_choice(source.as_deref())? {
        Some(source_name) => {
            crate::db::queries::provider_rules::set_field_default(
                db.read_pool(),
                normalized_field,
                source_name,
            )
            .await?;
        }
        None => {
            crate::db::queries::provider_rules::clear_field_default(
                db.read_pool(),
                normalized_field,
            )
            .await?;
        }
    }
    Ok(())
}

// ── Year-in-Review ──

#[derive(Serialize)]
pub struct YearInReview {
    pub year: i32,
    pub total_added: i64,
    pub total_completed: i64,
    pub total_hours_est: f64,
    pub top_brands: Vec<YirBrand>,
    pub monthly_breakdown: Vec<MonthlyCount>,
    pub favorite_tags: Vec<TagCount>,
}

#[derive(Serialize)]
pub struct YirBrand {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct MonthlyCount {
    pub month: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[tauri::command]
pub async fn get_year_in_review(
    db: State<'_, Database>,
    year: Option<i32>,
) -> Result<YearInReview, AppError> {
    let y = year.unwrap_or_else(|| {
        chrono::Utc::now()
            .format("%Y")
            .to_string()
            .parse()
            .unwrap_or(2025)
    });
    let pool = db.read_pool();
    let year_str = y.to_string();

    let (total_added,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM works WHERE created_at LIKE ? || '%'")
            .bind(&year_str)
            .fetch_one(pool)
            .await?;

    let (total_completed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM works WHERE library_status = 'completed' AND created_at LIKE ? || '%'",
    )
    .bind(&year_str)
    .fetch_one(pool)
    .await?;

    let brand_rows = sqlx::query(
        "SELECT developer as name, COUNT(*) as count FROM works \
         WHERE developer IS NOT NULL AND created_at LIKE ? || '%' \
         GROUP BY developer ORDER BY count DESC LIMIT 5",
    )
    .bind(&year_str)
    .fetch_all(pool)
    .await?;

    let top_brands: Vec<YirBrand> = brand_rows
        .iter()
        .map(|r| YirBrand {
            name: r.get("name"),
            count: r.get("count"),
        })
        .collect();

    let month_rows = sqlx::query(
        "SELECT SUBSTR(created_at, 6, 2) as month, COUNT(*) as count FROM works \
         WHERE created_at LIKE ? || '%' \
         GROUP BY month ORDER BY month",
    )
    .bind(&year_str)
    .fetch_all(pool)
    .await?;

    let monthly_breakdown: Vec<MonthlyCount> = month_rows
        .iter()
        .map(|r| MonthlyCount {
            month: r.get("month"),
            count: r.get("count"),
        })
        .collect();

    Ok(YearInReview {
        year: y,
        total_added,
        total_completed,
        total_hours_est: total_completed as f64 * 15.0,
        top_brands,
        monthly_breakdown,
        favorite_tags: Vec::new(),
    })
}

async fn ensure_work_exists(db: &Database, work_id: &str) -> Result<(), AppError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM works WHERE id = ?")
        .bind(work_id)
        .fetch_optional(db.read_pool())
        .await?;
    if row.is_none() {
        return Err(AppError::WorkNotFound(work_id.to_string()));
    }
    Ok(())
}

fn clear_linked_source(work: &mut crate::domain::work::Work, source: MetadataSource) {
    match source {
        MetadataSource::Vndb => work.vndb_id = None,
        MetadataSource::Bangumi => work.bangumi_id = None,
        MetadataSource::Dlsite => work.dlsite_id = None,
    }
    let source_key = source.as_str();
    work.field_sources.retain(|_, value| value != source_key);
    work.field_preferences
        .retain(|_, value| value != source_key);
}

async fn re_resolve_work(
    pool: &sqlx::SqlitePool,
    work: &mut crate::domain::work::Work,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    provider_defaults: &std::collections::HashMap<String, String>,
) -> Result<(), AppError> {
    let linked = provider::fetch_linked_records(work, vndb, bangumi, dlsite)
        .await
        .map_err(AppError::Internal)?;
    resolver::resolve_with_defaults(
        work,
        linked.0.as_ref().and_then(|record| record.as_vndb()),
        linked.1.as_ref().and_then(|record| record.as_bangumi()),
        linked.2.as_ref().and_then(|record| record.as_dlsite()),
        provider_defaults,
    );
    crate::db::queries::works::upsert_work(pool, work).await?;
    Ok(())
}

async fn refresh_provider_link_for_work(
    db: &Database,
    preferred_id: &str,
    source_kind: MetadataSource,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    provider_defaults: &std::collections::HashMap<String, String>,
) -> Result<bool, AppError> {
    let row = crate::db::queries::works::get_work_by_id(db.read_pool(), preferred_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(preferred_id.to_string()))?;
    let mut work = row.into_work();

    let linked = provider::fetch_linked_records_detailed(&work, vndb, bangumi, dlsite).await;
    let target = match source_kind {
        MetadataSource::Vndb => linked.vndb.clone(),
        MetadataSource::Bangumi => linked.bangumi.clone(),
        MetadataSource::Dlsite => linked.dlsite.clone(),
    };

    if target.state == ProviderLinkState::NotLinked {
        return Ok(false);
    }

    let mut vndb_record = linked.vndb.record.clone();
    let mut bangumi_record = linked.bangumi.record.clone();
    let mut dlsite_record = linked.dlsite.record.clone();

    match target.state {
        ProviderLinkState::Ready => {}
        ProviderLinkState::Missing => {
            clear_linked_source(&mut work, source_kind);
            match source_kind {
                MetadataSource::Vndb => vndb_record = None,
                MetadataSource::Bangumi => bangumi_record = None,
                MetadataSource::Dlsite => dlsite_record = None,
            }
        }
        ProviderLinkState::AuthError
        | ProviderLinkState::RateLimited
        | ProviderLinkState::TransientError => {
            return Err(AppError::Validation(
                target
                    .message
                    .clone()
                    .unwrap_or_else(|| "Provider refresh failed".to_string()),
            ));
        }
        ProviderLinkState::NotLinked => unreachable!(),
    }

    resolver::resolve_with_defaults(
        &mut work,
        vndb_record.as_ref().and_then(|record| record.as_vndb()),
        bangumi_record
            .as_ref()
            .and_then(|record| record.as_bangumi()),
        dlsite_record.as_ref().and_then(|record| record.as_dlsite()),
        provider_defaults,
    );
    work.enrichment_state =
        if vndb_record.is_some() || bangumi_record.is_some() || dlsite_record.is_some() {
            EnrichmentState::Matched
        } else {
            EnrichmentState::Unmatched
        };

    crate::db::queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
    sync_related_people(
        db,
        bangumi,
        preferred_id,
        bangumi_record
            .as_ref()
            .and_then(|record| record.as_bangumi()),
    )
    .await?;
    Ok(true)
}

fn normalize_preference_field(field: &str) -> Option<&'static str> {
    match field {
        "title" => Some("title"),
        "title_aliases" => Some("title_aliases"),
        "developer" => Some("developer"),
        "release_date" => Some("release_date"),
        "rating" => Some("rating"),
        "description" => Some("description"),
        "tags" => Some("tags"),
        "cover_path" | "cover_image" => Some("cover_path"),
        _ => None,
    }
}

fn normalize_source_choice(source: Option<&str>) -> Result<Option<&'static str>, AppError> {
    match source {
        None | Some("auto") => Ok(None),
        Some("vndb") => Ok(Some("vndb")),
        Some("bangumi") => Ok(Some("bangumi")),
        Some("dlsite") => Ok(Some("dlsite")),
        Some(other) => Err(AppError::Validation(format!(
            "Unsupported source preference '{}'",
            other
        ))),
    }
}

async fn sync_related_people(
    db: &Database,
    bangumi: &BangumiClient,
    work_id: &str,
    bangumi_record: Option<&crate::enrichment::bangumi::BangumiSubject>,
) -> Result<(), AppError> {
    if let Some(subject) = bangumi_record {
        let persons = bangumi
            .get_subject_persons(subject.id)
            .await
            .map_err(AppError::Internal)?;
        let characters = bangumi
            .get_subject_characters(subject.id)
            .await
            .map_err(AppError::Internal)?;
        let bundle = people::extract_bangumi_people(&persons, &characters);
        crate::db::queries::people::replace_for_work(
            db.read_pool(),
            work_id,
            &bundle.persons,
            &bundle.characters,
            &bundle.character_links,
            &bundle.credits,
        )
        .await?;
    } else {
        crate::db::queries::people::clear_for_work(db.read_pool(), work_id).await?;
    }
    Ok(())
}

async fn upsert_variant_override(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    work_id: &str,
    manual_group_key: &str,
    make_representative: bool,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO canonical_variant_overrides (work_id, manual_group_key, make_representative, updated_at) \
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(work_id) DO UPDATE SET \
         manual_group_key = excluded.manual_group_key, \
         make_representative = excluded.make_representative, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
    )
    .bind(work_id)
    .bind(manual_group_key)
    .bind(if make_representative { 1_i64 } else { 0_i64 })
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn canonical_group_key(work_id: &str) -> String {
    format!("manual:group:{work_id}")
}

fn split_group_key(work_id: &str) -> String {
    format!("manual:split:{work_id}")
}

fn unique_work_ids(work_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for work_id in work_ids {
        if seen.insert(work_id.clone()) {
            unique.push(work_id);
        }
    }
    unique
}

fn dedupe_diagnostics(items: Vec<WorkshopDiagnosticInput>) -> Vec<WorkshopDiagnosticInput> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for item in items {
        let key = format!("{}::{}", item.work_id, item.category);
        if seen.insert(key) {
            unique.push(item);
        }
    }
    unique
}
