//! Configuration — workspace-isolated architecture.
//!
//! Two layers:
//! - `LauncherConfig` (app-level): stored in OS app data dir, just tracks workspace paths
//! - `AppConfig` (workspace-level): stored inside the workspace folder, contains all settings
//!
//! Workspace folder is completely portable — backup = copy, restore = point app at folder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::domain::error::{AppError, AppResult};

// ── LauncherConfig (app-level, tiny) ───────────────────

/// Minimal app-level config — only stores workspace pointer.
/// Lives at `~/.config/galroon/launcher.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    pub last_workspace: Option<PathBuf>,
    #[serde(default)]
    pub recent_workspaces: Vec<PathBuf>,
    /// False on first launch — set to true when user explicitly picks a workspace.
    #[serde(default)]
    pub setup_complete: bool,
}

impl LauncherConfig {
    /// Load launcher config from OS app data directory.
    pub fn load() -> AppResult<Self> {
        let dir = Self::launcher_dir()?;
        let path = dir.join("launcher.toml");

        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| AppError::Config(format!("Failed to read launcher.toml: {}", e)))?;
            let config: LauncherConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(LauncherConfig {
                last_workspace: None,
                recent_workspaces: Vec::new(),
                setup_complete: false,
            })
        }
    }

    /// Save launcher config.
    pub fn save(&self) -> AppResult<()> {
        let dir = Self::launcher_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("launcher.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| AppError::Config(format!("Failed to serialize launcher config: {}", e)))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Record a workspace as the most recent.
    pub fn set_workspace(&mut self, path: PathBuf) {
        self.last_workspace = Some(path.clone());
        // Add to recent list, deduplicate
        self.recent_workspaces.retain(|p| p != &path);
        self.recent_workspaces.insert(0, path);
        // Keep only last 10
        self.recent_workspaces.truncate(10);
    }

    /// OS-level launcher config directory.
    fn launcher_dir() -> AppResult<PathBuf> {
        if let Ok(dir) = std::env::var("GALROON_LAUNCHER_PATH") {
            return Ok(PathBuf::from(dir));
        }
        let dirs = directories::ProjectDirs::from("com", "galroon", "Galroon")
            .ok_or_else(|| AppError::Config("Cannot determine app data directory".into()))?;
        Ok(dirs.data_dir().to_path_buf())
    }
}

// ── AppConfig (workspace-level) ────────────────────────

/// Application configuration — lives entirely inside the workspace folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// The workspace root directory (all data lives here)
    #[serde(skip)]
    pub workspace_dir: PathBuf,

    /// Directories containing game libraries
    pub library_roots: Vec<PathBuf>,

    /// Path to the SQLite database (derived from workspace_dir)
    #[serde(skip)]
    pub db_path: PathBuf,

    /// Path to log files (derived from workspace_dir)
    #[serde(skip)]
    pub log_dir: PathBuf,

    /// Path to trash directory (derived from workspace_dir)
    #[serde(skip)]
    pub trash_dir: PathBuf,

    /// Path to thumbnail cache (derived from workspace_dir)
    #[serde(skip)]
    pub thumbnail_dir: PathBuf,

    /// Scanner settings
    pub scanner: ScannerConfig,

    /// SFW mode — hide R18 covers and descriptions
    #[serde(default)]
    pub sfw_mode: bool,

    /// UI locale: "ja", "en", "zh-Hans", "zh-Hant"
    #[serde(default = "default_locale")]
    pub locale: String,

    /// UI theme mode: "system", "dark", "light"
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Optional Bangumi API credentials for R18-capable authenticated requests.
    #[serde(default)]
    pub bangumi: Option<BangumiAuthConfig>,

    /// Optional AI gateway / translation provider settings.
    #[serde(default)]
    pub ai: Option<AiProviderConfig>,

    /// Backup scheduling and retention policy.
    #[serde(default)]
    pub backups: BackupConfig,

    /// Update checking policy.
    #[serde(default)]
    pub updates: UpdateConfig,
}

fn default_locale() -> String {
    "ja".to_string()
}

fn default_theme() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BangumiAuthConfig {
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProviderConfig {
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    #[serde(default = "default_ai_base_url")]
    pub base_url: String,
    #[serde(default = "default_ai_model")]
    pub model: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_backup_interval_hours")]
    pub interval_hours: u32,
    #[serde(default)]
    pub destination_dir: Option<String>,
    #[serde(default = "default_backup_keep_last")]
    pub keep_last: u32,
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_hours: default_backup_interval_hours(),
            destination_dir: None,
            keep_last: default_backup_keep_last(),
            last_run_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default)]
    pub auto_check: bool,
    #[serde(default = "default_update_repo_owner")]
    pub repo_owner: String,
    #[serde(default = "default_update_repo_name")]
    pub repo_name: String,
    #[serde(default = "default_release_channel")]
    pub channel: String,
    #[serde(default)]
    pub last_checked_at: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            repo_owner: default_update_repo_owner(),
            repo_name: default_update_repo_name(),
            channel: default_release_channel(),
            last_checked_at: None,
        }
    }
}

fn default_ai_provider() -> String {
    "litellm".to_string()
}

fn default_ai_base_url() -> String {
    "http://127.0.0.1:4000/v1".to_string()
}

fn default_ai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_backup_interval_hours() -> u32 {
    24
}

fn default_backup_keep_last() -> u32 {
    5
}

fn default_update_repo_owner() -> String {
    "llpgf".to_string()
}

fn default_update_repo_name() -> String {
    "galroon".to_string()
}

fn default_release_channel() -> String {
    "stable".to_string()
}

// Keep config_dir for backward compat
impl AppConfig {
    /// Alias for workspace_dir (backward compat).
    pub fn config_dir(&self) -> &PathBuf {
        &self.workspace_dir
    }
}

