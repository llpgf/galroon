//! Atomic file operations with journal (R11).
//!
//! Handles cross-volume moves by detecting when source and destination
//! are on different filesystems, falling back to copy+fsync+rename.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::error::{AppError, AppResult};

/// A journaled file operation that can be committed or rolled back.
pub struct FileTransaction {
    journal: Vec<JournalEntry>,
    committed: bool,
}

#[derive(Debug)]
enum JournalEntry {
    /// A file was copied to a temp location before overwrite.
    Backup { original: PathBuf, backup: PathBuf },
    /// A new file was created (delete on rollback).
    Created { path: PathBuf },
}

impl FileTransaction {
    pub fn new() -> Self {
        Self {
            journal: Vec::new(),
            committed: false,
        }
    }

    /// Move a file safely, detecting cross-volume scenarios (R11).
    ///
    /// 1. Try `fs::rename` (same volume, atomic)
    /// 2. If fails (cross-device), fallback to copy → fsync → rename
    pub fn safe_move(&mut self, src: &Path, dst: &Path) -> AppResult<()> {
        // Ensure parent directory exists
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        // Try atomic rename first
        match fs::rename(src, dst) {
            Ok(()) => {
                self.journal.push(JournalEntry::Created {
                    path: dst.to_path_buf(),
                });
                return Ok(());
            }
            Err(e) if is_cross_device_error(&e) => {
                // Cross-volume: copy + delete
                tracing::info!(
                    src = %src.display(),
                    dst = %dst.display(),
                    "Cross-volume move detected, using copy fallback (R11)"
                );
            }
            Err(e) => return Err(AppError::Io(e)),
        }

        // Cross-volume fallback: copy to tmp → rename to final
        let tmp_path = dst.with_extension("tmp");
        fs::copy(src, &tmp_path)?;

        // fsync the temp file
        let f = fs::File::open(&tmp_path)?;
        f.sync_all()?;
        drop(f);

        // Atomic rename tmp → final
        fs::rename(&tmp_path, dst)?;

        // Record in journal
        self.journal.push(JournalEntry::Created {
            path: dst.to_path_buf(),
        });

        // Remove source
        fs::remove_file(src)?;

        Ok(())
    }

    /// Write a file atomically: write to tmp, fsync, rename.
    pub fn atomic_write(&mut self, path: &Path, content: &[u8]) -> AppResult<()> {
        let tmp_path = path.with_extension("tmp");

        // Backup existing file if present
        if path.exists() {
            let backup = path.with_extension("bak");
            fs::copy(path, &backup)?;
            self.journal.push(JournalEntry::Backup {
                original: path.to_path_buf(),
                backup,
            });
        }

        // Write → fsync → rename
        fs::write(&tmp_path, content)?;
        let f = fs::File::open(&tmp_path)?;
        f.sync_all()?;
        drop(f);

        fs::rename(&tmp_path, path)?;
        self.journal.push(JournalEntry::Created {
            path: path.to_path_buf(),
        });

        Ok(())
    }

    /// Commit the transaction — clear journal, no rollback possible.
    pub fn commit(mut self) {
        // Clean up backup files
        for entry in &self.journal {
            if let JournalEntry::Backup { backup, .. } = entry {
                let _ = fs::remove_file(backup);
            }
        }
        self.committed = true;
    }

    /// Rollback: restore backups, delete created files.
    pub fn rollback(mut self) {
        for entry in self.journal.drain(..).rev() {
            match entry {
                JournalEntry::Backup { original, backup } => {
                    if backup.exists() {
                        let _ = fs::rename(&backup, &original);
                        tracing::warn!(path = %original.display(), "Rolled back file write");
                    }
                }
                JournalEntry::Created { path } => {
                    let _ = fs::remove_file(&path);
                    tracing::warn!(path = %path.display(), "Rolled back file creation");
                }
            }
        }
        self.committed = true; // prevent Drop rollback
    }
}

impl Drop for FileTransaction {
    fn drop(&mut self) {
        if !self.committed {
            tracing::error!("FileTransaction dropped without commit/rollback — auto-rolling back");
            // Best-effort rollback
            for entry in self.journal.drain(..).rev() {
                match entry {
                    JournalEntry::Backup { original, backup } => {
                        let _ = fs::rename(&backup, &original);
                    }
                    JournalEntry::Created { path } => {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

/// Check if an IO error is a cross-device link error.
fn is_cross_device_error(e: &std::io::Error) -> bool {
    // Windows: ERROR_NOT_SAME_DEVICE (0x11)
    // Unix: EXDEV (18)
    matches!(e.raw_os_error(), Some(17) | Some(18))
}
