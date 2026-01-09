import { FolderOpen, ChevronDown, ChevronLeft, ChevronRight, Copy, Check, ArrowRight } from 'lucide-react';
import { ImageWithFallback } from '../components/figma/ImageWithFallback';
import { useState, useRef } from 'react';
import { MetadataEditor } from '../components/MetadataEditor';
import { VoiceActorDetail, VoiceActor } from '../components/VoiceActorDetail';
import { CanonicalGame } from './CanonicalDetailPage';
import { AssetTag, ScenarioWriter } from '../mockData';

// Related work for discovery section
interface RelatedWork {
      id: string;
      title: string;
      coverImage: string;
      relation: 'sequel' | 'prequel' | 'spinoff' | 'adaptation' | 'same_developer';
}

interface WorkDetailsPageProps {
      game: CanonicalGame;
      availableAssets?: AssetTag[];
      scenarioWriters?: ScenarioWriter[];
      relatedWorks?: RelatedWork[];
      extendedDescription?: string;
      onBack: () => void;
      onLaunchGame: (path: string) => void;
      onOpenFolder: (path: string) => void;
      onEditMetadata: () => void;
      onViewAllCharacters?: () => void; // New prop for navigation
}

export function WorkDetailsPage({
      game,
      availableAssets = [],
      scenarioWriters = [],
      relatedWorks = [],
      extendedDescription = '',
      onBack,
      onLaunchGame,
      onOpenFolder,
      onEditMetadata,
      onViewAllCharacters
}: WorkDetailsPageProps) {
      const [editorOpen, setEditorOpen] = useState(false);
      const [descriptionExpanded, setDescriptionExpanded] = useState(false);
      const [selectedVoiceActor, setSelectedVoiceActor] = useState<VoiceActor | null>(null);
      const [versionDropdownOpen, setVersionDropdownOpen] = useState(false);
      const [pathCopied, setPathCopied] = useState(false);
      const [showCharacterArrows, setShowCharacterArrows] = useState(false);
      const characterScrollRef = useRef<HTMLDivElement>(null);

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

      const scrollCharacters = (direction: 'left' | 'right') => {
            if (characterScrollRef.current) {
                  const scrollAmount = 300;
                  characterScrollRef.current.scrollBy({
                        left: direction === 'left' ? -scrollAmount : scrollAmount,
                        behavior: 'smooth'
                  });
            }
      };

      // Separate Tier 1 (core leads) and Tier 2 (other contributors)
      const tier1Roles = ['scenario writer', 'founder', 'director', 'lead writer', 'lead artist', 'lead programmer'];
      const allStaff = [
            ...scenarioWriters.map(w => ({ name: w.name, role: 'Scenario Writer', isTier1: true })),
            ...(game.credits || []).map(c => ({
                  name: c.name,
                  role: c.role,
                  isTier1: tier1Roles.some(r => c.role.toLowerCase().includes(r))
            }))
      ];
      const tier1Staff = allStaff.filter(s => s.isTier1);
      const tier2Staff = allStaff.filter(s => !s.isTier1);

      // Character logic for View All entry
      const visibleSlots = 4;
      const characters = game.characters || [];
      const showHeaderViewAll = characters.length >= visibleSlots;
      const showFillerCard = characters.length < visibleSlots && characters.length > 0;

      if (selectedVoiceActor) {
            return (
                  <VoiceActorDetail
                        actor={selectedVoiceActor}
                        onBack={() => setSelectedVoiceActor(null)}
                  />
            );
      }

      return (
            <div className="flex min-h-screen text-white" style={{ backgroundColor: '#0B0C0F' }}>

                  {/* ─────────────────────────────────────────────────────────────
                      GUTTER — 32px Fixed, Non-clickable breathing room
                      ───────────────────────────────────────────────────────────── */}
                  <div
                        className="shrink-0 pointer-events-none"
                        style={{ width: '32px' }}
                  />

                  {/* ─────────────────────────────────────────────────────────────
                      MAIN CONTENT — Max-width 1200px, Left aligned
                      ───────────────────────────────────────────────────────────── */}
                  <div className="flex-1 min-w-0" style={{ maxWidth: '1200px', marginRight: 'auto' }}>

                        {/* ═══════════════════════════════════════════════════════════════
                            HERO SECTION
                            ═══════════════════════════════════════════════════════════════ */}
                        <div className="relative pt-8 pb-16">
                              <div className="flex gap-10">
                                    {/* Cover Image (Left) */}
                                    <div className="shrink-0" style={{ width: '240px' }}>
                                          <div
                                                className="aspect-[3/4] rounded-2xl overflow-hidden"
                                                style={{
                                                      boxShadow: '0 20px 40px rgba(0,0,0,0.5)',
                                                      border: '1px solid rgba(255,255,255,0.08)'
                                                }}
                                          >
                                                <ImageWithFallback
                                                      src={game.coverImage}
                                                      alt={game.title}
                                                      className="w-full h-full object-cover"
                                                />
                                          </div>
                                    </div>

                                    {/* INFO (Right) */}
                                    <div className="flex-1 min-w-0 pt-4">
                                          <h1 className="text-4xl font-light text-white mb-2 tracking-tight">
                                                {game.title}
                                          </h1>

                                          <p className="text-lg text-gray-400 mb-6 font-light">
                                                {game.developer || 'Future Games Studio'}
                                          </p>

                                          <div className="flex items-center gap-3 text-sm text-gray-500 mb-8">
                                                <span className="text-gray-300">{game.releaseYear || 'Unknown'}</span>
                                                <span className="w-1 h-1 rounded-full bg-gray-600"></span>
                                                <span>{game.id || 'VN-ID'}</span>
                                                {game.developer && (
                                                      <>
                                                            <span className="w-1 h-1 rounded-full bg-gray-600"></span>
                                                            <span>{game.developer}</span>
                                                      </>
                                                )}
                                          </div>

                                          {/* Actions */}
                                          <div className="flex items-center gap-3 mb-8">
                                                <button
                                                      onClick={() => game.linkedFiles[0] && onOpenFolder(game.linkedFiles[0])}
                                                      className="h-9 px-4 flex items-center gap-2 rounded-md text-sm text-white transition-colors hover:bg-white/10"
                                                      style={{
                                                            backgroundColor: 'rgba(255,255,255,0.08)',
                                                            border: '1px solid rgba(255,255,255,0.12)'
                                                      }}
                                                >
                                                      <FolderOpen className="w-4 h-4" />
                                                      <span>Open Folder</span>
                                                </button>

                                                <button
                                                      onClick={() => setVersionDropdownOpen(!versionDropdownOpen)}
                                                      className="h-9 px-3 flex items-center gap-2 rounded-md text-xs text-gray-400 hover:text-white transition-colors hover:bg-white/10"
                                                      style={{
                                                            backgroundColor: 'rgba(255,255,255,0.05)',
                                                            border: '1px solid rgba(255,255,255,0.08)'
                                                      }}
                                                >
                                                      Steam Edition <ChevronDown className="w-3 h-3" />
                                                </button>
                                          </div>

                                          {/* Path */}
                                          {game.linkedFiles && game.linkedFiles.length > 0 && (
                                                <div
                                                      onClick={handleCopyPath}
                                                      className="mb-8 p-3 rounded-lg cursor-pointer transition-colors group flex items-center justify-between hover:bg-white/5"
                                                      style={{
                                                            backgroundColor: 'rgba(255,255,255,0.03)',
                                                            border: '1px solid rgba(255,255,255,0.06)',
                                                            maxWidth: '550px'
                                                      }}
                                                >
                                                      <code className="text-xs text-gray-500 font-mono truncate mr-3">
                                                            {game.linkedFiles[0]}
                                                      </code>
                                                      {pathCopied
                                                            ? <Check className="w-3 h-3 text-emerald-500 shrink-0" />
                                                            : <Copy className="w-3 h-3 text-gray-600 group-hover:text-gray-400 shrink-0" />
                                                      }
                                                </div>
                                          )}

                                          {/* Pills */}
                                          {availableAssets.length > 0 && (
                                                <div className="flex flex-wrap gap-2">
                                                      {availableAssets.map((asset, i) => (
                                                            <span
                                                                  key={i}
                                                                  style={{ backgroundColor: 'rgba(255,255,255,0.1)' }}
                                                                  className="px-3 py-1 rounded-full text-xs font-medium text-gray-300 hover:bg-white/20 transition-colors cursor-default"
                                                            >
                                                                  {asset.name}
                                                            </span>
                                                      ))}
                                                </div>
                                          )}
                                    </div>
                              </div>

                              {/* Hero Bottom Fade */}
                              <div
                                    className="absolute bottom-0 left-0 right-0 pointer-events-none"
                                    style={{
                                          height: '80px',
                                          background: 'linear-gradient(to bottom, transparent, #0B0C0F)'
                                    }}
                              />
                        </div>

                        {/* ═══════════════════════════════════════════════════════════════
                            SECTIONS CONTAINER — Spacing Rhythm
                            ═══════════════════════════════════════════════════════════════ */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '56px', paddingBottom: '80px' }}>

                              {/* ─────────────────────────────────────────────────────────────
                                  ABOUT THIS WORK — Layer 1 Container
                                  ───────────────────────────────────────────────────────────── */}
                              <section
                                    className="rounded-[18px]"
                                    style={{
                                          background: 'linear-gradient(to bottom, rgba(255,255,255,0.045), rgba(255,255,255,0.02))',
                                          padding: '28px 32px',
                                          maxWidth: '780px'
                                    }}
                              >
                                    <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-4">
                                          About This Work
                                    </h2>
                                    <h3 className="text-lg font-medium text-white mb-4">Description</h3>

                                    <div className="relative">
                                          <div
                                                className="text-sm leading-relaxed text-gray-300 transition-all duration-300 overflow-hidden"
                                                style={!descriptionExpanded ? {
                                                      maxHeight: '8em',
                                                      maskImage: 'linear-gradient(to bottom, black 0%, black 65%, transparent 100%)',
                                                      WebkitMaskImage: 'linear-gradient(to bottom, black 0%, black 65%, transparent 100%)'
                                                } : {}}
                                          >
                                                {extendedDescription || game.description || 'No description available.'}
                                          </div>
                                          <button
                                                onClick={() => setDescriptionExpanded(!descriptionExpanded)}
                                                className="mt-4 text-sm text-gray-500 hover:text-white transition-colors font-medium flex items-center gap-1"
                                          >
                                                {descriptionExpanded ? 'SHOW LESS' : 'READ MORE'}
                                                <span className="text-[10px]">{descriptionExpanded ? '▴' : '▾'}</span>
                                          </button>
                                    </div>
                              </section>

                              {/* ─────────────────────────────────────────────────────────────
                                  CHARACTERS — Horizontal Scroll
                                  ───────────────────────────────────────────────────────────── */}
                              {characters.length > 0 && (
                                    <section>
                                          <div className="flex items-center justify-between mb-5 pr-8">
                                                <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider">
                                                      Characters
                                                </h2>
                                                {showHeaderViewAll && onViewAllCharacters && (
                                                      <button
                                                            onClick={onViewAllCharacters}
                                                            className="text-xs text-gray-500 hover:text-white transition-colors flex items-center gap-1"
                                                      >
                                                            View all characters <ArrowRight className="w-3 h-3" />
                                                      </button>
                                                )}
                                          </div>

                                          <div
                                                className="relative group/scroll"
                                                onMouseEnter={() => setShowCharacterArrows(true)}
                                                onMouseLeave={() => setShowCharacterArrows(false)}
                                          >
                                                {/* Left Arrow */}
                                                <button
                                                      onClick={() => scrollCharacters('left')}
                                                      className={`absolute left-0 top-1/2 -translate-y-1/2 z-10 w-10 h-10 rounded-full flex items-center justify-center transition-opacity duration-200 ${showCharacterArrows ? 'opacity-100' : 'opacity-0'}`}
                                                      style={{
                                                            backgroundColor: 'rgba(0,0,0,0.7)',
                                                            border: '1px solid rgba(255,255,255,0.1)'
                                                      }}
                                                >
                                                      <ChevronLeft className="w-5 h-5 text-white" />
                                                </button>

                                                {/* Scroll Container */}
                                                <div
                                                      ref={characterScrollRef}
                                                      className="flex gap-4 overflow-x-auto pb-4 -ml-4 pl-4" // slight offset to allow overflow to left visually if needed, but mainly for padding
                                                      style={{
                                                            scrollbarWidth: 'none',
                                                            msOverflowStyle: 'none',
                                                            maskImage: 'linear-gradient(to right, black 95%, transparent 100%)' // optional fade at right edg
                                                      }}
                                                >
                                                      {characters.map((character, i) => (
                                                            <div
                                                                  key={i}
                                                                  className="shrink-0 rounded-2xl overflow-hidden transition-all duration-200 group relative"
                                                                  style={{
                                                                        width: '180px',
                                                                        backgroundColor: 'rgba(255,255,255,0.06)',
                                                                        border: '1px solid rgba(255,255,255,0.10)'
                                                                  }}
                                                            >
                                                                  <div className="aspect-[3/4] overflow-hidden bg-gray-900">
                                                                        <ImageWithFallback
                                                                              src={character.image}
                                                                              alt={character.name}
                                                                              className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                                                                        />
                                                                  </div>
                                                                  <div className="p-3">
                                                                        <div className="font-medium text-white text-sm truncate mb-2">
                                                                              {character.name}
                                                                        </div>

                                                                        {/* CV Badge - Prominent */}
                                                                        {character.voiceActor && (
                                                                              <div
                                                                                    onClick={() => handleVoiceActorClick(character.voiceActor!)}
                                                                                    className="inline-flex items-center gap-1.5 px-2 py-1 rounded bg-white/10 hover:bg-white/20 transition-colors cursor-pointer mb-2"
                                                                              >
                                                                                    <span className="text-[10px]">🎤</span>
                                                                                    <span className="text-xs text-gray-200">{character.voiceActor.name}</span>
                                                                              </div>
                                                                        )}

                                                                        {character.description && (
                                                                              <div className="text-xs text-gray-500 line-clamp-2">
                                                                                    {character.description}
                                                                              </div>
                                                                        )}
                                                                  </div>
                                                            </div>
                                                      ))}

                                                      {/* Filler Card for View All */}
                                                      {showFillerCard && onViewAllCharacters && (
                                                            <button
                                                                  onClick={onViewAllCharacters}
                                                                  className="shrink-0 rounded-2xl overflow-hidden flex flex-col items-center justify-center text-center gap-3 transition-colors hover:bg-white/10"
                                                                  style={{
                                                                        width: '180px',
                                                                        height: 'auto', // match height
                                                                        backgroundColor: 'rgba(255,255,255,0.03)',
                                                                        border: '1px dashed rgba(255,255,255,0.1)'
                                                                  }}
                                                            >
                                                                  <div className="w-10 h-10 rounded-full bg-white/10 flex items-center justify-center">
                                                                        <ArrowRight className="w-5 h-5 text-gray-400" />
                                                                  </div>
                                                                  <span className="text-sm text-gray-400 font-medium">View all<br />characters</span>
                                                            </button>
                                                      )}
                                                </div>

                                                {/* Right Arrow */}
                                                <button
                                                      onClick={() => scrollCharacters('right')}
                                                      className={`absolute right-0 top-1/2 -translate-y-1/2 z-10 w-10 h-10 rounded-full flex items-center justify-center transition-opacity duration-200 ${showCharacterArrows ? 'opacity-100' : 'opacity-0'}`}
                                                      style={{
                                                            backgroundColor: 'rgba(0,0,0,0.7)',
                                                            border: '1px solid rgba(255,255,255,0.1)'
                                                      }}
                                                >
                                                      <ChevronRight className="w-5 h-5 text-white" />
                                                </button>
                                          </div>
                                    </section>
                              )}

                              {/* ─────────────────────────────────────────────────────────────
                                  STAFF & CREDITS
                                  ───────────────────────────────────────────────────────────── */}
                              {allStaff.length > 0 && (
                                    <section>
                                          <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-8">
                                                Staff & Credits
                                          </h2>

                                          {/* Tier 1: Core Leads (Tiles) */}
                                          {tier1Staff.length > 0 && (
                                                <div className="flex flex-wrap gap-8 mb-10">
                                                      {tier1Staff.map((staff, i) => (
                                                            <div key={`t1-${i}`} className="flex items-center gap-4 group cursor-default">
                                                                  {/* Avatar */}
                                                                  <div
                                                                        className="rounded-full flex items-center justify-center text-lg font-medium text-gray-300 shrink-0 shadow-lg"
                                                                        style={{
                                                                              width: '60px',
                                                                              height: '60px',
                                                                              background: 'linear-gradient(135deg, #2A2C35, #15161A)',
                                                                              border: '1px solid rgba(255,255,255,0.1)'
                                                                        }}
                                                                  >
                                                                        {staff.name.charAt(0)}
                                                                  </div>
                                                                  <div>
                                                                        <div className="text-base font-medium text-gray-100 group-hover:text-white transition-colors">{staff.name}</div>
                                                                        <div className="text-xs text-gray-500 uppercase tracking-wide mt-0.5">{staff.role}</div>
                                                                  </div>
                                                            </div>
                                                      ))}
                                                </div>
                                          )}

                                          {/* Tier 2: Contributors (Chips) */}
                                          {tier2Staff.length > 0 && (
                                                <div className="flex flex-wrap gap-x-3 gap-y-3">
                                                      {tier2Staff.map((staff, i) => (
                                                            <div
                                                                  key={`t2-${i}`}
                                                                  className="px-3 py-1.5 rounded-full text-xs transition-colors hover:bg-white/5"
                                                                  style={{ border: '1px solid rgba(255,255,255,0.05)' }}
                                                            >
                                                                  <span className="text-gray-300 font-medium">{staff.name}</span>
                                                                  <span className="text-gray-600 mx-2">·</span>
                                                                  <span className="text-gray-500 uppercase relative top-[0.5px]">{staff.role}</span>
                                                            </div>
                                                      ))}
                                                </div>
                                          )}
                                    </section>
                              )}

                              {/* ─────────────────────────────────────────────────────────────
                                  RELATED WORKS
                                  ───────────────────────────────────────────────────────────── */}
                              {relatedWorks.length > 0 && (
                                    <section>
                                          <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-6">
                                                Related Works
                                          </h2>

                                          <div className="flex gap-4 overflow-x-auto pb-4" style={{ scrollbarWidth: 'none' }}>
                                                {relatedWorks.map((work) => (
                                                      <div
                                                            key={work.id}
                                                            className="shrink-0 rounded-xl overflow-hidden cursor-pointer group transition-all duration-300 hover:-translate-y-1"
                                                            style={{
                                                                  width: '140px',
                                                                  backgroundColor: 'rgba(255,255,255,0.06)',
                                                                  border: '1px solid rgba(255,255,255,0.10)'
                                                            }}
                                                      >
                                                            <div className="aspect-[3/4] overflow-hidden bg-gray-900 relative">
                                                                  <ImageWithFallback
                                                                        src={work.coverImage}
                                                                        alt={work.title}
                                                                        className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                                                                  />
                                                                  <div className="absolute inset-0 bg-black/0 group-hover:bg-black/10 transition-colors" />
                                                            </div>
                                                            <div className="p-3">
                                                                  <div className="text-sm text-gray-200 font-medium truncate mb-1 group-hover:text-white">{work.title}</div>
                                                                  <div className="text-[10px] text-gray-500 uppercase tracking-wide">{work.relation.replace('_', ' ')}</div>
                                                            </div>
                                                      </div>
                                                ))}
                                          </div>
                                    </section>
                              )}
                        </div>
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
