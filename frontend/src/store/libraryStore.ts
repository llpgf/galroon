/**
 * Library Store (Zustand)
 *
 * PHASE 13: The Framework
 * SPRINT 9: Tags, Privacy & i18n
 * SPRINT 9.5: Gallery/Workshop Dual View & Display Grammar
 *
 * Global state management for library view, filters, and selection.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface ViewMode {
  type: 'grid' | 'list' | 'focus';
  sortBy: 'title' | 'date' | 'developer' | 'rating';
  sortOrder: 'asc' | 'desc';
}

// Sprint 9.5: Display Grammar Types
export type DisplayMode = 'grid' | 'compact' | 'detail' | 'strip';

export interface ContextMenuState {
  x: number;
  y: number;
  itemId: string;
  selectedCount: number;
}

interface LibraryState {
  // View state
  viewMode: ViewMode;
  selectedGames: Set<string>;
  searchQuery: string;

  // Sprint 9.5: Display Grammar
  displayMode: DisplayMode;
  densityLevel: number; // 0-100
  contextMenu: ContextMenuState | null;

  // Filters (Sprint 9: Enhanced with include/exclude)
  filters: {
    developers: string[];
    tags: string[];
    includeTags: string[];
    excludeTags: string[];
    yearRange: [number, number] | null;
    engines: string[];
  };

  // UI state
  sidebarCollapsed: boolean;
  isLoading: boolean;
  isFilterPanelOpen: boolean;

  // Sprint 9: SFW Mode (Privacy Blur)
  sfwMode: boolean;

  // Actions
  setViewMode: (mode: Partial<ViewMode>) => void;
  toggleGameSelection: (gameId: string) => void;
  selectMultiple: (gameId: string, isCtrl: boolean, isShift: boolean) => void;
  clearSelection: () => void;
  setSearchQuery: (query: string) => void;
  setFilters: (filters: Partial<LibraryState['filters']>) => void;
  clearFilters: () => void;
  toggleSidebar: () => void;
  setLoading: (loading: boolean) => void;
  setFilterPanelOpen: (open: boolean) => void;
  toggleFilterPanel: () => void;

  // Sprint 9: New actions
  setSfwMode: (enabled: boolean) => void;
  toggleSfwMode: () => void;
  setIncludeTags: (tags: string[]) => void;
  setExcludeTags: (tags: string[]) => void;
  toggleTagFilter: (tag: string) => void;

  // Sprint 9.5: Display Grammar actions
  setDisplayMode: (mode: DisplayMode) => void;
  setDensityLevel: (level: number) => void;
  showContextMenu: (x: number, y: number, itemId: string) => void;
  hideContextMenu: () => void;
}

// Helper to get initial SFW mode from localStorage
const getInitialSfwMode = (): boolean => {
  if (typeof window !== 'undefined') {
    const stored = localStorage.getItem('galroon-sfw-mode');
    return stored === 'true';
  }
  return false;
};

// Helper to get initial density from localStorage
const getInitialDensity = (): number => {
  if (typeof window !== 'undefined') {
    const stored = localStorage.getItem('galroon-density-level');
    return stored ? parseInt(stored, 10) : 50;
  }
  return 50;
};

// Helper to get initial display mode from localStorage
const getInitialDisplayMode = (): DisplayMode => {
  if (typeof window !== 'undefined') {
    const stored = localStorage.getItem('galroon-display-mode');
    if (stored && ['grid', 'compact', 'detail', 'strip'].includes(stored)) {
      return stored as DisplayMode;
    }
  }
  return 'grid';
};

export const useLibraryStore = create<LibraryState>((set, get) => ({
  // Initial state
  viewMode: {
    type: 'grid',
    sortBy: 'title',
    sortOrder: 'asc',
  },
  selectedGames: new Set<string>(),
  searchQuery: '',

  // Sprint 9.5: Display Grammar initial state
  displayMode: getInitialDisplayMode(),
  densityLevel: getInitialDensity(),
  contextMenu: null,

  filters: {
    developers: [],
    tags: [],
    includeTags: [],
    excludeTags: [],
    yearRange: null,
    engines: [],
  },
  sidebarCollapsed: false,
  isLoading: false,
  isFilterPanelOpen: false,
  sfwMode: getInitialSfwMode(),

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

  // Sprint 9.5: Multi-select with Ctrl/Shift support
  selectMultiple: (gameId, isCtrl, isShift) =>
    set((state) => {
      const newSelection = new Set(state.selectedGames);

      if (isCtrl) {
        // Toggle selection
        if (newSelection.has(gameId)) {
          newSelection.delete(gameId);
        } else {
          newSelection.add(gameId);
        }
      } else if (isShift) {
        // Add to selection (range selection would need item order)
        newSelection.add(gameId);
      } else {
        // Single selection
        newSelection.clear();
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
        includeTags: [],
        excludeTags: [],
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

  // Sprint 9: SFW Mode actions
  setSfwMode: (enabled) => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('galroon-sfw-mode', String(enabled));
    }
    set({ sfwMode: enabled });
  },

  toggleSfwMode: () => {
    const newValue = !get().sfwMode;
    if (typeof window !== 'undefined') {
      localStorage.setItem('galroon-sfw-mode', String(newValue));
    }
    set({ sfwMode: newValue });
  },

  // Sprint 9: Tag filter actions
  setIncludeTags: (tags) =>
    set((state) => ({
      filters: { ...state.filters, includeTags: tags },
    })),

  setExcludeTags: (tags) =>
    set((state) => ({
      filters: { ...state.filters, excludeTags: tags },
    })),

  // Tri-state toggle: none -> include -> exclude -> none
  toggleTagFilter: (tag) =>
    set((state) => {
      const { includeTags, excludeTags } = state.filters;

      if (includeTags.includes(tag)) {
        return {
          filters: {
            ...state.filters,
            includeTags: includeTags.filter((t) => t !== tag),
            excludeTags: [...excludeTags, tag],
          },
        };
      } else if (excludeTags.includes(tag)) {
        return {
          filters: {
            ...state.filters,
            excludeTags: excludeTags.filter((t) => t !== tag),
          },
        };
      } else {
        return {
          filters: {
            ...state.filters,
            includeTags: [...includeTags, tag],
          },
        };
      }
    }),

  // Sprint 9.5: Display Grammar actions
  setDisplayMode: (mode) => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('galroon-display-mode', mode);
    }
    set({ displayMode: mode });
  },

  setDensityLevel: (level) => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('galroon-density-level', String(level));
    }
    set({ densityLevel: level });
  },

  showContextMenu: (x, y, itemId) =>
    set((state) => ({
      contextMenu: {
        x,
        y,
        itemId,
        selectedCount: state.selectedGames.size || 1,
      },
    })),

  hideContextMenu: () =>
    set({ contextMenu: null }),
}));

