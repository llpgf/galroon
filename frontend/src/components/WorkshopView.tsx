import { useState } from 'react';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { ContextMenu, ContextMenuItem } from './ContextMenu';

export interface WorkshopItem {
  id: string;
  title?: string;
  coverImage?: string;
  status: 'pending' | 'curated';
  tags?: string[];
}

interface WorkshopViewProps {
  items: WorkshopItem[];
  onMarkAsCurated: (ids: string[]) => void;
  onEditTags: (ids: string[]) => void;
  onBatchIdentify: (ids: string[]) => void;
}

export function WorkshopView({
  items,
  onMarkAsCurated,
  onEditTags,
  onBatchIdentify
}: WorkshopViewProps) {
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [contextMenu, setContextMenu] = useState<{ position: { x: number; y: number }; itemId: string } | null>(null);

  const pendingItems = items.filter(item => item.status === 'pending');

  const toggleSelection = (id: string, isMultiSelect: boolean = false) => {
    if (isMultiSelect) {
      setSelectedIds(prev =>
        prev.includes(id)
          ? prev.filter(i => i !== id)
          : [...prev, id]
      );
    } else {
      setSelectedIds([id]);
    }
  };

  const handleContextMenu = (e: React.MouseEvent, itemId: string) => {
    e.preventDefault();
    if (!selectedIds.includes(itemId)) {
      setSelectedIds([itemId]);
    }
    setContextMenu({
      position: { x: e.clientX, y: e.clientY },
      itemId
    });
  };

  const contextMenuItems: ContextMenuItem[] = [
    { label: 'Add to List', action: () => alert(`Add to list`) },
    { label: 'Edit Tags', action: () => onEditTags(selectedIds) },
    { label: 'Mark as Sensitive', action: () => alert(`Mark as sensitive`), separator: true },
    { label: 'Send to Workshop', action: () => alert(`Already in workshop`) }
  ];

  // Generate geometric gradient for missing covers
  const generateGradient = (id: string) => {
    const hue = parseInt(id.slice(-2), 36) % 360;
    return `linear-gradient(135deg, hsl(${hue}, 70%, 30%) 0%, hsl(${(hue + 60) % 360}, 70%, 20%) 100%)`;
  };

  return (
    <div className="min-h-screen bg-[#0A0A0A]">
      {/* Header */}
      <div className="sticky top-0 z-40 bg-[#0A0A0A]/80 backdrop-blur-xl border-b border-[#1a1a1a] px-12 py-6">
        <h1 className="text-white text-2xl font-light tracking-tight">
          Workshop: <span className="text-[#6b6b6b]">{pendingItems.length} Items Pending</span>
        </h1>
        {selectedIds.length > 0 && (
          <p className="text-[#6366f1] text-sm mt-2">
            {selectedIds.length} item{selectedIds.length > 1 ? 's' : ''} selected
          </p>
        )}
      </div>

      {/* High-Density Grid */}
      <div className="px-12 py-12">
        <div className="grid grid-cols-6 md:grid-cols-8 lg:grid-cols-10 xl:grid-cols-12 2xl:grid-cols-14 gap-4">
          {pendingItems.map((item) => {
            const isSelected = selectedIds.includes(item.id);

            return (
              <button
                key={item.id}
                onClick={(e) => toggleSelection(item.id, e.metaKey || e.ctrlKey || e.shiftKey)}
                onContextMenu={(e) => handleContextMenu(e, item.id)}
                className={`group relative aspect-square rounded-lg overflow-hidden transition-all ${
                  isSelected
                    ? 'ring-2 ring-[#6366f1] ring-offset-2 ring-offset-[#0A0A0A]'
                    : 'border border-[#2a2a2a]/50 hover:border-[#3a3a3a]'
                }`}
              >
                {/* Cover Image or Generative Art */}
                {item.coverImage ? (
                  <ImageWithFallback
                    src={item.coverImage}
                    alt={item.title || 'Untitled'}
                    className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                ) : (
                  <div
                    className="w-full h-full"
                    style={{ background: generateGradient(item.id) }}
                  >
                    {/* Geometric Pattern Overlay */}
                    <div className="w-full h-full opacity-20">
                      <svg className="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                        <pattern id={`pattern-${item.id}`} x="0" y="0" width="20" height="20" patternUnits="userSpaceOnUse">
                          <circle cx="10" cy="10" r="5" fill="white" opacity="0.3" />
                        </pattern>
                        <rect width="100" height="100" fill={`url(#pattern-${item.id})`} />
                      </svg>
                    </div>
                  </div>
                )}

                {/* Selection Indicator */}
                {isSelected && (
                  <div className="absolute top-2 right-2 w-6 h-6 rounded-full bg-[#6366f1] flex items-center justify-center">
                    <svg className="w-4 h-4 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                    </svg>
                  </div>
                )}

                {/* Title on Hover */}
                {item.title && (
                  <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/90 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-200 p-2">
                    <p className="text-white text-xs truncate">{item.title}</p>
                  </div>
                )}
              </button>
            );
          })}
        </div>

        {/* Empty State */}
        {pendingItems.length === 0 && (
          <div className="flex items-center justify-center py-24">
            <div className="text-center">
              <p className="text-[#6b6b6b] text-lg">Workshop is empty</p>
              <p className="text-[#4a4a4a] text-sm mt-2">All items have been curated</p>
            </div>
          </div>
        )}
      </div>

      {/* Bottom Action Bar - Floating Frosted Glass */}
      {selectedIds.length > 0 && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-50">
          <div className="bg-[#161616]/85 backdrop-blur-[20px] border border-[#2a2a2a] rounded-full shadow-2xl px-6 py-4 flex items-center gap-4">
            <button
              onClick={() => onMarkAsCurated(selectedIds)}
              className="px-6 py-2 bg-[#6366f1] hover:bg-[#5558e3] text-white rounded-full text-sm font-light transition-colors"
            >
              Mark as Curated (The Ritual)
            </button>

            <div className="w-px h-6 bg-[#2a2a2a]" />

            <button
              onClick={() => onEditTags(selectedIds)}
              className="px-6 py-2 text-white hover:bg-[#2a2a2a] rounded-full text-sm font-light transition-colors"
            >
              Edit Tags
            </button>

            <button
              onClick={() => onBatchIdentify(selectedIds)}
              className="px-6 py-2 text-white hover:bg-[#2a2a2a] rounded-full text-sm font-light transition-colors"
            >
              Batch Identity
            </button>

            <div className="w-px h-6 bg-[#2a2a2a]" />

            <button
              onClick={() => setSelectedIds([])}
              className="px-4 py-2 text-[#6b6b6b] hover:text-white text-sm font-light transition-colors"
            >
              Clear
            </button>
          </div>
        </div>
      )}

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
