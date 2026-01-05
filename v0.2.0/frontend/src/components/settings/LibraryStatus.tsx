/**
 * LibraryStatus - Roon-style Visual Scanner Component
 *
 * Phase 24.5: System Governance
 *
 * Features:
 * - Real-time progress tracking
 * - Visual progress bar with stages
 * - Pause/Resume/Cancel controls
 * - Added/Modified/Removed counts
 * - Current file display
 */

import React, { useState, useEffect } from 'react';
import { Play, Pause, X, RefreshCw, CheckCircle, Clock, FolderOpen } from 'lucide-react';
import { api } from '../../api/client';

interface ScanProgress {
  stage: 'idle' | 'scanning' | 'diffing' | 'processing';
  current_file: string;
  processed_count: number;
  total_changes: number;
  is_paused: boolean;
  added_count: number;
  modified_count: number;
  removed_count: number;
}

interface LibraryStatusProps {
  onScanStart?: () => void;
  onScanComplete?: (result: any) => void;
}

export const LibraryStatus: React.FC<LibraryStatusProps> = ({
  onScanStart,
  onScanComplete,
}) => {
  const [progress, setProgress] = useState<ScanProgress>({
    stage: 'idle',
    current_file: '',
    processed_count: 0,
    total_changes: 0,
    is_paused: false,
    added_count: 0,
    modified_count: 0,
    removed_count: 0,
  });
  const [isScanning, setIsScanning] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Poll scan progress every 500ms
   */
  useEffect(() => {
    if (!isScanning) return;

    const interval = setInterval(async () => {
      try {
        const response = await api.getScanProgress();
        if (response.data) {
          setProgress(response.data);

          // Check if scan completed
          if (response.data.stage === 'idle' && response.data.processed_count === response.data.total_changes) {
            setIsScanning(false);
            onScanComplete?.(response.data);
          }
        }
      } catch (err) {
        console.error('Failed to fetch scan progress:', err);
      }
    }, 500);

    return () => clearInterval(interval);
  }, [isScanning, onScanComplete]);

  /**
   * Trigger a manual scan
   */
  const handleStartScan = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const response = await api.triggerScan();

      if (response.data) {
        setIsScanning(true);
        onScanStart?.();
      }
    } catch (err: any) {
      setError(err.response?.data?.detail || 'Failed to start scan');
      console.error('Failed to start scan:', err);
    } finally {
      setIsLoading(false);
    }
  };

  /**
   * Pause the scan
   */
  const handlePause = async () => {
    try {
      const response = await api.pauseScan();

      if (response.data) {
        setProgress((prev) => ({ ...prev, is_paused: true }));
      }
    } catch (err) {
      console.error('Failed to pause scan:', err);
    }
  };

  /**
   * Resume the scan
   */
  const handleResume = async () => {
    try {
      const response = await api.resumeScan();

      if (response.data) {
        setProgress((prev) => ({ ...prev, is_paused: false }));
      }
    } catch (err) {
      console.error('Failed to resume scan:', err);
    }
  };

  /**
   * Cancel the scan
   */
  const handleCancel = async () => {
    try {
      const response = await api.cancelScan();

      if (response.data) {
        setIsScanning(false);
        setProgress({
          stage: 'idle',
          current_file: '',
          processed_count: 0,
          total_changes: 0,
          is_paused: false,
          added_count: 0,
          modified_count: 0,
          removed_count: 0,
        });
      }
    } catch (err) {
      console.error('Failed to cancel scan:', err);
    }
  };

  /**
   * Calculate progress percentage
   */
  const getProgressPercentage = () => {
    if (progress.total_changes === 0) return 0;
    return Math.round((progress.processed_count / progress.total_changes) * 100);
  };

  /**
   * Get stage display text
   */
  const getStageText = () => {
    switch (progress.stage) {
      case 'idle':
        return 'Idle';
      case 'scanning':
        return 'Scanning filesystem...';
      case 'diffing':
        return 'Calculating changes...';
      case 'processing':
        return 'Processing changes...';
      default:
        return 'Unknown';
    }
  };

  /**
   * Get stage icon
   */
  const getStageIcon = () => {
    switch (progress.stage) {
      case 'idle':
        return <CheckCircle size={20} className="text-green-400" />;
      case 'scanning':
      case 'diffing':
      case 'processing':
        return <RefreshCw size={20} className="text-blue-400 animate-spin" />;
      default:
        return <Clock size={20} className="text-zinc-400" />;
    }
  };

  return (
    <div className="bg-zinc-900 rounded-lg p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <FolderOpen size={24} className="text-blue-400" />
          <div>
            <h2 className="text-xl font-bold text-white">Library Status</h2>
            <p className="text-sm text-zinc-400">Monitor and manage library scans</p>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex gap-2">
          {!isScanning ? (
            <button
              onClick={handleStartScan}
              disabled={isLoading}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
            >
              {isLoading ? (
                'Starting...'
              ) : (
                <>
                  <Play size={18} />
                  Scan Library
                </>
              )}
            </button>
          ) : (
            <>
              {progress.is_paused ? (
                <button
                  onClick={handleResume}
                  className="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors flex items-center gap-2"
                >
                  <Play size={18} />
                  Resume
                </button>
              ) : (
                <button
                  onClick={handlePause}
                  className="px-4 py-2 bg-amber-600 text-white rounded-lg hover:bg-amber-700 transition-colors flex items-center gap-2"
                >
                  <Pause size={18} />
                  Pause
                </button>
              )}

              <button
                onClick={handleCancel}
                className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors flex items-center gap-2"
              >
                <X size={18} />
                Cancel
              </button>
            </>
          )}
        </div>
      </div>

      {/* Error Message */}
      {error && (
        <div className="mb-4 p-3 bg-red-900/30 border border-red-700 rounded-lg">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      {/* Progress Section */}
      {isScanning && (
        <div className="space-y-4">
          {/* Stage Indicator */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {getStageIcon()}
              <span className="text-white font-medium">{getStageText()}</span>
            </div>

            {progress.is_paused && (
              <span className="px-3 py-1 bg-amber-600/20 text-amber-400 rounded-full text-sm font-medium">
                Paused
              </span>
            )}
          </div>

          {/* Progress Bar */}
          <div className="relative">
            <div className="w-full h-2 bg-zinc-800 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 transition-all duration-300"
                style={{ width: `${getProgressPercentage()}%` }}
              />
            </div>

            {/* Progress Text */}
            <div className="mt-2 flex justify-between text-sm text-zinc-400">
              <span>{progress.processed_count} / {progress.total_changes} changes</span>
              <span>{getProgressPercentage()}%</span>
            </div>
          </div>

          {/* Current File */}
          {progress.current_file && (
            <div className="p-3 bg-zinc-800 rounded-lg">
              <div className="text-xs text-zinc-400 mb-1">Current File</div>
              <div className="text-sm text-white truncate">{progress.current_file}</div>
            </div>
          )}

          {/* Change Counts */}
          <div className="grid grid-cols-3 gap-3">
            <div className="p-3 bg-zinc-800 rounded-lg">
              <div className="text-xs text-zinc-400 mb-1">Added</div>
              <div className="text-lg font-bold text-green-400">{progress.added_count}</div>
            </div>

            <div className="p-3 bg-zinc-800 rounded-lg">
              <div className="text-xs text-zinc-400 mb-1">Modified</div>
              <div className="text-lg font-bold text-amber-400">{progress.modified_count}</div>
            </div>

            <div className="p-3 bg-zinc-800 rounded-lg">
              <div className="text-xs text-zinc-400 mb-1">Removed</div>
              <div className="text-lg font-bold text-red-400">{progress.removed_count}</div>
            </div>
          </div>
        </div>
      )}

      {/* Idle State */}
      {!isScanning && !isLoading && (
        <div className="text-center py-8 bg-zinc-800 rounded-lg">
          <CheckCircle size={48} className="text-green-400 mx-auto mb-3" />
          <p className="text-zinc-300 font-medium mb-1">Library is up to date</p>
          <p className="text-sm text-zinc-500">
            Last scan: {new Date().toLocaleDateString()}
          </p>
        </div>
      )}
    </div>
  );
};

export default LibraryStatus;
