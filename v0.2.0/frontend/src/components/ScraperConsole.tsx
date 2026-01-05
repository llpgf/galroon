import { useState, useEffect } from 'react';
import MatchModal from './MatchModal';

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

interface ReviewQueueItem {
  game_path: string;
  game_name: string;
  candidates: Array<{
    source: string;
    confidence: number;
    match_id: string;
    title: string;
    description: string;
    rating: number;
    metadata: any;
  }>;
  status: 'pending' | 'resolved' | 'skipped';
}

interface ScraperConsoleProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function ScraperConsole({ isOpen, onClose }: ScraperConsoleProps) {
  const [activeTab, setActiveTab] = useState<'scan' | 'review'>('scan');
  const [status, setStatus] = useState<BatchStatus | null>(null);
  const [dryRun, setDryRun] = useState(true);
  const [downloadScreenshots, setDownloadScreenshots] = useState(true);
  const [preferTraditional, setPreferTraditional] = useState(true);
  const [provider, setProvider] = useState<'vndb' | 'bangumi' | 'erogamescape' | 'steam'>('vndb');
  const [loading, setLoading] = useState(false);
  const [reviewQueue, setReviewQueue] = useState<ReviewQueueItem[]>([]);
  const [showMatchModal, setShowMatchModal] = useState(false);
  const [selectedGame, setSelectedGame] = useState<string | null>(null);
  const [selectedCandidates, setSelectedCandidates] = useState<any[]>([]);

  // Poll status every second when running/paused
  useEffect(() => {
    if (!isOpen) return;

    const fetchStatus = async () => {
      try {
        const response = await fetch('/api/metadata/batch/status');
        if (response.ok) {
          const data = await response.json();
          setStatus(data);
        }
      } catch (error) {
        console.error('Failed to fetch batch status:', error);
      }
    };

    fetchStatus();
    const interval = setInterval(fetchStatus, 1000);
    return () => clearInterval(interval);
  }, [isOpen]);

