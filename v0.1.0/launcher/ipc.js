/**
 * IPC Handlers for Vnite Launcher
 *
 * Phase 26.0: Portable Telemetry
 * - Open logs folder
 * - Export logs (zip)
 * - Get system info
 */

const { ipcMain, shell, app } = require('electron');
const path = require('path');
const fs = require('fs');

// ============================================================
// PHASE 26.0: LOG MANAGEMENT IPC
// ============================================================

/**
 * Register all IPC handlers
 */
function registerIpcHandlers() {
  // PHASE 27.0: Get session token for API authentication
  ipcMain.handle('vnite:get-session-token', async () => {
    // Get token from main process (passed via module global)
    // This is called by frontend to authenticate API requests
    const token = global.sessionToken;

    if (!token) {
      console.error('[PHASE 27.0] Session token not available!');
      return {
        success: false,
        error: 'Session token not available'
      };
    }

    return {
      success: true,
      token: token
    };
  });

  // PHASE 28.0: Get API port
  ipcMain.handle('vnite:get-api-port', async () => {
    const port = global.apiPort;

    if (!port) {
      console.error('[PHASE 28.0] API port not available!');
      return {
        success: false,
        error: 'API port not available'
      };
    }

    return {
      success: true,
      port: port
    };
  });

  // PHASE 26.0: Open Logs Folder
  ipcMain.handle('vnite:open-logs-folder', async () => {
    const isDev = !app.isPackaged;
    const APP_ROOT = isDev ? path.join(__dirname, '..') : path.dirname(process.execPath);
    const LOG_DIR = path.join(APP_ROOT, 'logs');

    // Ensure log directory exists
    if (!fs.existsSync(LOG_DIR)) {
      fs.mkdirSync(LOG_DIR, { recursive: true });
    }

    // Open in system file manager
    shell.openPath(LOG_DIR);

    return {
      success: true,
      path: LOG_DIR
    };
  });

  // PHASE 26.0: Export Logs (create zip)
  ipcMain.handle('vnite:export-logs', async () => {
    const archiver = require('archiver');
    const isDev = !app.isPackaged;
    const APP_ROOT = isDev ? path.join(__dirname, '..') : path.dirname(process.execPath);
    const LOG_DIR = path.join(APP_ROOT, 'logs');

    // Create zip file in app root
    const zipPath = path.join(APP_ROOT, `vnite-logs-${Date.now()}.zip`);
    const output = fs.createWriteStream(zipPath);
    const archive = archiver('zip', { zlib: { level: 9 } });

    return new Promise((resolve, reject) => {
      output.on('close', () => {
        console.log(`[PHASE 26.0] Logs exported: ${zipPath} (${archive.pointer()} bytes)`);
        resolve({
          success: true,
          path: zipPath,
          size: archive.pointer()
        });
      });

      archive.on('error', (err) => {
        console.error('[PHASE 26.0] Export logs error:', err);
        reject({
          success: false,
          error: err.message
        });
      });

      archive.pipe(output);
      archive.directory(LOG_DIR, 'logs');
      archive.finalize();
    });
  });

  // PHASE 26.0: Get System Info for debugging
  ipcMain.handle('vnite:get-system-info', async () => {
    const isDev = !app.isPackaged;
    const APP_ROOT = isDev ? path.join(__dirname, '..') : path.dirname(process.execPath);
    const LOG_DIR = path.join(APP_ROOT, 'logs');

    // Get log files
    let logFiles = [];
    if (fs.existsSync(LOG_DIR)) {
      logFiles = fs.readdirSync(LOG_DIR)
        .filter(f => f.endsWith('.log'))
        .map(f => {
          const filePath = path.join(LOG_DIR, f);
          const stats = fs.statSync(filePath);
          return {
            name: f,
            size: stats.size,
            modified: stats.mtime
          };
        })
        .sort((a, b) => b.modified - a.modified);
    }

    return {
      success: true,
      system: {
        platform: process.platform,
        arch: process.arch,
        nodeVersion: process.versions.node,
        electronVersion: process.versions.electron,
        appPath: app.getAppPath(),
        execPath: process.execPath
      },
      paths: {
        appRoot: APP_ROOT,
        logDir: LOG_DIR,
        userData: app.getPath('userData'),
        isDev: isDev
      },
      logFiles: logFiles,
      totalLogSize: logFiles.reduce((sum, f) => sum + f.size, 0)
    };
  });

  console.log('[PHASE 26.0] IPC handlers registered');
}

module.exports = {
  registerIpcHandlers
};
