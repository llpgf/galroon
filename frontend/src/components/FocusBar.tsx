import { ChevronDown, ChevronUp, Heart, Tag as TagIcon, X, Plus, Minus } from 'lucide-react';
import { useState } from 'react';
import { TagSelector } from './TagSelector';

export type TagState = 'include' | 'exclude';

export interface FocusTag {
  name: string;
  state: TagState;
}

interface FocusBarProps {
  tags: FocusTag[];
  onAddTag: (tagName: string) => void;
  onRemoveTag: (tagName: string) => void;
  onToggleTagState: (tagName: string) => void;
  availableTags: string[];
}

export function FocusBar({ tags, onAddTag, onRemoveTag, onToggleTagState, availableTags }: FocusBarProps) {
  const [focusExpanded, setFocusExpanded] = useState(false);
  const [showTagSelector, setShowTagSelector] = useState(false);

  const handleAddFavorite = () => {
    if (!tags.find(t => t.name === '我的最愛')) {
      onAddTag('我的最愛');
    }
  };

  const handleTagsSelected = (selectedTags: string[]) => {
    // Add all selected tags that aren't already in the focus bar
    selectedTags.forEach(tagName => {
      if (!tags.find(t => t.name === tagName)) {
        onAddTag(tagName);
      }
    });
  };

  return (
    <div className="border-b border-[#2a2a2a]">
      {/* Focus Bar */}
      <div className="px-12 py-4 flex items-center gap-3">
        {/* Focus Toggle Button */}
        <button
          onClick={() => setFocusExpanded(!focusExpanded)}
          className="flex items-center gap-2 text-[#b3b3b3] hover:text-white transition-colors"
        >
          {focusExpanded ? (
            <ChevronUp className="w-4 h-4" strokeWidth={2} />
          ) : (
            <ChevronDown className="w-4 h-4" strokeWidth={2} />
          )}
          <span className="text-sm">聚焦</span>
        </button>

        {/* Add Favorite Tag */}
        <button
          onClick={handleAddFavorite}
          className="w-9 h-9 flex items-center justify-center rounded-full bg-[#2a2a2a] text-[#b3b3b3] hover:text-white hover:bg-[#3a3a3a] transition-colors"
          title="添加我的最愛"
        >
          <Heart className="w-4 h-4" strokeWidth={2} />
        </button>

        {/* Open Tag Selector */}
        <button
          onClick={() => setShowTagSelector(true)}
          className="w-9 h-9 flex items-center justify-center rounded-full bg-[#2a2a2a] text-[#b3b3b3] hover:text-white hover:bg-[#3a3a3a] transition-colors"
          title="選擇標籤"
        >
          <TagIcon className="w-4 h-4" strokeWidth={2} />
        </button>

        {/* Active Tags */}
        <div className="flex items-center gap-2 flex-wrap">
          {tags.map((tag) => (
            <div
              key={tag.name}
              className={`flex items-center gap-1.5 px-4 py-1.5 rounded-full text-sm transition-all ${
                tag.state === 'include'
                  ? 'bg-[#6366f1] text-white'
                  : 'bg-red-500 text-white'
              }`}
            >
              {/* Toggle State Button */}
              <button
                onClick={() => onToggleTagState(tag.name)}
                className="hover:opacity-80 transition-opacity"
              >
                {tag.state === 'include' ? (
                  <Plus className="w-3 h-3" strokeWidth={3} />
                ) : (
                  <Minus className="w-3 h-3" strokeWidth={3} />
                )}
              </button>

              <span>{tag.name}</span>

              {/* Remove Tag Button */}
              <button
                onClick={() => onRemoveTag(tag.name)}
                className="hover:opacity-80 transition-opacity"
              >
                <X className="w-3 h-3" strokeWidth={3} />
              </button>
            </div>
          ))}
        </div>

        {/* Advanced Filters Link */}
        {tags.length > 0 && (
          <button className="ml-auto text-[#6366f1] hover:text-[#5558e3] text-sm transition-colors">
            依篩選智慧地改進庫
          </button>
        )}
      </div>

      {/* Expanded Focus Panel */}
      {focusExpanded && (
        <div className="px-12 py-6 bg-[#1a1a1a] border-t border-[#2a2a2a]">
          <div className="grid grid-cols-5 gap-8">
            {/* Genre Filter */}
            <div>
              <h3 className="text-white text-sm mb-3">音樂類型</h3>
              <div className="space-y-2">
                <button key="genre-pop" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  流行 (715)</button>
                <button key="genre-rock" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  搖滾 (411)</button>
                <button key="genre-jazz" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  爵士 (277)</button>
                <button key="genre-more" className="text-[#6366f1] hover:text-[#5558e3] text-sm text-left transition-colors">
                  顯示更多</button>
              </div>
            </div>

            {/* Release Year */}
            <div>
              <h3 className="text-white text-sm mb-3">發行日期</h3>
              <div className="space-y-2">
                <button key="year-1900s" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  1900s</button>
                <button key="year-1910s" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  1910s</button>
                <button key="year-1920s" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  1920s</button>
                <button key="year-more" className="text-[#6366f1] hover:text-[#5558e3] text-sm text-left transition-colors">
                  顯示更多</button>
              </div>
            </div>

            {/* Format */}
            <div>
              <h3 className="text-white text-sm mb-3">演出類型</h3>
              <div className="space-y-2">
                <button key="format-solo" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  獨唱者 (221)</button>
                <button key="format-ending" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  片尾曲 (79)</button>
                <button key="format-koi" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  KOI-KOI (64)</button>
                <button key="format-more" className="text-[#6366f1] hover:text-[#5558e3] text-sm text-left transition-colors">
                  顯示更多</button>
              </div>
            </div>

            {/* Artist */}
            <div>
              <h3 className="text-white text-sm mb-3">作曲家</h3>
              <div className="space-y-2">
                <button key="artist-sakimoto" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  崎元仁 (111)</button>
                <button key="artist-mitsuda" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  光田康典 (64)</button>
                <button key="artist-kajiura" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  Yuki Kajiura (62)</button>
                <button key="artist-more" className="text-[#6366f1] hover:text-[#5558e3] text-sm text-left transition-colors">
                  顯示更多</button>
              </div>
            </div>

            {/* Quality */}
            <div>
              <h3 className="text-white text-sm mb-3">製作</h3>
              <div className="space-y-2">
                <button key="prod-koiwa" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  Takashi Koiwa (71)</button>
                <button key="prod-azuma" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  Mitsunori Azuma (61)</button>
                <button key="prod-kanna" className="text-[#9ca3af] hover:text-white text-sm text-left transition-colors">
                  漢那直之 (56)</button>
                <button key="prod-more" className="text-[#6366f1] hover:text-[#5558e3] text-sm text-left transition-colors">
                  顯示更多</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tag Selector Modal */}
      {showTagSelector && (
        <TagSelector
          availableTags={availableTags}
          selectedTags={tags.map(t => t.name)}
          onConfirm={handleTagsSelected}
          onClose={() => setShowTagSelector(false)}
        />
      )}
    </div>
  );
}