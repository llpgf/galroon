import { useState, useEffect } from 'react';
import ScraperConsole from './ScraperConsole';
import MatchModal from './MatchModal';

interface LibraryFile {
  path: string;
  name: string;
  is_dir: boolean;
}

interface OrganizeResponse {
  success: boolean;
  message: string;
  organized_count?: number;
  created_directories?: number;
  moved_files?: number;
}

interface GameMetadata {
  vndb_id: string;
  title: {
    value: {
      ja: string;
      en: string;
      zh_hans: string;
      zh_hant: string;
      original: string;
    };
  };
  description: {
    value: string;
  };
  rating: {
    value: {
      score: number;
      count: number;
    };
  };
  background_url: {
    value: string;
  };
  cover_url: {
    value: string;
  };
  characters: {
    value: Array<{
      name: string;
      role: string;
      cv: string | null;
      image_url: string | null;
    }>;
  };
  staff: {
    value: Array<{
      name: string;
      role: string;
    }>;
  };
  library_status: {
    value: string;
  };
}

interface MatchCandidate {
  source: string;
  confidence: number;
  match_id: string;
  title: string;
  description: string;
  rating: number;
  metadata: any;
}

type LibraryStatusType = 'unstarted' | 'in_progress' | 'finished' | 'dropped' | 'on_hold' | 'planned';

