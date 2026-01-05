import { ArrowLeft, ArrowRight, Search, ListFilter, Wrench } from 'lucide-react';

export interface NavigationBarProps {
  canGoBack: boolean;
  canGoForward: boolean;
  onBack: () => void;
  onForward: () => void;
  currentPage: string;
  // Global functionality props
  totalAssets?: number;
  searchQuery?: string;
  onSearchChange?: (query: string) => void;
  onFocusClick?: () => void;
  onWorkbenchClick?: () => void;
  showLibraryTools?: boolean;
}

export function NavigationBar({
  canGoBack,
  canGoForward,
  onBack,
  onForward,
  currentPage,
  totalAssets,
  searchQuery = '',
  onSearchChange,
  onFocusClick,
  onWorkbenchClick,
  showLibraryTools = false,
}: NavigationBarProps) {
  return (
    <div className="flex items-center justify-between px-8 py-4 bg-zinc-900/50 border-b border-zinc-800">
      {/* Left: Navigation Controls */}
      <div className="flex items-center gap-3">
        {/* Back Button */}
        <button
          onClick={onBack}
          disabled={!canGoBack}
          className={`p-2 rounded-lg transition-all ${
            canGoBack
              ? 'bg-zinc-800 hover:bg-zinc-700 text-zinc-200 cursor-pointer'
              : 'bg-zinc-900 text-zinc-600 cursor-not-allowed'
          }`}
          title="Go back"
        >
          <ArrowLeft size={20} />
        </button>

        {/* Forward Button */}
        <button
          onClick={onForward}
          disabled={!canGoForward}
          className={`p-2 rounded-lg transition-all ${
            canGoForward
              ? 'bg-zinc-800 hover:bg-zinc-700 text-zinc-200 cursor-pointer'
              : 'bg-zinc-900 text-zinc-600 cursor-not-allowed'
          }`}
          title="Go forward"
        >
          <ArrowRight size={20} />
        </button>

        {/* Current Page Breadcrumb */}
        <div className="ml-4 text-sm text-zinc-400 border-l border-zinc-800 pl-4">
          {currentPage}
        </div>

        {/* Total Assets Count - Always visible */}
        {totalAssets !== undefined && (
          <div className="ml-6 text-sm text-zinc-400 border-l border-zinc-800 pl-6">
            Total Assets: <span className="text-white">{totalAssets.toLocaleString()}</span>
          </div>
        )}
      </div>

      {/* Right: Global Tools - Always visible */}
      <div className="flex items-center gap-4">
        {/* Workbench Button */}
        {onWorkbenchClick && (
          <button
            onClick={onWorkbenchClick}
            className="flex items-center gap-2 px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-100 rounded-lg transition-colors"
          >
            <Wrench size={18} />
            <span>Workbench</span>
          </button>
        )}

        {/* Focus Filter Button */}
        {showLibraryTools && onFocusClick && (
          <button
            onClick={onFocusClick}
            className="flex items-center gap-2 px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-100 rounded-lg transition-colors"
          >
            <ListFilter size={18} />
            <span>Focus</span>
          </button>
        )}

        {/* Search Input */}
        {onSearchChange && (
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" size={18} />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              placeholder="Search assets..."
              className="w-64 pl-10 pr-4 py-2 bg-zinc-800 text-zinc-100 placeholder-zinc-500 rounded-lg border border-zinc-700 focus:border-zinc-600 focus:outline-none focus:ring-2 focus:ring-zinc-600/50 transition-all"
            />
          </div>
        )}
      </div>
    </div>
  );
}
