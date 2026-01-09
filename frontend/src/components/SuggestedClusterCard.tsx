import { ImageWithFallback } from './figma/ImageWithFallback';

interface SuggestedClusterCardProps {
  images: string[];
  matchCount: number;
}

export function SuggestedClusterCard({ images, matchCount }: SuggestedClusterCardProps) {
  return (
    <div className="group cursor-pointer">
      <div className="relative aspect-[2/3] bg-[#1a1a1a] border-2 border-dashed border-[#3a3a3a] p-4 flex items-center justify-center">
        {/* Evidence box style - chaotic stacked images */}
        <div className="relative w-full h-full">
          {/* Multiple overlapping semi-transparent images */}
          {images.slice(0, 4).map((img, index) => (
            <div
              key={index}
              className="absolute inset-0"
              style={{
                transform: `rotate(${(index - 2) * 8}deg) translate(${(index - 2) * 6}px, ${(index - 2) * 8}px)`,
                opacity: 0.3 + (index * 0.15),
                zIndex: index
              }}
            >
              <ImageWithFallback 
                src={img} 
                alt={`Evidence ${index + 1}`}
                className="w-full h-full object-cover border border-[#4a4a4a]/60"
              />
            </div>
          ))}
          
          {/* Overlay grid pattern to suggest "evidence" */}
          <div 
            className="absolute inset-0 pointer-events-none"
            style={{
              backgroundImage: `
                repeating-linear-gradient(0deg, transparent, transparent 19px, rgba(255,255,255,0.02) 19px, rgba(255,255,255,0.02) 20px),
                repeating-linear-gradient(90deg, transparent, transparent 19px, rgba(255,255,255,0.02) 19px, rgba(255,255,255,0.02) 20px)
              `
            }}
          />
          
          {/* Count badge - evidence tag style */}
          <div className="absolute top-2 right-2 bg-[#2a2a2a] border border-[#4a4a4a] px-3 py-1 rotate-3">
            <code className="text-[#7ba8c7]">{matchCount}</code>
          </div>
        </div>
      </div>
      
      <div className="mt-4 px-1">
        <h3 className="text-[#7ba8c7]">Suggested Match</h3>
        <p className="mt-1 text-[#6b6b6b]">
          <small>Requires Investigation</small>
        </p>
      </div>
    </div>
  );
}