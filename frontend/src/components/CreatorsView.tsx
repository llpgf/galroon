import { useTranslation } from 'react-i18next';
import { ImageWithFallback } from './figma/ImageWithFallback';

export interface Creator {
  id: string;
  name: string;
  photo?: string;
  gameCount?: number;
  role?: string;
}

interface CreatorsViewProps {
  type: 'voice-actors' | 'artists' | 'writers' | 'composers' | 'series';
  creators: Creator[];
  onSelectCreator: (id: string) => void;
}

export function CreatorsView({ type, creators, onSelectCreator }: CreatorsViewProps) {
  const { t } = useTranslation();

  const titleKeys: Record<typeof type, string> = {
    'voice-actors': 'creators.myVoiceActors',
    'artists': 'creators.myArtists',
    'writers': 'creators.myWriters',
    'composers': 'creators.myComposers',
    'series': 'creators.mySeries'
  };

  const countKeys: Record<typeof type, string> = {
    'voice-actors': 'creators.voiceActorCount',
    'artists': 'creators.artistCount',
    'writers': 'creators.writerCount',
    'composers': 'creators.composerCount',
    'series': 'creators.seriesCount'
  };

  // Type names for empty state (without "my" prefix)
  const typeNames: Record<typeof type, string> = {
    'voice-actors': t('nav.voiceActors'),
    'artists': t('nav.artists'),
    'writers': t('nav.writers'),
    'composers': t('nav.composers'),
    'series': t('nav.series')
  };

  return (
    <div className="min-h-screen">
      <div className="px-12 py-12">
        {/* Header */}
        <header className="mb-12">
          <h1 className="text-white tracking-tight mb-2">{t(titleKeys[type])}</h1>
          <p className="text-[#6b6b6b]">{t(countKeys[type], { count: creators.length })}</p>
        </header>

        {/* Circular Avatar Grid */}
        <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-7 2xl:grid-cols-8 gap-x-8 gap-y-12">
          {creators.map((creator) => (
            <button
              key={creator.id}
              onClick={() => onSelectCreator(creator.id)}
              className="group flex flex-col items-center cursor-pointer"
            >
              {/* Circular Photo */}
              <div className="relative w-32 h-32 mb-4 rounded-full overflow-hidden bg-[#2a2a2a] border-2 border-[#3a3a3a] group-hover:border-[#4a4a4a] transition-all">
                {creator.photo ? (
                  <ImageWithFallback
                    src={creator.photo}
                    alt={creator.name}
                    className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center">
                    <span className="text-4xl text-[#4a4a4a] group-hover:text-[#5a5a5a] transition-colors">
                      {creator.name.charAt(0)}
                    </span>
                  </div>
                )}
              </div>

              {/* Name */}
              <h3 className="text-white text-sm text-center mb-1 group-hover:text-[#b3b3b3] transition-colors">
                {creator.name}
              </h3>

              {/* Game Count */}
              {creator.gameCount !== undefined && (
                <p className="text-[#6b6b6b] text-xs">
                  {creator.gameCount} {type === 'series' ? t('creators.works') : t('creators.games')}
                </p>
              )}

              {/* Role (optional) */}
              {creator.role && (
                <p className="text-[#6b6b6b] text-xs mt-0.5">{creator.role}</p>
              )}
            </button>
          ))}
        </div>

        {/* Empty State */}
        {creators.length === 0 && (
          <div className="flex items-center justify-center py-24">
            <div className="text-center">
              <p className="text-[#6b6b6b] text-lg">{t('creators.emptyTitle', { type: typeNames[type] })}</p>
              <p className="text-[#4a4a4a] text-sm mt-2">
                {t('creators.emptyHint', { type: typeNames[type] })}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
