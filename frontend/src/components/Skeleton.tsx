/**
 * Skeleton Components for Loading States
 * 
 * Displays animated skeleton placeholders when data is loading
 * or collections are empty.
 */

import { cn } from './ui/utils';

interface SkeletonProps {
      className?: string;
}

// Base skeleton with animation
export function Skeleton({ className }: SkeletonProps) {
      return (
            <div
                  className={cn(
                        "animate-pulse bg-[#2a2a2a] rounded-md",
                        className
                  )}
            />
      );
}

// Game card skeleton (16:9 poster ratio)
export function SkeletonCard({ className }: SkeletonProps) {
      return (
            <div className={cn("space-y-2", className)}>
                  <Skeleton className="aspect-[3/4] w-full rounded-lg" />
                  <Skeleton className="h-4 w-3/4" />
                  <Skeleton className="h-3 w-1/2" />
            </div>
      );
}

// Grid of skeleton cards
interface SkeletonGridProps {
      count?: number;
      className?: string;
}

export function SkeletonGrid({ count = 12, className }: SkeletonGridProps) {
      return (
            <div className={cn(
                  "grid grid-cols-2 gap-4 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7 md:gap-5 lg:gap-6",
                  className
            )}>
                  {Array.from({ length: count }).map((_, i) => (
                        <SkeletonCard key={i} />
                  ))}
            </div>
      );
}

// Text line skeleton
interface SkeletonTextProps {
      lines?: number;
      className?: string;
}

export function SkeletonText({ lines = 3, className }: SkeletonTextProps) {
      return (
            <div className={cn("space-y-2", className)}>
                  {Array.from({ length: lines }).map((_, i) => (
                        <div
                              key={i}
                              className="h-4 animate-pulse bg-[#2a2a2a] rounded-md"
                              style={{ width: `${60 + i * 10}%` }}
                        />
                  ))}
            </div>
      );
}

// Hero carousel skeleton
export function SkeletonHero({ className }: SkeletonProps) {
      return (
            <div className={cn("relative", className)}>
                  <Skeleton className="w-full aspect-[21/9] rounded-xl" />
                  <div className="absolute bottom-8 left-8 space-y-4">
                        <Skeleton className="h-10 w-64" />
                        <Skeleton className="h-6 w-48" />
                  </div>
            </div>
      );
}

// Empty state component
interface EmptyStateProps {
      title?: string;
      description?: string;
      icon?: React.ReactNode;
}

export function EmptyState({
      title = "尚無資料",
      description = "資料庫目前為空",
      icon
}: EmptyStateProps) {
      return (
            <div className="flex flex-col items-center justify-center py-16 text-center">
                  {icon && <div className="mb-4 text-[#4a4a4a]">{icon}</div>}
                  <h3 className="text-lg font-medium text-[#b3b3b3]">{title}</h3>
                  <p className="mt-2 text-sm text-[#6b6b6b]">{description}</p>
            </div>
      );
}
