import { useState, useEffect } from 'react';
import { X, FolderOpen, Trash2, RefreshCw } from 'lucide-react';

interface ScanStatus {
  mode: string;
  is_running: boolean;
  library_root: string;
}

interface TrashStatus {
  trash_items: number;
  trash_size_gb: number;
  max_size_gb: number;
}

interface HealthStatus {
  status: string;
  service: string;
  version: string;
  env: string;
  sandbox: boolean;
}

export default function SettingsPanel({ onClose }: { onClose: () => void }) {
  const [scanStatus, setScanStatus] = useState<ScanStatus | null>(null);
  const [trashStatus, setTrashStatus] = useState<TrashStatus | null>(null);
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'general' | 'scanner' | 'trash'>('general');

  useEffect(() => {
    fetchAllData();
  }, []);

  const fetchAllData = async () => {
    try {
      setLoading(true);

      // Fetch all statuses in parallel
      const [scanRes, trashRes, healthRes] = await Promise.all([
        fetch('/api/scan/status'),
        fetch('/api/trash/status'),
        fetch('/api/health')
      ]);

      if (scanRes.ok) setScanStatus(await scanRes.json());
      if (trashRes.ok) setTrashStatus(await trashRes.json());
      if (healthRes.ok) setHealth(await healthRes.json());
    } catch (error) {
      console.error('Failed to fetch settings data:', error);
    } finally {
      setLoading(false);
    }
  };

  const toggleScannerMode = async () => {
    if (!scanStatus) return;

    const newMode = scanStatus.mode === 'manual' ? 'realtime' : 'manual';
    try {
      const response = await fetch('/api/scan/mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode: newMode }),
      });

      if (response.ok) {
        fetchAllData();
      }
    } catch (error) {
      console.error('Failed to toggle scanner:', error);
    }
  };

  const emptyTrash = async () => {
    if (!confirm('Are you sure you want to empty the trash?')) return;

    try {
      const response = await fetch('/api/trash/empty', {
        method: 'POST'
      });

      if (response.ok) {
        fetchAllData();
        alert('Trash emptied successfully');
      }
    } catch (error) {
      console.error('Failed to empty trash:', error);
    }
  };

  if (loading) {
    return (
      <div className="fixed inset-0 bg-zinc-950/80 backdrop-blur-sm z-50 flex items-center justify-center">
        <div className="bg-zinc-900 rounded-lg p-8">
          <div className="w-12 h-12 border-4 border-zinc-700 border-t-zinc-500 rounded-full animate-spin"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-zinc-950/80 backdrop-blur-sm z-50">
      <div className="h-full flex">
        {/* Backdrop - click to close */}
        <div className="flex-1" onClick={onClose} />

        {/* Settings Panel */}
        <div className="w-full md:w-[600px] bg-zinc-900 border-l border-zinc-800 overflow-y-auto">
          {/* Header */}
          <div className="sticky top-0 z-10 flex justify-between items-center p-6 border-b border-zinc-800 bg-zinc-900">
            <h3 className="text-lg font-semibold">Settings</h3>
            <button
              onClick={onClose}
              className="p-2 hover:bg-zinc-800 rounded-md transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          {/* Tabs */}
          <div className="border-b border-zinc-800">
            <div className="flex">
              <button
                onClick={() => setActiveTab('general')}
                className={`px-6 py-3 text-sm font-medium border-b-2 transition-colors ${
                  activeTab === 'general'
                    ? 'border-zinc-500 text-zinc-100'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                General
              </button>
              <button
                onClick={() => setActiveTab('scanner')}
                className={`px-6 py-3 text-sm font-medium border-b-2 transition-colors ${
                  activeTab === 'scanner'
                    ? 'border-zinc-500 text-zinc-100'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                Scanner
              </button>
              <button
                onClick={() => setActiveTab('trash')}
                className={`px-6 py-3 text-sm font-medium border-b-2 transition-colors ${
                  activeTab === 'trash'
                    ? 'border-zinc-500 text-zinc-100'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                Trash
              </button>
            </div>
          </div>

          {/* Content */}
          <div className="p-6 space-y-6">
            {/* General Tab */}
            {activeTab === 'general' && health && (
              <div className="space-y-6">
                <div className="space-y-4">
                  <h4 className="text-sm font-medium text-zinc-400 uppercase">System Information</h4>
                  <div className="space-y-3">
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Service</span>
                      <span className="text-sm font-mono text-zinc-200">{health.service}</span>
                    </div>
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Version</span>
                      <span className="text-sm font-mono text-zinc-200">{health.version}</span>
                    </div>
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Environment</span>
                      <span className="text-sm font-mono text-zinc-200">{health.env}</span>
                    </div>
                    {scanStatus && (
                      <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                        <span className="text-sm text-zinc-400">Library Root</span>
                        <span className="text-sm font-mono text-zinc-200 truncate ml-4">{scanStatus.library_root}</span>
                      </div>
                    )}
                  </div>
                </div>

                {health.sandbox && (
                  <div className="p-4 bg-yellow-900/20 border border-yellow-700 rounded-md">
                    <p className="text-sm text-yellow-400 font-medium">⚠️ Sandbox Mode</p>
                    <p className="text-xs text-yellow-400/70 mt-1">Running in testing environment</p>
                  </div>
                )}
              </div>
            )}

            {/* Scanner Tab */}
            {activeTab === 'scanner' && scanStatus && (
              <div className="space-y-6">
                <div className="space-y-4">
                  <h4 className="text-sm font-medium text-zinc-400 uppercase">Scanner Status</h4>
                  <div className="space-y-3">
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Mode</span>
                      <span className="text-sm font-mono text-zinc-200 uppercase">{scanStatus.mode}</span>
                    </div>
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Status</span>
                      <span className={`text-sm font-medium ${scanStatus.is_running ? 'text-emerald-400' : 'text-zinc-400'}`}>
                        {scanStatus.is_running ? 'Running' : 'Idle'}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="space-y-4">
                  <h4 className="text-sm font-medium text-zinc-400 uppercase">Actions</h4>
                  <button
                    onClick={toggleScannerMode}
                    className="w-full flex items-center justify-center gap-2 py-3 bg-zinc-700 hover:bg-zinc-600 rounded-md text-sm font-medium transition-all"
                  >
                    <RefreshCw className="w-4 h-4" />
                    Toggle to {scanStatus.mode === 'manual' ? 'Realtime' : 'Manual'}
                  </button>
                </div>
              </div>
            )}

            {/* Trash Tab */}
            {activeTab === 'trash' && trashStatus && (
              <div className="space-y-6">
                <div className="space-y-4">
                  <h4 className="text-sm font-medium text-zinc-400 uppercase">Trash Status</h4>
                  <div className="space-y-3">
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Items</span>
                      <span className="text-sm font-mono text-zinc-200">{trashStatus.trash_items}</span>
                    </div>
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Size</span>
                      <span className="text-sm font-mono text-zinc-200">{trashStatus.trash_size_gb.toFixed(2)} GB</span>
                    </div>
                    <div className="flex justify-between items-center p-3 bg-zinc-950 rounded-md">
                      <span className="text-sm text-zinc-400">Max Size</span>
                      <span className="text-sm font-mono text-zinc-200">{trashStatus.max_size_gb} GB</span>
                    </div>
                  </div>
                </div>

                <div className="space-y-4">
                  <h4 className="text-sm font-medium text-zinc-400 uppercase">Actions</h4>
                  <button
                    onClick={emptyTrash}
                    className="w-full flex items-center justify-center gap-2 py-3 bg-red-900/30 hover:bg-red-900/50 text-red-400 border border-red-900/50 rounded-md text-sm font-medium transition-all"
                  >
                    <Trash2 className="w-4 h-4" />
                    Empty Trash
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="p-6 border-t border-zinc-800">
            <button
              onClick={fetchAllData}
              className="w-full flex items-center justify-center gap-2 py-2 bg-zinc-800 hover:bg-zinc-700 rounded-md text-sm transition-all"
            >
              <RefreshCw className="w-4 h-4" />
              Refresh
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
