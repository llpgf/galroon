//! Smart trash — hybrid: OS trash when possible, workspace .trash/ as fallback.
//!
//! - Windows/macOS desktop → OS Recycle Bin / Trash (via `trash` crate)
//! - Linux with desktop → FreeDesktop Trash (~/.local/share/Trash/)
//! - Linux NAS / network mount / headless → workspace .trash/ (browsable in app)
//!
//! The fallback ensures NAS users can always restore from within Galroon.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::domain::error::{AppError, AppResult};

/// Trash result — tells the caller where the file went.
pub enum TrashResult {
    /// Sent to OS recycle bin (user restores via Explorer/Finder)
    OsTrash,
    /// Moved to workspace .trash/ (user restores via Galroon UI)
    WorkspaceTrash(PathBuf),
}

/// Move a file to trash — tries OS trash first, falls back to workspace .trash/.
pub fn move_to_trash(path: &Path, workspace_trash_dir: &Path) -> AppResult<TrashResult> {
    if !path.exists() {
        return Err(AppError::Internal(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    // Try OS trash first (works on desktop environments)
    if should_use_os_trash(path) {
        match trash::delete(path) {
            Ok(()) => {
                tracing::info!(path = %path.display(), "Moved to OS trash");
                return Ok(TrashResult::OsTrash);
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "OS trash failed, falling back to workspace trash"
                );
            }
        }
    }

    // Fallback: workspace .trash/ directory
    move_to_workspace_trash(path, workspace_trash_dir)
}

/// Move to workspace-level .trash/ (NAS-safe, always works).
fn move_to_workspace_trash(path: &Path, trash_dir: &Path) -> AppResult<TrashResult> {
    fs::create_dir_all(trash_dir)?;

    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::Internal("No filename".to_string()))?;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let trash_name = format!("{}_{}", timestamp, file_name.to_string_lossy());
    let trash_path = trash_dir.join(&trash_name);

    // Try rename first, fallback to copy for cross-volume
    if fs::rename(path, &trash_path).is_err() {
        if path.is_dir() {
            copy_dir_recursive(path, &trash_path)?;
            fs::remove_dir_all(path)?;
        } else {
            fs::copy(path, &trash_path)?;
            fs::remove_file(path)?;
        }
    }

    tracing::info!(
        original = %path.display(),
        trash = %trash_path.display(),
        "Moved to workspace trash"
    );

    Ok(TrashResult::WorkspaceTrash(trash_path))
}

/// Restore a file from workspace .trash/ to its original location.
pub fn restore_from_workspace_trash(trash_path: &Path, restore_to: &Path) -> AppResult<()> {
    if !trash_path.exists() {
        return Err(AppError::Internal("Trash item not found".to_string()));
    }
    if let Some(parent) = restore_to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(trash_path, restore_to)?;
    tracing::info!(path = %restore_to.display(), "Restored from workspace trash");
    Ok(())
}

/// List items in workspace .trash/ directory.
pub fn list_workspace_trash(trash_dir: &Path) -> AppResult<Vec<WorkspaceTrashItem>> {
    if !trash_dir.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(trash_dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let age = SystemTime::now()
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();

        items.push(WorkspaceTrashItem {
            path: entry.path(),
            name: entry.file_name().to_string_lossy().to_string(),
            size: meta.len(),
            age_days: (age.as_secs() / 86400) as u32,
            is_dir: meta.is_dir(),
        });
    }
    items.sort_by(|a, b| a.age_days.cmp(&b.age_days));
    Ok(items)
}

/// Purge items older than retention_days from workspace .trash/.
pub fn purge_old_trash(trash_dir: &Path, retention_days: u32) -> AppResult<usize> {
    if !trash_dir.exists() {
        return Ok(0);
    }
    let mut purged = 0;
    for entry in fs::read_dir(trash_dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let age = SystemTime::now()
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();
        if age.as_secs() > (retention_days as u64) * 86400 {
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
            purged += 1;
        }
    }
    if purged > 0 {
        tracing::info!(purged, "Purged expired workspace trash items");
    }
    Ok(purged)
}

pub struct WorkspaceTrashItem {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub age_days: u32,
    pub is_dir: bool,
}

/// Decide whether to use OS trash for a given path.
///
/// Skip OS trash for:
/// - Network mounts (UNC paths on Windows, /mnt/ /media/ on Linux)
/// - Headless Linux (no DISPLAY / WAYLAND_DISPLAY)
fn should_use_os_trash(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Windows: skip for UNC network paths (\\server\share)
    if path_str.starts_with("\\\\") {
        return false;
    }

    // Linux: skip for common network mount points
    if cfg!(target_os = "linux") {
        if path_str.starts_with("/mnt/")
            || path_str.starts_with("/media/")
            || path_str.starts_with("/net/")
        {
            return false;
        }
        // Headless: no desktop environment
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            return false;
        }
    }

    true
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> AppResult<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
