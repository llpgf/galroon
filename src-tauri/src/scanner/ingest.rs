//! Ingest — convert discovered folders into Work entries.
//!
//! Responsibilities:
//! 1. Extract title from folder name (strip noise)
//! 2. Read existing metadata.json (if any)
//! 3. Generate stable work_id on first ingest (R19)
//! 4. Write metadata.json atomically: tmp → rename (R2)
//! 5. Write nonce + last_written_by for watcher suppression (R20)
//! 6. Create/update Work in DB via DbWriter actor (R1)

use std::path::Path;

use regex::Regex;
use tracing::{debug, info, warn};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::domain::asset::{AssetEntry, AssetType};
use crate::domain::metadata::MetadataJson;
use crate::domain::work::{FieldSource, LibraryStatus, Work};
use crate::scanner::{classifier, thumbs};

/// Title noise patterns to strip from folder names.
const NOISE_PATTERNS: &[&str] = &[
    "(18禁)",
    "(18+)",
    "[18+]",
    "(成年向け)",
    "(18禁ゲーム)",
    "(同人)",
    "[同人]",
    "(日本語)",
    "[日本語]",
    "(Japanese)",
];

const TITLE_STOP_MARKERS: &[&str] = &[
    " dl版",
    " dl版&特典",
    " 通常版",
    " 豪華限定版",
    " 豪華版",
    " 限定版",
    " 初回版",
    " パッケージ版",
    " 予約特典",
    " fanza特典",
    " sofmap特典",
    " 特典",
    " 壁紙セット",
    " オリジナルサウンドトラック",
    " サウンドトラック",
    " vocal cd",
    " theme song",
    " voice drama",
    " ドラマcd",
    " ボイスデータ",
    " 修正パッチ",
    " 追加コンテンツ",
    " 追加シナリオ",
    " dlc",
    " update",
    " crack",
];

/// Extract a clean title from a folder or filename.
pub fn extract_title(raw_name: &str) -> String {
    let mut title = raw_name.nfkc().collect::<String>();
    title = strip_archive_suffixes(&title);
    title = strip_known_codes(&title);
    title = strip_leading_groups(&title);

    for noise in NOISE_PATTERNS {
        title = title.replace(noise, "");
    }

    title = strip_trailing_groups(&title);

    for separator in [" + ", " ＋ ", " & ", " ＆ "] {
        if let Some((head, _)) = title.split_once(separator) {
            if !head.trim().is_empty() {
                title = head.to_string();
                break;
            }
        }
    }

    title = strip_stop_markers(&title);
    title = collapse_spaces(&title);
    title = dedupe_repeated_title(&title);

    if title.is_empty() {
        collapse_spaces(&raw_name.nfkc().collect::<String>())
    } else {
        title
    }
}

fn strip_archive_suffixes(input: &str) -> String {
    let mut value = input.trim().to_string();

    let multipart = Regex::new(r"(?i)\.part\d+$").expect("multipart regex");
    value = multipart.replace(&value, "").to_string();

    for suffix in [
        ".rar", ".zip", ".7z", ".iso", ".mdf", ".mds", ".bin", ".cue", ".exe",
    ] {
        if value.to_lowercase().ends_with(suffix) {
            value.truncate(value.len() - suffix.len());
            break;
        }
    }

    value
}

fn strip_known_codes(input: &str) -> String {
    let patterns = [
        Regex::new(r"(?i)[rv]j\d{5,8}").expect("rj regex"),
        Regex::new(r"\[\d{6,8}\]").expect("id regex"),
        Regex::new(r"\d{6,8}").expect("plain id regex"),
    ];

    let mut value = input.to_string();
    for pattern in patterns {
        value = pattern.replace_all(&value, " ").to_string();
    }
    value
}

fn strip_leading_groups(input: &str) -> String {
    let mut value = input.trim().to_string();
    loop {
        let trimmed = value.trim_start();
        let next = if trimmed.starts_with('[') {
            trimmed.find(']').map(|idx| trimmed[idx + 1..].to_string())
        } else if trimmed.starts_with('(') {
            trimmed.find(')').map(|idx| trimmed[idx + 1..].to_string())
        } else {
            None
        };

        match next {
            Some(rest) if !rest.trim().is_empty() => value = rest,
            _ => break,
        }
    }
    value
}

