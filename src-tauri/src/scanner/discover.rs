//! Discover — walk filesystem, compute diffs, detect moves (R19).
//!
//! The diff algorithm:
//! 1. Walk each library root → collect `HashMap<PathBuf, FolderInfo>` (FS state)
//! 2. Read DB state → `HashMap<PathBuf, FolderInfo>` + `HashMap<String, PathBuf>` (work_id → path)
//! 3. Compute: added, removed, modified, moved
//!
//! Move detection (R19): If a folder was "removed" but its metadata.json contains
//! a work_id that matches an "added" folder, it's a MOVE, not delete+add.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tracing::{debug, info, warn};

/// Information about a discovered folder.
#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub path: PathBuf,
    pub mtime: f64,
    /// Stable work_id from metadata.json (R19), if present.
    pub work_id: Option<String>,
}

/// Result of a diff between filesystem state and database state.
#[derive(Debug)]
pub struct ScanDiff {
    /// New folders not in DB (truly new, not moved)
    pub added: Vec<FolderInfo>,
    /// Folders in DB but gone from filesystem (truly removed, not moved)
    pub removed: Vec<PathBuf>,
    /// Folders whose mtime changed (metadata.json may have been edited)
    pub modified: Vec<FolderInfo>,
    /// Moved folders: (old_path, new_folder_info)
    pub moved: Vec<(PathBuf, FolderInfo)>,
}

/// Walk library roots and discover game folders.
///
/// A "game folder" is any immediate child directory of a library root
/// (we don't recurse deeper — games are top-level folders).
pub fn walk_library_roots(roots: &[PathBuf]) -> Vec<FolderInfo> {
    let mut folders = Vec::new();

    for root in roots {
        if !root.is_dir() {
            warn!(root = %root.display(), "Library root is not a directory, skipping");
            continue;
        }

        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                warn!(root = %root.display(), error = %e, "Failed to read library root");
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Only immediate child directories (not files)
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories (e.g., .trash, .cache)
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }

            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Try to read work_id from metadata.json (R19)
            let work_id = read_work_id_from_metadata(&path);

            folders.push(FolderInfo {
                path,
                mtime,
                work_id,
            });
        }
    }

    info!(count = folders.len(), "Discovered folders");
    folders
}

/// Read work_id from metadata.json without parsing the entire file.
///
/// Reads the file and extracts just the work_id field.
/// Returns None if file doesn't exist or doesn't contain work_id.
fn read_work_id_from_metadata(folder: &Path) -> Option<String> {
    let meta_path = folder.join("metadata.json");
    let content = std::fs::read_to_string(&meta_path).ok()?;

    // Parse just enough to get work_id
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("work_id")?.as_str().map(|s| s.to_string())
}

/// Data from the DB side for diff computation.
#[derive(Debug)]
pub struct DbState {
    /// path → (mtime, work_id)
    pub entries: HashMap<String, (f64, Option<String>)>,
}

