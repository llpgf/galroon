//! Settings API — config + workspace management + trash.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::{AiProviderConfig, AppConfig, BangumiAuthConfig, LauncherConfig, SharedConfig};
use crate::domain::error::{AppError, AppResult};
use crate::enrichment::bangumi::BangumiClient;
use crate::fs::trash;

const BANGUMI_OAUTH_PORT: u16 = 48573;
const BANGUMI_OAUTH_PATH: &str = "/bangumi/callback";
const BANGUMI_OAUTH_TIMEOUT_SECS: u64 = 300;

// ── Workspace first-launch ─────────────────────────────

#[derive(Serialize)]
pub struct WorkspaceStatus {
    pub has_workspace: bool,
    pub workspace_path: Option<String>,
    pub recent_workspaces: Vec<String>,
}

/// Check workspace status — uses setup_complete flag, not just path existence.
#[tauri::command]
pub async fn check_workspace_status() -> Result<WorkspaceStatus, AppError> {
    let launcher = LauncherConfig::load()?;

    Ok(WorkspaceStatus {
        has_workspace: launcher.setup_complete,
        workspace_path: launcher
            .last_workspace
            .map(|p| p.to_string_lossy().to_string()),
        recent_workspaces: launcher
            .recent_workspaces
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
    })
}

/// Init workspace — called from Welcome screen. Marks setup_complete = true.
#[tauri::command]
pub async fn init_workspace(path: String) -> Result<String, AppError> {
    let ws_path = PathBuf::from(&path);
    if !AppConfig::is_workspace(&ws_path) {
        AppConfig::init_workspace(&ws_path)?;
    }
    let mut launcher = LauncherConfig::load()?;
    launcher.set_workspace(ws_path);
    launcher.setup_complete = true;
    launcher.save()?;
    Ok(path)
}

// ── Workspace info ─────────────────────────────────────

#[derive(Serialize)]
pub struct WorkspaceInfo {
    pub workspace_path: String,
    pub db_path: String,
    pub thumbnail_dir: String,
    pub log_dir: String,
    pub trash_dir: String,
    pub db_size_bytes: u64,
    pub thumbnail_count: u32,
    pub trash_count: u32,
}

