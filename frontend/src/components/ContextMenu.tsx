import { useEffect, useRef } from 'react';

export interface ContextMenuItem {
  label: string;
  action: () => void;
  separator?: boolean;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  position: { x: number; y: number };
  onClose: () => void;
}

export function ContextMenu({ items, position, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="fixed z-50 min-w-[200px] rounded-lg overflow-hidden shadow-2xl
        bg-[#161616]/85 backdrop-blur-[20px] border border-[#2a2a2a]"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`
      }}
    >
      <div className="py-2">
        {items.map((item, index) => (
          <div key={index}>
            {item.separator && <div className="h-px bg-[#2a2a2a] my-2" />}
            <button
              onClick={() => {
                item.action();
                onClose();
              }}
              className="w-full px-4 py-3 text-left text-sm text-white hover:bg-[#2a2a2a] transition-colors"
            >
              {item.label}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
