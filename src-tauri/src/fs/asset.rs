//! Asset detection — identify game files in a folder.
//!
//! Scans a game folder for known asset types: executables, saves,
//! images (covers), config files, etc.

use std::fs;
use std::path::{Path, PathBuf};

/// Detected assets in a game folder.
#[derive(Debug, Default)]
pub struct FolderAssets {
    pub executables: Vec<PathBuf>,
    pub images: Vec<PathBuf>,
    pub saves: Vec<PathBuf>,
    pub config_files: Vec<PathBuf>,
    pub readme_files: Vec<PathBuf>,
    pub total_size_bytes: u64,
}

/// Known executable extensions.
const EXE_EXTS: &[&str] = &["exe", "bat", "cmd", "lnk", "app"];

/// Known image extensions.
const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// Known save/config patterns.
const SAVE_DIRS: &[&str] = &["save", "saves", "savedata", "userdata"];
const CONFIG_NAMES: &[&str] = &["config.ini", "settings.ini", "setup.exe", "config.cfg"];
const README_NAMES: &[&str] = &[
    "readme.txt",
    "readme.md",
    "read_me.txt",
    "manual.txt",
    "説明.txt",
];

/// Scan a game folder for assets (non-recursive, top-level only).
pub fn detect_assets(folder: &Path) -> FolderAssets {
    let mut assets = FolderAssets::default();

    let entries = match fs::read_dir(folder) {
        Ok(e) => e,
        Err(_) => return assets,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        assets.total_size_bytes += meta.len();

        let name_lower = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if meta.is_dir() {
            // Check for save directories
            if SAVE_DIRS.iter().any(|s| name_lower == *s) {
                assets.saves.push(path);
            }
            continue;
        }

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Executables
        if EXE_EXTS.iter().any(|e| ext == *e) {
            assets.executables.push(path.clone());
        }

        // Images
        if IMG_EXTS.iter().any(|e| ext == *e) {
            assets.images.push(path.clone());
        }

        // Config files
        if CONFIG_NAMES.iter().any(|c| name_lower == *c) {
            assets.config_files.push(path.clone());
        }

        // Readme files
        if README_NAMES.iter().any(|r| name_lower == *r) {
            assets.readme_files.push(path.clone());
        }
    }

    assets
}

/// Find the likely "main executable" in a game folder.
///
/// Heuristic: largest .exe file, or one matching the folder name.
pub fn find_main_executable(folder: &Path) -> Option<PathBuf> {
    let assets = detect_assets(folder);

    if assets.executables.is_empty() {
        return None;
    }

    let folder_name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Prefer exe matching folder name
    for exe in &assets.executables {
        let exe_name = exe
            .file_stem()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if folder_name.contains(&exe_name) || exe_name.contains(&folder_name) {
            return Some(exe.clone());
        }
    }

    // Fallback: largest exe (likely the game, not a setup utility)
    assets
        .executables
        .into_iter()
        .max_by_key(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
}
