import { useState, useEffect, useCallback, useMemo } from 'react';
import { api } from '../api/client';
import type { ApiError } from '../api/client';
import type { AssetCardProps } from '../components/library/AssetCard';
import { useLibraryStore } from '../store/libraryStore';

/**
 * Asset badge types
 */
type AssetBadge = 'ISO' | 'DLC' | 'Patch';

/**
 * useLibrary - Phase 18: The Great Wiring
 *
 * Fetches from GET /api/games and transforms backend UnifiedMetadata to AssetCardProps
 *
 * ✅ Phase 19.6: Semantic Sanitization
 * - Supports: search, tag, library_status, sort_by, limit, offset
 * - Returns: GameSummary[] with title, developer, cover_image, badges, rating, etc.
 */

interface UseLibraryResult {
  assets: AssetCardProps[];
  filteredAssets: AssetCardProps[];
  isLoading: boolean;
  error: string | null;
  totalAssets: number;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  refresh: () => void;
  // Phase 22.0: Pagination support
  currentPage: number;
  totalPages: number;
  goToPage: (page: number) => void;
  // Phase 22.0: Sort support
  sortBy: string;
  setSortBy: (sortBy: string) => void;
}

/**
 * Backend UnifiedMetadata structure (from backend/app/metadata/models.py)
 *
 * Only including fields we actually use for the Library View
 */
interface GameMetadata {
  folder_path: string;
  vndb_id?: string;
  title: {
    value: {
      original?: string;
      en?: string;
      ja?: string;
      zh_hans?: string;
      zh_hant?: string;
    };
    locked: boolean;
  };
  developer: {
    value: string;
    locked: boolean;
  };
  cover_url: {
    value: string;
    locked: boolean;
  };
  cover_path?: string;
  assets_detected: string[];  // ['ISO', 'DLC', 'OST', 'Crack', 'Chinese', etc.]
  // Phase 19.10: Sorting fields
  release_date?: {
    value: string;
    locked: boolean;
  };
  rating?: {
    value: {
      score: number;
      count: number;
    };
    locked: boolean;
  };
  // Level 3: Library status for inline editing
  library_status?: {
    value: string;
    locked?: boolean;
  };
}

