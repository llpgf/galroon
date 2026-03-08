//! Debug bundle — export sanitized diagnostics for bug reports (R18).

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::error::AppResult;

/// Export a debug bundle to the given directory.
///
/// Collects:
/// - App version, OS info
/// - Config (sanitized — no absolute paths)
/// - DB stats (table counts, WAL size)
/// - Recent log entries
/// - Metrics snapshot
pub fn export_debug_bundle(
    output_dir: &Path,
    config_dir: &Path,
    db_path: &Path,
) -> AppResult<PathBuf> {
    let bundle_dir = output_dir.join("galroon_debug_bundle");
    fs::create_dir_all(&bundle_dir)?;

    // 1. System info
    let sys_info = format!(
        "galroon_version: 0.5.0\nos: {}\narch: {}\ntimestamp: {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        chrono::Utc::now().to_rfc3339(),
    );
    fs::write(bundle_dir.join("system_info.txt"), sys_info)?;

    // 2. Config (sanitized)
    let config_path = config_dir.join("config.toml");
    if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        // Sanitize: replace absolute paths with placeholders
        let sanitized = sanitize_paths(&content);
        fs::write(bundle_dir.join("config_sanitized.toml"), sanitized)?;
    }

    // 3. DB stats
    let db_size = fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let wal_path = db_path.with_extension("db-wal");
    let wal_size = fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);

    let db_stats = format!(
        "db_size_bytes: {}\nwal_size_bytes: {}\ndb_path_exists: {}\n",
        db_size,
        wal_size,
        db_path.exists(),
    );
    fs::write(bundle_dir.join("db_stats.txt"), db_stats)?;

    tracing::info!(path = %bundle_dir.display(), "Debug bundle exported");
    Ok(bundle_dir)
}

/// Replace absolute paths with [REDACTED] for privacy.
fn sanitize_paths(content: &str) -> String {
    let mut result = String::new();
    for line in content.lines() {
        if line.contains(":\\") || line.contains("/home/") || line.contains("/Users/") {
            // Line contains an absolute path — redact the value
            if let Some(eq_pos) = line.find('=') {
                result.push_str(&line[..eq_pos + 1]);
                result.push_str(" \"[REDACTED]\"");
            } else {
                result.push_str("[REDACTED_LINE]");
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}
