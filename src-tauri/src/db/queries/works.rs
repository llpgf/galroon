//! Work CRUD queries.

use sqlx::SqlitePool;

use crate::db::models::{FolderMtimeRow, MetadataCheckRow, WorkRow, WorkSummaryRow};
use crate::domain::error::AppResult;
use crate::domain::work::Work;

/// Insert or update a work in the database.
pub async fn upsert_work(pool: &SqlitePool, work: &Work) -> AppResult<()> {
    let tags_json = serde_json::to_string(&work.tags)?;
    let user_tags_json = serde_json::to_string(&work.user_tags)?;
    let field_sources_json = serde_json::to_string(&work.field_sources)?;
    let field_preferences_json = serde_json::to_string(&work.field_preferences)?;
    let user_overrides_json = serde_json::to_string(&work.user_overrides)?;
    let aliases_json = serde_json::to_string(&work.title_aliases)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO works (
            id, folder_path, title, title_original, title_aliases,
            developer, publisher, release_date, rating, vote_count,
            description, cover_path, tags, user_tags, library_status,
            field_sources, field_preferences, user_overrides,
            vndb_id, bangumi_id, dlsite_id, enrichment_state, title_source,
            folder_mtime, metadata_mtime, metadata_hash, content_signature,
            created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23,
            ?24, ?25, ?26, ?27,
            ?28, ?29
        )
        ON CONFLICT(folder_path) DO UPDATE SET
            title = excluded.title,
            title_original = excluded.title_original,
            title_aliases = excluded.title_aliases,
            developer = excluded.developer,
            publisher = excluded.publisher,
            release_date = excluded.release_date,
            rating = excluded.rating,
            vote_count = excluded.vote_count,
            description = excluded.description,
            cover_path = excluded.cover_path,
            tags = excluded.tags,
            user_tags = excluded.user_tags,
            field_sources = excluded.field_sources,
            field_preferences = excluded.field_preferences,
            user_overrides = excluded.user_overrides,
            library_status = excluded.library_status,
            vndb_id = excluded.vndb_id,
            bangumi_id = excluded.bangumi_id,
            dlsite_id = excluded.dlsite_id,
            enrichment_state = excluded.enrichment_state,
            title_source = excluded.title_source,
            folder_mtime = excluded.folder_mtime,
            metadata_mtime = excluded.metadata_mtime,
            metadata_hash = excluded.metadata_hash,
            content_signature = excluded.content_signature,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(work.id.to_string())
    .bind(work.folder_path.to_string_lossy().to_string())
    .bind(&work.title)
    .bind(&work.title_original)
    .bind(&aliases_json)
    .bind(&work.developer)
    .bind(&work.publisher)
    .bind(work.release_date.map(|d| d.to_string()))
    .bind(work.rating)
    .bind(work.vote_count.map(|v| v as i64))
    .bind(&work.description)
    .bind(&work.cover_path)
    .bind(&tags_json)
    .bind(&user_tags_json)
    .bind(
        serde_json::to_string(&work.library_status)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(&field_sources_json)
    .bind(&field_preferences_json)
    .bind(&user_overrides_json)
    .bind(&work.vndb_id)
    .bind(&work.bangumi_id)
    .bind(&work.dlsite_id)
    .bind(
        serde_json::to_string(&work.enrichment_state)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(
        serde_json::to_string(&work.title_source)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(work.folder_mtime)
    .bind(work.metadata_mtime)
    .bind(&work.metadata_hash)
    .bind(&work.content_signature)
    .bind(&work.created_at.to_rfc3339())
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn move_work_and_refresh(
    pool: &SqlitePool,
    work: &Work,
    old_path: &str,
) -> AppResult<()> {
    let tags_json = serde_json::to_string(&work.tags)?;
    let user_tags_json = serde_json::to_string(&work.user_tags)?;
    let field_sources_json = serde_json::to_string(&work.field_sources)?;
    let field_preferences_json = serde_json::to_string(&work.field_preferences)?;
    let user_overrides_json = serde_json::to_string(&work.user_overrides)?;
    let aliases_json = serde_json::to_string(&work.title_aliases)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        UPDATE works SET
            folder_path = ?1,
            title = ?2,
            title_original = ?3,
            title_aliases = ?4,
            developer = ?5,
            publisher = ?6,
            release_date = ?7,
            rating = ?8,
            vote_count = ?9,
            description = ?10,
            cover_path = ?11,
            tags = ?12,
            user_tags = ?13,
            field_sources = ?14,
            field_preferences = ?15,
            user_overrides = ?16,
            library_status = ?17,
            vndb_id = ?18,
            bangumi_id = ?19,
            dlsite_id = ?20,
            enrichment_state = ?21,
            title_source = ?22,
            folder_mtime = ?23,
            metadata_mtime = ?24,
            metadata_hash = ?25,
            content_signature = ?26,
            updated_at = ?27
        WHERE id = ?28 OR folder_path = ?29
        "#,
    )
    .bind(work.folder_path.to_string_lossy().to_string())
    .bind(&work.title)
    .bind(&work.title_original)
    .bind(&aliases_json)
    .bind(&work.developer)
    .bind(&work.publisher)
    .bind(work.release_date.map(|d| d.to_string()))
    .bind(work.rating)
    .bind(work.vote_count.map(|v| v as i64))
    .bind(&work.description)
    .bind(&work.cover_path)
    .bind(&tags_json)
    .bind(&user_tags_json)
    .bind(&field_sources_json)
    .bind(&field_preferences_json)
    .bind(&user_overrides_json)
    .bind(
        serde_json::to_string(&work.library_status)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(&work.vndb_id)
    .bind(&work.bangumi_id)
    .bind(&work.dlsite_id)
    .bind(
        serde_json::to_string(&work.enrichment_state)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(
        serde_json::to_string(&work.title_source)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(work.folder_mtime)
    .bind(work.metadata_mtime)
    .bind(&work.metadata_hash)
    .bind(&work.content_signature)
    .bind(&now)
    .bind(work.id.to_string())
    .bind(old_path)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_works(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
    sort_by: &str,
    descending: bool,
) -> AppResult<(Vec<WorkSummaryRow>, i64)> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM works")
        .fetch_one(pool)
        .await?;

    let sort_col = match sort_by {
        "title" => "title",
        "developer" => "developer",
        "rating" => "rating",
        "release_date" => "release_date",
        "created_at" => "created_at",
        "updated_at" => "updated_at",
        _ => "title",
    };

    let dir = if descending { "DESC" } else { "ASC" };

    let query = format!(
        r#"
        SELECT id, title, cover_path, developer, rating,
               library_status, enrichment_state, tags, release_date,
               vndb_id, bangumi_id, dlsite_id, 1 as variant_count
        FROM works
        ORDER BY {} {} NULLS LAST
        LIMIT ? OFFSET ?
        "#,
        sort_col, dir
    );

    let rows: Vec<WorkSummaryRow> = sqlx::query_as(&query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((rows, total.0))
}

pub async fn list_all_works_sorted(
    pool: &SqlitePool,
    sort_by: &str,
    descending: bool,
) -> AppResult<Vec<WorkRow>> {
    let sort_col = match sort_by {
        "title" => "title",
        "developer" => "developer",
        "rating" => "rating",
        "release_date" => "release_date",
        "created_at" => "created_at",
        "updated_at" => "updated_at",
        _ => "title",
    };
    let dir = if descending { "DESC" } else { "ASC" };
    let query = format!(
        "SELECT * FROM works ORDER BY {} {} NULLS LAST",
        sort_col, dir
    );
    let rows: Vec<WorkRow> = sqlx::query_as(&query).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn get_work_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<WorkRow>> {
    let row: Option<WorkRow> = sqlx::query_as("SELECT * FROM works WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_work_by_path(pool: &SqlitePool, path: &str) -> AppResult<Option<WorkRow>> {
    let row: Option<WorkRow> = sqlx::query_as("SELECT * FROM works WHERE folder_path = ?")
        .bind(path)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_all_folder_mtimes(pool: &SqlitePool) -> AppResult<Vec<FolderMtimeRow>> {
    let rows: Vec<FolderMtimeRow> = sqlx::query_as("SELECT folder_path, folder_mtime FROM works")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn get_all_metadata_checks(pool: &SqlitePool) -> AppResult<Vec<MetadataCheckRow>> {
    let rows: Vec<MetadataCheckRow> =
        sqlx::query_as("SELECT folder_path, metadata_mtime, metadata_hash FROM works")
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

pub async fn delete_work_by_path(pool: &SqlitePool, path: &str) -> AppResult<u64> {
    let result = sqlx::query("DELETE FROM works WHERE folder_path = ?")
        .bind(path)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn get_unmatched_works(pool: &SqlitePool) -> AppResult<Vec<WorkRow>> {
    let rows: Vec<WorkRow> =
        sqlx::query_as("SELECT * FROM works WHERE enrichment_state = 'unmatched'")
            .fetch_all(pool)
            .await?;
    Ok(rows)
}
