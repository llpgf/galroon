/**
 * PosterCard - Workshop poster using shadcn/ui Card
 * 
 * Design System:
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { Card, CardContent } from '../ui/card';
import { ImageWithFallback } from '../figma/ImageWithFallback';
import { Memo, MemoData, MemoOverflow } from './Memo';
import { WorkshopStatus } from './TopTabs';

export interface WorkshopItem {
      id: string;
      title?: string;
      coverImage?: string;
      status: WorkshopStatus;
      tags?: string[];
      memos?: MemoData[];
      isInGallery?: boolean;
}

interface PosterCardProps {
      item: WorkshopItem;
      maxMemos: number;
      onClick: (item: WorkshopItem) => void;
      isSelected?: boolean;
}

// Status icon colors
const STATUS_COLORS: Record<WorkshopStatus, string> = {
      pending: 'bg-neutral-500',
      working: 'bg-purple-500',
      paused: 'bg-yellow-600',
};

export function PosterCard({ item, maxMemos, onClick, isSelected }: PosterCardProps) {
      const memos = item.memos || [];
      const visibleMemos = memos.slice(0, maxMemos);
      const overflowCount = memos.length - maxMemos;
      const statusColor = STATUS_COLORS[item.status];

      // Generate gradient for missing covers
      const generateGradient = (id: string) => {
            const hue = parseInt(id.slice(-2), 36) % 360;
            return `linear-gradient(135deg, hsl(${hue}, 40%, 20%) 0%, hsl(${(hue + 60) % 360}, 40%, 15%) 100%)`;
      };

      return (
            <Card
                  onClick={() => onClick(item)}
                  className={`
        group cursor-pointer overflow-hidden transition-all
        bg-neutral-900 border-neutral-800 rounded-xl
        hover:border-neutral-700
        ${isSelected ? 'ring-2 ring-purple-500 ring-offset-2 ring-offset-neutral-950' : ''}
      `}
            >
                  {/* Status Icon (top-left, no text) */}
                  <div className="absolute top-2 left-2 z-10">
                        <div
                              className={`w-3 h-3 rounded-full ${statusColor} ring-2 ring-neutral-900`}
                              title={item.status}
                        />
                  </div>

                  {/* Poster Image */}
                  <div className="relative aspect-[3/4] overflow-hidden">
                        {item.coverImage ? (
                              <ImageWithFallback
                                    src={item.coverImage}
                                    alt={item.title || 'Untitled'}
                                    className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
                              />
                        ) : (
                              <div
                                    className="w-full h-full"
                                    style={{ background: generateGradient(item.id) }}
                              />
                        )}

                        {/* Hover overlay with title */}
                        {item.title && (
                              <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/90 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-200 p-3">
                                    <p className="text-white text-sm truncate">{item.title}</p>
                              </div>
                        )}
                  </div>

                  {/* Memo Column */}
                  {memos.length > 0 && (
                        <CardContent className="p-2 space-y-1 bg-neutral-900">
                              {visibleMemos.map((memo) => (
                                    <Memo key={memo.id} memo={memo} compact />
                              ))}
                              {overflowCount > 0 && <MemoOverflow remainingCount={overflowCount} />}
                        </CardContent>
                  )}
            </Card>
      );
}
