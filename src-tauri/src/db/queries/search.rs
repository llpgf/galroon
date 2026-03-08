//! Full-text search queries (R9).

use sqlx::SqlitePool;

use crate::db::models::WorkRow;
use crate::domain::error::AppResult;

/// Search works using FTS5 trigram index.
///
/// Supports Japanese/CJK substring matching via trigram tokenizer.
pub async fn search_works(pool: &SqlitePool, query: &str, limit: i64) -> AppResult<Vec<WorkRow>> {
    // Escape special FTS5 characters
    let escaped = query.replace('"', "\"\"");

    let rows: Vec<WorkRow> = sqlx::query_as(
        r#"
        SELECT w.*
        FROM works w
        JOIN works_fts fts ON w.rowid = fts.rowid
        WHERE works_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
        "#,
    )
    .bind(format!("\"{}\"", escaped))
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
