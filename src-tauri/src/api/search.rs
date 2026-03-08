//! Search API — FTS5 trigram search (R9).

use tauri::State;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::WorkSummary;

/// Search works using full-text search.
#[tauri::command]
pub async fn search_works(
    db: State<'_, Database>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<WorkSummary>, AppError> {
    let limit = limit.unwrap_or(50).min(200);
    let rows = queries::search::search_works(db.read_pool(), &query, limit * 4).await?;
    let representative_by_work =
        queries::canonical::representative_work_map(db.read_pool()).await?;
    let canonical_by_id: std::collections::HashMap<String, WorkSummary> =
        queries::canonical::list_canonical_works(db.read_pool(), "title", false, None)
            .await?
            .into_iter()
            .map(|row| {
                let summary = row.into_summary();
                (summary.id.to_string(), summary)
            })
            .collect();

    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for row in rows {
        let work_id = row.id.clone();
        let representative_id = representative_by_work
            .get(&work_id)
            .cloned()
            .unwrap_or(work_id);
        if seen.insert(representative_id.clone()) {
            if let Some(summary) = canonical_by_id.get(&representative_id) {
                deduped.push(summary.clone());
            }
        }
        if deduped.len() as i64 >= limit {
            break;
        }
    }

    Ok(deduped)
}
