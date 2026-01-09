/**
 * PosterWall - Main container for poster grid
 * 
 * Design System:
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - rounded-xl
 */

import { Card, CardContent } from '../ui/card';
import { PosterCard, WorkshopItem } from './PosterCard';
import { PlaceholderPoster } from './PlaceholderPoster';
import { DistantBar, getGridColumnsFromDistance, getMaxMemosFromDistance } from './DistantBar';
import { WorkshopStatus } from './TopTabs';

interface PosterWallProps {
      items: WorkshopItem[];
      activeTab: WorkshopStatus;
      viewingDistance: number;
      onViewingDistanceChange: (value: number) => void;
      onPosterClick: (item: WorkshopItem) => void;
      onPlaceholderClick: () => void;
      selectedItemId?: string;
}

export function PosterWall({
      items,
      activeTab,
      viewingDistance,
      onViewingDistanceChange,
      onPosterClick,
      onPlaceholderClick,
      selectedItemId,
}: PosterWallProps) {
      const gridColumns = getGridColumnsFromDistance(viewingDistance);
      const maxMemos = getMaxMemosFromDistance(viewingDistance);

      // Show placeholder only in 未開始 (pending) tab
      const showPlaceholder = activeTab === 'pending';

      return (
            <Card className="mx-6 mb-6 bg-neutral-900 border-neutral-800 rounded-xl">
                  <CardContent className="p-6">
                        {/* DistantBar (top-right) */}
                        <div className="flex justify-end mb-4">
                              <DistantBar
                                    value={viewingDistance}
                                    onChange={onViewingDistanceChange}
                              />
                        </div>

                        {/* Poster Grid */}
                        <div className={`grid ${gridColumns} gap-4 transition-all duration-300`}>
                              {/* Placeholder Poster (always first in pending tab) */}
                              {showPlaceholder && (
                                    <PlaceholderPoster onClick={onPlaceholderClick} />
                              )}

                              {/* Poster Cards */}
                              {items.map((item) => (
                                    <PosterCard
                                          key={item.id}
                                          item={item}
                                          maxMemos={maxMemos}
                                          onClick={onPosterClick}
                                          isSelected={item.id === selectedItemId}
                                    />
                              ))}
                        </div>

                        {/* Empty State */}
                        {items.length === 0 && !showPlaceholder && (
                              <div className="flex items-center justify-center py-16">
                                    <p className="text-neutral-500 text-sm">此頁籤沒有作品</p>
                              </div>
                        )}
                  </CardContent>
            </Card>
      );
}