  const startScan = async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/metadata/batch/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          dry_run: dryRun,
          download_screenshots: downloadScreenshots,
          prefer_traditional: preferTraditional,
          provider: provider,
          targets: null, // Auto-discover all games
        }),
      });

      if (response.ok) {
        const data = await response.json();
        console.log('Scan started:', data);

        // Check for ambiguous matches and add to review queue
        // This is a placeholder - in real implementation, backend would return these
        if (data.ambiguous_matches && data.ambiguous_matches.length > 0) {
          const queueItems: ReviewQueueItem[] = data.ambiguous_matches.map((match: any) => ({
            game_path: match.game_path,
            game_name: match.game_name,
            candidates: match.candidates,
            status: 'pending',
          }));
          setReviewQueue(prev => [...prev, ...queueItems]);
          setActiveTab('review');
        }
      } else {
        console.error('Failed to start scan');
      }
    } catch (error) {
      console.error('Failed to start scan:', error);
    } finally {
      setLoading(false);
    }
  };

  const pauseScan = async () => {
    try {
      const response = await fetch('/api/metadata/batch/pause', { method: 'POST' });
      if (response.ok) {
        console.log('Scan paused');
      }
    } catch (error) {
      console.error('Failed to pause scan:', error);
    }
  };

  const resumeScan = async () => {
    try {
      const response = await fetch('/api/metadata/batch/resume', { method: 'POST' });
      if (response.ok) {
        console.log('Scan resumed');
      }
    } catch (error) {
      console.error('Failed to resume scan:', error);
    }
  };

  const stopScan = async () => {
    try {
      const response = await fetch('/api/metadata/batch/stop', { method: 'POST' });
      if (response.ok) {
        console.log('Scan stopped');
      }
    } catch (error) {
      console.error('Failed to stop scan:', error);
    }
  };

  const handleReviewGame = (game: ReviewQueueItem) => {
    setSelectedGame(game.game_path);
    setSelectedCandidates(game.candidates);
    setShowMatchModal(true);
  };

  const handleApplyCandidate = async (candidate: any) => {
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
        // Mark as resolved in queue
        setReviewQueue(prev =>
          prev.map(item =>
            item.game_path === selectedGame
              ? { ...item, status: 'resolved' }
              : item
          )
        );
        setShowMatchModal(false);
      }
    } catch (error) {
      console.error('Failed to apply metadata:', error);
    }
  };

  const handleSkipGame = (gamePath: string) => {
    setReviewQueue(prev =>
      prev.map(item =>
        item.game_path === gamePath
          ? { ...item, status: 'skipped' }
          : item
      )
    );
  };

  const formatETA = (seconds: number | null) => {
    if (!seconds) return '--';
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
  };

  const formatBytes = (bytes: number) => {
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  };

  const getLogColor = (level: string) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      case 'success': return 'text-green-400';
      case 'info': return 'text-blue-300';
      default: return 'text-gray-300';
    }
  };

  const getConfidenceBadge = (confidence: number) => {
    if (confidence >= 95) {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-green-500/80 text-white">
          {confidence.toFixed(0)}%
        </span>
      );
    } else if (confidence >= 80) {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-yellow-500/80 text-white">
          {confidence.toFixed(0)}%
        </span>
      );
    } else {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-red-500/80 text-white">
          {confidence.toFixed(0)}%
        </span>
      );
    }
  };

  if (!isOpen) return null;

  const isRunning = status?.status === 'running';
  const isPaused = status?.status === 'paused';
  const isIdle = status?.status === 'idle' || status?.status === 'completed' || status?.status === 'error';
  const canStart = isIdle && !loading;
  const canPause = isRunning;
  const canResume = isPaused;
  const pendingReviewCount = reviewQueue.filter(item => item.status === 'pending').length;

  return (
    <>
      <MatchModal
        isOpen={showMatchModal}
        onClose={() => setShowMatchModal(false)}
        gameName={selectedGame || ''}
        candidates={selectedCandidates}
        onApply={handleApplyCandidate}
      />

      <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" onClick={onClose}>
        <div className="bg-gray-900/95 backdrop-blur-xl rounded-2xl border border-white/20 shadow-2xl w-full max-w-6xl max-h-[90vh] overflow-hidden flex flex-col" onClick={(e) => e.stopPropagation()}>
          {/* Header with Tabs */}
          <div className="bg-gradient-to-r from-primary-600 to-primary-800 p-6">
            <div className="flex items-center justify-between mb-4">
              <div>
                <h2 className="text-2xl font-bold text-white flex items-center gap-2">
                  <span>📡</span>
                  Metadata Manager
                </h2>
                <p className="text-white/80 mt-1">
                  {activeTab === 'scan' && (status?.dry_run ? '🧪 DRY RUN MODE - Simulation' : '⚡ REAL EXECUTION - Downloads Enabled')}
                  {activeTab === 'review' && `🔍 Review Queue - ${pendingReviewCount} games pending review`}
                </p>
              </div>
              <button
                onClick={onClose}
                className="text-white/80 hover:text-white text-2xl font-bold px-2 transition-colors"
              >
                ✕
              </button>
            </div>

            {/* Tabs */}
            <div className="flex gap-2">
              <button
                onClick={() => setActiveTab('scan')}
                className={`px-4 py-2 rounded-lg font-semibold transition-all ${
                  activeTab === 'scan'
                    ? 'bg-white text-primary-700 shadow-lg'
                    : 'bg-white/20 text-white hover:bg-white/30'
                }`}
              >
                📊 Batch Scan
              </button>
              <button
                onClick={() => setActiveTab('review')}
                className={`px-4 py-2 rounded-lg font-semibold transition-all relative ${
                  activeTab === 'review'
                    ? 'bg-white text-primary-700 shadow-lg'
                    : 'bg-white/20 text-white hover:bg-white/30'
                }`}
              >
                🔍 Review Queue
                {pendingReviewCount > 0 && (
                  <span className="absolute -top-1 -right-1 bg-red-500 text-white text-xs rounded-full w-5 h-5 flex items-center justify-center">
                    {pendingReviewCount}
                  </span>
                )}
              </button>
            </div>
          </div>

          <div className="p-6 overflow-y-auto flex-1 space-y-6">
            {activeTab === 'scan' ? (
              <>
                {/* Configuration */}
                {isIdle && (
                  <div className="bg-gray-800/50 backdrop-blur-sm rounded-lg p-4 space-y-4 border border-white/10">
                    <h3 className="text-white font-semibold text-lg">Configuration</h3>

                    {/* Provider Selection */}
                    <div className="bg-gray-900/50 p-3 rounded-lg border border-white/5">
                      <label className="block text-white font-semibold mb-2">📚 Metadata Source</label>
                      <select
                        value={provider}
                        onChange={(e) => setProvider(e.target.value as 'vndb' | 'bangumi' | 'erogamescape' | 'steam')}
                        className="w-full bg-gray-800 text-white border border-white/20 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-primary-500"
                      >
                        <option value="vndb">🇯🇵 VNDB - 日本视觉小说数据库</option>
                        <option value="bangumi">🇨🇳 Bangumi - 番组计划（中文）</option>
                        <option value="erogamescape">⭐ ErogameScape - Erogame评分网站</option>
                        <option value="steam">🎮 Steam Store - Steam平台</option>
                      </select>
                      <div className="text-gray-400 text-sm mt-1">
                        {provider === 'vndb' && 'Best for Japanese visual novels with detailed staff information'}
                        {provider === 'bangumi' && 'Best for Chinese games with native Chinese descriptions'}
                        {provider === 'erogamescape' && 'Best for eroge ratings and user reviews'}
                        {provider === 'steam' && 'Best for Steam visual novels with rich media'}
                      </div>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <label className="flex items-center gap-3 cursor-pointer bg-gray-900/50 p-3 rounded-lg hover:bg-gray-800/50 transition-colors border border-white/5">
                        <input
                          type="checkbox"
                          checked={dryRun}
                          onChange={(e) => setDryRun(e.target.checked)}
                          className="w-5 h-5 rounded"
                        />
                        <div>
                          <div className="text-white font-semibold">🧪 Dry Run (Simulation)</div>
                          <div className="text-gray-400 text-sm">Test matches without downloading</div>
                        </div>
                      </label>

                      <label className="flex items-center gap-3 cursor-pointer bg-gray-900/50 p-3 rounded-lg hover:bg-gray-800/50 transition-colors border border-white/5">
                        <input
                          type="checkbox"
                          checked={preferTraditional}
                          onChange={(e) => setPreferTraditional(e.target.checked)}
                          className="w-5 h-5 rounded"
                        />
                        <div>
                          <div className="text-white font-semibold">🇹🇼 Prefer Traditional Chinese</div>
                          <div className="text-gray-400 text-sm">Use 繁體中文 instead of 简体中文</div>
                        </div>
                      </label>

                      <label className={`flex items-center gap-3 cursor-pointer p-3 rounded-lg transition-colors border border-white/5 ${dryRun ? 'bg-gray-800/30 opacity-50' : 'bg-gray-900/50 hover:bg-gray-800/50'}`}>
                        <input
                          type="checkbox"
                          checked={downloadScreenshots}
                          onChange={(e) => setDownloadScreenshots(e.target.checked)}
                          disabled={dryRun}
                          className="w-5 h-5 rounded disabled:opacity-50"
                        />
                        <div className={dryRun ? 'opacity-50' : ''}>
                          <div className="text-white font-semibold">🖼️ Download Screenshots</div>
                          <div className="text-gray-400 text-sm">Fetch game images (ignored in dry run)</div>
                        </div>
                      </label>
                    </div>
                  </div>
                )}

                {/* Progress Section */}
                {status && (isRunning || isPaused || status.status === 'completed' || status.status === 'stopping') && (
                  <div className="bg-gray-800/50 backdrop-blur-sm rounded-lg p-4 space-y-4 border border-white/10">
                    <div className="flex items-center justify-between">
                      <h3 className="text-white font-semibold text-lg">Progress</h3>
                      <div className="text-gray-400 text-sm">
                        {status.processed_count} / {status.total_count} items
                      </div>
                    </div>

                    {/* Progress Bar */}
                    <div className="w-full bg-gray-700 rounded-full h-6 overflow-hidden">
                      <div
                        className={`h-full transition-all duration-300 flex items-center justify-end pr-2 ${
                          status.status === 'completed' ? 'bg-green-500' :
                          status.status === 'paused' ? 'bg-yellow-500' :
                          status.status === 'stopping' ? 'bg-red-500' :
                          'bg-primary-500'
                        }`}
                        style={{ width: `${status.progress_percent}%` }}
                      >
                        <span className="text-white text-xs font-bold">{status.progress_percent.toFixed(1)}%</span>
                      </div>
                    </div>

                    {/* Stats Grid */}
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-center">
                      <div className="bg-gray-900/50 rounded p-3 border border-white/5">
                        <div className="text-green-400 text-2xl font-bold">{status.results.matched}</div>
                        <div className="text-gray-400 text-sm">Matched</div>
                      </div>
                      <div className="bg-gray-900/50 rounded p-3 border border-white/5">
                        <div className="text-blue-400 text-2xl font-bold">{status.results.downloaded}</div>
                        <div className="text-gray-400 text-sm">Downloaded</div>
                      </div>
                      <div className="bg-gray-900/50 rounded p-3 border border-white/5">
                        <div className="text-yellow-400 text-2xl font-bold">{status.results.skipped}</div>
                        <div className="text-gray-400 text-sm">Skipped</div>
                      </div>
                      <div className="bg-gray-900/50 rounded p-3 border border-white/5">
                        <div className="text-red-400 text-2xl font-bold">{status.results.failed}</div>
                        <div className="text-gray-400 text-sm">Failed</div>
                      </div>
                    </div>

                    {/* ETA and Current Item */}
                    <div className="bg-gray-900/50 rounded p-3 space-y-2 border border-white/5">
                      <div className="flex justify-between text-sm">
                        <span className="text-gray-400">ETA:</span>
                        <span className="text-white font-mono">{formatETA(status.eta_seconds)}</span>
                      </div>
                      <div className="text-sm">
                        <span className="text-gray-400">Current:</span>
                        <span className="text-white font-mono ml-2 break-all">
                          {status.current_item || 'Idle'}
                        </span>
                      </div>
                      {!status.dry_run && status.results.total_downloaded_bytes > 0 && (
                        <div className="flex justify-between text-sm">
                          <span className="text-gray-400">Downloaded:</span>
                          <span className="text-white font-mono">{formatBytes(status.results.total_downloaded_bytes)}</span>
                        </div>
                      )}
                      {status.quota && (
                        <>
                          <div className="flex justify-between text-sm">
                            <span className="text-gray-400">Quota:</span>
                            <span className="text-white font-mono">{status.quota.current_usage_gb.toFixed(2)}GB / {status.quota.quota_gb.toFixed(2)}GB</span>
                          </div>
                          <div className="flex justify-between text-sm">
                            <span className="text-gray-400">Remaining:</span>
                            <span className={status.quota.remaining_gb < 0.5 ? 'text-red-400' : 'text-white'} font-mono>
                              {status.quota.remaining_gb.toFixed(2)}GB
                            </span>
                          </div>
                          <div className="w-full bg-gray-700 rounded-full h-2 mt-1">
                            <div
                              className={`h-full rounded-full ${status.quota.usage_percent > 90 ? 'bg-red-500' : status.quota.usage_percent > 70 ? 'bg-yellow-500' : 'bg-green-500'}`}
                              style={{ width: `${Math.min(status.quota.usage_percent, 100)}%` }}
                            />
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                )}

                {/* Console Output */}
                <div className="bg-gray-950/80 backdrop-blur-sm rounded-lg p-4 border border-white/10">
                  <h3 className="text-white font-semibold text-lg mb-3">Console Output</h3>
                  <div className="bg-black rounded p-4 h-64 overflow-y-auto font-mono text-sm space-y-1">
                    {!status || status.logs.length === 0 ? (
                      <div className="text-gray-500 text-center py-8">
                        No logs yet. Start a scan to see output.
                      </div>
                    ) : (
                      status.logs.map((log, index) => (
                        <div key={index} className={`${getLogColor(log.level)} flex gap-2`}>
                          <span className="text-gray-600 flex-shrink-0">
                            [{new Date(log.timestamp).toLocaleTimeString()}]
                          </span>
                          <span className="flex-1 break-words">
                            {log.message}
                            {log.item && <span className="text-gray-500 ml-2">[{log.item}]</span>}
                          </span>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </>
            ) : (
              /* Review Queue Tab */
              <div className="space-y-4">
                {reviewQueue.length === 0 ? (
                  <div className="text-center py-16">
                    <div className="text-6xl mb-4">✅</div>
                    <h3 className="text-white text-xl font-semibold mb-2">Review Queue Empty</h3>
                    <p className="text-gray-400">
                      No games waiting for review. Run a batch scan to detect ambiguous matches.
                    </p>
                  </div>
                ) : (
                  <div className="space-y-3">
                    {reviewQueue.map((item) => (
                      <div
                        key={item.game_path}
                        className={`bg-gray-800/50 backdrop-blur-sm rounded-lg p-4 border ${
                          item.status === 'resolved'
                            ? 'border-green-500/30'
                            : item.status === 'skipped'
                            ? 'border-gray-500/30'
                            : 'border-yellow-500/30'
                        }`}
                      >
                        <div className="flex items-start justify-between gap-4">
                          <div className="flex-1">
                            <div className="flex items-center gap-2 mb-2">
                              <h4 className="text-white font-semibold">{item.game_name}</h4>
                              {item.status === 'resolved' && (
                                <span className="px-2 py-1 rounded-full text-xs font-semibold bg-green-500/80 text-white">
                                  ✓ Resolved
                                </span>
                              )}
                              {item.status === 'skipped' && (
                                <span className="px-2 py-1 rounded-full text-xs font-semibold bg-gray-500/80 text-white">
                                  Skipped
                                </span>
                              )}
                            </div>
                            <p className="text-gray-400 text-sm font-mono mb-3">{item.game_path}</p>

                            {/* Candidates Preview */}
                            <div className="flex items-center gap-2">
                              <span className="text-gray-500 text-sm">Best match:</span>
                              {item.candidates.length > 0 && (
                                <>
                                  <span className="text-white text-sm">{item.candidates[0].title}</span>
                                  {getConfidenceBadge(item.candidates[0].confidence)}
                                </>
                              )}
                              <span className="text-gray-500 text-sm">
                                ({item.candidates.length} candidates)
                              </span>
                            </div>
                          </div>

                          {item.status === 'pending' && (
                            <div className="flex items-center gap-2">
                              <button
                                onClick={() => handleReviewGame(item)}
                                className="px-4 py-2 bg-primary-600 hover:bg-primary-500 text-white rounded-lg font-semibold transition-colors"
                              >
                                🔍 Review
                              </button>
                              <button
                                onClick={() => handleSkipGame(item.game_path)}
                                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded-lg font-semibold transition-colors"
                              >
                                Skip
                              </button>
                            </div>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Footer - Control Buttons */}
          {activeTab === 'scan' && (
            <div className="bg-gray-900/50 backdrop-blur-sm p-4 border-t border-white/10 flex items-center justify-between gap-4">
              <div className="text-sm text-gray-400">
                {status && (
                  <span className={`font-semibold ${
                    status.status === 'running' ? 'text-green-400' :
                    status.status === 'paused' ? 'text-yellow-400' :
                    status.status === 'completed' ? 'text-blue-400' :
                    status.status === 'error' ? 'text-red-400' :
                    'text-gray-400'
                  }`}>
                    Status: {status.status.toUpperCase()}
                  </span>
                )}
              </div>

              <div className="flex items-center gap-3">
                {canStart && (
                  <button
                    onClick={startScan}
                    disabled={loading}
                    className="px-6 py-2 bg-green-600 hover:bg-green-500 disabled:bg-gray-600 disabled:cursor-not-allowed text-white rounded-lg font-semibold flex items-center gap-2 transition-colors"
                  >
                    {loading ? (
                      <>
                        <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
                        Starting...
                      </>
                    ) : (
                      <>
                        <span>▶️</span>
                        Start Scan
                      </>
                    )}
                  </button>
                )}

                {canPause && (
                  <button
                    onClick={pauseScan}
                    className="px-6 py-2 bg-yellow-600 hover:bg-yellow-500 text-white rounded-lg font-semibold flex items-center gap-2 transition-colors"
                  >
                    <span>⏸️</span>
                    Pause
                  </button>
                )}

                {canResume && (
                  <button
                    onClick={resumeScan}
                    className="px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-semibold flex items-center gap-2 transition-colors"
                  >
                    <span>▶️</span>
                    Resume
                  </button>
                )}

                {(isRunning || isPaused) && (
                  <button
                    onClick={stopScan}
                    className="px-6 py-2 bg-red-600 hover:bg-red-500 text-white rounded-lg font-semibold flex items-center gap-2 transition-colors"
                  >
                    <span>⏹️</span>
                    Stop
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
