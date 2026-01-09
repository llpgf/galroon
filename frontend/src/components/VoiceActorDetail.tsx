import { ArrowLeft, Play } from 'lucide-react';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { useState } from 'react';

export interface VoiceActor {
  id: string;
  name: string;
  photo?: string;
  bio?: string;
  birthDate?: string;
  nationality?: string;
  roles?: Array<{
    character: string;
    game: string;
    coverImage?: string;
  }>;
}

interface VoiceActorDetailProps {
  actor: VoiceActor;
  onBack: () => void;
}

export function VoiceActorDetail({ actor, onBack }: VoiceActorDetailProps) {
  const [activeTab, setActiveTab] = useState<'overview' | 'roles'>('overview');

  return (
    <div className="min-h-screen">
      {/* Back Button */}
      <button
        onClick={onBack}
        className="fixed top-6 left-72 z-20 flex items-center gap-2 px-4 py-2 bg-[#1e1e1e]/90 backdrop-blur-sm border border-[#3a3a3a] rounded-lg text-[#b3b3b3] hover:text-white hover:border-[#4a4a4a] transition-all"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={2} />
        <span><small>返回</small></span>
      </button>

      {/* Hero Section - Roon Artist Style */}
      <div className="relative h-[50vh] overflow-hidden">
        {/* Blurred Background */}
        {actor.photo && (
          <div className="absolute inset-0">
            <ImageWithFallback
              src={actor.photo}
              alt={`${actor.name} background`}
              className="w-full h-full object-cover blur-3xl opacity-15"
            />
            <div className="absolute inset-0 bg-gradient-to-b from-[#121212]/50 via-[#121212]/80 to-[#121212]" />
          </div>
        )}

        {/* Hero Content */}
        <div className="relative h-full flex items-end px-12 pb-16">
          <div className="flex gap-10 items-end">
            {/* Photo */}
            {actor.photo && (
              <div className="flex-shrink-0 w-56 h-56 rounded-full overflow-hidden shadow-2xl border-4 border-[#2a2a2a]">
                <ImageWithFallback
                  src={actor.photo}
                  alt={actor.name}
                  className="w-full h-full object-cover"
                />
              </div>
            )}

            {/* Info */}
            <div className="flex flex-col justify-end pb-2">
              <p className="text-[#9ca3af] text-sm mb-2 tracking-wider uppercase">聲優</p>
              <h1 className="text-5xl tracking-tight mb-3">{actor.name}</h1>
              {actor.nationality && (
                <p className="text-[#b3b3b3] text-lg">
                  {actor.nationality}
                  {actor.birthDate && ` · ${actor.birthDate}`}
                </p>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Tabs Navigation */}
      <div className="border-b border-[#2a2a2a] px-12">
        <div className="flex gap-8">
          <button
            onClick={() => setActiveTab('overview')}
            className={`px-2 py-4 transition-colors ${
              activeTab === 'overview'
                ? 'text-white border-b-2 border-white'
                : 'text-[#6b6b6b] hover:text-[#b3b3b3]'
            }`}
          >
            概覽
          </button>
          <button
            onClick={() => setActiveTab('roles')}
            className={`px-2 py-4 transition-colors ${
              activeTab === 'roles'
                ? 'text-white border-b-2 border-white'
                : 'text-[#6b6b6b] hover:text-[#b3b3b3]'
            }`}
          >
            出演作品
          </button>
        </div>
      </div>

      {/* Tab Content */}
      <div className="px-12 py-12">
        <div className="max-w-4xl">
          {/* Overview Tab */}
          {activeTab === 'overview' && (
            <div className="space-y-8">
              {actor.bio && (
                <section>
                  <h2 className="text-white mb-4">簡介</h2>
                  <p className="text-[#b3b3b3] leading-relaxed">{actor.bio}</p>
                </section>
              )}

              {actor.roles && actor.roles.length > 0 && (
                <section>
                  <h2 className="text-white mb-4">代表作品</h2>
                  <div className="grid grid-cols-3 gap-4">
                    {actor.roles.slice(0, 6).map((role, index) => (
                      <div
                        key={index}
                        className="bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg overflow-hidden hover:border-[#3a3a3a] transition-colors cursor-pointer"
                      >
                        {role.coverImage && (
                          <ImageWithFallback
                            src={role.coverImage}
                            alt={role.game}
                            className="w-full aspect-[3/4] object-cover"
                          />
                        )}
                        <div className="p-3">
                          <p className="text-white text-sm mb-1">{role.character}</p>
                          <p className="text-[#6b6b6b] text-xs">{role.game}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                </section>
              )}
            </div>
          )}

          {/* Roles Tab */}
          {activeTab === 'roles' && (
            <section>
              <h2 className="text-white mb-6">全部出演</h2>
              <div className="space-y-3">
                {actor.roles && actor.roles.length > 0 ? (
                  actor.roles.map((role, index) => (
                    <div
                      key={index}
                      className="flex items-center gap-4 p-4 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg hover:border-[#3a3a3a] transition-colors cursor-pointer"
                    >
                      {role.coverImage && (
                        <ImageWithFallback
                          src={role.coverImage}
                          alt={role.game}
                          className="w-16 h-24 object-cover rounded"
                        />
                      )}
                      <div className="flex-1">
                        <h3 className="text-white mb-1">{role.character}</h3>
                        <p className="text-[#6b6b6b] text-sm">{role.game}</p>
                      </div>
                    </div>
                  ))
                ) : (
                  <p className="text-[#6b6b6b] py-4">無出演記錄</p>
                )}
              </div>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
