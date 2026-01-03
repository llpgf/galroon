import { useState, useEffect } from 'react';
import {
  FolderOpen,
  Home,
  Trash2,
  Search,
  Settings as SettingsIcon,
  X,
  Folder,
  Edit3,
  Copy,
  RefreshCw,
  Database,
  Clock,
  Play,
  Pause,
  FolderOpen as FolderOpenIcon,
  History as HistoryIcon,
} from 'lucide-react';
import ScraperConsole from './components/ScraperConsole';
import TimeMachineLog from './components/TimeMachineLog';
import SettingsPanel from './components/SettingsPanel';
import ErrorBoundary from './components/ErrorBoundary';
import { api } from './api/client';
import toast from 'react-hot-toast';

// Type definitions
interface GameMetadata {
  title?: { value: string };
  title_jp?: { value: string };
  description?: { value: string };
  developer?: { value: string };
  release_date?: { value: string };
  genres?: { value: string[] };
  library_status?: { value: string };
  background_url?: { value: string };
}

interface Game {
  id: string;
  path: string;
  name: string;
  title: string;
  titleJP?: string;
  developer?: string;
  cover?: string;
  description?: string;
  releaseDate?: string;
  genres?: string[];
  metadata?: GameMetadata;
  is_dir?: boolean;
  playTime?: string;
  lastPlayed?: string;
  platform?: string;
  rating?: number;
  status?: string;
}

interface ScanStatus {
  mode: 'REALTIME' | 'MANUAL' | 'SCHEDULED';
  is_running: boolean;
  detected_directories: number;
  detected_files: number;
}

interface BatchStatus {
  status: 'idle' | 'running' | 'paused' | 'stopping' | 'completed' | 'error';
  progress_percent: number;
  processed_count: number;
  total_count: number;
  current_item: string;
  eta_seconds: number | null;
  logs: Array<{
    timestamp: string;
    level: string;
    message: string;
    item: string;
  }>;
  results: {
    matched: number;
    skipped: number;
    downloaded: number;
    failed: number;
    total_downloaded_bytes: number;
  };
  dry_run: boolean;
  quota: {
    current_usage_gb: number;
    quota_gb: number;
    remaining_gb: number;
    usage_percent: number;
  };
}

interface HistoryEntry {
  transaction_id: string;
  operation: string;
  details: string;
  timestamp: string;
}

interface TrashStatus {
  trash_items: number;
  trash_size_gb: number;
  max_size_gb: number;
}

type LibraryStatusType = 'unstarted' | 'in_progress' | 'finished' | 'dropped' | 'on_hold' | 'planned';

