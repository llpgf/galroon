import { useState } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { NavigationBar } from './components/layout/NavigationBar';
import { AssetCard } from './components/library/AssetCard';
import { ErrorBoundary } from './components/ErrorBoundary';
import { EmptyLibraryState } from './components/library/EmptyLibraryState';
import { NoResultsState } from './components/library/NoResultsState';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { SettingsView } from './views/SettingsView';
import { TrashView } from './views/TrashView';

// Types
interface Asset {
  id: string;
  title: string;
  developer: string;
  coverImage: string;
  badges?: ('ISO' | 'DLC' | 'Patch')[];
}

interface NavigationHistoryItem {
  type: 'library' | 'trash' | 'settings' | 'gameDetails' | 'creatorProfile' | 'actorProfile';
  data?: {
    gameId?: string;
    actorName?: string;
    scrollPosition?: number;
  };
}

// Mock Data
const mockAssets: Asset[] = [
  {
    id: '1',
    title: 'Cyberpunk Chronicles',
    developer: 'Neon Studios',
    coverImage: 'https://images.unsplash.com/photo-1646900614911-378fd0c1d86d?w=400&q=80',
    badges: ['ISO', 'DLC'],
  },
  {
    id: '2',
    title: 'Midnight Runner',
    developer: 'Dark Horse Interactive',
    coverImage: 'https://images.unsplash.com/photo-1746107690247-2342dc489c2f?w=400&q=80',
    badges: ['Patch'],
  },
  {
    id: '3',
    title: 'Stellar Odyssey',
    developer: 'Cosmic Games',
    coverImage: 'https://images.unsplash.com/photo-1711054824441-064a99073a0b?w=400&q=80',
    badges: ['ISO'],
  },
  {
    id: '4',
    title: 'Digital Dreams',
    developer: 'Pixel Perfect',
    coverImage: 'https://images.unsplash.com/photo-1648555412975-cfe0576b2f77?w=400&q=80',
    badges: ['DLC'],
  },
  {
    id: '5',
    title: 'Abstract Realms',
    developer: 'Avant-Garde Games',
    coverImage: 'https://images.unsplash.com/photo-1706189797798-30d44496b274?w=400&q=80',
  },
  {
    id: '6',
    title: 'The Last Chapter',
    developer: 'Story Forge',
    coverImage: 'https://images.unsplash.com/photo-1487147264018-f937fba0c817?w=400&q=80',
    badges: ['ISO', 'Patch'],
  },
  {
    id: '7',
    title: 'Neon Warfare',
    developer: 'Tactical Games Inc',
    coverImage: 'https://images.unsplash.com/photo-1646900614911-378fd0c1d86d?w=400&q=80',
    badges: ['DLC'],
  },
  {
    id: '8',
    title: 'Quantum Legacy',
    developer: 'Future Games',
    coverImage: 'https://images.unsplash.com/photo-1746107690247-2342dc489c2f?w=400&q=80',
  },
];

