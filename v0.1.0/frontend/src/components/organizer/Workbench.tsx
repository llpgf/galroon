/**
 * Workbench UI for Galgame Library Manager
 *
 * PHASE 9.5: The Curator Workbench
 *
 * A dual-column diff view for organizing messy game folders into the Scene Standard.
 *
 * Features:
 * - Left column: Source (messy) folder tree
 * - Right column: Target (standard) structure preview
 * - Drag & drop for unresolved files
 * - Visual indicators for file status
 * - Analyze and Execute buttons
 */

import React, { useState, useCallback } from 'react';
import { FileText, Folder, AlertCircle, CheckCircle, ArrowRight } from 'lucide-react';

// Types
interface FileMove {
  source: string;
  target: string;
  status: 'safe' | 'unresolved' | 'skip' | 'warning';
  category: string;
  reason: string;
  size: number;
}

interface ProposalResponse {
  proposal_id: string;
  source_path: string;
  target_structure: {
    base: string;
    Game: string;
    Repository: string;
    Patch_Work: string;
    Extras: string;
    Metadata: string;
    [key: string]: string;
  };
  vndb_metadata: {
    developer: string;
    year: string;
    title: string;
    vndb_id: string;
  };
  moves: FileMove[];
  categorized_moves: Record<string, FileMove[]>;
  unresolved_files: FileMove[];
  summary: {
    file_count: number;
    total_size_mb: number;
    categorized_counts: Record<string, number>;
    unresolved_count: number;
  };
  created_at: string;
}

interface Props {
  sourcePath: string;
  targetRoot: string;
  vndbMetadata: {
    developer: string;
    year: string;
    title: string;
    vndb_id: string;
  };
  onExecute?: (proposalId: string) => void;
  onCancel?: () => void;
}

