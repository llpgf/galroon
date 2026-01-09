import React, { useState } from 'react';
import { Trash2, Plus, FolderOpen, MoreVertical, RefreshCw, Power, Edit3, HardDrive, Clock, Download, Upload, ArrowRight, ChevronRight, Network, Folder } from 'lucide-react';
import { cn } from '../lib/utils';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';

interface SettingsProps {
  onBack?: () => void;
}

interface StorageFolder {
  id: string;
  path: string;
  displayName: string;
  status: 'scanning' | 'ready' | 'disabled' | 'error';
  tracksCount: number;
  enabled: boolean;
}

interface BackupItem {
  id: string;
  date: string;
  size: string;
  type: 'auto' | 'manual';
}

export function Settings({ onBack }: SettingsProps) {
  const [activeTab, setActiveTab] = useState('storage');

  // Enhanced storage folders with state
  const [storageFolders, setStorageFolders] = useState<StorageFolder[]>([
    { id: '1', path: 'D:/Games', displayName: 'Games', status: 'ready', tracksCount: 4241, enabled: true },
    { id: '2', path: 'C:/Program Files/Steam/steamapps/common', displayName: 'Steam', status: 'scanning', tracksCount: 1523, enabled: true },
    { id: '3', path: 'E:/Epic Games', displayName: 'Epic Games', status: 'ready', tracksCount: 892, enabled: true },
  ]);

  // Backup state
  const [autoBackup, setAutoBackup] = useState(true);
  const [backupFrequency, setBackupFrequency] = useState('weekly');
  const [backupLocation, setBackupLocation] = useState('C:/Users/Ben/Documents/Galroon/Backups');
  const [backupHistory] = useState<BackupItem[]>([
    { id: '1', date: '2026-01-06 14:30', size: '128 MB', type: 'auto' },
    { id: '2', date: '2026-01-05 09:15', size: '127 MB', type: 'manual' },
    { id: '3', date: '2026-01-01 14:30', size: '125 MB', type: 'auto' },
  ]);

  const [scanOnStartup, setScanOnStartup] = useState(true);
  const [autoImport, setAutoImport] = useState(false);
  const [ignoreHidden, setIgnoreHidden] = useState(true);

  const [cardSize, setCardSize] = useState(2);
  const [accentColor, setAccentColor] = useState('#7ba8c7');

  const accentColors = [
    '#7ba8c7', // Default blue
    '#8ab68a', // Green
    '#c78a7b', // Coral
    '#b87bc7', // Purple
    '#c7c77b', // Yellow
    '#7bc7c7', // Cyan
  ];

  // Storage folder handlers
  const removeFolder = (id: string) => {
    if (confirm('確定要移除此資料夾嗎？')) {
      setStorageFolders(storageFolders.filter(f => f.id !== id));
    }
  };

  const toggleFolder = (id: string) => {
    setStorageFolders(storageFolders.map(f =>
      f.id === id ? { ...f, enabled: !f.enabled, status: f.enabled ? 'disabled' : 'ready' } : f
    ));
  };

  const forceRescan = (id: string) => {
    setStorageFolders(storageFolders.map(f =>
      f.id === id ? { ...f, status: 'scanning' } : f
    ));
    // Simulate scan completion after 2 seconds
    setTimeout(() => {
      setStorageFolders(prev => prev.map(f =>
        f.id === id ? { ...f, status: 'ready' } : f
      ));
    }, 2000);
  };

  const editFolder = (id: string) => {
    const folder = storageFolders.find(f => f.id === id);
    if (folder) {
      const newPath = prompt('編輯路徑:', folder.path);
      if (newPath) {
        setStorageFolders(storageFolders.map(f =>
          f.id === id ? { ...f, path: newPath, displayName: newPath.split('/').pop() || newPath } : f
        ));
      }
    }
  };

  const addFolder = () => {
    const newFolder = prompt('輸入資料夾路徑:');
    if (newFolder) {
      setStorageFolders([...storageFolders, {
        id: Date.now().toString(),
        path: newFolder,
        displayName: newFolder.split('/').pop() || newFolder,
        status: 'scanning',
        tracksCount: 0,
        enabled: true
      }]);
    }
  };

  // Backup handlers
  const handleBackupNow = () => {
    alert('備份已開始...');
  };

  const handleRestore = (id: string) => {
    if (confirm('確定要從此備份還原嗎？這將覆蓋現有的元資料。')) {
      alert('還原中...');
    }
  };

  const getStatusText = (status: StorageFolder['status']) => {
    switch (status) {
      case 'scanning': return '正在掃描中...';
      case 'ready': return '正在即時監測新檔案';
      case 'disabled': return '已停用';
      case 'error': return '發生錯誤';
    }
  };

  const getStatusColor = (status: StorageFolder['status']) => {
    switch (status) {
      case 'scanning': return 'text-[#7ba8c7]';
      case 'ready': return 'text-green-500';
      case 'disabled': return 'text-[#6b6b6b]';
      case 'error': return 'text-red-500';
    }
  };

  return (
    <div className="flex h-full bg-[#121212]">
      {/* Middle Navigation Sidebar */}
      <div className="w-60 bg-[#121212] flex flex-col">
        <div className="p-6 pb-4">
          <h2 className="text-white text-xl tracking-wide">設定</h2>
        </div>

        {/* Navigation */}
        <nav className="flex-1 overflow-y-auto p-4">
          <div className="space-y-1">
            <button
              onClick={() => setActiveTab('general')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'general' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'general' ? { color: '#818cf8' } : undefined}
            >
              一般
            </button>
            <button
              onClick={() => setActiveTab('storage')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'storage' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'storage' ? { color: '#818cf8' } : undefined}
            >
              儲存位置
            </button>
            <button
              onClick={() => setActiveTab('backup')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'backup' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'backup' ? { color: '#818cf8' } : undefined}
            >
              備份
            </button>
            <button
              onClick={() => setActiveTab('library')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'library' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'library' ? { color: '#818cf8' } : undefined}
            >
              遊戲庫服務
            </button>
            <button
              onClick={() => setActiveTab('display')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'display' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'display' ? { color: '#818cf8' } : undefined}
            >
              顯示模式
            </button>
            <button
              onClick={() => setActiveTab('advanced')}
              className={`w-full text-left px-3 py-2 rounded-xl text-sm transition-colors ${activeTab === 'advanced' ? 'font-medium' : 'text-[#b3b3b3] hover:text-white'}`}
              style={activeTab === 'advanced' ? { color: '#818cf8' } : undefined}
            >
              進階設定
            </button>
          </div>
        </nav>
      </div>

      {/* Right Content Area */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl p-12">
          {/* General Tab */}
          {activeTab === 'general' && (
            <div className="space-y-8">
              <div>
                <h3 className="text-white text-2xl mb-6">一般</h3>
                <div className="space-y-6">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Launch on startup</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Start Galroon when system boots
                      </div>
                    </div>
                    <Toggle value={false} onChange={() => { }} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Minimize to tray</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Keep running in background
                      </div>
                    </div>
                    <Toggle value={true} onChange={() => { }} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Storage Tab - Enhanced */}
          {activeTab === 'storage' && (
            <div className="space-y-8">
              <div>
                {/* Header with title and add button */}
                <div className="flex items-start justify-between mb-8">
                  <div>
                    <h3 className="text-white text-2xl mb-2">儲存位置</h3>
                    <p className="text-[#6b6b6b] text-sm">Resource Folders</p>
                  </div>
                  <button
                    onClick={addFolder}
                    className="flex items-center gap-2 bg-[#1a1a1a] hover:bg-[#252525] rounded-xl px-4 py-2.5 text-white text-sm transition-colors"
                  >
                    <Plus className="w-4 h-4" strokeWidth={1.5} />
                    新增資料夾
                  </button>
                </div>

                {/* Folder List - Breadcrumb style */}
                <div className="space-y-3">
                  {storageFolders.map((folder) => {
                    // Parse path into breadcrumb segments
                    const pathSegments = folder.path.split(/[\/\\]/).filter(Boolean);

                    return (
                      <div
                        key={folder.id}
                        className={`flex items-center gap-4 bg-[#1a1a1a] rounded-xl p-4 group transition-opacity ${!folder.enabled ? 'opacity-50' : ''}`}
                      >
                        {/* Left: Folder icon */}
                        <div className="flex-shrink-0">
                          <div className="w-10 h-10 bg-[#252525] rounded-xl flex items-center justify-center">
                            <FolderOpen className="w-5 h-5 text-[#818cf8]" strokeWidth={1.5} />
                          </div>
                        </div>

                        {/* Center: Breadcrumb path and status */}
                        <div className="flex-1 min-w-0">
                          {/* Breadcrumb path */}
                          <div className="flex items-center gap-1 text-sm mb-2 flex-wrap">
                            {folder.path.split(/[\/\\]/).filter(Boolean).map((segment, i, arr) => {
                              const isLast = i === arr.length - 1;
                              // Determine icon for the first segment (root)
                              let RootIcon = null;
                              if (i === 0) {
                                if (segment.includes(':')) RootIcon = HardDrive; // Windows drive letter
                                else if (segment.startsWith('\\\\') || segment.startsWith('//')) RootIcon = Network; // Network path
                                else RootIcon = Folder; // Regular folder
                              }

                              return (
                                <React.Fragment key={i}>
                                  <div className="flex items-center">
                                    {RootIcon && (
                                      <RootIcon className="w-4 h-4 text-[#888] mr-1.5" />
                                    )}
                                    <span
                                      className={cn(
                                        isLast
                                          ? "text-white font-bold text-base"
                                          : "text-[#888] font-medium text-sm"
                                      )}
                                    >
                                      {segment}
                                    </span>
                                  </div>
                                  {!isLast && (
                                    <ChevronRight className="w-4 h-4 text-[#525252] mx-1" />
                                  )}
                                </React.Fragment>
                              );
                            })}
                          </div>
                          {/* Status text */}
                          <div className={`text-xs mb-1 ${getStatusColor(folder.status)}`}>
                            {folder.status === 'scanning' && (
                              <RefreshCw className="w-3 h-3 inline-block mr-1 animate-spin" />
                            )}
                            {getStatusText(folder.status)}
                          </div>
                          {/* Track count */}
                          <div className="text-[#6b6b6b] text-xs">
                            已匯入 {folder.tracksCount.toLocaleString()} 項資源
                          </div>
                        </div>

                        {/* Right: Three-dot menu */}
                        <DropdownMenu.Root>
                          <DropdownMenu.Trigger asChild>
                            <button className="p-2 text-[#6b6b6b] hover:text-white hover:bg-[#252525] rounded-xl transition-colors opacity-0 group-hover:opacity-100">
                              <MoreVertical className="w-5 h-5" strokeWidth={1.5} />
                            </button>
                          </DropdownMenu.Trigger>

                          <DropdownMenu.Portal>
                            <DropdownMenu.Content
                              className="min-w-[180px] bg-[#1a1a1a] rounded-xl shadow-2xl shadow-black/60 p-1 z-50"
                              sideOffset={5}
                              align="end"
                            >
                              <DropdownMenu.Item
                                className="flex items-center gap-3 px-3 py-2.5 text-sm text-[#b3b3b3] hover:bg-[#252525] hover:text-white rounded-xl cursor-pointer outline-none"
                                onClick={() => forceRescan(folder.id)}
                              >
                                <RefreshCw className="w-4 h-4" />
                                強制重新掃描
                              </DropdownMenu.Item>
                              <DropdownMenu.Item
                                className="flex items-center gap-3 px-3 py-2.5 text-sm text-[#b3b3b3] hover:bg-[#252525] hover:text-white rounded-xl cursor-pointer outline-none"
                                onClick={() => toggleFolder(folder.id)}
                              >
                                <Power className="w-4 h-4" />
                                {folder.enabled ? '停用' : '啟用'}
                              </DropdownMenu.Item>
                              <DropdownMenu.Item
                                className="flex items-center gap-3 px-3 py-2.5 text-sm text-[#b3b3b3] hover:bg-[#252525] hover:text-white rounded-xl cursor-pointer outline-none"
                                onClick={() => editFolder(folder.id)}
                              >
                                <Edit3 className="w-4 h-4" />
                                編輯
                              </DropdownMenu.Item>
                              <DropdownMenu.Separator className="h-px bg-[#3a3a3a] my-1" />
                              <DropdownMenu.Item
                                className="flex items-center gap-3 px-3 py-2.5 text-sm text-red-400 hover:bg-red-500/10 rounded-xl cursor-pointer outline-none"
                                onClick={() => removeFolder(folder.id)}
                              >
                                <Trash2 className="w-4 h-4" />
                                移除
                              </DropdownMenu.Item>
                            </DropdownMenu.Content>
                          </DropdownMenu.Portal>
                        </DropdownMenu.Root>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          )}

          {/* Backup Tab - NEW */}
          {activeTab === 'backup' && (
            <div className="space-y-8">
              <div>
                <h3 className="text-white text-2xl mb-2">備份</h3>
                <p className="text-[#b3b3b3] text-sm mb-8 leading-relaxed">
                  備份您的遊戲庫元資料，包括您的編輯、評分、遊玩狀態和自訂標籤。備份不包含遊戲檔案本身。
                </p>

                {/* Auto backup settings */}
                <div className="bg-[#1a1a1a] rounded-xl p-6 mb-6 shadow-md shadow-black/20">
                  <div className="flex items-start justify-between mb-6">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">自動備份</div>
                      <div className="text-[#6b6b6b] text-sm">
                        定期自動備份您的元資料
                      </div>
                    </div>
                    <Toggle value={autoBackup} onChange={setAutoBackup} />
                  </div>

                  {autoBackup && (
                    <div className="space-y-4 pt-4 border-t border-[#2a2a2a]">
                      <div>
                        <div className="text-[#b3b3b3] text-xs uppercase tracking-wider mb-2">備份頻率</div>
                        <div className="flex gap-2">
                          {['daily', 'weekly', 'monthly'].map((freq) => (
                            <button
                              key={freq}
                              onClick={() => setBackupFrequency(freq)}
                              className={`px-4 py-2 rounded-xl text-sm transition-colors ${backupFrequency === freq
                                ? 'bg-[#7ba8c7] text-white'
                                : 'bg-[#252525] text-[#b3b3b3] hover:bg-[#353535] hover:text-white'
                                }`}
                            >
                              {freq === 'daily' ? '每日' : freq === 'weekly' ? '每週' : '每月'}
                            </button>
                          ))}
                        </div>
                      </div>

                      <div>
                        <div className="text-[#b3b3b3] text-xs uppercase tracking-wider mb-2">備份位置</div>
                        <div className="flex items-center gap-2">
                          <div className="flex-1 bg-[#252525] rounded-xl px-3 py-2 text-white text-sm font-mono">
                            {backupLocation}
                          </div>
                          <button className="px-3 py-2 bg-[#252525] hover:bg-[#353535] rounded-xl text-white text-sm transition-colors">
                            瀏覽...
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </div>

                {/* Manual backup/restore actions */}
                <div className="flex gap-3 mb-8">
                  <button
                    onClick={handleBackupNow}
                    className="flex items-center gap-2 bg-[#7ba8c7] hover:bg-[#6a97b6] px-4 py-2.5 rounded-xl text-white text-sm transition-colors"
                  >
                    <Upload className="w-4 h-4" strokeWidth={1.5} />
                    立即備份
                  </button>
                  <button
                    className="flex items-center gap-2 bg-[#1a1a1a] hover:bg-[#252525] px-4 py-2.5 rounded-xl text-white text-sm transition-colors shadow-md shadow-black/20"
                  >
                    <Download className="w-4 h-4" strokeWidth={1.5} />
                    從檔案還原...
                  </button>
                </div>

                {/* Backup history */}
                <div>
                  <div className="text-[#6b6b6b] text-xs uppercase tracking-wider mb-4">
                    備份歷史記錄
                  </div>
                  <div className="space-y-2">
                    {backupHistory.map((backup) => (
                      <div
                        key={backup.id}
                        className="flex items-center gap-4 bg-[#1a1a1a] rounded-xl p-4 group shadow-md shadow-black/20"
                      >
                        <div className="w-10 h-10 bg-[#252525] rounded-lg flex items-center justify-center">
                          <Clock className="w-5 h-5 text-[#6b6b6b]" strokeWidth={1.5} />
                        </div>
                        <div className="flex-1">
                          <div className="text-white text-sm mb-1">
                            {backup.date}
                            {backup.type === 'auto' && (
                              <span className="ml-2 text-xs text-[#7ba8c7]">自動</span>
                            )}
                          </div>
                          <div className="text-[#6b6b6b] text-xs">
                            {backup.size}
                          </div>
                        </div>
                        <button
                          onClick={() => handleRestore(backup.id)}
                          className="px-3 py-1.5 bg-[#252525] hover:bg-[#353535] rounded-xl text-sm text-[#b3b3b3] hover:text-white transition-colors opacity-0 group-hover:opacity-100"
                        >
                          還原
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Library Tab */}
          {activeTab === 'library' && (
            <div className="space-y-8">
              <div>
                <h3 className="text-white text-2xl mb-6">遊戲庫服務</h3>
                <div className="space-y-6">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Show orphan cards</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Display unresolved game entries
                      </div>
                    </div>
                    <Toggle value={true} onChange={() => { }} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Group by series</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Organize games by franchise
                      </div>
                    </div>
                    <Toggle value={false} onChange={() => { }} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Scan on startup</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Automatically scan for new games
                      </div>
                    </div>
                    <Toggle value={scanOnStartup} onChange={setScanOnStartup} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Auto-import detected games</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Add new games to library without confirmation
                      </div>
                    </div>
                    <Toggle value={autoImport} onChange={setAutoImport} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Ignore hidden folders</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Skip folders starting with "."
                      </div>
                    </div>
                    <Toggle value={ignoreHidden} onChange={setIgnoreHidden} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Display Tab */}
          {activeTab === 'display' && (
            <div className="space-y-8">
              <div>
                <h3 className="text-white text-2xl mb-6">顯示模式</h3>

                {/* Card Size */}
                <div className="mb-8">
                  <div className="text-white text-sm mb-4">卡片大小</div>
                  <div className="space-y-3">
                    <div className="flex items-center justify-between text-sm mb-2">
                      <span className="text-[#6b6b6b]">小</span>
                      <span className="text-white">
                        {['小', '中', '大', '特大'][cardSize]}
                      </span>
                      <span className="text-[#6b6b6b]">大</span>
                    </div>
                    <input
                      type="range"
                      min="0"
                      max="3"
                      step="1"
                      value={cardSize}
                      onChange={(e) => setCardSize(Number(e.target.value))}
                      className="w-full h-1 bg-[#2a2a2a] rounded-lg appearance-none cursor-pointer slider"
                      style={{
                        background: `linear-gradient(to right, ${accentColor} 0%, ${accentColor} ${(cardSize / 3) * 100}%, #2a2a2a ${(cardSize / 3) * 100}%, #2a2a2a 100%)`
                      }}
                    />
                  </div>
                </div>

                {/* Accent Color */}
                <div className="mb-8">
                  <div className="text-white text-sm mb-4">主題色</div>
                  <div className="flex gap-3">
                    {accentColors.map((color) => (
                      <button
                        key={color}
                        onClick={() => setAccentColor(color)}
                        className="relative w-12 h-12 rounded-full transition-transform hover:scale-110"
                        style={{ backgroundColor: color }}
                      >
                        {accentColor === color && (
                          <div className="absolute inset-0 rounded-full border-2 border-white"></div>
                        )}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="space-y-6">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">High contrast mode</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Increase visibility of UI elements
                      </div>
                    </div>
                    <Toggle value={false} onChange={() => { }} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Reduce motion</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Minimize animations and transitions
                      </div>
                    </div>
                    <Toggle value={false} onChange={() => { }} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Advanced Tab */}
          {activeTab === 'advanced' && (
            <div className="space-y-8">
              <div>
                <h3 className="text-white text-2xl mb-6">進階設定</h3>
                <div className="space-y-6">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Enable debug mode</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Show detailed logs and diagnostics
                      </div>
                    </div>
                    <Toggle value={false} onChange={() => { }} />
                  </div>
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="text-white text-sm mb-1">Hardware acceleration</div>
                      <div className="text-[#6b6b6b] text-sm">
                        Use GPU for rendering (requires restart)
                      </div>
                    </div>
                    <Toggle value={true} onChange={() => { }} />
                  </div>
                </div>

                <div className="mt-10">
                  <div className="text-[#6b6b6b] text-xs uppercase tracking-wider mb-4">
                    DATA MANAGEMENT
                  </div>
                  <div className="space-y-3">
                    <button className="w-full text-left px-4 py-3 bg-[#1a1a1a] rounded-xl text-white text-sm hover:bg-[#252525] transition-colors shadow-md shadow-black/20">
                      Clear cache
                    </button>
                    <button className="w-full text-left px-4 py-3 bg-[#1a1a1a] rounded-xl text-white text-sm hover:bg-[#252525] transition-colors shadow-md shadow-black/20">
                      Export library
                    </button>
                    <button className="w-full text-left px-4 py-3 bg-[#1a1a1a] rounded-xl text-red-400 text-sm hover:bg-red-900/10 transition-colors shadow-md shadow-black/20">
                      Reset to defaults
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// iOS-style Toggle Component
function Toggle({ value, onChange }: { value: boolean; onChange: (value: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!value)}
      className={`relative w-12 h-7 rounded-full transition-colors flex-shrink-0 ${value ? 'bg-green-500' : 'bg-[#3a3a3a]'
        }`}
    >
      <div
        className={`absolute top-0.5 w-6 h-6 bg-white rounded-full transition-transform ${value ? 'translate-x-5' : 'translate-x-0.5'
          }`}
      ></div>
    </button>
  );
}