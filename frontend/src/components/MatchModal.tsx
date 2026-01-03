import { useState } from 'react';

interface MatchCandidate {
  source: string;
  confidence: number;
  match_id: string;
  title: string;
  description: string;
  rating: number;
  metadata: any;
}

interface MatchModalProps {
  isOpen: boolean;
  onClose: () => void;
  gameName: string;
  candidates: MatchCandidate[];
  onApply: (candidate: MatchCandidate) => void;
}

export default function MatchModal({
  isOpen,
  onClose,
  gameName,
  candidates,
  onApply,
}: MatchModalProps) {
  const [selectedCandidate, setSelectedCandidate] = useState<MatchCandidate | null>(null);
  const [activeTab, setActiveTab] = useState<'auto' | 'manual'>('auto');
  const [manualId, setManualId] = useState('');

  if (!isOpen) return null;

  // Helper functions (defined AFTER early return to avoid scope issues)
  const getConfidenceBadge = (confidence: number) => {
    if (confidence >= 95) {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-green-500/80 text-white">
          {confidence.toFixed(0)}% Match
        </span>
      );
    } else if (confidence >= 80) {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-yellow-500/80 text-white">
          {confidence.toFixed(0)}% Match
        </span>
      );
    } else {
      return (
        <span className="px-2 py-1 rounded-full text-xs font-semibold bg-red-500/80 text-white">
          {confidence.toFixed(0)}% Match
        </span>
      );
    }
  };

  const getSourceIcon = (source: string) => {
    const iconMap: Record<string, string> = {
      vndb: '🗃️',
      local: '💾',
      manual: '✏️',
    };
    return iconMap[source] || '📋';
  };

  // Sort candidates by confidence
  const sortedCandidates = [...candidates].sort((a, b) => b.confidence - a.confidence);
  const bestMatch = sortedCandidates[0];
  const otherCandidates = sortedCandidates.slice(1);

  const handleApply = () => {
    if (activeTab === 'auto' && selectedCandidate) {
      onApply(selectedCandidate);
    } else if (activeTab === 'manual' && manualId.trim()) {
      const manualCandidate: MatchCandidate = {
        source: 'manual',
        confidence: 100,
        match_id: manualId.trim(),
        title: `Manual: ${manualId.trim()}`,
        description: 'Manually entered VNDB ID',
        rating: 0,
        metadata: {},
      };
      onApply(manualCandidate);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/80 backdrop-blur-sm"
        onClick={onClose}
      ></div>

      {/* Modal Content */}
      <div className="relative bg-gray-900/95 backdrop-blur-xl rounded-2xl border border-white/20 shadow-2xl max-w-4xl w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-6 border-b border-white/10">
          <h2 className="text-2xl font-bold text-white mb-2">Select Metadata Match</h2>
          <p className="text-gray-400">
            Game: <span className="text-white font-semibold">{gameName}</span>
          </p>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-white/10">
          <button
            onClick={() => setActiveTab('auto')}
            className={`flex-1 px-6 py-3 font-semibold transition-colors ${
              activeTab === 'auto'
                ? 'bg-primary-600 text-white'
                : 'text-gray-400 hover:text-white hover:bg-white/5'
            }`}
          >
            Auto Match ({candidates.length})
          </button>
          <button
            onClick={() => setActiveTab('manual')}
            className={`flex-1 px-6 py-3 font-semibold transition-colors ${
              activeTab === 'manual'
                ? 'bg-primary-600 text-white'
                : 'text-gray-400 hover:text-white hover:bg-white/5'
            }`}
          >
            Manual Entry
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {activeTab === 'auto' ? (
            <div className="space-y-4">
              {/* Best Match */}
              {bestMatch && (
                <div>
                  <h3 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
                    <span>🏆</span>
                    <span>Best Match</span>
                    {getConfidenceBadge(bestMatch.confidence)}
                  </h3>
                  <CandidateCard
                    candidate={bestMatch}
                    isBest
                    isSelected={selectedCandidate?.match_id === bestMatch.match_id}
                    onSelect={() => setSelectedCandidate(bestMatch)}
                    getSourceIcon={getSourceIcon}
                    getConfidenceBadge={getConfidenceBadge}
                  />
                </div>
              )}

              {/* Other Candidates */}
              {otherCandidates.length > 0 && (
                <div>
                  <h3 className="text-lg font-semibold text-white mb-3 mt-6">
                    Other Candidates ({otherCandidates.length})
                  </h3>
                  <div className="space-y-2">
                    {otherCandidates.map((candidate) => (
                      <CandidateCard
                        key={candidate.match_id}
                        candidate={candidate}
                        isBest={false}
                        isSelected={selectedCandidate?.match_id === candidate.match_id}
                        onSelect={() => setSelectedCandidate(candidate)}
                        getSourceIcon={getSourceIcon}
                        getConfidenceBadge={getConfidenceBadge}
                      />
                    ))}
                  </div>
                </div>
              )}

              {candidates.length === 0 && (
                <div className="text-center py-8">
                  <p className="text-gray-400">No matches found</p>
                  <p className="text-gray-500 text-sm mt-2">Try manual entry</p>
                </div>
              )}
            </div>
          ) : (
            /* Manual Entry */
            <div className="space-y-4">
              <div>
                <label className="block text-white font-semibold mb-2">
                  VNDB ID
                </label>
                <input
                  type="text"
                  value={manualId}
                  onChange={(e) => setManualId(e.target.value)}
                  placeholder="e.g., v12345 or 12345"
                  className="w-full px-4 py-3 bg-gray-800 border border-white/20 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/50"
                />
                <p className="text-gray-400 text-sm mt-2">
                  Enter the VNDB ID (with or without 'v' prefix). Find IDs at{' '}
                  <a
                    href="https://vndb.org"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-400 hover:text-primary-300 underline"
                  >
                    vndb.org
                  </a>
                </p>
              </div>

              <div className="bg-blue-900/20 border border-blue-500/30 rounded-lg p-4">
                <p className="text-blue-300 text-sm">
                  <strong>Tip:</strong> Manual entry will fetch all metadata from VNDB including
                  description, characters, staff, and screenshots.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-white/10 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-6 py-2 rounded-lg font-semibold bg-gray-700 hover:bg-gray-600 text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleApply}
            disabled={
              activeTab === 'auto'
                ? !selectedCandidate
                : !manualId.trim()
            }
            className={`px-6 py-2 rounded-lg font-semibold transition-colors ${
              (activeTab === 'auto' && selectedCandidate) ||
              (activeTab === 'manual' && manualId.trim())
                ? 'bg-primary-600 hover:bg-primary-500 text-white'
                : 'bg-gray-700 text-gray-500 cursor-not-allowed'
            }`}
          >
            Apply Metadata
          </button>
        </div>
      </div>
    </div>
  );
}

interface CandidateCardProps {
  candidate: MatchCandidate;
  isBest: boolean;
  isSelected: boolean;
  onSelect: () => void;
  getSourceIcon: (source: string) => string;
  getConfidenceBadge: (confidence: number) => React.ReactNode;
}

function CandidateCard({
  candidate,
  isBest,
  isSelected,
  onSelect,
  getSourceIcon,
  getConfidenceBadge,
}: CandidateCardProps) {
  return (
    <div
      onClick={onSelect}
      className={`p-4 rounded-lg border-2 cursor-pointer transition-all ${
        isSelected
          ? 'border-primary-500 bg-primary-600/20'
          : isBest
          ? 'border-yellow-500/50 bg-yellow-500/10 hover:bg-yellow-500/20'
          : 'border-white/10 bg-gray-800/50 hover:bg-gray-800'
      }`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-2">
            <span className="text-xl">{getSourceIcon(candidate.source)}</span>
            <h4 className="text-white font-semibold truncate">{candidate.title}</h4>
            {getConfidenceBadge(candidate.confidence)}
          </div>

          {candidate.description && (
            <p className="text-gray-400 text-sm line-clamp-2 mb-2">
              {candidate.description}
            </p>
          )}

          <div className="flex items-center gap-4 text-sm">
            {candidate.rating > 0 && (
              <div className="flex items-center gap-1">
                <span>⭐</span>
                <span className="text-yellow-400 font-semibold">
                  {candidate.rating.toFixed(1)}
                </span>
              </div>
            )}
            <div className="text-gray-500">
              ID: <span className="text-gray-400 font-mono">{candidate.match_id}</span>
            </div>
          </div>
        </div>

        {isSelected && (
          <div className="text-primary-400 text-2xl">✓</div>
        )}
      </div>
    </div>
  );
}
