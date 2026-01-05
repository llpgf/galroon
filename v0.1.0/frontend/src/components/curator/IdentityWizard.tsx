/**
 * IdentityWizard - Roon-style Game Identification
 *
 * Phase 24.0: The Curator
 *
 * Workflow:
 * 1. User clicks "Re-identify" button
 * 2. Search VNDB directly (proxy via backend)
 * 3. Show candidate list with cover art and release date
 * 4. Split view preview: Current vs New match data
 * 5. Apply match: Smart merge (overwrites unlocked, preserves locked)
 */

import React, { useState, useEffect } from 'react';
import { Search, Check, X, Image as ImageIcon } from 'lucide-react';
import { api } from '../../api/client';

interface VNDBCandidate {
  vndb_id: string;
  title: string;
  original_title?: string;
  developer?: string;
  release_date?: string;
  cover_image?: string;
  description?: string;
}

interface CurrentMetadata {
  title: string;
  developer: string;
  release_date?: string;
  cover_image?: string;
  locked_fields: string[];
}

interface IdentityWizardProps {
  gameId: string;
  currentMetadata: CurrentMetadata;
  onMatchApplied: (vndbId: string) => void;
  onCancel: () => void;
}

export const IdentityWizard: React.FC<IdentityWizardProps> = ({
  gameId,
  currentMetadata,
  onMatchApplied,
  onCancel,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [isSearching, setIsSearching] = useState(false);
  const [candidates, setCandidates] = useState<VNDBCandidate[]>([]);
  const [selectedCandidate, setSelectedCandidate] = useState<VNDBCandidate | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Search VNDB for matching games
   */
  const handleSearch = async () => {
    if (!searchQuery.trim()) return;

    setIsSearching(true);
    setError(null);

    try {
      // Proxy search through backend
      const response = await api.search({
        query: searchQuery,
        engine: 'vndb',
        limit: 10,
      });

      if (response.data && response.data.data) {
        const vndbResults = response.data.data.filter(
          (item: any) => item.source === 'vndb'
        );

        setCandidates(vndbResults);
      } else {
        setCandidates([]);
      }
    } catch (err: any) {
      setError(err.apiError?.message || 'Search failed');
      setCandidates([]);
    } finally {
      setIsSearching(false);
    }
  };

  /**
   * Handle Enter key in search box
   */
  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch();
    }
  };

  /**
   * Select a candidate for preview
   */
  const handleSelectCandidate = (candidate: VNDBCandidate) => {
    setSelectedCandidate(candidate);
  };

  /**
   * Apply the selected match (Smart Merge)
   */
  const handleApplyMatch = async () => {
    if (!selectedCandidate) return;

    setIsApplying(true);
    setError(null);

    try {
      // Call backend to perform smart merge
      await api.identifyGame(gameId, selectedCandidate.vndb_id);

      onMatchApplied(selectedCandidate.vndb_id);
    } catch (err: any) {
      setError(err.apiError?.message || 'Failed to apply match');
    } finally {
      setIsApplying(false);
    }
  };

  /**
   * Check if a field is locked
   */
  const isFieldLocked = (fieldName: string) => {
    return currentMetadata.locked_fields?.includes(fieldName);
  };

  /**
   * Render field comparison (Current vs New)
   */
  const renderFieldComparison = (
    label: string,
    fieldName: string,
    currentValue: string,
    newValue: string
  ) => {
    const locked = isFieldLocked(fieldName);
    const changed = currentValue !== newValue;

    return (
      <div className="mb-3">
        <div className="text-xs text-zinc-400 mb-1 flex items-center gap-2">
          {label}
          {locked && <span className="text-amber-400">🔒 Locked</span>}
          {changed && <span className="text-blue-400">Will Update</span>}
        </div>
        <div className="grid grid-cols-2 gap-2">
          <div className="bg-zinc-800 p-2 rounded">
            <div className="text-xs text-zinc-500 mb-1">Current</div>
            <div className="text-sm text-zinc-300 truncate">{currentValue || '-'}</div>
          </div>
          <div className="bg-zinc-800 p-2 rounded">
            <div className="text-xs text-zinc-500 mb-1">New Match</div>
            <div className="text-sm text-zinc-300 truncate">{newValue || '-'}</div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
      <div className="bg-zinc-900 rounded-lg max-w-6xl w-full max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-6 border-b border-zinc-800">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-2xl font-bold text-white">Re-identify Game</h2>
            <button
              onClick={onCancel}
              className="p-2 hover:bg-zinc-800 rounded-lg transition-colors"
            >
              <X size={20} className="text-zinc-400" />
            </button>
          </div>

          {/* Search Bar */}
          <div className="flex gap-2">
            <div className="flex-1 relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" size={20} />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyPress={handleKeyPress}
                placeholder="Search VNDB..."
                className="w-full pl-10 pr-4 py-3 bg-zinc-800 text-white rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <button
              onClick={handleSearch}
              disabled={isSearching || !searchQuery.trim()}
              className="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors"
            >
              {isSearching ? 'Searching...' : 'Search'}
            </button>
          </div>

          {error && (
            <div className="mt-3 text-red-400 text-sm">{error}</div>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {!selectedCandidate ? (
            // Candidate List View
            <div>
              <h3 className="text-lg font-semibold text-white mb-4">
                {candidates.length > 0
                  ? `Found ${candidates.length} matches`
                  : 'Search VNDB to find matches'}
              </h3>

              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {candidates.map((candidate) => (
                  <button
                    key={candidate.vndb_id}
                    onClick={() => handleSelectCandidate(candidate)}
                    className="bg-zinc-800 rounded-lg p-4 hover:bg-zinc-750 transition-colors text-left"
                  >
                    {/* Cover Image */}
                    <div className="aspect-[3/4] bg-zinc-900 rounded-lg mb-3 flex items-center justify-center overflow-hidden">
                      {candidate.cover_image ? (
                        <img
                          src={candidate.cover_image}
                          alt={candidate.title}
                          className="w-full h-full object-cover"
                        />
                      ) : (
                        <ImageIcon size={48} className="text-zinc-700" />
                      )}
                    </div>

                    {/* Title */}
                    <div className="font-semibold text-white mb-1 line-clamp-2">
                      {candidate.title}
                    </div>

                    {/* Metadata */}
                    <div className="text-sm text-zinc-400 space-y-1">
                      {candidate.original_title && (
                        <div className="line-clamp-1">{candidate.original_title}</div>
                      )}
                      {candidate.developer && (
                        <div>{candidate.developer}</div>
                      )}
                      {candidate.release_date && (
                        <div>{candidate.release_date}</div>
                      )}
                    </div>

                    {/* VNDB ID */}
                    <div className="text-xs text-zinc-500 mt-2">
                      {candidate.vndb_id}
                    </div>
                  </button>
                ))}
              </div>

              {candidates.length === 0 && searchQuery && !isSearching && (
                <div className="text-center py-12 text-zinc-500">
                  No matches found. Try a different search term.
                </div>
              )}
            </div>
          ) : (
            // Split View Preview
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {/* Current Data */}
              <div>
                <h3 className="text-lg font-semibold text-white mb-4">Current Data</h3>

                {/* Current Cover */}
                <div className="aspect-[3/4] bg-zinc-800 rounded-lg mb-4 flex items-center justify-center overflow-hidden">
                  {currentMetadata.cover_image ? (
                    <img
                      src={currentMetadata.cover_image}
                      alt={currentMetadata.title}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <ImageIcon size={64} className="text-zinc-700" />
                  )}
                </div>

                {/* Current Fields */}
                <div className="space-y-1">
                  <div className="text-sm text-zinc-400">Title</div>
                  <div className="text-white font-semibold">{currentMetadata.title}</div>
                </div>

                <div className="space-y-1 mt-3">
                  <div className="text-sm text-zinc-400">Developer</div>
                  <div className="text-white">{currentMetadata.developer}</div>
                </div>

                {currentMetadata.release_date && (
                  <div className="space-y-1 mt-3">
                    <div className="text-sm text-zinc-400">Release Date</div>
                    <div className="text-white">{currentMetadata.release_date}</div>
                  </div>
                )}
              </div>

              {/* New Match Data */}
              <div>
                <h3 className="text-lg font-semibold text-white mb-4">
                  New Match from VNDB
                </h3>

                {/* New Cover */}
                <div className="aspect-[3/4] bg-zinc-800 rounded-lg mb-4 flex items-center justify-center overflow-hidden">
                  {selectedCandidate.cover_image ? (
                    <img
                      src={selectedCandidate.cover_image}
                      alt={selectedCandidate.title}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <ImageIcon size={64} className="text-zinc-700" />
                  )}
                </div>

                {/* New Fields */}
                <div className="space-y-1">
                  <div className="text-sm text-zinc-400">Title</div>
                  <div className="text-white font-semibold">{selectedCandidate.title}</div>
                  {selectedCandidate.original_title && (
                    <div className="text-sm text-zinc-500">{selectedCandidate.original_title}</div>
                  )}
                </div>

                {selectedCandidate.developer && (
                  <div className="space-y-1 mt-3">
                    <div className="text-sm text-zinc-400">Developer</div>
                    <div className="text-white">{selectedCandidate.developer}</div>
                  </div>
                )}

                {selectedCandidate.release_date && (
                  <div className="space-y-1 mt-3">
                    <div className="text-sm text-zinc-400">Release Date</div>
                    <div className="text-white">{selectedCandidate.release_date}</div>
                  </div>
                )}

                {/* Description */}
                {selectedCandidate.description && (
                  <div className="mt-3">
                    <div className="text-sm text-zinc-400 mb-1">Description</div>
                    <div className="text-sm text-zinc-300 line-clamp-4">
                      {selectedCandidate.description}
                    </div>
                  </div>
                )}

                {/* VNDB Link */}
                <a
                  href={`https://vndb.org/${selectedCandidate.vndb_id}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-blue-400 text-sm hover:underline mt-3 inline-block"
                >
                  View on VNDB →
                </a>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-zinc-800 flex justify-between">
          <div>
            {selectedCandidate && (
              <button
                onClick={() => setSelectedCandidate(null)}
                className="px-4 py-2 text-zinc-400 hover:text-white transition-colors"
              >
                ← Back to Results
              </button>
            )}
          </div>

          <div className="flex gap-3">
            <button
              onClick={onCancel}
              className="px-6 py-2 bg-zinc-800 text-white rounded-lg hover:bg-zinc-700 transition-colors"
            >
              Cancel
            </button>

            {selectedCandidate && (
              <button
                onClick={handleApplyMatch}
                disabled={isApplying}
                className="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
              >
                {isApplying ? 'Applying...' : (
                  <>
                    <Check size={20} />
                    Apply Match
                  </>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default IdentityWizard;
