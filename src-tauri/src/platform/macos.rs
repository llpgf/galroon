//! macOS platform handling (R14).
//!
//! Handles macOS-specific concerns: sandbox permissions,
//! file access dialogs, Gatekeeper considerations.

/// Initialize macOS-specific settings.
pub fn init() {
    tracing::info!("macOS platform init");
    // macOS sandbox and permission handling is managed by Tauri's
    // built-in capabilities system. No additional runtime init needed.
}

/// Check if the app has access to a directory (macOS sandbox).
///
/// On macOS with sandbox enabled, the app may need to request
/// access via NSOpenPanel for directories not in the app container.
pub fn check_directory_access(path: &std::path::Path) -> bool {
    // In a sandboxed macOS app, directory access is granted via
    // user-initiated file dialogs. We check if we can read the dir.
    path.is_dir() && std::fs::read_dir(path).is_ok()
}
