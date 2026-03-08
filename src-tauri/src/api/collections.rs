//! Collections + Playlists + Wishlist + Random Pick + Export/Import API.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::EnrichmentState;
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::cache;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::matcher::MatchVerdict;
use crate::enrichment::provider::{self, MetadataSource};
use crate::enrichment::query;
use crate::enrichment::resolver;
use crate::enrichment::search::SearchCandidate;
use crate::enrichment::vndb::VndbClient;
use crate::fs::metadata_io;

// ── Collection types ──

#[derive(Serialize, Deserialize, FromRow)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_smart: bool,
    pub smart_rule: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
}

#[derive(Serialize, FromRow)]
pub struct CollectionWork {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
}

#[derive(Serialize, FromRow)]
pub struct WishlistEntry {
    pub id: String,
    pub title: String,
    pub developer: Option<String>,
    pub vndb_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub notes: String,
    pub priority: i32,
    pub created_at: String,
}

#[derive(Serialize, FromRow)]
pub struct ActivityEntry {
    pub id: i64,
    pub action: String,
    pub target_id: Option<String>,
    pub target_type: Option<String>,
    pub detail: Option<String>,
    pub created_at: String,
}

// ── Collections CRUD ──

#[tauri::command]
pub async fn list_collections(db: State<'_, Database>) -> Result<Vec<Collection>, AppError> {
    let rows: Vec<Collection> = sqlx::query_as(
        "SELECT id, name, description, is_smart, smart_rule, sort_order, created_at \
         FROM collections ORDER BY sort_order, name",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn create_collection(
    db: State<'_, Database>,
    name: String,
    description: Option<String>,
    is_smart: Option<bool>,
    smart_rule: Option<String>,
) -> Result<Collection, AppError> {
    let id = Uuid::new_v4().to_string();
    let desc = description.unwrap_or_default();
    let smart = is_smart.unwrap_or(false);

    db.execute_write(
        "INSERT INTO collections (id, name, description, is_smart, smart_rule) VALUES (?1, ?2, ?3, ?4, ?5)"
            .to_string(),
        vec![
            serde_json::Value::String(id.clone()),
            serde_json::Value::String(name.clone()),
            serde_json::Value::String(desc.clone()),
            serde_json::Value::Bool(smart),
            smart_rule
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        ],
    )
    .await?;

    Ok(Collection {
        id,
        name,
        description: desc,
        is_smart: smart,
        smart_rule,
        sort_order: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn delete_collection(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    db.execute_write(
        "DELETE FROM collections WHERE id = ?1".to_string(),
        vec![serde_json::Value::String(id)],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn add_to_collection(
    db: State<'_, Database>,
    collection_id: String,
    work_id: String,
) -> Result<(), AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    db.execute_write(
        "INSERT OR IGNORE INTO collection_items (collection_id, work_id, position) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM collection_items WHERE collection_id = ?1))"
            .to_string(),
        vec![
            serde_json::Value::String(collection_id),
            serde_json::Value::String(work_id),
        ],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_from_collection(
    db: State<'_, Database>,
    collection_id: String,
    work_id: String,
) -> Result<(), AppError> {
    let variant_ids = queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;
    let mut removed = 0;
    for variant_id in variant_ids {
        removed += db
            .execute_write(
                "DELETE FROM collection_items WHERE collection_id = ?1 AND work_id = ?2"
                    .to_string(),
                vec![
                    serde_json::Value::String(collection_id.clone()),
                    serde_json::Value::String(variant_id),
                ],
            )
            .await?;
    }

    if removed == 0 {
        db.execute_write(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND work_id = ?2".to_string(),
            vec![
                serde_json::Value::String(collection_id),
                serde_json::Value::String(work_id),
            ],
        )
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_collection_works(
    db: State<'_, Database>,
    collection_id: String,
) -> Result<Vec<CollectionWork>, AppError> {
    let rows: Vec<CollectionWork> = sqlx::query_as(
        "SELECT
            COALESCE(cw.preferred_work_id, w.id) as id,
            COALESCE(cw.title, w.title) as title,
            COALESCE(cw.cover_path, w.cover_path) as cover_path,
            COALESCE(cw.developer, w.developer) as developer,
            COALESCE(cw.rating, w.rating) as rating
         FROM collection_items ci
         JOIN works w ON w.id = ci.work_id
         LEFT JOIN work_variants wv ON wv.work_id = ci.work_id
         LEFT JOIN canonical_works cw ON cw.canonical_key = wv.canonical_key
         WHERE ci.collection_id = ?
         GROUP BY COALESCE(cw.canonical_key, ci.work_id)
         ORDER BY MIN(ci.position)",
    )
    .bind(&collection_id)
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

// ── Wishlist ──

#[tauri::command]
pub async fn list_wishlist(db: State<'_, Database>) -> Result<Vec<WishlistEntry>, AppError> {
    let rows: Vec<WishlistEntry> = sqlx::query_as(
        "SELECT id, title, developer, vndb_id, dlsite_id, notes, priority, created_at \
         FROM wishlist ORDER BY priority DESC, title",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_wishlist(
    db: State<'_, Database>,
    title: String,
    developer: Option<String>,
    vndb_id: Option<String>,
    dlsite_id: Option<String>,
    notes: Option<String>,
    priority: Option<i32>,
) -> Result<WishlistEntry, AppError> {
    let id = Uuid::new_v4().to_string();
    let dev = developer.clone().unwrap_or_default();
    let n = notes.clone().unwrap_or_default();
    let p = priority.unwrap_or(0);

    db.execute_write(
        "INSERT INTO wishlist (id, title, developer, vndb_id, dlsite_id, notes, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
            .to_string(),
        vec![
            serde_json::Value::String(id.clone()),
            serde_json::Value::String(title.clone()),
            developer
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            vndb_id
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            dlsite_id
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
            serde_json::Value::String(n.clone()),
            serde_json::Value::Number(serde_json::Number::from(p as i64)),
        ],
    )
    .await?;

    Ok(WishlistEntry {
        id,
        title,
        developer: Some(dev),
        vndb_id,
        dlsite_id,
        notes: n,
        priority: p,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn remove_wishlist(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    db.execute_write(
        "DELETE FROM wishlist WHERE id = ?1".to_string(),
        vec![serde_json::Value::String(id)],
    )
    .await?;
    Ok(())
}

// ── Random Pick ──

#[tauri::command]
pub async fn random_pick(db: State<'_, Database>) -> Result<Option<CollectionWork>, AppError> {
    let row: Option<CollectionWork> = sqlx::query_as(
        "SELECT preferred_work_id as id, title, cover_path, developer, rating
         FROM canonical_works ORDER BY RANDOM() LIMIT 1",
    )
    .fetch_optional(db.read_pool())
    .await?;
    Ok(row)
}

// ── Activity Feed ──

#[tauri::command]
pub async fn get_activity_feed(
    db: State<'_, Database>,
    limit: Option<i64>,
) -> Result<Vec<ActivityEntry>, AppError> {
    let limit = limit.unwrap_or(50).min(200);
    let rows: Vec<ActivityEntry> = sqlx::query_as(
        "SELECT id, action, target_id, target_type, detail, created_at \
         FROM activity_log ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

// ── Export / Import ──

#[derive(Serialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: String,
    pub works_count: i64,
    pub collections: Vec<Collection>,
    pub wishlist: Vec<WishlistEntry>,
}

#[tauri::command]
pub async fn export_library(db: State<'_, Database>) -> Result<ExportData, AppError> {
    let pool = db.read_pool();

    let (works_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM canonical_works")
        .fetch_one(pool)
        .await?;

    let collections: Vec<Collection> = sqlx::query_as(
        "SELECT id, name, description, is_smart, smart_rule, sort_order, created_at FROM collections",
    )
    .fetch_all(pool)
    .await?;

    let wishlist: Vec<WishlistEntry> = sqlx::query_as(
        "SELECT id, title, developer, vndb_id, dlsite_id, notes, priority, created_at FROM wishlist",
    )
    .fetch_all(pool)
    .await?;

    Ok(ExportData {
        version: "0.5.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        works_count,
        collections,
        wishlist,
    })
}

// ── Smart Collection Evaluator ──

#[tauri::command]
pub async fn evaluate_smart_collection(
    db: State<'_, Database>,
    collection_id: String,
) -> Result<Vec<CollectionWork>, AppError> {
    let pool = db.read_pool();

    let (smart_rule,): (Option<String>,) =
        sqlx::query_as("SELECT smart_rule FROM collections WHERE id = ? AND is_smart = 1")
            .bind(&collection_id)
            .fetch_one(pool)
            .await?;

    let rule_json = smart_rule.ok_or_else(|| AppError::NotFound("No smart rule found".into()))?;
    let rule: SmartRule = serde_json::from_str(&rule_json)
        .map_err(|e| AppError::Validation(format!("Invalid smart rule JSON: {}", e)))?;

    let where_clause = build_where_clause(&rule);
    let query = format!(
        "SELECT preferred_work_id as id, title, cover_path, developer, rating FROM canonical_works WHERE {} ORDER BY title",
        where_clause
    );

    let rows: Vec<CollectionWork> = sqlx::query_as(&query).fetch_all(pool).await?;

    Ok(rows)
}

#[derive(Deserialize)]
struct SmartRule {
    operator: String,
    conditions: Vec<SmartCondition>,
}

#[derive(Deserialize)]
struct SmartCondition {
    field: String,
    op: String,
    value: String,
}

fn build_where_clause(rule: &SmartRule) -> String {
    let joiner = if rule.operator == "or" {
        " OR "
    } else {
        " AND "
    };

    let parts: Vec<String> = rule
        .conditions
        .iter()
        .map(|c| {
            let safe_val = c.value.replace('\'', "''");
            match c.op.as_str() {
                "eq" => format!("{} = '{}'", sanitize_field(&c.field), safe_val),
                "neq" => format!("{} != '{}'", sanitize_field(&c.field), safe_val),
                "gt" => format!("CAST({} AS REAL) > {}", sanitize_field(&c.field), safe_val),
                "gte" => format!("CAST({} AS REAL) >= {}", sanitize_field(&c.field), safe_val),
                "lt" => format!("CAST({} AS REAL) < {}", sanitize_field(&c.field), safe_val),
                "lte" => format!("CAST({} AS REAL) <= {}", sanitize_field(&c.field), safe_val),
                "contains" => format!("{} LIKE '%{}%'", sanitize_field(&c.field), safe_val),
                "starts" => format!("{} LIKE '{}%'", sanitize_field(&c.field), safe_val),
                "is_null" => format!("{} IS NULL", sanitize_field(&c.field)),
                "not_null" => format!("{} IS NOT NULL", sanitize_field(&c.field)),
                _ => "1=1".to_string(),
            }
        })
        .collect();

    if parts.is_empty() {
        "1=1".to_string()
    } else {
        parts.join(joiner)
    }
}

fn sanitize_field(field: &str) -> &str {
    match field {
        "title" | "developer" | "rating" | "release_date" | "description" | "library_status"
        | "enrichment_status" | "enrichment_state" | "vndb_id" | "dlsite_id" | "bangumi_id"
        | "folder_path" | "cover_path" | "tags" => field,
        _ => "title",
    }
}

// ── Drag-Drop Reorder ──

#[tauri::command]
pub async fn reorder_collection(
    db: State<'_, Database>,
    collection_id: String,
    work_ids: Vec<String>,
) -> Result<(), AppError> {
    for (i, wid) in work_ids.iter().enumerate() {
        db.execute_write(
            "UPDATE collection_items SET position = ?1 WHERE collection_id = ?2 AND work_id = ?3"
                .to_string(),
            vec![
                serde_json::Value::Number(serde_json::Number::from(i as i64)),
                serde_json::Value::String(collection_id.clone()),
                serde_json::Value::String(wid.clone()),
            ],
        )
        .await?;
    }
    Ok(())
}

// ── Multi-Source Match Pipeline ──

#[tauri::command]
pub async fn multi_source_match(
    db: State<'_, Database>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
    work_id: String,
) -> Result<String, AppError> {
    run_multi_source_match(
        db.inner(),
        vndb.inner(),
        bangumi.inner(),
        dlsite.inner(),
        &work_id,
    )
    .await
}

#[tauri::command]
pub async fn batch_multi_source_match(
    db: State<'_, Database>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<Vec<String>, AppError> {
    let pool = db.read_pool();

    let unmatched: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM works \
         WHERE enrichment_state IN ('pending', 'pending_review', 'unmatched') OR enrichment_state IS NULL \
         LIMIT 50",
    )
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();
    for (work_id,) in unmatched {
        let result = run_multi_source_match(
            db.inner(),
            vndb.inner(),
            bangumi.inner(),
            dlsite.inner(),
            &work_id,
        )
        .await?;
        results.push(format!("{}: {}", work_id, result));
    }

    Ok(results)
}

async fn run_multi_source_match(
    db: &Database,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    work_id: &str,
) -> Result<String, AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), work_id)
        .await?
        .unwrap_or_else(|| work_id.to_string());
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(work_id.clone()))?;
    let mut work = row.into_work();
    let title = work.title.clone();
    let folder_path = work.folder_path.to_string_lossy().to_string();

    let rj_match = regex::Regex::new(r"(?i)(RJ\d{6,8})")
        .unwrap()
        .captures(&title)
        .or_else(|| {
            regex::Regex::new(r"(?i)(RJ\d{6,8})")
                .unwrap()
                .captures(&folder_path)
        })
        .map(|cap| cap[1].to_uppercase());

    let mut query_input = query::build_query_input(&work);
    let (linked_vndb, linked_bangumi, linked_dlsite) =
        provider::fetch_linked_records(&work, vndb, bangumi, dlsite)
            .await
            .map_err(AppError::Internal)?;

    for record in [
        linked_vndb.as_ref(),
        linked_bangumi.as_ref(),
        linked_dlsite.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        query::extend_query_input(&mut query_input, record.search_titles());
    }

    let provider_defaults = queries::provider_rules::list_field_defaults(db.read_pool()).await?;

    if linked_vndb.is_some() || linked_bangumi.is_some() || linked_dlsite.is_some() {
        resolver::resolve_with_defaults(
            &mut work,
            linked_vndb.as_ref().and_then(|record| record.as_vndb()),
            linked_bangumi
                .as_ref()
                .and_then(|record| record.as_bangumi()),
            linked_dlsite.as_ref().and_then(|record| record.as_dlsite()),
            &provider_defaults,
        );
        work.enrichment_state = EnrichmentState::Matched;
        queries::works::upsert_work(db.read_pool(), &work)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to persist enriched work: {}", e)))?;
        metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;

        if let Some(record) = linked_vndb.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }
        if let Some(record) = linked_bangumi.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }
        if let Some(record) = linked_dlsite.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }

        let mut sources = Vec::new();
        if let Some(record) = linked_vndb.as_ref() {
            sources.push(format!("vndb:{}", record.id()));
        }
        if let Some(record) = linked_bangumi.as_ref() {
            sources.push(format!("bangumi:{}", record.id()));
        }
        if let Some(record) = linked_dlsite.as_ref() {
            sources.push(format!("dlsite:{}", record.id()));
        }
        if let Some(rj) = rj_match {
            sources.push(format!("dlsite:{}", rj));
        }

        return Ok(sources.join(" | "));
    }

    let candidates =
        cache::search_candidates(db.read_pool(), vndb, bangumi, dlsite, &query_input, 10).await;

    let vndb_best = best_candidate_for_source(&candidates, MetadataSource::Vndb);
    let bangumi_best = best_candidate_for_source(&candidates, MetadataSource::Bangumi);
    let dlsite_best = best_candidate_for_source(&candidates, MetadataSource::Dlsite);

    let has_auto = vndb_best
        .as_ref()
        .is_some_and(|candidate| candidate.verdict == MatchVerdict::AutoMatch)
        || bangumi_best
            .as_ref()
            .is_some_and(|candidate| candidate.verdict == MatchVerdict::AutoMatch)
        || dlsite_best
            .as_ref()
            .is_some_and(|candidate| candidate.verdict == MatchVerdict::AutoMatch);
    let has_pending = vndb_best
        .as_ref()
        .is_some_and(|candidate| candidate.verdict == MatchVerdict::PendingReview)
        || bangumi_best
            .as_ref()
            .is_some_and(|candidate| candidate.verdict == MatchVerdict::PendingReview)
        || dlsite_best
            .as_ref()
            .is_some_and(|candidate| candidate.verdict == MatchVerdict::PendingReview);

    if has_auto {
        let vndb_auto = vndb_best
            .as_ref()
            .filter(|candidate| candidate.verdict == MatchVerdict::AutoMatch);
        let bangumi_auto = bangumi_best
            .as_ref()
            .filter(|candidate| candidate.verdict == MatchVerdict::AutoMatch);
        let dlsite_auto = dlsite_best
            .as_ref()
            .filter(|candidate| candidate.verdict == MatchVerdict::AutoMatch);

        let vndb_record = if let Some(candidate) = vndb_auto {
            provider::fetch_record(MetadataSource::Vndb, &candidate.id, vndb, bangumi, dlsite)
                .await
                .map_err(AppError::VndbApi)?
                .or_else(|| candidate.record.clone())
        } else {
            None
        };
        let bangumi_record = if let Some(candidate) = bangumi_auto {
            provider::fetch_record(
                MetadataSource::Bangumi,
                &candidate.id,
                vndb,
                bangumi,
                dlsite,
            )
            .await
            .map_err(AppError::Internal)?
            .or_else(|| candidate.record.clone())
        } else {
            None
        };

        let dlsite_record = if let Some(candidate) = dlsite_auto {
            provider::fetch_record(MetadataSource::Dlsite, &candidate.id, vndb, bangumi, dlsite)
                .await
                .map_err(AppError::Internal)?
                .or_else(|| candidate.record.clone())
        } else {
            None
        };

        resolver::resolve_with_defaults(
            &mut work,
            vndb_record.as_ref().and_then(|record| record.as_vndb()),
            bangumi_record
                .as_ref()
                .and_then(|record| record.as_bangumi()),
            dlsite_record.as_ref().and_then(|record| record.as_dlsite()),
            &provider_defaults,
        );
        work.enrichment_state = EnrichmentState::Matched;
        queries::works::upsert_work(db.read_pool(), &work)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to persist enriched work: {}", e)))?;
        metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;

        if let Some(record) = vndb_record.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }
        if let Some(record) = bangumi_record.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }
        if let Some(record) = dlsite_record.as_ref() {
            cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
        }

        if let Some(rj) = &rj_match {
            persist_dlsite_match(db, &work_id, rj, true).await?;
        }

        let mut sources = Vec::new();
        if let Some(candidate) = vndb_auto {
            sources.push(format!("vndb:{}", candidate.id));
        }
        if let Some(candidate) = bangumi_auto {
            sources.push(format!("bangumi:{}", candidate.id));
        }
        if let Some(candidate) = dlsite_auto {
            sources.push(format!("dlsite:{}", candidate.id));
        }
        if let Some(rj) = rj_match {
            sources.push(format!("dlsite:{}", rj));
        }

        return Ok(sources.join(" | "));
    }

    if has_pending {
        let now = chrono::Utc::now().to_rfc3339();
        db.execute_write(
            "UPDATE works SET enrichment_state = 'pending_review', updated_at = ?1 WHERE id = ?2"
                .to_string(),
            vec![
                serde_json::Value::String(now),
                serde_json::Value::String(work_id.to_string()),
            ],
        )
        .await?;

        let mut sources = Vec::new();
        if let Some(candidate) = &vndb_best {
            sources.push(format!("pending:vndb:{}", candidate.id));
        }
        if let Some(candidate) = &bangumi_best {
            sources.push(format!("pending:bangumi:{}", candidate.id));
        }
        if let Some(candidate) = &dlsite_best {
            sources.push(format!("pending:dlsite:{}", candidate.id));
        }
        return Ok(sources.join(" | "));
    }

    if let Some(rj) = rj_match {
        persist_dlsite_match(db, &work_id, &rj, true).await?;
        return Ok(format!("dlsite:{}", rj));
    }

    let now = chrono::Utc::now().to_rfc3339();
    db.execute_write(
        "UPDATE works SET enrichment_state = 'unmatched', updated_at = ?1 WHERE id = ?2"
            .to_string(),
        vec![
            serde_json::Value::String(now),
            serde_json::Value::String(work_id.to_string()),
        ],
    )
    .await?;
    queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;

    Ok("none".to_string())
}

fn best_candidate_for_source(
    candidates: &[SearchCandidate],
    source: MetadataSource,
) -> Option<SearchCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.source == source)
        .max_by(|left, right| {
            left.score
                .partial_cmp(&right.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

async fn persist_dlsite_match(
    db: &Database,
    work_id: &str,
    rj_code: &str,
    mark_matched: bool,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let enrichment_state = if mark_matched {
        "matched"
    } else {
        "pending_review"
    };
    db.execute_write(
        "UPDATE works SET dlsite_id = ?1, enrichment_state = ?2, updated_at = ?3 WHERE id = ?4"
            .to_string(),
        vec![
            serde_json::Value::String(rj_code.to_string()),
            serde_json::Value::String(enrichment_state.to_string()),
            serde_json::Value::String(now),
            serde_json::Value::String(work_id.to_string()),
        ],
    )
    .await?;
    Ok(())
}
