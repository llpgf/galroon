import { useState } from 'react';
import { HeroCarousel, HeroSlide } from './HeroCarousel';
import { ViewModeSelector, ViewingDistanceSlider, ViewMode } from './DisplayGrammarControls';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { ContextMenu, ContextMenuItem } from './ContextMenu';
import { FocusBar, FocusTag } from './FocusBar';

export interface GalleryItem {
  id: string;
  title: string;
  coverImage: string;
  tags?: string[];
}

interface GalleryViewProps {
  heroSlides: HeroSlide[];
  items: GalleryItem[];
  onItemClick: (id: string) => void;
}

export function GalleryView({ heroSlides, items, onItemClick }: GalleryViewProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [viewingDistance, setViewingDistance] = useState(50);
  const [focusTags, setFocusTags] = useState<FocusTag[]>([]);
  const [contextMenu, setContextMenu] = useState<{ position: { x: number; y: number }; itemId: string } | null>(null);

  // All available tags from items
  const availableTags = Array.from(new Set(items.flatMap(item => item.tags || []))).sort();

  // Calculate grid columns based on viewing distance
  const getGridColumns = () => {
    if (viewMode === 'strip') return 'grid-cols-12';
    if (viewMode === 'compact') return 'grid-cols-8';
    if (viewMode === 'detail') return 'grid-cols-4';
    
    // Grid mode - responsive based on viewing distance
    if (viewingDistance < 33) return 'grid-cols-8';
    if (viewingDistance < 66) return 'grid-cols-6';
    return 'grid-cols-4';
  };

  const handleContextMenu = (e: React.MouseEvent, itemId: string) => {
    e.preventDefault();
    setContextMenu({
      position: { x: e.clientX, y: e.clientY },
      itemId
    });
  };

  const contextMenuItems: ContextMenuItem[] = [
    { label: 'Add to List', action: () => alert(`Add ${contextMenu?.itemId} to list`) },
    { label: 'Edit Tags', action: () => alert(`Edit tags for ${contextMenu?.itemId}`) },
    { label: 'Mark as Sensitive', action: () => alert(`Mark ${contextMenu?.itemId} as sensitive`), separator: true },
    { label: 'Send to Workshop', action: () => alert(`Send ${contextMenu?.itemId} to workshop`) }
  ];

  const handleAddTag = (tagName: string) => {
    if (!focusTags.find(t => t.name === tagName)) {
      setFocusTags([...focusTags, { name: tagName, state: 'include' }]);
    }
  };

  const handleRemoveTag = (tagName: string) => {
    setFocusTags(focusTags.filter(t => t.name !== tagName));
  };

  const handleToggleTagState = (tagName: string) => {
    setFocusTags(focusTags.map(tag => 
      tag.name === tagName 
        ? { ...tag, state: tag.state === 'include' ? 'exclude' : 'include' }
        : tag
    ));
  };

  const filteredItems = focusTags.length > 0
    ? items.filter(item => {
        const includeTags = focusTags.filter(t => t.state === 'include');
        const excludeTags = focusTags.filter(t => t.state === 'exclude');
        
        // Must have all include tags
        const hasIncludes = includeTags.length === 0 || includeTags.every(t => item.tags?.includes(t.name));
        // Must not have any exclude tags
        const hasNoExcludes = !excludeTags.some(t => item.tags?.includes(t.name));
        
        return hasIncludes && hasNoExcludes;
      })
    : items;

  return (
    <div className="min-h-screen bg-[#0A0A0A]">
      {/* Hero Stage */}
      <HeroCarousel slides={heroSlides} />

      {/* Focus Bar - The Transition */}
      <div className="sticky top-0 z-40 bg-[#0A0A0A]/80 backdrop-blur-xl border-b border-[#1a1a1a]">
        <div className="flex items-center justify-between">
          {/* FocusBar Component (Left Side - Flexible) */}
          <div className="flex-1 min-w-0">
            <FocusBar
              tags={focusTags}
              onAddTag={handleAddTag}
              onRemoveTag={handleRemoveTag}
              onToggleTagState={handleToggleTagState}
              availableTags={availableTags}
            />
          </div>

          {/* Display Grammar Controller (Right Side - Stacked) */}
          <div className="flex flex-col items-end gap-3 shrink-0 px-12 py-4">
            {/* View Mode Icons */}
            <ViewModeSelector
              viewMode={viewMode}
              onViewModeChange={setViewMode}
            />
            
            {/* Viewing Distance Slider */}
            <ViewingDistanceSlider
              viewingDistance={viewingDistance}
              onViewingDistanceChange={setViewingDistance}
            />
          </div>
        </div>
      </div>

      {/* The Grid */}
      <div className="px-12 py-12">
        <div className={`grid ${getGridColumns()} gap-6 transition-all duration-300`}>
          {filteredItems.map((item) => (
            <button
              key={item.id}
              onClick={() => onItemClick(item.id)}
              onContextMenu={(e) => handleContextMenu(e, item.id)}
              className="group relative aspect-square rounded-lg overflow-hidden bg-[#161616] border border-[#2a2a2a]/50 hover:border-[#3a3a3a] transition-all"
            >
              {/* Poster Image */}
              <ImageWithFallback
                src={item.coverImage}
                alt={item.title}
                className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
              />

              {/* Hover Title Overlay */}
              <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/90 via-black/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300 p-4">
                <p className="text-white text-sm font-light">{item.title}</p>
              </div>
            </button>
          ))}
        </div>

        {/* Empty State */}
        {filteredItems.length === 0 && (
          <div className="flex items-center justify-center py-24">
            <p className="text-[#6b6b6b]">No items match the selected filters</p>
          </div>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <ContextMenu
          items={contextMenuItems}
          position={contextMenu.position}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}