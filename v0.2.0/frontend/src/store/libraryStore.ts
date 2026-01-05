/**
 * Library Store (Zustand)
 *
 * PHASE 13: The Framework
 *
 * Global state management for library view, filters, and selection.
 */

import { create } from 'zustand';

export interface ViewMode {
  type: 'grid' | 'list' | 'focus';
  sortBy: 'title' | 'date' | 'developer' | 'rating';
  sortOrder: 'asc' | 'desc';
}

interface LibraryState {
  // View state
  viewMode: ViewMode;
  selectedGames: Set<string>;
  searchQuery: string;

  // Filters
  filters: {
    developers: string[];
    tags: string[];
    yearRange: [number, number] | null;
    engines: string[];
  };

  // UI state
  sidebarCollapsed: boolean;
  isLoading: boolean;
  isFilterPanelOpen: boolean;

  // Actions
  setViewMode: (mode: Partial<ViewMode>) => void;
  toggleGameSelection: (gameId: string) => void;
  clearSelection: () => void;
  setSearchQuery: (query: string) => void;
  setFilters: (filters: Partial<LibraryState['filters']>) => void;
  clearFilters: () => void;
  toggleSidebar: () => void;
  setLoading: (loading: boolean) => void;
  setFilterPanelOpen: (open: boolean) => void;
  toggleFilterPanel: () => void;
}

export const useLibraryStore = create<LibraryState>((set) => ({
  // Initial state
  viewMode: {
    type: 'grid',
    sortBy: 'title',
    sortOrder: 'asc',
  },
  selectedGames: new Set<string>(),
  searchQuery: '',
  filters: {
    developers: [],
    tags: [],
    yearRange: null,
    engines: [],
  },
  sidebarCollapsed: false,
  isLoading: false,
  isFilterPanelOpen: false,

  // Actions
  setViewMode: (mode) =>
    set((state) => ({
      viewMode: { ...state.viewMode, ...mode },
    })),

  toggleGameSelection: (gameId) =>
    set((state) => {
      const newSelection = new Set(state.selectedGames);
      if (newSelection.has(gameId)) {
        newSelection.delete(gameId);
      } else {
        newSelection.add(gameId);
      }
      return { selectedGames: newSelection };
    }),

  clearSelection: () =>
    set({ selectedGames: new Set() }),

  setSearchQuery: (query) =>
    set({ searchQuery: query }),

  setFilters: (newFilters) =>
    set((state) => ({
      filters: { ...state.filters, ...newFilters },
    })),

  clearFilters: () =>
    set({
      filters: {
        developers: [],
        tags: [],
        yearRange: null,
        engines: [],
      },
    }),

  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  setLoading: (loading) =>
    set({ isLoading: loading }),

  setFilterPanelOpen: (open) =>
    set({ isFilterPanelOpen: open }),

  toggleFilterPanel: () =>
    set((state) => ({ isFilterPanelOpen: !state.isFilterPanelOpen })),
}));
