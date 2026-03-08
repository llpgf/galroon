//! Duplicate detector — surfaces poster-level duplicate groups for review/merge.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub title: String,
    pub representative_id: String,
    pub representative_cover_path: Option<String>,
    pub variant_count: u32,
    pub review_flags: Vec<String>,
    pub entries: Vec<DuplicateEntry>,
}

#[derive(Serialize)]
pub struct DuplicateEntry {
    pub id: String,
    pub folder_path: String,
    pub title: String,
    pub developer: Option<String>,
    pub cover_path: Option<String>,
    pub enrichment_state: String,
    pub asset_count: i64,
    pub asset_types: Vec<String>,
    pub has_completion: bool,
    pub has_people: bool,
    pub is_representative: bool,
    pub manual_group_key: Option<String>,
    pub manual_representative: bool,
}

#[tauri::command]
pub async fn find_duplicates(db: State<'_, Database>) -> Result<Vec<DuplicateGroup>, AppError> {
    detect_duplicates(&db).await
}

async fn detect_duplicates(db: &Database) -> Result<Vec<DuplicateGroup>, AppError> {
    let pool = db.read_pool();
    let duplicate_groups = queries::canonical::duplicate_groups(pool).await?;

    let asset_counts = load_count_map(
        pool,
        "SELECT work_id, COUNT(*) as count FROM assets GROUP BY work_id",
    )
    .await?;
    let asset_types = load_asset_type_map(pool).await?;
    let people_counts = load_count_map(
        pool,
        "SELECT work_id, COUNT(*) as count FROM work_credits GROUP BY work_id",
    )
    .await?;
    let completion_ids = load_id_set(pool, "SELECT work_id FROM completion_tracking").await?;

    let mut groups = Vec::new();
    for group in duplicate_groups {
        let canonical_key = group.canonical_key.clone();
        let representative_id = group.preferred_work_id.clone();
        let rows = sqlx::query(
            "SELECT w.id, w.folder_path, w.title, w.developer, w.cover_path, w.enrichment_state,
                    wv.is_representative, cvo.manual_group_key, cvo.make_representative
             FROM work_variants wv
             JOIN works w ON w.id = wv.work_id
             LEFT JOIN canonical_variant_overrides cvo ON cvo.work_id = w.id
             WHERE wv.canonical_key = ?
             ORDER BY wv.is_representative DESC, w.updated_at DESC",
        )
        .bind(&canonical_key)
        .fetch_all(pool)
        .await?;

        let mut entries: Vec<DuplicateEntry> = rows
            .into_iter()
            .map(|row| {
                let id: String = row.get("id");
                DuplicateEntry {
                    id: id.clone(),
                    folder_path: row.get("folder_path"),
                    title: row.get("title"),
                    developer: row.get("developer"),
                    cover_path: row.get("cover_path"),
                    enrichment_state: row.get("enrichment_state"),
                    asset_count: asset_counts.get(&id).copied().unwrap_or_default(),
                    asset_types: asset_types.get(&id).cloned().unwrap_or_default(),
                    has_completion: completion_ids.contains(&id),
                    has_people: people_counts.get(&id).copied().unwrap_or_default() > 0,
                    is_representative: row.get::<i64, _>("is_representative") == 1,
                    manual_group_key: row.get("manual_group_key"),
                    manual_representative: row
                        .get::<Option<i64>, _>("make_representative")
                        .unwrap_or_default()
                        == 1,
                }
            })
            .collect();

        entries.sort_by(|left, right| {
            right
                .is_representative
                .cmp(&left.is_representative)
                .then_with(|| right.manual_representative.cmp(&left.manual_representative))
                .then_with(|| right.asset_count.cmp(&left.asset_count))
                .then_with(|| left.folder_path.cmp(&right.folder_path))
        });

        let review_flags = build_review_flags(&entries);

        groups.push(DuplicateGroup {
            title: group.title,
            representative_id: representative_id.clone(),
            representative_cover_path: group.cover_path,
            variant_count: group.variant_count as u32,
            review_flags,
            entries,
        });
    }

    groups.sort_by(|left, right| {
        right
            .variant_count
            .cmp(&left.variant_count)
            .then_with(|| left.title.cmp(&right.title))
    });

    Ok(groups)
}

fn build_review_flags(entries: &[DuplicateEntry]) -> Vec<String> {
    let mut flags = Vec::new();

    let title_count = entries
        .iter()
        .map(|entry| entry.title.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    if title_count > 1 {
        flags.push("title-conflict".to_string());
    }

    let developer_count = entries
        .iter()
        .filter_map(|entry| entry.developer.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    if developer_count > 1 {
        flags.push("developer-conflict".to_string());
    }

    let asset_mix_count = entries
        .iter()
        .filter_map(|entry| entry.asset_types.first())
        .map(|value| value.as_str())
        .collect::<HashSet<_>>()
        .len();
    if asset_mix_count > 1 {
        flags.push("mixed-assets".to_string());
    }

    if entries.iter().any(|entry| entry.manual_group_key.is_some()) {
        flags.push("manual-review".to_string());
    }

    if entries
        .iter()
        .any(|entry| entry.enrichment_state != "matched")
    {
        flags.push("needs-enrichment".to_string());
    }

    flags
}

async fn load_count_map(
    pool: &sqlx::SqlitePool,
    sql: &str,
) -> Result<HashMap<String, i64>, AppError> {
    let rows = sqlx::query(sql).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("work_id"), row.get("count")))
        .collect())
}

async fn load_id_set(pool: &sqlx::SqlitePool, sql: &str) -> Result<HashSet<String>, AppError> {
    let rows = sqlx::query(sql).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|row| row.get("work_id")).collect())
}

async fn load_asset_type_map(
    pool: &sqlx::SqlitePool,
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let rows = sqlx::query(
        "SELECT work_id, asset_type, COUNT(*) as count FROM assets GROUP BY work_id, asset_type ORDER BY work_id, count DESC, asset_type"
    )
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<String, Vec<(String, i64)>> = HashMap::new();
    for row in rows {
        let work_id: String = row.get("work_id");
        let asset_type: String = row.get("asset_type");
        let count: i64 = row.get("count");
        map.entry(work_id).or_default().push((asset_type, count));
    }

    Ok(map
        .into_iter()
        .map(|(work_id, mut values)| {
            values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            (
                work_id,
                values
                    .into_iter()
                    .map(|(asset_type, _)| asset_type)
                    .collect(),
            )
        })
        .collect())
}
