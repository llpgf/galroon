/**
 * WorkshopPage - Main Workshop container using shadcn/ui
 * 
 * Design System:
 * - bg-neutral-950 (background)
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { useState, useMemo } from 'react';
import { TopTabs, WorkshopStatus } from '../components/workshop/TopTabs';
import { WorkshopFocusBar, FocusTag } from '../components/workshop/WorkshopFocusBar';
import { PosterWall } from '../components/workshop/PosterWall';
import { WorkshopItem } from '../components/workshop/PosterCard';
import { MetadataEditModal } from '../components/workshop/MetadataEditModal';

// Mock data for demo
const MOCK_ITEMS: WorkshopItem[] = [
      {
            id: '1',
            title: 'Summer Pockets',
            coverImage: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=300&h=400&fit=crop',
            status: 'pending',
            tags: ['Visual Novel', 'Romance'],
            memos: [
                  { id: 'm1', text: '需要翻譯 patch', color: 'yellow', isManual: false },
                  { id: 'm2', text: '記得備份存檔', color: 'red', isManual: true },
            ],
      },
      {
            id: '2',
            title: '穢翼のユースティア',
            coverImage: 'https://images.unsplash.com/photo-1534447677768-be436bb09401?w=300&h=400&fit=crop',
            status: 'working',
            tags: ['Visual Novel', 'Fantasy'],
            memos: [
                  { id: 'm3', text: '進度 40%', color: 'gray', isManual: true },
            ],
      },
      {
            id: '3',
            title: 'Fate/stay night',
            status: 'pending',
            tags: ['Visual Novel', 'Action'],
      },
      {
            id: '4',
            title: 'Clannad',
            coverImage: 'https://images.unsplash.com/photo-1516641051054-9df6a1aad654?w=300&h=400&fit=crop',
            status: 'paused',
            tags: ['Visual Novel', 'Drama'],
            memos: [
                  { id: 'm4', text: '等待新版 patch', color: 'yellow', isManual: false },
            ],
      },
      {
            id: '5',
            title: 'Steins;Gate',
            coverImage: 'https://images.unsplash.com/photo-1518770660439-4636190af475?w=300&h=400&fit=crop',
            status: 'pending',
            tags: ['Visual Novel', 'Sci-Fi'],
      },
      {
            id: '6',
            title: 'Muv-Luv Alternative',
            coverImage: 'https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=300&h=400&fit=crop',
            status: 'working',
            tags: ['Visual Novel', 'Mecha'],
            memos: [
                  { id: 'm5', text: '進度 75%', color: 'gray', isManual: true },
                  { id: 'm6', text: '需要確認結局', color: 'red', isManual: true },
            ],
      },
];

const AVAILABLE_TAGS = ['Visual Novel', 'Romance', 'Fantasy', 'Action', 'Drama', 'Sci-Fi', 'Mecha', 'Horror', 'Mystery'];

export function WorkshopPage() {
      const [items, setItems] = useState<WorkshopItem[]>(MOCK_ITEMS);
      const [activeTab, setActiveTab] = useState<WorkshopStatus>('pending');
      const [focusTags, setFocusTags] = useState<FocusTag[]>([]);
      const [searchQuery, setSearchQuery] = useState('');
      const [recentSearches, setRecentSearches] = useState<string[]>(['Summer', 'Fate']);
      const [viewingDistance, setViewingDistance] = useState(50);
      const [selectedItem, setSelectedItem] = useState<WorkshopItem | null>(null);
      const [isModalOpen, setIsModalOpen] = useState(false);

      const counts = useMemo(() => ({
            pending: items.filter(i => i.status === 'pending').length,
            working: items.filter(i => i.status === 'working').length,
            paused: items.filter(i => i.status === 'paused').length,
      }), [items]);

      const filteredItems = useMemo(() => {
            let result = items.filter(i => i.status === activeTab);

            if (focusTags.length > 0) {
                  const includeTags = focusTags.filter(t => t.state === 'include').map(t => t.name);
                  const excludeTags = focusTags.filter(t => t.state === 'exclude').map(t => t.name);

                  result = result.filter(item => {
                        if (item.isInGallery) return false;
                        const itemTags = item.tags || [];
                        const hasIncludes = includeTags.length === 0 || includeTags.some(t => itemTags.includes(t));
                        const hasNoExcludes = !excludeTags.some(t => itemTags.includes(t));
                        return hasIncludes && hasNoExcludes;
                  });
            }

            return result;
      }, [items, activeTab, focusTags]);

      const searchResults = useMemo(() => {
            if (!searchQuery.trim()) return [];
            const query = searchQuery.toLowerCase();
            return items.filter(item =>
                  item.title?.toLowerCase().includes(query) ||
                  item.tags?.some(t => t.toLowerCase().includes(query))
            );
      }, [items, searchQuery]);

      const handleAddTag = (name: string) => {
            if (!focusTags.find(t => t.name === name)) {
                  setFocusTags([...focusTags, { name, state: 'include' }]);
            }
      };

      const handleRemoveTag = (name: string) => {
            setFocusTags(focusTags.filter(t => t.name !== name));
      };

      const handleToggleTagState = (name: string) => {
            setFocusTags(focusTags.map(t =>
                  t.name === name ? { ...t, state: t.state === 'include' ? 'exclude' : 'include' } : t
            ));
      };

      const handleSearchChange = (query: string) => {
            setSearchQuery(query);
      };

      const handleSearchResultClick = (item: WorkshopItem) => {
            if (item.isInGallery) {
                  alert('Navigate to Gallery detail page');
            } else {
                  setSelectedItem(item);
                  setIsModalOpen(true);
            }
            if (searchQuery && !recentSearches.includes(searchQuery)) {
                  setRecentSearches([searchQuery, ...recentSearches.slice(0, 4)]);
            }
            setSearchQuery('');
      };

      const handlePosterClick = (item: WorkshopItem) => {
            setSelectedItem(item);
            setIsModalOpen(true);
      };

      const handlePlaceholderClick = () => {
            setSelectedItem(null);
            setIsModalOpen(true);
      };

      const handleSaveItem = (savedItem: WorkshopItem) => {
            setItems(prev => {
                  const exists = prev.find(i => i.id === savedItem.id);
                  if (exists) {
                        return prev.map(i => i.id === savedItem.id ? savedItem : i);
                  }
                  return [savedItem, ...prev];
            });
      };

      const handleDeleteItem = (id: string) => {
            setItems(prev => prev.filter(i => i.id !== id));
      };

      return (
            <div className="min-h-screen bg-neutral-950">
                  <TopTabs
                        activeTab={activeTab}
                        onTabChange={setActiveTab}
                        counts={counts}
                  />

                  <WorkshopFocusBar
                        tags={focusTags}
                        onAddTag={handleAddTag}
                        onRemoveTag={handleRemoveTag}
                        onToggleTagState={handleToggleTagState}
                        searchQuery={searchQuery}
                        onSearchChange={handleSearchChange}
                        searchResults={searchResults}
                        onSearchResultClick={handleSearchResultClick}
                        recentSearches={recentSearches}
                        availableTags={AVAILABLE_TAGS}
                  />

                  <PosterWall
                        items={filteredItems}
                        activeTab={activeTab}
                        viewingDistance={viewingDistance}
                        onViewingDistanceChange={setViewingDistance}
                        onPosterClick={handlePosterClick}
                        onPlaceholderClick={handlePlaceholderClick}
                        selectedItemId={selectedItem?.id}
                  />

                  <MetadataEditModal
                        item={selectedItem}
                        isOpen={isModalOpen}
                        onClose={() => {
                              setIsModalOpen(false);
                              setSelectedItem(null);
                        }}
                        onSave={handleSaveItem}
                        onDelete={handleDeleteItem}
                  />
            </div>
      );
}
