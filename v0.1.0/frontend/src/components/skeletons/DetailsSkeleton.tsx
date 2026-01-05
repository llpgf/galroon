import React from 'react';

/**
 * Details View Skeleton Loader - Phase 19
 *
 * Shimmering placeholder content shown while game details are loading.
 * Matches the layout of DetailsView component.
 */
export const DetailsSkeleton: React.FC = () => {
  return (
    <div className="relative h-full overflow-y-auto">
      {/* Hero Background Skeleton */}
      <div className="sticky top-0 h-[40vh] w-full relative">
        <div className="absolute inset-0 bg-zinc-800 animate-pulse" />
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-zinc-900/50 to-zinc-900" />
      </div>

      {/* Content Container */}
      <div className="relative -mt-[40vh] px-8 pb-8">
        <div className="max-w-7xl mx-auto">
          <div className="flex gap-8">
            {/* Left Column: Cover Art */}
            <div className="flex-shrink-0">
              <div className="w-[300px] aspect-[3/4] bg-zinc-800 rounded-xl overflow-hidden shadow-2xl border border-zinc-700/50 animate-pulse" />
            </div>

            {/* Right Column: Metadata */}
            <div className="flex-1 min-w-0 space-y-6">
              {/* Title Section */}
              <div className="space-y-4">
                <div className="h-10 w-3/4 bg-zinc-800 rounded-lg animate-pulse" />
                <div className="h-6 w-1/2 bg-zinc-800/50 rounded animate-pulse" />
                <div className="flex items-center gap-4">
                  <div className="h-5 w-32 bg-zinc-800/50 rounded animate-pulse" />
                  <div className="h-5 w-20 bg-zinc-800/50 rounded animate-pulse" />
                </div>
              </div>

              {/* Description Skeleton */}
              <div className="space-y-3">
                <div className="h-6 w-32 bg-zinc-800 rounded-lg animate-pulse" />
                <div className="space-y-2">
                  <div className="h-4 w-full bg-zinc-800/30 rounded animate-pulse" />
                  <div className="h-4 w-full bg-zinc-800/30 rounded animate-pulse" />
                  <div className="h-4 w-3/4 bg-zinc-800/30 rounded animate-pulse" />
                </div>
              </div>

              {/* Tags Section Skeleton */}
              <div className="space-y-3">
                <div className="h-6 w-20 bg-zinc-800 rounded-lg animate-pulse" />
                <div className="flex flex-wrap gap-2">
                  {[...Array(5)].map((_, i) => (
                    <div key={i} className="h-8 w-20 bg-zinc-800/50 rounded-lg animate-pulse" />
                  ))}
                </div>
              </div>

              {/* Assets Section Skeleton */}
              <div className="space-y-3">
                <div className="h-6 w-36 bg-zinc-800 rounded-lg animate-pulse" />
                <div className="flex flex-wrap gap-2">
                  {[...Array(4)].map((_, i) => (
                    <div key={i} className="h-8 w-16 bg-zinc-800/50 rounded-lg animate-pulse" />
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DetailsSkeleton;
