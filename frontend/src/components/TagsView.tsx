import { useTranslation } from 'react-i18next';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { useState } from 'react';

export interface TagWithGames {
  id: string;
  name: string;
  gameCount: number;
  coverImages: string[]; // All game covers with this tag
}

interface TagsViewProps {
  tags: TagWithGames[];
  onSelectTag: (id: string) => void;
}

export function TagsView({ tags, onSelectTag }: TagsViewProps) {
  const { t } = useTranslation();
  const [expandedTag, setExpandedTag] = useState<string | null>(null);

  const toggleTag = (tagId: string) => {
    setExpandedTag(expandedTag === tagId ? null : tagId);
  };

  return (
    <div className="min-h-screen">
      <div className="px-12 py-12">
        {/* Header */}
        <header className="mb-12">
          <h1 className="text-white tracking-tight mb-2">{t('filter.tags')}</h1>
          <p className="text-[#6b6b6b]">{t('library.itemCount', { count: tags.length })} {t('filter.tags').toLowerCase()}</p>
        </header>

        {/* Tags List */}
        <div className="space-y-8">
          {tags.map((tag) => {
            const isExpanded = expandedTag === tag.id;
            const displayCount = isExpanded ? tag.coverImages.length : Math.min(12, tag.coverImages.length);
            const hasMore = tag.coverImages.length > 12;

            return (
              <div key={tag.id} className="space-y-4">
                {/* Tag Header */}
                <div className="flex items-center justify-between">
                  <button
                    onClick={() => onSelectTag(tag.id)}
                    className="group flex items-center gap-3"
                  >
                    <h2 className="text-white text-xl group-hover:text-[#b3b3b3] transition-colors">
                      {tag.name}
                    </h2>
                    <span className="text-[#6b6b6b] text-sm">
                      {tag.gameCount} {t('creators.games')}
                    </span>
                  </button>

                  {hasMore && (
                    <button
                      onClick={() => toggleTag(tag.id)}
                      className="text-[#6366f1] hover:text-[#5558e3] text-sm transition-colors"
                    >
                      {isExpanded ? t('tags.collapse') : t('tags.showAll')}
                    </button>
                  )}
                </div>

                {/* Cover Wall - Masonry-like Grid */}
                <div className="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8 2xl:grid-cols-10 gap-4">
                  {tag.coverImages.slice(0, displayCount).map((cover, index) => (
                    <button
                      key={index}
                      onClick={() => onSelectTag(tag.id)}
                      className="group relative aspect-square rounded overflow-hidden bg-[#1e1e1e] border border-[#2a2a2a] hover:border-[#3a3a3a] transition-all"
                    >
                      <ImageWithFallback
                        src={cover}
                        alt={`${tag.name} ${index + 1}`}
                        className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
                      />

                      {/* Hover Overlay */}
                      <div className="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity" />
                    </button>
                  ))}
                </div>
              </div>
            );
          })}
        </div>

        {/* Empty State */}
        {tags.length === 0 && (
          <div className="flex items-center justify-center py-24">
            <div className="text-center">
              <p className="text-[#6b6b6b] text-lg">{t('creators.emptyTitle', { type: t('filter.tags') })}</p>
              <p className="text-[#4a4a4a] text-sm mt-2">
                {t('filter.addTag')}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
