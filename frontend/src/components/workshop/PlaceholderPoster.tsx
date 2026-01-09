/**
 * PlaceholderPoster - Dashed border placeholder using shadcn/ui Card
 * 
 * Design System:
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { Plus } from 'lucide-react';
import { Card } from '../ui/card';

interface PlaceholderPosterProps {
      onClick: () => void;
}

export function PlaceholderPoster({ onClick }: PlaceholderPosterProps) {
      return (
            <Card
                  onClick={onClick}
                  className="
        group cursor-pointer aspect-[3/4] overflow-hidden
        bg-neutral-900/50 border-2 border-dashed border-neutral-700
        hover:border-purple-500/50 hover:bg-neutral-800/50
        transition-all duration-200 rounded-xl
        flex items-center justify-center
      "
            >
                  <div className="flex flex-col items-center gap-3 text-neutral-500 group-hover:text-purple-400 transition-colors">
                        <div className="w-12 h-12 rounded-full bg-neutral-800 group-hover:bg-purple-500/20 flex items-center justify-center transition-colors">
                              <Plus className="w-6 h-6" strokeWidth={1.5} />
                        </div>
                        <span className="text-xs font-medium tracking-wider">新增作品</span>
                  </div>
            </Card>
      );
}
