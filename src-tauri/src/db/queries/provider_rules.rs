use std::collections::HashMap;

use sqlx::{Row, SqlitePool};

use crate::domain::error::AppResult;

pub async fn list_field_defaults(pool: &SqlitePool) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query("SELECT field, source FROM provider_field_defaults")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("field"), row.get("source")))
        .collect())
}

pub async fn set_field_default(pool: &SqlitePool, field: &str, source: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO provider_field_defaults (field, source, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(field) DO UPDATE SET source = excluded.source, updated_at = datetime('now')",
    )
    .bind(field)
    .bind(source)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_field_default(pool: &SqlitePool, field: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM provider_field_defaults WHERE field = ?")
        .bind(field)
        .execute(pool)
        .await?;
    Ok(())
}
