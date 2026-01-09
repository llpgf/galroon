/**
 * Character type definitions for Characters Page
 * Encyclopedia-style, VNDB-inspired character list
 */

export interface VoiceActor {
  id: string;
  name: string;
  language: string; // e.g., "Japanese", "English"
  avatar?: string; // 40-48px circular avatar URL
  avatarColor?: string; // Fallback color if no avatar (e.g., "#FF9100")
}

export interface CharacterMetadata {
  role?: string; // e.g., "Main Heroine", "Supporting Character"
  gender?: string; // e.g., "Female", "Male"
  age?: string; // e.g., "17", "Unknown"
  affiliation?: string; // e.g., "Neo-Tokyo Security Bureau"
}

export interface Character {
  id: string;
  name: string;
  nameRomanization?: string; // Optional romanized name
  metadata: CharacterMetadata;
  voiceActor?: VoiceActor;
  description: string;
  thumbnailImage: string; // 140-170px width thumbnail
  fullImage?: string; // Full standing art (for lightbox/detail)
}

export interface CharactersPageProps {
  characters: Character[];
  parentGameTitle: string; // e.g., "Neon Dystopia"
  onBack: () => void;
}
