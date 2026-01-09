/**
 * DistantBar - Viewing Distance Slider using shadcn/ui Slider
 * 
 * Design System:
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { Eye } from 'lucide-react';
import { Slider } from '../ui/slider';

interface DistantBarProps {
      value: number; // 0-100
      onChange: (value: number) => void;
}

export function DistantBar({ value, onChange }: DistantBarProps) {
      return (
            <div className="flex items-center gap-3 bg-neutral-900 rounded-xl px-4 py-2 border border-neutral-800">
                  {/* Icon */}
                  <Eye className="w-4 h-4 text-neutral-500" strokeWidth={1.5} />

                  {/* Slider */}
                  <Slider
                        value={[value]}
                        onValueChange={(v) => onChange(v[0])}
                        min={0}
                        max={100}
                        step={1}
                        className="w-24 [&_[data-slot=slider-track]]:h-1.5 [&_[data-slot=slider-range]]:bg-purple-500 [&_[data-slot=slider-thumb]]:border-purple-500 [&_[data-slot=slider-thumb]]:w-3 [&_[data-slot=slider-thumb]]:h-3"
                  />

                  {/* Labels */}
                  <div className="flex gap-1.5 text-[10px] text-neutral-500">
                        <span>近</span>
                        <span>/</span>
                        <span>遠</span>
                  </div>
            </div>
      );
}

/**
 * Calculate grid columns based on viewing distance
 */
export function getGridColumnsFromDistance(distance: number): string {
      if (distance < 25) return 'grid-cols-3';
      if (distance < 50) return 'grid-cols-4';
      if (distance < 75) return 'grid-cols-6';
      return 'grid-cols-8';
}

/**
 * Calculate max visible memos based on viewing distance
 */
export function getMaxMemosFromDistance(distance: number): number {
      if (distance < 25) return 4;
      if (distance < 50) return 3;
      if (distance < 75) return 2;
      return 1;
}
