//! File classifier — detect asset types within a game folder.
//!
//! Scans a game folder and classifies each file/subfolder as:
//! Game, Crack, OST, Voice Drama, Save, Guide, Bonus, DLC, Update, or Unknown.
//!
//! Detection uses filename patterns, extensions, file size, and bundle context.

use std::path::Path;

use crate::domain::asset::{AssetEntry, AssetType};

/// Size threshold: files >100MB are likely game archives.
const GAME_SIZE_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Classify all files and immediate subdirectories in a game folder.
pub fn classify_folder(folder: &Path) -> Vec<AssetEntry> {
    let mut assets = Vec::new();

    let entries = match std::fs::read_dir(folder) {
        Ok(e) => e,
        Err(_) => return assets,
    };

    let folder_context = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let filename = entry.file_name().to_string_lossy().to_string();
        let lower = filename.to_lowercase();
        let is_dir = meta.is_dir();
        let size = if is_dir { dir_size(&path) } else { meta.len() };

        let asset_type = classify_entry(&lower, &path, is_dir, size, &folder_context);

        assets.push(AssetEntry {
            path,
            filename,
            asset_type,
            size_bytes: size,
            is_dir,
        });
    }

    assets.sort_by(|a, b| {
        asset_rank(&a.asset_type)
            .cmp(&asset_rank(&b.asset_type))
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| a.filename.cmp(&b.filename))
    });

    assets
}

/// Classify a single entry by filename, extension, directory flag, size, and folder context.
fn classify_entry(
    lower: &str,
    path: &Path,
    is_dir: bool,
    size: u64,
    folder_context: &str,
) -> AssetType {
    let ext = extension_lower(path);
    if is_metadata_noise(lower) {
        return AssetType::Unknown;
    }

    if is_crack(lower) {
        return AssetType::Crack;
    }

    if is_save(lower, path) {
        return AssetType::Save;
    }

    if is_update(lower, &ext, size, folder_context) {
        return AssetType::Update;
    }

    if is_voice_drama(lower) {
        return AssetType::VoiceDrama;
    }

    if is_ost(lower, path, is_dir, folder_context) {
        return AssetType::Ost;
    }

    if is_guide(lower) {
        return AssetType::Guide;
    }

    if is_dlc(lower) {
        return AssetType::Dlc;
    }

    if is_bonus(lower, folder_context) {
        return AssetType::Bonus;
    }

    if is_game(lower, path, is_dir, size) {
        return AssetType::Game;
    }

    AssetType::Unknown
}

fn asset_rank(asset_type: &AssetType) -> usize {
    match asset_type {
        AssetType::Game => 0,
        AssetType::Dlc => 1,
        AssetType::Update => 2,
        AssetType::Ost => 3,
        AssetType::VoiceDrama => 4,
        AssetType::Bonus => 5,
        AssetType::Crack => 6,
        AssetType::Save => 7,
        AssetType::Guide => 8,
        AssetType::Unknown => 9,
    }
}

// ── Detection functions ────────────────────────────────

fn is_metadata_noise(name: &str) -> bool {
    matches!(name, "metadata.json") || name.ends_with(".txt")
}

fn is_crack(name: &str) -> bool {
    let patterns = [
        "crack",
        "patch",
        "nodvd",
        "no-dvd",
        "nocd",
        "no-cd",
        "keygen",
        "loader",
        "bypass",
        "クラック",
    ];
    patterns.iter().any(|p| name.contains(p))
}

fn is_save(name: &str, path: &Path) -> bool {
    let name_patterns = ["save", "セーブ", "savdata", "savedata", "sav", "save_data"];
    if name_patterns.iter().any(|p| name.contains(p)) {
        return true;
    }
    let ext = extension_lower(path);
    matches!(ext.as_str(), "sav" | "dat" | "rpgsave")
}

fn is_update(name: &str, ext: &str, size: u64, folder_context: &str) -> bool {
    let patterns = [
        "update",
        "アップデート",
        "修正パッチ",
        "hotfix",
        "ver.",
        "version",
        "patch ver",
        "v1.",
        "v2.",
        "v3.",
    ];
    if patterns.iter().any(|p| name.contains(p)) {
        return true;
    }

    if matches!(ext, "zip" | "rar" | "7z")
        && size < 200 * 1024 * 1024
        && folder_context.contains("update")
    {
        let stem = name.rsplit_once('.').map(|(base, _)| base).unwrap_or(name);
        let compact = stem.replace(['.', '_', '-'], "");
        let romanized = compact.chars().all(|c| c.is_ascii_alphanumeric());
        let has_version_tail = compact
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count()
            >= 1;
        if romanized && has_version_tail {
            return true;
        }
    }

    false
}

fn is_voice_drama(name: &str) -> bool {
    let patterns = [
        "voice drama",
        "ドラマcd",
        "ドラマ cd",
        "ボイスドラマ",
        "ボイスデータ",
        "special voice",
        "スペシャルボイス",
    ];
    patterns.iter().any(|p| name.contains(p))
}

