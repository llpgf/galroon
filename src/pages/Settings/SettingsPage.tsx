// Settings Page — workspace management, library paths, trash, backups, plugins, i18n.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart';
import './SettingsPage.css';

interface SourcePlugin {
      id: string;
      name: string;
      enabled: boolean;
      priority: number;
}

function SourcePluginsPanel({ showMessage }: { showMessage: (m: string) => void }) {
      const [plugins, setPlugins] = useState<SourcePlugin[]>([]);

      useEffect(() => {
            invoke<SourcePlugin[]>('list_source_plugins')
                  .then(setPlugins)
                  .catch(() => { });
      }, []);

      async function toggle(id: string, enabled: boolean) {
            try {
                  await invoke('toggle_source_plugin', { id, enabled: !enabled });
                  setPlugins(ps => ps.map(p => p.id === id ? { ...p, enabled: !enabled } : p));
                  showMessage(`${id} ${!enabled ? 'enabled' : 'disabled'}`);
            } catch { }
      }

      return (
            <div className="plugins-list">
                  {plugins.map(p => (
                        <div key={p.id} className="plugin-row">
                              <span className={`plugin-dot ${p.enabled ? 'on' : 'off'}`} />
                              <span className="plugin-name">{p.name}</span>
                              <span className="plugin-priority">P{p.priority}</span>
                              <button className="plugin-toggle" onClick={() => toggle(p.id, p.enabled)}>
                                    {p.enabled ? 'Disable' : 'Enable'}
                              </button>
                        </div>
                  ))}
            </div>
      );
}

interface WorkspaceInfo {
      workspace_path: string;
      db_path: string;
      thumbnail_dir: string;
      log_dir: string;
      trash_dir: string;
      db_size_bytes: number;
      thumbnail_count: number;
      trash_count: number;
}

interface TrashItem {
      name: string;
      size_bytes: number;
      age_days: number;
      is_dir: boolean;
}

interface AppSettings {
      library_roots: string[];
      theme: string;
      locale: string;
}

interface AppJobStatus {
      id: number;
      kind: string;
      state: string;
      title: string;
      progress_pct: number;
      current_step: string | null;
      last_error: string | null;
      result_json: Record<string, unknown> | null;
      can_pause: boolean;
      can_resume: boolean;
      can_cancel: boolean;
      created_at: string;
      updated_at: string;
}

interface EnrichmentQueueStatus {
      paused: boolean;
      queued: number;
      running: number;
      retry_wait: number;
      failed: number;
      success: number;
      total_pending: number;
}

interface BackupScheduleStatus {
      enabled: boolean;
      interval_hours: number;
      destination_dir: string | null;
      keep_last: number;
      last_run_at: string | null;
}

interface UpdateSettingsStatus {
      auto_check: boolean;
      repo_owner: string;
      repo_name: string;
      channel: string;
      last_checked_at: string | null;
}

interface NativeUpdateCheckStatus {
      current_version: string;
      release_version: string | null;
      release_name: string | null;
      release_notes: string | null;
      release_url: string | null;
      checked_at: string;
      compatible_package_available: boolean;
      install_version: string | null;
      install_target: string | null;
      manifest_endpoint: string;
      message: string;
}

interface NativeUpdateProgressEvent {
      phase: string;
      downloaded: number;
      total: number | null;
      message: string;
}

type ThemeMode = 'system' | 'dark' | 'light';

interface AiProviderStatus {
      configured: boolean;
      provider: string;
      base_url: string;
      model: string;
      has_api_key: boolean;
      api_key_hint: string | null;
}

interface AiProviderProbeResult {
      ok: boolean;
      provider: string;
      base_url: string;
      model: string;
      message: string;
      models: string[];
}

interface BangumiAuthStatus {
      connected: boolean;
      has_access_token: boolean;
      has_app_id: boolean;
      has_app_secret: boolean;
      token_hint: string | null;
      app_id_hint: string | null;
}

interface BangumiProbeResult {
      connected: boolean;
      username: string;
      nickname: string | null;
      user_id: number;
      avatar: string | null;
}

interface BangumiOAuthFlowStatus {
      phase: string;
      authorize_url: string | null;
      callback_url: string | null;
      message: string | null;
      probe: BangumiProbeResult | null;
}