const Workbench: React.FC<Props> = ({
  sourcePath,
  targetRoot,
  vndbMetadata,
  onExecute,
  onCancel,
}) => {
  const [proposal, setProposal] = useState<ProposalResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [executing, setExecuting] = useState(false);

  // Analyze source directory
  const analyzeDirectory = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch('http://localhost:8000/api/organizer/generate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          source_path: sourcePath,
          target_root: targetRoot,
          vndb_metadata: vndbMetadata,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.detail || 'Failed to generate proposal');
      }

      const data: ProposalResponse = await response.json();
      setProposal(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, [sourcePath, targetRoot, vndbMetadata]);

  // Execute proposal
  const executeProposal = useCallback(async () => {
    if (!proposal) return;

    setExecuting(true);
    setError(null);

    try {
      const response = await fetch('http://localhost:8000/api/organizer/execute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          proposal: proposal,
          skip_unresolved: false, // Require all files to be resolved
          cleanup_empty_dirs: true,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.detail || 'Execution failed');
      }

      const result = await response.json();

      if (result.success) {
        onExecute?.(proposal.proposal_id);
      } else {
        throw new Error(result.errors?.join(', ') || 'Execution failed');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setExecuting(false);
    }
  }, [proposal, onExecute]);

  // Handle drag start for unresolved files
  const handleDragStart = useCallback((e: React.DragEvent, file: FileMove) => {
    e.dataTransfer.setData('application/json', JSON.stringify(file));
  }, []);

  // Handle drop on target directory
  const handleDrop = useCallback(async (
    e: React.DragEvent,
    targetCategory: string
  ) => {
    e.preventDefault();

    try {
      const fileData = JSON.parse(e.dataTransfer.getData('application/json'));

      // Update the file's target path and category
      const updatedProposal = { ...proposal };
      const fileMove = updatedProposal.moves?.find(m => m.source === fileData.source);

      if (fileMove) {
        // Update target path to new category
        const newTargetDir = updatedProposal.target_structure?.[targetCategory] || '';
        const fileName = fileData.source.split(/[\\/]/).pop() || '';
        fileMove.target = `${newTargetDir}/${fileName}`;
        fileMove.category = targetCategory;
        fileMove.status = 'safe';
        fileMove.reason = `User-assigned to ${targetCategory}`;

        // Update categorized moves
        if (!updatedProposal.categorized_moves?.[targetCategory]) {
          if (updatedProposal.categorized_moves) {
            updatedProposal.categorized_moves[targetCategory] = [];
          }
        }
        updatedProposal.categorized_moves?.[targetCategory]?.push(fileMove);

        // Remove from unresolved
        if (updatedProposal.unresolved_files) {
          updatedProposal.unresolved_files = updatedProposal.unresolved_files.filter(
            f => f.source !== fileData.source
          );
        }

        setProposal(updatedProposal as any);
      }
    } catch (err) {
      console.error('Drop error:', err);
    }
  }, [proposal]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
  }, []);

  // Render file tree item
  const renderFileItem = (file: FileMove, isUnresolved: boolean = false) => {
    const fileName = file.source.split(/[\\/]/).pop() || '';
    const fileSizeMB = (file.size / (1024 * 1024)).toFixed(2);

    return (
      <div
        key={file.source}
        draggable={isUnresolved}
        onDragStart={(e) => handleDragStart(e, file)}
        className={`flex items-center gap-2 p-2 rounded mb-1 text-sm ${
          isUnresolved
            ? 'bg-orange-50 border border-orange-300 cursor-move hover:bg-orange-100'
            : 'bg-gray-50 border border-gray-200'
        }`}
      >
        <FileText className="w-4 h-4 text-gray-500 flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="truncate font-medium">{fileName}</div>
          <div className="text-xs text-gray-500 truncate">
            {fileSizeMB} MB • {file.category}
          </div>
        </div>
        {file.status === 'unresolved' && (
          <AlertCircle className="w-4 h-4 text-orange-500 flex-shrink-0" />
        )}
        {file.status === 'safe' && (
          <CheckCircle className="w-4 h-4 text-green-500 flex-shrink-0" />
        )}
      </div>
    );
  };

  // Render target directory drop zone
  const renderTargetDirectory = (name: string, description: string) => {
    const files = proposal?.categorized_moves[name] || [];
    const hasUnresolved = (proposal?.unresolved_files?.length || 0) > 0;

    return (
      <div
        key={name}
        onDrop={(e) => handleDrop(e, name)}
        onDragOver={handleDragOver}
        className={`border-2 rounded-lg p-3 mb-2 transition-colors ${
          hasUnresolved
            ? 'border-dashed border-blue-300 bg-blue-50 cursor-pointer hover:bg-blue-100'
            : 'border-gray-300 bg-gray-50'
        }`}
      >
        <div className="flex items-center gap-2 mb-2">
          <Folder className="w-5 h-5 text-blue-600" />
          <div>
            <div className="font-semibold text-sm">{name}</div>
            <div className="text-xs text-gray-600">{description}</div>
          </div>
        </div>
        {files.length > 0 && (
          <div className="text-xs text-gray-600 mb-1">
            {files.length} file{files.length !== 1 ? 's' : ''}
          </div>
        )}
        {hasUnresolved && (
          <div className="text-xs text-blue-600 mt-1">
            Drop files here
          </div>
        )}
      </div>
    );
  };

  // Can execute only if no unresolved files
  const canExecute = proposal && proposal.unresolved_files.length === 0;

  return (
    <div className="flex flex-col h-full bg-white">
      {/* Header */}
      <div className="border-b border-gray-200 p-4 bg-gray-50">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-xl font-bold text-gray-800">Curator Workbench</h2>
          {onCancel && (
            <button
              onClick={onCancel}
              className="px-3 py-1 text-sm text-gray-600 hover:text-gray-800"
            >
              Close
            </button>
          )}
        </div>

        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <div className="font-medium text-gray-700">Source</div>
            <div className="text-gray-600 truncate">{sourcePath}</div>
          </div>
          <div>
            <div className="font-medium text-gray-700">Target</div>
            <div className="text-gray-600 truncate">
              {targetRoot}/{vndbMetadata.developer}/
              {vndbMetadata.year} {vndbMetadata.title} [{vndbMetadata.vndb_id}]
            </div>
          </div>
        </div>

        {/* Summary */}
        {proposal && (
          <div className="mt-3 p-2 bg-blue-50 rounded text-sm">
            <div className="flex items-center gap-4">
              <span><strong>{proposal.summary.file_count}</strong> files</span>
              <span><strong>{proposal.summary.total_size_mb}</strong> MB</span>
              <span><strong>{proposal.summary.unresolved_count}</strong> unresolved</span>
            </div>
          </div>
        )}
      </div>

      {/* Action Bar */}
      <div className="border-b border-gray-200 p-4 flex gap-3">
        {!proposal ? (
          <button
            onClick={analyzeDirectory}
            disabled={loading}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400 font-medium"
          >
            {loading ? 'Analyzing...' : 'Analyze'}
          </button>
        ) : (
          <>
            <button
              onClick={analyzeDirectory}
              disabled={loading}
              className="px-4 py-2 border border-gray-300 rounded hover:bg-gray-50 font-medium"
            >
              Re-Analyze
            </button>
            <button
              onClick={executeProposal}
              disabled={!canExecute || executing}
              className={`px-4 py-2 rounded font-medium ${
                canExecute
                  ? 'bg-green-600 text-white hover:bg-green-700'
                  : 'bg-gray-300 text-gray-500 cursor-not-allowed'
              }`}
            >
              {executing ? 'Executing...' : 'Execute'}
            </button>
            {!canExecute && (
              <span className="text-sm text-orange-600 flex items-center gap-1">
                <AlertCircle className="w-4 h-4" />
                Resolve all files before executing
              </span>
            )}
          </>
        )}
      </div>

      {/* Error Display */}
      {error && (
        <div className="mx-4 mt-3 p-3 bg-red-50 border border-red-200 rounded text-red-700 text-sm">
          {error}
        </div>
      )}

      {/* Dual-Column View */}
      {proposal && (
        <div className="flex-1 overflow-hidden">
          <div className="flex h-full">
            {/* Left: Source Tree (Unresolved Files) */}
            <div className="w-1/2 border-r border-gray-200 p-4 overflow-y-auto">
              <h3 className="font-bold text-gray-800 mb-3 flex items-center gap-2">
                <AlertCircle className="w-5 h-5 text-orange-500" />
                Unresolved Files ({proposal.unresolved_files.length})
              </h3>
              <p className="text-sm text-gray-600 mb-3">
                Drag these files to the appropriate folders on the right
              </p>
              {proposal.unresolved_files.length === 0 ? (
                <div className="text-sm text-green-600 flex items-center gap-2">
                  <CheckCircle className="w-4 h-4" />
                  All files resolved!
                </div>
              ) : (
                proposal.unresolved_files.map(file => renderFileItem(file, true))
              )}
            </div>

            {/* Right: Target Structure */}
            <div className="w-1/2 p-4 overflow-y-auto bg-gray-50">
              <h3 className="font-bold text-gray-800 mb-3 flex items-center gap-2">
                <Folder className="w-5 h-5 text-blue-600" />
                Target Structure
              </h3>
              <p className="text-sm text-gray-600 mb-3">
                Scene Standard folder layout
              </p>

              {renderTargetDirectory('Game', 'Extracted game files and executables')}
              {renderTargetDirectory('Repository', 'ISOs, installers, archives')}
              {renderTargetDirectory('Patch_Work', 'Patches, cracks, translations')}
              {renderTargetDirectory('Extras', 'OSTs, artbooks, manuals')}
              {renderTargetDirectory('Metadata', 'System metadata and images')}
            </div>
          </div>
        </div>
      )}

      {/* Footer */}
      {proposal && (
        <div className="border-t border-gray-200 p-3 bg-gray-50 text-xs text-gray-600">
          <div className="flex items-center justify-between">
            <span>Proposal ID: {proposal.proposal_id}</span>
            <span>Created: {new Date(proposal.created_at).toLocaleString()}</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default Workbench;
