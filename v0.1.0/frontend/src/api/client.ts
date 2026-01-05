/**
 * API Client - Phase 18: The Great Wiring
 *
 * Central nervous system for all backend communication.
 * Single entry point for Axios calls with global error handling.
 *
 * Phase 19: Added toast notifications for errors
 * Phase 27.0: Added API token authentication
 */

import axios from 'axios';
import type { AxiosError, AxiosInstance, AxiosResponse } from 'axios';
import toast from 'react-hot-toast';

// ============================================================================
// PHASE 27.0: API TOKEN MANAGEMENT
// ============================================================================

let sessionToken: string | null = null;
let apiPort: number = 8000;  // PHASE 28.0: Dynamic API port (default fallback)

// Get session token from Electron (only in production)
async function initSessionToken() {
  // Check if we're running in Electron
  if (typeof window !== 'undefined' && (window as any).electronAPI) {
    try {
      const response = await (window as any).electronAPI.auth.getSessionToken();
      if (response && response.success) {
        sessionToken = response.token;
        console.log('[PHASE 27.0] Session token loaded from Electron');
      }
    } catch (error) {
      console.error('[PHASE 27.0] Failed to get session token:', error);
    }
  } else {
    console.log('[PHASE 27.0] Not running in Electron - no session token');
  }
}

// PHASE 28.0: Get dynamic API port from Electron
async function initApiPort() {
  if (typeof window !== 'undefined' && (window as any).electronAPI) {
    try {
      const response = await (window as any).electronAPI.auth.getApiPort();
      if (response && response.success) {
        apiPort = response.port;
        console.log(`[PHASE 28.0] Dynamic API port loaded: ${apiPort}`);
        // Update API base URL with dynamic port
        apiClient.defaults.baseURL = `http://127.0.0.1:${apiPort}`;
      }
    } catch (error) {
      console.error('[PHASE 28.0] Failed to get API port:', error);
      console.log('[PHASE 28.0] Using default port: 8000');
    }
  } else {
    console.log('[PHASE 28.0] Not running in Electron - using default port: 8000');
  }
}

// Initialize token and port on module load (async, non-blocking)
initSessionToken();
initApiPort();

// ============================================================================
// TYPE DEFINITIONS (Prepare for Phase 19)
// ============================================================================

/**
 * Generic API Response wrapper matching Backend schemas
 */
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  message?: string;
  error?: string;
}

/**
 * Standard API Error structure
 */
export interface ApiError {
  status?: number;
  message: string;
  code?: string;
  details?: any;
}

// ============================================================================
// CONFIGURATION
// ============================================================================

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8000';

// ============================================================================
// AXIOS INSTANCE
// ============================================================================

const apiClient: AxiosInstance = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// ============================================================================
// REQUEST INTERCEPTOR
// ============================================================================

apiClient.interceptors.request.use(
  (config) => {
    console.log(`[API] ${config.method?.toUpperCase()} ${config.url}`);

    // PHASE 27.0: Inject session token for authentication
    if (sessionToken) {
      config.headers = config.headers || {};
      config.headers['X-Vnite-Token'] = sessionToken;
    }

    return config;
  },
  (error) => {
    console.error('[API] Request error:', error);
    return Promise.reject(error);
  }
);

// ============================================================================
// RESPONSE INTERCEPTOR (Global Error Handling)
// ============================================================================

