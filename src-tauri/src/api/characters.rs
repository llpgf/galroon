//! Characters API — Tauri IPC commands for character CRUD.

use std::collections::{HashMap, HashSet};

use tauri::State;

use crate::db::queries;
use crate::db::queries::characters::CharacterRow;
use crate::db::Database;
use crate::domain::error::AppError;

/// List characters for a work.
#[tauri::command]
pub async fn list_characters(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Vec<CharacterRow>, AppError> {
    let variant_ids = queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;

    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for variant_id in variant_ids {
        for mut row in queries::characters::list_for_work(db.read_pool(), &variant_id).await? {
            row.work_id = Some(work_id.clone());
            if seen.insert(row.id.clone()) {
                merged.push(row);
            }
        }
    }

    merged.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(merged)
}

/// Get a single character by ID.
#[tauri::command]
pub async fn get_character(
    db: State<'_, Database>,
    id: String,
) -> Result<Option<CharacterRow>, AppError> {
    let row = queries::characters::get_by_id(db.read_pool(), &id).await?;
    Ok(row)
}

/// Search characters by name.
#[tauri::command]
pub async fn search_characters(
    db: State<'_, Database>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<CharacterRow>, AppError> {
    let limit = limit.unwrap_or(50).min(200);
    let rows = queries::characters::search_by_name(db.read_pool(), &query, limit * 4).await?;
    let representative_by_work: HashMap<String, String> =
        queries::canonical::representative_work_map(db.read_pool()).await?;
    let canonical_by_id: HashMap<String, String> =
        queries::canonical::list_canonical_works(db.read_pool(), "title", false, None)
            .await?
            .into_iter()
            .map(|row| (row.id.clone(), row.title))
            .collect();

    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    let mut deduped = Vec::new();
    for mut row in rows {
        if let Some(work_id) = row.work_id.clone() {
            if let Some(representative_id) = representative_by_work.get(&work_id) {
                row.work_id = Some(representative_id.clone());
                row.work_title = canonical_by_id.get(representative_id).cloned();
            }
        }

        let key = (row.id.clone(), row.work_id.clone());
        if seen.insert(key) {
            deduped.push(row);
        }
        if deduped.len() as i64 >= limit {
            break;
        }
    }

    Ok(deduped)
}
