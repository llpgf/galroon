import { ImageWithFallback } from './figma/ImageWithFallback';

interface CanonicalCardProps {
  title: string;
  coverImage: string;
  versionCount: number;
  featured?: boolean;
}

export function CanonicalCard({ title, coverImage, versionCount, featured = false }: CanonicalCardProps) {
  return (
    <div className={`group cursor-pointer ${featured ? 'col-span-2 row-span-2' : ''}`}>
      <div className={`relative overflow-hidden bg-[#1e1e1e] ${featured ? 'aspect-[4/3]' : 'aspect-[2/3]'}`}>
        <ImageWithFallback 
          src={coverImage} 
          alt={title}
          className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
        />
      </div>
      <div className={`px-1 ${featured ? 'mt-8' : 'mt-4'}`}>
        <h3 className={`text-white ${featured ? 'text-2xl' : ''}`}>{title}</h3>
        <p className={`text-[#6b6b6b] ${featured ? 'mt-2' : 'mt-1'}`}>
          <small>{versionCount} {versionCount === 1 ? 'Version' : 'Versions'}</small>
        </p>
      </div>
    </div>
  );
}