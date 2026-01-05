import React, { useState, useMemo } from 'react';
import { AssetCard } from '../components/library/AssetCard';
import { FocusFilter } from '../components/library/FocusFilter';
import { useLibrary } from '../hooks/useLibrary';
import { useLibraryStore } from '../store/libraryStore';
import { ChevronLeft, ChevronRight, ChevronLeft as PageLeft, ChevronRight as PageRight, Filter } from 'lucide-react';

/**
 * LibraryView - Main Library Grid View (Figma Design)
 *
 * Phase 22.0: ✅ BACKEND POWERED
 * - Search handled by backend (FTS5 full-text search)
 * - Sorting handled by backend (SQLite ORDER BY)
 * - Pagination handled by backend (skip/limit)
 *
 * Layout:
 * - "最近游戏" (horizontal scroll, snap-x) - First 6 from current page
 * - "我的收藏" (responsive grid + tag filters + sort + pagination)
 */

type SortOption = '最近添加' | '名称' | '发行日期' | '评分';

export const LibraryView: React.FC<{ onAssetClick: (assetId: string) => void }> = ({
  onAssetClick,
}) => {
  // Phase 22.0: Use backend-powered hook
  const {
    filteredAssets,
    searchQuery,
    setSearchQuery,
    sortBy,
    setSortBy,
    currentPage,
    totalPages,
    goToPage,
    isLoading,
  } = useLibrary();

  // Get filter panel state
  const { setFilterPanelOpen } = useLibraryStore();

  const [selectedTag, setSelectedTag] = useState<string>('所有游戏');
  const [recentScrollPosition, setRecentScrollPosition] = useState(0);

  // Split into recently added (first 6) and collection
  const recentlyAdded = filteredAssets.slice(0, 6);
  const myCollection = filteredAssets;

  const scrollRecentGames = (direction: 'left' | 'right') => {
    const container = document.getElementById('recent-games-scroll');
    if (container) {
      const scrollAmount = 300;
      const newPosition = direction === 'left'
        ? recentScrollPosition - scrollAmount
        : recentScrollPosition + scrollAmount;
      container.scrollTo({ left: newPosition, behavior: 'smooth' });
      setRecentScrollPosition(newPosition);
    }
  };

  return (
    <>
      {/* Focus Filter Panel */}
      <FocusFilter />

      <main className="flex-1 overflow-y-auto p-8">
        {/* Recently Added Section */}
        <div className="mb-12">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-2xl text-white">最近游戏</h2>
            <div className="flex gap-2">
              <button
                onClick={() => scrollRecentGames('left')}
                className="p-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors"
                aria-label="Scroll left"
              >
                <ChevronLeft size={20} />
              </button>
              <button
                onClick={() => scrollRecentGames('right')}
                className="p-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors"
                aria-label="Scroll right"
              >
                <ChevronRight size={20} />
              </button>
            </div>
          </div>

        <div
          id="recent-games-scroll"
          className="flex gap-6 overflow-x-auto pb-4 scrollbar-hide snap-x snap-mandatory"
          style={{ scrollbarWidth: 'none', msOverflowStyle: 'none' }}
        >
          {recentlyAdded.map((asset) => (
            <div
              key={asset.id}
              onClick={() => onAssetClick(asset.id)}
              className="flex-shrink-0 w-56 snap-start"
            >
              <AssetCard {...asset} />
            </div>
          ))}
        </div>
      </div>

      {/* My Collection Section */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-2xl text-white">我的收藏</h2>

          <div className="flex items-center gap-4">
            {/* Tag Filter */}
            <div className="flex gap-2">
              {['所有游戏', '视觉小说', 'RPG', '动作'].map((tag) => (
                <button
                  key={tag}
                  onClick={() => setSelectedTag(tag)}
                  className={`px-4 py-2 rounded-lg text-sm transition-colors ${
                    selectedTag === tag
                      ? 'bg-blue-600 text-white'
                      : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200'
                  }`}
                >
                  {tag}
                </button>
              ))}
            </div>

            {/* Focus Filter Button */}
            <button
              onClick={() => setFilterPanelOpen(true)}
              className="flex items-center gap-2 px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-100 rounded-lg transition-colors"
              title="Open Focus Filter"
            >
              <Filter size={18} />
              <span>Focus</span>
            </button>

            {/* Sort Dropdown */}
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as SortOption)}
              className="px-4 py-2 bg-zinc-800 text-zinc-300 rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="最近添加">排序依据: 最近添加</option>
              <option value="名称">排序依据: 名称</option>
              <option value="发行日期">排序依据: 发行日期</option>
              <option value="评分">排序依据: 评分</option>
            </select>
          </div>
        </div>

        {/* Phase 22.0: Loading State */}
        {isLoading ? (
          <div className="flex items-center justify-center py-12">
            <div className="text-zinc-400">加载中...</div>
          </div>
        ) : (
          <>
            {/* Games Grid */}
            <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6">
              {myCollection.map((asset) => (
                <div key={asset.id} onClick={() => onAssetClick(asset.id)}>
                  <AssetCard {...asset} />
                </div>
              ))}
            </div>

            {/* Phase 22.0: Pagination Controls */}
            {totalPages > 1 && (
              <div className="flex items-center justify-center gap-4 mt-8">
                <button
                  onClick={() => goToPage(Math.max(0, currentPage - 1))}
                  disabled={currentPage === 0}
                  className="p-2 bg-zinc-800 hover:bg-zinc-700 disabled:bg-zinc-900 disabled:text-zinc-600 text-zinc-300 rounded-lg transition-colors"
                  aria-label="Previous page"
                >
                  <PageLeft size={20} />
                </button>

                <span className="text-zinc-400">
                  第 {currentPage + 1} / {totalPages} 页
                </span>

                <button
                  onClick={() => goToPage(Math.min(totalPages - 1, currentPage + 1))}
                  disabled={currentPage >= totalPages - 1}
                  className="p-2 bg-zinc-800 hover:bg-zinc-700 disabled:bg-zinc-900 disabled:text-zinc-600 text-zinc-300 rounded-lg transition-colors"
                  aria-label="Next page"
                >
                  <PageRight size={20} />
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </main>
    </>
  );
};

export default LibraryView;
