// GalleryFilters — search, sort, tag filter, status filter.

import { useState, useCallback } from 'react';
import { SortField, SortDirection } from '../hooks/api';
import './GalleryFilters.css';

interface Props {
      searchQuery: string;
      onSearchChange: (query: string) => void;
      sortField: SortField;
      sortDirection: SortDirection;
      onSortChange: (field: SortField, direction: SortDirection) => void;
      statusFilter: string | null;
      onStatusFilterChange: (status: string | null) => void;
      assetFilter: string | null;
      onAssetFilterChange: (assetType: string | null) => void;
      totalCount: number;
      onScanClick: () => void;
      isScanning: boolean;
      density: number;
      onDensityChange: (d: number) => void;
      viewMode: 'grid' | 'list';
      onViewModeChange: (m: 'grid' | 'list') => void;
}

const STATUS_OPTIONS = [
      { value: null, label: 'All' },
      { value: 'unplayed', label: 'Unplayed' },
      { value: 'playing', label: 'Playing' },
      { value: 'completed', label: 'Completed' },
      { value: 'on_hold', label: 'On Hold' },
      { value: 'dropped', label: 'Dropped' },
      { value: 'wishlist', label: 'Wishlist' },
] as const;

const ASSET_OPTIONS = [
      { value: null, label: 'All Assets' },
      { value: 'game', label: 'Game' },
      { value: 'dlc', label: 'DLC' },
      { value: 'update', label: 'Update' },
      { value: 'ost', label: 'OST' },
      { value: 'voice_drama', label: 'Voice Drama' },
      { value: 'bonus', label: 'Bonus' },
      { value: 'crack', label: 'Crack' },
] as const;

const SORT_OPTIONS: { value: SortField; label: string }[] = [
      { value: 'title', label: 'Title' },
      { value: 'rating', label: 'Rating' },
      { value: 'release_date', label: 'Release Date' },
      { value: 'created_at', label: 'Date Added' },
];

export function GalleryFilters({
      searchQuery,
      onSearchChange,
      sortField,
      sortDirection,
      onSortChange,
      statusFilter,
      onStatusFilterChange,
      assetFilter,
      onAssetFilterChange,
      totalCount,
      onScanClick,
      isScanning,
      density,
      onDensityChange,
      viewMode,
      onViewModeChange,
}: Props) {
      const [searchFocused, setSearchFocused] = useState(false);

      const toggleSortDirection = useCallback(() => {
            onSortChange(sortField, sortDirection === 'asc' ? 'desc' : 'asc');
      }, [sortField, sortDirection, onSortChange]);

      return (
            <div className="gallery-filters">
                  <div className={`filter-search ${searchFocused ? 'focused' : ''}`}>
                        <span className="search-icon">🔍</span>
                        <input
                              type="text"
                              placeholder="Search games..."
                              value={searchQuery}
                              onChange={(e) => onSearchChange(e.target.value)}
                              onFocus={() => setSearchFocused(true)}
                              onBlur={() => setSearchFocused(false)}
                              id="gallery-search"
                        />
                        {searchQuery && (
                              <button
                                    className="search-clear"
                                    onClick={() => onSearchChange('')}
                                    aria-label="Clear search"
                              >
                                    ✕
                              </button>
                        )}
                  </div>

                  <div className="filter-status-pills">
                        {STATUS_OPTIONS.map((opt) => (
                              <button
                                    key={opt.value ?? 'all'}
                                    className={`status-pill ${statusFilter === opt.value ? 'active' : ''}`}
                                    onClick={() => onStatusFilterChange(opt.value)}
                              >
                                    {opt.label}
                              </button>
                        ))}
                  </div>

                  <div className="filter-actions">
                        <select
                              className="sort-select"
                              value={assetFilter ?? ''}
                              onChange={(e) => onAssetFilterChange(e.target.value || null)}
                              id="gallery-asset-filter"
                        >
                              {ASSET_OPTIONS.map((opt) => (
                                    <option key={opt.value ?? 'all-assets'} value={opt.value ?? ''}>
                                          {opt.label}
                                    </option>
                              ))}
                        </select>

                        <select
                              className="sort-select"
                              value={sortField}
                              onChange={(e) => onSortChange(e.target.value as SortField, sortDirection)}
                              id="gallery-sort"
                        >
                              {SORT_OPTIONS.map((opt) => (
                                    <option key={opt.value} value={opt.value}>
                                          {opt.label}
                                    </option>
                              ))}
                        </select>

                        <button
                              className="sort-direction"
                              onClick={toggleSortDirection}
                              title={sortDirection === 'asc' ? 'Ascending' : 'Descending'}
                              aria-label={`Sort ${sortDirection === 'asc' ? 'ascending' : 'descending'}`}
                        >
                              {sortDirection === 'asc' ? '↑' : '↓'}
                        </button>

                        <span className="filter-count">{totalCount} posters</span>

                        <button
                              className="scan-button"
                              onClick={onScanClick}
                              disabled={isScanning}
                              title="Scan library folders"
                        >
                              {isScanning ? '⟳' : '📂'} Scan
                        </button>

                        <div className="view-toggle">
                              <button
                                    className={viewMode === 'grid' ? 'active' : ''}
                                    onClick={() => onViewModeChange('grid')}
                                    title="Grid view"
                              >▦</button>
                              <button
                                    className={viewMode === 'list' ? 'active' : ''}
                                    onClick={() => onViewModeChange('list')}
                                    title="List view"
                              >☰</button>
                        </div>

                        <input
                              type="range"
                              className="density-slider"
                              min={120}
                              max={280}
                              value={density}
                              onChange={(e) => onDensityChange(Number(e.target.value))}
                              title={`Card size: ${density}px`}
                        />
                  </div>
            </div>
      );
}
