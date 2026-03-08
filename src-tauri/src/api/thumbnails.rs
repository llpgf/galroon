//! Thumbnail API — generate and serve thumbnails for works.

use tauri::State;

use crate::config::SharedConfig;
use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::scanner::thumbs;

/// Get the thumbnail URL for a work. Generates if missing.
#[tauri::command]
pub async fn get_thumbnail(
    config: State<'_, SharedConfig>,
    db: State<'_, Database>,
    work_id: String,
    size: Option<u32>,
) -> Result<Option<String>, AppError> {
    let target_size = size.unwrap_or(thumbs::THUMB_GALLERY);
    let cfg = config.read().await;
    let cache_dir = cfg.thumbnail_dir.clone();
    drop(cfg);

    // Check cache first
    if thumbs::thumb_exists(&cache_dir, &work_id, target_size) {
        let path = thumbs::get_thumb_path(&cache_dir, &work_id, target_size);
        return Ok(Some(path.to_string_lossy().to_string()));
    }

    // Find cover image from work's folder
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id).await?;
    let row = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    let work_folder = std::path::Path::new(&row.folder_path);
    let cover_path = match thumbs::resolve_cover_path(work_folder, row.cover_path.as_deref()) {
        Some(path) => path,
        None => return Ok(None),
    };

    match thumbs::generate_thumbnail(&cover_path, &cache_dir, &work_id, target_size) {
        Ok(thumb_path) => Ok(Some(thumb_path.to_string_lossy().to_string())),
        Err(e) => {
            tracing::warn!(work_id = %work_id, error = %e, "Thumbnail generation failed");
            Ok(None)
        }
    }
}
