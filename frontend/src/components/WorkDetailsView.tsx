import { FolderOpen, ChevronDown, Copy, Check } from 'lucide-react';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { useState } from 'react';
import { MetadataEditor } from './MetadataEditor';
import { VoiceActorDetail, VoiceActor } from './VoiceActorDetail';
import { CanonicalGame } from './CanonicalDetailViewEnhanced';
import { AssetTag, ScenarioWriter } from '../mockData';

// Related work for discovery section
interface RelatedWork {
      id: string;
      title: string;
      coverImage: string;
      relation: 'sequel' | 'prequel' | 'spinoff' | 'adaptation' | 'same_developer';
}

interface WorkDetailsViewProps {
      game: CanonicalGame;
      availableAssets?: AssetTag[];
      scenarioWriters?: ScenarioWriter[];
      relatedWorks?: RelatedWork[];
      extendedDescription?: string;
      onBack: () => void;
      onLaunchGame: (path: string) => void;
      onOpenFolder: (path: string) => void;
      onEditMetadata: () => void;
}

export function WorkDetailsView({
      game,
      availableAssets = [],
      scenarioWriters = [],
      relatedWorks = [],
      extendedDescription = '',
      onBack,
      onLaunchGame,
      onOpenFolder,
      onEditMetadata
}: WorkDetailsViewProps) {
      const [editorOpen, setEditorOpen] = useState(false);
      const [descriptionExpanded, setDescriptionExpanded] = useState(false);
      const [selectedVoiceActor, setSelectedVoiceActor] = useState<VoiceActor | null>(null);
      const [selectedVersion, setSelectedVersion] = useState(game.versions?.[0]?.name || 'Steam Ed.');
      const [versionDropdownOpen, setVersionDropdownOpen] = useState(false);
      const [pathCopied, setPathCopied] = useState(false);

      const handleCopyPath = () => {
            if (game.linkedFiles[0]) {
                  navigator.clipboard.writeText(game.linkedFiles[0]);
                  setPathCopied(true);
                  setTimeout(() => setPathCopied(false), 2000);
            }
      };

      const handleVoiceActorClick = (voiceActor: { id: string; name: string }) => {
            const mockActor: VoiceActor = {
                  id: voiceActor.id,
                  name: voiceActor.name,
                  photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400',
                  bio: '知名聲優，擁有豐富的遊戲配音經驗。',
                  nationality: '日本',
                  birthDate: '1990年3月15日',
                  roles: []
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

      const baseDescription = game.description || '';
      const displayDescription = baseDescription.length < 100 && extendedDescription
            ? baseDescription + ' ' + extendedDescription
            : baseDescription || 'No description available.';

      return (
            <div className="min-h-screen text-white relative overflow-hidden">
                  {/* Cinematic gradient background */}
                  <div
                        className="absolute inset-0 pointer-events-none"
                        style={{
                              background: `
            radial-gradient(ellipse 80% 50% at 50% 0%, rgba(59, 130, 246, 0.08) 0%, transparent 50%),
            linear-gradient(to bottom, #0a0a0a 0%, #000000 100%)
          `
                        }}
                  />

                  {/* Content */}
                  <div className="relative z-10 max-w-6xl mx-auto px-8 py-10">

                        {/* ===== HERO SECTION ===== */}
                        <div className="flex gap-10 mb-12">

                              {/* Left: Cover Image */}
                              <div className="flex-shrink-0">
                                    <div
                                          className="w-64 rounded-xl overflow-hidden"
                                          style={{
                                                aspectRatio: '3/4',
                                                boxShadow: '0 25px 50px -12px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05)'
                                          }}
                                    >
                                          <ImageWithFallback
                                                src={game.coverImage || ''}
                                                alt={game.title}
                                                className="w-full h-full object-cover"
                                          />
                                    </div>
                              </div>

                              {/* Right: Title + Metadata + Actions */}
                              <div className="flex-1 pt-4">
                                    {/* Title Block */}
                                    <h1
                                          className="text-4xl text-white mb-2"
                                          style={{ fontWeight: 300, letterSpacing: '0.02em' }}
                                    >
                                          {game.title}
                                    </h1>
                                    {game.developer && (
                                          <p className="text-lg text-gray-500 mb-8">
                                                {game.developer}
                                          </p>
                                    )}

                                    {/* Metadata Row */}
                                    <div className="flex gap-12 mb-8">
                                          <div>
                                                <div className="text-xs text-gray-600 uppercase tracking-wider mb-1">Developer</div>
                                                <div className="text-sm text-gray-300">{game.developer || 'Unknown'}</div>
                                          </div>
                                          <div>
                                                <div className="text-xs text-gray-600 uppercase tracking-wider mb-1">Release Year</div>
                                                <div className="text-sm text-gray-300">{game.releaseYear || 'N/A'}</div>
                                          </div>
                                          <div>
                                                <div className="text-xs text-gray-600 uppercase tracking-wider mb-1">VNDB ID</div>
                                                <div className="text-sm text-gray-300">v{game.id}</div>
                                          </div>
                                    </div>

                                    {/* Action Buttons */}
                                    <div className="flex items-center gap-3 mb-6">
                                          <button
                                                onClick={() => game.linkedFiles[0] && onOpenFolder(game.linkedFiles[0])}
                                                className="h-10 px-5 bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium rounded-lg flex items-center gap-2 transition-colors"
                                          >
                                                <FolderOpen className="w-4 h-4" />
                                                <span>Open Folder</span>
                                          </button>

                                          {game.versions && game.versions.length > 0 && (
                                                <div className="relative">
                                                      <button
                                                            onClick={() => setVersionDropdownOpen(!versionDropdownOpen)}
                                                            className="h-10 px-4 bg-gray-800/80 hover:bg-gray-700/80 text-gray-300 text-sm rounded-lg flex items-center gap-2 transition-colors"
                                                      >
                                                            <span>Version: {selectedVersion}</span>
                                                            <ChevronDown className={`w-4 h-4 transition-transform ${versionDropdownOpen ? 'rotate-180' : ''}`} />
                                                      </button>

                                                      {versionDropdownOpen && (
                                                            <div className="absolute top-full left-0 mt-2 w-52 bg-gray-900/95 backdrop-blur border border-gray-700/50 rounded-lg shadow-2xl z-50 overflow-hidden">
                                                                  {game.versions.map((ver, i) => (
                                                                        <button
                                                                              key={i}
                                                                              onClick={() => {
                                                                                    setSelectedVersion(ver.name);
                                                                                    setVersionDropdownOpen(false);
                                                                              }}
                                                                              className="w-full px-4 py-2.5 text-left text-sm text-gray-300 hover:bg-gray-800 hover:text-white transition-colors"
                                                                        >
                                                                              {ver.name}
                                                                        </button>
                                                                  ))}
                                                            </div>
                                                      )}
                                                </div>
                                          )}
                                    </div>

                                    {/* Install Path */}
                                    {game.linkedFiles && game.linkedFiles.length > 0 && (
                                          <div
                                                onClick={handleCopyPath}
                                                className="flex items-center justify-between bg-gray-900/60 rounded-lg px-4 py-3 cursor-pointer hover:bg-gray-800/60 transition-colors group mb-6"
                                                style={{ border: '1px solid rgba(255,255,255,0.05)' }}
                                          >
                                                <code className="text-sm text-gray-500 font-mono truncate flex-1">
                                                      {game.linkedFiles[0]}
                                                </code>
                                                <div className="ml-4 flex-shrink-0">
                                                      {pathCopied ? (
                                                            <Check className="w-4 h-4 text-emerald-500" />
                                                      ) : (
                                                            <Copy className="w-4 h-4 text-gray-600 group-hover:text-gray-400" />
                                                      )}
                                                </div>
                                          </div>
                                    )}

                                    {/* Available Assets */}
                                    {availableAssets.length > 0 && (
                                          <div>
                                                <div className="text-xs text-gray-600 uppercase tracking-wider mb-3">Available Assets</div>
                                                <div className="flex flex-wrap gap-2">
                                                      {availableAssets.map((asset, i) => (
                                                            <span
                                                                  key={i}
                                                                  style={{ backgroundColor: asset.color }}
                                                                  className="px-3 py-1.5 rounded-full text-xs font-medium text-white"
                                                            >
                                                                  {asset.name}
                                                            </span>
                                                      ))}
                                                </div>
                                          </div>
                                    )}
                              </div>
                        </div>

                        {/* ===== CHARACTERS SECTION (CRITICAL) ===== */}
                        {game.characters && game.characters.length > 0 && (
                              <section className="mb-12">
                                    <h2 className="text-lg font-medium text-white mb-6">Characters</h2>
                                    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-6">
                                          {game.characters.map((character, i) => (
                                                <div
                                                      key={i}
                                                      className="group"
                                                >
                                                      {/* Character Portrait */}
                                                      <div className="aspect-[3/4] rounded-lg overflow-hidden mb-3 bg-gray-900">
                                                            <ImageWithFallback
                                                                  src={character.image}
                                                                  alt={character.name}
                                                                  className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                                                            />
                                                      </div>

                                                      {/* Character Identity: Name + CV inline */}
                                                      <div className="space-y-1">
                                                            {/* Character Name with CV inline */}
                                                            <div className="flex items-baseline gap-1.5 flex-wrap">
                                                                  <span className="text-sm font-medium text-white">{character.name}</span>
                                                                  {character.voiceActor && (
                                                                        <span
                                                                              className="text-xs text-gray-400 hover:text-blue-400 cursor-pointer transition-colors"
                                                                              onClick={() => handleVoiceActorClick(character.voiceActor!)}
                                                                        >
                                                                              · CV {character.voiceActor.name}
                                                                        </span>
                                                                  )}
                                                            </div>

                                                            {/* Role/Description (muted, subordinate) */}
                                                            {character.description && (
                                                                  <div className="text-xs text-gray-600 line-clamp-1">{character.description}</div>
                                                            )}
                                                      </div>
                                                </div>
                                          ))}
                                    </div>
                              </section>
                        )}

                        {/* ===== DESCRIPTION SECTION ===== */}
                        <section className="mb-12">
                              <h2 className="text-lg font-medium text-white mb-4">Description</h2>
                              <div className="max-w-3xl relative">
                                    <p
                                          className={`text-sm text-gray-400 leading-relaxed ${!descriptionExpanded ? 'line-clamp-4' : ''}`}
                                          style={{ lineHeight: '1.8' }}
                                    >
                                          {displayDescription}
                                    </p>

                                    {/* Fade gradient when collapsed */}
                                    {!descriptionExpanded && displayDescription.length > 200 && (
                                          <div
                                                className="absolute bottom-0 left-0 right-0 h-16 pointer-events-none"
                                                style={{
                                                      background: 'linear-gradient(to top, rgba(0,0,0,1) 0%, transparent 100%)'
                                                }}
                                          />
                                    )}

                                    {displayDescription.length > 200 && (
                                          <button
                                                onClick={() => setDescriptionExpanded(!descriptionExpanded)}
                                                className="mt-3 text-blue-400 hover:text-blue-300 text-sm transition-colors"
                                          >
                                                {descriptionExpanded ? 'Show Less' : 'Read More'}
                                          </button>
                                    )}
                              </div>
                        </section>

                        {/* ===== STAFF SECTION ===== */}
                        {game.credits && game.credits.length > 0 && (
                              <section className="mb-12">
                                    <h2 className="text-lg font-medium text-white mb-4">Staff</h2>
                                    <div className="flex flex-wrap gap-3">
                                          {game.credits.map((credit, i) => (
                                                <div
                                                      key={i}
                                                      className="flex items-center gap-3 px-4 py-2.5 rounded-lg"
                                                      style={{
                                                            backgroundColor: 'rgba(255,255,255,0.03)',
                                                            border: '1px solid rgba(255,255,255,0.06)'
                                                      }}
                                                >
                                                      <span className="text-xs text-gray-500">{credit.role}</span>
                                                      <span className="text-sm text-gray-300">{credit.name}</span>
                                                </div>
                                          ))}
                                    </div>
                              </section>
                        )}

                        {/* ===== SCENARIO WRITERS SECTION ===== */}
                        {scenarioWriters.length > 0 && (
                              <section className="mb-12">
                                    <h2 className="text-lg font-medium text-white mb-4">Scenario Writers</h2>
                                    <div className="flex gap-4">
                                          {scenarioWriters.map((writer, i) => (
                                                <div
                                                      key={i}
                                                      className="px-5 py-3 rounded-lg"
                                                      style={{
                                                            backgroundColor: 'rgba(255,255,255,0.03)',
                                                            border: '1px solid rgba(255,255,255,0.06)'
                                                      }}
                                                >
                                                      <div className="text-sm text-white font-medium">{writer.name}</div>
                                                      <div className="text-xs text-gray-500 mt-1">{writer.role}</div>
                                                </div>
                                          ))}
                                    </div>
                              </section>
                        )}

                        {/* ===== RELATED WORKS SECTION ===== */}
                        {relatedWorks.length > 0 && (
                              <section className="mb-12">
                                    <h2 className="text-lg font-medium text-white mb-4">Related Works</h2>
                                    <div className="flex gap-4 overflow-x-auto pb-4">
                                          {relatedWorks.map((work) => (
                                                <div
                                                      key={work.id}
                                                      className="flex-shrink-0 w-32 cursor-pointer group"
                                                >
                                                      <div className="aspect-[3/4] rounded-lg overflow-hidden mb-2 bg-gray-900">
                                                            <ImageWithFallback
                                                                  src={work.coverImage}
                                                                  alt={work.title}
                                                                  className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                                                            />
                                                      </div>
                                                      <div className="text-xs text-gray-400 group-hover:text-white transition-colors truncate">
                                                            {work.title}
                                                      </div>
                                                      <div className="text-xs text-gray-600 capitalize">{work.relation.replace('_', ' ')}</div>
                                                </div>
                                          ))}
                                    </div>
                              </section>
                        )}

                  </div>

                  {/* Metadata Editor Modal */}
                  <MetadataEditor
                        isOpen={editorOpen}
                        game={game}
                        onSave={(data) => { console.log("Saved", data); }}
                        onClose={() => setEditorOpen(false)}
                  />
            </div>
      );
}
