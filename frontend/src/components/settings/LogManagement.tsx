/**
 * LogManagement Component
 *
 * Phase 26.0: Portable Telemetry - Log Management UI
 *
 * Features:
 * - View system information
 * - List log files with sizes
 * - Open logs folder in system file manager
 * - Export logs to ZIP file
 */

import React, { useState, useEffect } from 'react';
import {
  FileText,
  FolderOpen,
  Download,
  RefreshCw,
  HardDrive,
  Info,
  CheckCircle,
  AlertCircle
} from 'lucide-react';
import toast from 'react-hot-toast';

/**
 * Log File Info
 */
interface LogFile {
  name: string;
  size: number;
  modified: string;
}

/**
 * System Info
 */
interface SystemInfo {
  system: {
    platform: string;
    arch: string;
    nodeVersion: string;
    electronVersion: string;
    appPath: string;
    execPath: string;
  };
  paths: {
    appRoot: string;
    logDir: string;
    userData: string;
    isDev: boolean;
  };
  logFiles: LogFile[];
  totalLogSize: number;
}

/**
 * Format bytes to human readable
 */
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

/**
 * Format timestamp
 */
function formatTimestamp(timestamp: string): string {
  return new Date(timestamp).toLocaleString();
}

export const LogManagement: React.FC = () => {
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    loadSystemInfo();
  }, []);

  const loadSystemInfo = async () => {
    setIsLoading(true);
    try {
      // Check if running in Electron
      if (typeof window !== 'undefined' && (window as any).electronAPI?.logs) {
        const info = await (window as any).electronAPI.logs.getSystemInfo();
        if (info.success) {
          setSystemInfo(info);
        } else {
          toast.error('Failed to load system information');
        }
      } else {
        toast.error('Log management only available in desktop app');
      }
    } catch (error: any) {
      console.error('Failed to load system info:', error);
      toast.error('Failed to load system information');
    } finally {
      setIsLoading(false);
    }
  };

  const openLogsFolder = async () => {
    try {
      if (typeof window !== 'undefined' && (window as any).electronAPI?.logs) {
        const result = await (window as any).electronAPI.logs.openFolder();
        if (result.success) {
          toast.success('Logs folder opened');
        } else {
          toast.error('Failed to open logs folder');
        }
      }
    } catch (error: any) {
      console.error('Failed to open logs folder:', error);
      toast.error('Failed to open logs folder');
    }
  };

  const exportLogs = async () => {
    setIsExporting(true);
    try {
      if (typeof window !== 'undefined' && (window as any).electronAPI?.logs) {
        const result = await (window as any).electronAPI.logs.export();
        if (result.success) {
          toast.success(`Logs exported: ${result.path} (${formatBytes(result.size)})`);
        } else {
          toast.error('Failed to export logs');
        }
      }
    } catch (error: any) {
      console.error('Failed to export logs:', error);
      toast.error('Failed to export logs');
    } finally {
      setIsExporting(false);
    }
  };

  if (!systemInfo) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <RefreshCw className={`w-8 h-8 mx-auto mb-4 text-zinc-400 ${isLoading ? 'animate-spin' : ''}`} />
          <p className="text-zinc-400">Loading system information...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Header */}
      <div>
        <h2 className="text-2xl font-bold text-white mb-2">Log Management</h2>
        <p className="text-zinc-400">
          View and export application logs for debugging and support
        </p>
      </div>

      {/* System Information */}
      <div className="bg-zinc-900/50 rounded-lg p-6 border border-zinc-800">
        <div className="flex items-center gap-3 mb-4">
          <Info className="w-5 h-5 text-blue-400" />
          <h3 className="text-lg font-semibold text-white">System Information</h3>
        </div>

        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-zinc-500">Platform:</span>
            <span className="ml-2 text-zinc-300">{systemInfo.system.platform} {systemInfo.system.arch}</span>
          </div>
          <div>
            <span className="text-zinc-500">Electron:</span>
            <span className="ml-2 text-zinc-300">v{systemInfo.system.electronVersion}</span>
          </div>
          <div>
            <span className="text-zinc-500">Node.js:</span>
            <span className="ml-2 text-zinc-300">v{systemInfo.system.nodeVersion}</span>
          </div>
          <div>
            <span className="text-zinc-500">Mode:</span>
            <span className="ml-2 text-zinc-300">{systemInfo.paths.isDev ? 'Development' : 'Production'}</span>
          </div>
        </div>

        <div className="mt-4 space-y-2 text-sm">
          <div>
            <span className="text-zinc-500">App Root:</span>
            <span className="ml-2 text-zinc-300 font-mono text-xs">{systemInfo.paths.appRoot}</span>
          </div>
          <div>
            <span className="text-zinc-500">Log Directory:</span>
            <span className="ml-2 text-zinc-300 font-mono text-xs">{systemInfo.paths.logDir}</span>
          </div>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-4">
        <button
          onClick={openLogsFolder}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
        >
          <FolderOpen className="w-4 h-4" />
          Open Logs Folder
        </button>

        <button
          onClick={exportLogs}
          disabled={isExporting || systemInfo.logFiles.length === 0}
          className="flex items-center gap-2 px-4 py-2 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-500 text-white rounded-lg transition-colors"
        >
          {isExporting ? (
            <RefreshCw className="w-4 h-4 animate-spin" />
          ) : (
            <Download className="w-4 h-4" />
          )}
          Export Logs (ZIP)
        </button>

        <button
          onClick={loadSystemInfo}
          disabled={isLoading}
          className="flex items-center gap-2 px-4 py-2 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-500 text-white rounded-lg transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {/* Log Files */}
      <div className="bg-zinc-900/50 rounded-lg border border-zinc-800 overflow-hidden">
        <div className="flex items-center justify-between px-6 py-4 border-b border-zinc-800">
          <div className="flex items-center gap-3">
            <FileText className="w-5 h-5 text-zinc-400" />
            <h3 className="text-lg font-semibold text-white">Log Files</h3>
            <span className="text-sm text-zinc-500">
              ({systemInfo.logFiles.length} files, {formatBytes(systemInfo.totalLogSize)})
            </span>
          </div>
        </div>

        {systemInfo.logFiles.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-zinc-500">
            No log files found
          </div>
        ) : (
          <div className="divide-y divide-zinc-800">
            {systemInfo.logFiles.map((file, index) => (
              <div
                key={index}
                className="flex items-center justify-between px-6 py-3 hover:bg-zinc-800/50 transition-colors"
              >
                <div className="flex items-center gap-3 flex-1 min-w-0">
                  <FileText className="w-4 h-4 text-zinc-500 flex-shrink-0" />
                  <span className="text-sm text-zinc-300 font-mono truncate">{file.name}</span>
                </div>
                <div className="flex items-center gap-4 text-sm text-zinc-500">
                  <span>{formatBytes(file.size)}</span>
                  <span>{formatTimestamp(file.modified)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Help Text */}
      <div className="bg-blue-900/20 border border-blue-800/50 rounded-lg p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
          <div className="text-sm text-blue-200">
            <p className="font-medium mb-1">About Logs</p>
            <p className="text-blue-300/80">
              Logs are stored in the application directory (portable mode). You can open the logs folder
              to view them directly or export all logs to a ZIP file for sharing with support.
              Log files are automatically rotated when they reach 10MB.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
