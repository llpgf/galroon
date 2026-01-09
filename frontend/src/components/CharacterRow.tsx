import { useState } from 'react';
import { Character } from '../types/character.types';
import { CVBlock } from './CVBlock';
import { ImageWithFallback } from './figma/ImageWithFallback';

interface CharacterRowProps {
  character: Character;
  index: number; // For alternating background
}

/**
 * CharacterRow - Compact Character List Row
 *
 * VNDB-inspired encyclopedia-style character row.
 * Target height: 220-280px in normal state.
 *
 * Layout:
 * ┌────────────┬──────────────────────────────────────┐
 * │            │ 名字 (36px, Light)                   │
 * │            │ ─────────────────────────────────   │
 * │  150px宽   │ [CV头像44px] 花澤香菜  Japanese      │
 * │  220px高   │ ─────────────────────────────────   │
 * │            │ Role    Main Heroine                  │
 * │            │ Gender  Female                        │
 * │            │ ─────────────────────────────────   │
 * │            │ 描述文字（最多4行，可展开）            │
 * │            │ [Read more ▾]                        │
 * └────────────┴──────────────────────────────────────┘
 */
export function CharacterRow({ character, index }: CharacterRowProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  // Toggle expand/collapse
  const toggleExpand = () => {
    setIsExpanded(prev => !prev);
  };

  // Check if description is long enough to need expand
  const needsExpand = character.description.length > 200;
  const shouldShowExpand = needsExpand || isExpanded;

  return (
    <div
      className={`
        relative flex gap-6 transition-colors
        ${index % 2 === 0 ? 'bg-transparent' : 'bg-[rgba(255,255,255,0.015)]'}
      `}
    >
      {/* LEFT: Thumbnail Portrait */}
      {/* Fixed: 150px width, 220px height, object-fit: cover */}
      <div className="shrink-0">
        <div className="relative w-[150px] h-[220px] rounded-2xl overflow-hidden bg-[#1A1A1A] border border-subtle shadow-xl">
          <ImageWithFallback
            src={character.thumbnailImage}
            alt={character.name}
            className="w-full h-full object-cover"
          />
        </div>
      </div>

      {/* RIGHT: Dense Info Column */}
      <div className="flex-1 min-w-0 py-1">
        {/* A) Name Row */}
        <div className="mb-4">
          <h3 className="text-4xl font-light text-white tracking-tight leading-[1.1]">
            {character.name}
          </h3>
          {character.nameRomanization && (
            <p className="text-sm text-[#6B6B6B] mt-1">
              {character.nameRomanization}
            </p>
          )}
        </div>

        {/* B) CV Block (HIGH PRIORITY) */}
        {character.voiceActor && (
          <CVBlock voiceActor={character.voiceActor} />
        )}

        {/* C) Metadata (VNDB-style definition list) */}
        {character.metadata && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 mb-5">
            {character.metadata.role && (
              <>
                <dt className="text-sm text-[#6B6B6B]">Role</dt>
                <dd className="text-sm text-white">{character.metadata.role}</dd>
              </>
            )}

            {character.metadata.gender && (
              <>
                <dt className="text-sm text-[#6B6B6B]">Gender</dt>
                <dd className="text-sm text-white">{character.metadata.gender}</dd>
              </>
            )}

            {character.metadata.age && (
              <>
                <dt className="text-sm text-[#6B6B6B]">Age</dt>
                <dd className="text-sm text-white">{character.metadata.age}</dd>
              </>
            )}

            {character.metadata.affiliation && (
              <>
                <dt className="text-sm text-[#6B6B6B]">Affiliation</dt>
                <dd className="text-sm text-white">{character.metadata.affiliation}</dd>
              </>
            )}
          </dl>
        )}

        {/* D) Description (with expand/collapse) */}
        <div className="relative">
          <div
            className={`
              text-sm text-[#B3B3B3] leading-relaxed
              transition-all duration-300
              ${!isExpanded ? 'line-clamp-4' : 'line-clamp-none'}
            `}
          >
            {character.description}
          </div>

          {/* Expand/Collapse Toggle */}
          {shouldShowExpand && (
            <button
              onClick={toggleExpand}
              className="mt-3 text-sm text-[#6B6B6B] hover:text-white transition-colors font-medium inline-flex items-center gap-1 cursor-pointer focus:outline-none"
            >
              {isExpanded ? (
                <>
                  Show less
                  <span className="text-xs">▴</span>
                </>
              ) : (
                <>
                  Read more
                  <span className="text-xs">▾</span>
                </>
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
