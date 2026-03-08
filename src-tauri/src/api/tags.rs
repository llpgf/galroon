//! Tag CRUD — add, remove, search, bulk operations for user and auto tags.

use serde::Serialize;
use sqlx::FromRow;
use tauri::State;

use crate::db::Database;
use crate::domain::error::AppError;

#[derive(Serialize, FromRow)]
pub struct TagInfo {
    pub id: String,
    pub name: String,
    pub tag_type: String,
}

// ── User Tags ──

#[tauri::command]
pub async fn list_user_tags(db: State<'_, Database>) -> Result<Vec<TagInfo>, AppError> {
    let rows: Vec<TagInfo> = sqlx::query_as(
        "SELECT ut.id, ut.name, 'user' as tag_type FROM user_tags ut ORDER BY ut.name",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_user_tag(db: State<'_, Database>, name: String) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    db.execute_write(
        "INSERT OR IGNORE INTO user_tags (id, name) VALUES (?1, ?2)".to_string(),
        vec![
            serde_json::Value::String(id),
            serde_json::Value::String(name.clone()),
        ],
    )
    .await?;

    let (found_id,): (String,) = sqlx::query_as("SELECT id FROM user_tags WHERE name = ?")
        .bind(&name)
        .fetch_one(db.read_pool())
        .await?;
    Ok(found_id)
}

#[tauri::command]
pub async fn delete_user_tag(db: State<'_, Database>, tag_id: String) -> Result<(), AppError> {
    db.execute_write(
        "DELETE FROM work_user_tags WHERE tag_id = ?1".to_string(),
        vec![serde_json::Value::String(tag_id.clone())],
    )
    .await?;
    db.execute_write(
        "DELETE FROM user_tags WHERE id = ?1".to_string(),
        vec![serde_json::Value::String(tag_id)],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn rename_user_tag(
    db: State<'_, Database>,
    tag_id: String,
    new_name: String,
) -> Result<(), AppError> {
    db.execute_write(
        "UPDATE user_tags SET name = ?1 WHERE id = ?2".to_string(),
        vec![
            serde_json::Value::String(new_name),
            serde_json::Value::String(tag_id),
        ],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn tag_work(
    db: State<'_, Database>,
    work_id: String,
    tag_id: String,
) -> Result<(), AppError> {
    db.execute_write(
        "INSERT OR IGNORE INTO work_user_tags (work_id, tag_id) VALUES (?1, ?2)".to_string(),
        vec![
            serde_json::Value::String(work_id),
            serde_json::Value::String(tag_id),
        ],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn untag_work(
    db: State<'_, Database>,
    work_id: String,
    tag_id: String,
) -> Result<(), AppError> {
    db.execute_write(
        "DELETE FROM work_user_tags WHERE work_id = ?1 AND tag_id = ?2".to_string(),
        vec![
            serde_json::Value::String(work_id),
            serde_json::Value::String(tag_id),
        ],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn get_work_tags(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Vec<TagInfo>, AppError> {
    let user: Vec<TagInfo> = sqlx::query_as(
        "SELECT ut.id, ut.name, 'user' as tag_type \
         FROM user_tags ut JOIN work_user_tags wut ON ut.id = wut.tag_id \
         WHERE wut.work_id = ? ORDER BY ut.name",
    )
    .bind(&work_id)
    .fetch_all(db.read_pool())
    .await?;

    let auto: Vec<TagInfo> = sqlx::query_as(
        "SELECT at.id, at.name, 'auto' as tag_type \
         FROM auto_tags at JOIN work_auto_tags wat ON at.id = wat.tag_id \
         WHERE wat.work_id = ? ORDER BY at.name",
    )
    .bind(&work_id)
    .fetch_all(db.read_pool())
    .await?;

    let mut all = user;
    all.extend(auto);
    Ok(all)
}

#[tauri::command]
pub async fn search_tags(db: State<'_, Database>, query: String) -> Result<Vec<TagInfo>, AppError> {
    let q = format!("%{}%", query);
    let user: Vec<TagInfo> = sqlx::query_as(
        "SELECT id, name, 'user' as tag_type FROM user_tags WHERE name LIKE ? ORDER BY name LIMIT 20",
    )
    .bind(&q)
    .fetch_all(db.read_pool())
    .await?;
    let auto: Vec<TagInfo> = sqlx::query_as(
        "SELECT id, name, 'auto' as tag_type FROM auto_tags WHERE name LIKE ? ORDER BY name LIMIT 20",
    )
    .bind(&q)
    .fetch_all(db.read_pool())
    .await?;
    let mut all = user;
    all.extend(auto);
    Ok(all)
}

#[tauri::command]
pub async fn bulk_tag_works(
    db: State<'_, Database>,
    work_ids: Vec<String>,
    tag_id: String,
) -> Result<u64, AppError> {
    let mut count: u64 = 0;
    for wid in &work_ids {
        db.execute_write(
            "INSERT OR IGNORE INTO work_user_tags (work_id, tag_id) VALUES (?1, ?2)".to_string(),
            vec![
                serde_json::Value::String(wid.clone()),
                serde_json::Value::String(tag_id.clone()),
            ],
        )
        .await?;
        count += 1;
    }
    Ok(count)
}
