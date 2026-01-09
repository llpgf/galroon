import { Search, Radar, X, RefreshCw } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useTranslation } from 'react-i18next';
import { cn } from './ui/utils';

interface StoragePath {
  path: string;
  usage: number; // 0-100
  label: string;
}

interface TopBarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  libraryPaths?: StoragePath[];
  scanningPaths?: Set<string>;
  onScanPath?: (path: string) => void;
  onScanAll?: () => void;
}

export function TopBar({
  searchQuery,
  onSearchChange,
  libraryPaths = [],
  scanningPaths = new Set(),
  onScanPath,
  onScanAll
}: TopBarProps) {
  const { t } = useTranslation();

  // Apple-style gradient for storage usage
  const getUsageGradient = (usage: number) => {
    if (usage > 90) return 'from-red-500 to-red-400';
    if (usage > 70) return 'from-amber-500 to-yellow-400';
    return 'from-emerald-500 to-teal-400';
  };

  return (
    <div className="sticky top-0 z-50 backdrop-blur-xl bg-[var(--color-surface)]/90 px-8 py-4 transition-all duration-300">
      <div className="flex items-center justify-between">

        {/* Left Side: Empty */}
        <div className="flex items-center gap-3">
        </div>

        {/* Right Side Group: Search + Quick Scan */}
        <div className="flex items-center gap-4">

          {/* Search Bar */}
          <div className="relative w-80 group">
            <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-tertiary)] group-focus-within:text-[var(--color-text-strong)] transition-colors" strokeWidth={2} />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              placeholder={t('topbar.searchPlaceholder')}
              className="w-full bg-[var(--color-surface-muted)] rounded-xl pl-10 pr-10 py-2.5 text-[var(--color-text-strong)] placeholder:text-[var(--color-text-tertiary)] focus:outline-none focus:bg-[var(--color-surface-hover)] transition-all shadow-lg shadow-black/30"
            />
            {searchQuery && (
              <button
                onClick={() => onSearchChange('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-strong)] rounded-full hover:bg-[var(--color-surface-hover)] transition-colors"
              >
                <X className="w-3 h-3" strokeWidth={3} />
              </button>
            )}
          </div>

          <div className="w-px h-6 bg-[var(--color-border-medium)]" />

          {/* Quick Scan Dropdown - Larger Icon */}
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <button
                className="flex items-center justify-center w-12 h-12 bg-[var(--color-surface-muted)] rounded-xl text-[var(--color-accent-primary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-accent-primary-hover)] hover:shadow-lg hover:shadow-black/30 transition-all group data-[state=open]:bg-[var(--color-surface-hover)] data-[state=open]:scale-95 shadow-lg shadow-black/20"
                title={t('topbar.quickScan')}
              >
                <Radar className="w-6 h-6 group-hover:scale-110 transition-transform duration-300" strokeWidth={1.5} />
              </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
              <DropdownMenu.Content
                className="z-50 min-w-[360px] backdrop-blur-xl bg-[var(--color-surface-muted)] rounded-xl shadow-2xl shadow-black/60 p-3 animate-in fade-in zoom-in-95 duration-200"
                sideOffset={12}
                align="end"
              >
                <DropdownMenu.Label className="px-3 py-2 text-xs text-[var(--color-text-secondary)] uppercase tracking-wider font-medium">
                  {t('topbar.scanTarget')}
                </DropdownMenu.Label>

                <div className="space-y-2">
                  {libraryPaths.length > 0 ? (
                    libraryPaths.map((item) => {
                      const isScanning = scanningPaths.has(item.path);
                      return (
                        <div
                          key={item.path}
                          className="flex items-center gap-4 px-4 py-3 rounded-xl bg-[var(--color-surface-hover)] hover:bg-[var(--color-surface-strong)] transition-all group shadow-md shadow-black/20"
                        >
                          {/* Info Section */}
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center justify-between mb-2">
                              <span className="text-[var(--color-text-strong)] text-sm font-medium truncate">{item.label}</span>
                            </div>

                            {/* Apple-style Storage Bar */}
                            <div className="w-full h-2 rounded-full bg-black/30 overflow-hidden mb-2">
                              <div
                                className={cn(
                                  "h-full rounded-full bg-gradient-to-r transition-all duration-700 ease-out",
                                  getUsageGradient(item.usage)
                                )}
                                style={{ width: `${item.usage}%` }}
                              />
                            </div>

                            <div className="text-[10px] text-[var(--color-text-tertiary)] truncate font-mono">
                              {item.path}
                            </div>
                          </div>

                          {/* Scan Button */}
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              onScanPath?.(item.path);
                            }}
                            disabled={isScanning}
                            className={cn(
                              "flex items-center justify-center w-9 h-9 rounded-lg transition-all duration-300 flex-shrink-0",
                              isScanning
                                ? "bg-[var(--color-accent-primary)] text-[var(--color-text-strong)] cursor-wait"
                                : "bg-[var(--color-accent-primary)] text-[var(--color-text-strong)] hover:bg-[var(--color-accent-primary-hover)] hover:scale-110 shadow-lg shadow-black/30"
                            )}
                            title={t('topbar.scanThisPath')}
                          >
                            <RefreshCw className={cn("w-4 h-4", isScanning && "animate-spin")} />
                          </button>
                        </div>
                      );
                    })
                  ) : (
                    <div className="px-4 py-6 text-center text-sm text-[var(--color-text-tertiary)]">
                      {t('topbar.noPaths')}
                    </div>
                  )}
                </div>

                <DropdownMenu.Separator className="h-px bg-[var(--color-border-medium)] my-3" />

                {/* Scan All Button */}
                <DropdownMenu.Item
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-[var(--color-accent-primary)] hover:bg-[var(--color-accent-primary-hover)] text-[var(--color-text-strong)] rounded-xl cursor-pointer transition-all outline-none group hover:scale-[1.02] hover:shadow-lg hover:shadow-black/30"
                  onClick={() => onScanAll?.()}
                >
                  <RefreshCw className="w-5 h-5 group-hover:rotate-180 transition-transform duration-500" />
                  <span className="text-sm font-semibold">{t('topbar.scanAll')}</span>
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </div>
    </div>
  );
}
