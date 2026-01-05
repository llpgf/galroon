import { Library, Trash, Settings, ChevronsLeft, ChevronsRight, Home } from 'lucide-react';
import { useState } from 'react';

interface SidebarProps {
  activeItem: string;
  onItemClick: (item: string) => void;
}

/**
 * Sidebar - App Shell Sidebar (Figma Design)
 *
 * Features:
 * - Lucide React icons (not emoji)
 * - Collapsible/Expandable (80px / 192px)
 * - Tooltips when collapsed
 * - Active indicator (blue vertical bar)
 * - Home button at top
 */
export const Sidebar = ({ activeItem, onItemClick }: SidebarProps) => {
  const [isExpanded, setIsExpanded] = useState(false);

  const navItems = [
    { id: 'library', icon: Library, label: 'Library' },
    { id: 'trash', icon: Trash, label: 'Trash' },
    { id: 'settings', icon: Settings, label: 'Settings' },
  ];

  return (
    <aside
      className={`bg-zinc-950 border-r border-zinc-800 flex flex-col py-6 transition-all duration-300 ${
        isExpanded ? 'w-48' : 'w-20'
      }`}
    >
      {/* Home Button at Top */}
      <div className="mb-4 pb-4 border-b border-zinc-800">
        <button
          onClick={() => onItemClick('library')}
          className={`group relative h-14 w-full flex items-center transition-colors ${
            isExpanded ? 'px-6' : 'justify-center'
          } text-zinc-500 hover:text-white`}
          aria-label="Home"
        >
          <Home size={28} />

          {/* Tooltip on hover when collapsed */}
          {!isExpanded && (
            <div className="absolute left-full ml-2 px-3 py-1.5 bg-zinc-800 text-white text-sm rounded opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity whitespace-nowrap shadow-lg z-50">
              Home
            </div>
          )}

          {/* Label when expanded */}
          {isExpanded && (
            <span className="ml-3">Home</span>
          )}
        </button>
      </div>

      <nav className="flex flex-col gap-2 flex-1">
        {navItems.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              onClick={() => onItemClick(item.id)}
              className={`group relative h-14 flex items-center transition-colors ${
                isExpanded ? 'px-6' : 'justify-center'
              } ${
                activeItem === item.id
                  ? 'text-white'
                  : 'text-zinc-500 hover:text-zinc-300'
              }`}
              aria-label={item.label}
            >
              {activeItem === item.id && (
                <div className="absolute left-0 top-0 bottom-0 w-1 bg-blue-500 rounded-r" />
              )}
              <Icon size={24} />

              {/* Tooltip on hover when collapsed */}
              {!isExpanded && (
                <div className="absolute left-full ml-2 px-3 py-1.5 bg-zinc-800 text-white text-sm rounded opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity whitespace-nowrap shadow-lg z-50">
                  {item.label}
                </div>
              )}

              {/* Label when expanded */}
              {isExpanded && (
                <span className="ml-3 text-sm">{item.label}</span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Collapse/Expand Toggle */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="h-12 flex items-center justify-center text-zinc-500 hover:text-zinc-300 transition-colors border-t border-zinc-800"
        aria-label={isExpanded ? 'Collapse sidebar' : 'Expand sidebar'}
      >
        {isExpanded ? <ChevronsLeft size={20} /> : <ChevronsRight size={20} />}
      </button>
    </aside>
  );
};

/**
 * Sidebar Styles
 * (Using Tailwind instead of inline styles)
 */
export const sidebarStyles = ``;