export const useLibrary = (): UseLibraryResult => {
  // Phase 22.0: State management for backend pagination
  const [assets, setAssets] = useState<AssetCardProps[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalAssets, setTotalAssets] = useState(0);
  const [currentPage, setCurrentPage] = useState(0);
  const [sortBy, setSortBy] = useState('最近添加');

  // Phase 18.5: Get filters from store for tag filtering
  const filters = useLibraryStore((state) => state.filters);
  const viewMode = useLibraryStore((state) => state.viewMode);

  // Phase 22.0: Items per page (can be adjusted based on view mode)
  const itemsPerPage = viewMode.type === 'grid' ? 50 : 100;

  /**
   * Transform backend asset types to AssetBadge enum
   *
   * Only show ISO, DLC, Patch badges (as per Figma design)
   * Filter out: OST, Crack, Chinese, Setup, etc.
   */
  const transformAssetsToBadges = (assets: string[] = []): AssetBadge[] => {
    const validBadges: Set<string> = new Set(['ISO', 'DLC', 'Patch']);

    return assets
      .map(asset => {
        // Normalize casing
        const normalized = asset.toLowerCase();

        // Map variations to standard badge names
        if (normalized === 'iso') return 'ISO';
        if (normalized === 'dlc') return 'DLC';
        if (normalized.includes('patch')) return 'Patch';

        return null;
      })
      .filter((badge): badge is AssetBadge => badge !== null && validBadges.has(badge));
  };

  /**
   * Extract title with multilingual fallback
   *
   * Priority: zh_hant → zh_hans → en → ja → original
   */
  const extractTitle = (titleData: GameMetadata['title']['value']): string => {
    return (
      titleData.zh_hant ||
      titleData.zh_hans ||
      titleData.en ||
      titleData.ja ||
      titleData.original ||
      'Untitled'
    );
  };

  /**
   * Transform backend GameMetadata to AssetCardProps
   *
   * Critical mapping:
   * - assets_detected (string[]) -> badges (AssetBadge[])
   * - cover_path || cover_url.value -> coverImage
   * - title (with fallback) -> title
   * - release_date.value -> release_date (for sorting)
   * - rating.value.score -> rating (for sorting)
   */
  const transformMetadataToCard = (metadata: GameMetadata): AssetCardProps => {
    const title = extractTitle(metadata.title.value);

    // Cover image fallback: local path first, then URL
    const coverImage = metadata.cover_path || metadata.cover_url.value;

    // Phase 18.1: Safety check for assets_detected
    // Ensure we don't crash if assets_detected is undefined/null
    const badges = metadata.assets_detected
      ? transformAssetsToBadges(metadata.assets_detected)
      : [];

    return {
      id: metadata.folder_path,
      title,
      developer: metadata.developer?.value || 'Unknown Developer',
      coverImage,
      badges,
      // Phase 18.5: Include tags for filtering
      tags: (metadata as any).tags?.value || (metadata as any).tags || [],
      user_tags: (metadata as any).user_tags || [],
      // Phase 19.10: Sorting support
      release_date: metadata.release_date?.value,
      rating: metadata.rating?.value?.score,
      // Level 3: Library status for inline editing
      library_status: metadata.library_status?.value || 'unstarted',
    };
  };

  /**
   * Fetch games from backend
   *
   * Phase 22.0: ✅ BACKEND PAGINATION, SEARCH, SORT
   * Endpoint: GET /api/games
   *
   * Supports:
   * - search: Full-text search query (FTS5)
   * - sort_by: Sorting field ("最近添加", "名称", "发行日期", "评分")
   * - skip: Pagination offset
   * - limit: Results per page
   * - filter_tag: Tag filtering
   *
   * Response: GamesListResponse { data: GameMetadata[], total, strategy }
   */
  const fetchGames = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);

      // Phase 22.0: Build query parameters for backend
      const params: any = {
        skip: currentPage * itemsPerPage,
        limit: itemsPerPage,
        sort_by: sortBy,
      };

      // Add search query if provided
      if (searchQuery.trim()) {
        params.search = searchQuery.trim();
      }

      // Add tag filter if selected
      if (filters.tags.length > 0) {
        // Backend expects filter_tag parameter
        params.filter_tag = filters.tags[0]; // Use first selected tag
      }

      console.log(`[useLibrary] Fetching with params:`, params);

      const response = await api.getAllGames(params);

      // Phase 22.0: Handle new response format
      // Backend returns: { data: GameMetadata[], total, strategy }
      const gamesData = response.data.data || response.data; // Handle both formats
      const total = response.data.total || gamesData.length;

      const transformedAssets = gamesData.map(transformMetadataToCard);
      setAssets(transformedAssets);
      setTotalAssets(total);

      console.log(`[useLibrary] ✅ Loaded ${transformedAssets.length}/${total} games (strategy: ${response.data.strategy || 'unknown'})`);
    } catch (err) {
      const enhancedError = err as { apiError?: ApiError };

      if (enhancedError.apiError?.status === 404) {
        // Endpoint doesn't exist
        const criticalMessage =
          'CRITICAL: Backend endpoint /api/games does not exist. ' +
          'Ensure Phase 20.0 backend is running.';

        console.error('[useLibrary] 🔥 ' + criticalMessage);
        setError(criticalMessage);
        setAssets([]);
        setTotalAssets(0);
      } else {
        // Other errors (network, 500, etc.)
        const errorMessage = enhancedError.apiError?.message || 'Failed to load games';
        console.error('[useLibrary] ❌ ' + errorMessage);
        setError(errorMessage);
      }
    } finally {
      setIsLoading(false);
    }
  }, [currentPage, itemsPerPage, sortBy, searchQuery, filters.tags]);

  /**
   * Filter assets based on tags only (search now handled by backend)
   *
   * Phase 22.0: Tag filtering for client-side additional filtering
   * Backend handles search, we handle remaining tag filtering
   */
  const filteredAssets = useMemo(() => {
    if (filters.tags.length === 0) {
      return assets;
    }

    // Check if asset has any of the selected tags
    return assets.filter((asset) => {
      const hasTag = filters.tags.some(tag =>
        asset.tags?.includes(tag) || asset.user_tags?.includes(tag)
      );
      return hasTag;
    });
  }, [assets, filters.tags]);

  /**
   * Calculate total pages
   */
  const totalPages = Math.ceil(totalAssets / itemsPerPage);

  /**
   * Go to specific page
   */
  const goToPage = useCallback((page: number) => {
    setCurrentPage(page);
  }, []);

  /**
   * Refetch when dependencies change
   */
  useEffect(() => {
    fetchGames();
  }, [fetchGames]);

  /**
   * Reset to page 0 when search query or sort changes
   */
  useEffect(() => {
    setCurrentPage(0);
  }, [searchQuery, sortBy]);

  return {
    assets,
    filteredAssets,
    isLoading,
    error,
    totalAssets,
    searchQuery,
    setSearchQuery,
    refresh: fetchGames,
    // Phase 22.0: Pagination
    currentPage,
    totalPages,
    goToPage,
    // Phase 22.0: Sort
    sortBy,
    setSortBy,
  };
};

export default useLibrary;
