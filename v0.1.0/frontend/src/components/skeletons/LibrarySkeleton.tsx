import React from 'react';

/**
 * Library Skeleton Loader - Phase 19
 *
 * Shimmering placeholder cards shown while library is loading.
 * Provides visual feedback and prevents layout shift.
 */
export const LibrarySkeleton: React.FC = () => {
  return (
    <div className="space-y-12">
      {/* Recently Added Section Skeleton */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <div className="h-7 w-40 bg-zinc-800 rounded-lg animate-pulse" />
          <div className="flex gap-2">
            <div className="h-9 w-9 bg-zinc-800 rounded-lg animate-pulse" />
            <div className="h-9 w-9 bg-zinc-800 rounded-lg animate-pulse" />
          </div>
        </div>

        {/* Horizontal Scroll Skeleton */}
        <div className="flex gap-6 overflow-hidden">
          {[...Array(6)].map((_, i) => (
            <div key={i} className="flex-shrink-0 w-56">
              <AssetCardSkeleton />
            </div>
          ))}
        </div>
      </div>

      {/* My Collection Section Skeleton */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <div className="h-7 w-32 bg-zinc-800 rounded-lg animate-pulse" />
          <div className="flex items-center gap-4">
            <div className="h-9 w-24 bg-zinc-800 rounded-lg animate-pulse" />
            <div className="h-9 w-40 bg-zinc-800 rounded-lg animate-pulse" />
          </div>
        </div>

        {/* Grid Skeleton */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6">
          {[...Array(12)].map((_, i) => (
            <AssetCardSkeleton key={i} />
          ))}
        </div>
      </div>
    </div>
  );
};

/**
 * Asset Card Skeleton Component
 */
const AssetCardSkeleton: React.FC = () => {
  return (
    <div className="space-y-3">
      {/* Cover Image Skeleton */}
      <div className="relative aspect-[3/4] bg-zinc-800 rounded-lg overflow-hidden">
        <div className="absolute inset-0 bg-gradient-to-br from-zinc-800 to-zinc-700 animate-pulse" />
      </div>

      {/* Title Skeleton */}
      <div className="space-y-2">
        <div className="h-5 w-full bg-zinc-800 rounded animate-pulse" />
        <div className="h-4 w-3/4 bg-zinc-800/50 rounded animate-pulse" />
      </div>
    </div>
  );
};

export default LibrarySkeleton;
