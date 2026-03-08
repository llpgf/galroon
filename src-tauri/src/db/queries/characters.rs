//! Character queries — backed by `work_characters` so character browse/detail actually matches the schema.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::domain::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct CharacterRow {
    pub id: String,
    pub name: String,
    pub name_original: Option<String>,
    pub role: Option<String>,
    pub work_id: Option<String>,
    pub work_title: Option<String>,
    pub vndb_id: Option<String>,
    pub image_url: Option<String>,
    pub description: Option<String>,
}

pub async fn list_for_work(pool: &SqlitePool, work_id: &str) -> AppResult<Vec<CharacterRow>> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.name_original, wc.role, wc.work_id, w.title as work_title, \
         c.vndb_id, c.image_url, c.description \
         FROM characters c \
         JOIN work_characters wc ON wc.character_id = c.id \
         JOIN works w ON w.id = wc.work_id \
         WHERE wc.work_id = ? \
         ORDER BY c.name",
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| CharacterRow {
            id: r.get("id"),
            name: r.get("name"),
            name_original: r.get("name_original"),
            role: r.get("role"),
            work_id: r.get("work_id"),
            work_title: r.get("work_title"),
            vndb_id: r.get("vndb_id"),
            image_url: r.get("image_url"),
            description: r.get("description"),
        })
        .collect())
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<CharacterRow>> {
    let row = sqlx::query(
        "SELECT c.id, c.name, c.name_original, wc.role, wc.work_id, w.title as work_title, \
         c.vndb_id, c.image_url, c.description \
         FROM characters c \
         LEFT JOIN work_characters wc ON wc.character_id = c.id \
         LEFT JOIN works w ON w.id = wc.work_id \
         WHERE c.id = ? \
         ORDER BY w.title \
         LIMIT 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| CharacterRow {
        id: r.get("id"),
        name: r.get("name"),
        name_original: r.get("name_original"),
        role: r.get("role"),
        work_id: r.get("work_id"),
        work_title: r.get("work_title"),
        vndb_id: r.get("vndb_id"),
        image_url: r.get("image_url"),
        description: r.get("description"),
    }))
}

pub async fn search_by_name(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> AppResult<Vec<CharacterRow>> {
    let pattern = if query.trim().is_empty() {
        "%".to_string()
    } else {
        format!("%{}%", query)
    };

    let rows = sqlx::query(
        "SELECT c.id, c.name, c.name_original, wc.role, wc.work_id, w.title as work_title, \
         c.vndb_id, c.image_url, c.description \
         FROM characters c \
         LEFT JOIN work_characters wc ON wc.character_id = c.id \
         LEFT JOIN works w ON w.id = wc.work_id \
         WHERE c.name LIKE ? OR c.name_original LIKE ? \
         ORDER BY c.name, w.title \
         LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| CharacterRow {
            id: r.get("id"),
            name: r.get("name"),
            name_original: r.get("name_original"),
            role: r.get("role"),
            work_id: r.get("work_id"),
            work_title: r.get("work_title"),
            vndb_id: r.get("vndb_id"),
            image_url: r.get("image_url"),
            description: r.get("description"),
        })
        .collect())
}
