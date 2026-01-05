/**
 * BackupManager - Roon-style Backup/Restore Component
 *
 * Phase 24.5: System Governance - The Time Machine
 *
 * Features:
 * - Create backup button
 * - List of backups (date, size)
 * - Restore button with confirmation
 * - Delete button with confirmation
 * - Auto-prune settings
 * - Backup statistics
 */

import React, { useState, useEffect } from 'react';
import {
  Download,
  Upload,
  Trash2,
  RefreshCw,
  HardDrive,
  Calendar,
  FileArchive,
  Settings,
  AlertTriangle,
  CheckCircle
} from 'lucide-react';
import { api } from '../../api/client';

interface Backup {
  filename: string;
  created_at: string;
  size_bytes: number;
  size_mb: number;
  version: string;
}

interface BackupStats {
  total_backups: number;
  total_size_bytes: number;
  total_size_mb: number;
  max_backups: number;
  oldest_backup: string | null;
  newest_backup: string | null;
}

interface BackupManagerProps {
  onRestore?: () => void;
}

export const BackupManager: React.FC<BackupManagerProps> = ({
  onRestore,
}) => {
  const [backups, setBackups] = useState<Backup[]>([]);
  const [stats, setStats] = useState<BackupStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [isRestoring, setIsRestoring] = useState<string | false>(false);
  const [isDeleting, setIsDeleting] = useState<string | false>(false);
  const [maxBackups, setMaxBackups] = useState(10);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  /**
   * Load backups and stats
   */
  const loadData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const [backupsResponse, statsResponse] = await Promise.all([
        api.listBackups(),
        api.getBackupStats(),
      ]);

      if (backupsResponse.data) {
        setBackups(backupsResponse.data.backups);
      }

      if (statsResponse.data) {
        setStats(statsResponse.data);
        setMaxBackups(statsResponse.data.max_backups);
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to load backups');
      console.error('Failed to load backups:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  /**
   * Create a new backup
   */
  const handleCreateBackup = async () => {
    setIsCreating(true);
    setError(null);
    setSuccess(null);

    try {
      const response = await api.createBackup();

      if (response.data) {
        setSuccess('Backup created successfully!');
        await loadData(); // Reload backups
        onRestore?.();

        // Clear success message after 3 seconds
        setTimeout(() => setSuccess(null), 3000);
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to create backup');
      console.error('Failed to create backup:', err);
    } finally {
      setIsCreating(false);
    }
  };

  /**
   * Restore a backup
   */
  const handleRestore = async (filename: string) => {
    if (!confirm(`Are you sure you want to restore backup: ${filename}?\n\nThis will overwrite your current database and settings!`)) {
      return;
    }

    setIsRestoring(filename);
    setError(null);
    setSuccess(null);

    try {
      const response = await api.restoreBackup({ filename });

      if (response.data) {
        setSuccess('Backup restored successfully!');
        onRestore?.();

        // Clear success message after 3 seconds
        setTimeout(() => setSuccess(null), 3000);
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to restore backup');
      console.error('Failed to restore backup:', err);
    } finally {
      setIsRestoring('');
    }
  };

  /**
   * Delete a backup
   */
  const handleDelete = async (filename: string) => {
    if (!confirm(`Are you sure you want to delete backup: ${filename}?`)) {
      return;
    }

    setIsDeleting(filename);
    setError(null);
    setSuccess(null);

    try {
      const response = await api.deleteBackup(filename);

      if (response.data) {
        setSuccess('Backup deleted successfully!');
        await loadData(); // Reload backups

        // Clear success message after 3 seconds
        setTimeout(() => setSuccess(null), 3000);
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to delete backup');
      console.error('Failed to delete backup:', err);
    } finally {
      setIsDeleting('');
    }
  };

  /**
   * Update max backups setting
   */
  const handleUpdateMaxBackups = async (value: number) => {
    setError(null);
    setSuccess(null);

    try {
      const response = await api.setMaxBackups(value);

      if (response.data) {
        setStats(response.data);
        setSuccess(`Max backups set to ${value}`);
        await loadData(); // Reload backups

        // Clear success message after 3 seconds
        setTimeout(() => setSuccess(null), 3000);
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to update max backups');
      console.error('Failed to update max backups:', err);
    }
  };

  /**
   * Format date for display
   */
  const formatDate = (isoDate: string) => {
    const date = new Date(isoDate);
    return date.toLocaleString();
  };

  return (
    <div className="bg-zinc-900 rounded-lg p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <HardDrive size={24} className="text-blue-400" />
          <div>
            <h2 className="text-xl font-bold text-white">Backup & Restore</h2>
            <p className="text-sm text-zinc-400">The Time Machine - Protect your library</p>
          </div>
        </div>

        <button
          onClick={handleCreateBackup}
          disabled={isCreating}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
        >
          {isCreating ? (
            'Creating...'
          ) : (
            <>
              <Download size={18} />
              Create Backup
            </>
          )}
        </button>
      </div>

      {/* Error Message */}
      {error && (
        <div className="mb-4 p-3 bg-red-900/30 border border-red-700 rounded-lg">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      {/* Success Message */}
      {success && (
        <div className="mb-4 p-3 bg-green-900/30 border border-green-700 rounded-lg">
          <p className="text-green-400 text-sm">{success}</p>
        </div>
      )}

      {/* Statistics */}
      {stats && (
        <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-3">
          <div className="p-3 bg-zinc-800 rounded-lg">
            <div className="text-xs text-zinc-400 mb-1">Total Backups</div>
            <div className="text-lg font-bold text-white">{stats.total_backups}</div>
          </div>

          <div className="p-3 bg-zinc-800 rounded-lg">
            <div className="text-xs text-zinc-400 mb-1">Total Size</div>
            <div className="text-lg font-bold text-white">{stats.total_size_mb} MB</div>
          </div>

          <div className="p-3 bg-zinc-800 rounded-lg">
            <div className="text-xs text-zinc-400 mb-1">Max Backups</div>
            <div className="text-lg font-bold text-white">{stats.max_backups}</div>
          </div>
        </div>
      )}

      {/* Auto-Prune Settings */}
      {stats && (
        <div className="mb-6 p-4 bg-zinc-800 rounded-lg">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Settings size={20} className="text-zinc-400" />
              <div>
                <div className="text-sm font-medium text-white">Auto-Prune Settings</div>
                <div className="text-xs text-zinc-400">Automatically delete old backups</div>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <label className="text-sm text-zinc-400">Keep last</label>
              <input
                type="number"
                min={1}
                max={100}
                value={maxBackups}
                onChange={(e) => setMaxBackups(parseInt(e.target.value) || 1)}
                onBlur={(e) => handleUpdateMaxBackups(parseInt(e.target.value) || 1)}
                className="w-16 px-2 py-1 bg-zinc-900 text-white rounded border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <label className="text-sm text-zinc-400">backups</label>
            </div>
          </div>
        </div>
      )}

      {/* Backups List */}
      {isLoading ? (
        <div className="text-center py-8">
          <RefreshCw size={32} className="text-zinc-600 mx-auto mb-3 animate-spin" />
          <p className="text-zinc-400">Loading backups...</p>
        </div>
      ) : backups.length === 0 ? (
        <div className="text-center py-8 bg-zinc-800 rounded-lg">
          <FileArchive size={48} className="text-zinc-600 mx-auto mb-3" />
          <p className="text-zinc-400 mb-2">No backups found</p>
          <p className="text-sm text-zinc-500">
            Create your first backup to protect your library
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {backups.map((backup) => (
            <div
              key={backup.filename}
              className="p-4 bg-zinc-800 rounded-lg hover:bg-zinc-750 transition-colors"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3 flex-1">
                  <FileArchive size={20} className="text-blue-400" />

                  <div className="flex-1">
                    <div className="text-white font-medium">{backup.filename}</div>
                    <div className="text-sm text-zinc-400 flex items-center gap-3">
                      <span className="flex items-center gap-1">
                        <Calendar size={14} />
                        {formatDate(backup.created_at)}
                      </span>
                      <span>{backup.size_mb} MB</span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    onClick={() => handleRestore(backup.filename)}
                    disabled={isRestoring === backup.filename}
                    className="px-3 py-1.5 bg-green-600 text-white rounded hover:bg-green-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-1.5 text-sm"
                    title="Restore backup"
                  >
                    {isRestoring === backup.filename ? (
                      'Restoring...'
                    ) : (
                      <>
                        <Upload size={16} />
                        Restore
                      </>
                    )}
                  </button>

                  <button
                    onClick={() => handleDelete(backup.filename)}
                    disabled={isDeleting === backup.filename}
                    className="px-3 py-1.5 bg-red-600 text-white rounded hover:bg-red-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-1.5 text-sm"
                    title="Delete backup"
                  >
                    {isDeleting === backup.filename ? (
                      'Deleting...'
                    ) : (
                      <>
                        <Trash2 size={16} />
                        Delete
                      </>
                    )}
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Warning */}
      <div className="mt-6 p-3 bg-amber-900/20 border border-amber-700 rounded-lg">
        <div className="flex items-start gap-2">
          <AlertTriangle size={20} className="text-amber-400 flex-shrink-0 mt-0.5" />
          <div className="text-sm text-amber-400">
            <strong>Warning:</strong> Restoring a backup will overwrite your current database and settings.
            Make sure to create a new backup before restoring if you're unsure.
          </div>
        </div>
      </div>
    </div>
  );
};

export default BackupManager;
