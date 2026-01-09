import { useTranslation } from 'react-i18next';
import { ImageWithFallback } from '../components/figma/ImageWithFallback';

export interface GameType {
  id: string;
  name: string;
  gameCount: number;
  coverImages: string[]; // Up to 4 representative game covers
}

interface GameTypesPageProps {
  types: GameType[];
  onSelectType: (id: string) => void;
}

export function GameTypesPage({ types, onSelectType }: GameTypesPageProps) {
  const { t } = useTranslation();

  return (
    <div className="min-h-screen">
      <div className="px-12 py-12">
        {/* Header */}
        <header className="mb-12">
          <h1 className="text-white tracking-tight mb-2">{t('gameTypes.title')}</h1>
          <p className="text-[#6b6b6b]">{t('gameTypes.typeCount', { count: types.length })}</p>
        </header>

        {/* Type Cards Grid */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6">
          {types.map((type) => (
            <button
              key={type.id}
              onClick={() => onSelectType(type.id)}
              className="group flex flex-col cursor-pointer"
            >
              {/* Cover Grid - 2x2 or 1 image */}
              <div className="relative aspect-square rounded-lg overflow-hidden bg-[#1e1e1e] border border-[#2a2a2a] group-hover:border-[#3a3a3a] transition-all mb-3">
                {type.coverImages.length === 1 ? (
                  <ImageWithFallback
                    src={type.coverImages[0]}
                    alt={type.name}
                    className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                  />
                ) : type.coverImages.length >= 2 ? (
                  <div className="grid grid-cols-2 gap-1 w-full h-full p-1">
                    {type.coverImages.slice(0, 4).map((cover, index) => (
                      <div key={index} className="relative overflow-hidden rounded">
                        <ImageWithFallback
                          src={cover}
                          alt={`${type.name} ${index + 1}`}
                          className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                        />
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="w-full h-full flex items-center justify-center">
                    <span className="text-6xl text-[#3a3a3a]">{type.name.charAt(0)}</span>
                  </div>
                )}

                {/* Overlay with count */}
                <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity">
                  <div className="absolute bottom-3 left-3 right-3">
                    <p className="text-white text-sm">{t('gameTypes.gameCount', { count: type.gameCount })}</p>
                  </div>
                </div>
              </div>

              {/* Type Name */}
              <h3 className="text-white text-sm px-1 group-hover:text-[#b3b3b3] transition-colors">
                {type.name}
              </h3>

              {/* Game Count */}
              <p className="text-[#6b6b6b] text-xs px-1 mt-0.5">
                {t('gameTypes.gameCount', { count: type.gameCount })}
              </p>
            </button>
          ))}
        </div>

        {/* Empty State */}
        {types.length === 0 && (
          <div className="flex items-center justify-center py-24">
            <div className="text-center">
              <p className="text-[#6b6b6b] text-lg">{t('gameTypes.emptyTitle')}</p>
              <p className="text-[#4a4a4a] text-sm mt-2">
                {t('gameTypes.emptyHint')}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
