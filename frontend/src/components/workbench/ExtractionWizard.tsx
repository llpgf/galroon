import React, { useState, useEffect, useRef } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { X, Check, AlertCircle, FolderOpen } from 'lucide-react';
import { UtilityAPI } from '../../api/utilityApi';
import { api } from '../../api/client';
import type { ApiError } from '../../api/client';
import { directoryInputProps } from '../../utils/domUtils';

/**
 * Extraction Wizard States
 */
type WizardState = 'form' | 'extracting' | 'completed' | 'error';

/**
 * Extraction Task Status (from backend)
 */
interface ExtractionTask {
  task_id: string;
  source_path: string;
  target_dir: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  current_file: string;
  total_files: number;
  error: string | null;
}

/**
 * ExtractionWizard Props
 */
interface ExtractionWizardProps {
  isOpen: boolean;
  onClose: () => void;
  archivePath: string;
  defaultTargetDir?: string;
}

/**
 * ExtractionWizard Modal Component
 *
 * 3 States:
 * 1. Form - Select destination and options
 * 2. Extracting - Show progress bar with polling
 * 3. Completed - Show success with "Open Folder" button
 * 4. Error - Show error message
 */
export const ExtractionWizard: React.FC<ExtractionWizardProps> = ({
  isOpen,
  onClose,
  archivePath,
  defaultTargetDir,
}) => {
  const [wizardState, setWizardState] = useState<WizardState>('form');
  const [targetDir, setTargetDir] = useState(defaultTargetDir || '');
  const [createSubfolder, setCreateSubfolder] = useState(true);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Phase 19.12: Hidden folder input for destination folder selection
  const folderInputRef = useRef<HTMLInputElement>(null);

  /**
   * Handle folder selection for destination
   * Phase 19.12: Extract folder path from selected files
   */
  const handleFolderSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files || files.length === 0) return;

    const firstFile = files[0];
    const folderPath = (firstFile as any).webkitRelativePath.split('/')[0];

    setTargetDir(folderPath);

    // Reset input
    if (folderInputRef.current) {
      folderInputRef.current.value = '';
    }
  };

  /**
   * Reset wizard state when opening
   */
  useEffect(() => {
    if (isOpen) {
      setWizardState('form');
      setTargetDir(defaultTargetDir || '');
      setCreateSubfolder(true);
      setTaskId(null);
      setProgress(0);
      setCurrentFile('');
      setError(null);
    }
  }, [isOpen, defaultTargetDir]);

  /**
   * Handle extract button click
   */
  const handleExtract = async () => {
    try {
      const result = await UtilityAPI.extractArchive(archivePath, targetDir);

      if (result) {
        setTaskId(result);
        setWizardState('extracting');
        // Start polling
      } else {
        setError('Failed to start extraction');
        setWizardState('error');
      }
    } catch (err) {
      console.error('Failed to extract:', err);
      setError(err instanceof Error ? err.message : 'Failed to start extraction');
      setWizardState('error');
    }
  };

  /**
   * Poll task status every 1 second
   *
   * Phase 18: Using centralized API client
   * Endpoint: GET /api/utils/tasks/{task_id}
   *
   * CRITICAL: Only poll when wizard is open AND state is 'extracting'
   * Stop polling on:
   * - Modal close (isOpen becomes false)
   * - Task completion (success/failure)
   */
  useEffect(() => {
    // CRITICAL: Only poll when wizard is open AND state is 'extracting'
    if (wizardState !== 'extracting' || !taskId || !isOpen) return;

    console.log(`[ExtractionWizard] 🔄 Starting polling for task: ${taskId}`);

    const pollInterval = setInterval(async () => {
      try {
        const response = await api.getTaskStatus(taskId);
        const task = response.data;

        // Update progress
        setProgress(task.progress);
        setCurrentFile(task.current_file || '');

        // Check if completed
        if (task.status === 'completed') {
          console.log(`[ExtractionWizard] ✅ Task completed: ${taskId}`);
          setProgress(100);
          setWizardState('completed');
          clearInterval(pollInterval);
        } else if (task.status === 'failed') {
          console.error(`[ExtractionWizard] ❌ Task failed: ${taskId} - ${task.error}`);
          setError(task.error || 'Extraction failed');
          setWizardState('error');
          clearInterval(pollInterval);
        } else if (task.status === 'cancelled') {
          console.log(`[ExtractionWizard] ⚠️ Task cancelled: ${taskId}`);
          setError('Extraction was cancelled');
          setWizardState('error');
          clearInterval(pollInterval);
        }
      } catch (err) {
        const enhancedError = err as { apiError?: ApiError };
        console.error('[ExtractionWizard] ❌ Failed to poll task status:', enhancedError.apiError?.message);
        setError('Failed to check extraction progress');
        setWizardState('error');
        clearInterval(pollInterval);
      }
    }, 1000); // Poll every 1 second

    return () => {
      clearInterval(pollInterval);
      console.log(`[ExtractionWizard] 🛑 Stopped polling for task: ${taskId}`);
    };
  }, [wizardState, taskId, isOpen]);

  /**
   * Handle "Open Target Folder" button click
   */
  const handleOpenTargetFolder = async () => {
    const success = await UtilityAPI.revealGameFolder(targetDir);
    if (success) {
      // Close wizard
      onClose();
    }
  };

  /**
   * Handle close button
   */
  const handleClose = () => {
    // Don't close if extracting
    if (wizardState === 'extracting') return;
    onClose();
  };

  /**
   * Get archive file name from path
   */
  const getArchiveFileName = () => {
    const parts = archivePath.split(/[/\\]/);
    return parts[parts.length - 1] || archivePath;
  };

  return (
    <>
      {/* Phase 19.12: Hidden folder input for destination selection */}
      <input
        ref={folderInputRef}
        type="file"
        {...directoryInputProps}
        style={{ display: 'none' }}
        onChange={handleFolderSelect}
      />

      <Dialog.Root open={isOpen} onOpenChange={handleClose}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <Dialog.Content className="fixed left-[50%] top-[50%] z-50 translate-x-[-50%] translate-y-[-50%] w-full max-w-md bg-zinc-800 border border-zinc-700 rounded-xl shadow-2xl data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] p-6">
          {/* Header */}
          <div className="flex items-center justify-between mb-6">
            <Dialog.Title className="text-xl font-semibold text-white">
              {wizardState === 'form' && 'Extract Archive'}
              {wizardState === 'extracting' && 'Extracting...'}
              {wizardState === 'completed' && 'Extraction Complete'}
              {wizardState === 'error' && 'Extraction Failed'}
            </Dialog.Title>
            <button
              onClick={handleClose}
              className="p-2 hover:bg-zinc-700 rounded-lg transition-colors"
              disabled={wizardState === 'extracting'}
            >
              <X size={20} className="text-zinc-400" />
            </button>
          </div>

          {/* Content */}
          <div className="min-h-[200px]">
            {/* Form State */}
            {wizardState === 'form' && (
              <div className="space-y-4">
                {/* Source (Read-only) */}
                <div>
                  <label className="block text-sm font-medium text-zinc-400 mb-2">
                    Source Archive
                  </label>
                  <div className="flex items-center gap-2 px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg">
                    <span className="text-2xl">📦</span>
                    <div className="flex-1 min-w-0">
                      <div className="text-white text-sm truncate">
                        {getArchiveFileName()}
                      </div>
                      <div className="text-zinc-500 text-xs truncate">
                        {archivePath}
                      </div>
                    </div>
                  </div>
                </div>

                {/* Destination */}
                <div>
                  <label className="block text-sm font-medium text-zinc-400 mb-2">
                    Destination Folder
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={targetDir}
                      onChange={(e) => setTargetDir(e.target.value)}
                      placeholder="Select extraction folder..."
                      className="flex-1 px-3 py-2 bg-zinc-900 border border-zinc-700 rounded-lg text-white text-sm focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                    />
                    <button
                      onClick={() => {
                        // Phase 19.12: Trigger folder browser
                        folderInputRef.current?.click();
                      }}
                      className="px-3 py-2 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors text-sm"
                      title="Select destination folder"
                    >
                      Browse...
                    </button>
                  </div>
                </div>

                {/* Options */}
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="create-subfolder"
                    checked={createSubfolder}
                    onChange={(e) => setCreateSubfolder(e.target.checked)}
                    className="w-4 h-4 rounded border-zinc-600 bg-zinc-900 text-blue-500 focus:ring-2 focus:ring-blue-500/50"
                  />
                  <label
                    htmlFor="create-subfolder"
                    className="text-sm text-zinc-300 cursor-pointer"
                  >
                    Create subfolder from archive name
                  </label>
                </div>
              </div>
            )}

            {/* Extracting State */}
            {wizardState === 'extracting' && (
              <div className="space-y-4">
                {/* Progress Bar */}
                <div>
                  <div className="flex justify-between text-sm mb-2">
                    <span className="text-zinc-400">Extracting...</span>
                    <span className="text-white font-medium">{progress}%</span>
                  </div>
                  <div className="w-full bg-zinc-900 rounded-full h-3 overflow-hidden border border-zinc-700">
                    <div
                      className="h-full bg-blue-500 transition-all duration-300 ease-out"
                      style={{ width: `${progress}%` }}
                    />
                  </div>
                </div>

                {/* Current File */}
                {currentFile && (
                  <div className="text-sm text-zinc-400">
                    <div className="truncate">Current: {currentFile}</div>
                  </div>
                )}

                {/* Info Message */}
                <div className="flex items-start gap-2 p-3 bg-zinc-900/50 border border-zinc-700 rounded-lg">
                  <span className="text-blue-400">ℹ️</span>
                  <p className="text-sm text-zinc-300">
                    You can close this window. Extraction will continue in the background.
                  </p>
                </div>
              </div>
            )}

            {/* Completed State */}
            {wizardState === 'completed' && (
              <div className="space-y-4 text-center">
                {/* Success Icon */}
                <div className="flex justify-center">
                  <div className="w-16 h-16 bg-green-500/20 rounded-full flex items-center justify-center">
                    <Check size={32} className="text-green-500" />
                  </div>
                </div>

                {/* Success Message */}
                <div>
                  <h3 className="text-lg font-semibold text-white mb-1">
                    Extraction Complete!
                  </h3>
                  <p className="text-sm text-zinc-400">
                    Archive successfully extracted to:
                  </p>
                  <p className="text-sm text-zinc-300 mt-1 break-all">
                    {targetDir}
                  </p>
                </div>

                {/* Action Buttons */}
                <div className="flex gap-3 justify-center">
                  <button
                    onClick={handleOpenTargetFolder}
                    className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors font-medium"
                  >
                    <FolderOpen size={18} />
                    Open Folder
                  </button>
                  <button
                    onClick={handleClose}
                    className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors"
                  >
                    Close
                  </button>
                </div>
              </div>
            )}

            {/* Error State */}
            {wizardState === 'error' && (
              <div className="space-y-4 text-center">
                {/* Error Icon */}
                <div className="flex justify-center">
                  <div className="w-16 h-16 bg-red-500/20 rounded-full flex items-center justify-center">
                    <AlertCircle size={32} className="text-red-500" />
                  </div>
                </div>

                {/* Error Message */}
                <div>
                  <h3 className="text-lg font-semibold text-white mb-1">
                    Extraction Failed
                  </h3>
                  <p className="text-sm text-red-400">
                    {error || 'An unknown error occurred'}
                  </p>
                </div>

                {/* Action Button */}
                <button
                  onClick={handleClose}
                  className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors"
                >
                  Close
                </button>
              </div>
            )}
          </div>

          {/* Footer (Form state only) */}
          {wizardState === 'form' && (
            <div className="flex justify-end gap-3 mt-6 pt-6 border-t border-zinc-700">
              <button
                onClick={handleClose}
                className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleExtract}
                disabled={!targetDir || wizardState === ('extracting' as WizardState)}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 disabled:cursor-not-allowed text-white rounded-lg transition-colors font-medium"
              >
                Extract
              </button>
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
    </>
  );
};

export default ExtractionWizard;