fn is_ost(name: &str, path: &Path, is_dir: bool, folder_context: &str) -> bool {
    let name_patterns = [
        "ost",
        "soundtrack",
        "bgm",
        "music",
        "サウンドトラック",
        "vocal",
        "theme song",
        "opening",
        "ending",
        "カバーソング",
        "ヴォーカルcd",
        "オリジナルヴォーカルcd",
    ];
    if name_patterns.iter().any(|p| name.contains(p)) {
        return true;
    }
    if folder_context.contains("theme song") && name.ends_with(".rar") {
        return false;
    }
    if is_dir {
        return dir_has_mostly_audio(path);
    }
    let ext = extension_lower(path);
    matches!(
        ext.as_str(),
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "wma" | "opus"
    )
}

fn is_guide(name: &str) -> bool {
    let patterns = [
        "攻略",
        "walkthrough",
        "guide",
        "faq",
        "tips",
        "ガイド",
        "hint",
        "strategy",
        "チャート",
        "manual",
    ];
    patterns.iter().any(|p| name.contains(p))
}

fn is_dlc(name: &str) -> bool {
    let patterns = [
        "dlc",
        "append",
        "追加シナリオ",
        "extra scenario",
        "追加コンテンツ",
        "append disc",
    ];
    patterns.iter().any(|p| name.contains(p))
}

fn is_bonus(name: &str, _folder_context: &str) -> bool {
    let patterns = [
        "特典",
        "予約特典",
        "fanza特典",
        "sofmap特典",
        "限定版特典",
        "wallpaper",
        "壁紙",
        "artbook",
        "art book",
        "設定資料",
        "設定資料集",
        "pdf",
        "bonus",
        "tokuten",
        "omake",
        "おまけ",
    ];
    patterns.iter().any(|p| name.contains(p))
}

fn is_game(name: &str, path: &Path, is_dir: bool, size: u64) -> bool {
    let ext = extension_lower(path);
    if matches!(ext.as_str(), "mdf" | "mds" | "iso" | "bin" | "cue") {
        return true;
    }

    if matches!(ext.as_str(), "zip" | "rar" | "7z" | "tar" | "gz") {
        if name.contains("(files)") || name.contains("dl版") || name.contains("パッケージ版")
        {
            return true;
        }
        if size > GAME_SIZE_THRESHOLD {
            return true;
        }
        if !(is_bonus(name, "")
            || is_voice_drama(name)
            || is_ost(name, path, false, "")
            || is_update(name, ext.as_str(), size, "")
            || is_dlc(name)
            || is_crack(name))
        {
            return true;
        }
    }

    if is_dir && dir_contains_exe(path) {
        return true;
    }

    ext == "exe" && !is_crack(name)
}

// ── Helpers ────────────────────────────────────────────

fn extension_lower(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn dir_contains_exe(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| extension_lower(&e.path()) == "exe")
        })
        .unwrap_or(false)
}

fn dir_has_mostly_audio(dir: &Path) -> bool {
    let audio_exts = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "wma", "opus"];
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map(|e| e.flatten().collect())
        .unwrap_or_default();

    if entries.is_empty() {
        return false;
    }

    let audio_count = entries
        .iter()
        .filter(|e| {
            let ext = extension_lower(&e.path());
            audio_exts.contains(&ext.as_str())
        })
        .count();

    audio_count * 2 > entries.len()
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_crack() {
        assert_eq!(
            classify_entry("nodvd_fix.exe", Path::new("nodvd_fix.exe"), false, 500, ""),
            AssetType::Crack
        );
    }

    #[test]
    fn test_classify_voice_drama() {
        assert_eq!(
            classify_entry(
                &"豪華限定版特典 ドラマCD.rar".to_lowercase(),
                Path::new("drama.rar"),
                false,
                5000,
                ""
            ),
            AssetType::VoiceDrama
        );
    }

    #[test]
    fn test_classify_update() {
        assert_eq!(
            classify_entry(
                "修正パッチVer1.01.rar",
                Path::new("patch.rar"),
                false,
                5000,
                ""
            ),
            AssetType::Update
        );
    }

    #[test]
    fn test_classify_bonus() {
        assert_eq!(
            classify_entry(
                "壁紙セット.rar",
                Path::new("wallpaper.rar"),
                false,
                5000,
                ""
            ),
            AssetType::Bonus
        );
    }

    #[test]
    fn test_classify_large_archive_as_game() {
        let big = GAME_SIZE_THRESHOLD + 1;
        assert_eq!(
            classify_entry("game.zip", Path::new("game.zip"), false, big, ""),
            AssetType::Game
        );
    }

    #[test]
    fn test_classify_files_archive_as_game() {
        assert_eq!(
            classify_entry(
                "作品名 dl版 (files).rar",
                Path::new("game.rar"),
                false,
                1000,
                ""
            ),
            AssetType::Game
        );
    }
}