export default function App() {
  const [activeNav, setActiveNav] = useState('library');
  const [searchQuery, setSearchQuery] = useState('');
  const [sortBy, setSortBy] = useState('最近添加');
  const [selectedTag, setSelectedTag] = useState('所有游戏');
  const [isWorkbenchOpen, setIsWorkbenchOpen] = useState(false);
  const [isFocusOpen, setIsFocusOpen] = useState(false);

  // Navigation History System
  const [navigationHistory, setNavigationHistory] = useState<NavigationHistoryItem[]>([
    { type: 'library' }
  ]);
  const [currentHistoryIndex, setCurrentHistoryIndex] = useState(0);

  const currentPage = navigationHistory[currentHistoryIndex];

  // Recently added games (first 6)
  const recentlyAdded = mockAssets.slice(0, 6);

  const filteredAssets = mockAssets.filter((asset) =>
    asset.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    asset.developer.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Navigation functions
  const handleSidebarNavClick = (navItem: string) => {
    setActiveNav(navItem);

    // Add to navigation history
    const newHistory = navigationHistory.slice(0, currentHistoryIndex + 1);
    newHistory.push({ type: navItem as NavigationHistoryItem['type'] });
    setNavigationHistory(newHistory);
    setCurrentHistoryIndex(newHistory.length - 1);
  };

  const handleAssetClick = (assetId: string) => {
    const newHistory = navigationHistory.slice(0, currentHistoryIndex + 1);
    newHistory.push({ type: 'gameDetails', data: { gameId: assetId } });
    setNavigationHistory(newHistory);
    setCurrentHistoryIndex(newHistory.length - 1);
    setActiveNav('library');
  };

  const navigateBack = () => {
    if (currentHistoryIndex > 0) {
      setCurrentHistoryIndex(currentHistoryIndex - 1);
      setActiveNav(navigationHistory[currentHistoryIndex - 1].type);
    }
  };

  const navigateForward = () => {
    if (currentHistoryIndex < navigationHistory.length - 1) {
      setCurrentHistoryIndex(currentHistoryIndex + 1);
      setActiveNav(navigationHistory[currentHistoryIndex + 1].type);
    }
  };

  const getCurrentPageName = (): string => {
    switch (currentPage.type) {
      case 'library':
        return 'Library';
      case 'trash':
        return 'Trash';
      case 'settings':
        return 'Settings';
      default:
        return 'Library';
    }
  };

  const scrollRecentGames = (direction: 'left' | 'right') => {
    const container = document.getElementById('recent-games-scroll');
    if (container) {
      const scrollAmount = 300;
      container.scrollBy({
        left: direction === 'left' ? -scrollAmount : scrollAmount,
        behavior: 'smooth'
      });
    }
  };

  const clearFilters = () => {
    setSearchQuery('');
    setSelectedTag('所有游戏');
  };

  // Render Library Page
  const renderLibraryPage = () => {
    // Phase 19: Empty state for completely empty library
    if (mockAssets.length === 0) {
      return <EmptyLibraryState />;
    }

    return (
      <>
        {/* Recently Added Section */}
        <div className="mb-12">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-2xl text-white">最近游戏</h2>
            <div className="flex gap-2">
              <button
                onClick={() => scrollRecentGames('left')}
                className="p-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors"
              >
                <ChevronLeft size={20} />
              </button>
              <button
                onClick={() => scrollRecentGames('right')}
                className="p-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors"
              >
                <ChevronRight size={20} />
              </button>
            </div>
          </div>

          <div
            id="recent-games-scroll"
            className="flex gap-6 overflow-x-auto pb-4"
            style={{ scrollbarWidth: 'none', msOverflowStyle: 'none' }}
          >
            {recentlyAdded.map((asset) => (
              <div
                key={asset.id}
                className="flex-shrink-0 w-56"
                onClick={() => handleAssetClick(asset.id)}
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

              {/* Sort Dropdown */}
              <select
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value)}
                className="px-4 py-2 bg-zinc-800 text-zinc-300 rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="最近添加">排序依据: 最近添加</option>
                <option value="名称">排序依据: 名称</option>
                <option value="发行日期">排序依据: 发行日期</option>
                <option value="评分">排序依据: 评分</option>
              </select>
            </div>
          </div>

          {/* Phase 19: Empty state for no filter results */}
          {filteredAssets.length === 0 ? (
            <NoResultsState
              searchQuery={searchQuery}
              selectedTag={selectedTag}
              onClearFilters={clearFilters}
            />
          ) : (
            <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6">
              {filteredAssets.map((asset) => (
                <div key={asset.id} onClick={() => handleAssetClick(asset.id)}>
                  <AssetCard {...asset} />
                </div>
              ))}
            </div>
          )}
        </div>
      </>
    );
  };

  // Render current page content
  const renderPageContent = () => {
    switch (currentPage.type) {
      case 'library':
        return (
          <ErrorBoundary>
            {renderLibraryPage()}
          </ErrorBoundary>
        );

      case 'gameDetails':
        return (
          <ErrorBoundary>
            <div className="flex items-center justify-center h-full">
              <div className="text-center">
                <div className="text-6xl mb-4">🎮</div>
                <h2 className="text-2xl font-semibold text-white mb-2">
                  Game Details
                </h2>
                <p className="text-zinc-400">Game ID: {currentPage.data?.gameId}</p>
                <button
                  onClick={() => navigateBack()}
                  className="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
                >
                  Back to Library
                </button>
              </div>
            </div>
          </ErrorBoundary>
        );

      case 'trash':
        return <TrashView />;

      case 'settings':
        return <SettingsView onBack={() => setActiveNav('library')} />;

      default:
        return renderLibraryPage();
    }
  };

  return (
    <div className="flex h-screen bg-zinc-900">
      {/* Sidebar */}
      <Sidebar activeItem={activeNav} onItemClick={handleSidebarNavClick} />

      {/* Right Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Navigation Bar */}
        <NavigationBar
          canGoBack={currentHistoryIndex > 0}
          canGoForward={currentHistoryIndex < navigationHistory.length - 1}
          onBack={navigateBack}
          onForward={navigateForward}
          currentPage={getCurrentPageName()}
          totalAssets={1240}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          onFocusClick={() => setIsFocusOpen(!isFocusOpen)}
          onWorkbenchClick={() => setIsWorkbenchOpen(!isWorkbenchOpen)}
          showLibraryTools={currentPage.type === 'library'}
        />

        {/* Page Content */}
        <main className="flex-1 overflow-y-auto p-8">
          {renderPageContent()}
        </main>
      </div>
    </div>
  );
}