function App() {
  // Navigation state
  const [activeNav, setActiveNav] = useState<'library' | 'metadata' | 'history' | 'trash' | 'settings'>('library');

  // Library state
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedGame, setSelectedGame] = useState<Game | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [editingTitle, setEditingTitle] = useState(false);
  const [tempTitle, setTempTitle] = useState('');

  // Modal states
  const [showSettings, setShowSettings] = useState(false);

  // Scan status
  const [scanStatus, setScanStatus] = useState<ScanStatus | null>(null);

  // Fetch games from backend
  useEffect(() => {
    const fetchGames = async () => {
      try {
        setLoading(true);

        const response = await fetch('/api/library/files?limit=1000');
        if (!response.ok) {
          throw new Error('Failed to fetch library files');
        }

        const data = await response.json();

        // Filter only directories
        const gameDirs = data.files.filter((file: any) => file.is_dir);

        // Fetch metadata for each game
        const gamesWithMetadata: Game[] = await Promise.all(
          gameDirs.map(async (dir: any) => {
            try {
              const metadataResponse = await fetch(`/api/metadata/game/${encodeURIComponent(dir.path)}`);
              const metadataData = await metadataResponse.json();

              if (metadataData.success && metadataData.metadata) {
                const metadata = metadataData.metadata;

                const title = metadata.title?.value || dir.name;
                const titleJP = metadata.title_jp?.value;
                const developer = metadata.developer?.value;
                const description = metadata.description?.value;
                const releaseDate = metadata.release_date?.value;
                const genres = metadata.genres?.value;
                const backgroundUrl = metadata.background_url?.value;
                const libraryStatus = metadata.library_status?.value;

                return {
                  id: dir.path,
                  path: dir.path,
                  name: dir.name,
                  title,
                  titleJP,
                  developer,
                  description,
                  releaseDate,
                  genres,
                  cover: backgroundUrl,
                  status: libraryStatus || 'unstarted',
                  is_dir: dir.is_dir,
                  metadata
                };
              }

              return {
                id: dir.path,
                path: dir.path,
                name: dir.name,
                title: dir.name,
                status: 'unstarted',
                is_dir: dir.is_dir,
              };
            } catch (error) {
              console.error(`Error fetching metadata for ${dir.path}:`, error);
              return {
                id: dir.path,
                path: dir.path,
                name: dir.name,
                title: dir.name,
                status: 'unstarted',
                is_dir: dir.is_dir,
              };
            }
          })
        );

        setGames(gamesWithMetadata);
      } catch (error) {
        console.error('Error fetching games:', error);
      } finally {
        setLoading(false);
      }
    };

    fetchGames();
  }, []);

  // Fetch scan status
  useEffect(() => {
    const fetchScanStatus = async () => {
      try {
        const response = await fetch('/api/scan/status');
        if (response.ok) {
          const data = await response.json();
          setScanStatus(data);
        }
      } catch (error) {
        console.error('Failed to fetch scan status:', error);
      }
    };

    fetchScanStatus();
    const interval = setInterval(fetchScanStatus, 2000);
    return () => clearInterval(interval);
  }, []);

  // Filter games based on search
  const filteredGames = games.filter(game =>
    game.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (game.titleJP && game.titleJP.toLowerCase().includes(searchQuery.toLowerCase())) ||
    (game.developer && game.developer.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  // Open folder in system explorer
  const openFolder = async (path: string) => {
    try {
      const response = await fetch('/api/system/open_folder', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path })
      });

      if (!response.ok) {
        const error = await response.json();
        alert(`Failed to open folder: ${error.detail}`);
      }
    } catch (error) {
      console.error('Error opening folder:', error);
      alert('Failed to open folder');
    }
  };

  // Move to trash
  const moveToTrash = async (path: string) => {
    if (!confirm(`Move "${path}" to trash?`)) return;

    try {
      const response = await fetch('/api/trash/throw', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ paths: [path] })
      });

      if (response.ok) {
        window.location.reload();
      } else {
        const error = await response.json();
        alert(`Failed to move to trash: ${error.detail}`);
      }
    } catch (error) {
      console.error('Error moving to trash:', error);
      alert('Failed to move to trash');
    }
  };

  // Update library status
  const updateLibraryStatus = async (gamePath: string, status: LibraryStatusType) => {
    try {
      const response = await fetch('/api/metadata/play_status', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          game_path: gamePath,
          library_status: status
        })
      });

      if (response.ok) {
        // Refresh games
        window.location.reload();
      }
    } catch (error) {
      console.error('Error updating library status:', error);
    }
  };

  // Copy path to clipboard
  const copyPath = (path: string) => {
    navigator.clipboard.writeText(path);
    alert('Path copied to clipboard');
  };

  // Handle title edit
  const handleTitleEdit = async () => {
    if (!selectedGame || !tempTitle) return;

    try {
      toast.loading('Updating title...');

      // Call API to update title field
      await api.updateField(selectedGame.path, 'title', {
        original: tempTitle,
        en: tempTitle,
        ja: tempTitle,
        zh_hans: tempTitle,
        zh_hant: tempTitle,
      });

      toast.success('Title updated successfully');

      // Update local state
      setSelectedGame({
        ...selectedGame,
        title: tempTitle,
        name: tempTitle,
      });

      setEditingTitle(false);
    } catch (error: any) {
      console.error('Error updating title:', error);
      const errorMsg = error?.response?.data?.detail || 'Failed to update title';
      toast.error(errorMsg);
    }
  };

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 font-sans antialiased">
      {/* Sidebar */}
      <aside className="fixed left-0 top-0 bottom-0 w-64 bg-zinc-900 border-r border-zinc-800 z-50">
        <div className="flex flex-col h-full">
          {/* Logo */}
          <div className="p-6 border-b border-zinc-800">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg bg-zinc-800 flex items-center justify-center">
                <FolderOpen className="w-5 h-5 text-zinc-300" />
              </div>
              <div>
                <h1 className="font-bold text-lg">Library Manager</h1>
                <p className="text-xs text-zinc-500">Resource Manager</p>
              </div>
            </div>
          </div>

          {/* Navigation */}
          <nav className="flex-1 p-4 space-y-2">
            <NavButton icon={<Home />} label="Library" active={activeNav === 'library'} onClick={() => setActiveNav('library')} />
            <NavButton icon={<Database />} label="Metadata" active={activeNav === 'metadata'} onClick={() => setActiveNav('metadata')} />
            <NavButton icon={<Clock />} label="History" active={activeNav === 'history'} onClick={() => setActiveNav('history')} />
            <NavButton icon={<Trash2 />} label="Trash" active={activeNav === 'trash'} onClick={() => setActiveNav('trash')} />
            <div className="flex-1" />
            <NavButton icon={<SettingsIcon />} label="Settings" active={activeNav === 'settings'} onClick={() => setShowSettings(true)} />
          </nav>
        </div>
      </aside>

      {/* Main Content */}
      <main className="ml-64">
        {/* Header */}
        <header className="sticky top-0 z-40 bg-zinc-900/90 backdrop-blur-md border-b border-zinc-800">
          <div className="px-8 py-4">
            <div className="flex items-center gap-4">
              {/* Search */}
              <div className="flex-1 relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                <input
                  type="text"
                  placeholder="Search folders..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full pl-10 pr-4 py-2.5 bg-zinc-800 border border-zinc-700 rounded-md text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none focus:ring-2 focus:ring-zinc-600"
                />
              </div>

              {/* Refresh */}
              <button
                onClick={() => window.location.reload()}
                className="flex items-center gap-2 px-4 py-2.5 bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 rounded-md text-sm font-medium transition-all"
              >
                <RefreshCw className="w-4 h-4" />
                Refresh
              </button>

              {/* Scan Status Indicator */}
              {scanStatus && (
                <div className="flex items-center gap-2 px-4 py-2.5 bg-zinc-800 border border-zinc-700 rounded-md">
                  <div className={`w-2 h-2 rounded-full ${scanStatus.is_running ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-500'}`} />
                  <span className="text-sm text-zinc-300">{scanStatus.mode}</span>
                  <span className="text-xs text-zinc-500">{scanStatus.detected_directories} dirs</span>
                </div>
              )}
            </div>

            {/* Stats */}
            <div className="mt-4 flex items-center gap-6 text-sm text-zinc-400">
              <span>Total: {filteredGames.length} items</span>
              {loading && <span>Loading...</span>}
            </div>
          </div>
        </header>

        {/* Content Area */}
        <ErrorBoundary>
          {activeNav === 'metadata' ? (
            <div className="p-8">
              <h2 className="text-2xl font-bold mb-6">Metadata Scanner</h2>
              <ScraperConsole isOpen={true} onClose={() => setActiveNav('library')} />
            </div>
          ) : activeNav === 'history' ? (
            <div className="p-8">
              <TimeMachineLog />
            </div>
          ) : activeNav === 'trash' ? (
            <div className="p-8">
              <TrashView />
            </div>
          ) : activeNav === 'settings' ? (
            <div className="p-8">
              <SettingsView />
            </div>
          ) : (
            /* Library View */
            <div className="flex">
            {/* File List */}
            <div className="flex-1 p-8">
              {/* Loading State */}
              {loading && (
                <div className="flex items-center justify-center py-20">
                  <div className="text-center">
                    <div className="w-12 h-12 border-4 border-zinc-700 border-t-zinc-500 rounded-full animate-spin mx-auto mb-4"></div>
                    <p className="text-zinc-400">Loading library...</p>
                  </div>
                </div>
              )}

              {/* Empty State */}
              {!loading && filteredGames.length === 0 && (
                <div className="flex items-center justify-center py-20">
                  <div className="text-center">
                    <p className="text-zinc-400 text-lg">No items found</p>
                  </div>
                </div>
              )}

              {/* File Grid */}
              {!loading && filteredGames.length > 0 && (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                  {filteredGames.map((game) => (
                    <div
                      key={game.id}
                      onClick={() => setSelectedGame(game)}
                      className={`group cursor-pointer p-4 bg-zinc-900 border rounded-md transition-all ${
                        selectedGame?.id === game.id
                          ? 'border-zinc-500 bg-zinc-800'
                          : 'border-zinc-800 hover:border-zinc-700'
                      }`}
                    >
                      <div className="flex items-start gap-3">
                        <Folder className="w-8 h-8 text-zinc-500 flex-shrink-0 mt-1" />
                        <div className="flex-1 min-w-0">
                          <h3 className="font-medium text-sm text-zinc-100 truncate mb-1">
                            {game.title}
                          </h3>
                          {game.developer && (
                            <p className="text-xs text-zinc-500 truncate mb-2">{game.developer}</p>
                          )}
                          <p className="text-xs text-zinc-600 font-mono truncate">{game.path}</p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Details Panel */}
            {selectedGame && (
              <>
                {/* Backdrop */}
                <div
                  className="fixed inset-0 bg-zinc-950/80 backdrop-blur-sm z-40"
                  onClick={() => setSelectedGame(null)}
                />

                {/* Details Panel */}
                <div className="fixed right-0 top-0 bottom-0 w-full md:w-[500px] z-50">
                  <div className="h-full bg-zinc-900 border-l border-zinc-800 overflow-y-auto">
                    {/* Header */}
                    <div className="sticky top-0 z-10 flex justify-between items-center p-6 border-b border-zinc-800 bg-zinc-900">
                      <h3 className="text-lg font-semibold">Details</h3>
                      <button
                        onClick={() => setSelectedGame(null)}
                        className="p-2 hover:bg-zinc-800 rounded-md transition-colors"
                      >
                        <X className="w-5 h-5" />
                      </button>
                    </div>

                    {/* Content */}
                    <div className="p-6 space-y-6">
                      {/* Title (Editable) */}
                      <div className="space-y-2">
                        <label className="text-xs font-medium text-zinc-500 uppercase">Name</label>
                        {editingTitle ? (
                          <div className="flex gap-2">
                            <input
                              type="text"
                              value={tempTitle}
                              onChange={(e) => setTempTitle(e.target.value)}
                              className="flex-1 px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-md text-sm"
                              autoFocus
                            />
                            <button
                              onClick={handleTitleEdit}
                              className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 rounded-md text-sm"
                            >
                              Save
                            </button>
                          </div>
                        ) : (
                          <div
                            onClick={() => {
                              setTempTitle(selectedGame.title);
                              setEditingTitle(true);
                            }}
                            className="flex items-center justify-between p-3 bg-zinc-800 rounded-md cursor-pointer hover:bg-zinc-750"
                          >
                            <h2 className="text-xl font-semibold">{selectedGame.title}</h2>
                            <Edit3 className="w-4 h-4 text-zinc-500" />
                          </div>
                        )}
                      </div>

                      {/* Full Path */}
                      <div className="space-y-2">
                        <label className="text-xs font-medium text-zinc-500 uppercase">Path</label>
                        <div
                          onClick={() => copyPath(selectedGame.path)}
                          className="flex items-center justify-between p-3 bg-zinc-950 rounded-md cursor-pointer hover:bg-zinc-850 font-mono text-xs border border-zinc-800"
                        >
                          <span className="truncate flex-1">{selectedGame.path}</span>
                          <Copy className="w-4 h-4 text-zinc-500 ml-2 flex-shrink-0" />
                        </div>
                      </div>

                      {/* Library Status */}
                      <div className="space-y-2">
                        <label className="text-xs font-medium text-zinc-500 uppercase">Status</label>
                        <select
                          value={selectedGame.status}
                          onChange={(e) => updateLibraryStatus(selectedGame.path, e.target.value as LibraryStatusType)}
                          className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-md text-sm"
                        >
                          <option value="unstarted">📚 Unstarted</option>
                          <option value="in_progress">📖 In Progress</option>
                          <option value="finished">✅ Finished</option>
                          <option value="on_hold">⏸️ On Hold</option>
                          <option value="dropped">❌ Dropped</option>
                          <option value="planned">📝 Planned</option>
                        </select>
                      </div>

                      {/* Metadata */}
                      {selectedGame.developer && (
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-zinc-500 uppercase">Developer</label>
                          <p className="text-sm text-zinc-300">{selectedGame.developer}</p>
                        </div>
                      )}

                      {selectedGame.releaseDate && (
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-zinc-500 uppercase">Release Date</label>
                          <p className="text-sm text-zinc-300">{selectedGame.releaseDate}</p>
                        </div>
                      )}

                      {selectedGame.description && (
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-zinc-500 uppercase">Description</label>
                          <p className="text-sm text-zinc-300 leading-relaxed">{selectedGame.description}</p>
                        </div>
                      )}

                      {/* Actions */}
                      <div className="space-y-3 pt-4 border-t border-zinc-800">
                        <button
                          onClick={() => openFolder(selectedGame.path)}
                          className="w-full flex items-center justify-center gap-2 py-3 bg-zinc-700 hover:bg-zinc-600 rounded-md font-medium transition-all"
                        >
                          <FolderOpenIcon className="w-5 h-5" />
                          Open Folder
                        </button>

                        <button
                          onClick={() => moveToTrash(selectedGame.path)}
                          className="w-full flex items-center justify-center gap-2 py-3 bg-red-900/30 hover:bg-red-900/50 text-red-400 border border-red-900/50 rounded-md font-medium transition-all"
                        >
                          <Trash2 className="w-5 h-5" />
                          Move to Trash
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </>
            )}
          </div>
        )}
        </ErrorBoundary>
      </main>

      {/* Settings Panel */}
      {showSettings && <SettingsPanel onClose={() => setShowSettings(false)} />}
    </div>
  );
}

// Helper Components
function NavButton({ icon, label, active, onClick }: { icon: React.ReactNode; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-4 py-3 rounded-md text-sm font-medium transition-all ${
        active
          ? 'bg-zinc-800 text-white'
          : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'
      }`}
    >
      {icon}
      {label}
    </button>
  );
}

function TrashView() {
  const [status, setStatus] = useState<TrashStatus | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('/api/trash/status')
      .then(res => res.json())
      .then(data => {
        setStatus(data);
        setLoading(false);
      });
  }, []);

  if (loading) {
    return <div className="text-zinc-400">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Trash</h2>

      {status && (
        <div className="grid grid-cols-3 gap-4">
          <div className="bg-zinc-900 p-4 rounded-md border border-zinc-800">
            <p className="text-zinc-500 text-sm">Items</p>
            <p className="text-2xl font-bold text-zinc-100">{status.trash_items}</p>
          </div>
          <div className="bg-zinc-900 p-4 rounded-md border border-zinc-800">
            <p className="text-zinc-500 text-sm">Size</p>
            <p className="text-2xl font-bold text-zinc-100">{status.trash_size_gb.toFixed(2)} GB</p>
          </div>
          <div className="bg-zinc-900 p-4 rounded-md border border-zinc-800">
            <p className="text-zinc-500 text-sm">Max Size</p>
            <p className="text-2xl font-bold text-zinc-100">{status.max_size_gb} GB</p>
          </div>
        </div>
      )}

      <div className="bg-zinc-900 p-6 rounded-md border border-zinc-800">
        <h3 className="font-semibold mb-4">Trash is currently empty</h3>
        <p className="text-zinc-400 text-sm">Items moved to trash can be restored before emptying.</p>
      </div>
    </div>
  );
}

function SettingsView() {
  const [scanMode, setScanMode] = useState<string>('');

  useEffect(() => {
    fetch('/api/scan/status')
      .then(res => res.json())
      .then(data => {
        setScanMode(data.mode.toLowerCase());
      });
  }, []);

  const toggleScanMode = async () => {
    const newMode = scanMode === 'realtime' ? 'manual' : 'realtime';
    try {
      await fetch('/api/scan/mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode: newMode.toUpperCase() })
      });
      setScanMode(newMode);
    } catch (error) {
      console.error('Failed to toggle scan mode:', error);
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Settings</h2>

      <div className="bg-zinc-900 p-6 rounded-md border border-zinc-800 space-y-4">
        <h3 className="font-semibold">Scanner Mode</h3>
        <div className="flex items-center justify-between">
          <span className="text-zinc-400">Current: {scanMode?.toUpperCase()}</span>
          <button
            onClick={toggleScanMode}
            className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 rounded-md text-sm"
          >
            Toggle to {scanMode === 'realtime' ? 'Manual' : 'Realtime'}
          </button>
        </div>
      </div>

      <div className="bg-zinc-900 p-6 rounded-md border border-zinc-800">
        <h3 className="font-semibold mb-2">About</h3>
        <p className="text-zinc-400 text-sm">Galgame Library Manager v1.0.0</p>
        <p className="text-zinc-500 text-xs mt-1">Industrial-grade backend with transaction safety</p>
      </div>
    </div>
  );
}

export default App;
