import { X, Search } from 'lucide-react';
import { useState } from 'react';

interface TagSelectorProps {
  availableTags: string[];
  selectedTags: string[];
  onConfirm: (tags: string[]) => void;
  onClose: () => void;
}

export function TagSelector({ availableTags, selectedTags, onConfirm, onClose }: TagSelectorProps) {
  const [localSelected, setLocalSelected] = useState<string[]>(selectedTags);
  const [searchQuery, setSearchQuery] = useState('');

  const toggleTag = (tag: string) => {
    if (localSelected.includes(tag)) {
      setLocalSelected(localSelected.filter(t => t !== tag));
    } else {
      setLocalSelected([...localSelected, tag]);
    }
  };

  const handleConfirm = () => {
    onConfirm(localSelected);
    onClose();
  };

  const filteredTags = availableTags.filter(tag =>
    tag.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <>
      {/* Backdrop */}
      <div 
        className="fixed inset-0 bg-black/60 backdrop-blur-sm z-40"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[500px] max-h-[600px] bg-[#1e1e1e] border border-[#3a3a3a] rounded-lg shadow-2xl z-50 flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-[#2a2a2a]">
          <h2 className="text-white text-lg">選擇標籤</h2>
          <button
            onClick={onClose}
            className="text-[#6b6b6b] hover:text-white transition-colors"
          >
            <X className="w-5 h-5" strokeWidth={2} />
          </button>
        </div>

        {/* Search Bar */}
        <div className="p-6 border-b border-[#2a2a2a]">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[#6b6b6b]" strokeWidth={2} />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜尋標籤..."
              className="w-full pl-10 pr-4 py-2 bg-[#121212] border border-[#3a3a3a] rounded-lg text-white placeholder-[#6b6b6b] focus:outline-none focus:border-[#6366f1] transition-colors"
            />
          </div>
        </div>

        {/* Tags List */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="flex flex-wrap gap-2">
            {filteredTags.map((tag) => {
              const isSelected = localSelected.includes(tag);
              return (
                <button
                  key={tag}
                  onClick={() => toggleTag(tag)}
                  className={`px-4 py-2 rounded-full text-sm transition-all ${
                    isSelected
                      ? 'bg-[#6366f1] text-white border border-[#6366f1]'
                      : 'bg-[#2a2a2a] text-[#b3b3b3] border border-[#3a3a3a] hover:border-[#4a4a4a] hover:text-white'
                  }`}
                >
                  {tag}
                </button>
              );
            })}
          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-[#2a2a2a]">
          <button
            onClick={handleConfirm}
            className="w-full px-6 py-3 bg-[#6366f1] text-white rounded-lg hover:bg-[#5558e3] transition-colors"
          >
            完成
          </button>
        </div>
      </div>
    </>
  );
}
