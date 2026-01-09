/**
 * useLibrary Hook
 * 
 * Fetches library data from API with loading/error states.
 * Falls back to mock data in development if API fails.
 */

import { useState, useEffect, useCallback } from 'react';
import { getLibrary, LibraryEntry } from '../api';
import { mockLibraryData } from '../mockData';
import { GameCardData } from '../types/GameCard';

interface UseLibraryResult {
      data: (GameCardData & { id: string; featured?: boolean })[];
      loading: boolean;
      error: Error | null;
      refetch: () => void;
}

/**
 * Convert API LibraryEntry to GameCardData format for UI
 */
function libraryEntryToGameCard(entry: LibraryEntry): GameCardData & { id: string; featured?: boolean } {
      return {
            id: entry.entry_id,
            entry_type: entry.entry_type,
            display_title: entry.display_title,
            cover_image: entry.cover_image_url,
            instance_count: entry.instance_count,
            actions_allowed: 'NONE',
            // Mark as featured if has high confidence or first 2 items
            featured: false,
      };
}

export function useLibrary(): UseLibraryResult {
      const [data, setData] = useState<(GameCardData & { id: string; featured?: boolean })[]>([]);
      const [loading, setLoading] = useState(true);
      const [error, setError] = useState<Error | null>(null);

      const fetchData = useCallback(async () => {
            setLoading(true);
            setError(null);

            try {
                  const response = await getLibrary(0, 100);

                  if (response.entries.length === 0) {
                        // Empty database - return empty array (will show skeleton)
                        setData([]);
                  } else {
                        setData(response.entries.map(libraryEntryToGameCard));
                  }
            } catch (err) {
                  console.warn('API unavailable, using mock data:', err);
                  // Fallback to mock data in development
                  if (import.meta.env.DEV) {
                        setData(mockLibraryData);
                  } else {
                        setError(err instanceof Error ? err : new Error('API request failed'));
                  }
            } finally {
                  setLoading(false);
            }
      }, []);

      useEffect(() => {
            fetchData();
      }, [fetchData]);

      return { data, loading, error, refetch: fetchData };
}