apiClient.interceptors.response.use(
  (response: AxiosResponse) => {
    console.log(`[API] ✅ ${response.config.url} - ${response.status}`);
    return response;
  },
  (error: AxiosError<ApiError>) => {
    // Phase 18 Hotfix: Fixed error handling logic
    const message =
      (error.response?.data as any)?.details?.[0]?.msg ||
      (error.response?.data as any)?.detail ||
      error.message;

    let apiError: ApiError = {
      message: message || 'Unknown error occurred',
    };

    if (error.response) {
      // Server responded with error status
      const status = error.response.status;
      const data = error.response.data;

      apiError = {
        status,
        message,
        code: (data as any)?.code,
        details: data,
      };

      console.error(`[API] Error ${status}:`, apiError.message);

      // Phase 19: Trigger toast notifications
      if (status >= 500) {
        toast.error(`Server Error: ${apiError.message}`, {
          duration: 5000,
          id: `server-error-${status}-${Date.now()}`, // Prevent duplicates
        });
      } else if (status === 404) {
        toast.error(`Not Found: ${apiError.message}`, {
          duration: 4000,
        });
      } else if (status === 401) {
        toast.error('Unauthorized: Please check your credentials', {
          duration: 4000,
        });
      } else if (status >= 400) {
        toast.error(`Request Failed: ${apiError.message}`, {
          duration: 4000,
        });
      }

    } else if (error.request) {
      // Request made but no response (backend offline)
      apiError = {
        status: 0,
        message: 'Backend is offline or unreachable',
      };

      console.error('[API] 🔴 No response - Backend offline?');

      // Phase 19: Trigger connection lost toast
      toast.error('Connection Lost: Backend is offline or unreachable', {
        duration: 6000,
        id: 'connection-lost',
      });

    } else {
      // Error setting up request
      apiError = {
        message: error.message || 'Request setup failed',
      };

      console.error('[API] Request setup error:', error.message);
      toast.error('Request Failed: Please check your connection', {
        duration: 4000,
      });
    }

    // Reject with ApiError structure
    const enhancedError = {
      ...error,
      apiError,
    };

    return Promise.reject(enhancedError);
  }
);

// ============================================================================
// API METHODS (Organized by Router)
// ============================================================================

