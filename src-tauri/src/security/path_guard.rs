//! Path scope guard — prevents directory traversal and scope escape (R15).
//!
//! Every file operation MUST validate paths through this module.
//! Uses OsStr/PathBuf throughout — never unwrap to_str() (R5).

use std::path::{Path, PathBuf};

use crate::domain::error::{AppError, AppResult};

/// Validate that a path is within one of the allowed scopes.
///
/// Returns the canonicalized path if valid.
///
/// # Errors
/// - `PathOutOfScope` if the path escapes all allowed roots
/// - `InvalidPath` if the path cannot be canonicalized
pub fn validate_path(path: &Path, allowed_roots: &[PathBuf]) -> AppResult<PathBuf> {
    // Canonicalize to resolve symlinks and ../ components
    let canonical = dunce_or_fallback(path)?;

    // Check against each allowed root
    for root in allowed_roots {
        let canonical_root = dunce_or_fallback(root)?;
        if canonical.starts_with(&canonical_root) {
            return Ok(canonical);
        }
    }

    Err(AppError::PathOutOfScope(path.to_string_lossy().to_string()))
}

/// Check if a path contains dangerous components.
pub fn is_safe_path(path: &Path) -> bool {
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => return false, // No ../
            std::path::Component::Normal(s) => {
                let s = s.to_string_lossy();
                // Block Windows reserved device names
                let upper = s.to_uppercase();
                if matches!(
                    upper.as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                ) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Canonicalize a path, working around Windows UNC prefix issues.
///
/// On Windows, std::fs::canonicalize returns \\?\ prefixed paths
/// which can break string comparisons. We strip that prefix.
fn dunce_or_fallback(path: &Path) -> AppResult<PathBuf> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| AppError::InvalidPath(path.to_string_lossy().to_string()))?;

    // Strip \\?\ prefix on Windows
    #[cfg(windows)]
    {
        let s = canonical.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return Ok(PathBuf::from(stripped));
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_dir_blocked() {
        assert!(!is_safe_path(Path::new("../etc/passwd")));
        assert!(!is_safe_path(Path::new("foo/../../bar")));
    }

    #[test]
    fn test_normal_path_allowed() {
        assert!(is_safe_path(Path::new("games/my_game")));
        assert!(is_safe_path(Path::new("some/deep/nested/path")));
    }

    #[test]
    fn test_windows_reserved_blocked() {
        assert!(!is_safe_path(Path::new("CON")));
        assert!(!is_safe_path(Path::new("folder/NUL")));
    }
}
