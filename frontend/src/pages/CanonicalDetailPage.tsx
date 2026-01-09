import { FolderOpen, ArrowLeft, Play, Edit, MoreHorizontal, Heart, Copy, ChevronDown, ChevronUp, Mic } from 'lucide-react';
import { ImageWithFallback } from '../components/figma/ImageWithFallback';
import { useState } from 'react';
import type { MetadataDraft } from '../types/metadata';
import { MetadataEditor } from '../components/MetadataEditor';
import { VoiceActorDetail, VoiceActor } from '../components/VoiceActorDetail';

export interface CharacterMetadata {
  role?: string;
  gender?: string;
  age?: string;
  affiliation?: string;
}

export interface CharacterVoiceActor {
  id: string;
  name: string;
  language?: string;
  avatar?: string;  // Circular avatar photo URL
  avatarColor?: string;
}

export interface Character {
  id?: string;
  name: string;
  nameRomanization?: string;
  thumbnailImage?: string;
  image?: string;
  description: string;
  voiceActor?: CharacterVoiceActor;
  metadata?: CharacterMetadata;
  traits?: string[];  // VNDB-style tags: Hair, Eyes, Body, Clothes, Personality, Role values
}

export interface CanonicalGame {
  id: string;
  title: string;
  developer?: string;
  releaseYear?: string;
  coverImage?: string;
  linkedFiles: string[];
  description?: string;
  tags?: string[];
  characters?: Character[];
  versions?: Array<{
    name: string;
    path: string;
    platform: string;
  }>;
  credits?: Array<{
    role: string;
    name: string;
  }>;
}

interface CanonicalDetailPageProps {
  game: CanonicalGame;
  onBack: () => void;
  onLaunchGame: (path: string) => void;
  onOpenFolder: (path: string) => void;
  onEditMetadata: () => void;
}

type Tab = 'overview' | 'versions' | 'credits';

