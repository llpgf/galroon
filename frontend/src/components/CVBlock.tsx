import { VoiceActor } from '../types/character.types';

interface CVBlockProps {
  voiceActor: VoiceActor;
}

/**
 * CVBlock - Voice Actor Block Component
 *
 * High visual priority component showing voice actor information
 * with avatar, name, and language.
 *
 * Layout:
 * [圆形头像44px] 名字(18px semibold) 语言(14px muted)
 */
export function CVBlock({ voiceActor }: CVBlockProps) {
  // Generate initials for fallback avatar
  const getInitials = (name: string): string => {
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return name.slice(0, 2).toUpperCase();
  };

  return (
    <div className="flex items-center gap-3 mb-5">
      {/* Label */}
      <span className="text-xs font-semibold text-[#6B6B6B] uppercase tracking-wider">
        VOICE ACTOR
      </span>

      {/* Avatar Circle */}
      <div className="flex items-center gap-3 ml-2">
        {voiceActor.avatar ? (
          <img
            src={voiceActor.avatar}
            alt={voiceActor.name}
            className="shrink-0 w-11 h-11 rounded-full object-cover border border-medium"
          />
        ) : (
          <div
            className="shrink-0 w-11 h-11 rounded-full flex items-center justify-center text-sm font-semibold"
            style={{
              backgroundColor: voiceActor.avatarColor || '#FF9100',
              color: voiceActor.avatarColor ? '#FFFFFF' : '#000000'
            }}
          >
            {getInitials(voiceActor.name)}
          </div>
        )}

        {/* Name + Language */}
        <div className="flex flex-col">
          <span className="text-base font-semibold text-white leading-tight">
            {voiceActor.name}
          </span>
          <span className="text-sm text-[#6B6B6B]">
            {voiceActor.language}
          </span>
        </div>
      </div>
    </div>
  );
}