export default function SettingsPage() {
      const [settings, setSettings] = useState<AppSettings>({
            library_roots: [], theme: 'system', locale: 'ja',
      });
      const [workspace, setWorkspace] = useState<WorkspaceInfo | null>(null);
      const [trashItems, setTrashItems] = useState<TrashItem[]>([]);
      const [newPath, setNewPath] = useState('');
      const [relocatePath, setRelocatePath] = useState('');
      const [backupPath, setBackupPath] = useState('');
      const [isSaving, setIsSaving] = useState(false);
      const [isSavingBangumi, setIsSavingBangumi] = useState(false);
      const [isVerifyingBangumi, setIsVerifyingBangumi] = useState(false);
      const [isStartingBangumiOAuth, setIsStartingBangumiOAuth] = useState(false);
      const [message, setMessage] = useState('');
      const [bangumiStatus, setBangumiStatus] = useState<BangumiAuthStatus | null>(null);
      const [bangumiProbe, setBangumiProbe] = useState<BangumiProbeResult | null>(null);
      const [bangumiOAuth, setBangumiOAuth] = useState<BangumiOAuthFlowStatus>({
            phase: 'idle',
            authorize_url: null,
            callback_url: null,
            message: null,
            probe: null,
      });
      const [bangumiForm, setBangumiForm] = useState({
            accessToken: '',
            appId: '',
            appSecret: '',
      });
      const [isSavingAi, setIsSavingAi] = useState(false);
      const [isTestingAi, setIsTestingAi] = useState(false);
      const [aiStatus, setAiStatus] = useState<AiProviderStatus | null>(null);
      const [aiProbe, setAiProbe] = useState<AiProviderProbeResult | null>(null);
      const [aiForm, setAiForm] = useState({
            provider: 'litellm',
            baseUrl: 'http://127.0.0.1:4000/v1',
            model: 'gpt-4o-mini',
            apiKey: '',
      });
      const [appJobs, setAppJobs] = useState<AppJobStatus[]>([]);
      const [enrichmentQueue, setEnrichmentQueue] = useState<EnrichmentQueueStatus | null>(null);
      const [backupSchedule, setBackupSchedule] = useState<BackupScheduleStatus | null>(null);
      const [updateSettings, setUpdateSettings] = useState<UpdateSettingsStatus | null>(null);
      const [autostartEnabled, setAutostartEnabled] = useState(false);
      const [isCheckingUpdates, setIsCheckingUpdates] = useState(false);
      const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
      const [updateSummary, setUpdateSummary] = useState<string>('');
      const [downloadProgress, setDownloadProgress] = useState<string>('');
      const [nativeUpdate, setNativeUpdate] = useState<NativeUpdateCheckStatus | null>(null);

      useEffect(() => {
            loadSettings();
            loadWorkspaceInfo();
            loadTrash();
            loadBangumiStatus();
            loadBangumiOAuthStatus();
            loadAiStatus();
            loadAppJobs();
            loadEnrichmentQueue();
            loadBackupSchedule();
            loadUpdateSettings();
            loadAutostartStatus();
      }, []);

      const loadSettings = useCallback(async () => {
            try {
                  const data = await invoke<AppSettings>('get_settings');
                  setSettings(data);
                  applyTheme((data.theme as ThemeMode) || 'system');
            } catch { /* */ }
      }, [applyTheme]);

      const loadWorkspaceInfo = useCallback(async () => {
            try {
                  setWorkspace(await invoke<WorkspaceInfo>('get_workspace_info'));
            } catch { /* */ }
      }, []);

      const loadTrash = useCallback(async () => {
            try {
                  setTrashItems(await invoke<TrashItem[]>('list_trash'));
            } catch { /* */ }
      }, []);

      const loadBangumiStatus = useCallback(async () => {
            try {
                  const status = await invoke<BangumiAuthStatus>('get_bangumi_auth_status');
                  setBangumiStatus(status);
            } catch { /* */ }
      }, []);

      const loadBangumiOAuthStatus = useCallback(async () => {
            try {
                  const status = await invoke<BangumiOAuthFlowStatus>('get_bangumi_oauth_status');
                  setBangumiOAuth(status);
                  if (status.probe) {
                        setBangumiProbe(status.probe);
                  }
                  if (status.phase === 'success') {
                        loadBangumiStatus();
                  }
            } catch { /* */ }
      }, [loadBangumiStatus]);

      const loadAiStatus = useCallback(async () => {
            try {
                  const status = await invoke<AiProviderStatus>('get_ai_provider_status');
                  setAiStatus(status);
                  setAiForm((current) => ({
                        ...current,
                        provider: status.provider || current.provider,
                        baseUrl: status.base_url || current.baseUrl,
                        model: status.model || current.model,
                        apiKey: current.apiKey || localStorage.getItem('galroon_ai_key') || '',
                  }));
            } catch { /* */ }
      }, []);

      const loadAppJobs = useCallback(async () => {
            try {
                  const jobs = await invoke<AppJobStatus[]>('list_app_jobs', { limit: 16 });
                  setAppJobs(Array.isArray(jobs) ? jobs : []);
            } catch { /* */ }
      }, []);

      const loadEnrichmentQueue = useCallback(async () => {
            try {
                  const status = await invoke<EnrichmentQueueStatus>('get_enrichment_queue_status');
                  setEnrichmentQueue(status);
            } catch { /* */ }
      }, []);

      const loadBackupSchedule = useCallback(async () => {
            try {
                  const status = await invoke<BackupScheduleStatus>('get_backup_schedule');
                  setBackupSchedule(status);
                  if (status.destination_dir && !backupPath) {
                        setBackupPath(status.destination_dir);
                  }
            } catch { /* */ }
      }, [backupPath]);

      const loadUpdateSettings = useCallback(async () => {
            try {
                  const status = await invoke<UpdateSettingsStatus>('get_update_settings');
                  setUpdateSettings(status);
            } catch { /* */ }
      }, []);

      const loadAutostartStatus = useCallback(async () => {
            try {
                  setAutostartEnabled(await isAutostartEnabled());
            } catch { /* */ }
      }, []);

      useEffect(() => {
            if (!['waiting_browser', 'waiting_callback', 'exchanging'].includes(bangumiOAuth.phase)) {
                  return;
            }

            const timer = window.setInterval(() => {
                  loadBangumiOAuthStatus();
            }, 1200);

            return () => window.clearInterval(timer);
      }, [bangumiOAuth.phase, loadBangumiOAuthStatus]);

      useEffect(() => {
            let unlistenPromise: Promise<() => void> | null = null;
            unlistenPromise = listen<NativeUpdateProgressEvent>('native-update-progress', (event) => {
                  const payload = event.payload;
                  if (!payload) {
                        return;
                  }
                  setDownloadProgress(payload.message);
                  if (payload.phase === 'installing') {
                        setUpdateSummary('Installer launched. Follow the native updater flow to finish the update.');
                  }
            });

            return () => {
                  if (unlistenPromise) {
                        void unlistenPromise.then((unlisten) => unlisten());
                  }
            };
      }, []);

      useEffect(() => {
            const hasActiveJobs = appJobs.some((job) => ['queued', 'running', 'paused'].includes(job.state))
                  || (enrichmentQueue?.total_pending ?? 0) > 0;
            if (!hasActiveJobs) {
                  return;
            }

            const timer = window.setInterval(() => {
                  loadAppJobs();
                  loadEnrichmentQueue();
            }, 1500);

            return () => window.clearInterval(timer);
      }, [appJobs, enrichmentQueue, loadAppJobs, loadEnrichmentQueue]);

      const addLibraryRoot = useCallback(() => {
            if (!newPath.trim()) return;
            setSettings(s => ({
                  ...s,
                  library_roots: [...s.library_roots, newPath.trim()],
            }));
            setNewPath('');
      }, [newPath]);

      const removeLibraryRoot = useCallback((index: number) => {
            setSettings(s => ({
                  ...s,
                  library_roots: s.library_roots.filter((_, i) => i !== index),
            }));
      }, []);

      const saveSettings = useCallback(async () => {
            setIsSaving(true);
            try {
                  await invoke('update_settings', { settings });
                  showMessage('Settings saved ✓');
            } catch (e) {
                  showMessage(`Save failed: ${e}`);
            } finally {
                  setIsSaving(false);
            }
      }, [settings]);

      const handleRelocate = useCallback(async () => {
            if (!relocatePath.trim()) return;
            try {
                  await invoke('relocate_workspace', { newPath: relocatePath.trim() });
                  showMessage('Workspace relocated — restart app to apply');
            } catch (e) {
                  showMessage(`Relocate failed: ${e}`);
            }
      }, [relocatePath]);

      const handleBackup = useCallback(async () => {
            if (!backupPath.trim()) return;
            try {
                  await invoke('backup_workspace', { backupPath: backupPath.trim() });
                  showMessage('Backup created ✓');
            } catch (e) {
                  showMessage(`Backup failed: ${e}`);
            }
      }, [backupPath]);

      const handleEmptyTrash = useCallback(async () => {
            try {
                  const count = await invoke<number>('empty_trash');
                  showMessage(`Removed ${count} items from trash`);
                  loadTrash();
            } catch (e) {
                  showMessage(`Failed: ${e}`);
            }
      }, [loadTrash]);

      const handlePurgeOld = useCallback(async () => {
            try {
                  const count = await invoke<number>('purge_trash', { retentionDays: 30 });
                  showMessage(`Purged ${count} items older than 30 days`);
                  loadTrash();
            } catch (e) {
                  showMessage(`Failed: ${e}`);
            }
      }, [loadTrash]);

      const handleSaveBangumiAuth = useCallback(async () => {
            setIsSavingBangumi(true);
            try {
                  const status = await invoke<BangumiAuthStatus>('update_bangumi_auth', {
                        bangumi: {
                              access_token: bangumiForm.accessToken || null,
                              app_id: bangumiForm.appId || null,
                              app_secret: bangumiForm.appSecret || null,
                        },
                  });
                  setBangumiStatus(status);
                  setBangumiProbe(null);
                  setBangumiForm({
                        accessToken: '',
                        appId: '',
                        appSecret: '',
                  });
                  showMessage('Bangumi auth saved ✓');
            } catch (e) {
                  showMessage(`Bangumi auth failed: ${e}`);
            } finally {
                  setIsSavingBangumi(false);
            }
      }, [bangumiForm]);

      const handleDisconnectBangumi = useCallback(async () => {
            try {
                  await invoke('cancel_bangumi_oauth').catch(() => null);
                  await invoke('clear_bangumi_auth');
                  setBangumiStatus({
                        connected: false,
                        has_access_token: false,
                        has_app_id: false,
                        has_app_secret: false,
                        token_hint: null,
                        app_id_hint: null,
                  });
                  setBangumiProbe(null);
                  setBangumiOAuth({
                        phase: 'idle',
                        authorize_url: null,
                        callback_url: null,
                        message: null,
                        probe: null,
                  });
                  setBangumiForm({
                        accessToken: '',
                        appId: '',
                        appSecret: '',
                  });
                  showMessage('Bangumi auth cleared');
            } catch (e) {
                  showMessage(`Disconnect failed: ${e}`);
            }
      }, []);

      const handleStartBangumiOAuth = useCallback(async () => {
            setIsStartingBangumiOAuth(true);
            try {
                  const status = await invoke<BangumiOAuthFlowStatus>('start_bangumi_oauth');
                  setBangumiOAuth(status);
                  if (status.authorize_url) {
                        await open(status.authorize_url);
                  }
                  showMessage('Bangumi login started in your browser');
            } catch (e) {
                  showMessage(`Bangumi OAuth failed: ${e}`);
            } finally {
                  setIsStartingBangumiOAuth(false);
            }
      }, []);

      const handleCancelBangumiOAuth = useCallback(async () => {
            try {
                  const status = await invoke<BangumiOAuthFlowStatus>('cancel_bangumi_oauth');
                  setBangumiOAuth(status);
                  showMessage('Bangumi login cancelled');
            } catch (e) {
                  showMessage(`Cancel failed: ${e}`);
            }
      }, []);

      const handleVerifyBangumi = useCallback(async () => {
            setIsVerifyingBangumi(true);
            try {
                  const result = await invoke<BangumiProbeResult>('probe_bangumi_auth');
                  setBangumiProbe(result);
                  showMessage(`Bangumi connected as ${result.nickname || result.username} ✓`);
            } catch (e) {
                  setBangumiProbe(null);
                  showMessage(`Bangumi verify failed: ${e}`);
            } finally {
                  setIsVerifyingBangumi(false);
            }
      }, []);

      const handleSaveAiSettings = useCallback(async () => {
            setIsSavingAi(true);
            try {
                  const status = await invoke<AiProviderStatus>('update_ai_provider_settings', {
                        ai: {
                              provider: aiForm.provider,
                              base_url: aiForm.baseUrl,
                              model: aiForm.model,
                              api_key: aiForm.apiKey || null,
                        },
                  });
                  setAiStatus(status);
                  if (aiForm.apiKey) {
                        localStorage.setItem('galroon_ai_key', aiForm.apiKey);
                  }
                  setAiForm((current) => ({ ...current, apiKey: '' }));
                  showMessage('AI provider settings saved ✓');
            } catch (e) {
                  showMessage(`AI settings failed: ${e}`);
            } finally {
                  setIsSavingAi(false);
            }
      }, [aiForm]);

      const handleClearAiSettings = useCallback(async () => {
            try {
                  await invoke('clear_ai_provider_settings');
                  setAiStatus({
                        configured: false,
                        provider: 'litellm',
                        base_url: 'http://127.0.0.1:4000/v1',
                        model: 'gpt-4o-mini',
                        has_api_key: false,
                        api_key_hint: null,
                  });
                  setAiForm({
                        provider: 'litellm',
                        baseUrl: 'http://127.0.0.1:4000/v1',
                        model: 'gpt-4o-mini',
                        apiKey: '',
                  });
                  localStorage.removeItem('galroon_ai_key');
                  showMessage('AI provider settings cleared');
            } catch (e) {
                  showMessage(`Clear failed: ${e}`);
            }
      }, []);

      const openBangumiTokenPage = useCallback(async () => {
            try {
                  await open('https://next.bgm.tv/demo/access-token');
                  showMessage('Opened Bangumi access token page');
            } catch (e) {
                  showMessage(`Open failed: ${e}`);
            }
      }, []);

      const openBangumiDocs = useCallback(async () => {
            try {
                  await open('https://bangumi.github.io/api/');
            } catch (e) {
                  showMessage(`Open failed: ${e}`);
            }
      }, []);

      function applyTheme(mode: ThemeMode) {
            const resolved = mode === 'system'
                  ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
                  : mode;
            document.documentElement.setAttribute('data-theme', resolved);
            document.documentElement.setAttribute('data-theme-mode', mode);
            localStorage.setItem('galroon_theme', mode);
      }

      const handleQueueFullEnrichment = useCallback(async () => {
            try {
                  const result = await invoke<{ queued: number }>('enqueue_library_enrichment');
                  showMessage(`Queued metadata refresh for ${result.queued} posters`);
                  loadEnrichmentQueue();
            } catch (e) {
                  showMessage(`Queue failed: ${e}`);
            }
      }, [loadEnrichmentQueue]);

      const handlePauseEnrichment = useCallback(async () => {
            try {
                  await invoke('pause_enrichment_queue');
                  showMessage('Enrichment queue paused');
                  loadEnrichmentQueue();
            } catch (e) {
                  showMessage(`Pause failed: ${e}`);
            }
      }, [loadEnrichmentQueue]);

      const handleResumeEnrichment = useCallback(async () => {
            try {
                  await invoke('resume_enrichment_queue');
                  showMessage('Enrichment queue resumed');
                  loadEnrichmentQueue();
            } catch (e) {
                  showMessage(`Resume failed: ${e}`);
            }
      }, [loadEnrichmentQueue]);

      const handlePauseJob = useCallback(async (jobId: number) => {
            try {
                  await invoke('pause_app_job', { jobId });
                  loadAppJobs();
            } catch (e) {
                  showMessage(`Pause failed: ${e}`);
            }
      }, [loadAppJobs]);

      const handleResumeJob = useCallback(async (jobId: number) => {
            try {
                  await invoke('resume_app_job', { jobId });
                  loadAppJobs();
            } catch (e) {
                  showMessage(`Resume failed: ${e}`);
            }
      }, [loadAppJobs]);

      const handleCancelJob = useCallback(async (jobId: number) => {
            try {
                  await invoke('cancel_app_job', { jobId });
                  loadAppJobs();
            } catch (e) {
                  showMessage(`Cancel failed: ${e}`);
            }
      }, [loadAppJobs]);

      const handleRunScheduledBackupNow = useCallback(async () => {
            try {
                  await invoke('enqueue_backup_job', { destinationDir: backupSchedule?.destination_dir || backupPath || null });
                  showMessage('Backup job queued ✓');
                  loadAppJobs();
            } catch (e) {
                  showMessage(`Backup queue failed: ${e}`);
            }
      }, [backupPath, backupSchedule, loadAppJobs]);

      const handleSaveBackupSchedule = useCallback(async () => {
            try {
                  const status = await invoke<BackupScheduleStatus>('update_backup_schedule', {
                        schedule: {
                              enabled: backupSchedule?.enabled ?? false,
                              interval_hours: backupSchedule?.interval_hours ?? 24,
                              destination_dir: backupSchedule?.destination_dir ?? (backupPath || null),
                              keep_last: backupSchedule?.keep_last ?? 5,
                        },
                  });
                  setBackupSchedule(status);
                  showMessage('Backup schedule saved ✓');
            } catch (e) {
                  showMessage(`Backup schedule failed: ${e}`);
            }
      }, [backupPath, backupSchedule]);

      const handleToggleAutostart = useCallback(async () => {
            try {
                  if (autostartEnabled) {
                        await disableAutostart();
                        setAutostartEnabled(false);
                        showMessage('Launch at login disabled');
                  } else {
                        await enableAutostart();
                        setAutostartEnabled(true);
                        showMessage('Launch at login enabled');
                  }
            } catch (e) {
                  showMessage(`Autostart failed: ${e}`);
            }
      }, [autostartEnabled]);

      const handleSaveUpdateSettings = useCallback(async () => {
            try {
                  const status = await invoke<UpdateSettingsStatus>('update_update_settings', {
                        updates: {
                              auto_check: updateSettings?.auto_check ?? true,
                              repo_owner: updateSettings?.repo_owner ?? 'llpgf',
                              repo_name: updateSettings?.repo_name ?? 'galroon',
                              channel: updateSettings?.channel ?? 'stable',
                        },
                  });
                  setUpdateSettings(status);
                  showMessage('Update settings saved ✓');
            } catch (e) {
                  showMessage(`Update settings failed: ${e}`);
            }
      }, [updateSettings]);

      const handleCheckUpdates = useCallback(async () => {
            setIsCheckingUpdates(true);
            setDownloadProgress('');
            try {
                  await invoke('enqueue_update_check');
                  loadAppJobs();
                  const status = await invoke<NativeUpdateCheckStatus>('check_native_update');
                  setNativeUpdate(status);
                  const release = status.release_version ? `GitHub release ${status.release_version}` : 'No GitHub release metadata';
                  const install = status.compatible_package_available
                        ? `Signed installer ${status.install_version || 'available'} for ${status.install_target || 'this target'}`
                        : 'No compatible signed updater package found yet';
                  setUpdateSummary(`${release}. ${install}. ${status.message}`);
                  if (!status.compatible_package_available) {
                        setDownloadProgress(`Manifest checked at ${status.manifest_endpoint}`);
                  }
            } catch (e) {
                  setUpdateSummary(`Update check fallback active: ${e}`);
                  showMessage(`Updater check failed: ${e}`);
            } finally {
                  setIsCheckingUpdates(false);
            }
      }, [loadAppJobs]);

      const handleInstallUpdate = useCallback(async () => {
            setIsInstallingUpdate(true);
            setDownloadProgress('Preparing signed updater package...');
            try {
                  await invoke('install_native_update');
                  setUpdateSummary('Installer launched. The app may close to complete the update.');
            } catch (e) {
                  setUpdateSummary(`Install failed: ${e}`);
                  showMessage(`Update install failed: ${e}`);
            } finally {
                  setIsInstallingUpdate(false);
            }
      }, []);

      const handleTestAiSettings = useCallback(async () => {
            setIsTestingAi(true);
            try {
                  const probe = await invoke<AiProviderProbeResult>('probe_ai_provider');
                  setAiProbe(probe);
                  showMessage(probe.message);
            } catch (e) {
                  setAiProbe(null);
                  showMessage(`AI connection failed: ${e}`);
            } finally {
                  setIsTestingAi(false);
            }
      }, []);

      function showMessage(msg: string) {
            setMessage(msg);
            setTimeout(() => setMessage(''), 4000);
      }

      function formatBytes(bytes: number): string {
            if (bytes < 1024) return `${bytes} B`;
            if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
            return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
      }

      function formatDate(value: string | null | undefined): string {
            if (!value) return '—';
            return new Date(value).toLocaleString();
      }

      return (
            <div className="settings-page">
                  <div className="settings-header">
                        <h1>Settings</h1>
                        {message && <span className="settings-msg">{message}</span>}
                  </div>

                  <div className="settings-content">
                        {/* ── Workspace Info ── */}
                        {workspace && (
                              <section className="settings-section">
                                    <h2>📁 Workspace</h2>
                                    <p className="section-desc">
                                          All data in one folder. Backup = copy it. Restore = point app here.
                                    </p>
                                    <div className="ws-info-grid">
                                          <div className="ws-info-item">
                                                <span className="ws-label">Path</span>
                                                <code>{workspace.workspace_path}</code>
                                          </div>
                                          <div className="ws-info-item">
                                                <span className="ws-label">Database</span>
                                                <code>{formatBytes(workspace.db_size_bytes)}</code>
                                          </div>
                                          <div className="ws-info-item">
                                                <span className="ws-label">Thumbnails</span>
                                                <code>{workspace.thumbnail_count} files</code>
                                          </div>
                                          <div className="ws-info-item">
                                                <span className="ws-label">Trash</span>
                                                <code>{workspace.trash_count} items</code>
                                          </div>
                                    </div>
                              </section>
                        )}

                        {/* ── Relocate Workspace ── */}
                        <section className="settings-section">
                              <h2>📦 Relocate Workspace</h2>
                              <p className="section-desc">
                                    Move all workspace data to a new location (e.g., external drive).
                              </p>
                              <form className="settings-inline-form" onSubmit={(e) => { e.preventDefault(); handleRelocate(); }}>
                                    <div className="input-row">
                                          <input
                                                id="relocate-workspace"
                                                name="relocate-workspace"
                                                type="text"
                                                placeholder="E:\Backup\GalroonWorkspace"
                                                value={relocatePath}
                                                onChange={(e) => setRelocatePath(e.target.value)}
                                          />
                                          <button className="action-btn" type="submit">Relocate</button>
                                    </div>
                              </form>
                        </section>

                        {/* ── Backup ── */}
                        <section className="settings-section">
                              <h2>💾 Backup Workspace</h2>
                              <p className="section-desc">
                                    Create a copy of the entire workspace (DB, config, thumbnails).
                              </p>
                              <form className="settings-inline-form" onSubmit={(e) => { e.preventDefault(); handleBackup(); }}>
                                    <div className="input-row">
                                          <input
                                                id="backup-workspace"
                                                name="backup-workspace"
                                                type="text"
                                                placeholder="D:\Backups\galroon_2024"
                                                value={backupPath}
                                                onChange={(e) => setBackupPath(e.target.value)}
                                          />
                                          <button className="action-btn" type="submit">Backup</button>
                                          <button className="action-btn secondary" type="button" onClick={handleRunScheduledBackupNow}>
                                                Queue Backup Job
                                          </button>
                                    </div>
                              </form>
                        </section>

                        <section className="settings-section">
                              <h2>🕒 Scheduled Backups</h2>
                              <p className="section-desc">
                                    Run timestamped workspace backups in the background. Jobs survive app restarts; launch-at-login makes scheduling practical.
                              </p>
                              <div className="setting-row">
                                    <label htmlFor="backup-enabled">Enable Schedule</label>
                                    <input
                                          id="backup-enabled"
                                          name="backup-enabled"
                                          type="checkbox"
                                          checked={backupSchedule?.enabled ?? false}
                                          onChange={(e) => setBackupSchedule((current) => ({
                                                enabled: e.target.checked,
                                                interval_hours: current?.interval_hours ?? 24,
                                                destination_dir: current?.destination_dir ?? backupPath,
                                                keep_last: current?.keep_last ?? 5,
                                                last_run_at: current?.last_run_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="backup-interval">Interval (hours)</label>
                                    <input
                                          id="backup-interval"
                                          name="backup-interval"
                                          type="number"
                                          min={1}
                                          value={backupSchedule?.interval_hours ?? 24}
                                          onChange={(e) => setBackupSchedule((current) => ({
                                                enabled: current?.enabled ?? false,
                                                interval_hours: Number(e.target.value) || 24,
                                                destination_dir: current?.destination_dir ?? backupPath,
                                                keep_last: current?.keep_last ?? 5,
                                                last_run_at: current?.last_run_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="backup-keep-last">Keep Last</label>
                                    <input
                                          id="backup-keep-last"
                                          name="backup-keep-last"
                                          type="number"
                                          min={1}
                                          value={backupSchedule?.keep_last ?? 5}
                                          onChange={(e) => setBackupSchedule((current) => ({
                                                enabled: current?.enabled ?? false,
                                                interval_hours: current?.interval_hours ?? 24,
                                                destination_dir: current?.destination_dir ?? backupPath,
                                                keep_last: Number(e.target.value) || 5,
                                                last_run_at: current?.last_run_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="backup-schedule-destination">Destination</label>
                                    <input
                                          id="backup-schedule-destination"
                                          name="backup-schedule-destination"
                                          type="text"
                                          className="api-key-input"
                                          value={backupSchedule?.destination_dir ?? backupPath}
                                          onChange={(e) => setBackupSchedule((current) => ({
                                                enabled: current?.enabled ?? false,
                                                interval_hours: current?.interval_hours ?? 24,
                                                destination_dir: e.target.value,
                                                keep_last: current?.keep_last ?? 5,
                                                last_run_at: current?.last_run_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="bangumi-auth-actions">
                                    <button className="action-btn" type="button" onClick={handleSaveBackupSchedule}>
                                          Save Backup Schedule
                                    </button>
                                    <button className="action-btn secondary" type="button" onClick={handleToggleAutostart}>
                                          {autostartEnabled ? 'Disable Launch at Login' : 'Enable Launch at Login'}
                                    </button>
                              </div>
                              <p className="section-note">
                                    Last scheduled run: {formatDate(backupSchedule?.last_run_at)}. Launch at login is recommended if you want missed schedules to resume on the next session automatically.
                              </p>
                        </section>

                        {/* ── Trash ── */}
                        <section className="settings-section">
                              <h2>🗑️ Workspace Trash</h2>
                              <p className="section-desc">
                                    Items moved to workspace trash (NAS/network paths). OS trash items are in your Recycle Bin.
                              </p>

                              {trashItems.length > 0 ? (
                                    <>
                                          <div className="trash-list">
                                                {trashItems.map((item, i) => (
                                                      <div key={i} className="trash-item">
                                                            <span className="trash-icon">{item.is_dir ? '📂' : '📄'}</span>
                                                            <span className="trash-name">{item.name}</span>
                                                            <span className="trash-meta">
                                                                  {formatBytes(item.size_bytes)} · {item.age_days}d ago
                                                            </span>
                                                      </div>
                                                ))}
                                          </div>
                                          <div className="trash-actions">
                                                <button className="action-btn secondary" onClick={handlePurgeOld}>
                                                      Purge &gt;30 days
                                                </button>
                                                <button className="action-btn danger" onClick={handleEmptyTrash}>
                                                      Empty All
                                                </button>
                                          </div>
                                    </>
                              ) : (
                                    <p className="empty-state">Workspace trash is empty</p>
                              )}
                        </section>

                        {/* ── Library Roots ── */}
                        <section className="settings-section">
                              <h2>📚 Library Folders</h2>
                              <p className="section-desc">
                                    Folders where games are stored. Each subfolder = one game.
                              </p>
                              <div className="library-roots">
                                    {settings.library_roots.map((root, i) => (
                                          <div key={i} className="library-root-item">
                                                <span className="root-path">{root}</span>
                                                <button
                                                      className="root-remove"
                                                      onClick={() => removeLibraryRoot(i)}
                                                      title="Remove"
                                                >✕</button>
                                          </div>
                                    ))}
                                    {settings.library_roots.length === 0 && (
                                          <div className="library-root-empty">No library folders added.</div>
                                    )}
                              </div>
                              <form className="settings-inline-form" onSubmit={(e) => { e.preventDefault(); addLibraryRoot(); }}>
                                    <div className="input-row">
                                          <input
                                                id="library-root-input"
                                                name="library-root-input"
                                                type="text"
                                                placeholder="D:\Games\Galgame"
                                                value={newPath}
                                                onChange={(e) => setNewPath(e.target.value)}
                                          />
                                          <button className="action-btn" type="submit">Add</button>
                                    </div>
                              </form>
                        </section>

                        {/* ── Appearance ── */}
                        <section className="settings-section">
                              <h2>🎨 Appearance</h2>
                              <div className="setting-row">
                                    <label htmlFor="theme-select">Theme</label>
                                    <select
                                          id="theme-select"
                                          name="theme-select"
                                          value={settings.theme}
                                          onChange={(e) => {
                                                const theme = e.target.value as ThemeMode;
                                                setSettings({ ...settings, theme });
                                                applyTheme(theme);
                                          }}
                                    >
                                          <option value="system">System</option>
                                          <option value="dark">Dark</option>
                                          <option value="light">Light</option>
                                    </select>
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="locale-select">Language</label>
                                    <select
                                          id="locale-select"
                                          name="locale-select"
                                          value={settings.locale}
                                          onChange={async (e) => {
                                                const locale = e.target.value;
                                                setSettings({ ...settings, locale });
                                                try { await invoke('set_locale', { locale }); showMessage('Language updated ✓'); } catch { }
                                          }}
                                    >
                                          <option value="ja">日本語</option>
                                          <option value="en">English</option>
                                          <option value="zh-Hant">繁體中文</option>
                                          <option value="zh-Hans">简体中文</option>
                                    </select>
                              </div>
                        </section>

                        {/* ── Source Plugins ── */}
                        <section className="settings-section">
                              <h2>🔌 Data Sources</h2>
                              <p className="section-desc">Enable or disable enrichment data sources. Higher priority sources are preferred for matching.</p>
                              <SourcePluginsPanel showMessage={showMessage} />
                        </section>

                        <section className="settings-section">
                              <h2>🧵 Background Jobs</h2>
                              <p className="section-desc">
                                    Scan, backup, and update checks run as persisted jobs. Closing the app will resume them on the next launch.
                              </p>
                              <div className="bangumi-auth-actions">
                                    <button className="action-btn" type="button" onClick={async () => {
                                          try {
                                                await invoke('trigger_scan');
                                                showMessage('Library scan queued ✓');
                                                loadAppJobs();
                                          } catch (e) {
                                                showMessage(`Scan queue failed: ${e}`);
                                          }
                                    }}>
                                          Queue Library Scan
                                    </button>
                                    <button className="action-btn secondary" type="button" onClick={handleQueueFullEnrichment}>
                                          Queue Full Match
                                    </button>
                              </div>
                              {appJobs.length === 0 ? (
                                    <p className="empty-state">No recent background jobs.</p>
                              ) : (
                                    <div className="job-list">
                                          {appJobs.map((job) => (
                                                <div key={job.id} className="job-row">
                                                      <div className="job-row-main">
                                                            <div className="job-row-topline">
                                                                  <strong>{job.title}</strong>
                                                                  <span className={`job-state job-state-${job.state}`}>{job.state}</span>
                                                            </div>
                                                            <div className="job-row-meta">
                                                                  <span>{job.kind}</span>
                                                                  <span>{Math.round(job.progress_pct)}%</span>
                                                                  <span>{job.current_step || 'Waiting'}</span>
                                                            </div>
                                                            <div className="job-progress-track">
                                                                  <div className="job-progress-fill" style={{ width: `${Math.max(4, Math.min(job.progress_pct, 100))}%` }} />
                                                            </div>
                                                            {job.last_error && <div className="job-error">{job.last_error}</div>}
                                                      </div>
                                                      <div className="job-row-actions">
                                                            {job.can_pause && job.state === 'running' && (
                                                                  <button className="action-btn secondary" type="button" onClick={() => void handlePauseJob(job.id)}>Pause</button>
                                                            )}
                                                            {job.can_resume && job.state === 'paused' && (
                                                                  <button className="action-btn secondary" type="button" onClick={() => void handleResumeJob(job.id)}>Resume</button>
                                                            )}
                                                            {job.can_cancel && ['queued', 'running', 'paused'].includes(job.state) && (
                                                                  <button className="action-btn danger" type="button" onClick={() => void handleCancelJob(job.id)}>Cancel</button>
                                                            )}
                                                      </div>
                                                </div>
                                          ))}
                                    </div>
                              )}
                        </section>

                        <section className="settings-section">
                              <h2>🔄 Enrichment Queue</h2>
                              <p className="section-desc">
                                    Matching already runs in the background. Use these controls to queue the whole library, pause after the current work, or resume.
                              </p>
                              <div className="ws-info-grid">
                                    <div className="ws-info-item"><span className="ws-label">Pending</span><code>{enrichmentQueue?.total_pending ?? 0}</code></div>
                                    <div className="ws-info-item"><span className="ws-label">Queued</span><code>{enrichmentQueue?.queued ?? 0}</code></div>
                                    <div className="ws-info-item"><span className="ws-label">Running</span><code>{enrichmentQueue?.running ?? 0}</code></div>
                                    <div className="ws-info-item"><span className="ws-label">Failed</span><code>{enrichmentQueue?.failed ?? 0}</code></div>
                              </div>
                              <div className="bangumi-auth-actions">
                                    <button className="action-btn" type="button" onClick={handleQueueFullEnrichment}>Queue Full Match</button>
                                    <button className="action-btn secondary" type="button" onClick={handlePauseEnrichment} disabled={enrichmentQueue?.paused}>Pause Queue</button>
                                    <button className="action-btn secondary" type="button" onClick={handleResumeEnrichment} disabled={!enrichmentQueue?.paused}>Resume Queue</button>
                              </div>
                              <p className="section-note">
                                    Queue state: {enrichmentQueue?.paused ? 'Paused' : 'Live'}.
                              </p>
                        </section>

                        <section className="settings-section">
                              <h2>⬆️ Updates</h2>
                              <p className="section-desc">
                                    Check GitHub releases for new builds. Direct install uses the official Tauri updater when a compatible update package is published.
                              </p>
                              <div className="setting-row">
                                    <label htmlFor="updates-auto-check">Auto-check on Launch</label>
                                    <input
                                          id="updates-auto-check"
                                          name="updates-auto-check"
                                          type="checkbox"
                                          checked={updateSettings?.auto_check ?? true}
                                          onChange={(e) => setUpdateSettings((current) => ({
                                                auto_check: e.target.checked,
                                                repo_owner: current?.repo_owner ?? 'llpgf',
                                                repo_name: current?.repo_name ?? 'galroon',
                                                channel: current?.channel ?? 'stable',
                                                last_checked_at: current?.last_checked_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="updates-owner">GitHub Owner</label>
                                    <input
                                          id="updates-owner"
                                          name="updates-owner"
                                          type="text"
                                          className="api-key-input"
                                          value={updateSettings?.repo_owner ?? 'llpgf'}
                                          onChange={(e) => setUpdateSettings((current) => ({
                                                auto_check: current?.auto_check ?? true,
                                                repo_owner: e.target.value,
                                                repo_name: current?.repo_name ?? 'galroon',
                                                channel: current?.channel ?? 'stable',
                                                last_checked_at: current?.last_checked_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="updates-repo">Repository</label>
                                    <input
                                          id="updates-repo"
                                          name="updates-repo"
                                          type="text"
                                          className="api-key-input"
                                          value={updateSettings?.repo_name ?? 'galroon'}
                                          onChange={(e) => setUpdateSettings((current) => ({
                                                auto_check: current?.auto_check ?? true,
                                                repo_owner: current?.repo_owner ?? 'llpgf',
                                                repo_name: e.target.value,
                                                channel: current?.channel ?? 'stable',
                                                last_checked_at: current?.last_checked_at ?? null,
                                          }))}
                                    />
                              </div>
                              <div className="setting-row">
                                    <label htmlFor="updates-channel">Channel</label>
                                    <select
                                          id="updates-channel"
                                          name="updates-channel"
                                          value={updateSettings?.channel ?? 'stable'}
                                          onChange={(e) => setUpdateSettings((current) => ({
                                                auto_check: current?.auto_check ?? true,
                                                repo_owner: current?.repo_owner ?? 'llpgf',
                                                repo_name: current?.repo_name ?? 'galroon',
                                                channel: e.target.value,
                                                last_checked_at: current?.last_checked_at ?? null,
                                          }))}
                                    >
                                          <option value="stable">Stable</option>
                                          <option value="beta">Beta</option>
                                    </select>
                              </div>
                              <div className="bangumi-auth-actions">
                                    <button className="action-btn" type="button" onClick={handleSaveUpdateSettings}>Save Update Settings</button>
                                    <button className="action-btn secondary" type="button" onClick={handleCheckUpdates} disabled={isCheckingUpdates}>
                                          {isCheckingUpdates ? 'Checking...' : 'Check for Updates'}
                                    </button>
                                    <button
                                          className="action-btn secondary"
                                          type="button"
                                          onClick={handleInstallUpdate}
                                          disabled={isInstallingUpdate || !nativeUpdate?.compatible_package_available}
                                    >
                                          {isInstallingUpdate ? 'Installing...' : 'Download & Install'}
                                    </button>
                                    <button
                                          className="action-btn secondary"
                                          type="button"
                                          onClick={() => void open(`https://github.com/${updateSettings?.repo_owner ?? 'llpgf'}/${updateSettings?.repo_name ?? 'galroon'}/releases`)}
                                    >
                                          Open Releases
                                    </button>
                              </div>
                              <p className="section-note">
                                    Last checked: {formatDate(updateSettings?.last_checked_at)}. {updateSummary || 'No update check has been run in this session.'}
                              </p>
                              {nativeUpdate && (
                                    <div className="bangumi-auth-summary">
                                          <span className={`bangumi-status-badge ${nativeUpdate.compatible_package_available ? 'connected' : 'disconnected'}`}>
                                                {nativeUpdate.compatible_package_available ? 'Installer Ready' : 'Release Only'}
                                          </span>
                                          <span className="bangumi-auth-hint">Current {nativeUpdate.current_version}</span>
                                          {nativeUpdate.release_version && (
                                                <span className="bangumi-auth-hint">Latest {nativeUpdate.release_version}</span>
                                          )}
                                          {nativeUpdate.install_target && (
                                                <span className="bangumi-auth-hint">{nativeUpdate.install_target}</span>
                                          )}
                                    </div>
                              )}
                              {nativeUpdate?.release_url && (
                                    <div className="bangumi-auth-actions">
                                          <button className="action-btn secondary" type="button" onClick={() => void open(nativeUpdate.release_url!)}>
                                                Open Latest Release
                                          </button>
                                    </div>
                              )}
                              {downloadProgress && <p className="section-note">{downloadProgress}</p>}
                        </section>

                        <section className="settings-section">
                              <h2>🧷 Bangumi Connect</h2>
                              <p className="section-desc">
                                    Use your own Bangumi account to unlock authenticated and R18-only subject data. Stored only in this workspace.
                              </p>
                              <p className="section-note">
                                    Recommended flow: save your App ID and App Secret once, then click <strong>Connect via Browser</strong>. Galroon will listen on localhost and store the returned token automatically.
                              </p>

                              <div className="bangumi-auth-summary">
                                    <div className={`bangumi-status-badge ${bangumiStatus?.connected ? 'connected' : 'disconnected'}`}>
                                          {bangumiStatus?.connected ? 'Connected' : 'Not Connected'}
                                    </div>
                                    {bangumiStatus?.token_hint && (
                                          <span className="bangumi-auth-hint">Token {bangumiStatus.token_hint}</span>
                                    )}
                                    {bangumiStatus?.app_id_hint && (
                                          <span className="bangumi-auth-hint">App {bangumiStatus.app_id_hint}</span>
                                    )}
                              </div>

                              <div className={`bangumi-oauth-card phase-${bangumiOAuth.phase}`}>
                                    <div className="bangumi-oauth-header">
                                          <strong>Browser OAuth</strong>
                                          <span className="bangumi-oauth-phase">{bangumiOAuth.phase.split('_').join(' ')}</span>
                                    </div>
                                    <p className="bangumi-oauth-message">
                                          {bangumiOAuth.message || 'Use browser login for the cleanest Bangumi connection flow.'}
                                    </p>
                                    {bangumiOAuth.callback_url && (
                                          <code className="bangumi-oauth-callback">{bangumiOAuth.callback_url}</code>
                                    )}
                                    {bangumiOAuth.probe && (
                                          <div className="bangumi-oauth-user">
                                                <strong>{bangumiOAuth.probe.nickname || bangumiOAuth.probe.username}</strong>
                                                <span>@{bangumiOAuth.probe.username}</span>
                                          </div>
                                    )}
                              </div>

                              <div className="bangumi-auth-actions bangumi-oauth-actions">
                                    <button
                                          className="action-btn"
                                          onClick={handleStartBangumiOAuth}
                                          disabled={isStartingBangumiOAuth || ['waiting_browser', 'waiting_callback', 'exchanging'].includes(bangumiOAuth.phase)}
                                    >
                                          {isStartingBangumiOAuth ? 'Starting...' : 'Connect via Browser'}
                                    </button>
                                    <button
                                          className="action-btn secondary"
                                          onClick={handleCancelBangumiOAuth}
                                          disabled={!['waiting_browser', 'waiting_callback', 'exchanging'].includes(bangumiOAuth.phase)}
                                    >
                                          Cancel Login
                                    </button>
                              </div>

                              <form
                                    className="settings-inline-form"
                                    onSubmit={(e) => {
                                          e.preventDefault();
                                          handleSaveBangumiAuth();
                                    }}
                              >
                                    <input
                                          type="text"
                                          name="bangumi-username"
                                          autoComplete="username"
                                          tabIndex={-1}
                                          aria-hidden="true"
                                          className="visually-hidden-field"
                                    />
                                    <div className="bangumi-auth-grid">
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="bangumi-access-token">Access Token</label>
                                                <input
                                                      id="bangumi-access-token"
                                                      name="bangumi-access-token"
                                                      type="password"
                                                      autoComplete="new-password"
                                                      className="api-key-input"
                                                      placeholder={bangumiStatus?.has_access_token ? 'Stored token kept unless you paste a new one' : 'Paste your Bangumi access token or callback URL'}
                                                      value={bangumiForm.accessToken}
                                                      onChange={(e) => setBangumiForm((current) => ({ ...current, accessToken: e.target.value }))}
                                                />
                                          </div>
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="bangumi-app-id">App ID</label>
                                                <input
                                                      id="bangumi-app-id"
                                                      name="bangumi-app-id"
                                                      type="text"
                                                      className="api-key-input"
                                                      placeholder={bangumiStatus?.has_app_id ? 'Stored app id kept unless you paste a new one' : 'Optional but recommended'}
                                                      value={bangumiForm.appId}
                                                      onChange={(e) => setBangumiForm((current) => ({ ...current, appId: e.target.value }))}
                                                />
                                          </div>
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="bangumi-app-secret">App Secret</label>
                                                <input
                                                      id="bangumi-app-secret"
                                                      name="bangumi-app-secret"
                                                      type="password"
                                                      autoComplete="new-password"
                                                      className="api-key-input"
                                                      placeholder={bangumiStatus?.has_app_secret ? 'Stored app secret kept unless you paste a new one' : 'Required for browser OAuth'}
                                                      value={bangumiForm.appSecret}
                                                      onChange={(e) => setBangumiForm((current) => ({ ...current, appSecret: e.target.value }))}
                                                />
                                          </div>
                                    </div>

                                    <div className="bangumi-auth-actions">
                                          <button className="action-btn" type="submit" disabled={isSavingBangumi}>
                                                {isSavingBangumi ? 'Saving...' : 'Save Bangumi Auth'}
                                          </button>
                                    </div>
                              </form>

                              <div className="bangumi-auth-actions">
                                    <button className="action-btn secondary" type="button" onClick={openBangumiTokenPage}>
                                          Open Token Page
                                    </button>
                                    <button className="action-btn secondary" type="button" onClick={openBangumiDocs}>
                                          API Docs
                                    </button>
                                    <button className="action-btn secondary" type="button" onClick={handleVerifyBangumi} disabled={isVerifyingBangumi || !bangumiStatus?.has_access_token}>
                                          {isVerifyingBangumi ? 'Verifying...' : 'Verify Token'}
                                    </button>
                                    <button className="action-btn danger" type="button" onClick={handleDisconnectBangumi} disabled={!bangumiStatus?.connected}>
                                          Disconnect
                                    </button>
                              </div>

                              {bangumiProbe && (
                                    <div className="bangumi-probe-card">
                                          {bangumiProbe.avatar && (
                                                <img
                                                      className="bangumi-probe-avatar"
                                                      src={bangumiProbe.avatar}
                                                      alt={bangumiProbe.nickname || bangumiProbe.username}
                                                />
                                          )}
                                          <div className="bangumi-probe-meta">
                                                <strong>{bangumiProbe.nickname || bangumiProbe.username}</strong>
                                                <span>@{bangumiProbe.username}</span>
                                                <span>User #{bangumiProbe.user_id}</span>
                                          </div>
                                    </div>
                              )}
                        </section>

                        {/* ── AI Translation ── */}
                        <section className="settings-section">
                              <h2>🤖 AI Gateway</h2>
                              <p className="section-desc">
                                    Configure one OpenAI-compatible endpoint. LiteLLM is the recommended default because it can front OpenAI, Anthropic, Gemini, OpenRouter, and Ollama through a single API surface.
                              </p>
                              <div className="bangumi-auth-summary">
                                    <div className={`bangumi-status-badge ${aiStatus?.configured ? 'connected' : 'disconnected'}`}>
                                          {aiStatus?.configured ? 'Configured' : 'Not Configured'}
                                    </div>
                                    {aiStatus?.api_key_hint && (
                                          <span className="bangumi-auth-hint">Key {aiStatus.api_key_hint}</span>
                                    )}
                                    <span className="bangumi-auth-hint">{aiStatus?.provider || aiForm.provider}</span>
                              </div>
                              <form className="settings-inline-form" onSubmit={(e) => { e.preventDefault(); handleSaveAiSettings(); }}>
                                    <input
                                          type="text"
                                          name="translation-username"
                                          autoComplete="username"
                                          tabIndex={-1}
                                          aria-hidden="true"
                                          className="visually-hidden-field"
                                    />
                                    <div className="bangumi-auth-grid">
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="translation-provider">Gateway Preset</label>
                                                <select
                                                      id="translation-provider"
                                                      name="translation-provider"
                                                      value={aiForm.provider}
                                                      onChange={(e) => {
                                                            const provider = e.target.value;
                                                            const defaults: Record<string, { baseUrl: string; model: string }> = {
                                                                  litellm: { baseUrl: 'http://127.0.0.1:4000/v1', model: 'gpt-4o-mini' },
                                                                  'openai-compatible': { baseUrl: 'http://127.0.0.1:4000/v1', model: 'gpt-4o-mini' },
                                                                  openai: { baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' },
                                                                  openrouter: { baseUrl: 'https://openrouter.ai/api/v1', model: 'openai/gpt-4o-mini' },
                                                                  ollama: { baseUrl: 'http://127.0.0.1:11434/v1', model: 'llama3.2' },
                                                            };
                                                            const preset = defaults[provider] || defaults.litellm;
                                                            setAiForm((current) => ({
                                                                  ...current,
                                                                  provider,
                                                                  baseUrl: preset.baseUrl,
                                                                  model: current.model || preset.model,
                                                            }));
                                                      }}
                                                >
                                                      <option value="litellm">LiteLLM</option>
                                                      <option value="openai-compatible">Generic OpenAI-Compatible</option>
                                                      <option value="openai">OpenAI</option>
                                                      <option value="openrouter">OpenRouter</option>
                                                      <option value="ollama">Ollama</option>
                                                </select>
                                          </div>
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="translation-base-url">Base URL</label>
                                                <input
                                                      id="translation-base-url"
                                                      name="translation-base-url"
                                                      type="text"
                                                      className="api-key-input"
                                                      placeholder="http://127.0.0.1:4000/v1"
                                                      value={aiForm.baseUrl}
                                                      onChange={(e) => setAiForm((current) => ({ ...current, baseUrl: e.target.value }))}
                                                />
                                          </div>
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="translation-model">Default Model</label>
                                                <input
                                                      id="translation-model"
                                                      name="translation-model"
                                                      type="text"
                                                      className="api-key-input"
                                                      placeholder="gpt-4o-mini"
                                                      value={aiForm.model}
                                                      onChange={(e) => setAiForm((current) => ({ ...current, model: e.target.value }))}
                                                />
                                          </div>
                                          <div className="setting-row bangumi-auth-row">
                                                <label htmlFor="translation-api-key">API Key</label>
                                                <input
                                                      id="translation-api-key"
                                                      name="translation-api-key"
                                                      type="password"
                                                      autoComplete="new-password"
                                                      placeholder={aiStatus?.has_api_key ? 'Stored key kept unless you paste a new one' : 'sk-... or your gateway key'}
                                                      className="api-key-input"
                                                      value={aiForm.apiKey}
                                                      onChange={(e) => setAiForm((current) => ({ ...current, apiKey: e.target.value }))}
                                                />
                                          </div>
                                    </div>
                                    <div className="bangumi-auth-actions">
                                          <button className="action-btn" type="submit" disabled={isSavingAi}>
                                                {isSavingAi ? 'Saving...' : 'Save AI Gateway'}
                                          </button>
                                          <button className="action-btn secondary" type="button" onClick={handleTestAiSettings} disabled={isTestingAi || !aiStatus?.configured}>
                                                {isTestingAi ? 'Testing...' : 'Test Connection'}
                                          </button>
                                          <button className="action-btn secondary" type="button" onClick={() => void open('https://www.litellm.ai/oss')}>
                                                LiteLLM
                                          </button>
                                          <button className="action-btn danger" type="button" onClick={handleClearAiSettings} disabled={!aiStatus?.configured}>
                                                Clear
                                          </button>
                                    </div>
                              </form>
                              <p className="section-note">
                                    Recommended presets: LiteLLM for Anthropic/Gemini/OpenAI/OpenRouter through one gateway, or Ollama for local models.
                              </p>
                              {aiProbe && (
                                    <div className="bangumi-auth-summary">
                                          <span className={`bangumi-status-badge ${aiProbe.ok ? 'connected' : 'disconnected'}`}>
                                                {aiProbe.ok ? 'Connected' : 'Unavailable'}
                                          </span>
                                          <span className="bangumi-auth-hint">{aiProbe.provider}</span>
                                          <span className="bangumi-auth-hint">{aiProbe.message}</span>
                                          {aiProbe.models.slice(0, 3).map((model) => (
                                                <span key={model} className="bangumi-auth-hint">{model}</span>
                                          ))}
                                    </div>
                              )}
                        </section>

                        {/* ── Save ── */}
                        <div className="settings-actions">
                              <button
                                    className="save-btn"
                                    onClick={saveSettings}
                                    disabled={isSaving}
                              >
                                    {isSaving ? 'Saving...' : 'Save Settings'}
                              </button>
                        </div>
                  </div>
            </div>
      );
}
