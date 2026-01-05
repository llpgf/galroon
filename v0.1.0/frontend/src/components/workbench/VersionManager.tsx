import React, { useState, useEffect, useRef } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { X, Star, Trash, FolderPlus, Check } from 'lucide-react';
import { api } from '../../api/client';
import apiClient from '../../api/client';
import toast from 'react-hot-toast';
import { directoryInputProps } from '../../utils/domUtils';

/**
 * Game Version Data Structure
 */
interface GameVersion {
  id: string;
  path: string;
  label: string;
  is_primary: boolean;
  added_at: string;
}

/**
 * VersionManager Props
 */
interface VersionManagerProps {
  isOpen: boolean;
  onClose: () => void;
  gameId: string;
  gameTitle: string;
}

/**
 * VersionManager Modal Component
 *
 * Allows users to:
 * - View all versions of a game
 * - Set primary version
 * - Unlink/remove versions
 * - Add new versions (manual identify)
 */
export const VersionManager: React.FC<VersionManagerProps> = ({
  isOpen,
  onClose,
  gameId,
  gameTitle,
}) => {
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  // Phase 19.12: Hidden folder input for adding versions
  const folderInputRef = useRef<HTMLInputElement>(null);

  /**
   * Fetch versions when modal opens
   */
  useEffect(() => {
    if (isOpen) {
      fetchVersions();
    }
  }, [isOpen, gameId]);

  /**
   * Fetch game versions from backend
   */
  const fetchVersions = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const response = await apiClient.get(`http://localhost:8000/api/games/${gameId}/versions`);
      if (response.data.success) {
        setVersions(response.data.versions || []);
      } else {
        setError('Failed to load versions');
      }
    } catch (err) {
      console.error('Failed to fetch versions:', err);
      setError('Failed to load versions');
    } finally {
      setIsLoading(false);
    }
  };

  /**
   * Handle "Set Primary" action
   */
  const handleSetPrimary = async (versionId: string) => {
    try {
      const response = await apiClient.patch(
        `http://localhost:8000/api/games/${gameId}/versions/${versionId}/set_primary`
      );

      if (response.data.success) {
        showActionMessage('✓ Primary version updated');
        // Refresh versions list
        await fetchVersions();
      } else {
        showActionMessage('✗ Failed to set primary');
      }
    } catch (err) {
      console.error('Failed to set primary:', err);
      showActionMessage('✗ Failed to set primary');
    }
  };

  /**
   * Handle "Unlink" action
   */
  const handleUnlink = async (versionId: string) => {
    if (!confirm('Are you sure you want to unlink this version?')) {
      return;
    }

    try {
      const response = await apiClient.delete(
        `http://localhost:8000/api/games/${gameId}/versions/${versionId}`
      );

      if (response.data.success) {
        showActionMessage('✓ Version unlinked');
        // Refresh versions list
        await fetchVersions();
      } else {
        showActionMessage('✗ Failed to unlink version');
      }
    } catch (err) {
      console.error('Failed to unlink:', err);
      showActionMessage('✗ Failed to unlink version');
    }
  };

  /**
   * Handle "Add Version" action
   * Phase 19.12: Trigger folder browser and add version
   */
  const handleAddVersion = () => {
    // Trigger hidden file input
    folderInputRef.current?.click();
  };

  /**
   * Handle folder selection for adding version
   * Phase 19.12: Add selected folder as a new version
   */
  const handleFolderSelect = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files || files.length === 0) return;

    // Get the first folder path
    const firstFile = files[0];
    const folderPath = (firstFile as any).webkitRelativePath.split('/')[0];

    try {
      toast.loading('Adding version...');

      // Call API to add version
      const response = await apiClient.post(`/api/games/${gameId}/versions`, {
        path: folderPath,
        label: folderPath.split(/[/\\]/).pop() || folderPath,  // Use folder name as label
      });

      toast.success('Version added successfully');
      showActionMessage('✓ Version added');
      await fetchVersions();  // Refresh versions list
    } catch (error: any) {
      console.error('Failed to add version:', error);
      const errorMsg = error?.response?.data?.detail || 'Failed to add version';
      toast.error(errorMsg);
      showActionMessage('✗ Failed to add version');
    } finally {
      // Reset input
      if (folderInputRef.current) {
        folderInputRef.current.value = '';
      }
    }
  };

  /**
   * Show action feedback
   */
  const showActionMessage = (message: string) => {
    setActionMessage(message);
    setTimeout(() => setActionMessage(null), 3000);
  };

  return (
    <>
      {/* Phase 19.12: Hidden folder input for adding versions */}
      <input
        ref={folderInputRef}
        type="file"
        {...directoryInputProps}
        style={{ display: 'none' }}
        onChange={handleFolderSelect}
      />

      <Dialog.Root open={isOpen} onOpenChange={onClose}>
        <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <Dialog.Content className="fixed left-[50%] top-[50%] z-50 translate-x-[-50%] translate-y-[-50%] w-full max-w-2xl bg-zinc-800 border border-zinc-700 rounded-xl shadow-2xl data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] p-6">
          {/* Header */}
          <div className="flex items-center justify-between mb-6">
            <Dialog.Title className="text-xl font-semibold text-white">
              Manage Versions
            </Dialog.Title>
            <button
              onClick={onClose}
              className="p-2 hover:bg-zinc-700 rounded-lg transition-colors"
            >
              <X size={20} className="text-zinc-400" />
            </button>
          </div>

          {/* Game Title */}
          <div className="mb-4">
            <p className="text-sm text-zinc-400">Game</p>
            <p className="text-lg font-medium text-white">{gameTitle}</p>
          </div>

          {/* Loading State */}
          {isLoading && (
            <div className="flex items-center justify-center py-12">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
              <p className="ml-3 text-zinc-400">Loading versions...</p>
            </div>
          )}

          {/* Error State */}
          {error && !isLoading && (
            <div className="flex items-center justify-center py-12">
              <div className="text-center">
                <div className="text-4xl mb-2">⚠️</div>
                <p className="text-red-400">{error}</p>
              </div>
            </div>
          )}

          {/* Versions List */}
          {!isLoading && !error && versions.length === 0 && (
            <div className="flex items-center justify-center py-12">
              <div className="text-center">
                <div className="text-4xl mb-2">📦</div>
                <p className="text-zinc-400">No versions found</p>
                <p className="text-sm text-zinc-500 mt-1">
                  Add a folder to create a new version
                </p>
              </div>
            </div>
          )}

          {!isLoading && !error && versions.length > 0 && (
            <div className="space-y-3 max-h-[400px] overflow-y-auto">
              {versions.map((version) => (
                <div
                  key={version.id}
                  className="flex items-center justify-between p-4 bg-zinc-900 border border-zinc-700 rounded-lg hover:border-zinc-600 transition-colors"
                >
                  {/* Version Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      {version.is_primary && (
                        <span className="flex items-center gap-1 px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded-full border border-blue-500/50">
                          <Star size={12} fill="currentColor" />
                          Primary
                        </span>
                      )}
                      <h3 className="text-white font-medium truncate">
                        {version.label}
                      </h3>
                    </div>
                    <p className="text-sm text-zinc-400 truncate">{version.path}</p>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 ml-4">
                    {!version.is_primary && (
                      <button
                        onClick={() => handleSetPrimary(version.id)}
                        className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded-lg transition-colors"
                        title="Set as primary version"
                      >
                        <Star size={14} fill="currentColor" />
                        Set Primary
                      </button>
                    )}
                    <button
                      onClick={() => handleUnlink(version.id)}
                      className="p-2 hover:bg-red-500/20 text-red-400 rounded-lg transition-colors"
                      title="Unlink this version"
                    >
                      <Trash size={16} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Footer Actions */}
          <div className="flex justify-end gap-3 mt-6 pt-6 border-t border-zinc-700">
            <button
              onClick={handleAddVersion}
              className="flex items-center gap-2 px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors"
            >
              <FolderPlus size={18} />
              Add Folder...
            </button>
            <button
              onClick={onClose}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
            >
              Done
            </button>
          </div>

          {/* Action Feedback Toast */}
          {actionMessage && (
            <div className="absolute bottom-4 right-4 px-4 py-2 bg-zinc-900 text-white rounded-lg shadow-lg border border-zinc-700 animate-fade-in">
              {actionMessage}
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
    </>
  );
};

export default VersionManager;
