//! Thumbnail cache — generates and serves fixed-size thumbnails (R21).
//!
//! Generates resized cover images as WebP thumbnails stored in app cache dir.
//! Prevents I/O storms from reading full-size covers during gallery scroll.

use image::imageops::FilterType;
use image::GenericImageView;
use image::ImageFormat;
use std::path::{Path, PathBuf};

/// Standard thumbnail sizes (width in pixels, height auto-scaled).
pub const THUMB_GALLERY: u32 = 250;
pub const THUMB_DETAIL: u32 = 500;

/// Get the path to a cached thumbnail.
pub fn get_thumb_path(cache_dir: &Path, work_id: &str, size: u32) -> PathBuf {
    cache_dir
        .join("thumbs")
        .join(format!("{}_{}.webp", work_id, size))
}

/// Check if a thumbnail exists in the cache.
pub fn thumb_exists(cache_dir: &Path, work_id: &str, size: u32) -> bool {
    get_thumb_path(cache_dir, work_id, size).exists()
}

/// Supported image extensions for cover discovery.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp"];

/// Resolve a cover path from metadata or folder contents.
///
/// `cover_hint` may be an absolute path or a path relative to the work folder.
pub fn resolve_cover_path(folder: &Path, cover_hint: Option<&str>) -> Option<PathBuf> {
    if let Some(hint) = cover_hint.map(str::trim).filter(|s| !s.is_empty()) {
        let hinted_path = Path::new(hint);
        let resolved = if hinted_path.is_absolute() {
            hinted_path.to_path_buf()
        } else {
            folder.join(hint)
        };

        if is_supported_image(&resolved) {
            return Some(resolved);
        }
    }

    find_cover_image(folder)
}

/// Find the first image file in a folder that looks like a cover.
///
/// Searches the work folder first, then shallow subfolders, so extracted extras
/// can still provide a usable poster without recursing through huge archives.
pub fn find_cover_image(folder: &Path) -> Option<PathBuf> {
    find_cover_image_inner(folder, 0)
}

fn find_cover_image_inner(folder: &Path, depth: usize) -> Option<PathBuf> {
    let cover_names = [
        "cover",
        "folder",
        "thumbnail",
        "box",
        "package",
        "ジャケット",
    ];
    let mut image_files: Vec<PathBuf> = Vec::new();
    let mut child_dirs: Vec<PathBuf> = Vec::new();

    let entries = std::fs::read_dir(folder).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 1 {
                child_dirs.push(path);
            }
            continue;
        }

        if !is_supported_image(&path) {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if cover_names.contains(&stem.as_str()) {
            return Some(path);
        }

        image_files.push(path);
    }

    if !image_files.is_empty() {
        image_files.sort();
        return image_files.into_iter().next();
    }

    child_dirs.sort();
    for dir in child_dirs {
        if let Some(found) = find_cover_image_inner(&dir, depth + 1) {
            return Some(found);
        }
    }

    None
}

fn is_supported_image(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .map(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
            .unwrap_or(false)
}

/// Generate a thumbnail from a source image.
///
/// Resizes to target width, maintains aspect ratio, saves as WebP.
/// Uses tmp→rename for atomicity.
pub fn generate_thumbnail(
    source: &Path,
    cache_dir: &Path,
    work_id: &str,
    target_width: u32,
) -> Result<PathBuf, String> {
    let dest = get_thumb_path(cache_dir, work_id, target_width);

    // Ensure thumbs directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create thumbs dir: {}", e))?;
    }

    // Load source image
    let img = image::open(source)
        .map_err(|e| format!("Failed to open image {}: {}", source.display(), e))?;

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("Image has zero dimensions".into());
    }

    // Calculate target height maintaining aspect ratio
    let target_height = (target_width as f64 * h as f64 / w as f64) as u32;

    // Resize
    let resized = img.resize_exact(target_width, target_height, FilterType::Lanczos3);

    // Save as WebP explicitly; a bare .tmp extension makes image format inference fail.
    let tmp_path = dest.with_extension("tmp.webp");
    resized
        .save_with_format(&tmp_path, ImageFormat::WebP)
        .map_err(|e| format!("Failed to save thumbnail: {}", e))?;

    // Atomic rename
    std::fs::rename(&tmp_path, &dest).map_err(|e| format!("Failed to rename thumbnail: {}", e))?;

    tracing::debug!(
        work_id = work_id,
        size = target_width,
        path = %dest.display(),
        "Thumbnail generated"
    );

    Ok(dest)
}
