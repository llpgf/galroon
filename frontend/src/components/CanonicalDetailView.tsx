import { FolderOpen, ArrowLeft } from 'lucide-react';
import { ImageWithFallback } from './figma/ImageWithFallback';

export interface CanonicalGame {
  id: string;
  title: string;
  developer?: string;
  releaseYear?: string;
  coverImage?: string;
  linkedFiles: string[];
}

interface CanonicalDetailViewProps {
  game: CanonicalGame;
  onBack: () => void;
  onOpenFolder: (path: string) => void;
}

export function CanonicalDetailView({ game, onBack, onOpenFolder }: CanonicalDetailViewProps) {
  return (
    <div className="min-h-screen">
      {/* Back Button */}
      <button
        onClick={onBack}
        className="fixed top-6 left-72 z-20 flex items-center gap-2 px-4 py-2 bg-[#1e1e1e]/90 backdrop-blur-sm border border-[#3a3a3a] rounded-lg text-[#b3b3b3] hover:text-white hover:border-[#4a4a4a] transition-all"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={2} />
        <span><small>Back</small></span>
      </button>

      {/* Hero Section with Ambient Background */}
      <div className="relative h-[60vh] overflow-hidden">
        {/* Blurred Background */}
        {game.coverImage && (
          <div className="absolute inset-0">
            <ImageWithFallback
              src={game.coverImage}
              alt={`${game.title} background`}
              className="w-full h-full object-cover blur-3xl opacity-20"
            />
            <div className="absolute inset-0 bg-gradient-to-b from-[#121212]/50 via-[#121212]/80 to-[#121212]" />
          </div>
        )}

        {/* Hero Content */}
        <div className="relative h-full flex items-end px-12 pb-16">
          <div className="flex gap-10">
            {/* Cover Art */}
            {game.coverImage && (
              <div className="flex-shrink-0 w-72 aspect-[2/3] rounded-lg overflow-hidden shadow-2xl">
                <ImageWithFallback
                  src={game.coverImage}
                  alt={game.title}
                  className="w-full h-full object-cover"
                />
              </div>
            )}

            {/* Title & Metadata */}
            <div className="flex flex-col justify-end pb-4">
              <h1 className="text-5xl tracking-tight mb-4">{game.title}</h1>
              {(game.developer || game.releaseYear) && (
                <p className="text-[#b3b3b3] text-xl mb-8">
                  {[game.developer, game.releaseYear].filter(Boolean).join(' · ')}
                </p>
              )}

              {/* Action Bar */}
              <div className="flex gap-3">
                <button
                  onClick={() => game.linkedFiles[0] && onOpenFolder(game.linkedFiles[0])}
                  className="flex items-center gap-2 px-6 py-3 border-2 border-white text-white rounded-lg hover:bg-white hover:text-black transition-colors"
                >
                  <FolderOpen className="w-5 h-5" strokeWidth={2} />
                  <span>Open Folder</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Body Content */}
      <div className="px-12 py-12">
        <div className="max-w-4xl">
          {/* Linked Files Section */}
          <section>
            <h2 className="text-white mb-6">Linked Files</h2>
            <div className="space-y-2">
              {game.linkedFiles.map((path, index) => (
                <div
                  key={index}
                  className="flex items-center gap-4 p-4 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg"
                >
                  <FolderOpen className="w-5 h-5 text-[#6b6b6b]" strokeWidth={1.5} />
                  <code className="text-[#b3b3b3] flex-1">{path}</code>
                </div>
              ))}
              
              {game.linkedFiles.length === 0 && (
                <p className="text-[#6b6b6b] py-4">No linked files</p>
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
