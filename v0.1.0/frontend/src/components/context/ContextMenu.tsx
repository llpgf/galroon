import React, { useState, useEffect, useRef } from 'react';
import { FolderOpen, Copy, Settings, Package } from 'lucide-react';

/**
 * Context Menu - Right-click menu for utility actions
 *
 * Appears on right-click on cover art or background
 */

interface ContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  onClose: () => void;
  onReveal: () => void;
  onCopyPath: () => void;
  onExtract?: () => void;
  onManageVersions?: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  visible,
  x,
  y,
  onClose,
  onReveal,
  onCopyPath,
  onExtract,
  onManageVersions,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);

  /**
   * Close menu on click outside
   */
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    if (visible) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [visible, onClose]);

  /**
   * Handle menu item click
   */
  const handleItemClick = (action: () => void) => {
    action();
    onClose();
  };

  if (!visible) return null;

  return (
    <div
      ref={menuRef}
      className="fixed z-50 min-w-[200px] py-2 bg-zinc-800 rounded-lg shadow-2xl border border-zinc-700"
      style={{ left: `${x}px`, top: `${y}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      <button
        onClick={() => handleItemClick(onReveal)}
        className="w-full px-4 py-2 flex items-center gap-3 hover:bg-zinc-700 transition-colors text-left"
      >
        <FolderOpen size={18} className="text-zinc-400" />
        <span className="text-white">Reveal in Explorer</span>
      </button>

      <button
        onClick={() => handleItemClick(onCopyPath)}
        className="w-full px-4 py-2 flex items-center gap-3 hover:bg-zinc-700 transition-colors text-left"
      >
        <Copy size={18} className="text-zinc-400" />
        <span className="text-white">Copy Path</span>
      </button>

      {onExtract && (
        <button
          onClick={() => handleItemClick(onExtract)}
          className="w-full px-4 py-2 flex items-center gap-3 hover:bg-zinc-700 transition-colors text-left"
        >
          <Package size={18} className="text-zinc-400" />
          <span className="text-white">Extract...</span>
        </button>
      )}

      {onManageVersions && (
        <>
          <div className="my-2 border-t border-zinc-700" />
          <button
            onClick={() => handleItemClick(onManageVersions)}
            className="w-full px-4 py-2 flex items-center gap-3 hover:bg-zinc-700 transition-colors text-left"
          >
            <Settings size={18} className="text-zinc-400" />
            <span className="text-white">Manage Versions</span>
          </button>
        </>
      )}
    </div>
  );
};

/**
 * Context Menu Hook
 *
 * Easy way to add context menu to any component
 */
export const useContextMenu = () => {
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
  }>({
    visible: false,
    x: 0,
    y: 0,
  });

  const showContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    setContextMenu({
      visible: true,
      x: e.clientX,
      y: e.clientY,
    });
  };

  const hideContextMenu = () => {
    setContextMenu({ visible: false, x: 0, y: 0 });
  };

  return {
    contextMenu,
    showContextMenu,
    hideContextMenu,
  };
};

export default ContextMenu;
