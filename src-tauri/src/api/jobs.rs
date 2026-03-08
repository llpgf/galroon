use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;

use crate::config::SharedConfig;
use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;

#[derive(Debug, Serialize)]
pub struct AppJobStatus {
    pub id: i64,
    pub kind: String,
    pub state: String,
    pub title: String,
    pub progress_pct: f64,
    pub current_step: Option<String>,
    pub last_error: Option<String>,
    pub result_json: Option<serde_json::Value>,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_cancel: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct EnrichmentQueueStatus {
    pub paused: bool,
    pub queued: i64,
    pub running: i64,
    pub retry_wait: i64,
    pub failed: i64,
    pub success: i64,
    pub total_pending: i64,
}

#[derive(Debug, Serialize)]
pub struct BackupScheduleStatus {
    pub enabled: bool,
    pub interval_hours: u32,
    pub destination_dir: Option<String>,
    pub keep_last: u32,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSettingsStatus {
    pub auto_check: bool,
    pub repo_owner: String,
    pub repo_name: String,
    pub channel: String,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NativeUpdateCheckStatus {
    pub current_version: String,
    pub release_version: Option<String>,
    pub release_name: Option<String>,
    pub release_notes: Option<String>,
    pub release_url: Option<String>,
    pub checked_at: String,
    pub compatible_package_available: bool,
    pub install_version: Option<String>,
    pub install_target: Option<String>,
    pub manifest_endpoint: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct NativeUpdateProgressEvent {
    pub phase: String,
    pub downloaded: usize,
    pub total: Option<u64>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct BackupScheduleInput {
    pub enabled: Option<bool>,
    pub interval_hours: Option<u32>,
    pub destination_dir: Option<String>,
    pub keep_last: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsInput {
    pub auto_check: Option<bool>,
    pub repo_owner: Option<String>,
    pub repo_name: Option<String>,
    pub channel: Option<String>,
}

#[tauri::command]
pub async fn list_app_jobs(db: State<'_, Database>, limit: Option<i64>) -> Result<Vec<AppJobStatus>, AppError> {
    let rows = queries::app_jobs::list_jobs(db.read_pool(), limit.unwrap_or(20).clamp(1, 100)).await?;
    Ok(rows.into_iter().map(map_job_status).collect())
}

#[tauri::command]
pub async fn pause_app_job(db: State<'_, Database>, job_id: i64) -> Result<(), AppError> {
    queries::app_jobs::pause_job(db.read_pool(), job_id).await
}

#[tauri::command]
pub async fn resume_app_job(db: State<'_, Database>, job_id: i64) -> Result<(), AppError> {
    queries::app_jobs::resume_job(db.read_pool(), job_id).await
}

#[tauri::command]
pub async fn cancel_app_job(db: State<'_, Database>, job_id: i64) -> Result<(), AppError> {
    queries::app_jobs::cancel_job(db.read_pool(), job_id).await
}

#[tauri::command]
pub async fn enqueue_backup_job(
    db: State<'_, Database>,
    config: State<'_, SharedConfig>,
    destination_dir: Option<String>,
) -> Result<i64, AppError> {
    let snapshot = config.snapshot().await;
    let target = destination_dir
        .or(snapshot.backups.destination_dir.clone())
        .ok_or_else(|| AppError::Validation("Backup destination is not configured".to_string()))?;
    let payload = serde_json::json!({
        "destination_dir": target,
        "keep_last": snapshot.backups.keep_last,
    });
    queries::app_jobs::enqueue_job(
        db.read_pool(),
        "workspace_backup",
        "Backup workspace",
        Some(&payload),
        None,
        true,
        true,
        true,
    )
    .await
}

#[tauri::command]
pub async fn enqueue_update_check(db: State<'_, Database>) -> Result<i64, AppError> {
    queries::app_jobs::enqueue_job(
        db.read_pool(),
        "update_check",
        "Check for updates",
        None,
        Some("update:check"),
        false,
        false,
        true,
    )
    .await
}

#[tauri::command]
pub async fn enqueue_library_enrichment(db: State<'_, Database>) -> Result<serde_json::Value, AppError> {
    let works = queries::canonical::list_canonical_works(db.read_pool(), "title", false, None).await?;
    let mut count = 0_i64;
    for work in works {
        let work_id = work.id.to_string();
        let dedup_key = format!("refresh:{work_id}");
        let _ = queries::jobs::enqueue_job(
            db.read_pool(),
            &work_id,
            "metadata_refresh",
            Some(&dedup_key),
            None,
        )
        .await?;
        count += 1;
    }
    Ok(serde_json::json!({ "queued": count }))
}

#[tauri::command]
pub async fn get_enrichment_queue_status(db: State<'_, Database>) -> Result<EnrichmentQueueStatus, AppError> {
    let stats = queries::jobs::job_stats(db.read_pool()).await?;
    let paused = queries::app_jobs::get_runtime_flag(db.read_pool(), "enrichment_paused")
        .await?
        .is_some_and(|value| value == "true");
    let mut status = EnrichmentQueueStatus {
        paused,
        queued: 0,
        running: 0,
        retry_wait: 0,
        failed: 0,
        success: 0,
        total_pending: 0,
    };
    for (state, count) in stats {
        match state.as_str() {
            "queued" => status.queued = count,
            "claimed" | "running" => status.running += count,
            "retry_wait" => status.retry_wait = count,
            "failed" => status.failed = count,
            "success" => status.success = count,
            _ => {}
        }
    }
    status.total_pending = status.queued + status.running + status.retry_wait;
    Ok(status)
}

#[tauri::command]
pub async fn pause_enrichment_queue(db: State<'_, Database>) -> Result<(), AppError> {
    queries::app_jobs::set_runtime_flag(db.read_pool(), "enrichment_paused", "true").await
}

#[tauri::command]
pub async fn resume_enrichment_queue(db: State<'_, Database>) -> Result<(), AppError> {
    queries::app_jobs::set_runtime_flag(db.read_pool(), "enrichment_paused", "false").await
}

#[tauri::command]
pub async fn get_backup_schedule(config: State<'_, SharedConfig>) -> Result<BackupScheduleStatus, AppError> {
    let snapshot = config.snapshot().await;
    Ok(BackupScheduleStatus {
        enabled: snapshot.backups.enabled,
        interval_hours: snapshot.backups.interval_hours,
        destination_dir: snapshot.backups.destination_dir,
        keep_last: snapshot.backups.keep_last,
        last_run_at: snapshot.backups.last_run_at,
    })
}

#[tauri::command]
pub async fn update_backup_schedule(
    config: State<'_, SharedConfig>,
    schedule: BackupScheduleInput,
) -> Result<BackupScheduleStatus, AppError> {
    config
        .update(|cfg| {
            if let Some(enabled) = schedule.enabled {
                cfg.backups.enabled = enabled;
            }
            if let Some(interval) = schedule.interval_hours {
                cfg.backups.interval_hours = interval.max(1);
            }
            if let Some(destination) = schedule.destination_dir.clone() {
                cfg.backups.destination_dir = Some(destination);
            }
            if let Some(keep_last) = schedule.keep_last {
                cfg.backups.keep_last = keep_last.max(1);
            }
        })
        .await?;
    get_backup_schedule(config).await
}

#[tauri::command]
pub async fn get_update_settings(config: State<'_, SharedConfig>) -> Result<UpdateSettingsStatus, AppError> {
    let snapshot = config.snapshot().await;
    Ok(UpdateSettingsStatus {
        auto_check: snapshot.updates.auto_check,
        repo_owner: snapshot.updates.repo_owner,
        repo_name: snapshot.updates.repo_name,
        channel: snapshot.updates.channel,
        last_checked_at: snapshot.updates.last_checked_at,
    })
}

#[tauri::command]
pub async fn update_update_settings(
    config: State<'_, SharedConfig>,
    updates: UpdateSettingsInput,
) -> Result<UpdateSettingsStatus, AppError> {
    config
        .update(|cfg| {
            if let Some(auto_check) = updates.auto_check {
                cfg.updates.auto_check = auto_check;
            }
            if let Some(repo_owner) = updates.repo_owner.clone() {
                cfg.updates.repo_owner = repo_owner;
            }
            if let Some(repo_name) = updates.repo_name.clone() {
                cfg.updates.repo_name = repo_name;
            }
            if let Some(channel) = updates.channel.clone() {
                cfg.updates.channel = channel;
            }
        })
        .await?;
    get_update_settings(config).await
}

#[tauri::command]
pub async fn check_native_update(
    app: AppHandle,
    config: State<'_, SharedConfig>,
) -> Result<NativeUpdateCheckStatus, AppError> {
    let snapshot = config.snapshot().await;
    let checked_at = chrono::Utc::now().to_rfc3339();
    let manifest_endpoint = build_updater_manifest_endpoint(&snapshot.updates);
    let release = fetch_latest_release_metadata(&snapshot.updates).await?;

    config
        .update(|cfg| {
            cfg.updates.last_checked_at = Some(checked_at.clone());
        })
        .await?;

    let release_version = release
        .get("tag_name")
        .and_then(|value| value.as_str())
        .map(|value| value.trim_start_matches('v').to_string());
    let release_name = release
        .get("name")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let release_notes = release
        .get("body")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let release_url = release
        .get("html_url")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);

    let updater_probe = build_runtime_updater(&app, &snapshot.updates)
        .and_then(|builder| builder.build().map_err(|e| AppError::Internal(e.to_string())));

    let (compatible_package_available, install_version, install_target, message) =
        match updater_probe {
            Ok(updater) => match updater.check().await {
                Ok(Some(update)) => (
                    true,
                    Some(update.version),
                    Some(update.target),
                    format!("Compatible signed updater package is available via {}", manifest_endpoint),
                ),
                Ok(None) => (
                    false,
                    None,
                    None,
                    "No compatible signed updater package was found for this installed target".to_string(),
                ),
                Err(error) => (
                    false,
                    None,
                    None,
                    format!("GitHub release found, but updater manifest was not installable: {}", error),
                ),
            },
            Err(error) => (
                false,
                None,
                None,
                format!("Updater runtime is not ready: {}", error),
            ),
        };

    Ok(NativeUpdateCheckStatus {
        current_version: app.package_info().version.to_string(),
        release_version,
        release_name,
        release_notes,
        release_url,
        checked_at,
        compatible_package_available,
        install_version,
        install_target,
        manifest_endpoint,
        message,
    })
}

#[tauri::command]
pub async fn install_native_update(
    app: AppHandle,
    config: State<'_, SharedConfig>,
) -> Result<(), AppError> {
    let snapshot = config.snapshot().await;
    let updater = build_runtime_updater(&app, &snapshot.updates)?
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("No compatible updater package is currently available".to_string()))?;

    let _ = app.emit(
        "native-update-progress",
        NativeUpdateProgressEvent {
            phase: "starting".to_string(),
            downloaded: 0,
            total: None,
            message: format!("Preparing update {}", update.version),
        },
    );

    let app_handle = app.clone();
    let mut downloaded_total = 0_usize;
    update
        .download_and_install(
            move |chunk_length, total| {
                downloaded_total += chunk_length;
                let _ = app_handle.emit(
                    "native-update-progress",
                    NativeUpdateProgressEvent {
                        phase: "downloading".to_string(),
                        downloaded: downloaded_total,
                        total,
                        message: match total {
                            Some(total) => format!("Downloading signed updater package ({}/{})", downloaded_total, total),
                            None => format!("Downloading signed updater package (+{} bytes)", chunk_length),
                        },
                    },
                );
            },
            {
                let app_handle = app.clone();
                move || {
                    let _ = app_handle.emit(
                        "native-update-progress",
                        NativeUpdateProgressEvent {
                            phase: "installing".to_string(),
                            downloaded: 0,
                            total: None,
                            message: "Download finished. Launching installer…".to_string(),
                        },
                    );
                }
            },
        )
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    Ok(())
}

fn map_job_status(row: crate::db::models::AppJobRow) -> AppJobStatus {
    AppJobStatus {
        id: row.id,
        kind: row.kind,
        state: row.state,
        title: row.title,
        progress_pct: row.progress_pct,
        current_step: row.current_step,
        last_error: row.last_error,
        result_json: row
            .result_json
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok()),
        can_pause: row.can_pause != 0,
        can_resume: row.can_resume != 0,
        can_cancel: row.can_cancel != 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn build_updater_manifest_endpoint(updates: &crate::config::UpdateConfig) -> String {
    let asset_name = build_updater_manifest_name(&updates.channel);
    format!(
        "https://github.com/{}/{}/releases/latest/download/{}",
        updates.repo_owner, updates.repo_name, asset_name
    )
}

fn build_updater_manifest_name(channel: &str) -> String {
    let channel = channel.trim();
    if channel.is_empty() || channel.eq_ignore_ascii_case("stable") {
        "latest.json".to_string()
    } else {
        let normalized = channel
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect::<String>();
        format!("latest-{}.json", normalized.to_lowercase())
    }
}

fn build_runtime_updater(
    app: &AppHandle,
    updates: &crate::config::UpdateConfig,
) -> Result<tauri_plugin_updater::UpdaterBuilder, AppError> {
    let endpoint = build_updater_manifest_endpoint(updates);
    let parsed = endpoint
        .parse()
        .map_err(|e| AppError::Validation(format!("Invalid updater endpoint: {}", e)))?;
    app.updater_builder()
        .endpoints(vec![parsed])
        .map_err(|e| AppError::Validation(format!("Invalid updater endpoint configuration: {}", e)))
}

async fn fetch_latest_release_metadata(
    updates: &crate::config::UpdateConfig,
) -> Result<serde_json::Value, AppError> {
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

    if stable_channel {
        return Ok(payload);
    }

    select_release_for_channel(payload, &updates.channel)
        .ok_or_else(|| AppError::NotFound(format!("No GitHub release matched channel '{}'", updates.channel)))
}

fn select_release_for_channel(payload: serde_json::Value, channel: &str) -> Option<serde_json::Value> {
    let releases = payload.as_array()?;
    let lowered = channel.to_lowercase();
    releases
        .iter()
        .find(|release| {
            release.get("prerelease").and_then(|value| value.as_bool()).unwrap_or(false)
                && release
                    .get("tag_name")
                    .and_then(|value| value.as_str())
                    .map(|tag| tag.to_lowercase().contains(&lowered))
                    .unwrap_or(true)
        })
        .cloned()
        .or_else(|| releases.first().cloned())
}
