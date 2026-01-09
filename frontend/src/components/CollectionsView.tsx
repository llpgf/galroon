import { FolderHeart, Plus } from 'lucide-react';
import { ImageWithFallback } from './figma/ImageWithFallback';

export interface Collection {
  id: string;
  name: string;
  gameCount: number;
  coverImages: string[];
}

interface CollectionsViewProps {
  collections: Collection[];
  onSelectCollection: (id: string) => void;
  onCreateCollection: () => void;
}

export function CollectionsView({ collections, onSelectCollection, onCreateCollection }: CollectionsViewProps) {
  return (
    <div className="p-12">
      {/* Header */}
      <header className="mb-12 flex items-center justify-between">
        <div>
          <h1 className="text-white tracking-tight">Collections</h1>
          <p className="mt-2 text-[#6b6b6b]">
            {collections.length} {collections.length === 1 ? 'Collection' : 'Collections'}
          </p>
        </div>

        <button
          onClick={onCreateCollection}
          className="flex items-center gap-2 px-6 py-3 bg-white text-black rounded-lg hover:bg-[#f0f0f0] transition-colors"
        >
          <Plus className="w-5 h-5" strokeWidth={2} />
          <span>New Collection</span>
        </button>
      </header>

      {/* Collections Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
        {collections.map((collection) => (
          <button
            key={collection.id}
            onClick={() => onSelectCollection(collection.id)}
            className="group text-left"
          >
            {/* Collection Preview - Mosaic of covers */}
            <div className="aspect-square bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg overflow-hidden mb-4 relative hover:border-[#3a3a3a] transition-colors">
              {collection.coverImages.length > 0 ? (
                <div className="grid grid-cols-2 gap-1 w-full h-full p-1">
                  {collection.coverImages.slice(0, 4).map((image, index) => (
                    <div key={index} className="relative overflow-hidden rounded">
                      <ImageWithFallback
                        src={image}
                        alt={`${collection.name} preview ${index + 1}`}
                        className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                      />
                    </div>
                  ))}
                  {/* Fill empty slots */}
                  {Array.from({ length: Math.max(0, 4 - collection.coverImages.length) }).map((_, index) => (
                    <div key={`empty-${index}`} className="bg-[#2a2a2a] rounded flex items-center justify-center">
                      <FolderHeart className="w-8 h-8 text-[#4a4a4a]" strokeWidth={1} />
                    </div>
                  ))}
                </div>
              ) : (
                <div className="w-full h-full flex items-center justify-center">
                  <FolderHeart className="w-16 h-16 text-[#4a4a4a]" strokeWidth={1} />
                </div>
              )}
            </div>

            {/* Collection Info */}
            <div className="px-1">
              <h3 className="text-white mb-1 group-hover:text-[#7ba8c7] transition-colors">
                {collection.name}
              </h3>
              <p className="text-[#6b6b6b]">
                <small>{collection.gameCount} {collection.gameCount === 1 ? 'game' : 'games'}</small>
              </p>
            </div>
          </button>
        ))}

        {collections.length === 0 && (
          <div className="col-span-full text-center py-20 text-[#6b6b6b]">
            <FolderHeart className="w-16 h-16 mx-auto mb-4 text-[#4a4a4a]" strokeWidth={1} />
            <p className="mb-6">No collections yet</p>
            <button
              onClick={onCreateCollection}
              className="inline-flex items-center gap-2 px-6 py-3 bg-white text-black rounded-lg hover:bg-[#f0f0f0] transition-colors"
            >
              <Plus className="w-5 h-5" strokeWidth={2} />
              <span>Create Your First Collection</span>
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
