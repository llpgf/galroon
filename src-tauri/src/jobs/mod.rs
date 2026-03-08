use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::api::scanner::{run_scan_job, ScanResult};
use crate::config::{BackupConfig, SharedConfig, UpdateConfig};
use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;

#[derive(Clone)]
pub struct AppJobWorker {
    db: Arc<Database>,
    config: SharedConfig,
}

impl AppJobWorker {
    pub fn new(db: Arc<Database>, config: SharedConfig) -> Self {
        Self { db, config }
    }

    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        if let Ok(recovered) = queries::app_jobs::recover_interrupted_jobs(self.db.read_pool()).await {
            if recovered > 0 {
                tracing::info!(recovered, "Recovered interrupted app jobs");
            }
        }

        loop {
            if *shutdown.borrow() {
                break;
            }

            match queries::app_jobs::claim_next_job(self.db.read_pool()).await {
                Ok(Some(job)) => {
                    let result = self.process_job(&job).await;
                    match result {
                        Ok(value) => {
                            let _ = queries::app_jobs::complete_job(self.db.read_pool(), job.id, Some(&value)).await;
                        }
                        Err(AppError::Internal(message)) if message == "job_paused" => {
                            let _ = queries::app_jobs::update_progress(
                                self.db.read_pool(),
                                job.id,
                                job.progress_pct,
                                Some("Paused"),
                                None,
                            )
                            .await;
                        }
                        Err(AppError::Internal(message)) if message == "job_cancelled" => {
                            let _ = queries::app_jobs::cancel_job(self.db.read_pool(), job.id).await;
                        }
                        Err(error) => {
                            let _ = queries::app_jobs::fail_job(self.db.read_pool(), job.id, &error.to_string()).await;
                        }
                    }
                }
                Ok(None) => {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                        _ = shutdown.changed() => break,
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "App job worker failed to claim job");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn process_job(&self, job: &crate::db::models::AppJobRow) -> Result<serde_json::Value, AppError> {
        match job.kind.as_str() {
            "scan_library" => {
                let result: ScanResult = run_scan_job(&self.config, &self.db, job.id).await?;
                Ok(serde_json::to_value(result)?)
            }
            "workspace_backup" => self.run_backup_job(job.id, &job.payload).await,
            "update_check" => self.run_update_check(job.id).await,
            _ => Err(AppError::Validation(format!("Unsupported app job kind: {}", job.kind))),
        }
    }

    async fn run_backup_job(
        &self,
        job_id: i64,
        payload: &Option<String>,
    ) -> Result<serde_json::Value, AppError> {
        queries::app_jobs::update_progress(
            self.db.read_pool(),
            job_id,
            5.0,
            Some("Preparing backup destination"),
            None,
        )
        .await?;

        let payload_value = payload
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .unwrap_or_else(|| json!({}));
        let snapshot = self.config.snapshot().await;
        let destination_root = payload_value
            .get("destination_dir")
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
            .or_else(|| snapshot.backups.destination_dir.clone().map(PathBuf::from))
            .ok_or_else(|| AppError::Validation("Backup destination is not configured".to_string()))?;
        let keep_last = payload_value
            .get("keep_last")
            .and_then(|value| value.as_u64())
            .unwrap_or(snapshot.backups.keep_last as u64)
            .max(1);

        let workspace_dir = snapshot.workspace_dir;
        tokio::fs::create_dir_all(&destination_root).await?;

        let timestamp = chrono::Local::now().format("galroon_%Y%m%d_%H%M%S").to_string();
        let backup_dir = destination_root.join(timestamp);

        copy_dir_all_with_progress(
            &workspace_dir,
            &backup_dir,
            |progress, step| async move {
                let _ = queries::app_jobs::update_progress(
                    self.db.read_pool(),
                    job_id,
                    10.0 + progress * 80.0,
                    Some(step),
                    None,
                )
                .await;
            },
        )
        .await?;

        prune_old_backups(&destination_root, keep_last as usize)?;
        self.config
            .update(|cfg| {
                cfg.backups.last_run_at = Some(chrono::Utc::now().to_rfc3339());
            })
            .await?;

        Ok(json!({
            "backup_dir": backup_dir.to_string_lossy().to_string(),
            "keep_last": keep_last,
        }))
    }

    async fn run_update_check(&self, job_id: i64) -> Result<serde_json::Value, AppError> {
        queries::app_jobs::update_progress(
            self.db.read_pool(),
            job_id,
            10.0,
            Some("Checking GitHub releases"),
            None,
        )
        .await?;

        let updates = self.config.snapshot().await.updates;
        let stable_channel = updates.channel.trim().is_empty()
            || updates.channel.eq_ignore_ascii_case("stable");
        let url = if stable_channel {
            format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                updates.repo_owner, updates.repo_name
            )
        } else {
            format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=20",
                updates.repo_owner, updates.repo_name
            )
        };

        let response = reqwest::Client::new()
            .get(url)
            .header("User-Agent", "Galroon/0.5.0")
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let response = response
            .error_for_status()
            .map_err(|error| AppError::Network(error.to_string()))?;
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let payload = if stable_channel {
            payload
        } else {
            select_release_for_channel(payload, &updates.channel).ok_or_else(|| {
                AppError::NotFound(format!(
                    "No GitHub release matched channel '{}'",
                    updates.channel
                ))
            })?
        };

        self.config
            .update(|cfg| {
                cfg.updates.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
            })
            .await?;

        queries::app_jobs::update_progress(
            self.db.read_pool(),
            job_id,
            90.0,
            Some("Latest release metadata downloaded"),
            Some(&payload),
        )
        .await?;

        Ok(payload)
    }
}

