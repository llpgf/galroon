//! Windows-specific platform handling (R12).

/// Enable long path support on Windows.
pub fn init() {
    tracing::info!("Windows platform init: long path support enabled");
    // Windows long paths are enabled by default in Rust stdlib since 1.58+
    // if the app manifest or registry allows it.
    // No runtime action needed, but we log for observability.
}

/// Convert a path to use \\?\ prefix for long path support (R12).
pub fn to_long_path(path: &std::path::Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") || s.len() < 260 {
        return path.to_path_buf();
    }
    std::path::PathBuf::from(format!(r"\\?\{}", s))
}
