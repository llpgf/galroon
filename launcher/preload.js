/**
 * Electron Preload Script - Galroon
 *
 * Phase 25.0: Security bridge between main and renderer
 * Phase 26.0: Log management APIs
 */

const { contextBridge, ipcRenderer } = require('electron');

// Expose protected methods to renderer
contextBridge.exposeInMainWorld('electronAPI', {
  getVersions: () => process.versions,
  getPlatform: () => process.platform,

  // Environment info
  isDev: () => process.env.NODE_ENV === 'development',

  // Paths (read-only)
  getPath: (name) => {
    const { app } = require('electron');
    return app.getPath(name);
  },

  // PHASE 26.0: Log Management APIs
  logs: {
    // Open logs folder in system file manager
    openFolder: () => ipcRenderer.invoke('galroon:open-logs-folder'),

    // Export logs to zip file
    export: () => ipcRenderer.invoke('galroon:export-logs'),

    // Get system info and log files
    getSystemInfo: () => ipcRenderer.invoke('galroon:get-system-info')
  }
});

console.log('Preload script loaded (Phase 26.0: Log Management)');
