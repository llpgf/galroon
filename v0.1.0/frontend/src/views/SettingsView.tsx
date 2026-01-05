/**
 * SettingsView - Roon-style Settings Interface
 *
 * Phase 24.5: System Governance - Settings UI Overhaul
 *
 * Features:
 * - Vertical navigation tabs (Roon-style)
 * - General: App settings, theme, language
 * - Storage: Library roots, trash management
 * - Library: Scanner config, scheduler settings
 * - Backups: Backup management, restore
 * - About: Version, credits
 */

import React, { useState, useEffect } from 'react';
import {
  Settings as SettingsIcon,
  HardDrive,
  Library,
  Archive,
  Info,
  X,
  FolderOpen,
  Plus,
  Trash2,
  RefreshCw,
  Download,
  FileText
} from 'lucide-react';
import { api } from '../api/client';
import toast from 'react-hot-toast';
import { directoryInputProps } from '../utils/domUtils';
import { LibraryStatus } from '../components/settings/LibraryStatus';
import { BackupManager } from '../components/settings/BackupManager';
import { UpdateManager } from '../components/settings/UpdateManager';
import { LogManagement } from '../components/settings/LogManagement';

/**
 * Library Root Data Structure
 */
interface LibraryRoot {
  id: string;
  path: string;
  exists: boolean;
}

/**
 * Scanner Config
 */
interface ScannerConfig {
  scan_on_startup: boolean;
  scan_interval_min: number;
}

/**
 * SettingsView Props
 */
interface SettingsViewProps {
  onBack?: () => void;
}

type Tab = 'general' | 'storage' | 'library' | 'backups' | 'updates' | 'logs' | 'about';