export function CanonicalDetailPage({
  game,
  onBack,
  onLaunchGame,
  onOpenFolder,
  onEditMetadata
}: CanonicalDetailPageProps) {
  const [activeTab, setActiveTab] = useState<Tab>('overview');
  const [showMoreMenu, setShowMoreMenu] = useState(false);
  const [isFavorite, setIsFavorite] = useState(false);
  const [editorOpen, setEditorOpen] = useState(false);
  const [descriptionExpanded, setDescriptionExpanded] = useState(false);
  const [expandedCharacters, setExpandedCharacters] = useState<Set<number>>(new Set());
  const [selectedVoiceActor, setSelectedVoiceActor] = useState<VoiceActor | null>(null);

  const handleSaveMetadata = (data: MetadataDraft) => {
    console.log('Saving metadata:', data);
    // In a real app, this would update the game data
  };

  const toggleCharacterExpansion = (index: number) => {
    const newExpanded = new Set(expandedCharacters);
    if (newExpanded.has(index)) {
      newExpanded.delete(index);
    } else {
      newExpanded.add(index);
    }
    setExpandedCharacters(newExpanded);
  };

  const handleVoiceActorClick = (actorId: string, actorName: string) => {
    // Mock voice actor data - in real app, fetch from database
    const mockActor: VoiceActor = {
      id: actorId,
      name: actorName,
      photo: 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=400',
      bio: '知名聲優，擁有豐富的遊戲配音經驗。聲線多變，能夠演繹各種類型的角色。代表作品包括多部知名視覺小說和動畫作品。',
      nationality: '日本',
      birthDate: '1990年3月15日',
      roles: [
        {
          character: '櫻井美咲',
          game: 'Neon Dystopia',
          coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400'
        },
        {
          character: '神崎葵',
          game: 'Summer Dreams',
          coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400'
        }
      ]
    };
    setSelectedVoiceActor(mockActor);
  };

  if (selectedVoiceActor) {
    return (
      <VoiceActorDetail
        actor={selectedVoiceActor}
        onBack={() => setSelectedVoiceActor(null)}
      />
    );
  }

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
      <div className="relative min-h-[70vh] overflow-hidden">
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
        <div className="relative px-12 pt-20 pb-12">
          {/* Title First */}
          <div className="mb-12">
            <h1 className="text-6xl tracking-tight mb-4">{game.title}</h1>
            {(game.developer || game.releaseYear) && (
              <p className="text-[#b3b3b3] text-xl">
                {[game.developer, game.releaseYear].filter(Boolean).join(' · ')}
              </p>
            )}
          </div>

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

            {/* Right Column: About + Actions */}
            <div className="flex-1 flex flex-col gap-8">
              {/* About Section */}
              {game.description && (
                <div className="bg-[#1e1e1e]/60 backdrop-blur-sm border border-[#2a2a2a] rounded-lg p-6">
                  <h2 className="text-white text-lg mb-4">About</h2>
                  <div className="relative">
                    <p className={`text-[#b3b3b3] leading-relaxed ${!descriptionExpanded && 'line-clamp-4'}`}>
                      {game.description}
                    </p>
                    {game.description.length > 200 && (
                      <button
                        onClick={() => setDescriptionExpanded(!descriptionExpanded)}
                        className="flex items-center gap-2 mt-3 text-[#9ca3af] hover:text-white transition-colors"
                      >
                        {descriptionExpanded ? (
                          <>
                            <ChevronUp className="w-4 h-4" strokeWidth={2} />
                            <span className="text-sm">收起</span>
                          </>
                        ) : (
                          <>
                            <ChevronDown className="w-4 h-4" strokeWidth={2} />
                            <span className="text-sm">展開全文</span>
                          </>
                        )}
                      </button>
                    )}
                  </div>
                </div>
              )}

              {/* Action Strip */}
              <div className="flex gap-3">
                {/* Open Folder */}
                <button
                  onClick={() => game.linkedFiles[0] && onOpenFolder(game.linkedFiles[0])}
                  className="flex items-center gap-2 px-6 py-4 border-2 border-white text-white rounded-lg hover:bg-white hover:text-black transition-colors"
                >
                  <FolderOpen className="w-5 h-5" strokeWidth={2} />
                  <span>Open Folder</span>
                </button>

                {/* Edit Metadata */}
                <button
                  onClick={() => setEditorOpen(true)}
                  className="flex items-center gap-2 px-6 py-4 border-2 border-white text-white rounded-lg hover:bg-white hover:text-black transition-colors"
                >
                  <Edit className="w-5 h-5" strokeWidth={2} />
                  <span>Edit Metadata</span>
                </button>

                {/* More Menu */}
                <div className="relative">
                  <button
                    onClick={() => setShowMoreMenu(!showMoreMenu)}
                    className="flex items-center justify-center w-14 h-14 border-2 border-white text-white rounded-lg hover:bg-white hover:text-black transition-colors"
                  >
                    <MoreHorizontal className="w-6 h-6" strokeWidth={2} />
                  </button>

                  {showMoreMenu && (
                    <div className="absolute top-full mt-2 right-0 w-48 bg-[#1e1e1e] border border-[#3a3a3a] rounded-lg shadow-2xl overflow-hidden z-10">
                      <button className="w-full px-4 py-3 text-left text-[#b3b3b3] hover:bg-[#2a2a2a] hover:text-white transition-colors">
                        Add to Collection
                      </button>
                      <button className="w-full px-4 py-3 text-left text-[#b3b3b3] hover:bg-[#2a2a2a] hover:text-white transition-colors">
                        Refresh Metadata
                      </button>
                      <button className="w-full px-4 py-3 text-left text-red-400 hover:bg-[#2a2a2a] hover:text-red-300 transition-colors border-t border-[#2a2a2a]">
                        Delete Entry
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Tabs Navigation */}
      <div className="border-b border-[#2a2a2a] px-12">
        <div className="flex gap-8">
          <button
            onClick={() => setActiveTab('overview')}
            className={`px-2 py-4 transition-colors ${activeTab === 'overview'
              ? 'text-white border-b-2 border-white'
              : 'text-[#6b6b6b] hover:text-[#b3b3b3]'
              }`}
          >
            Overview
          </button>
          <button
            onClick={() => setActiveTab('versions')}
            className={`px-2 py-4 transition-colors ${activeTab === 'versions'
              ? 'text-white border-b-2 border-white'
              : 'text-[#6b6b6b] hover:text-[#b3b3b3]'
              }`}
          >
            Versions
          </button>
          <button
            onClick={() => setActiveTab('credits')}
            className={`px-2 py-4 transition-colors ${activeTab === 'credits'
              ? 'text-white border-b-2 border-white'
              : 'text-[#6b6b6b] hover:text-[#b3b3b3]'
              }`}
          >
            Credits
          </button>
        </div>
      </div>

      {/* Tab Content */}
      <div className="px-12 py-12">
        <div className="max-w-4xl">
          {/* Overview Tab */}
          {activeTab === 'overview' && (
            <div className="space-y-8">
              <section>
                <h2 className="text-white mb-6">Linked Files</h2>
                <div className="space-y-2">
                  {game.linkedFiles.map((path, index) => (
                    <div
                      key={index}
                      className="flex items-center gap-4 p-4 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg"
                    >
                      <FolderOpen className="w-5 h-5 text-[#6b6b6b]" strokeWidth={1.5} />
                      <code className="text-[#b3b3b3] flex-1 text-sm">{path}</code>
                    </div>
                  ))}
                </div>
              </section>

              {game.characters && game.characters.length > 0 && (
                <section>
                  <h2 className="text-white mb-6">角色簡介</h2>
                  <div className="space-y-4">
                    {game.characters.map((character, index) => {
                      const isExpanded = expandedCharacters.has(index);
                      const needsTruncate = character.description.length > 150;

                      return (
                        <div
                          key={index}
                          className="flex gap-6 p-6 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg hover:border-[#3a3a3a] transition-colors"
                        >
                          {/* Character Image */}
                          {character.image && (
                            <div className="flex-shrink-0 w-32 h-32 rounded-lg overflow-hidden">
                              <ImageWithFallback
                                src={character.image}
                                alt={character.name}
                                className="w-full h-full object-cover"
                              />
                            </div>
                          )}

                          {/* Character Info */}
                          <div className="flex-1 flex flex-col">
                            <h3 className="text-white text-lg mb-3">{character.name}</h3>

                            {/* Description with expand/collapse */}
                            <div className="mb-4">
                              <p className={`text-[#b3b3b3] leading-relaxed text-sm ${!isExpanded && needsTruncate ? 'line-clamp-3' : ''}`}>
                                {character.description}
                              </p>
                              {needsTruncate && (
                                <button
                                  onClick={() => toggleCharacterExpansion(index)}
                                  className="flex items-center gap-1 mt-2 text-[#9ca3af] hover:text-white transition-colors text-sm"
                                >
                                  {isExpanded ? (
                                    <>
                                      <ChevronUp className="w-4 h-4" strokeWidth={2} />
                                      <span>收起</span>
                                    </>
                                  ) : (
                                    <>
                                      <ChevronDown className="w-4 h-4" strokeWidth={2} />
                                      <span>展開全文</span>
                                    </>
                                  )}
                                </button>
                              )}
                            </div>

                            {/* Voice Actor */}
                            {(() => {
                              const voiceActor = character.voiceActor;
                              if (!voiceActor) return null;
                              return (
                                <div className="mt-auto">
                                  <button
                                    onClick={() => handleVoiceActorClick(voiceActor.id, voiceActor.name)}
                                    className="flex items-center gap-2 text-[#9ca3af] hover:text-white transition-colors group"
                                  >
                                    <Mic className="w-4 h-4" strokeWidth={1.5} />
                                    <span className="text-sm">CV: {voiceActor.name}</span>
                                  </button>
                                </div>
                              );
                            })()}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </section>
              )}
            </div>
          )}

          {/* Versions Tab */}
          {activeTab === 'versions' && (
            <section>
              <h2 className="text-white mb-6">Game Versions</h2>
              <div className="space-y-3">
                {game.versions && game.versions.length > 0 ? (
                  game.versions.map((version, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between p-5 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg hover:border-[#3a3a3a] transition-colors"
                    >
                      <div>
                        <h3 className="text-white mb-1">{version.name}</h3>
                        <code className="text-[#6b6b6b]"><small>{version.path}</small></code>
                      </div>
                      <div className="px-3 py-1 bg-[#2a2a2a] border border-[#3a3a3a] rounded">
                        <small className="text-[#b3b3b3]">{version.platform}</small>
                      </div>
                    </div>
                  ))
                ) : (
                  <p className="text-[#6b6b6b] py-4">No version information available</p>
                )}
              </div>
            </section>
          )}

          {/* Credits Tab */}
          {activeTab === 'credits' && (
            <section>
              <h2 className="text-white mb-6">Credits</h2>
              <div className="space-y-6">
                {game.credits && game.credits.length > 0 ? (
                  game.credits.map((credit, index) => (
                    <div key={index}>
                      <small className="text-[#6b6b6b] block mb-1">{credit.role}</small>
                      <p className="text-white">{credit.name}</p>
                    </div>
                  ))
                ) : (
                  <p className="text-[#6b6b6b] py-4">No credits information available</p>
                )}
              </div>
            </section>
          )}
        </div>
      </div>

      {/* Metadata Editor */}
      <MetadataEditor
        isOpen={editorOpen}
        game={game}
        onSave={handleSaveMetadata}
        onClose={() => setEditorOpen(false)}
      />
    </div>
  );
}