/// Compute the diff between filesystem and database state.
///
/// Handles move detection (R19): if a folder disappears but its work_id
/// reappears in a new location, it's classified as MOVED, not removed+added.
pub fn compute_diff(fs_folders: Vec<FolderInfo>, db_state: &DbState) -> ScanDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut moved = Vec::new();

    // Build sets for comparison
    let fs_paths: HashSet<String> = fs_folders
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    let db_paths: HashSet<String> = db_state.entries.keys().cloned().collect();

    // Build work_id → path maps for move detection (R19)
    let mut db_workid_to_path: HashMap<String, String> = HashMap::new();
    for (path, (_, work_id)) in &db_state.entries {
        if let Some(wid) = work_id {
            db_workid_to_path.insert(wid.clone(), path.clone());
        }
    }

    let mut fs_workid_to_folder: HashMap<String, &FolderInfo> = HashMap::new();
    for folder in &fs_folders {
        if let Some(ref wid) = folder.work_id {
            fs_workid_to_folder.insert(wid.clone(), folder);
        }
    }

    // Track which paths are handled by move detection
    let mut handled_old_paths: HashSet<String> = HashSet::new();
    let mut handled_new_paths: HashSet<String> = HashSet::new();

    // Move detection (R19): work_id in DB at old path, now at new path
    for (work_id, old_path) in &db_workid_to_path {
        if let Some(fs_folder) = fs_workid_to_folder.get(work_id) {
            let new_path = fs_folder.path.to_string_lossy().to_string();
            if *old_path != new_path {
                debug!(
                    work_id = %work_id,
                    old = %old_path,
                    new = %new_path,
                    "Detected move (R19)"
                );
                moved.push((PathBuf::from(old_path), (*fs_folder).clone()));
                handled_old_paths.insert(old_path.clone());
                handled_new_paths.insert(new_path);
            }
        }
    }

    // Process FS folders
    for folder in &fs_folders {
        let path_str = folder.path.to_string_lossy().to_string();

        // Skip if already handled by move detection
        if handled_new_paths.contains(&path_str) {
            continue;
        }

        if let Some((db_mtime, _)) = db_state.entries.get(&path_str) {
            // Exists in both FS and DB — check if modified
            if (folder.mtime - db_mtime).abs() > 0.001 {
                modified.push(folder.clone());
            }
        } else {
            // In FS but not DB → added
            added.push(folder.clone());
        }
    }

    // Process DB entries not in FS
    for db_path in &db_paths {
        if handled_old_paths.contains(db_path) {
            continue;
        }
        if !fs_paths.contains(db_path) {
            removed.push(PathBuf::from(db_path));
        }
    }

    info!(
        added = added.len(),
        removed = removed.len(),
        modified = modified.len(),
        moved = moved.len(),
        "Scan diff computed"
    );

    ScanDiff {
        added,
        removed,
        modified,
        moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_basic_added() {
        let fs = vec![FolderInfo {
            path: PathBuf::from("/games/new_game"),
            mtime: 100.0,
            work_id: None,
        }];
        let db = DbState {
            entries: HashMap::new(),
        };

        let diff = compute_diff(fs, &db);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
    }

    #[test]
    fn test_diff_basic_removed() {
        let fs = vec![];
        let mut entries = HashMap::new();
        entries.insert("/games/old_game".to_string(), (100.0, None));
        let db = DbState { entries };

        let diff = compute_diff(fs, &db);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 1);
    }

    #[test]
    fn test_diff_move_detection_r19() {
        let work_id = "550e8400-e29b-41d4-a716-446655440000".to_string();

        let fs = vec![FolderInfo {
            path: PathBuf::from("/games/renamed_game"),
            mtime: 200.0,
            work_id: Some(work_id.clone()),
        }];

        let mut entries = HashMap::new();
        entries.insert(
            "/games/original_game".to_string(),
            (100.0, Some(work_id.clone())),
        );
        let db = DbState { entries };

        let diff = compute_diff(fs, &db);

        // Should be a move, not add+remove
        assert_eq!(diff.added.len(), 0, "Should not be added");
        assert_eq!(diff.removed.len(), 0, "Should not be removed");
        assert_eq!(diff.moved.len(), 1, "Should be detected as move");

        let (old_path, new_info) = &diff.moved[0];
        assert_eq!(old_path, &PathBuf::from("/games/original_game"));
        assert_eq!(new_info.path, PathBuf::from("/games/renamed_game"));
    }

    #[test]
    fn test_diff_modified() {
        let fs = vec![FolderInfo {
            path: PathBuf::from("/games/my_game"),
            mtime: 200.0,
            work_id: None,
        }];
        let mut entries = HashMap::new();
        entries.insert("/games/my_game".to_string(), (100.0, None));
        let db = DbState { entries };

        let diff = compute_diff(fs, &db);
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
    }
}