export const SettingsView: React.FC<SettingsViewProps> = ({ onBack }) => {
  const [activeTab, setActiveTab] = useState<Tab>('general');

  // Storage tab state
  const [libraryRoots, setLibraryRoots] = useState<LibraryRoot[]>([]);
  const [isLoadingRoots, setIsLoadingRoots] = useState(false);

  // Library tab state
  const [scannerConfig, setScannerConfig] = useState<ScannerConfig | null>(null);
  const [scanInterval, setScanInterval] = useState(0);

  // Refs
  const folderInputRef = React.useRef<HTMLInputElement>(null);

  /**
   * Fetch data on mount
   */
  useEffect(() => {
    fetchLibraryRoots();
    fetchScannerConfig();
  }, []);

  /**
   * Fetch library roots
   */
  const fetchLibraryRoots = async () => {
    setIsLoadingRoots(true);
    try {
      const response = await api.getLibraryRoots();
      setLibraryRoots(response.data?.roots || []);
    } catch (error: any) {
      console.error('Failed to fetch library roots:', error);
      toast.error('Failed to load library roots');
    } finally {
      setIsLoadingRoots(false);
    }
  };

  /**
   * Fetch scanner configuration
   */
  const fetchScannerConfig = async () => {
    try {
      const response = await api.getScannerConfig();
      setScannerConfig(response.data);
      setScanInterval(response.data?.scan_interval_min || 0);
    } catch (error: any) {
      console.error('Failed to fetch scanner config:', error);
    }
  };

  /**
   * Add library root
   */
  const handleAddRoot = async (path: string) => {
    try {
      await api.addLibraryRoot(path);
      toast.success('Library root added');
      fetchLibraryRoots();
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || 'Failed to add library root';
      toast.error(errorMsg);
    }
  };

  /**
   * Delete library root
   */
  const handleDeleteRoot = async (rootId: string) => {
    try {
      await api.deleteLibraryRoot(rootId);
      toast.success('Library root deleted');
      fetchLibraryRoots();
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || 'Failed to delete library root';
      toast.error(errorMsg);
    }
  };

  /**
   * Update scanner config
   */
  const handleUpdateScannerConfig = async () => {
    if (!scannerConfig) return;

    try {
      await api.updateScannerConfig(scannerConfig.scan_on_startup, scanInterval);
      toast.success('Scanner configuration updated');
      fetchScannerConfig();
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || 'Failed to update scanner config';
      toast.error(errorMsg);
    }
  };

  /**
   * Render tab icon
   */
  const getTabIcon = (tab: Tab) => {
    switch (tab) {
      case 'general':
        return <SettingsIcon size={20} />;
      case 'storage':
        return <HardDrive size={20} />;
      case 'library':
        return <Library size={20} />;
      case 'backups':
        return <Archive size={20} />;
      case 'updates':
        return <Download size={20} />;
      case 'logs':
        return <FileText size={20} />;
      case 'about':
        return <Info size={20} />;
    }
  };

  /**
   * Render tab content
   */
  const renderTabContent = () => {
    switch (activeTab) {
      case 'general':
        return renderGeneralTab();
      case 'storage':
        return renderStorageTab();
      case 'library':
        return renderLibraryTab();
      case 'backups':
        return renderBackupsTab();
      case 'updates':
        return renderUpdatesTab();
      case 'logs':
        return <LogManagement />;
      case 'about':
        return renderAboutTab();
    }
  };

  /**
   * General Tab
   */
  const renderGeneralTab = () => (
    <div className="space-y-6">
      <div className="bg-zinc-900 rounded-lg p-6">
        <h3 className="text-lg font-bold text-white mb-4">Application Settings</h3>

        <div className="space-y-4">
          <div className="flex items-center justify-between p-3 bg-zinc-800 rounded-lg">
            <div>
              <div className="text-white font-medium">Dark Mode</div>
              <div className="text-sm text-zinc-400">Use dark theme</div>
            </div>
            <div className="px-3 py-1 bg-green-600/20 text-green-400 rounded text-sm">
              Enabled
            </div>
          </div>

          <div className="flex items-center justify-between p-3 bg-zinc-800 rounded-lg">
            <div>
              <div className="text-white font-medium">Language</div>
              <div className="text-sm text-zinc-400">Interface language</div>
            </div>
            <select className="px-3 py-1 bg-zinc-700 text-white rounded border border-zinc-600 focus:outline-none focus:ring-2 focus:ring-blue-500">
              <option>English</option>
              <option>中文</option>
              <option>日本語</option>
            </select>
          </div>
        </div>
      </div>

      <div className="bg-zinc-900 rounded-lg p-6">
        <h3 className="text-lg font-bold text-white mb-4">Version Information</h3>
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-zinc-400">Application Version</span>
            <span className="text-white">1.0.0</span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-400">API Version</span>
            <span className="text-white">1.0.0</span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-400">Phase</span>
            <span className="text-white">24.5 - System Governance</span>
          </div>
        </div>
      </div>
    </div>
  );

  /**
   * Storage Tab
   */
  const renderStorageTab = () => (
    <div className="space-y-6">
      <div className="bg-zinc-900 rounded-lg p-6">
        <h3 className="text-lg font-bold text-white mb-4">Library Roots</h3>

        {isLoadingRoots ? (
          <div className="text-center py-8">
            <RefreshCw size={32} className="text-zinc-600 mx-auto mb-3 animate-spin" />
            <p className="text-zinc-400">Loading library roots...</p>
          </div>
        ) : (
          <>
            <div className="space-y-2 mb-4">
              {libraryRoots.map((root) => (
                <div
                  key={root.id}
                  className="flex items-center justify-between p-3 bg-zinc-800 rounded-lg"
                >
                  <div className="flex items-center gap-3 flex-1">
                    <FolderOpen size={20} className="text-blue-400" />
                    <div className="flex-1">
                      <div className="text-white font-medium">{root.path}</div>
                      <div className="text-xs text-zinc-400">
                        {root.exists ? '✓ Available' : '✗ Not found'}
                      </div>
                    </div>
                  </div>

                  <button
                    onClick={() => handleDeleteRoot(root.id)}
                    className="px-3 py-1.5 bg-red-600 text-white rounded hover:bg-red-700 transition-colors flex items-center gap-1.5 text-sm"
                  >
                    <Trash2 size={16} />
                    Delete
                  </button>
                </div>
              ))}
            </div>

            <button
              onClick={() => folderInputRef.current?.click()}
              className="w-full px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center justify-center gap-2"
            >
              <Plus size={18} />
              Add Library Root
            </button>

            <input
              ref={folderInputRef}
              type="file"
              {...directoryInputProps}
              className="hidden"
              onChange={(e) => {
                const files = e.target.files;
                if (files && files.length > 0) {
                  handleAddRoot(files[0].name);
                }
              }}
            />
          </>
        )}
      </div>
    </div>
  );

  /**
   * Library Tab
   */
  const renderLibraryTab = () => (
    <div className="space-y-6">
      <LibraryStatus />

      {scannerConfig && (
        <div className="bg-zinc-900 rounded-lg p-6">
          <h3 className="text-lg font-bold text-white mb-4">Scanner Configuration</h3>

          <div className="space-y-4">
            <div className="flex items-center justify-between p-3 bg-zinc-800 rounded-lg">
              <div>
                <div className="text-white font-medium">Scan on Startup</div>
                <div className="text-sm text-zinc-400">Automatically scan library when server starts</div>
              </div>
              <button
                onClick={() => {
                  setScannerConfig({
                    ...scannerConfig,
                    scan_on_startup: !scannerConfig.scan_on_startup
                  });
                }}
                className={`w-12 h-6 rounded-full transition-colors ${
                  scannerConfig.scan_on_startup
                    ? 'bg-blue-600'
                    : 'bg-zinc-700'
                }`}
              >
                <div
                  className={`w-5 h-5 bg-white rounded-full transition-transform ${
                    scannerConfig.scan_on_startup ? 'translate-x-6' : 'translate-x-0.5'
                  }`}
                />
              </button>
            </div>

            <div className="p-3 bg-zinc-800 rounded-lg">
              <div className="text-white font-medium mb-2">Scan Interval</div>
              <div className="text-sm text-zinc-400 mb-3">
                Automatic scan interval (0 = manual mode)
              </div>
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min={0}
                  max={1440}
                  value={scanInterval}
                  onChange={(e) => setScanInterval(parseInt(e.target.value) || 0)}
                  className="flex-1 px-3 py-2 bg-zinc-900 text-white rounded border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <span className="text-zinc-400">minutes</span>
              </div>
            </div>

            <button
              onClick={handleUpdateScannerConfig}
              className="w-full px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Save Configuration
            </button>
          </div>
        </div>
      )}
    </div>
  );

  /**
   * Backups Tab
   */
  const renderBackupsTab = () => (
    <div className="space-y-6">
      <BackupManager onRestore={() => {
        // Refresh data after restore
        fetchScannerConfig();
        fetchLibraryRoots();
      }} />
    </div>
  );

  /**
   * Updates Tab
   */
  const renderUpdatesTab = () => (
    <div className="space-y-6">
      <UpdateManager />
    </div>
  );

  /**
   * About Tab
   */
  const renderAboutTab = () => (
    <div className="space-y-6">
      <div className="bg-zinc-900 rounded-lg p-6 text-center">
        <div className="w-20 h-20 bg-blue-600 rounded-full flex items-center justify-center mx-auto mb-4">
          <Library size={40} className="text-white" />
        </div>
        <h2 className="text-2xl font-bold text-white mb-2">Galgame Library Manager</h2>
        <p className="text-zinc-400 mb-4">Enterprise-grade Visual Novel library management</p>
        <div className="text-sm text-zinc-500">Version 1.0.0</div>
      </div>

      <div className="bg-zinc-900 rounded-lg p-6">
        <h3 className="text-lg font-bold text-white mb-4">Phase 24.5 - System Governance</h3>
        <div className="space-y-3 text-sm text-zinc-400">
          <div className="flex items-start gap-2">
            <div className="w-2 h-2 bg-green-500 rounded-full mt-1.5 flex-shrink-0" />
            <div>
              <div className="text-white font-medium">Visual Scanner Engine</div>
              <div>Real-time progress tracking with pause/resume/cancel controls</div>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <div className="w-2 h-2 bg-green-500 rounded-full mt-1.5 flex-shrink-0" />
            <div>
              <div className="text-white font-medium">Task Scheduler</div>
              <div>APScheduler integration for automated scans and backups</div>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <div className="w-2 h-2 bg-green-500 rounded-full mt-1.5 flex-shrink-0" />
            <div>
              <div className="text-white font-medium">Time Machine</div>
              <div>Zip-based backup system with auto-prune and restore</div>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <div className="w-2 h-2 bg-green-500 rounded-full mt-1.5 flex-shrink-0" />
            <div>
              <div className="text-white font-medium">Roon-style Settings UI</div>
              <div>Elegant vertical tab navigation for all settings</div>
            </div>
          </div>
        </div>
      </div>

      <div className="bg-zinc-900 rounded-lg p-6">
        <h3 className="text-lg font-bold text-white mb-4">Credits</h3>
        <div className="text-sm text-zinc-400 space-y-1">
          <div>Developed with Claude (Sonnet 4.5)</div>
          <div>© 2026 Galgame Library Manager</div>
        </div>
      </div>
    </div>
  );

  return (
    <div className="flex h-screen bg-zinc-950">
      {/* Sidebar Navigation */}
      <div className="w-64 bg-zinc-900 border-r border-zinc-800 flex flex-col">
        {/* Header */}
        <div className="p-6 border-b border-zinc-800">
          <h1 className="text-xl font-bold text-white flex items-center gap-2">
            <SettingsIcon size={24} className="text-blue-400" />
            Settings
          </h1>
        </div>

        {/* Navigation Tabs */}
        <div className="flex-1 p-4 space-y-1">
          {(
            [
              { id: 'general', label: 'General' },
              { id: 'storage', label: 'Storage' },
              { id: 'library', label: 'Library' },
              { id: 'backups', label: 'Backups' },
              { id: 'updates', label: 'Updates' },
              { id: 'logs', label: 'Logs' },
              { id: 'about', label: 'About' },
            ] as const
          ).map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as Tab)}
              className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg transition-colors ${
                activeTab === tab.id
                  ? 'bg-blue-600 text-white'
                  : 'text-zinc-400 hover:bg-zinc-800 hover:text-white'
              }`}
            >
              {getTabIcon(tab.id as Tab)}
              <span className="font-medium">{tab.label}</span>
            </button>
          ))}
        </div>

        {/* Back Button */}
        <div className="p-4 border-t border-zinc-800">
          <button
            onClick={onBack}
            className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-zinc-800 text-zinc-300 rounded-lg hover:bg-zinc-700 hover:text-white transition-colors"
          >
            <X size={18} />
            Close
          </button>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-8">
          {/* Tab Header */}
          <div className="mb-6">
            <div className="flex items-center gap-3 mb-2">
              {getTabIcon(activeTab)}
              <h2 className="text-2xl font-bold text-white capitalize">{activeTab}</h2>
            </div>
            <p className="text-zinc-400">
              {activeTab === 'general' && 'Application settings and preferences'}
              {activeTab === 'storage' && 'Manage library roots and storage'}
              {activeTab === 'library' && 'Scanner configuration and scheduled tasks'}
              {activeTab === 'backups' && 'Backup and restore your library'}
              {activeTab === 'updates' && 'Check for application updates'}
              {activeTab === 'logs' && 'View and export application logs'}
              {activeTab === 'about' && 'Version information and credits'}
            </p>
          </div>

          {/* Tab Content */}
          {renderTabContent()}
        </div>
      </div>
    </div>
  );
};

export default SettingsView;
