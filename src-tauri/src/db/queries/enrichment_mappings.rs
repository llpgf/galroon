//! Enrichment title-to-source mapping cache queries.

use sqlx::SqlitePool;

use crate::db::models::EnrichmentMappingRow;
use crate::domain::error::AppResult;

pub async fn find_mappings_for_title(
    pool: &SqlitePool,
    normalized_title: &str,
) -> AppResult<Vec<EnrichmentMappingRow>> {
    let rows = sqlx::query_as(
        r#"
        SELECT normalized_title, source, external_id, resolved_title, title_original,
               developer, rating, confidence, created_at, updated_at
        FROM enrichment_mappings
        WHERE normalized_title = ?1
        ORDER BY confidence DESC, updated_at DESC
        "#,
    )
    .bind(normalized_title)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn upsert_mapping(
    pool: &SqlitePool,
    normalized_title: &str,
    source: &str,
    external_id: &str,
    resolved_title: &str,
    title_original: Option<&str>,
    developer: Option<&str>,
    rating: Option<f64>,
    confidence: f64,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO enrichment_mappings (
            normalized_title, source, external_id, resolved_title, title_original,
            developer, rating, confidence, created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8, ?9, ?10
        )
        ON CONFLICT(normalized_title, source) DO UPDATE SET
            external_id = excluded.external_id,
            resolved_title = excluded.resolved_title,
            title_original = excluded.title_original,
            developer = excluded.developer,
            rating = excluded.rating,
            confidence = excluded.confidence,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(normalized_title)
    .bind(source)
    .bind(external_id)
    .bind(resolved_title)
    .bind(title_original)
    .bind(developer)
    .bind(rating)
    .bind(confidence)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}