#[tauri::command]
pub async fn get_workspace_info(
    config: State<'_, SharedConfig>,
) -> Result<WorkspaceInfo, AppError> {
    let cfg = config.read().await;

    let db_size = std::fs::metadata(&cfg.db_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let thumb_count = count_files(&cfg.thumbnail_dir);
    let trash_count = count_files(&cfg.trash_dir);

    Ok(WorkspaceInfo {
        workspace_path: cfg.workspace_dir.to_string_lossy().to_string(),
        db_path: cfg.db_path.to_string_lossy().to_string(),
        thumbnail_dir: cfg.thumbnail_dir.to_string_lossy().to_string(),
        log_dir: cfg.log_dir.to_string_lossy().to_string(),
        trash_dir: cfg.trash_dir.to_string_lossy().to_string(),
        db_size_bytes: db_size,
        thumbnail_count: thumb_count,
        trash_count: trash_count,
    })
}

#[tauri::command]
pub async fn get_recent_workspaces() -> Result<Vec<String>, AppError> {
    let launcher = LauncherConfig::load()?;
    Ok(launcher
        .recent_workspaces
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

// ── Workspace management (relocate / backup) ──────────

/// Relocate workspace to a new path (copies all data).
#[tauri::command]
pub async fn relocate_workspace(
    config: State<'_, SharedConfig>,
    new_path: String,
) -> Result<String, AppError> {
    let cfg = config.read().await;
    let old_dir = cfg.workspace_dir.clone();
    let new_dir = PathBuf::from(&new_path);
    drop(cfg);

    if new_dir.exists()
        && new_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(AppError::Internal(
            "Target directory is not empty".to_string(),
        ));
    }

    // Copy entire workspace
    copy_dir_all(&old_dir, &new_dir)?;

    // Update launcher to point to new location
    let mut launcher = LauncherConfig::load()?;
    launcher.set_workspace(new_dir.clone());
    launcher.setup_complete = true;
    launcher.save()?;

    tracing::info!(
        from = %old_dir.display(),
        to = %new_dir.display(),
        "Workspace relocated — restart required"
    );

    Ok(new_path)
}

/// Create a backup of the workspace (copies to target dir).
#[tauri::command]
pub async fn backup_workspace(
    config: State<'_, SharedConfig>,
    backup_path: String,
) -> Result<String, AppError> {
    let cfg = config.read().await;
    let ws_dir = cfg.workspace_dir.clone();
    drop(cfg);

    let target = PathBuf::from(&backup_path);
    copy_dir_all(&ws_dir, &target)?;

    tracing::info!(to = %target.display(), "Workspace backup created");
    Ok(backup_path)
}

// ── Trash management ───────────────────────────────────

#[derive(Serialize)]
pub struct TrashItem {
    pub name: String,
    pub size_bytes: u64,
    pub age_days: u32,
    pub is_dir: bool,
}

/// List items in workspace .trash/.
#[tauri::command]
pub async fn list_trash(config: State<'_, SharedConfig>) -> Result<Vec<TrashItem>, AppError> {
    let cfg = config.read().await;
    let items = trash::list_workspace_trash(&cfg.trash_dir)?;
    Ok(items
        .into_iter()
        .map(|i| TrashItem {
            name: i.name,
            size_bytes: i.size,
            age_days: i.age_days,
            is_dir: i.is_dir,
        })
        .collect())
}

/// Purge trash items older than N days.
#[tauri::command]
pub async fn purge_trash(
    config: State<'_, SharedConfig>,
    retention_days: Option<u32>,
) -> Result<u32, AppError> {
    let cfg = config.read().await;
    let count = trash::purge_old_trash(&cfg.trash_dir, retention_days.unwrap_or(30))?;
    Ok(count as u32)
}

/// Empty all trash.
#[tauri::command]
pub async fn empty_trash(config: State<'_, SharedConfig>) -> Result<u32, AppError> {
    let cfg = config.read().await;
    let count = trash::purge_old_trash(&cfg.trash_dir, 0)?;
    Ok(count as u32)
}

// ── Settings CRUD ──────────────────────────────────────

#[derive(Serialize)]
struct SafeSettings {
    library_roots: Vec<String>,
    theme: String,
    locale: String,
}

#[derive(Serialize)]
pub struct BangumiAuthStatus {
    pub connected: bool,
    pub has_access_token: bool,
    pub has_app_id: bool,
    pub has_app_secret: bool,
    pub token_hint: Option<String>,
    pub app_id_hint: Option<String>,
}

#[derive(Deserialize)]
pub struct BangumiAuthInput {
    pub access_token: Option<String>,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

#[derive(Serialize)]
pub struct AiProviderStatus {
    pub configured: bool,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiProviderProbeResult {
    pub ok: bool,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub message: String,
    pub models: Vec<String>,
}

#[derive(Deserialize)]
pub struct AiProviderInput {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BangumiProbeResult {
    pub connected: bool,
    pub username: String,
    pub nickname: Option<String>,
    pub user_id: u64,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BangumiOAuthFlowStatus {
    pub phase: String,
    pub authorize_url: Option<String>,
    pub callback_url: Option<String>,
    pub message: Option<String>,
    pub probe: Option<BangumiProbeResult>,
}

impl Default for BangumiOAuthFlowStatus {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            authorize_url: None,
            callback_url: None,
            message: None,
            probe: None,
        }
    }
}

#[derive(Default, Clone)]
pub struct BangumiOAuthManager {
    inner: Arc<RwLock<BangumiOAuthRuntime>>,
}

#[derive(Clone, Default)]
struct BangumiOAuthRuntime {
    session_id: Option<String>,
    status: BangumiOAuthFlowStatus,
}

impl BangumiOAuthManager {
    async fn status(&self) -> BangumiOAuthFlowStatus {
        self.inner.read().await.status.clone()
    }

    async fn begin(&self, session_id: String, status: BangumiOAuthFlowStatus) {
        let mut inner = self.inner.write().await;
        inner.session_id = Some(session_id);
        inner.status = status;
    }

    async fn cancel(&self, message: &str) -> BangumiOAuthFlowStatus {
        let mut inner = self.inner.write().await;
        inner.session_id = None;
        inner.status = BangumiOAuthFlowStatus {
            phase: "cancelled".to_string(),
            message: Some(message.to_string()),
            ..BangumiOAuthFlowStatus::default()
        };
        inner.status.clone()
    }

    async fn update_if_current(&self, session_id: &str, status: BangumiOAuthFlowStatus) -> bool {
        let mut inner = self.inner.write().await;
        if inner.session_id.as_deref() != Some(session_id) {
            return false;
        }
        inner.status = status;
        true
    }

    async fn finish_if_current(&self, session_id: &str, status: BangumiOAuthFlowStatus) -> bool {
        let mut inner = self.inner.write().await;
        if inner.session_id.as_deref() != Some(session_id) {
            return false;
        }
        inner.session_id = None;
        inner.status = status;
        true
    }

    async fn is_current(&self, session_id: &str) -> bool {
        self.inner.read().await.session_id.as_deref() == Some(session_id)
    }
}

#[tauri::command]
pub async fn get_settings(config: State<'_, SharedConfig>) -> Result<serde_json::Value, AppError> {
    let cfg = config.read().await;
    Ok(serde_json::to_value(SafeSettings {
        library_roots: cfg
            .library_roots
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        theme: cfg.theme.clone(),
        locale: cfg.locale.clone(),
    })?)
}

#[tauri::command]
pub async fn update_settings(
    config: State<'_, SharedConfig>,
    settings: serde_json::Value,
) -> Result<(), AppError> {
    if let Some(roots) = settings.get("library_roots") {
        if let Some(arr) = roots.as_array() {
            let new_roots: Vec<PathBuf> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect();

            config
                .update(|cfg| {
                    cfg.library_roots = new_roots;
                })
                .await?;

            tracing::info!("Library roots updated");
        }
    }

    if let Some(locale) = settings.get("locale").and_then(|value| value.as_str()) {
        config
            .update(|cfg| {
                cfg.locale = locale.to_string();
            })
            .await?;
    }

    if let Some(theme) = settings.get("theme").and_then(|value| value.as_str()) {
        config
            .update(|cfg| {
                cfg.theme = theme.to_string();
            })
            .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_bangumi_auth_status(
    config: State<'_, SharedConfig>,
) -> Result<BangumiAuthStatus, AppError> {
    let cfg = config.read().await;
    Ok(build_bangumi_auth_status(cfg.bangumi.as_ref()))
}

#[tauri::command]
pub async fn update_bangumi_auth(
    config: State<'_, SharedConfig>,
    bangumi: BangumiAuthInput,
    client: State<'_, BangumiClient>,
) -> Result<BangumiAuthStatus, AppError> {
    let existing = {
        let cfg = config.read().await;
        cfg.bangumi.clone()
    };

    let merged = merge_bangumi_auth(existing, bangumi)?;

    config
        .update(|cfg| {
            cfg.bangumi = Some(merged.clone());
        })
        .await?;

    client.update_auth(Some(merged.clone())).await;

    Ok(build_bangumi_auth_status(Some(&merged)))
}

#[tauri::command]
pub async fn clear_bangumi_auth(
    config: State<'_, SharedConfig>,
    client: State<'_, BangumiClient>,
) -> Result<(), AppError> {
    config
        .update(|cfg| {
            cfg.bangumi = None;
        })
        .await?;

    client.update_auth(None).await;
    Ok(())
}

#[tauri::command]
pub async fn get_ai_provider_status(
    config: State<'_, SharedConfig>,
) -> Result<AiProviderStatus, AppError> {
    let cfg = config.read().await;
    Ok(build_ai_provider_status(cfg.ai.as_ref()))
}

#[tauri::command]
pub async fn update_ai_provider_settings(
    config: State<'_, SharedConfig>,
    ai: AiProviderInput,
) -> Result<AiProviderStatus, AppError> {
    let existing = {
        let cfg = config.read().await;
        cfg.ai.clone()
    };

    let merged = merge_ai_provider(existing, ai)?;
    config
        .update(|cfg| {
            cfg.ai = Some(merged.clone());
        })
        .await?;

    Ok(build_ai_provider_status(Some(&merged)))
}

#[tauri::command]
pub async fn clear_ai_provider_settings(config: State<'_, SharedConfig>) -> Result<(), AppError> {
    config
        .update(|cfg| {
            cfg.ai = None;
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn probe_ai_provider(
    config: State<'_, SharedConfig>,
) -> Result<AiProviderProbeResult, AppError> {
    let ai = {
        let cfg = config.read().await;
        cfg.ai.clone()
    }
    .ok_or_else(|| AppError::Validation("AI gateway is not configured".to_string()))?;

    let base_url = ai.base_url.trim_end_matches('/').to_string();
    let provider = ai.provider.clone();
    let mut client = reqwest::Client::builder();
    if provider == "ollama" {
        client = client.no_proxy();
    }
    let client = client
        .build()
        .map_err(|error| AppError::Network(error.to_string()))?;

    if provider == "ollama" {
        let root = base_url.trim_end_matches("/v1").to_string();
        let response = client
            .get(format!("{}/api/tags", root))
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?
            .error_for_status()
            .map_err(|error| AppError::Network(error.to_string()))?;
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let models = payload
            .get("models")
            .and_then(|value| value.as_array())
            .map(|items| {
                items.iter()
                    .filter_map(|item| item.get("name").and_then(|value| value.as_str()))
                    .take(8)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        return Ok(AiProviderProbeResult {
            ok: true,
            provider,
            base_url,
            model: ai.model,
            message: if models.is_empty() {
                "Connected to Ollama, but no local models were reported".to_string()
            } else {
                format!("Connected to Ollama. Found {} local models", models.len())
            },
            models,
        });
    }

    let mut request = client.get(format!("{}/models", base_url));
    if let Some(key) = ai
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.bearer_auth(key);
    }
    if provider == "openrouter" {
        request = request
            .header("HTTP-Referer", "https://galroon.app")
            .header("X-Title", "Galroon");
    }

    let response = request
        .send()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?
        .error_for_status()
        .map_err(|error| AppError::Network(error.to_string()))?;
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?;
    let models = payload
        .get("data")
        .and_then(|value| value.as_array())
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("id").and_then(|value| value.as_str()))
                .take(8)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(AiProviderProbeResult {
        ok: true,
        provider,
        base_url,
        model: ai.model,
        message: if models.is_empty() {
            "Connected to the AI gateway, but it did not return any model IDs".to_string()
        } else {
            format!("Connected to the AI gateway. Found {} model entries", models.len())
        },
        models,
    })
}

#[tauri::command]
pub async fn start_bangumi_oauth(
    config: State<'_, SharedConfig>,
    client: State<'_, BangumiClient>,
    oauth: State<'_, BangumiOAuthManager>,
) -> Result<BangumiOAuthFlowStatus, AppError> {
    let auth = {
        let cfg = config.read().await;
        cfg.bangumi.clone().unwrap_or_default()
    };

    let app_id = auth
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("Bangumi App ID is required before OAuth login".to_string())
        })?
        .to_string();
    let app_secret = auth
        .app_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("Bangumi App Secret is required before OAuth login".to_string())
        })?
        .to_string();

    let callback_url = build_bangumi_callback_url();
    let listener = TcpListener::bind(("127.0.0.1", BANGUMI_OAUTH_PORT))
        .await
        .map_err(|e| {
            AppError::Network(format!(
                "Failed to bind Bangumi callback on {}: {}",
                callback_url, e
            ))
        })?;

    let session_id = Uuid::new_v4().to_string();
    let authorize_url = build_bangumi_authorize_url(&app_id, &callback_url, &session_id);
    let initial_status = BangumiOAuthFlowStatus {
        phase: "waiting_browser".to_string(),
        authorize_url: Some(authorize_url.clone()),
        callback_url: Some(callback_url.clone()),
        message: Some(
            "Browser login started. Complete Bangumi authorization in the opened page.".to_string(),
        ),
        probe: None,
    };

    oauth
        .begin(session_id.clone(), initial_status.clone())
        .await;

    let oauth_manager = (*oauth).clone();
    let shared_config = (*config).clone();
    let bangumi_client = (*client).clone();

    tokio::spawn(async move {
        run_bangumi_oauth_flow(
            oauth_manager,
            shared_config,
            bangumi_client,
            listener,
            session_id,
            app_id,
            app_secret,
            callback_url,
        )
        .await;
    });

    Ok(initial_status)
}

#[tauri::command]
pub async fn get_bangumi_oauth_status(
    oauth: State<'_, BangumiOAuthManager>,
) -> Result<BangumiOAuthFlowStatus, AppError> {
    Ok(oauth.status().await)
}

#[tauri::command]
pub async fn cancel_bangumi_oauth(
    oauth: State<'_, BangumiOAuthManager>,
) -> Result<BangumiOAuthFlowStatus, AppError> {
    Ok(oauth.cancel("Bangumi OAuth login cancelled").await)
}

#[tauri::command]
pub async fn probe_bangumi_auth(
    config: State<'_, SharedConfig>,
    client: State<'_, BangumiClient>,
) -> Result<BangumiProbeResult, AppError> {
    probe_or_refresh_bangumi_auth(&config, &client).await
}

async fn run_bangumi_oauth_flow(
    oauth: BangumiOAuthManager,
    config: SharedConfig,
    client: BangumiClient,
    listener: TcpListener,
    session_id: String,
    app_id: String,
    app_secret: String,
    callback_url: String,
) {
    let waiting_status = BangumiOAuthFlowStatus {
        phase: "waiting_callback".to_string(),
        authorize_url: Some(build_bangumi_authorize_url(
            &app_id,
            &callback_url,
            &session_id,
        )),
        callback_url: Some(callback_url.clone()),
        message: Some("Waiting for Bangumi browser callback on localhost…".to_string()),
        probe: None,
    };
    let _ = oauth.update_if_current(&session_id, waiting_status).await;

    let accept_result = tokio::time::timeout(
        Duration::from_secs(BANGUMI_OAUTH_TIMEOUT_SECS),
        listener.accept(),
    )
    .await;
    let (mut stream, _) = match accept_result {
        Ok(Ok(pair)) => pair,
        Ok(Err(error)) => {
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "error".to_string(),
                        message: Some(format!("Bangumi OAuth callback failed: {}", error)),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
        Err(_) => {
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "timeout".to_string(),
                        message: Some(
                            "Bangumi OAuth timed out. Start login again if needed.".to_string(),
                        ),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
    };

    let request_target = match read_http_request_target(&mut stream).await {
        Ok(target) => target,
        Err(error) => {
            let _ = write_http_html_response(
                &mut stream,
                400,
                "Bad Request",
                build_oauth_error_page("Invalid OAuth callback request."),
            )
            .await;
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "error".to_string(),
                        message: Some(error.to_string()),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
    };

    if !request_target.starts_with(BANGUMI_OAUTH_PATH) {
        let _ = write_http_html_response(
            &mut stream,
            404,
            "Not Found",
            build_oauth_error_page("Unknown Bangumi OAuth callback path."),
        )
        .await;
        let _ = oauth
            .finish_if_current(
                &session_id,
                BangumiOAuthFlowStatus {
                    phase: "error".to_string(),
                    message: Some(format!(
                        "Unexpected OAuth callback path: {}",
                        request_target
                    )),
                    ..BangumiOAuthFlowStatus::default()
                },
            )
            .await;
        return;
    }

    let returned_state = extract_query_param(&request_target, "state");
    if returned_state.as_deref() != Some(session_id.as_str()) {
        let _ = write_http_html_response(
            &mut stream,
            400,
            "State Mismatch",
            build_oauth_error_page("Bangumi OAuth state mismatch. Close this tab and try again."),
        )
        .await;
        let _ = oauth
            .finish_if_current(
                &session_id,
                BangumiOAuthFlowStatus {
                    phase: "error".to_string(),
                    message: Some("Bangumi OAuth state mismatch".to_string()),
                    ..BangumiOAuthFlowStatus::default()
                },
            )
            .await;
        return;
    }

    let code = match extract_query_param(&request_target, "code") {
        Some(code) if !code.is_empty() => code,
        _ => {
            let _ = write_http_html_response(
                &mut stream,
                400,
                "Missing Code",
                build_oauth_error_page(
                    "Bangumi OAuth callback did not include an authorization code.",
                ),
            )
            .await;
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "error".to_string(),
                        message: Some("Bangumi OAuth callback missing code".to_string()),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
    };

    if !oauth.is_current(&session_id).await {
        let _ = write_http_html_response(
            &mut stream,
            409,
            "Cancelled",
            build_oauth_error_page("Bangumi OAuth was cancelled."),
        )
        .await;
        return;
    }

    let exchanging = BangumiOAuthFlowStatus {
        phase: "exchanging".to_string(),
        callback_url: Some(callback_url.clone()),
        message: Some("Bangumi callback received. Exchanging authorization code…".to_string()),
        ..BangumiOAuthFlowStatus::default()
    };
    let _ = oauth.update_if_current(&session_id, exchanging).await;

    let auth = match exchange_and_store_bangumi_oauth(
        &config,
        &client,
        &app_id,
        &app_secret,
        &code,
        &callback_url,
    )
    .await
    {
        Ok(auth) => auth,
        Err(error) => {
            let _ = write_http_html_response(
                &mut stream,
                500,
                "OAuth Failed",
                build_oauth_error_page(
                    "Bangumi token exchange failed. Return to Galroon and retry.",
                ),
            )
            .await;
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "error".to_string(),
                        message: Some(error.to_string()),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
    };

    let probe = match bangumi_probe_from_auth(&client, &auth).await {
        Ok(probe) => probe,
        Err(error) => {
            let _ = write_http_html_response(
                &mut stream,
                500,
                "OAuth Failed",
                build_oauth_error_page("Bangumi login succeeded but account verification failed."),
            )
            .await;
            let _ = oauth
                .finish_if_current(
                    &session_id,
                    BangumiOAuthFlowStatus {
                        phase: "error".to_string(),
                        message: Some(error.to_string()),
                        ..BangumiOAuthFlowStatus::default()
                    },
                )
                .await;
            return;
        }
    };

    let success_message = format!(
        "Connected as {}",
        probe
            .nickname
            .clone()
            .unwrap_or_else(|| probe.username.clone())
    );

    let _ = write_http_html_response(
        &mut stream,
        200,
        "Connected",
        build_oauth_success_page(&success_message),
    )
    .await;

    let _ = oauth
        .finish_if_current(
            &session_id,
            BangumiOAuthFlowStatus {
                phase: "success".to_string(),
                message: Some(success_message),
                probe: Some(probe),
                ..BangumiOAuthFlowStatus::default()
            },
        )
        .await;
}

async fn exchange_and_store_bangumi_oauth(
    config: &SharedConfig,
    client: &BangumiClient,
    app_id: &str,
    app_secret: &str,
    code: &str,
    callback_url: &str,
) -> Result<BangumiAuthConfig, AppError> {
    let token = client
        .exchange_oauth_code(app_id, app_secret, code, callback_url)
        .await
        .map_err(AppError::Network)?;

    let auth = BangumiAuthConfig {
        access_token: Some(token.access_token),
        refresh_token: token.refresh_token,
        expires_at: token.expires_in.map(|seconds| {
            (chrono::Utc::now() + chrono::Duration::seconds(seconds as i64)).to_rfc3339()
        }),
        app_id: Some(app_id.to_string()),
        app_secret: Some(app_secret.to_string()),
    };

    config
        .update(|cfg| {
            cfg.bangumi = Some(auth.clone());
        })
        .await?;

    client.update_auth(Some(auth.clone())).await;
    Ok(auth)
}

async fn refresh_and_store_bangumi_auth(
    config: &SharedConfig,
    client: &BangumiClient,
    existing: &BangumiAuthConfig,
) -> Result<Option<BangumiAuthConfig>, AppError> {
    let refresh_token = existing
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("Bangumi refresh token is not configured".to_string())
        })?;
    let app_id = existing
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("Bangumi App ID is not configured".to_string()))?;
    let app_secret = existing
        .app_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("Bangumi App Secret is not configured".to_string()))?;

    let token = client
        .refresh_oauth_token(
            app_id,
            app_secret,
            refresh_token,
            &build_bangumi_callback_url(),
        )
        .await
        .map_err(AppError::Network)?;

    let refreshed = BangumiAuthConfig {
        access_token: Some(token.access_token),
        refresh_token: token
            .refresh_token
            .or_else(|| existing.refresh_token.clone()),
        expires_at: token.expires_in.map(|seconds| {
            (chrono::Utc::now() + chrono::Duration::seconds(seconds as i64)).to_rfc3339()
        }),
        app_id: existing.app_id.clone(),
        app_secret: existing.app_secret.clone(),
    };

    config
        .update(|cfg| {
            cfg.bangumi = Some(refreshed.clone());
        })
        .await?;

    client.update_auth(Some(refreshed.clone())).await;
    Ok(Some(refreshed))
}

async fn probe_or_refresh_bangumi_auth(
    config: &SharedConfig,
    client: &BangumiClient,
) -> Result<BangumiProbeResult, AppError> {
    let auth = client.auth_snapshot().await;
    let has_token = auth
        .as_ref()
        .and_then(|value| value.access_token.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    if !has_token {
        return Err(AppError::Validation(
            "Bangumi access token is not configured".to_string(),
        ));
    }

    if let Some(me) = client.get_me().await.map_err(AppError::Network)? {
        return Ok(map_bangumi_probe(me));
    }

    let auth = auth.ok_or_else(|| {
        AppError::Validation("Bangumi access token is not configured".to_string())
    })?;
    let _ = refresh_and_store_bangumi_auth(config, client, &auth).await?;
    bangumi_probe_from_auth(client, &auth).await
}

async fn bangumi_probe_from_auth(
    client: &BangumiClient,
    _auth: &BangumiAuthConfig,
) -> Result<BangumiProbeResult, AppError> {
    let me = client
        .get_me()
        .await
        .map_err(AppError::Network)?
        .ok_or_else(|| AppError::Network("Bangumi token is invalid or lacks access".to_string()))?;
    Ok(map_bangumi_probe(me))
}

fn map_bangumi_probe(me: crate::enrichment::bangumi::BangumiMe) -> BangumiProbeResult {
    BangumiProbeResult {
        connected: true,
        username: me.username,
        nickname: me.nickname,
        user_id: me.id,
        avatar: me.avatar.as_ref().and_then(|images| {
            images
                .large
                .clone()
                .or(images.medium.clone())
                .or(images.small.clone())
        }),
    }
}

async fn read_http_request_target(stream: &mut tokio::net::TcpStream) -> Result<String, AppError> {
    let mut buffer = [0_u8; 8192];
    let bytes_read = stream.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Err(AppError::Network(
            "Bangumi OAuth callback connection closed early".to_string(),
        ));
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let line = request.lines().next().ok_or_else(|| {
        AppError::Network("Bangumi OAuth callback did not include a request line".to_string())
    })?;

    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();

    if method != "GET" || target.is_empty() {
        return Err(AppError::Network(format!(
            "Unsupported OAuth callback request: {}",
            line
        )));
    }

    Ok(target.to_string())
}

async fn write_http_html_response(
    stream: &mut tokio::net::TcpStream,
    status_code: u16,
    status_text: &str,
    body: String,
) -> Result<(), AppError> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        status_text,
        body.as_bytes().len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

fn build_bangumi_authorize_url(app_id: &str, callback_url: &str, state: &str) -> String {
    format!(
        "https://bgm.tv/oauth/authorize?client_id={}&response_type=code&redirect_uri={}&state={}",
        urlencoding_simple(app_id),
        urlencoding_simple(callback_url),
        urlencoding_simple(state)
    )
}

fn build_bangumi_callback_url() -> String {
    format!(
        "http://127.0.0.1:{}{}",
        BANGUMI_OAUTH_PORT, BANGUMI_OAUTH_PATH
    )
}

fn urlencoding_simple(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                for b in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

fn build_oauth_success_page(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Bangumi Connected</title><style>body{{font-family:Segoe UI,sans-serif;background:#0b0b12;color:#f6f6fb;display:grid;place-items:center;min-height:100vh;margin:0}}main{{max-width:560px;padding:32px;border:1px solid #2a2a3a;border-radius:18px;background:#13131d;box-shadow:0 24px 80px rgba(0,0,0,.35)}}h1{{margin:0 0 12px;font-size:28px}}p{{margin:0;color:#b6b6c9;line-height:1.6}}</style></head><body><main><h1>Bangumi connected</h1><p>{}</p><p>You can close this tab and return to Galroon.</p></main></body></html>",
        html_escape(message)
    )
}

fn build_oauth_error_page(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Bangumi OAuth Failed</title><style>body{{font-family:Segoe UI,sans-serif;background:#0b0b12;color:#f6f6fb;display:grid;place-items:center;min-height:100vh;margin:0}}main{{max-width:560px;padding:32px;border:1px solid #3a2228;border-radius:18px;background:#1a1115;box-shadow:0 24px 80px rgba(0,0,0,.35)}}h1{{margin:0 0 12px;font-size:28px;color:#ff8d9a}}p{{margin:0;color:#f0c4ca;line-height:1.6}}</style></head><body><main><h1>Bangumi login failed</h1><p>{}</p><p>Return to Galroon and try again.</p></main></body></html>",
        html_escape(message)
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── Helpers ────────────────────────────────────────────

fn count_files(dir: &std::path::Path) -> u32 {
    std::fs::read_dir(dir)
        .map(|entries| entries.count() as u32)
        .unwrap_or(0)
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> AppResult<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn merge_bangumi_auth(
    existing: Option<BangumiAuthConfig>,
    input: BangumiAuthInput,
) -> Result<BangumiAuthConfig, AppError> {
    let mut merged = existing.unwrap_or_default();

    if let Some(value) =
        normalize_optional_string(input.access_token).map(|value| extract_bangumi_token(&value))
    {
        merged.access_token = Some(value);
    }
    if let Some(value) = normalize_optional_string(input.app_id) {
        merged.app_id = Some(value);
    }
    if let Some(value) = normalize_optional_string(input.app_secret) {
        merged.app_secret = Some(value);
    }

    if merged
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(AppError::Validation(
            "Bangumi access token is required. Use Disconnect to clear stored auth.".to_string(),
        ));
    }

    Ok(merged)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}

fn extract_bangumi_token(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(token) = extract_query_param(trimmed, "access_token") {
        return token;
    }
    if let Some(token) = extract_query_param(trimmed, "token") {
        return token;
    }
    trimmed.to_string()
}

fn extract_query_param(value: &str, key: &str) -> Option<String> {
    let haystacks = [value, value.strip_prefix('#').unwrap_or(value)];
    for haystack in haystacks {
        for segment in haystack.split(['?', '#', '&']) {
            if let Some((name, raw)) = segment.split_once('=') {
                if name == key {
                    return Some(raw.to_string());
                }
            }
        }
    }
    None
}

fn build_bangumi_auth_status(auth: Option<&BangumiAuthConfig>) -> BangumiAuthStatus {
    let token = auth
        .and_then(|value| value.access_token.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let app_id = auth
        .and_then(|value| value.app_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let app_secret = auth
        .and_then(|value| value.app_secret.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    BangumiAuthStatus {
        connected: token.is_some(),
        has_access_token: token.is_some(),
        has_app_id: app_id.is_some(),
        has_app_secret: app_secret.is_some(),
        token_hint: token.map(mask_token),
        app_id_hint: app_id.map(mask_token),
    }
}

fn build_ai_provider_status(ai: Option<&AiProviderConfig>) -> AiProviderStatus {
    let current = ai.cloned().unwrap_or_default();
    let key = current
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    AiProviderStatus {
        configured: ai.is_some(),
        provider: current.provider,
        base_url: current.base_url,
        model: current.model,
        has_api_key: key.is_some(),
        api_key_hint: key.map(mask_token),
    }
}

fn merge_ai_provider(
    existing: Option<AiProviderConfig>,
    input: AiProviderInput,
) -> Result<AiProviderConfig, AppError> {
    let mut merged = existing.unwrap_or_default();

    if let Some(value) = normalize_optional_string(input.provider) {
        let normalized = value.to_lowercase();
        let allowed = [
            "litellm",
            "openai-compatible",
            "openai",
            "openrouter",
            "ollama",
        ];
        if !allowed.contains(&normalized.as_str()) {
            return Err(AppError::Validation(format!(
                "Unsupported AI provider: {}",
                value
            )));
        }
        merged.provider = normalized;
    }

    if let Some(value) = normalize_optional_string(input.base_url) {
        merged.base_url = value.trim_end_matches('/').to_string();
    }

    if let Some(value) = normalize_optional_string(input.model) {
        merged.model = value;
    }

    if let Some(value) = normalize_optional_string(input.api_key) {
        merged.api_key = Some(value);
    }

    Ok(merged)
}

fn mask_token(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "stored".to_string();
    }

    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}…{suffix}")
}

#[cfg(test)]
mod tests {
    use super::extract_bangumi_token;

    #[test]
    fn extract_bangumi_token_accepts_raw_token() {
        assert_eq!(extract_bangumi_token("abc123"), "abc123");
    }

    #[test]
    fn extract_bangumi_token_from_callback_url() {
        assert_eq!(
            extract_bangumi_token("https://example.com/callback?access_token=abc123&state=1"),
            "abc123"
        );
    }

    #[test]
    fn extract_bangumi_token_from_fragment() {
        assert_eq!(
            extract_bangumi_token("#access_token=abc123&token_type=bearer"),
            "abc123"
        );
    }
}
