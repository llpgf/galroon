/**
 * Memo - Sticker-style memo using shadcn/ui Badge
 * 
 * Colors (3 only):
 * - red: user reminders (manual only)
 * - yellow: auto-generated (crack/patch detection)
 * - gray: informational
 */

import { Badge } from '../ui/badge';

export type MemoColor = 'red' | 'yellow' | 'gray';

export interface MemoData {
      id: string;
      text: string;
      color: MemoColor;
      isManual: boolean;
}

interface MemoProps {
      memo: MemoData;
      compact?: boolean;
}

const COLOR_CLASSES: Record<MemoColor, string> = {
      red: 'bg-red-500/15 text-red-400 border-red-500/30',
      yellow: 'bg-yellow-500/15 text-yellow-400 border-yellow-500/30',
      gray: 'bg-neutral-500/15 text-neutral-400 border-neutral-500/30',
};

export function Memo({ memo, compact = false }: MemoProps) {
      return (
            <Badge
                  variant="outline"
                  className={`
        flex items-center gap-1.5 rounded-md font-normal
        ${COLOR_CLASSES[memo.color]}
        ${compact ? 'text-xs px-2 py-0.5' : 'text-sm px-2.5 py-1'}
      `}
            >
                  <span className="shrink-0">–</span>
                  <span className="truncate">{memo.text}</span>
            </Badge>
      );
}

interface MemoOverflowProps {
      remainingCount: number;
}

export function MemoOverflow({ remainingCount }: MemoOverflowProps) {
      return (
            <span
                  className="text-neutral-500 text-xs py-0.5 text-center"
                  title={`+${remainingCount} more memos`}
            >
                  ⋯
            </span>
      );
}