export default function LibraryView() {
  const [files, setFiles] = useState<LibraryFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [organizing, setOrganizing] = useState(false);
  const [result, setResult] = useState<OrganizeResponse | null>(null);
  const [showScraper, setShowScraper] = useState(false);

  // New states for enhanced UI
  const [selectedGame, setSelectedGame] = useState<string | null>(null);
  const [metadata, setMetadata] = useState<GameMetadata | null>(null);
  const [showMatchModal, setShowMatchModal] = useState(false);
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);
  const [metadataLoading, setMetadataLoading] = useState(false);

  const fetchFiles = async () => {
    try {
      const response = await fetch('/api/library/files');
      if (response.ok) {
        const data = await response.json();
        setFiles(data.files || []);
      }
    } catch (error) {
      console.error('Failed to fetch library files:', error);
    } finally {
      setLoading(false);
    }
  };

  const fetchMetadata = async (gamePath: string) => {
    setMetadataLoading(true);
    try {
      const response = await fetch(`/api/metadata/game/${encodeURIComponent(gamePath)}`);
      if (response.ok) {
        const data = await response.json();
        setMetadata(data.metadata);
      }
    } catch (error) {
      console.error('Failed to fetch metadata:', error);
    } finally {
      setMetadataLoading(false);
    }
  };

  useEffect(() => {
    fetchFiles();
  }, []);

  const organizeLibrary = async () => {
    setOrganizing(true);
    setResult(null);
    try {
      const response = await fetch('/api/library/organize', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });

      if (response.ok) {
        const data = await response.json();
        setResult(data);
        await fetchFiles();
      } else {
        setResult({ success: false, message: 'Failed to organize library' });
      }
    } catch (error) {
      console.error('Failed to organize:', error);
      setResult({ success: false, message: 'Network error' });
    } finally {
      setOrganizing(false);
    }
  };

  const selectGame = (gamePath: string) => {
    if (selectedGame === gamePath) {
      setSelectedGame(null);
      setMetadata(null);
    } else {
      setSelectedGame(gamePath);
      fetchMetadata(gamePath);
    }
  };

  const cycleLibraryStatus = async () => {
    if (!metadata || !selectedGame) return;

    // Phase 19.6: Updated to new library status enum values
    const statusMap: Record<string, LibraryStatusType> = {
      'unstarted': 'in_progress',
      'in_progress': 'finished',
      'finished': 'dropped',
      'dropped': 'on_hold',
      'on_hold': 'planned',
      'planned': 'unstarted',
    };

    const newStatus = statusMap[metadata.library_status.value] || 'unstarted';

    try {
      const response = await fetch('/api/metadata/play_status', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          game_path: selectedGame,
          library_status: newStatus,
        }),
      });

      if (response.ok) {
        setMetadata((prev) => ({
          ...prev!,
          library_status: { ...prev!.library_status, value: newStatus },
        }));
      }
    } catch (error) {
      console.error('Failed to update library status:', error);
    }
  };

  const getLibraryStatusBadge = (status: string) => {
    // Phase 19.6: Updated to new library status enum values
    const statusConfig: Record<string, { color: string; emoji: string; label: string }> = {
      unstarted: { color: 'bg-gray-500', emoji: '📚', label: 'Unstarted' },
      in_progress: { color: 'bg-blue-500', emoji: '📖', label: 'In Progress' },
      finished: { color: 'bg-green-500', emoji: '✅', label: 'Finished' },
      dropped: { color: 'bg-red-500', emoji: '❌', label: 'Dropped' },
      on_hold: { color: 'bg-yellow-500', emoji: '⏸️', label: 'On Hold' },
      planned: { color: 'bg-purple-500', emoji: '📝', label: 'Planned' },
    };

    const config = statusConfig[status] || statusConfig.unstarted;
    return (
      <button
        onClick={cycleLibraryStatus}
        className={`${config.color} px-3 py-1 rounded-full text-xs font-semibold flex items-center gap-1 hover:opacity-80 transition-opacity`}
      >
        <span>{config.emoji}</span>
        <span>{config.label}</span>
      </button>
    );
  };

  if (loading) {
    return (
      <div className="bg-gray-800 rounded-lg p-6">
        <h2 className="text-white text-xl font-bold mb-4">Library Inbox</h2>
        <div className="space-y-2 animate-pulse">
          <div className="h-12 bg-gray-700 rounded"></div>
          <div className="h-12 bg-gray-700 rounded"></div>
          <div className="h-12 bg-gray-700 rounded"></div>
        </div>
      </div>
    );
  }

  return (
    <>
      <ScraperConsole isOpen={showScraper} onClose={() => setShowScraper(false)} />
      <MatchModal
        isOpen={showMatchModal}
        onClose={() => setShowMatchModal(false)}
        gameName={selectedGame || ''}
        candidates={candidates}
        onApply={async (candidate) => {
          try {
            const response = await fetch('/api/metadata/apply', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                game_path: selectedGame,
                match_id: candidate.match_id,
                source: candidate.source,
              }),
            });
            if (response.ok) {
              setShowMatchModal(false);
              if (selectedGame) fetchMetadata(selectedGame);
            }
          } catch (error) {
            console.error('Failed to apply metadata:', error);
          }
        }}
      />

      {/* Background Image for Glassmorphism */}
      {selectedGame && metadata?.background_url?.value && (
        <div
          className="fixed inset-0 -z-10"
          style={{
            backgroundImage: `url(${metadata.background_url.value})`,
            backgroundSize: 'cover',
            backgroundPosition: 'center',
          }}
        >
          <div className="absolute inset-0 bg-black/70 backdrop-blur-sm"></div>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left Panel: Game List */}
        <div className="lg:col-span-1 bg-gray-800 rounded-lg p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-white text-xl font-bold">Library</h2>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setShowScraper(true)}
                className="px-4 py-2 rounded-lg font-semibold flex items-center gap-2 bg-blue-600 hover:bg-blue-500 transition-colors text-white text-sm"
              >
                <span>📡</span>
                Scan
              </button>
              <button
                onClick={organizeLibrary}
                disabled={organizing}
                className={`px-4 py-2 rounded-lg font-semibold flex items-center gap-2 transition-colors text-white text-sm ${
                  organizing ? 'bg-gray-600' : 'bg-primary-600 hover:bg-primary-500'
                }`}
              >
                {organizing ? (
                  <>
                    <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
                  Org...
                  </>
                ) : (
                  <>
                    <span>🪄</span>
                    Magic
                  </>
                )}
              </button>
            </div>
          </div>

          {result && (
            <div className={`p-3 rounded-lg mb-4 text-sm ${
              result.success ? 'bg-green-900/50 border border-green-500' : 'bg-red-900/50 border border-red-500'
            }`}>
              <p className="text-white">{result.message}</p>
            </div>
          )}

          <div className="bg-gray-900 rounded-lg p-4 max-h-[600px] overflow-y-auto">
            {files.length === 0 ? (
              <div className="text-center py-8">
                <p className="text-gray-400">No files found</p>
              </div>
            ) : (
              <div className="space-y-1">
                {files.map((file, index) => (
                  <div
                    key={index}
                    onClick={() => file.is_dir && selectGame(file.path)}
                    className={`
                      p-3 rounded-lg cursor-pointer transition-all
                      ${selectedGame === file.path ? 'bg-primary-600' : 'bg-gray-800 hover:bg-gray-700'}
                      ${file.is_dir ? 'text-white' : 'text-gray-400'}
                    `}
                  >
                    <div className="flex items-center gap-2">
                      <span className="text-lg">{file.is_dir ? '🎮' : '📄'}</span>
                      <span className="text-sm font-mono truncate">{file.name}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right Panel: Game Details with Glassmorphism */}
        {selectedGame && (
          <div className="lg:col-span-2 space-y-6">
            {/* Header with Glassmorphism */}
            <div className="backdrop-blur-md bg-white/10 rounded-lg p-6 border border-white/20 shadow-2xl">
              {metadataLoading ? (
                <div className="text-center py-8">
                  <div className="w-12 h-12 border-4 border-white/30 border-t-white rounded-full animate-spin mx-auto"></div>
                  <p className="text-white mt-4">Loading metadata...</p>
                </div>
              ) : metadata ? (
                <>
                  <div className="flex items-start justify-between mb-4">
                    <div className="flex-1">
                      <h2 className="text-3xl font-bold text-white mb-2">
                        {metadata.title.value.zh_hant || metadata.title.value.en}
                      </h2>
                      <p className="text-gray-300 text-sm">{metadata.title.value.ja}</p>
                      {getLibraryStatusBadge(metadata.library_status.value)}
                    </div>
                    {metadata.rating?.value && (
                      <div className="text-right">
                        <div className="text-4xl font-bold text-yellow-400">
                          {metadata.rating.value.score.toFixed(1)}
                        </div>
                        <div className="text-gray-400 text-sm">
                          ⭐ {metadata.rating.value.count} votes
                        </div>
                      </div>
                    )}
                  </div>

                  <p className="text-gray-300 text-sm leading-relaxed line-clamp-4">
                    {metadata.description.value}
                  </p>
                </>
              ) : (
                <div className="text-center py-8">
                  <p className="text-gray-400">No metadata available</p>
                  <button
                    onClick={() => setShowScraper(true)}
                    className="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg"
                  >
                    Scan for Metadata
                  </button>
                </div>
              )}
            </div>

            {/* Cast List */}
            {metadata?.characters?.value && metadata.characters.value.length > 0 && (
              <div className="backdrop-blur-md bg-white/10 rounded-lg p-6 border border-white/20 shadow-2xl">
                <h3 className="text-xl font-bold text-white mb-4 flex items-center gap-2">
                  <span>👥</span>
                  Cast
                </h3>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
                  {metadata.characters.value.map((char, idx) => (
                    <div key={idx} className="flex items-center gap-3 bg-black/30 rounded-lg p-3">
                      {char.image_url && (
                        <img
                          src={char.image_url}
                          alt={char.name}
                          className="w-12 h-12 rounded-full object-cover border-2 border-white/30"
                        />
                      )}
                      <div className="flex-1 min-w-0">
                        <div className="text-white font-semibold text-sm truncate">{char.name}</div>
                        <div className="text-gray-400 text-xs">CV: {char.cv || 'Unknown'}</div>
                        <div className="text-gray-500 text-xs capitalize">{char.role}</div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Staff List */}
            {metadata?.staff?.value && metadata.staff.value.length > 0 && (
              <div className="backdrop-blur-md bg-white/10 rounded-lg p-6 border border-white/20 shadow-2xl">
                <h3 className="text-xl font-bold text-white mb-4 flex items-center gap-2">
                  <span>🎬</span>
                  Staff
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {metadata.staff.value.map((staff, idx) => (
                    <div key={idx} className="bg-black/30 rounded-lg p-3">
                      <div className="text-white font-semibold text-sm">{staff.name}</div>
                      <div className="text-gray-400 text-xs capitalize">{staff.role}</div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </>
  );
}
