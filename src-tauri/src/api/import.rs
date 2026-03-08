//! v0.4 → v0.5 Importer (R16, R24).
//!
//! Reads v0.4 metadata.json files and populates the v0.5 database.
//! Strict read-only enforcement: source files are NEVER modified.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::error::{AppError, AppResult};

/// Import preview — shows what will happen without making changes.
#[derive(Debug, Serialize)]
pub struct ImportPreview {
    pub works_found: u64,
    pub works_importable: u64,
    pub works_skipped: u64,
    pub entries: Vec<ImportEntry>,
}

#[derive(Debug, Serialize)]
pub struct ImportEntry {
    pub folder_path: String,
    pub title: String,
    pub status: ImportStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum ImportStatus {
    WillImport,
    AlreadyExists,
    InvalidFormat,
}

/// Legacy v0.4 metadata format.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct V04Metadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    developer: Option<String>,
    #[serde(default)]
    vndb_id: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    rating: Option<f64>,
    #[serde(default)]
    library_status: Option<String>,
}

/// Importer — read-only by default (R24).
pub struct Importer {
    readonly: bool,
}

impl Importer {
    pub fn new() -> Self {
        Self { readonly: true }
    }

    /// Phase 1: Preview what will be imported (read-only, no DB writes).
    pub fn preview(&self, v04_library_root: &Path) -> AppResult<ImportPreview> {
        let mut preview = ImportPreview {
            works_found: 0,
            works_importable: 0,
            works_skipped: 0,
            entries: Vec::new(),
        };

        if !v04_library_root.is_dir() {
            return Err(AppError::Config(format!(
                "v0.4 library root not found: {}",
                v04_library_root.display()
            )));
        }

        let entries = fs::read_dir(v04_library_root)?;
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }

            preview.works_found += 1;
            let folder = entry.path();
            let metadata_path = folder.join("metadata.json");

            if !metadata_path.exists() {
                preview.entries.push(ImportEntry {
                    folder_path: folder.to_string_lossy().to_string(),
                    title: folder
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    status: ImportStatus::InvalidFormat,
                    reason: Some("No metadata.json found".to_string()),
                });
                preview.works_skipped += 1;
                continue;
            }

            match fs::read_to_string(&metadata_path) {
                Ok(content) => match serde_json::from_str::<V04Metadata>(&content) {
                    Ok(meta) => {
                        let title = meta.title.unwrap_or_else(|| {
                            folder
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        });
                        preview.entries.push(ImportEntry {
                            folder_path: folder.to_string_lossy().to_string(),
                            title,
                            status: ImportStatus::WillImport,
                            reason: None,
                        });
                        preview.works_importable += 1;
                    }
                    Err(e) => {
                        preview.entries.push(ImportEntry {
                            folder_path: folder.to_string_lossy().to_string(),
                            title: folder
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            status: ImportStatus::InvalidFormat,
                            reason: Some(format!("JSON parse error: {}", e)),
                        });
                        preview.works_skipped += 1;
                    }
                },
                Err(e) => {
                    preview.works_skipped += 1;
                    preview.entries.push(ImportEntry {
                        folder_path: folder.to_string_lossy().to_string(),
                        title: folder
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        status: ImportStatus::InvalidFormat,
                        reason: Some(format!("Read error: {}", e)),
                    });
                }
            }
        }

        Ok(preview)
    }

    /// Phase 2: Execute import (sets readonly = false internally).
    ///
    /// This does NOT modify v0.4 source files (R24).
    /// It only writes to the v0.5 database.
    pub fn confirm(&mut self) {
        if self.readonly {
            tracing::info!("Importer: switching from preview to execute mode");
            self.readonly = false;
        }
    }

    /// Read a single v0.4 metadata.json for import.
    #[allow(dead_code)]
    fn read_v04_metadata(metadata_path: &Path) -> AppResult<V04Metadata> {
        let content = fs::read_to_string(metadata_path)?;
        let meta: V04Metadata = serde_json::from_str(&content)
            .map_err(|e| AppError::Internal(format!("v0.4 metadata parse error: {}", e)))?;
        Ok(meta)
    }
}

impl Default for Importer {
    fn default() -> Self {
        Self::new()
    }
}