fn strip_trailing_groups(input: &str) -> String {
    let mut value = input.trim().to_string();
    loop {
        let trimmed = value.trim_end();
        if trimmed.ends_with(']') {
            if let Some(start) = trimmed.rfind('[') {
                value = trimmed[..start].to_string();
                continue;
            }
        }
        if trimmed.ends_with(')') {
            if let Some(start) = trimmed.rfind('(') {
                value = trimmed[..start].to_string();
                continue;
            }
        }
        break;
    }
    value
}

fn strip_stop_markers(input: &str) -> String {
    let mut value = input.trim().to_string();
    let lower = value.to_lowercase();
    let mut cut_at = value.len();

    for marker in TITLE_STOP_MARKERS {
        if let Some(idx) = lower.find(marker) {
            if idx > 0 {
                cut_at = cut_at.min(idx);
            }
        }
    }

    value.truncate(cut_at);
    value
}

fn dedupe_repeated_title(input: &str) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() >= 2 && parts.len() % 2 == 0 {
        let half = parts.len() / 2;
        if parts[..half] == parts[half..] {
            return parts[..half].join(" ");
        }
    }
    input.to_string()
}

fn collapse_spaces(input: &str) -> String {
    let mut prev_space = false;
    input
        .trim()
        .trim_matches('_')
        .chars()
        .filter_map(|c| {
            if c == '_' || c.is_whitespace() {
                if prev_space {
                    None
                } else {
                    prev_space = true;
                    Some(' ')
                }
            } else {
                prev_space = false;
                Some(c)
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn infer_title(folder: &Path, folder_name: &str) -> String {
    let folder_title = extract_title(folder_name);
    if !looks_like_placeholder_title(&folder_title) {
        return folder_title;
    }

    infer_title_from_assets(folder).unwrap_or(folder_title)
}

fn looks_like_placeholder_title(title: &str) -> bool {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return true;
    }

    let simple = Regex::new(r"(?i)^[a-z]{0,2}\d{5,10}$").expect("placeholder regex");
    let codename = Regex::new(r"^[A-Z0-9_-]{4,}$").expect("codename regex");
    simple.is_match(trimmed)
        || codename.is_match(trimmed)
        || trimmed.chars().all(|c| c.is_ascii_digit())
        || trimmed.len() <= 2
}

fn infer_title_from_assets(folder: &Path) -> Option<String> {
    let assets = classifier::classify_folder(folder);
    let mut candidates: Vec<&AssetEntry> = assets.iter().filter(|asset| !asset.is_dir).collect();
    candidates.sort_by(|a, b| {
        title_asset_rank(&a.asset_type)
            .cmp(&title_asset_rank(&b.asset_type))
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| a.filename.cmp(&b.filename))
    });

    for asset in candidates {
        let candidate = extract_title(&asset.filename);
        if !looks_like_placeholder_title(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn title_asset_rank(asset_type: &AssetType) -> usize {
    match asset_type {
        AssetType::Game => 0,
        AssetType::Dlc => 1,
        AssetType::Update => 2,
        AssetType::Bonus => 3,
        AssetType::Ost => 4,
        AssetType::VoiceDrama => 5,
        AssetType::Crack => 6,
        AssetType::Save => 7,
        AssetType::Guide => 8,
        AssetType::Unknown => 9,
    }
}

/// Read metadata.json from a game folder.
pub fn read_metadata(folder: &Path) -> Option<MetadataJson> {
    let path = folder.join("metadata.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write metadata.json atomically: tmp → rename (R2).
///
/// Also writes write_nonce and last_written_by for watcher suppression (R20).
pub fn write_metadata(folder: &Path, metadata: &mut MetadataJson) -> std::io::Result<()> {
    metadata.write_nonce = Some(Uuid::now_v7().to_string());
    metadata.last_written_by = Some("galroon".to_string());

    let target = folder.join("metadata.json");
    let tmp = folder.join(".metadata.json.tmp");

    let content = serde_json::to_string_pretty(metadata)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    debug!(path = %target.display(), "metadata.json written atomically");
    Ok(())
}

/// Ingest a single folder into a Work entry.
pub fn ingest_folder(folder: &Path, mtime: f64) -> Option<Work> {
    let folder_name = folder.file_name()?.to_string_lossy().to_string();
    let mut metadata = read_metadata(folder).unwrap_or_default();
    let content_signature = compute_content_signature(folder);

    let is_first_ingest = metadata.work_id.is_none();
    if is_first_ingest {
        let new_id = Uuid::now_v7().to_string();
        metadata.work_id = Some(new_id);
        info!(folder = %folder_name, "First ingest — generated work_id (R19)");
    }
    if metadata.content_signature.is_none() {
        metadata.content_signature = content_signature.clone();
    }

    let title = metadata
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| infer_title(folder, &folder_name));

    let mut work = Work::from_discovery(folder.to_path_buf(), title, mtime);

    if let Some(ref wid) = metadata.work_id {
        if let Ok(parsed) = crate::domain::ids::WorkId::parse(wid) {
            work.id = parsed;
        }
    }
    work.title_original = metadata.title_original.clone();
    work.title_aliases = metadata.title_aliases.clone();
    work.developer = metadata.developer.clone();
    work.publisher = metadata.publisher.clone();
    work.release_date = metadata.release_date;
    work.description = metadata.description.clone();
    work.cover_path = thumbs::resolve_cover_path(folder, metadata.cover.as_deref())
        .map(|path| path.to_string_lossy().to_string())
        .or_else(|| {
            metadata
                .cover_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    work.tags = metadata.tags.clone();
    work.user_tags = metadata.user_tags.clone();
    work.field_sources = metadata.field_sources.clone();
    work.field_preferences = metadata.field_preferences.clone();
    work.user_overrides = metadata.user_overrides.clone();
    work.vndb_id = metadata.vndb_id.clone();
    work.bangumi_id = metadata.bangumi_id.clone();
    work.dlsite_id = metadata.dlsite_id.clone();
    work.rating = metadata.rating;
    work.vote_count = metadata.vote_count;
    work.metadata_hash = Some(compute_metadata_hash(folder));
    work.content_signature = content_signature;

    if let Some(ref state) = metadata.enrichment_state {
        work.enrichment_state = serde_json::from_str(&format!("\"{}\"", state)).unwrap_or_default();
    }

    if let Some(ref status) = metadata.library_status {
        work.library_status = serde_json::from_str(&format!("\"{}\"", status)).unwrap_or_default();
    }

    apply_user_overrides(&mut work);

    if is_first_ingest {
        if let Err(e) = write_metadata(folder, &mut metadata) {
            warn!(folder = %folder_name, error = %e, "Failed to write metadata.json");
        }
    }

    Some(work)
}

/// Compute a hash of metadata.json for sanity checking (R2).
fn compute_metadata_hash(folder: &Path) -> String {
    let path = folder.join("metadata.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut hash: u64 = 14695981039346656037;
            for byte in &bytes {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(1099511628211);
            }
            format!("{:016x}", hash)
        }
        Err(_) => "no_file".to_string(),
    }
}

fn compute_content_signature(folder: &Path) -> Option<String> {
    let mut assets = classifier::classify_folder(folder);
    assets.retain(|asset| !asset.is_dir);
    if assets.is_empty() {
        return None;
    }

    assets.sort_by(|left, right| {
        canonical_asset_type(&left.asset_type)
            .cmp(canonical_asset_type(&right.asset_type))
            .then_with(|| left.size_bytes.cmp(&right.size_bytes))
            .then_with(|| left.filename.cmp(&right.filename))
    });

    let canonical = assets
        .into_iter()
        .map(|asset| {
            format!(
                "{}|{}|{}",
                canonical_asset_type(&asset.asset_type),
                asset.size_bytes,
                normalized_extension(&asset.filename)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut hash: u64 = 14695981039346656037;
    for byte in canonical.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    Some(format!("{:016x}", hash))
}

fn canonical_asset_type(asset_type: &AssetType) -> &'static str {
    match asset_type {
        AssetType::Game => "game",
        AssetType::Dlc => "dlc",
        AssetType::Update => "update",
        AssetType::Ost => "ost",
        AssetType::VoiceDrama => "voice_drama",
        AssetType::Bonus => "bonus",
        AssetType::Crack => "crack",
        AssetType::Save => "save",
        AssetType::Guide => "guide",
        AssetType::Unknown => "unknown",
    }
}

fn normalized_extension(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn apply_user_overrides(work: &mut Work) {
    for (field, value) in work.user_overrides.clone() {
        match field.as_str() {
            "title" => {
                if let Some(text) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    work.title = text.to_string();
                    work.title_source = FieldSource::UserOverride;
                    work.field_sources
                        .insert("title".to_string(), "user_override".to_string());
                }
            }
            "developer" => {
                if let Some(text) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    work.developer = Some(text.to_string());
                    work.field_sources
                        .insert("developer".to_string(), "user_override".to_string());
                }
            }
            "publisher" => {
                if let Some(text) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    work.publisher = Some(text.to_string());
                    work.field_sources
                        .insert("publisher".to_string(), "user_override".to_string());
                }
            }
            "description" => {
                if let Some(text) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    work.description = Some(text.to_string());
                    work.field_sources
                        .insert("description".to_string(), "user_override".to_string());
                }
            }
            "cover_path" => {
                if let Some(text) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    work.cover_path = Some(text.to_string());
                    work.field_sources
                        .insert("cover_path".to_string(), "user_override".to_string());
                }
            }
            "rating" => {
                if let Some(number) = value.as_f64() {
                    work.rating = Some(number);
                    work.field_sources
                        .insert("rating".to_string(), "user_override".to_string());
                }
            }
            "tags" => {
                if let Some(values) = value.as_array() {
                    work.tags = values
                        .iter()
                        .filter_map(|entry| entry.as_str().map(|value| value.trim().to_string()))
                        .filter(|value| !value.is_empty())
                        .collect();
                    work.field_sources
                        .insert("tags".to_string(), "user_override".to_string());
                }
            }
            "library_status" => {
                if let Some(text) = value.as_str() {
                    work.library_status = serde_json::from_str(&format!("\"{}\"", text))
                        .unwrap_or(LibraryStatus::Unplayed);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_folder_bundle() {
        assert_eq!(
            extract_title("[251219] [しばそふと] ママ×カノEX 豪華限定版 + Vocal CD + Update"),
            "ママ×カノEX"
        );
    }

    #[test]
    fn test_extract_title_numeric_folder_candidate() {
        assert_eq!(
            extract_title("[250725][1321849][エスクード] 廃村少女［弐］ ～陰り誘う秘姫の匣～ DL版 (files).rar"),
            "廃村少女[弐] ~陰り誘う秘姫の匣~"
        );
    }

    #[test]
    fn test_extract_title_noise() {
        assert_eq!(extract_title("ゲーム名 (18禁) [Japanese]"), "ゲーム名");
    }

    #[test]
    fn test_extract_title_placeholder() {
        assert!(looks_like_placeholder_title("1261651"));
        assert!(looks_like_placeholder_title("VJ01004242"));
    }

    #[test]
    fn test_extract_title_unicode_normalization() {
        let title = extract_title("ＡＢＣゲーム");
        assert!(title.contains("ABC"));
    }

    #[test]
    fn content_signature_is_stable_for_same_assets() {
        let root = std::env::temp_dir().join(format!("galroon_sig_{}", Uuid::new_v4()));
        let first = root.join("a");
        let second = root.join("b");
        std::fs::create_dir_all(&first).expect("first dir");
        std::fs::create_dir_all(&second).expect("second dir");
        std::fs::write(first.join("game.iso"), vec![0_u8; 128]).expect("write first");
        std::fs::write(second.join("renamed.iso"), vec![0_u8; 128]).expect("write second");

        let left = compute_content_signature(&first);
        let right = compute_content_signature(&second);

        assert_eq!(left, right);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ingest_folder_uses_remote_cover_url_from_metadata() {
        let root = std::env::temp_dir().join(format!("galroon_cover_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("game.iso"), vec![0_u8; 32]).expect("asset");
        std::fs::write(
            root.join("metadata.json"),
            serde_json::json!({
                "schema_version": 1,
                "title": "Sample",
                "cover_url": "https://example.com/poster.webp"
            })
            .to_string(),
        )
        .expect("metadata");

        let work = ingest_folder(&root, 0.0).expect("ingest");
        assert_eq!(
            work.cover_path.as_deref(),
            Some("https://example.com/poster.webp")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