/// Scanner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub scan_on_startup: bool,
    pub stability_threshold_secs: f64,
    pub watcher_channel_capacity: usize,
    pub flush_interval_ms: u64,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            scan_on_startup: true,
            stability_threshold_secs: 45.0,
            watcher_channel_capacity: 1024,
            flush_interval_ms: 500,
        }
    }
}

/// On-disk TOML schema (workspace/config.toml).
#[derive(Debug, Deserialize, Serialize)]
struct ConfigFile {
    library_roots: Option<Vec<String>>,
    scanner: Option<ScannerConfig>,
    sfw_mode: Option<bool>,
    locale: Option<String>,
    theme: Option<String>,
    bangumi: Option<BangumiAuthConfig>,
    ai: Option<AiProviderConfig>,
    backups: Option<BackupConfig>,
    updates: Option<UpdateConfig>,
}

/// Workspace metadata (workspace/workspace.toml).
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub version: String,
    pub created_at: String,
}

impl AppConfig {
    /// Initialize a new workspace at the given directory.
    pub fn init_workspace(workspace_dir: &std::path::Path) -> AppResult<Self> {
        std::fs::create_dir_all(workspace_dir)?;

        // Write workspace.toml marker
        let meta = WorkspaceMeta {
            version: "0.5.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let meta_path = workspace_dir.join("workspace.toml");
        let meta_content = toml::to_string_pretty(&meta)
            .map_err(|e| AppError::Config(format!("Failed to serialize workspace meta: {}", e)))?;
        std::fs::write(&meta_path, meta_content)?;

        // Create default config
        let config = Self::load_from(workspace_dir)?;
        config.save()?;

        tracing::info!(path = %workspace_dir.display(), "Workspace initialized");
        Ok(config)
    }

    /// Check if a directory is a valid workspace.
    pub fn is_workspace(dir: &std::path::Path) -> bool {
        dir.join("workspace.toml").exists()
    }

    /// Load config from a workspace directory.
    pub fn load_from(workspace_dir: &std::path::Path) -> AppResult<Self> {
        let config_path = workspace_dir.join("config.toml");

        let file_config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| AppError::Config(format!("Failed to read config: {}", e)))?;
            toml::from_str::<ConfigFile>(&content)?
        } else {
            ConfigFile {
                library_roots: None,
                scanner: None,
                sfw_mode: None,
                locale: None,
                theme: None,
                bangumi: None,
                ai: None,
                backups: None,
                updates: None,
            }
        };

        // All paths derive from workspace_dir
        let db_path = workspace_dir.join("galroon.db");
        let log_dir = workspace_dir.join("logs");
        let trash_dir = workspace_dir.join(".trash");
        let thumbnail_dir = workspace_dir.join("thumbnails");

        // Ensure sub-directories exist
        std::fs::create_dir_all(&log_dir)?;
        std::fs::create_dir_all(&trash_dir)?;
        std::fs::create_dir_all(&thumbnail_dir)?;

        let library_roots = file_config
            .library_roots
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();

        Ok(Self {
            workspace_dir: workspace_dir.to_path_buf(),
            library_roots,
            db_path,
            log_dir,
            trash_dir,
            thumbnail_dir,
            scanner: file_config.scanner.unwrap_or_default(),
            sfw_mode: file_config.sfw_mode.unwrap_or(false),
            locale: file_config.locale.unwrap_or_else(default_locale),
            theme: file_config.theme.unwrap_or_else(default_theme),
            bangumi: file_config.bangumi,
            ai: file_config.ai,
            backups: file_config.backups.unwrap_or_default(),
            updates: file_config.updates.unwrap_or_default(),
        })
    }

    /// Backward-compat: load() tries launcher.toml → last_workspace → fallback.
    pub fn load() -> AppResult<Self> {
        let launcher = LauncherConfig::load()?;
        if let Some(ws) = &launcher.last_workspace {
            if ws.exists() && Self::is_workspace(ws) {
                return Self::load_from(ws);
            }
        }
        // Fallback: use OS app data dir as workspace (migration path)
        let fallback = LauncherConfig::launcher_dir_static()?;
        Self::load_from(&fallback)
    }

    /// Save config to workspace/config.toml.
    pub fn save(&self) -> AppResult<()> {
        let config_path = self.workspace_dir.join("config.toml");
        let file_config = ConfigFile {
            library_roots: Some(
                self.library_roots
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            ),
            scanner: Some(self.scanner.clone()),
            sfw_mode: Some(self.sfw_mode),
            locale: Some(self.locale.clone()),
            theme: Some(self.theme.clone()),
            bangumi: self.bangumi.clone(),
            ai: self.ai.clone(),
            backups: Some(self.backups.clone()),
            updates: Some(self.updates.clone()),
        };

        let content = toml::to_string_pretty(&file_config)
            .map_err(|e| AppError::Config(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

impl LauncherConfig {
    /// Static helper for fallback dir (avoids &self).
    fn launcher_dir_static() -> AppResult<PathBuf> {
        Self::launcher_dir()
    }
}

// ── SharedConfig (hot-reload via RwLock) ───────────────

use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe, hot-reloadable configuration wrapper.
#[derive(Clone)]
pub struct SharedConfig {
    inner: Arc<RwLock<AppConfig>>,
}

impl SharedConfig {
    pub fn new(config: AppConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        self.inner.read().await
    }

    pub async fn update<F>(&self, f: F) -> AppResult<()>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut config = self.inner.write().await;
        f(&mut config);
        config.save()?;
        tracing::info!("Configuration updated and saved to workspace");
        Ok(())
    }

    pub async fn snapshot(&self) -> AppConfig {
        self.inner.read().await.clone()
    }
}
