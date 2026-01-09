/**
 * TopTabs - Workshop status tabs using shadcn/ui Tabs
 * 
 * Design System:
 * - bg-neutral-950 (background)
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { Tabs, TabsList, TabsTrigger } from '../ui/tabs';

export type WorkshopStatus = 'pending' | 'working' | 'paused';

interface TopTabsProps {
      activeTab: WorkshopStatus;
      onTabChange: (tab: WorkshopStatus) => void;
      counts: {
            pending: number;
            working: number;
            paused: number;
      };
}

const TAB_CONFIG: { key: WorkshopStatus; label: string }[] = [
      { key: 'pending', label: '未開始' },
      { key: 'working', label: '工作中' },
      { key: 'paused', label: '擱置' },
];

export function TopTabs({ activeTab, onTabChange, counts }: TopTabsProps) {
      return (
            <div className="px-6 pt-4">
                  <Tabs
                        value={activeTab}
                        onValueChange={(v) => onTabChange(v as WorkshopStatus)}
                        className="w-fit"
                  >
                        <TabsList className="h-12 gap-1 bg-neutral-900 border border-neutral-800 rounded-xl p-1">
                              {TAB_CONFIG.map(({ key, label }) => {
                                    const count = counts[key];
                                    return (
                                          <TabsTrigger
                                                key={key}
                                                value={key}
                                                className="
                  px-6 py-2 rounded-lg text-sm font-medium
                  data-[state=inactive]:bg-transparent
                  data-[state=inactive]:text-neutral-500
                  data-[state=inactive]:hover:text-neutral-300
                  data-[state=active]:bg-purple-500
                  data-[state=active]:text-white
                "
                                          >
                                                {label}
                                                {count > 0 && (
                                                      <span className="ml-2 text-xs opacity-70">({count})</span>
                                                )}
                                          </TabsTrigger>
                                    );
                              })}
                        </TabsList>
                  </Tabs>
            </div>
      );
}
