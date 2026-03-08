// GalleryPage — main page shell with state management + toast notifications.

import { useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import {
      WorkSummary,
      SortField,
      SortDirection,
      listWorks,
      searchWorks,
      triggerScan,
} from '../../hooks/api';
import { useToast } from '../../components/Toast';
import { GalleryFilters } from '../../components/GalleryFilters';
import { GalleryGrid } from '../../components/GalleryGrid';
import './GalleryPage.css';

export default function GalleryPage() {
      const { showToast } = useToast();

      const [works, setWorks] = useState<WorkSummary[]>([]);
      const [isLoading, setIsLoading] = useState(true);
      const [isScanning, setIsScanning] = useState(false);

      const [searchQuery, setSearchQuery] = useState('');
      const [sortField, setSortField] = useState<SortField>('title');
      const [sortDirection, setSortDirection] = useState<SortDirection>('asc');
      const [statusFilter, setStatusFilter] = useState<string | null>(null);
      const [assetFilter, setAssetFilter] = useState<string | null>(null);

      useEffect(() => {
            loadWorks(assetFilter);
      }, [assetFilter]);

      const loadWorks = useCallback(async (nextAssetFilter: string | null = assetFilter) => {
            setIsLoading(true);
            try {
                  const data = await listWorks(1, 500, nextAssetFilter);
                  setWorks(Array.isArray(data) ? data : []);
            } catch (err) {
                  showToast('Failed to load library', 'error');
            } finally {
                  setIsLoading(false);
            }
      }, [assetFilter, showToast]);

      useEffect(() => {
            if (!searchQuery.trim()) return;

            const timer = setTimeout(async () => {
                  try {
                        const results = await searchWorks(searchQuery);
                        setWorks(results);
                  } catch {
                        // Search failed, keep current results
                  }
            }, 300);

            return () => clearTimeout(timer);
      }, [searchQuery]);

      useEffect(() => {
            if (searchQuery === '') {
                  loadWorks(assetFilter);
            }
      }, [searchQuery, assetFilter, loadWorks]);

      const filteredWorks = useMemo(() => {
            const safeWorks = Array.isArray(works) ? works : [];
            let result = [...safeWorks];

            if (statusFilter) {
                  result = result.filter((w) => w.library_status === statusFilter);
            }

            if (assetFilter) {
                  result = result.filter((w) => w.asset_types.includes(assetFilter));
            }

            result.sort((a, b) => {
                  let cmp = 0;
                  switch (sortField) {
                        case 'title':
                              cmp = a.title.localeCompare(b.title, 'ja');
                              break;
                        case 'rating':
                              cmp = (a.rating ?? 0) - (b.rating ?? 0);
                              break;
                        case 'release_date':
                              cmp = (a.release_date ?? '').localeCompare(b.release_date ?? '');
                              break;
                        default:
                              cmp = a.title.localeCompare(b.title, 'ja');
                  }
                  return sortDirection === 'asc' ? cmp : -cmp;
            });

            return result;
      }, [works, statusFilter, assetFilter, sortField, sortDirection]);

      const handleScan = useCallback(async () => {
            setIsScanning(true);
            try {
                  const result = await triggerScan();
                  showToast(
                        result.job_id
                              ? `Scan queued as background job #${result.job_id}`
                              : 'Scan queued',
                        'success',
                  );
            } catch (err) {
                  showToast('Scan failed — check Settings for library paths', 'error');
            } finally {
                  setIsScanning(false);
            }
      }, [assetFilter, loadWorks, showToast]);

      const handleSearchChange = useCallback((query: string) => {
            setSearchQuery(query);
      }, []);

      const handleSortChange = useCallback((field: SortField, direction: SortDirection) => {
            setSortField(field);
            setSortDirection(direction);
      }, []);

      const navigate = useNavigate();

      const handleCardClick = useCallback((id: string) => {
            navigate(`/work/${id}`);
      }, [navigate]);

      const [density, setDensity] = useState(180);
      const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');

      return (
            <div className="gallery-page" style={{ '--card-width': `${density}px` } as React.CSSProperties}>
                  <GalleryFilters
                        searchQuery={searchQuery}
                        onSearchChange={handleSearchChange}
                        sortField={sortField}
                        sortDirection={sortDirection}
                        onSortChange={handleSortChange}
                        statusFilter={statusFilter}
                        onStatusFilterChange={setStatusFilter}
                        assetFilter={assetFilter}
                        onAssetFilterChange={setAssetFilter}
                        totalCount={filteredWorks.length}
                        onScanClick={handleScan}
                        isScanning={isScanning}
                        density={density}
                        onDensityChange={setDensity}
                        viewMode={viewMode}
                        onViewModeChange={setViewMode}
                  />
                  <GalleryGrid
                        works={filteredWorks}
                        isLoading={isLoading}
                        onCardClick={handleCardClick}
                        viewMode={viewMode}
                  />
            </div>
      );
}

