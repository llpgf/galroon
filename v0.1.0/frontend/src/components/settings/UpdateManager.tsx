/**
 * UpdateManager - Auto-Update Component
 *
 * Phase 24.5: System Governance - Auto-update system
 *
 * Features:
 * - Check for updates from GitHub
 * - Display update availability
 * - Auto-check configuration
 * - Release notes display
 * - Download link
 */

import React, { useState, useEffect } from 'react';
import {
  Download,
  RefreshCw,
  CheckCircle,
  AlertCircle,
  Info,
  Bell,
  BellOff
} from 'lucide-react';
import { api } from '../../api/client';
import toast from 'react-hot-toast';

interface UpdateInfo {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  release_url: string | null;
  release_notes: string | null;
  published_at: string | null;
}

interface UpdateConfig {
  auto_check_enabled: boolean;
  check_interval_hours: number;
  last_check_at: string | null;
}

interface UpdateManagerProps {
  onUpdateAvailable?: (info: UpdateInfo) => void;
}

export const UpdateManager: React.FC<UpdateManagerProps> = ({
  onUpdateAvailable,
}) => {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateConfig, setUpdateConfig] = useState<UpdateConfig | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(false);

  // Load config on mount
  useEffect(() => {
    loadConfig();
    // Auto-check if enabled
    checkForUpdates(false);
  }, []);

  const loadConfig = async () => {
    setIsLoadingConfig(true);
    try {
      const response = await api.getUpdateConfig();
      setUpdateConfig(response.data);
    } catch (err) {
      console.error('Failed to load update config:', err);
    } finally {
      setIsLoadingConfig(false);
    }
  };

  const checkForUpdates = async (showToast = true) => {
    setIsChecking(true);
    try {
      const response = await api.checkForUpdates();
      setUpdateInfo(response.data);

      if (response.data.has_update) {
        toast.success(`新版本 ${response.data.latest_version} 可用！`);
        onUpdateAvailable?.(response.data);
      } else if (showToast) {
        toast.success('已经是最新版本');
      }
    } catch (err: any) {
      const errorMsg = err.response?.data?.detail || '检查更新失败';
      if (showToast) {
        toast.error(errorMsg);
      }
      console.error('Failed to check for updates:', err);
    } finally {
      setIsChecking(false);
    }
  };

  const handleToggleAutoCheck = async () => {
    if (!updateConfig) return;

    try {
      const response = await api.updateUpdateConfig(
        !updateConfig.auto_check_enabled,
        updateConfig.check_interval_hours
      );
      setUpdateConfig(response.data);
      toast.success('更新设置已保存');
    } catch (err: any) {
      toast.error('保存更新设置失败');
      console.error('Failed to update config:', err);
    }
  };

  const handleIntervalChange = async (hours: number) => {
    if (!updateConfig) return;

    try {
      const response = await api.updateUpdateConfig(
        updateConfig.auto_check_enabled,
        hours
      );
      setUpdateConfig(response.data);
      toast.success('检查间隔已更新');
    } catch (err: any) {
      toast.error('更新检查间隔失败');
      console.error('Failed to update interval:', err);
    }
  };

  return (
    <div className="bg-zinc-900 rounded-lg p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <Download size={24} className="text-blue-400" />
          <div>
            <h2 className="text-xl font-bold text-white">自动更新</h2>
            <p className="text-sm text-zinc-400">检查和更新应用程序</p>
          </div>
        </div>

        <button
          onClick={() => checkForUpdates(true)}
          disabled={isChecking}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
        >
          {isChecking ? (
            <>
              <RefreshCw size={18} className="animate-spin" />
              检查中...
            </>
          ) : (
            <>
              <RefreshCw size={18} />
              检查更新
            </>
          )}
        </button>
      </div>

      {/* Update Status */}
      {updateInfo && (
        <div className="mb-6 p-4 rounded-lg border">
          {updateInfo.has_update ? (
            <div className="bg-green-900/20 border border-green-700 rounded-lg p-4">
              <div className="flex items-start gap-3">
                <CheckCircle size={24} className="text-green-400 flex-shrink-0 mt-0.5" />
                <div className="flex-1">
                  <div className="text-white font-medium mb-1">
                    发现新版本！
                  </div>
                  <div className="text-sm text-zinc-300 mb-3">
                    当前版本: <span className="font-mono">{updateInfo.current_version}</span>
                    {' → '}
                    最新版本: <span className="font-mono text-green-400">{updateInfo.latest_version}</span>
                  </div>

                  {updateInfo.published_at && (
                    <div className="text-xs text-zinc-400 mb-3">
                      发布时间: {new Date(updateInfo.published_at).toLocaleString('zh-CN')}
                    </div>
                  )}

                  {updateInfo.release_url && (
                    <a
                      href={updateInfo.release_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-2 px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors text-sm"
                    >
                      <Download size={16} />
                      前往 GitHub 下载
                    </a>
                  )}
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-blue-900/20 border border-blue-700 rounded-lg p-4">
              <div className="flex items-start gap-3">
                <CheckCircle size={24} className="text-blue-400 flex-shrink-0 mt-0.5" />
                <div className="flex-1">
                  <div className="text-white font-medium mb-1">
                    已经是最新版本
                  </div>
                  <div className="text-sm text-zinc-300">
                    当前版本: <span className="font-mono">{updateInfo.current_version}</span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Release Notes */}
      {updateInfo?.has_update && updateInfo.release_notes && (
        <div className="mb-6 p-4 bg-zinc-800 rounded-lg">
          <h3 className="text-sm font-medium text-zinc-300 mb-2">更新说明</h3>
          <div className="text-sm text-zinc-400 whitespace-pre-wrap max-h-40 overflow-y-auto">
            {updateInfo.release_notes}
          </div>
        </div>
      )}

      {/* Auto-Check Configuration */}
      {updateConfig && (
        <div className="p-4 bg-zinc-800 rounded-lg">
          <h3 className="text-lg font-bold text-white mb-4">自动检查设置</h3>

          <div className="space-y-4">
            {/* Auto-Check Toggle */}
            <div className="flex items-center justify-between">
              <div>
                <div className="text-white font-medium">启用自动检查</div>
                <div className="text-sm text-zinc-400">定期自动检查更新</div>
              </div>
              <button
                onClick={handleToggleAutoCheck}
                className={`w-12 h-6 rounded-full transition-colors ${
                  updateConfig.auto_check_enabled
                    ? 'bg-blue-600'
                    : 'bg-zinc-700'
                }`}
              >
                <div
                  className={`w-5 h-5 bg-white rounded-full transition-transform ${
                    updateConfig.auto_check_enabled ? 'translate-x-6' : 'translate-x-0.5'
                  }`}
                />
              </button>
            </div>

            {/* Check Interval */}
            {updateConfig.auto_check_enabled && (
              <div>
                <div className="text-white font-medium mb-2">检查间隔</div>
                <div className="text-sm text-zinc-400 mb-3">
                  自动检查更新的时间间隔
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={1}
                    max={168}
                    value={updateConfig.check_interval_hours}
                    onChange={(e) => {
                      const value = parseInt(e.target.value) || 24;
                      handleIntervalChange(value);
                    }}
                    className="w-24 px-3 py-2 bg-zinc-900 text-white rounded border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                  <span className="text-zinc-400">小时</span>
                </div>
                <div className="text-xs text-zinc-500 mt-1">
                  建议值: 24（每天检查一次）
                </div>
              </div>
            )}

            {/* Last Check */}
            {updateConfig.last_check_at && (
              <div className="text-sm text-zinc-400">
                上次检查: {new Date(updateConfig.last_check_at).toLocaleString('zh-CN')}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Info Box */}
      <div className="mt-6 p-3 bg-blue-900/20 border border-blue-700 rounded-lg">
        <div className="flex items-start gap-2">
          <Info size={20} className="text-blue-400 flex-shrink-0 mt-0.5" />
          <div className="text-sm text-blue-400">
            <strong>提示：</strong>
            应用程序会自动从 GitHub 检查更新。下载后请手动安装新版本。
          </div>
        </div>
      </div>
    </div>
  );
};

export default UpdateManager;
