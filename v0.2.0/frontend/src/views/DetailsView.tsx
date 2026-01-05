import React, { useState } from 'react';
import { useGameDetails } from '../hooks/useGameDetails';
import type { GameDetails } from '../hooks/useGameDetails';
import { UtilityAPI } from '../api/utilityApi';
import { api } from '../api/client';
import { ContextMenu, useContextMenu } from '../components/context/ContextMenu';
import { ExtractionWizard } from '../components/workbench/ExtractionWizard';
import { VersionManager } from '../components/workbench/VersionManager';
import { Lock, Unlock, FolderOpen, Copy, ChevronRight, X, Plus } from 'lucide-react';
import toast from 'react-hot-toast';

/**
 * DetailsView - Game Details Command Center
 *
 * Layout:
 * - Top 40%: Blurred hero image background
 * - Left column: High-res cover art
 * - Right column: Metadata + Action Bar
 */

interface DetailsViewProps {
  gameId: string;
  onBack?: () => void;
}

export const DetailsView: React.FC<DetailsViewProps> = ({ gameId, onBack }) => {
  const { details, isLoading, error, toggleLock } = useGameDetails(gameId);

  // Context menu
  const { contextMenu, showContextMenu, hideContextMenu } = useContextMenu();

  // Extraction wizard
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [selectedArchive, setSelectedArchive] = useState<string | null>(null);

  // Version manager
  const [isVersionManagerOpen, setIsVersionManagerOpen] = useState(false);

  /**
   * Handle action feedback
   * Phase 18 Hotfix: Use toast.success for action messages
   */
  const showActionMessage = (msg: string) => toast.success(msg);

  /**
   * Handle "Open Folder" action
   */
  const handleOpenFolder = async () => {
    if (!details?.folder_path) return;

    const success = await UtilityAPI.revealGameFolder(details.folder_path);
    if (success) {
      showActionMessage('✓ Folder opened in Explorer');
    } else {
      showActionMessage('✗ Failed to open folder');
    }
  };

  /**
   * Handle "Copy Path" action
   */
  const handleCopyPath = async () => {
    if (!details?.folder_path) return;

    const success = await UtilityAPI.copyGamePath(details.folder_path);
    if (success) {
      showActionMessage('✓ Path copied to clipboard');
    } else {
      showActionMessage('✗ Failed to copy path');
    }
  };

  /**
   * Handle field lock toggle
   */
  const handleToggleLock = async (field: string) => {
    const success = await toggleLock(field);
    if (success) {
      showActionMessage(`✓ ${field} lock toggled`);
    } else {
      showActionMessage('✗ Failed to toggle lock');
    }
  };

  /**
   * Context menu handlers
   */
  const handleContextReveal = async () => {
    if (!details?.folder_path) return;
    await handleOpenFolder();
  };

  const handleContextCopy = async () => {
    if (!details?.folder_path) return;
    await handleCopyPath();
  };

  const handleContextExtract = () => {
    // Open extraction wizard
    // Use the first detected archive or default to folder path
    const archivePath = details?.assets_detected?.find(a =>
      a.toLowerCase().includes('rar') ||
      a.toLowerCase().includes('zip') ||
      a.toLowerCase().includes('7z')
    );

    if (archivePath && details?.folder_path) {
      const fullPath = `${details.folder_path}/${archivePath}`;
      setSelectedArchive(fullPath);
      setIsWizardOpen(true);
    } else {
      showActionMessage('No archives found to extract');
    }
  };

  const handleContextManage = () => {
    setIsVersionManagerOpen(true);
  };

  /**
   * Handle asset chip click
   * Phase 19.11: Implemented manual and ISO handling
   */
  const handleAssetClick = async (asset: string) => {
    const lower = asset.toLowerCase();

    if (lower.includes('manual')) {
      console.log('Open manual:', asset);
      // Phase 19.11: Open manual file in default viewer
      const fullPath = details?.folder_path
        ? `${details.folder_path}/${asset}`
        : asset;

      try {
        await api.revealFolder(fullPath);
        showActionMessage('Opening manual...');
      } catch (error) {
        console.error('Error opening manual:', error);
        showActionMessage('Failed to open manual');
      }
    } else if (lower.includes('iso')) {
      console.log('Mount ISO:', asset);
      // Phase 19.11: Reveal ISO file (user can double-click to mount on Windows)
      const fullPath = details?.folder_path
        ? `${details.folder_path}/${asset}`
        : asset;

      try {
        await api.revealFolder(fullPath);
        showActionMessage('Opening ISO location...');
      } catch (error) {
        console.error('Error opening ISO location:', error);
        showActionMessage('Failed to open ISO location');
      }
    } else if (lower.includes('rar') || lower.includes('zip') || lower.includes('7z')) {
      // Open extraction wizard
      // Construct full archive path from folder_path
      const archivePath = details?.folder_path
        ? `${details.folder_path}/${asset}`
        : asset;

      setSelectedArchive(archivePath);
      setIsWizardOpen(true);
      showActionMessage('Opening extraction wizard...');
    } else {
      console.log('Asset clicked:', asset);
    }
  };

  if (isLoading || !details) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-zinc-400 mb-4"></div>
          <p className="text-zinc-400">Loading game details...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="text-6xl mb-4">⚠️</div>
          <h2 className="text-xl font-semibold text-white mb-2">Failed to Load Details</h2>
          <p className="text-zinc-400 mb-4">{error}</p>
          <button
            onClick={onBack}
            className="px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-white rounded-lg transition-colors"
          >
            Go Back
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="relative h-full overflow-y-auto">
      {/* Level 7: Atmospheric Immersion - Ambient Hero Backdrop */}
      {/* This creates "breathing" atmosphere using the game's cover image */}
      {/* Sits behind everything (z-0) to provide emotional tone */}
      {details?.cover_image && (
        <div className="fixed inset-0 z-0 overflow-hidden pointer-events-none">
          {/* The Atmospheric Mesh - Super-scaled and heavily blurred */}
          <div
            className="absolute inset-0 bg-cover bg-center transition-all duration-1000 ease-in-out
                       transform scale-150 blur-[100px] opacity-30"
            style={{
              backgroundImage: `url("${details.cover_image}")`,
              // Hardware acceleration for performance
              willChange: 'transform',
              transform: 'translate3d(0, 0, 0) scale(1.5)'
            }}
          />
          {/* The Vignette/Gradient Overlay - Ensures text readability */}
          <div className="absolute inset-0 bg-gradient-to-b from-zinc-900/40 via-zinc-900/80 to-zinc-900" />
        </div>
      )}

      {/* Hero Background (Top 40%) */}
      <div
        className="sticky top-0 h-[40vh] w-full relative z-10"
        onContextMenu={showContextMenu}
      >
        {details.hero_image ? (
          <>
            <div
              className="absolute inset-0 bg-cover bg-center blur-2xl scale-110"
              style={{ backgroundImage: `url(${details.hero_image})` }}
            />
            <div className="absolute inset-0 bg-gradient-to-b from-transparent via-zinc-900/50 to-zinc-900" />
          </>
        ) : (
          <div className="absolute inset-0 bg-gradient-to-br from-zinc-800 to-zinc-900" />
        )}
      </div>

      {/* Content Container */}
      <div className="relative -mt-[40vh] px-8 pb-8 z-10">
        <div className="max-w-7xl mx-auto">
          <div className="flex gap-8">
            {/* Left Column: Cover Art */}
            <div className="flex-shrink-0">
              <div
                className="w-[300px] aspect-[3/4] rounded-xl overflow-hidden shadow-2xl border border-zinc-700/50 bg-zinc-800 cursor-pointer"
                onContextMenu={showContextMenu}
                title="Right-click for options"
              >
                {details.cover_image ? (
                  <img
                    src={details.cover_image}
                    alt={details.title}
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center">
                    <span className="text-6xl text-zinc-600">🎮</span>
                  </div>
                )}
              </div>

              {/* Action Bar (Below Cover) */}
              <div className="mt-6 space-y-3">
                <button
                  onClick={handleOpenFolder}
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors font-medium"
                >
                  <FolderOpen size={20} />
                  Open Folder
                </button>

                <button
                  onClick={handleCopyPath}
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-zinc-700 hover:bg-zinc-600 text-white rounded-lg transition-colors font-medium"
                >
                  <Copy size={20} />
                  Copy Path
                </button>
              </div>
            </div>

            {/* Right Column: Metadata */}
            <div className="flex-1 min-w-0">
              {/* Title Section */}
              <div className="mb-6">
                <h1 className="text-4xl font-bold text-white mb-2">
                  {details.title}
                </h1>
                {details.title_original && details.title_original !== details.title && (
                  <p className="text-xl text-zinc-400 mb-4">
                    {details.title_original}
                  </p>
                )}
                <div className="flex items-center gap-4 text-zinc-400">
                  {details.developer && (
                    <span className="flex items-center gap-2">
                      <span>by</span>
                      <span className="text-white font-medium">{details.developer}</span>
                    </span>
                  )}
                  {details.release_date && (
                    <span>• {details.release_date}</span>
                  )}
                  {details.vndb_id && (
                    <a
                      href={`https://vndb.org/${details.vndb_id}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-400 hover:text-blue-300"
                    >
                      VNDB: {details.vndb_id}
                    </a>
                  )}
                </div>
              </div>

              {/* Phase 19.6: Library Status Selector */}
              <LibraryStatusSection
                gameId={gameId}
                currentStatus={details.library_status || 'unstarted'}
                folderPath={details.folder_path}
                onStatusUpdated={(newStatus) => {
                  if (details) {
                    details.library_status = newStatus;
                  }
                }}
              />

              {/* Description with Lock */}
              {details.description && (
                <div className="mb-6">
                  <div className="flex items-center justify-between mb-2">
                    <h2 className="text-lg font-semibold text-white">Description</h2>
                    <button
                      onClick={() => handleToggleLock('description')}
                      className="p-2 hover:bg-zinc-800 rounded-lg transition-colors"
                      title="Toggle description lock"
                    >
                      {details.metadata?.description?.locked ? (
                        <Lock size={18} className="text-blue-400" />
                      ) : (
                        <Unlock size={18} className="text-zinc-500" />
                      )}
                    </button>
                  </div>
                  <p className="text-zinc-300 leading-relaxed">
                    {details.description}
                  </p>
                </div>
              )}

              {/* Phase 18.5: Tags Section (Roon-style) */}
              {((details.tags && details.tags.length > 0) || (details.user_tags && details.user_tags.length > 0)) && (
                <div className="mb-6">
                  <h2 className="text-lg font-semibold text-white mb-3">Tags</h2>
                  <TagSection
                    providerTags={details.tags || []}
                    userTags={details.user_tags || []}
                    folderPath={details.folder_path}
                    onTagsUpdated={(newTags) => {
                      // Update local state
                      if (details) {
                        details.user_tags = newTags;
                      }
                    }}
                  />
                </div>
              )}

              {/* Asset Matrix */}
              {details.assets_detected && details.assets_detected.length > 0 && (
                <div className="mb-6">
                  <h2 className="text-lg font-semibold text-white mb-3">Detected Assets</h2>
                  <AssetMatrix assets={details.assets_detected} onAssetClick={handleAssetClick} />
                </div>
              )}

              {/* External IDs */}
              {details.external_ids && (
                <div className="mb-6">
                  <h2 className="text-lg font-semibold text-white mb-3">External Links</h2>
                  <div className="flex flex-wrap gap-2">
                    {details.external_ids.steam && (
                      <a
                        href={`https://store.steampowered.com/app/${details.external_ids.steam}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="px-3 py-1.5 bg-zinc-800 hover:bg-zinc-700 text-blue-400 rounded-lg transition-colors text-sm flex items-center gap-2"
                      >
                        Steam
                        <ChevronRight size={16} />
                      </a>
                    )}
                    {details.external_ids.bangumi && (
                      <a
                        href={`https://bgm.tv/subject/${details.external_ids.bangumi}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="px-3 py-1.5 bg-zinc-800 hover:bg-zinc-700 text-blue-400 rounded-lg transition-colors text-sm flex items-center gap-2"
                      >
                        Bangumi
                        <ChevronRight size={16} />
                      </a>
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Context Menu */}
      <ContextMenu
        visible={contextMenu.visible}
        x={contextMenu.x}
        y={contextMenu.y}
        onClose={hideContextMenu}
        onReveal={handleContextReveal}
        onCopyPath={handleContextCopy}
        onExtract={handleContextExtract}
        onManageVersions={handleContextManage}
      />

      {/* Extraction Wizard */}
      <ExtractionWizard
        isOpen={isWizardOpen}
        onClose={() => setIsWizardOpen(false)}
        archivePath={selectedArchive || ''}
        defaultTargetDir={details?.folder_path}
      />

      {/* Version Manager */}
      <VersionManager
        isOpen={isVersionManagerOpen}
        onClose={() => setIsVersionManagerOpen(false)}
        gameId={gameId}
        gameTitle={details?.title || ''}
      />
    </div>
  );
};

/**
 * Asset Matrix Component
 * Display detected assets as interactive chips
 */
interface AssetMatrixProps {
  assets: string[];
  onAssetClick: (asset: string) => void;
}

const AssetMatrix: React.FC<AssetMatrixProps> = ({ assets, onAssetClick }) => {
  /**
   * Get asset chip style
   */
  const getAssetStyle = (asset: string): string => {
    const lower = asset.toLowerCase();

    if (lower.includes('iso')) {
      return 'bg-badge-iso-bg/20 text-badge-iso-text border-badge-iso-bg/50';
    }
    if (lower.includes('dlc')) {
      return 'bg-badge-dlc-bg/20 text-badge-dlc-text border-badge-dlc-bg/50';
    }
    if (lower.includes('patch') || lower.includes('update')) {
      return 'bg-badge-patch-bg/20 text-badge-patch-text border-badge-patch-bg/50';
    }
    if (lower.includes('manual')) {
      return 'bg-blue-500/20 text-blue-400 border-blue-500/50';
    }
    if (lower.includes('ost') || lower.includes('soundtrack')) {
      return 'bg-purple-500/20 text-purple-400 border-purple-500/50';
    }

    return 'bg-zinc-700/50 text-zinc-300 border-zinc-600/50';
  };

  return (
    <div className="flex flex-wrap gap-2">
      {assets.map((asset, index) => (
        <button
          key={index}
          onClick={() => onAssetClick(asset)}
          className={`px-3 py-1.5 rounded-lg border transition-colors cursor-pointer hover:opacity-80 ${getAssetStyle(asset)}`}
        >
          {asset}
        </button>
      ))}
    </div>
  );
};

/**
 * Library Status Section Component - Phase 19.6: Semantic Sanitization
 *
 * Allows users to track their library engagement status.
 *
 * Phase 19.6: Renamed from "Play Status" to "Library Status"
 * - Removed "Game Launcher" language
 * - Updated to asset/library management terminology
 * - Optimistic updates + API persistence
 */
interface LibraryStatusSectionProps {
  gameId: string;
  currentStatus: string;
  folderPath?: string;
  onStatusUpdated: (newStatus: string) => void;
}

const LibraryStatusSection: React.FC<LibraryStatusSectionProps> = ({
  gameId,
  currentStatus,
  folderPath,
  onStatusUpdated,
}) => {
  const [isSaving, setIsSaving] = useState(false);

  const statusOptions = [
    { value: 'unstarted', label: '未开始', icon: '📚', color: 'bg-zinc-700/50 text-zinc-300 border-zinc-600/50' },
    { value: 'in_progress', label: '进行中', icon: '📖', color: 'bg-blue-500/20 text-blue-400 border-blue-500/50' },
    { value: 'finished', label: '已完成', icon: '✅', color: 'bg-green-500/20 text-green-400 border-green-500/50' },
    { value: 'on_hold', label: '搁置', icon: '⏸️', color: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50' },
    { value: 'dropped', label: '弃坑', icon: '❌', color: 'bg-red-500/20 text-red-400 border-red-500/50' },
    { value: 'planned', label: '计划中', icon: '📝', color: 'bg-purple-500/20 text-purple-400 border-purple-500/50' },
  ];

  const handleStatusChange = async (newStatus: string) => {
    if (newStatus === currentStatus) return;

    // Optimistic update: update UI immediately
    const previousStatus = currentStatus;
    onStatusUpdated(newStatus);
    toast.success(`状态更新: ${statusOptions.find(s => s.value === newStatus)?.label}`);

    try {
      if (folderPath) {
        setIsSaving(true);
        // Phase 19.6: Call API to persist library status
        await api.updateLibraryStatus(gameId, newStatus);
        console.log(`[LibraryStatus] ✅ Persisted ${gameId} as ${newStatus}`);
      }
    } catch (error: any) {
      console.error('[LibraryStatus] ❌ Failed to persist status:', error);
      // Rollback on failure
      onStatusUpdated(previousStatus);
      const errorMsg = error?.response?.data?.detail || 'Failed to save status';
      toast.error(`${errorMsg} - Rolled back`);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="mb-6">
      <h2 className="text-lg font-semibold text-white mb-3">状态</h2>
      <div className="flex flex-wrap gap-2">
        {statusOptions.map((option) => {
          const isActive = currentStatus === option.value;
          return (
            <button
              key={option.value}
              onClick={() => !isSaving && handleStatusChange(option.value)}
              disabled={isSaving}
              className={`px-4 py-2 rounded-lg border transition-all text-sm flex items-center gap-2 ${
                isActive
                  ? option.color + ' ring-2 ring-white/20'
                  : 'bg-zinc-800/50 text-zinc-400 border-zinc-700/50 hover:bg-zinc-800 hover:text-zinc-300'
              } disabled:opacity-50 disabled:cursor-not-allowed`}
              title={option.label}
            >
              <span>{option.icon}</span>
              <span>{option.label}</span>
              {isActive && <span className="ml-1">✓</span>}
            </button>
          );
        })}
      </div>
    </div>
  );
};

/**
 * Tag Section Component - Phase 18.5: Roon-style Custom Tags
 *
 * Displays two types of tags:
 * 1. Provider Tags (Gray) - Read-only tags from VNDB, Bangumi, etc.
 * 2. User Tags (Purple) - Editable custom tags for personal organization
 *
 * Features:
 * - Remove user tags by clicking X
 * - Add new user tags via "+" button
 * - Inline tag input field
 * - Saves to backend via API
 */
interface TagSectionProps {
  providerTags: string[];
  userTags: string[];
  folderPath?: string;
  onTagsUpdated: (newTags: string[]) => void;
}

const TagSection: React.FC<TagSectionProps> = ({
  providerTags,
  userTags,
  folderPath,
  onTagsUpdated,
}) => {
  const [isAdding, setIsAdding] = useState(false);
  const [newTag, setNewTag] = useState('');
  const [isSaving, setIsSaving] = useState(false);

  /**
   * Handle remove user tag
   */
  const handleRemoveTag = async (tagToRemove: string) => {
    const updatedTags = userTags.filter(tag => tag !== tagToRemove);

    try {
      if (folderPath) {
        setIsSaving(true);
        await api.updateTags(folderPath, updatedTags);
        console.log(`[TagSection] ✅ Removed tag: ${tagToRemove}`);
      }

      onTagsUpdated(updatedTags);
    } catch (error) {
      console.error('[TagSection] ❌ Failed to remove tag:', error);
      // Rollback on error
      onTagsUpdated(userTags);
    } finally {
      setIsSaving(false);
    }
  };

  /**
   * Handle add new tag
   */
  const handleAddTag = async () => {
    const trimmedTag = newTag.trim();

    if (!trimmedTag) return;
    if (userTags.includes(trimmedTag)) {
      setNewTag('');
      setIsAdding(false);
      return;
    }

    const updatedTags = [...userTags, trimmedTag];

    try {
      if (folderPath) {
        setIsSaving(true);
        await api.updateTags(folderPath, updatedTags);
        console.log(`[TagSection] ✅ Added tag: ${trimmedTag}`);
      }

      onTagsUpdated(updatedTags);
      setNewTag('');
      setIsAdding(false);
    } catch (error) {
      console.error('[TagSection] ❌ Failed to add tag:', error);
      // Rollback on error
      onTagsUpdated(userTags);
    } finally {
      setIsSaving(false);
    }
  };

  /**
   * Handle Enter key in input
   */
  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddTag();
    } else if (e.key === 'Escape') {
      setNewTag('');
      setIsAdding(false);
    }
  };

  /**
   * Get tag style based on type
   */
  const getTagStyle = (tag: string, isUserTag: boolean): string => {
    if (isUserTag) {
      return 'bg-purple-500/20 text-purple-300 border-purple-500/50 hover:bg-purple-500/30';
    } else {
      return 'bg-zinc-700/50 text-zinc-400 border-zinc-600/50';
    }
  };

  return (
    <div className="space-y-3">
      {/* Provider Tags (Gray - Read-only) */}
      {providerTags.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {providerTags.slice(0, 10).map((tag, index) => (
            <span
              key={`provider-${index}`}
              className={`px-3 py-1 rounded-lg border text-sm cursor-default ${getTagStyle(tag, false)}`}
              title="Provider tag (read-only)"
            >
              {tag}
            </span>
          ))}
          {providerTags.length > 10 && (
            <span className="text-zinc-500 text-sm px-2 py-1">
              +{providerTags.length - 10} more
            </span>
          )}
        </div>
      )}

      {/* User Tags (Purple - Editable) */}
      <div className="flex flex-wrap gap-2 items-center">
        {userTags.map((tag) => (
          <span
            key={`user-${tag}`}
            className={`px-3 py-1 rounded-lg border text-sm flex items-center gap-1.5 transition-colors ${getTagStyle(tag, true)}`}
          >
            {tag}
            <button
              onClick={() => !isSaving && handleRemoveTag(tag)}
              disabled={isSaving}
              className="hover:bg-red-500/30 rounded p-0.5 transition-colors"
              title="Remove tag"
            >
              <X size={14} />
            </button>
          </span>
        ))}

        {/* Add Tag Button/Input */}
        {!isAdding ? (
          <button
            onClick={() => !isSaving && setIsAdding(true)}
            disabled={isSaving}
            className="px-3 py-1 rounded-lg border border-dashed border-zinc-600 text-zinc-500 hover:text-zinc-300 hover:border-zinc-500 text-sm flex items-center gap-1 transition-colors disabled:opacity-50"
          >
            <Plus size={14} />
            Add Tag
          </button>
        ) : (
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={newTag}
              onChange={(e) => setNewTag(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Tier 0, Favorites..."
              className="px-3 py-1 bg-zinc-900 border border-zinc-600 rounded-lg text-white text-sm focus:border-purple-500 focus:outline-none focus:ring-2 focus:ring-purple-500/50 w-40"
              autoFocus
              disabled={isSaving}
            />
            <button
              onClick={handleAddTag}
              disabled={isSaving || !newTag.trim()}
              className="px-3 py-1 bg-purple-600 hover:bg-purple-700 disabled:bg-zinc-700 text-white rounded-lg text-sm transition-colors disabled:opacity-50"
            >
              {isSaving ? '...' : 'Add'}
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default DetailsView;
