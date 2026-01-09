import { useState } from 'react';
import { ArrowLeft, Mic } from 'lucide-react';
import { CanonicalGame } from './CanonicalDetailPage';
import { ImageWithFallback } from '../components/figma/ImageWithFallback';

// shadcn/ui components
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '../components/ui/sheet';
import { Avatar, AvatarFallback, AvatarImage } from '../components/ui/avatar';
import { ScrollArea } from '../components/ui/scroll-area';

// Role colors for badges
const roleColors: Record<string, string> = {
  'Main Heroine': '#6C63FF',
  'Main Character': '#6C63FF',
  'Supporting': '#8B5CF6',
  'Supporting Character': '#8B5CF6',
  'Side Character': '#9B9BA4',
};

/**
 * CharactersPage - VNDB-style character listing
 * 
 * Design tokens:
 * - Page bg: #0E0E11
 * - Card bg: #15151A
 * - Border: rgba(255,255,255,0.08)
 * - Accent: #6C63FF (purple)
 * 
 * Grid: Uses auto-fit to work with sidebar present
 */
export function CharactersPage({ game, onBack }: { game: CanonicalGame; onBack: () => void }) {
  const characters = game.characters || [];

  // State
  const [selectedCharacter, setSelectedCharacter] = useState<(typeof characters)[0] | null>(null);
  const [isSheetOpen, setIsSheetOpen] = useState(false);

  // Handle card click
  const handleCharacterClick = (character: (typeof characters)[0]) => {
    setSelectedCharacter(character);
    setIsSheetOpen(true);
  };

  // Get initials for avatar fallback
  const getInitials = (name: string): string => {
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return name.slice(0, 2).toUpperCase();
  };

  return (
    <div className="min-h-screen gal-char-page">
      {/* Character Detail Sheet */}
      <Sheet open={isSheetOpen} onOpenChange={setIsSheetOpen}>
        <SheetContent
          side="right"
          className="w-[600px] sm:max-w-[600px] gal-char-sheet p-0"
        >
          {selectedCharacter && (
            <ScrollArea className="h-full">
              {/* Hero Image */}
              <div className="relative w-full aspect-[3/4] max-h-[400px]">
                <ImageWithFallback
                  src={selectedCharacter.thumbnailImage}
                  alt={selectedCharacter.name}
                  className="w-full h-full object-cover"
                />
                <div className="absolute inset-0 bg-gradient-to-t from-[#0E0E11] via-transparent to-transparent" />
              </div>

              {/* Content */}
              <div className="p-6 -mt-16 relative z-10">
                <SheetHeader className="p-0 mb-6">
                  <SheetTitle className="text-3xl font-light text-white tracking-tight">
                    {selectedCharacter.name}
                  </SheetTitle>
                  {selectedCharacter.nameRomanization && (
                    <SheetDescription className="text-[#9B9BA4] text-sm">
                      {selectedCharacter.nameRomanization}
                    </SheetDescription>
                  )}
                </SheetHeader>

                {/* Voice Actor Block - HIGH PRIORITY */}
                {selectedCharacter.voiceActor && (
                  <div
                    className="mb-6 p-4 rounded-xl"
                    style={{
                      backgroundColor: 'rgba(108, 99, 255, 0.12)',
                      border: '1px solid rgba(108, 99, 255, 0.25)'
                    }}
                  >
                    <div className="flex items-center gap-2 mb-3 text-[#9B9BA4] text-xs uppercase tracking-wider">
                      <Mic className="w-3 h-3" />
                      <span>Voice Actor</span>
                    </div>
                    <div className="flex items-center gap-4">
                      <Avatar className="w-14 h-14" style={{ border: '2px solid #6C63FF', boxShadow: '0 4px 12px rgba(108, 99, 255, 0.2)' }}>
                        <AvatarFallback
                          className="text-lg font-bold"
                          style={{
                            backgroundColor: selectedCharacter.voiceActor.avatarColor || '#6C63FF',
                            color: '#fff'
                          }}
                        >
                          {getInitials(selectedCharacter.voiceActor.name)}
                        </AvatarFallback>
                      </Avatar>
                      <div>
                        <p className="text-white font-semibold text-lg">{selectedCharacter.voiceActor.name}</p>
                        <p className="text-sm text-[#9B9BA4]">{selectedCharacter.voiceActor.language}</p>
                      </div>
                    </div>
                  </div>
                )}

                {/* Metadata */}
                {selectedCharacter.metadata && (
                  <div className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm mb-6 p-4 bg-[rgba(255,255,255,0.03)] rounded-lg">
                    {selectedCharacter.metadata.gender && (
                      <>
                        <span className="text-[#6B6B6B]">Gender</span>
                        <span className="text-white">{selectedCharacter.metadata.gender}</span>
                      </>
                    )}
                    {selectedCharacter.metadata.age && (
                      <>
                        <span className="text-[#6B6B6B]">Age</span>
                        <span className="text-white">{selectedCharacter.metadata.age}</span>
                      </>
                    )}
                    {selectedCharacter.metadata.role && (
                      <>
                        <span className="text-[#6B6B6B]">Role</span>
                        <span className="text-white">{selectedCharacter.metadata.role}</span>
                      </>
                    )}
                  </div>
                )}

                {/* Description */}
                <div className="prose prose-invert prose-sm max-w-none">
                  <p className="text-[#B3B3B3] leading-relaxed whitespace-pre-line">
                    {selectedCharacter.description}
                  </p>
                </div>
              </div>
            </ScrollArea>
          )}
        </SheetContent>
      </Sheet>

      {/* Top Header Bar - Fixed position with divider */}
      <div className="sticky top-0 z-10 gal-char-topbar">
        <div style={{ maxWidth: '1200px', margin: '0 auto', padding: '0 24px' }}>
          <div className="flex items-center justify-between py-4">
            {/* Left: Back Button - ALWAYS left-aligned */}
            <button
              onClick={onBack}
              className="flex items-center gap-2 px-4 py-2 rounded-lg gal-char-backbtn transition-colors text-[#B3B3B3] hover:text-white"
            >
              <ArrowLeft className="w-4 h-4" />
              <span className="text-sm">Back to Detail</span>
            </button>

            {/* Center: Title */}
            <div className="text-center">
              <h1 className="text-xl font-medium">Characters</h1>
              <p className="text-xs text-[#6B6B6B] mt-0.5">All Characters in This Work</p>
            </div>

            {/* Right: Count */}
            <div className="text-sm text-[#6B6B6B]">
              {characters.length} Characters
            </div>
          </div>
        </div>
      </div>

      {/* Main Content: Auto-fit Grid (works with sidebar) */}
      <div style={{ maxWidth: '1200px', margin: '0 auto', padding: '32px 24px' }}>
        {/* 
          Grid uses auto-fit with minmax to adapt to container width
          This ensures 2 columns even when sidebar is present
          minmax(480px, 1fr) = each card at least 480px, grow to fill
        */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(480px, 1fr))',
            gap: '24px'
          }}
        >
          {characters.map((character) => {
            const role = character.metadata?.role || 'Supporting';
            const roleColor = roleColors[role] || '#9B9BA4';

            return (
              <div
                key={character.id || character.name}
                onClick={() => handleCharacterClick(character)}
                className="gal-char-card cursor-pointer group p-6"
              >
                <div className="flex gap-6">
                  {/* Left: Portrait - fixed 140×187, radius 16 per Figma */}
                  <div style={{ width: '140px', minWidth: '140px', flexShrink: 0 }}>
                    <ImageWithFallback
                      src={character.thumbnailImage}
                      alt={character.name}
                      style={{ width: '140px', height: '187px', borderRadius: '16px', objectFit: 'cover' }}
                    />
                  </div>

                  {/* Right: Content column - fixed order per Figma */}
                  <div className="flex-1 min-w-0 flex flex-col">
                    {/* 1) Header: Name + Role pill same line */}
                    <div className="flex items-center gap-3 flex-wrap">
                      <h3 className="text-xl font-semibold text-white leading-tight">{character.name}</h3>
                      <span
                        className="px-2.5 py-0.5 rounded-full text-xs font-medium whitespace-nowrap"
                        style={{
                          backgroundColor: `${roleColor}20`,
                          color: roleColor,
                        }}
                      >
                        {role}
                      </span>
                    </div>

                    {/* 1b) Sub-name: romanization, dimmer */}
                    {character.nameRomanization && (
                      <p className="text-sm gal-char-label mt-1">{character.nameRomanization}</p>
                    )}

                    {/* 2) VA bar: purple subpanel with circular photo */}
                    {character.voiceActor && (
                      <div className="gal-cv-block flex items-center gap-3 mt-3">
                        <Avatar className="w-10 h-10" style={{ border: '2px solid #6C63FF' }}>
                          {character.voiceActor.avatar ? (
                            <AvatarImage
                              src={character.voiceActor.avatar}
                              alt={character.voiceActor.name}
                              className="object-cover"
                            />
                          ) : null}
                          <AvatarFallback
                            className="text-xs font-bold"
                            style={{
                              backgroundColor: character.voiceActor.avatarColor || '#6C63FF',
                              color: '#fff'
                            }}
                          >
                            {getInitials(character.voiceActor.name)}
                          </AvatarFallback>
                        </Avatar>
                        <div className="min-w-0">
                          <p className="font-medium text-sm text-white truncate">{character.voiceActor.name}</p>
                          <p className="text-xs gal-char-label">{character.voiceActor.language}</p>
                        </div>
                      </div>
                    )}

                    {/* 3) Meta: Gender / Age as rows */}
                    {character.metadata && (
                      <div className="flex flex-col gap-1 mt-3 text-sm">
                        {character.metadata.gender && (
                          <div className="flex justify-between">
                            <span className="gal-char-label">Gender</span>
                            <span className="gal-char-meta">{character.metadata.gender}</span>
                          </div>
                        )}
                        {character.metadata.age && (
                          <div className="flex justify-between">
                            <span className="gal-char-label">Age</span>
                            <span className="gal-char-meta">{character.metadata.age}</span>
                          </div>
                        )}
                      </div>
                    )}

                    {/* 4) Trait chips: VNDB-style character tags */}
                    {character.traits && character.traits.length > 0 && (
                      <div className="flex flex-wrap gap-1.5 mt-3">
                        {character.traits.slice(0, 5).map((trait, idx) => (
                          <span
                            key={idx}
                            className="px-2 py-0.5 rounded-full text-xs"
                            style={{
                              backgroundColor: 'rgba(255, 255, 255, 0.06)',
                              color: 'rgba(255, 255, 255, 0.6)',
                              border: '1px solid rgba(255, 255, 255, 0.08)'
                            }}
                          >
                            {trait}
                          </span>
                        ))}
                        {character.traits.length > 5 && (
                          <span
                            className="px-2 py-0.5 rounded-full text-xs"
                            style={{
                              backgroundColor: 'rgba(108, 99, 255, 0.15)',
                              color: 'rgba(108, 99, 255, 0.8)'
                            }}
                          >
                            +{character.traits.length - 5}
                          </span>
                        )}
                      </div>
                    )}

                    {/* 5) Description: 72% white */}
                    <p className="text-sm gal-char-body leading-relaxed line-clamp-3 mt-2">
                      {character.description}
                    </p>

                    {/* 6) Read more: bottom of card */}
                    <span className="text-sm text-[#6C63FF] hover:underline cursor-pointer mt-3 inline-block group-hover:text-[#8B7FFF]">
                      Read more →
                    </span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