fn select_release_for_channel(payload: serde_json::Value, channel: &str) -> Option<serde_json::Value> {
    let releases = payload.as_array()?;
    let lowered = channel.to_lowercase();
    releases
        .iter()
        .find(|release| {
            release
                .get("prerelease")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                && release
                    .get("tag_name")
                    .and_then(|value| value.as_str())
                    .map(|tag| tag.to_lowercase().contains(&lowered))
                    .unwrap_or(true)
        })
        .cloned()
        .or_else(|| releases.first().cloned())
}

pub async fn backup_scheduler_loop(config: SharedConfig, db: Arc<Database>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() {
            break;
        }

        if let Err(error) = maybe_enqueue_scheduled_backup(&config, db.read_pool()).await {
            tracing::warn!(error = %error, "Scheduled backup evaluation failed");
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(60)) => {},
            _ = shutdown.changed() => break,
        }
    }
}

pub async fn maybe_enqueue_scheduled_backup(
    config: &SharedConfig,
    pool: &sqlx::SqlitePool,
) -> Result<(), AppError> {
    let snapshot = config.snapshot().await;
    let backup = snapshot.backups.clone();
    if !backup.enabled {
        return Ok(());
    }

    let destination = backup
        .destination_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("Backup schedule enabled without destination".to_string()))?;

    if !is_backup_due(&backup) {
        return Ok(());
    }

    let payload = json!({
        "destination_dir": destination,
        "keep_last": backup.keep_last,
    });
    let hour_bucket = chrono::Utc::now().format("%Y%m%d%H").to_string();
    queries::app_jobs::enqueue_job(
        pool,
        "workspace_backup",
        "Scheduled workspace backup",
        Some(&payload),
        Some(&format!("backup:{hour_bucket}")),
        true,
        true,
        true,
    )
    .await?;

    config
        .update(|cfg| {
            cfg.backups.last_run_at = Some(chrono::Utc::now().to_rfc3339());
        })
        .await?;
    Ok(())
}

fn is_backup_due(config: &BackupConfig) -> bool {
    let Some(last_run_at) = config.last_run_at.as_deref() else {
        return true;
    };
    let Ok(last) = chrono::DateTime::parse_from_rfc3339(last_run_at) else {
        return true;
    };
    let next = last + chrono::Duration::hours(config.interval_hours.max(1) as i64);
    chrono::Utc::now() >= next.with_timezone(&chrono::Utc)
}

pub fn should_auto_check_updates(config: &UpdateConfig) -> bool {
    config.auto_check
}

fn prune_old_backups(destination_root: &Path, keep_last: usize) -> Result<(), AppError> {
    let mut entries = std::fs::read_dir(destination_root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    if entries.len() <= keep_last {
        return Ok(());
    }
    let remove_count = entries.len() - keep_last;
    for entry in entries.into_iter().take(remove_count) {
        std::fs::remove_dir_all(entry.path())?;
    }
    Ok(())
}

async fn copy_dir_all_with_progress<F, Fut>(
    src: &Path,
    dst: &Path,
    mut on_progress: F,
) -> Result<(), AppError>
where
    F: FnMut(f64, &'static str) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut entries = Vec::new();
    collect_files(src, &mut entries)?;
    let total = entries.len().max(1) as f64;

    tokio::fs::create_dir_all(dst).await?;
    for (index, path) in entries.into_iter().enumerate() {
        let relative = path
            .strip_prefix(src)
            .map_err(|_| AppError::PathOutOfScope(path.to_string_lossy().to_string()))?;
        let target = dst.join(relative);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(&path, &target).await?;
        on_progress((index as f64 + 1.0) / total, "Copying workspace files").await;
    }
    Ok(())
}

fn collect_files(dir: &Path, acc: &mut Vec<PathBuf>) -> Result<(), AppError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, acc)?;
        } else {
            acc.push(path);
        }
    }
    Ok(())
}
