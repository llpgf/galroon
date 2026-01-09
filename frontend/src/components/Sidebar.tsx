import { Library, Inbox, Tag, Gamepad2, Clock, User, FileText, Music, TrendingUp, ChevronRight, ChevronDown, Plus, MoreHorizontal, Settings, Palette, Image, Wrench, GripVertical, Trash2, Edit3, FolderPlus, ArrowUpDown, ZoomIn } from 'lucide-react';
import { useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import * as ContextMenu from '@radix-ui/react-context-menu';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';

interface SidebarProps {
  currentView: string;
  onNavigate: (view: string) => void;
  inboxCount: number;
}

interface DraggableItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  view: string;
}

export function Sidebar({ currentView, onNavigate, inboxCount }: SidebarProps) {
  const { t } = useTranslation();
  const [myLibraryExpanded, setMyLibraryExpanded] = useState(true);
  const [playlistsExpanded, setPlaylistsExpanded] = useState(true);

  // Drag-drop state for my collection
  const [collectionItems, setCollectionItems] = useState<DraggableItem[]>([
    { id: 'my-games', label: t('nav.games'), view: 'my-games' },
    { id: 'voice-actors', label: t('nav.voiceActors'), view: 'voice-actors' },
    { id: 'artists', label: t('nav.artists'), view: 'artists' },
    { id: 'writers', label: t('nav.writers'), view: 'writers' },
    { id: 'composers', label: t('nav.composers'), view: 'composers' },
    { id: 'series', label: t('nav.series'), view: 'series' },
  ]);

  // Drag-drop state for playlists
  const [playlistItems, setPlaylistItems] = useState<DraggableItem[]>([
    { id: 'playlist-favorites', label: t('nav.favorites'), view: 'playlist-favorites' },
    { id: 'playlist-playing', label: t('nav.playing'), view: 'playlist-playing' },
    { id: 'playlist-completed', label: t('nav.completed'), view: 'playlist-completed' },
  ]);

  const [draggedItem, setDraggedItem] = useState<string | null>(null);
  const [dragOverItem, setDragOverItem] = useState<string | null>(null);

  const handleDragStart = (id: string) => {
    setDraggedItem(id);
  };

  const handleDragOver = (e: React.DragEvent, id: string) => {
    e.preventDefault();
    setDragOverItem(id);
  };

  const handleDrop = (targetId: string, items: DraggableItem[], setItems: React.Dispatch<React.SetStateAction<DraggableItem[]>>) => {
    if (!draggedItem || draggedItem === targetId) return;

    const dragIndex = items.findIndex(i => i.id === draggedItem);
    const dropIndex = items.findIndex(i => i.id === targetId);

    const newItems = [...items];
    const [removed] = newItems.splice(dragIndex, 1);
    newItems.splice(dropIndex, 0, removed);

    setItems(newItems);
    setDraggedItem(null);
    setDragOverItem(null);
  };

  const handleDragEnd = () => {
    setDraggedItem(null);
    setDragOverItem(null);
  };

  const handleRenamePlaylist = (id: string) => {
    const newName = prompt(t('nav.renamePlaylist'));
    if (newName) {
      setPlaylistItems(items => items.map(item =>
        item.id === id ? { ...item, label: newName } : item
      ));
    }
  };

  const handleDeletePlaylist = (id: string) => {
    if (confirm(t('nav.confirmDelete'))) {
      setPlaylistItems(items => items.filter(item => item.id !== id));
    }
  };

  const getItemIcon = (id: string) => {
    const icons: Record<string, React.ReactNode> = {
      'my-games': <Gamepad2 className="w-4 h-4" strokeWidth={1.5} />,
      'voice-actors': <User className="w-4 h-4" strokeWidth={1.5} />,
      'artists': <Palette className="w-4 h-4" strokeWidth={1.5} />,
      'writers': <FileText className="w-4 h-4" strokeWidth={1.5} />,
      'composers': <Music className="w-4 h-4" strokeWidth={1.5} />,
      'series': <TrendingUp className="w-4 h-4" strokeWidth={1.5} />,
    };
    return icons[id] || null;
  };

  return (
    <aside className="w-60 bg-[var(--color-surface)] flex flex-col h-screen sticky top-0">
      {/* App Header - Galroon goes to Gallery */}
      <div className="p-6 pb-4 flex items-center justify-between">
        <button
          onClick={() => onNavigate('gallery')}
          className="text-[var(--color-text-strong)] text-2xl tracking-wide hover:text-[var(--color-text-secondary)] transition-colors"
        >
          galroon
        </button>
        <button className="p-2 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-strong)] hover:bg-[var(--color-surface-hover)] rounded transition-colors" onClick={() => onNavigate('settings')}>
          <Settings className="w-4 h-4" strokeWidth={1.5} />
        </button>
      </div>

      {/* Navigation Items */}
      <nav className="flex-1 overflow-y-auto">
        <div className="p-4 space-y-1">
          {/* Gallery */}
          <button
            onClick={() => onNavigate('gallery')}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === 'gallery' ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'}`}
          >
            <Image className="w-4 h-4" strokeWidth={1.5} />
            <span className="text-sm">{t('nav.gallery')}</span>
          </button>

          {/* Dashboard */}
          <button
            onClick={() => onNavigate('dashboard')}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === 'dashboard' ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'}`}
          >
            <TrendingUp className="w-4 h-4" strokeWidth={1.5} />
            <span className="text-sm">{t('nav.dashboard')}</span>
          </button>

          {/* Workshop */}
          <button
            onClick={() => onNavigate('workshop')}
            className={`group w-full flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === 'workshop' ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'}`}
          >
            <Wrench className="w-4 h-4" strokeWidth={1.5} />
            <span className="text-sm">{t('nav.workshop')}</span>
            {inboxCount > 0 && (
              <span className="ml-auto bg-[var(--color-accent-blue)] text-[var(--color-text-strong)] text-xs px-2 py-0.5 rounded-full opacity-0 group-hover:opacity-100 transition-opacity">
                {inboxCount}
              </span>
            )}
          </button>

          {/* Tags */}
          <button
            onClick={() => onNavigate('tags')}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === 'tags' ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'}`}
          >
            <Tag className="w-4 h-4" strokeWidth={1.5} />
            <span className="text-sm">{t('nav.tags')}</span>
          </button>

          {/* Game Types */}
          <button
            onClick={() => onNavigate('game-types')}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === 'game-types' ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'}`}
          >
            <Gamepad2 className="w-4 h-4" strokeWidth={1.5} />
            <span className="text-sm">{t('nav.gameTypes')}</span>
          </button>
        </div>

        {/* My Collection Library - Drag & Drop */}
        <div className="mt-6">
          <button
            onClick={() => setMyLibraryExpanded(!myLibraryExpanded)}
            className="w-full flex items-center gap-2 px-6 py-2 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors"
          >
            {myLibraryExpanded ? (
              <ChevronDown className="w-4 h-4" strokeWidth={1.5} />
            ) : (
              <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
            )}
            <span className="text-xs uppercase tracking-wider">{t('nav.myCollection')}</span>
          </button>

          {myLibraryExpanded && (
            <div className="mt-1 px-4 space-y-1">
              {collectionItems.map((item) => (
                <div
                  key={item.id}
                  draggable
                  onDragStart={() => handleDragStart(item.id)}
                  onDragOver={(e) => handleDragOver(e, item.id)}
                  onDrop={() => handleDrop(item.id, collectionItems, setCollectionItems)}
                  onDragEnd={handleDragEnd}
                  className={`group flex items-center ${dragOverItem === item.id ? 'border-t-2 border-[var(--color-accent-primary)]' : ''}`}
                >
                  <button
                    onClick={() => onNavigate(item.view)}
                    className={`flex-1 flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === item.view ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'} ${draggedItem === item.id ? 'opacity-50 cursor-move' : ''}`}
                  >
                    {getItemIcon(item.id)}
                    <span className="text-sm">{item.label}</span>
                  </button>
                  <GripVertical
                    className="w-4 h-4 text-[var(--color-text-tertiary)] opacity-0 group-hover:opacity-100 cursor-move transition-opacity"
                    strokeWidth={1.5}
                  />
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Playlists - Drag & Drop with Context Menu */}
        <div className="mt-6">
          <div className="flex items-center justify-between px-4 py-2">
            <button
              onClick={() => setPlaylistsExpanded(!playlistsExpanded)}
              className="flex items-center gap-2 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors"
            >
              {playlistsExpanded ? (
                <ChevronDown className="w-4 h-4" strokeWidth={1.5} />
              ) : (
                <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
              )}
              <span className="text-xs uppercase tracking-wider">{t('nav.playlists')}</span>
            </button>

            <div className="flex items-center gap-1">
              <button
                className="p-1 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] rounded transition-colors"
                onClick={() => {
                  const newName = prompt(t('nav.renamePlaylist'));
                  if (newName) {
                    setPlaylistItems([...playlistItems, {
                      id: `playlist-${Date.now()}`,
                      label: newName,
                      view: `playlist-${Date.now()}`
                    }]);
                  }
                }}
              >
                <Plus className="w-4 h-4" strokeWidth={1.5} />
              </button>

              <DropdownMenu.Root>
                <DropdownMenu.Trigger asChild>
                  <button className="p-1 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] rounded transition-colors">
                    <MoreHorizontal className="w-4 h-4" strokeWidth={1.5} />
                  </button>
                </DropdownMenu.Trigger>

                <DropdownMenu.Portal>
                  <DropdownMenu.Content
                    className="min-w-[180px] bg-[var(--color-surface-muted)] rounded-xl shadow-2xl shadow-black/60 p-1 z-50"
                    sideOffset={5}
                    align="end"
                  >
                    <DropdownMenu.Item
                      className="flex items-center gap-3 px-3 py-2.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-strong)] rounded-xl cursor-pointer outline-none"
                      onClick={() => {
                        const newName = prompt(t('nav.renamePlaylist'));
                        if (newName) {
                          setPlaylistItems([...playlistItems, {
                            id: `playlist-${Date.now()}`,
                            label: newName,
                            view: `playlist-${Date.now()}`
                          }]);
                        }
                      }}
                    >
                      <Plus className="w-4 h-4" />
                      新增清單
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                      className="flex items-center gap-3 px-3 py-2.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-strong)] rounded-xl cursor-pointer outline-none"
                      onClick={() => {
                        alert('新增文件夾功能即將推出');
                      }}
                    >
                      <FolderPlus className="w-4 h-4" />
                      新增文件夾
                    </DropdownMenu.Item>
                    <DropdownMenu.Separator className="h-px bg-[var(--color-border-strong)] my-1" />
                    <DropdownMenu.Item
                      className="flex items-center gap-3 px-3 py-2.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-strong)] rounded-xl cursor-pointer outline-none"
                      onClick={() => {
                        alert('排序方式功能即將推出');
                      }}
                    >
                      <ArrowUpDown className="w-4 h-4" />
                      排序方式...
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                      className="flex items-center gap-3 px-3 py-2.5 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-strong)] rounded-xl cursor-pointer outline-none"
                      onClick={() => {
                        alert('縮放項目功能即將推出');
                      }}
                    >
                      <ZoomIn className="w-4 h-4" />
                      縮放項目
                    </DropdownMenu.Item>
                  </DropdownMenu.Content>
                </DropdownMenu.Portal>
              </DropdownMenu.Root>
            </div>
          </div>

          {playlistsExpanded && (
            <div className="mt-1 px-4 space-y-1">
              {playlistItems.map((item) => (
                <ContextMenu.Root key={item.id}>
                  <ContextMenu.Trigger asChild>
                    <div
                      draggable
                      onDragStart={() => handleDragStart(item.id)}
                      onDragOver={(e) => handleDragOver(e, item.id)}
                      onDrop={() => handleDrop(item.id, playlistItems, setPlaylistItems)}
                      onDragEnd={handleDragEnd}
                      className={`group flex items-center ${dragOverItem === item.id ? 'border-t-2 border-[var(--color-accent-primary)]' : ''}`}
                    >
                      <button
                        onClick={() => onNavigate(item.view)}
                        className={`flex-1 flex items-center gap-3 px-3 py-2 rounded transition-colors ${currentView === item.view ? 'font-medium text-[var(--color-accent-primary-hover)]' : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-strong)]'} ${draggedItem === item.id ? 'opacity-50 cursor-move' : ''}`}
                      >
                        <span className="text-sm">{item.label}</span>
                      </button>
                      <GripVertical
                        className="w-4 h-4 text-[var(--color-text-tertiary)] opacity-0 group-hover:opacity-100 cursor-move transition-opacity"
                        strokeWidth={1.5}
                      />
                    </div>
                  </ContextMenu.Trigger>

                  <ContextMenu.Portal>
                    <ContextMenu.Content className="min-w-[160px] bg-[var(--color-surface-muted)] rounded-xl shadow-2xl shadow-black/60 p-1 z-50">
                      <ContextMenu.Item
                        className="flex items-center gap-2 px-3 py-2 text-sm text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-strong)] rounded-xl cursor-pointer outline-none"
                        onClick={() => handleRenamePlaylist(item.id)}
                      >
                        <Edit3 className="w-4 h-4" />
                        {t('nav.rename')}
                      </ContextMenu.Item>
                      <ContextMenu.Item
                        className="flex items-center gap-2 px-3 py-2 text-sm text-red-400 hover:bg-red-500/10 rounded-xl cursor-pointer outline-none"
                        onClick={() => handleDeletePlaylist(item.id)}
                      >
                        <Trash2 className="w-4 h-4" />
                        {t('nav.delete')}
                      </ContextMenu.Item>
                    </ContextMenu.Content>
                  </ContextMenu.Portal>
                </ContextMenu.Root>
              ))}
            </div>
          )}
        </div>
      </nav>

      {/* System Area */}
      <div className="p-4 space-y-2">
        {/* System Status */}
        <div className="flex items-center gap-2 px-3 text-[var(--color-text-tertiary)]">
          <div className="w-2 h-2 rounded-full bg-green-500"></div>
          <span className="text-xs">{t('nav.systemReady')}</span>
        </div>
      </div>
    </aside>
  );
}
