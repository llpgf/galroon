import React, { useState, useEffect } from 'react';
import { Trash2, HardDrive, Clock, RefreshCw, AlertTriangle } from 'lucide-react';
import { api } from '../api/client';
import toast from 'react-hot-toast';

/**
 * Trash Status (from backend)
 */
interface TrashStatus {
  trash_items: number;
  trash_size_gb: number;
}

/**
 * Trash Config (from backend)
 */
interface TrashConfig {
  max_size_gb: number;
  retention_days: number;
  min_disk_free_gb: number;
}

/**
 * TrashView Component - Phase 19: ✅ IMPLEMENTED
 *
 * Displays and manages the trash:
 * - Shows current trash status (count, size)
 * - Displays trash configuration
 * - Allows emptying trash
 */
export const TrashView: React.FC = () => {
  const [status, setStatus] = useState<TrashStatus | null>(null);
  const [config, setConfig] = useState<TrashConfig | null>(null);
  const [isLoadingStatus, setIsLoadingStatus] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(false);
  const [is_emptying, setIsEmptying] = useState(false);

  /**
   * Fetch trash status on mount
   */
  useEffect(() => {
    fetchTrashStatus();
    fetchTrashConfig();
  }, []);

  /**
   * Fetch trash status
   */
  const fetchTrashStatus = async () => {
    setIsLoadingStatus(true);
    try {
      const response = await api.getTrashStatus();
      setStatus(response.data);
    } catch (error: any) {
      console.error('Failed to fetch trash status:', error);
      const errorMsg = error?.response?.data?.detail || 'Failed to fetch trash status';
      toast.error(errorMsg);
    } finally {
      setIsLoadingStatus(false);
    }
  };

  /**
   * Fetch trash configuration
   */
  const fetchTrashConfig = async () => {
    setIsLoadingConfig(true);
    try {
      const response = await api.getTrashConfig();
      setConfig(response.data);
    } catch (error: any) {
      console.error('Failed to fetch trash config:', error);
      const errorMsg = error?.response?.data?.detail || 'Failed to fetch trash config';
      toast.error(errorMsg);
    } finally {
      setIsLoadingConfig(false);
    }
  };

  /**
   * Handle empty trash
   */
  const handleEmptyTrash = async () => {
    if (!confirm('Are you sure you want to permanently delete all trash? This action cannot be undone!')) {
      return;
    }

    setIsEmptying(true);
    try {
      const response = await api.emptyTrash();
      toast.success(response.data.message || 'Trash emptied successfully');
      // Refresh status
      await fetchTrashStatus();
    } catch (error: any) {
      console.error('Failed to empty trash:', error);
      const errorMsg = error?.response?.data?.detail || 'Failed to empty trash';
      toast.error(errorMsg);
    } finally {
      setIsEmptying(false);
    }
  };

  /**
   * Format size for display
   */
  const formatSize = (size_gb: number) => {
    if (size_gb < 1) {
      return `${(size_gb * 1024).toFixed(0)} MB`;
    }
    return `${size_gb.toFixed(2)} GB`;
  };

  return (
    <div className="flex-1 overflow-y-auto p-8">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <div className="flex items-center gap-3 mb-2">
            <Trash2 size={32} className="text-red-400" />
            <h1 className="text-3xl font-bold text-white">Trash</h1>
          </div>
          <p className="text-zinc-400">
            Manage deleted items. Items in trash can be restored until they are permanently deleted.
          </p>
        </div>

        {/* Trash Status Card */}
        <div className="bg-zinc-800 border border-zinc-700 rounded-xl p-6 mb-6">
          <h2 className="text-xl font-semibold text-white mb-4">Trash Status</h2>

          {isLoadingStatus ? (
            <div className="flex items-center justify-center py-8">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
              <p className="ml-3 text-zinc-400">Loading trash status...</p>
            </div>
          ) : status ? (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              {/* Items Count */}
              <div className="flex items-center gap-4 p-4 bg-zinc-900 border border-zinc-700 rounded-lg">
                <div className="p-3 bg-red-500/20 rounded-lg">
                  <Trash2 size={24} className="text-red-400" />
                </div>
                <div>
                  <p className="text-sm text-zinc-400">Items</p>
                  <p className="text-2xl font-semibold text-white">{status.trash_items}</p>
                </div>
              </div>

              {/* Total Size */}
              <div className="flex items-center gap-4 p-4 bg-zinc-900 border border-zinc-700 rounded-lg">
                <div className="p-3 bg-blue-500/20 rounded-lg">
                  <HardDrive size={24} className="text-blue-400" />
                </div>
                <div>
                  <p className="text-sm text-zinc-400">Total Size</p>
                  <p className="text-2xl font-semibold text-white">{formatSize(status.trash_size_gb)}</p>
                </div>
              </div>

              {/* Refresh Button */}
              <div className="flex items-center">
                <button
                  onClick={fetchTrashStatus}
                  disabled={isLoadingStatus}
                  className="flex-1 flex items-center justify-center gap-2 p-4 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  <RefreshCw size={20} className={isLoadingStatus ? 'animate-spin' : ''} />
                  Refresh
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-center py-8 text-zinc-400">
              Failed to load trash status
            </div>
          )}
        </div>

        {/* Trash Configuration Card */}
        <div className="bg-zinc-800 border border-zinc-700 rounded-xl p-6 mb-6">
          <h2 className="text-xl font-semibold text-white mb-4">Configuration</h2>

          {isLoadingConfig ? (
            <div className="flex items-center justify-center py-8">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
              <p className="ml-3 text-zinc-400">Loading configuration...</p>
            </div>
          ) : config ? (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* Max Size */}
              <div className="flex items-center gap-4 p-4 bg-zinc-900 border border-zinc-700 rounded-lg">
                <div className="p-3 bg-purple-500/20 rounded-lg">
                  <HardDrive size={24} className="text-purple-400" />
                </div>
                <div>
                  <p className="text-sm text-zinc-400">Max Trash Size</p>
                  <p className="text-lg font-semibold text-white">
                    {config.max_size_gb === 0 ? 'Unlimited' : `${config.max_size_gb} GB`}
                  </p>
                </div>
              </div>

              {/* Retention Days */}
              <div className="flex items-center gap-4 p-4 bg-zinc-900 border border-zinc-700 rounded-lg">
                <div className="p-3 bg-green-500/20 rounded-lg">
                  <Clock size={24} className="text-green-400" />
                </div>
                <div>
                  <p className="text-sm text-zinc-400">Retention Days</p>
                  <p className="text-lg font-semibold text-white">{config.retention_days} days</p>
                </div>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-center py-8 text-zinc-400">
              Failed to load configuration
            </div>
          )}
        </div>

        {/* Actions Card */}
        <div className="bg-zinc-800 border border-zinc-700 rounded-xl p-6">
          <h2 className="text-xl font-semibold text-white mb-4">Actions</h2>

          {/* Warning Message */}
          <div className="flex items-start gap-3 p-4 bg-red-500/10 border border-red-500/50 rounded-lg mb-6">
            <AlertTriangle size={20} className="text-red-400 flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-white font-medium mb-1">Warning</p>
              <p className="text-sm text-zinc-300">
                Emptying the trash will permanently delete all items. This action cannot be undone.
              </p>
            </div>
          </div>

          {/* Empty Trash Button */}
          <button
            onClick={handleEmptyTrash}
            disabled={is_emptying || !status || status.trash_items === 0}
            className="w-full flex items-center justify-center gap-2 p-4 bg-red-600 hover:bg-red-700 disabled:bg-zinc-700 disabled:text-zinc-500 disabled:cursor-not-allowed text-white rounded-lg transition-colors font-medium"
          >
            <Trash2 size={20} />
            {is_emptying ? 'Emptying...' : 'Empty Trash'}
          </button>

          {status && status.trash_items === 0 && (
            <p className="text-center text-sm text-zinc-500 mt-3">
              Trash is empty
            </p>
          )}
        </div>
      </div>
    </div>
  );
};

export default TrashView;
