import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';
import type { ApiError } from '../api/client';

/**
 * Game details from backend UnifiedMetadata schema
 *
 * Matches backend/app/metadata/models.py:UnifiedMetadata
 */
export interface GameDetails {
  id: string;
  vndb_id?: string;
  title: string;
  title_original?: string;
  developer?: string;
  release_date?: string;
  description?: string;
  cover_image?: string;
  hero_image?: string;
  folder_path?: string;

  // Metadata (Phase 9)
  metadata?: {
    title?: {
      value: string | { original?: string; en?: string; ja?: string; zh_hans?: string; zh_hant?: string };
      locked: boolean;
    };
    developer?: {
      value: string;
      locked: boolean;
    };
    description?: {
      value: string;
      locked: boolean;
    };
  };

  // Assets (Phase 9)
  assets_detected?: string[];

  // Library Status (Phase 19.6: Semantic Sanitization)
  library_status?: string;

  // Tags (Phase 18.5)
  tags?: string[];  // Read-only provider tags (VNDB, Bangumi, etc.)
  user_tags?: string[];  // User-defined custom tags (editable)

  // Lock status
  locked_fields?: string[];

  // External IDs
  external_ids?: {
    steam?: string;
    bangumi?: string;
    erogamescape?: string;
  };

  // Versions
  versions?: Array<{
    id: string;
    type: string;
    path?: string;
    primary?: boolean;
  }>;
}

/**
 * useGameDetails - Phase 18: The Great Wiring
 *
 * Fetches game metadata from backend using centralized API client
 *
 * Endpoint: GET /api/metadata/game/{id}
 */
export const useGameDetails = (gameId: string) => {
  const [details, setDetails] = useState<GameDetails | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchDetails = useCallback(async () => {
    if (!gameId) {
      setError('No game ID provided');
      setIsLoading(false);
      return;
    }

    try {
      setIsLoading(true);
      setError(null);

      const response = await api.getGameMetadata(gameId);

      // Transform backend UnifiedMetadata to GameDetails
      const metadata = response.data;

      // Extract title with multilingual fallback
      const titleData = typeof metadata.title === 'object'
        ? metadata.title.value || metadata.title
        : metadata.title;

      const title = typeof titleData === 'object'
        ? (titleData.zh_hant || titleData.zh_hans || titleData.en || titleData.ja || titleData.original || 'Untitled')
        : titleData;

      const transformedDetails: GameDetails = {
        id: metadata.folder_path || gameId,
        vndb_id: metadata.vndb_id,
        title,
        title_original: typeof titleData === 'object' ? titleData.original : undefined,
        developer: metadata.developer?.value || metadata.developer,
        release_date: metadata.release_date?.value || metadata.release_date,
        description: metadata.description?.value || metadata.description,
        cover_image: metadata.cover_path || metadata.cover_url?.value,
        hero_image: metadata.hero_path || metadata.hero_url?.value,
        folder_path: metadata.folder_path,
        metadata: {
          title: metadata.title,
          developer: metadata.developer,
          description: metadata.description,
        },
        assets_detected: metadata.assets_detected,
        tags: metadata.tags?.value || metadata.tags,
        user_tags: metadata.user_tags || [],
        locked_fields: metadata.locked_fields,
        external_ids: metadata.external_ids,
        versions: metadata.versions,
      };

      setDetails(transformedDetails);
      console.log(`[useGameDetails] ✅ Loaded details for: ${title}`);
    } catch (err) {
      const enhancedError = err as { apiError?: ApiError };

      if (enhancedError.apiError?.status === 404) {
        setError('Game not found');
      } else {
        const errorMessage = enhancedError.apiError?.message || 'Failed to load game details';
        console.error('[useGameDetails] ❌ ' + errorMessage);
        setError(errorMessage);
      }
    } finally {
      setIsLoading(false);
    }
  }, [gameId]);

  /**
   * Toggle field lock status
   *
   * Uses Curator API endpoints:
   * - POST /api/curator/lock_fields
   * - POST /api/curator/unlock_fields
   */
  const toggleLock = useCallback(async (field: string) => {
    if (!details?.folder_path) return false;

    try {
      const isLocked = details.metadata?.[field as keyof typeof details.metadata]?.locked;

      // Call lock or unlock endpoint based on current state
      if (isLocked) {
        await api.unlockFields(details.folder_path, [field]);
        console.log(`[useGameDetails] 🔓 Unlocked field: ${field}`);
      } else {
        await api.lockFields(details.folder_path, [field]);
        console.log(`[useGameDetails] 🔒 Locked field: ${field}`);
      }

      // Refresh details to get updated state
      await fetchDetails();
      return true;
    } catch (err) {
      console.error('[useGameDetails] ❌ Failed to toggle lock:', err);
      return false;
    }
  }, [details, fetchDetails]);

  /**
   * Update metadata field
   *
   * Endpoint: POST /api/curator/update_field
   */
  const updateField = useCallback(async (field: string, value: any) => {
    if (!details?.folder_path) return false;

    try {
      await api.updateField(details.folder_path, field, value);
      console.log(`[useGameDetails] ✅ Updated field: ${field}`);

      // Refresh details to get updated state
      await fetchDetails();
      return true;
    } catch (err) {
      console.error('[useGameDetails] ❌ Failed to update field:', err);
      return false;
    }
  }, [details, fetchDetails]);

  useEffect(() => {
    fetchDetails();
  }, [fetchDetails]);

  return {
    details,
    isLoading,
    error,
    toggleLock,
    updateField,
    refresh: fetchDetails,
  };
};

export default useGameDetails;