export const api = {
  // ============================================================
  // GAMES & LIBRARY
  // ============================================================

  /**
   * Get all games from library
   * Endpoint: GET /api/games
   *
   * Phase 19.6: ✅ SEMANTIC SANITIZATION
   * Supports: search, tag, library_status, sort_by, limit, offset
   *
   * Example: getAllGames({ search: 'fate', sort_by: 'rating', limit: 20 })
   */
  getAllGames: (params?: {
    search?: string;
    tag?: string;
    library_status?: string;
    sort_by?: string;
    limit?: number;
    offset?: number;
  }) => apiClient.get('/api/games', { params }),

  /**
   * Get game by ID
   * Endpoint: GET /api/games/{id}
   *
   * Phase 19.7: ✅ IMPLEMENTED
   * Returns full game details with versions, characters, staff, screenshots
   */
  getGameById: (id: string) => apiClient.get(`/api/games/${id}`),

  /**
   * Update library status
   * Endpoint: PATCH /api/games/{id}/status
   *
   * Phase 19.6: ✅ SEMANTIC SANITIZATION
   * Persists library status to metadata.json
   */
  updateLibraryStatus: (gameId: string, libraryStatus: string) =>
    apiClient.patch(`/api/games/${gameId}/status`, { library_status: libraryStatus }),

  // ============================================================
  // METADATA
  // ============================================================

  /**
   * Get game metadata
   * Endpoint: GET /api/metadata/game/{id}
   */
  getGameMetadata: (id: string) => apiClient.get(`/api/metadata/game/${id}`),

  // ============================================================
  // ANALYTICS
  // ============================================================

  /**
   * Get dashboard statistics
   * Endpoint: GET /api/analytics/dashboard
   */
  getDashboardStats: () => apiClient.get('/api/analytics/dashboard'),

  /**
   * Search staff (knowledge graph)
   * Endpoint: GET /api/analytics/staff/{name}
   */
  searchStaff: (name: string) =>
    apiClient.get(`/api/analytics/staff/${encodeURIComponent(name)}`),

  /**
   * Search cast (knowledge graph)
   * Endpoint: GET /api/analytics/cast/{name}
   */
  searchCast: (name: string) =>
    apiClient.get(`/api/analytics/cast/${encodeURIComponent(name)}`),

  /**
   * Search series (knowledge graph)
   * Endpoint: GET /api/analytics/series/{series_name}
   */
  searchSeries: (seriesName: string) =>
    apiClient.get(`/api/analytics/series/${encodeURIComponent(seriesName)}`),

  // ============================================================
  // SEARCH
  // ============================================================

  /**
   * Faceted search
   * Endpoint: GET /api/search/
   */
  search: (params: {
    query?: string;
    year_min?: number;
    year_max?: number;
    tags?: string;
    engine?: string;
    developer?: string;
    limit?: number;
  }) => apiClient.get('/api/search/', { params }),

  /**
   * Get search facets
   * Endpoint: GET /api/search/facets
   */
  getSearchFacets: () => apiClient.get('/api/search/facets'),

  // ============================================================
  // ORGANIZER (Workbench)
  // ============================================================

  /**
   * Generate organization proposal
   * Endpoint: POST /api/organizer/generate
   */
  generateProposal: (data: {
    source_path: string;
    target_root: string;
    vndb_metadata: any;
  }) => apiClient.post('/api/organizer/generate', data),

  /**
   * Execute organization proposal
   * Endpoint: POST /api/organizer/execute
   */
  executeProposal: (proposalId: string) =>
    apiClient.post('/api/organizer/execute', { proposal_id: proposalId }),

  /**
   * Rollback transaction
   * Endpoint: POST /api/organizer/rollback
   */
  rollbackTransaction: (transactionId: string) =>
    apiClient.post('/api/organizer/rollback', { transaction_id: transactionId }),

  /**
   * Analyze directory
   * Endpoint: POST /api/organizer/analyze
   */
  analyzeDirectory: (path: string) =>
    apiClient.post('/api/organizer/analyze', { path }),

  // ============================================================
  // CURATOR (Metadata Management)
  // ============================================================

  /**
   * Manual VNDB ID identification
   * Endpoint: POST /api/curator/identify
   */
  identifyGame: (folderPath: string, vndbId: string) =>
    apiClient.post('/api/curator/identify', {
      folder_path: folderPath,
      vndb_id: vndbId,
    }),

  /**
   * Lock metadata fields
   * Endpoint: POST /api/curator/lock_fields
   */
  lockFields: (folderPath: string, fields: string[]) =>
    apiClient.post('/api/curator/lock_fields', {
      folder_path: folderPath,
      fields,
    }),

  /**
   * Unlock metadata fields
   * Endpoint: POST /api/curator/unlock_fields
   */
  unlockFields: (folderPath: string, fields: string[]) =>
    apiClient.post('/api/curator/unlock_fields', {
      folder_path: folderPath,
      fields,
    }),

  /**
   * Update single metadata field
   * Endpoint: POST /api/curator/update_field
   */
  updateField: (folderPath: string, field: string, value: any) =>
    apiClient.post('/api/curator/update_field', {
      folder_path: folderPath,
      field,
      value,
    }),

  /**
   * Browse extras
   * Endpoint: GET /api/curator/extras/{folder_path}
   */
  getExtras: (folderPath: string) =>
    apiClient.get(`/api/curator/extras/${encodeURIComponent(folderPath)}`),

  /**
   * Merge game versions
   * Endpoint: POST /api/curator/merge_versions
   */
  mergeVersions: (primaryPath: string, secondaryPaths: string[]) =>
    apiClient.post('/api/curator/merge_versions', {
      primary_path: primaryPath,
      secondary_paths: secondaryPaths,
    }),

  /**
   * Update user tags
   * Phase 18.5: Custom user tags for personal organization
   * Endpoint: PATCH /api/curator/games/tags
   */
  updateTags: (folderPath: string, userTags: string[]) =>
    apiClient.patch('/api/curator/games/tags', {
      folder_path: folderPath,
      user_tags: userTags,
    }),

  // ============================================================
  // UTILITIES (The "Tools")
  // ============================================================

  /**
   * Open folder in file manager
   * Endpoint: POST /api/utils/reveal
   */
  revealFolder: (path: string) =>
    apiClient.post('/api/utils/reveal', { path }),

  /**
   * Copy path to clipboard
   * Endpoint: POST /api/utils/copy
   */
  copyToClipboard: (text: string) =>
    apiClient.post('/api/utils/copy', { text }),

  /**
   * Extract archive
   * Endpoint: POST /api/utils/extract
   */
  extractArchive: (archivePath: string, targetDir: string) =>
    apiClient.post('/api/utils/extract', {
      archive_path: archivePath,
      target_dir: targetDir,
    }),

  /**
   * Get task status (for polling)
   * Endpoint: GET /api/utils/tasks/{task_id}
   */
  getTaskStatus: (taskId: string) =>
    apiClient.get(`/api/utils/tasks/${taskId}`),

  /**
   * Get utility status
   * Endpoint: GET /api/utils/status
   */
  getUtilityStatus: () => apiClient.get('/api/utils/status'),

  /**
   * Open file or folder with default application
   * Endpoint: POST /api/utils/open
   */
  openFile: (path: string) => apiClient.post('/api/utils/open', { path }),

  // ============================================================
  // SYSTEM
  // ============================================================

  /**
   * Open folder in system file manager
   * Endpoint: POST /api/system/open_folder
   */
  openFolder: (path: string) =>
    apiClient.post('/api/system/open_folder', { path }),

  /**
   * Get system info
   * Endpoint: GET /api/system/info
   */
  getSystemInfo: () => apiClient.get('/api/system/info'),

  // ============================================================
  // CONNECTORS
  // ============================================================

  /**
   * Sync game metadata
   * Endpoint: POST /api/connectors/sync
   */
  syncGame: (gamePath: string, options: {
    force_steam?: boolean;
    force_bangumi?: boolean;
    download_assets?: boolean;
  }) => apiClient.post('/api/connectors/sync', {
    game_path: gamePath,
    ...options,
  }),

  /**
   * Get connector status
   * Endpoint: GET /api/connectors/status
   */
  getConnectorStatus: () => apiClient.get('/api/connectors/status'),

  /**
   * Test connector
   * Endpoint: POST /api/connectors/test/{connector_name}
   */
  testConnector: (connectorName: string) =>
    apiClient.post(`/api/connectors/test/${connectorName}`),

  // ============================================================
  // SCANNER
  // ============================================================

  /**
   * Get scanner status
   * Endpoint: GET /api/scanner/mode
   */
  getScannerStatus: () => apiClient.get('/api/scanner/mode'),

  /**
   * Set scanner mode
   * Endpoint: POST /api/scanner/mode
   */
  setScannerMode: (mode: 'realtime' | 'scheduled' | 'manual', scheduledTime?: string) =>
    apiClient.post('/api/scanner/mode', {
      mode,
      scheduled_time: scheduledTime,
    }),

  /**
   * Trigger manual scan
   * Endpoint: POST /api/scanner/scan
   */
  triggerScan: () => apiClient.post('/api/scanner/scan'),

  /**
   * Start scanner
   * Endpoint: POST /api/scanner/start
   */
  startScanner: () => apiClient.post('/api/scanner/start'),

  /**
   * Stop scanner
   * Endpoint: POST /api/scanner/stop
   */
  stopScanner: () => apiClient.post('/api/scanner/stop'),

  // ============================================================
  // PHASE 20.0: INSTANT INDEX (SQLite-based scanner)
  // ============================================================

  /**
   * Trigger library scan (Phase 20.0 scanner)
   * Endpoint: POST /api/games/scan
   * Phase 20.0: ✅ IMPLEMENTED - Background scanning with fast diff
   */
  triggerLibraryScan: () => apiClient.post('/api/games/scan'),

  /**
   * Get library scan status (Phase 20.0 scanner)
   * Endpoint: GET /api/games/scan/status
   * Phase 20.0: ✅ IMPLEMENTED - Check if scan is in progress
   */
  getLibraryScanStatus: () => apiClient.get('/api/games/scan/status'),

  // ============================================================
  // SETTINGS (Phase 19.6+)
  // ============================================================

  /**
   * Get all library roots
   * Endpoint: GET /api/settings/roots
   */
  getLibraryRoots: () => apiClient.get('/api/settings/roots'),

  /**
   * Add library root
   * Endpoint: POST /api/settings/roots
   */
  addLibraryRoot: (path: string) =>
    apiClient.post('/api/settings/roots', { path }),

  /**
   * Delete library root
   * Endpoint: DELETE /api/settings/roots/{root_id}
   */
  deleteLibraryRoot: (rootId: string) =>
    apiClient.delete(`/api/settings/roots/${rootId}`),

  /**
   * Set primary library root
   * Endpoint: PATCH /api/settings/roots/{root_id}/set_primary
   */
  setPrimaryRoot: (rootId: string) =>
    apiClient.patch(`/api/settings/roots/${rootId}/set_primary`),

  /**
   * Get all connectors
   * Endpoint: GET /api/settings/connectors
   */
  getConnectors: () => apiClient.get('/api/settings/connectors'),

  /**
   * Update connector enabled status
   * Endpoint: PATCH /api/settings/connectors/{connector_name}
   */
  updateConnector: (connectorName: string, enabled: boolean) =>
    apiClient.patch(`/api/settings/connectors/${connectorName}`, { enabled }),

  // ============================================================
  // SCANNER CONFIG (Phase 19.5)
  // ============================================================

  /**
   * Get scanner configuration
   * Endpoint: GET /api/settings/scanner
   * Phase 19.5: ✅ IMPLEMENTED
   */
  getScannerConfig: () => apiClient.get('/api/settings/scanner'),

  /**
   * Update scanner configuration
   * Endpoint: POST /api/settings/scanner
   * Phase 19.5: ✅ IMPLEMENTED
   */
  updateScannerConfig: (scanOnStartup: boolean, scanIntervalMin: number) =>
    apiClient.post('/api/settings/scanner', {
      scan_on_startup: scanOnStartup,
      scan_interval_min: scanIntervalMin,
    }),

  // ============================================================
  // PHASE 24.5: VISUAL SCANNER (Progress tracking & control)
  // ============================================================

  /**
   * Get scan progress
   * Endpoint: GET /api/settings/scanner/progress
   * Phase 24.5: ✅ IMPLEMENTED - Real-time scan progress
   */
  getScanProgress: () => apiClient.get('/api/settings/scanner/progress'),

  /**
   * Pause scan
   * Endpoint: POST /api/settings/scanner/pause
   * Phase 24.5: ✅ IMPLEMENTED - Pause current scan
   */
  pauseScan: () => apiClient.post('/api/settings/scanner/pause'),

  /**
   * Resume scan
   * Endpoint: POST /api/settings/scanner/resume
   * Phase 24.5: ✅ IMPLEMENTED - Resume paused scan
   */
  resumeScan: () => apiClient.post('/api/settings/scanner/resume'),

  /**
   * Cancel scan
   * Endpoint: POST /api/settings/scanner/cancel
   * Phase 24.5: ✅ IMPLEMENTED - Cancel current scan
   */
  cancelScan: () => apiClient.post('/api/settings/scanner/cancel'),

  // ============================================================
  // PHASE 24.5: SCHEDULER (Task scheduling)
  // ============================================================

  /**
   * Get scheduler status
   * Endpoint: GET /api/settings/scheduler/status
   * Phase 24.5: ✅ IMPLEMENTED - Scheduler configuration
   */
  getSchedulerStatus: () => apiClient.get('/api/settings/scheduler/status'),

  /**
   * Get scheduled jobs
   * Endpoint: GET /api/settings/scheduler/jobs
   * Phase 24.5: ✅ IMPLEMENTED - List all scheduled jobs
   */
  getScheduledJobs: () => apiClient.get('/api/settings/scheduler/jobs'),

  /**
   * Update scan interval
   * Endpoint: POST /api/settings/scheduler/interval
   * Phase 24.5: ✅ IMPLEMENTED - Configure scan interval
   */
  updateScanInterval: (intervalMin: number) =>
    apiClient.post('/api/settings/scheduler/interval', { interval_min: intervalMin }),

  /**
   * Trigger manual scan
   * Endpoint: POST /api/settings/scheduler/trigger
   * Phase 24.5: ✅ IMPLEMENTED - Immediate scan trigger
   */
  triggerSchedulerScan: () => apiClient.post('/api/settings/scheduler/trigger'),

  // ============================================================
  // PHASE 24.5: BACKUP (Time Machine)
  // ============================================================

  /**
   * Create backup
   * Endpoint: POST /api/settings/backup/create
   * Phase 24.5: ✅ IMPLEMENTED - Create backup
   */
  createBackup: () => apiClient.post('/api/settings/backup/create'),

  /**
   * List backups
   * Endpoint: GET /api/settings/backup/list
   * Phase 24.5: ✅ IMPLEMENTED - List all backups
   */
  listBackups: () => apiClient.get('/api/settings/backup/list'),

  /**
   * Restore backup
   * Endpoint: POST /api/settings/backup/restore
   * Phase 24.5: ✅ IMPLEMENTED - Restore from backup
   */
  restoreBackup: (filename: { filename: string }) =>
    apiClient.post('/api/settings/backup/restore', filename),

  /**
   * Delete backup
   * Endpoint: DELETE /api/settings/backup/{filename}
   * Phase 24.5: ✅ IMPLEMENTED - Delete backup
   */
  deleteBackup: (filename: string) => apiClient.delete(`/api/settings/backup/${filename}`),

  /**
   * Get backup statistics
   * Endpoint: GET /api/settings/backup/stats
   * Phase 24.5: ✅ IMPLEMENTED - Backup stats
   */
  getBackupStats: () => apiClient.get('/api/settings/backup/stats'),

  /**
   * Set max backups
   * Endpoint: POST /api/settings/backup/max-backups
   * Phase 24.5: ✅ IMPLEMENTED - Configure auto-prune
   */
  setMaxBackups: (maxBackups: number) =>
    apiClient.post('/api/settings/backup/max-backups', { max_backups: maxBackups }),

  // ============================================================
  // PHASE 24.5: AUTO-UPDATE
  // ============================================================

  /**
   * Check for updates
   * Endpoint: GET /api/settings/update/check
   * Phase 24.5: ✅ IMPLEMENTED - Check GitHub releases
   */
  checkForUpdates: () => apiClient.get('/api/settings/update/check'),

  /**
   * Get update configuration
   * Endpoint: GET /api/settings/update/config
   * Phase 24.5: ✅ IMPLEMENTED - Update settings
   */
  getUpdateConfig: () => apiClient.get('/api/settings/update/config'),

  /**
   * Update update configuration
   * Endpoint: POST /api/settings/update/config
   * Phase 24.5: ✅ IMPLEMENTED - Save update settings
   */
  updateUpdateConfig: (autoCheckEnabled: boolean, checkIntervalHours: number) =>
    apiClient.post('/api/settings/update/config', {
      auto_check_enabled: autoCheckEnabled,
      check_interval_hours: checkIntervalHours
    }),

  // ============================================================
  // TRASH (Phase 19)
  // ============================================================

  /**
   * Get trash status
   * Endpoint: GET /api/trash/status
   * Phase 19: ✅ IMPLEMENTED
   */
  getTrashStatus: () => apiClient.get('/api/trash/status'),

  /**
   * Get trash config
   * Endpoint: GET /api/trash/config
   * Phase 19: ✅ IMPLEMENTED
   */
  getTrashConfig: () => apiClient.get('/api/trash/config'),

  /**
   * Empty trash
   * Endpoint: POST /api/trash/empty
   * Phase 19: ✅ IMPLEMENTED
   */
  emptyTrash: () => apiClient.post('/api/trash/empty'),
};

export default apiClient;
