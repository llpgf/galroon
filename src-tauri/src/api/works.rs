//! Works API — Tauri IPC commands for work CRUD.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::Row;
use tauri::State;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::{FieldSource, WorkSummary};
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::provider;
use crate::enrichment::resolver;
use crate::enrichment::vndb::VndbClient;
use crate::fs::metadata_io;
use crate::scanner::ingest;

#[derive(Serialize)]
pub struct ListWorksResponse {
    pub data: Vec<WorkSummary>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(Serialize)]
pub struct WorkCreditSummary {
    pub person_id: String,
    pub name: String,
    pub name_original: Option<String>,
    pub image_url: Option<String>,
    pub description: Option<String>,
    pub role: String,
    pub character_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct WorkVariantSummary {
    pub id: String,
    pub folder_path: String,
    pub title: String,
    pub developer: Option<String>,
    pub enrichment_state: String,
    pub asset_count: i64,
    pub asset_types: Vec<String>,
    pub has_completion: bool,
    pub is_representative: bool,
}

#[derive(Serialize)]
pub struct WorkAssetGroupVariant {
    pub work_id: String,
    pub folder_path: String,
    pub asset_count: i64,
}

#[derive(Serialize)]
pub struct WorkAssetGroupSummary {
    pub asset_type: String,
    pub relation_role: String,
    pub parent_asset_type: Option<String>,
    pub asset_count: i64,
    pub variant_count: i64,
    pub representative_work_id: Option<String>,
    pub representative_path: Option<String>,
    pub variants: Vec<WorkAssetGroupVariant>,
}

#[tauri::command]
pub async fn list_works(
    db: State<'_, Database>,
    page: Option<i64>,
    size: Option<i64>,
    sort_by: Option<String>,
    descending: Option<bool>,
    asset_type: Option<String>,
) -> Result<ListWorksResponse, AppError> {
    let page = page.unwrap_or(1).max(1);
    let size = size.unwrap_or(50).min(200);
    let offset = (page - 1) * size;
    let sort = sort_by.as_deref().unwrap_or("title");
    let desc = descending.unwrap_or(false);

    let rows =
        queries::canonical::list_canonical_works(db.read_pool(), sort, desc, asset_type.as_deref())
            .await?;
    let total = rows.len() as i64;
    let data: Vec<WorkSummary> = rows
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .map(|row| row.into_summary())
        .collect();

    Ok(ListWorksResponse {
        data,
        total,
        page,
        size,
    })
}

#[tauri::command]
pub async fn get_work(
    db: State<'_, Database>,
    id: String,
) -> Result<Option<serde_json::Value>, AppError> {
    let preferred_id = queries::canonical::get_preferred_work_id(db.read_pool(), &id)
        .await?
        .unwrap_or(id);
    let row = queries::works::get_work_by_id(db.read_pool(), &preferred_id).await?;
    match row {
        Some(r) => Ok(Some(serde_json::to_value(r.into_work())?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn list_work_credits(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Vec<WorkCreditSummary>, AppError> {
    let variant_ids = queries::canonical::list_variant_ids(db.read_pool(), &work_id).await?;

    let mut credits = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for variant_id in variant_ids {
        let rows = sqlx::query(
            "SELECT p.id as person_id, p.name, p.name_original, p.image_url, p.description, \
             wc.role, wc.character_name, wc.notes \
             FROM work_credits wc \
             JOIN persons p ON p.id = wc.person_id \
             WHERE wc.work_id = ? \
             ORDER BY wc.role, p.name",
        )
        .bind(&variant_id)
        .fetch_all(db.read_pool())
        .await?;

        for row in rows {
            let summary = WorkCreditSummary {
                person_id: row.get("person_id"),
                name: row.get("name"),
                name_original: row.get("name_original"),
                image_url: row.get("image_url"),
                description: row.get("description"),
                role: row.get("role"),
                character_name: row.get("character_name"),
                notes: row.get("notes"),
            };

            let key = (
                summary.person_id.clone(),
                summary.role.clone(),
                summary.character_name.clone(),
            );
            if seen.insert(key) {
                credits.push(summary);
            }
        }
    }

    Ok(credits)
}

#[tauri::command]
pub async fn list_work_variants(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Vec<WorkVariantSummary>, AppError> {
    let preferred_work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let variant_ids =
        queries::canonical::list_variant_ids(db.read_pool(), &preferred_work_id).await?;

    let completion_rows = sqlx::query("SELECT work_id FROM completion_tracking")
        .fetch_all(db.read_pool())
        .await?;
    let completion_ids: std::collections::HashSet<String> = completion_rows
        .into_iter()
        .map(|row| row.get("work_id"))
        .collect();

    let mut variants = Vec::new();
    for variant_id in variant_ids {
        if let Some(row) = queries::works::get_work_by_id(db.read_pool(), &variant_id).await? {
            let work = row.into_work();
            let asset_rows = sqlx::query(
                "SELECT asset_type, COUNT(*) as count FROM assets WHERE work_id = ? GROUP BY asset_type ORDER BY count DESC, asset_type"
            )
            .bind(&variant_id)
            .fetch_all(db.read_pool())
            .await?;

            let asset_count = asset_rows
                .iter()
                .map(|row| row.get::<i64, _>("count"))
                .sum();
            let asset_types = asset_rows
                .into_iter()
                .map(|row| row.get::<String, _>("asset_type"))
                .collect();

            variants.push(WorkVariantSummary {
                id: variant_id.clone(),
                folder_path: work.folder_path.to_string_lossy().to_string(),
                title: work.title,
                developer: work.developer,
                enrichment_state: serde_json::to_string(&work.enrichment_state)
                    .unwrap_or_else(|_| "unmatched".to_string())
                    .trim_matches('"')
                    .to_string(),
                asset_count,
                asset_types,
                has_completion: completion_ids.contains(&variant_id),
                is_representative: variant_id == preferred_work_id,
            });
        }
    }

    Ok(variants)
}

#[tauri::command]
pub async fn list_work_asset_groups(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Vec<WorkAssetGroupSummary>, AppError> {
    let preferred_work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let canonical_key = queries::canonical::get_canonical_key(db.read_pool(), &preferred_work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(preferred_work_id.clone()))?;

    let groups = queries::canonical::list_asset_groups(db.read_pool(), &preferred_work_id).await?;
    let mut summaries = Vec::new();

    for group in groups {
        let rows = sqlx::query(
            "SELECT w.id as work_id, w.folder_path, COUNT(*) as asset_count
             FROM assets a
             JOIN works w ON w.id = a.work_id
             JOIN work_variants wv ON wv.work_id = w.id
             WHERE wv.canonical_key = ?1 AND a.asset_type = ?2
             GROUP BY w.id, w.folder_path
             ORDER BY asset_count DESC, w.folder_path",
        )
        .bind(&canonical_key)
        .bind(&group.asset_type)
        .fetch_all(db.read_pool())
        .await?;

        summaries.push(WorkAssetGroupSummary {
            asset_type: group.asset_type,
            relation_role: group.relation_role,
            parent_asset_type: group.parent_asset_type,
            asset_count: group.asset_count,
            variant_count: group.variant_count,
            representative_work_id: group.representative_work_id,
            representative_path: group.representative_path,
            variants: rows
                .into_iter()
                .map(|row| WorkAssetGroupVariant {
                    work_id: row.get("work_id"),
                    folder_path: row.get("folder_path"),
                    asset_count: row.get("asset_count"),
                })
                .collect(),
        });
    }

    Ok(summaries)
}

#[tauri::command]
pub async fn update_work_field(
    db: State<'_, Database>,
    id: String,
    field: String,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let allowed_fields = [
        "title",
        "title_aliases",
        "developer",
        "publisher",
        "release_date",
        "description",
        "cover_path",
        "library_status",
        "user_tags",
        "rating",
    ];

    if !allowed_fields.contains(&field.as_str()) {
        return Err(AppError::Internal(format!(
            "Field '{}' cannot be updated",
            field
        )));
    }

    let preferred_id = queries::canonical::get_preferred_work_id(db.read_pool(), &id)
        .await?
        .unwrap_or(id.clone());
    let row = queries::works::get_work_by_id(db.read_pool(), &preferred_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(preferred_id.clone()))?;
    let mut work = row.into_work();

    match field.as_str() {
        "title" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation("Title cannot be empty".to_string()));
            };
            work.title = text.to_string();
            work.title_source = FieldSource::UserOverride;
            work.user_overrides.insert(
                "title".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("title".to_string(), "user_override".to_string());
        }
        "title_aliases" => {
            let Some(values) = value.as_array() else {
                return Err(AppError::Validation(
                    "title_aliases must be an array".to_string(),
                ));
            };
            work.title_aliases = values
                .iter()
                .filter_map(|entry| entry.as_str().map(|value| value.trim().to_string()))
                .filter(|value| !value.is_empty())
                .collect();
            work.user_overrides.insert(
                "title_aliases".to_string(),
                serde_json::Value::Array(
                    work.title_aliases
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
            work.field_sources
                .insert("title_aliases".to_string(), "user_override".to_string());
        }
        "developer" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation(
                    "Developer cannot be empty".to_string(),
                ));
            };
            work.developer = Some(text.to_string());
            work.user_overrides.insert(
                "developer".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("developer".to_string(), "user_override".to_string());
        }
        "publisher" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation(
                    "Publisher cannot be empty".to_string(),
                ));
            };
            work.publisher = Some(text.to_string());
            work.user_overrides.insert(
                "publisher".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("publisher".to_string(), "user_override".to_string());
        }
        "release_date" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation(
                    "Release date cannot be empty".to_string(),
                ));
            };
            let parsed = NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .map_err(|_| AppError::Validation("Release date must be YYYY-MM-DD".to_string()))?;
            work.release_date = Some(parsed);
            work.user_overrides.insert(
                "release_date".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("release_date".to_string(), "user_override".to_string());
        }
        "description" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation(
                    "Description cannot be empty".to_string(),
                ));
            };
            work.description = Some(text.to_string());
            work.user_overrides.insert(
                "description".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("description".to_string(), "user_override".to_string());
        }
        "cover_path" => {
            let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                return Err(AppError::Validation(
                    "Cover path cannot be empty".to_string(),
                ));
            };
            work.cover_path = Some(text.to_string());
            work.user_overrides.insert(
                "cover_path".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("cover_path".to_string(), "user_override".to_string());
        }
        "library_status" => {
            let Some(text) = value.as_str() else {
                return Err(AppError::Validation(
                    "library_status must be a string".to_string(),
                ));
            };
            work.library_status = serde_json::from_str(&format!("\"{}\"", text))
                .map_err(|_| AppError::Validation("Invalid library_status".to_string()))?;
            work.user_overrides.insert(
                "library_status".to_string(),
                serde_json::Value::String(text.to_string()),
            );
            work.field_sources
                .insert("library_status".to_string(), "user_override".to_string());
        }
        "user_tags" => {
            let Some(values) = value.as_array() else {
                return Err(AppError::Validation(
                    "user_tags must be an array".to_string(),
                ));
            };
            work.user_tags = values
                .iter()
                .filter_map(|entry| entry.as_str().map(|value| value.trim().to_string()))
                .filter(|value| !value.is_empty())
                .collect();
            work.user_overrides.insert(
                "user_tags".to_string(),
                serde_json::Value::Array(
                    work.user_tags
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        "rating" => {
            let Some(number) = value.as_f64() else {
                return Err(AppError::Validation("rating must be numeric".to_string()));
            };
            work.rating = Some(number);
            work.user_overrides
                .insert("rating".to_string(), serde_json::json!(number));
            work.field_sources
                .insert("rating".to_string(), "user_override".to_string());
        }
        _ => {}
    }

    queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;

    queries::canonical::sync_work_ids(db.read_pool(), &[preferred_id]).await?;

    Ok(())
}

#[tauri::command]
pub async fn reset_work_field_override(
    db: State<'_, Database>,
    id: String,
    field: String,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<(), AppError> {
    let resettable_fields = [
        "title",
        "title_aliases",
        "developer",
        "publisher",
        "release_date",
        "description",
        "cover_path",
        "rating",
    ];
    if !resettable_fields.contains(&field.as_str()) {
        return Err(AppError::Validation(format!(
            "Field '{}' cannot be reset",
            field
        )));
    }

    let preferred_id = queries::canonical::get_preferred_work_id(db.read_pool(), &id)
        .await?
        .unwrap_or(id);
    let row = queries::works::get_work_by_id(db.read_pool(), &preferred_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(preferred_id.clone()))?;
    let mut work = row.into_work();

    work.user_overrides.remove(&field);
    work.field_sources.remove(&field);

    match field.as_str() {
        "title" => {
            let folder_name = work
                .folder_path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| work.title.clone());
            work.title = ingest::extract_title(&folder_name);
            work.title_aliases.clear();
            work.title_source = FieldSource::Filesystem;
            work.field_sources.remove("title_aliases");
            work.user_overrides.remove("title_aliases");
        }
        "title_aliases" => work.title_aliases.clear(),
        "developer" => work.developer = None,
        "publisher" => work.publisher = None,
        "release_date" => work.release_date = None,
        "description" => work.description = None,
        "cover_path" => work.cover_path = None,
        "rating" => {
            work.rating = None;
            work.vote_count = None;
        }
        _ => {}
    }

    let linked = provider::fetch_linked_records(&work, &vndb, &bangumi, &dlsite)
        .await
        .map_err(AppError::Internal)?;
    let provider_defaults = queries::provider_rules::list_field_defaults(db.read_pool()).await?;
    resolver::resolve_with_defaults(
        &mut work,
        linked.0.as_ref().and_then(|record| record.as_vndb()),
        linked.1.as_ref().and_then(|record| record.as_bangumi()),
        linked.2.as_ref().and_then(|record| record.as_dlsite()),
        &provider_defaults,
    );

    queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
    queries::canonical::sync_work_ids(db.read_pool(), &[preferred_id]).await?;
    Ok(())
}
