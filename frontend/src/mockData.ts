/**
 * Mock Data for Development
 * 
 * This file contains all mock data extracted from App.tsx.
 * Used as development fallback when API is unavailable.
 * 
 * In production, components should fetch data from API endpoints.
 */

import { ClusterItem } from './pages/InboxPage';
import { Collection } from './pages/CollectionsPage';
import { CanonicalGame } from './pages/CanonicalDetailPage';
import { ClusterDecision } from './pages/ClusterDecisionPage';
import { Creator } from './pages/CreatorsPage';
import { GameType } from './pages/GameTypesPage';
import { TagWithGames } from './pages/TagsPage';
import { GalleryItem } from './pages/GalleryPage';
import { HeroSlide } from './components/HeroCarousel';
import { WorkshopItem } from './components/WorkshopView';
import { GameCardData } from './types/GameCard';

// Library entries (for main grid)
export const mockLibraryData: (GameCardData & { id: string; featured?: boolean })[] = [
      {
            id: '1',
            entry_type: 'canonical',
            display_title: 'Neon Dystopia',
            cover_image: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxjeWJlcnB1bmslMjBnYW1lJTIwcG9zdGVyfGVufDF8fHx8MTc2NzU5MzE0Mnww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 3,
            actions_allowed: 'NONE',
            featured: true
      },
      {
            id: '2',
            entry_type: 'canonical',
            display_title: 'Mystic Realms',
            cover_image: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxmYW50YXN5JTIwZ2FtZSUyMGFydHxlbnwxfHx8fDE3Njc1OTMxODF8MA&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 2,
            actions_allowed: 'NONE'
      },
      {
            id: '3',
            entry_type: 'suggested',
            display_title: 'Suggested Match',
            instance_count: 4,
            actions_allowed: 'NONE'
      },
      {
            id: '4',
            entry_type: 'canonical',
            display_title: 'Velocity Overdrive',
            cover_image: 'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxyYWNpbmclMjBnYW1lJTIwcG9zdGVyfGVufDF8fHx8MTc2NzU5MzE4Mnww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 1,
            actions_allowed: 'NONE'
      },
      {
            id: '5',
            entry_type: 'orphan',
            display_title: 'D:/Games/Unknown/install_v2/GameSetup.exe',
            actions_allowed: 'NONE'
      },
      {
            id: '6',
            entry_type: 'canonical',
            display_title: 'Wilderness Chronicles',
            cover_image: 'https://images.unsplash.com/photo-1765706729543-348de9e073b1?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxhZHZlbnR1cmUlMjBnYW1lJTIwbGFuZHNjYXBlfGVufDF8fHx8MTc2NzU5MzE4Mnww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 2,
            actions_allowed: 'NONE'
      },
      {
            id: '7',
            entry_type: 'canonical',
            display_title: 'Stellar Odyssey',
            cover_image: 'https://images.unsplash.com/photo-1656381620321-bddff61435c3?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxzcGFjZSUyMGdhbWUlMjBhcnR3b3JrfGVufDF8fHx8MTc2NzU5MzE4Mnww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 5,
            actions_allowed: 'NONE',
            featured: true
      },
      {
            id: '8',
            entry_type: 'orphan',
            display_title: 'E:/SteamLibrary/common/untitled/launcher.dat',
            actions_allowed: 'NONE'
      },
      {
            id: '9',
            entry_type: 'canonical',
            display_title: 'Apex Legends',
            cover_image: 'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxhY3Rpb24lMjBnYW1lJTIwcG9zdGVyfGVufDF8fHx8MTc2NzU5MzE0M3ww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 4,
            actions_allowed: 'NONE'
      },
      {
            id: '10',
            entry_type: 'canonical',
            display_title: 'Dark Protocol',
            cover_image: 'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxkYXJrJTIwc2NpZmklMjBnYW1lfGVufDF8fHx8MTc2NzU5MzY5Mnww&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 2,
            actions_allowed: 'NONE'
      },
      {
            id: '11',
            entry_type: 'canonical',
            display_title: 'Silent Hollow',
            cover_image: 'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxob3Jyb3IlMjBnYW1lJTIwYXRtb3NwaGVyZXxlbnwxfHx8fDE3Njc1OTMxNDN8MA&ixlib=rb-4.1.0&q=80&w=1080',
            instance_count: 1,
            actions_allowed: 'NONE'
      },
      {
            id: '12',
            entry_type: 'suggested',
            display_title: 'Suggested Match',
            instance_count: 7,
            actions_allowed: 'NONE'
      }
];

// Inbox clusters
export const mockInboxClusters: ClusterItem[] = [
      { id: 'cluster-1', suggestedTitle: 'Elden Ring', confidence: 98, fileCount: 127 },
      { id: 'cluster-2', suggestedTitle: 'Red Dead Redemption 2', confidence: 95, fileCount: 203 },
      { id: 'cluster-3', suggestedTitle: 'Cyberpunk 2077', confidence: 92, fileCount: 156 },
      { id: 'cluster-4', suggestedTitle: 'The Witcher 3', confidence: 89, fileCount: 184 }
];

// Collections
export const mockCollections: Collection[] = [
      {
            id: 'coll-1',
            name: 'Favorites',
            gameCount: 8,
            coverImages: [
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400'
            ]
      },
      {
            id: 'coll-2',
            name: 'Currently Playing',
            gameCount: 3,
            coverImages: [
                  'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400',
                  'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400'
            ]
      },
      {
            id: 'coll-3',
            name: 'RPG Collection',
            gameCount: 12,
            coverImages: [
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400',
                  'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400'
            ]
      }
];

// Canonical game detail
export const mockCanonicalGame: CanonicalGame = {
      id: '1',
      title: 'Neon Dystopia',
      developer: 'Future Games Studio',
      releaseYear: '2024',
      coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxjeWJlcnB1bmslMjBnYW1lJTIwcG9zdGVyfGVufDF8fHx8MTc2NzU5MzE0Mnww&ixlib=rb-4.1.0&q=80&w=1080',
      description: 'Dive into a breathtaking cyberpunk universe where technology and humanity collide.',
      tags: ['賽博龐克', 'RPG', '冒險', '單人'],
      linkedFiles: [
            'D:/Games/NeonDystopia/NeonDystopia.exe',
            'E:/SteamLibrary/steamapps/common/NeonDystopia/game.exe',
            'C:/Program Files/Epic Games/NeonDystopia/Binaries/Win64/NeonDystopia.exe'
      ],
      characters: [
            {
                  id: 'char-1',
                  name: '櫻井美咲',
                  nameRomanization: 'Sakura Misaki',
                  thumbnailImage: 'https://images.unsplash.com/photo-1697059172415-f1e08f9151bb?w=400',
                  image: 'https://images.unsplash.com/photo-1697059172415-f1e08f9151bb?w=400',
                  description: '主角的青梅竹馬，擁有明亮的笑容和溫柔的性格。She is often seen wandering the district boundaries, looking for something she lost long ago. Her quiet demeanor hides a sharp tactical mind, making her an invaluable ally in the field. Despite her reserved nature, she cares deeply for her team.',
                  voiceActor: {
                        id: 'va-1',
                        name: '花澤香菜',
                        language: 'Japanese',
                        avatar: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400',
                        avatarColor: '#FF9100'
                  },
                  metadata: {
                        role: 'Main Heroine',
                        gender: 'Female',
                        age: '17',
                        affiliation: 'Neo-Tokyo Security Bureau'
                  },
                  traits: ['Brown Hair', 'Long Hair', 'Brown Eyes', 'Slim', 'Teen', 'School Uniform', 'Cheerful', 'Kind']
            },
            {
                  id: 'char-2',
                  name: '神崎葵',
                  nameRomanization: 'Kanzaki Aoi',
                  thumbnailImage: 'https://images.unsplash.com/photo-1623252729328-9941b271ad48?w=400',
                  image: 'https://images.unsplash.com/photo-1623252729328-9941b271ad48?w=400',
                  description: '神秘的駭客，擁有卓越的技術能力。A mysterious hacker with exceptional technical skills, she can breach almost any system in seconds. Her past remains shrouded in secrecy, but her loyalty to her friends is unquestionable. She specializes in information warfare and electronic countermeasures.',
                  voiceActor: {
                        id: 'va-2',
                        name: '早見沙織',
                        language: 'Japanese',
                        avatar: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400',
                        avatarColor: '#689F38'
                  },
                  metadata: {
                        role: 'Main Heroine',
                        gender: 'Female',
                        age: '19',
                        affiliation: 'Independent'
                  },
                  traits: ['Black Hair', 'Short Hair', 'Blue Eyes', 'Pale', 'Slim', 'Glasses', 'Hacker', 'Mysterious']
            },
            {
                  id: 'char-3',
                  name: '雷克斯',
                  nameRomanization: 'Rex',
                  thumbnailImage: 'https://images.unsplash.com/photo-1695747003335-ac77eeea43c2?w=400',
                  image: 'https://images.unsplash.com/photo-1695747003335-ac77eeea43c2?w=400',
                  description: '賽博改造戰士，身體的大部分已被機械取代。A cybernetically enhanced warrior with most of his body replaced by machinery. He struggles with maintaining his humanity while embracing his enhanced capabilities. His combat experience is unmatched, and his mechanical enhancements provide tactical advantages in any situation.',
                  voiceActor: {
                        id: 'va-3',
                        name: '中村悠一',
                        language: 'Japanese',
                        avatar: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400',
                        avatarColor: '#7986CB'
                  },
                  metadata: {
                        role: 'Main Heroine',
                        gender: 'Female',
                        age: 'Unknown',
                        affiliation: 'Neo-Tokyo Defense Force'
                  },
                  traits: ['Cyborg', 'Silver Hair', 'Red Eyes', 'Muscular', 'Adult', 'Combat Suit', 'Stoic', 'Veteran']
            }
      ],
      versions: [
            { name: 'Steam Edition', path: 'E:/SteamLibrary/steamapps/common/NeonDystopia/game.exe', platform: 'Windows' },
            { name: 'Epic Games', path: 'C:/Program Files/Epic Games/NeonDystopia/Binaries/Win64/NeonDystopia.exe', platform: 'Windows' },
            { name: 'Standalone', path: 'D:/Games/NeonDystopia/NeonDystopia.exe', platform: 'Windows' }
      ],
      credits: [
            { role: 'Director', name: 'Alex Chen' },
            { role: 'Lead Designer', name: 'Sarah Martinez' },
            { role: 'Lead Programmer', name: 'David Kim' },
            { role: 'Art Director', name: 'Emily Rodriguez' },
            { role: 'Composer', name: 'Marcus Thompson' }
      ]
};

// Cluster decision
export const mockClusterDecision: ClusterDecision = {
      id: 'cluster-1',
      detectedFolder: 'D:/Games/EldenRing_Setup/ELDEN_RING',
      fileSize: '48.6 GB',
      fileCount: 127,
      suggestedTitle: 'Elden Ring',
      suggestedCover: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?crop=entropy&cs=tinysrgb&fit=max&fm=jpg&ixid=M3w3Nzg4Nzd8MHwxfHNlYXJjaHwxfHxmYW50YXN5JTIwZ2FtZSUyMGFydHxlbnwxfHx8fDE3Njc1OTMxODF8MA&ixlib=rb-4.1.0&q=80&w=1080',
      developer: 'FromSoftware',
      releaseYear: '2022',
      confidence: 98
};

// Creators
export const mockVoiceActors: Creator[] = [
      { id: 'va-1', name: '花澤香菜', photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400', gameCount: 12 },
      { id: 'va-2', name: '早見沙織', photo: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400', gameCount: 8 },
      { id: 'va-3', name: '中村悠一', photo: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400', gameCount: 15 },
      { id: 'va-4', name: '釘宮理惠', photo: 'https://images.unsplash.com/photo-1580971739182-ccd8cfef3707?w=400', gameCount: 10 },
      { id: 'va-5', name: '宮野真守', photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400', gameCount: 14 },
      { id: 'va-6', name: '坂本真綾', photo: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400', gameCount: 9 },
];

export const mockArtists: Creator[] = [
      { id: 'art-1', name: 'Yoshitaka Amano', photo: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400', gameCount: 7 },
      { id: 'art-2', name: '天野喜孝', photo: 'https://images.unsplash.com/photo-1580971739182-ccd8cfef3707?w=400', gameCount: 5 },
      { id: 'art-3', name: 'Tetsuya Nomura', photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400', gameCount: 11 },
      { id: 'art-4', name: '野村哲也', photo: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400', gameCount: 8 },
];

export const mockWriters: Creator[] = [
      { id: 'wr-1', name: 'Yoko Taro', photo: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400', gameCount: 6 },
      { id: 'wr-2', name: '橫尾太郎', photo: 'https://images.unsplash.com/photo-1580971739182-ccd8cfef3707?w=400', gameCount: 4 },
      { id: 'wr-3', name: 'Kazushige Nojima', photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400', gameCount: 9 },
      { id: 'wr-4', name: '野島一成', photo: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400', gameCount: 7 },
];

export const mockComposers: Creator[] = [
      { id: 'com-1', name: '崎元仁', photo: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400', gameCount: 18 },
      { id: 'com-2', name: '光田康典', photo: 'https://images.unsplash.com/photo-1580971739182-ccd8cfef3707?w=400', gameCount: 12 },
      { id: 'com-3', name: 'Yuki Kajiura', photo: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400', gameCount: 14 },
      { id: 'com-4', name: '植松伸夫', photo: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400', gameCount: 20 },
];

export const mockSeries: Creator[] = [
      { id: 'ser-1', name: 'Final Fantasy', photo: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400', gameCount: 16 },
      { id: 'ser-2', name: 'Nier', photo: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400', gameCount: 4 },
      { id: 'ser-3', name: 'Tales of', photo: 'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400', gameCount: 12 },
      { id: 'ser-4', name: 'Persona', photo: 'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400', gameCount: 8 },
];

// Game types
export const mockGameTypes: GameType[] = [
      {
            id: 'type-1',
            name: 'RPG',
            gameCount: 24,
            coverImages: [
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400'
            ]
      },
      {
            id: 'type-2',
            name: '動作',
            gameCount: 18,
            coverImages: [
                  'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400',
                  'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400',
                  'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400'
            ]
      },
      {
            id: 'type-3',
            name: '冒險',
            gameCount: 15,
            coverImages: [
                  'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400',
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400'
            ]
      },
      {
            id: 'type-4',
            name: '賽博龐克',
            gameCount: 8,
            coverImages: [
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400'
            ]
      },
      {
            id: 'type-5',
            name: '恐怖',
            gameCount: 6,
            coverImages: ['https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400']
      },
      {
            id: 'type-6',
            name: '策略',
            gameCount: 12,
            coverImages: [
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400'
            ]
      }
];

// Tags
export const mockTagsWithGames: TagWithGames[] = [
      {
            id: 'tag-1',
            name: '我的最愛',
            gameCount: 8,
            coverImages: [
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400',
                  'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400',
                  'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400',
                  'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400',
                  'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400'
            ]
      },
      {
            id: 'tag-2',
            name: 'RPG',
            gameCount: 24,
            coverImages: [
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400',
                  'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400',
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400',
                  'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400',
                  'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400'
            ]
      },
      {
            id: 'tag-3',
            name: '單人',
            gameCount: 18,
            coverImages: [
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400',
                  'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400',
                  'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400'
            ]
      },
      {
            id: 'tag-4',
            name: '賽博龐克',
            gameCount: 5,
            coverImages: [
                  'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400',
                  'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400',
                  'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400'
            ]
      }
];

// Hero slides for Gallery
export const mockHeroSlides: HeroSlide[] = [
      { id: 'hero-1', image: 'https://images.unsplash.com/photo-1754472898907-d3b531bef6bb?w=1920', title: 'Epic Fantasy Realms', dominantColor: '#4a90e2' },
      { id: 'hero-2', image: 'https://images.unsplash.com/photo-1691180782998-da8fdb529623?w=1920', title: 'Dramatic Landscapes', dominantColor: '#e24a90' },
      { id: 'hero-3', image: 'https://images.unsplash.com/photo-1686807561227-18e9e7f2d472?w=1920', title: "Nature's Majesty", dominantColor: '#4ae290' },
      { id: 'hero-4', image: 'https://images.unsplash.com/photo-1732808460864-b8e5eb489a52?w=1920', title: 'Moody Atmospheres', dominantColor: '#9034e2' }
];

// Gallery items
export const mockGalleryItems: GalleryItem[] = [
      { id: 'g-1', title: 'Neon Dystopia', coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400', tags: ['賽博龐克', 'RPG'] },
      { id: 'g-2', title: 'Mystic Realms', coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400', tags: ['RPG', '冒險'] },
      { id: 'g-3', title: 'Velocity Overdrive', coverImage: 'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400', tags: ['動作'] },
      { id: 'g-4', title: 'Wilderness Chronicles', coverImage: 'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400', tags: ['冒險'] },
      { id: 'g-5', title: 'Stellar Odyssey', coverImage: 'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400', tags: ['RPG'] },
      { id: 'g-6', title: 'Apex Legends', coverImage: 'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400', tags: ['動作', '多人'] },
      { id: 'g-7', title: 'Dark Protocol', coverImage: 'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400', tags: ['賽博龐克'] },
      { id: 'g-8', title: 'Silent Hollow', coverImage: 'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400', tags: ['恐怖', '單人'] },
      { id: 'g-9', title: 'Cyber Runner', coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400', tags: ['賽博龐克', '動作'] },
      { id: 'g-10', title: 'Fantasy Quest', coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400', tags: ['RPG', '冒險'] },
      { id: 'g-11', title: 'Speed Demon', coverImage: 'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400', tags: ['動作'] },
      { id: 'g-12', title: 'Forest Tales', coverImage: 'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400', tags: ['冒險'] },
      { id: 'g-13', title: 'Space Explorer', coverImage: 'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400', tags: ['RPG', '策略'] },
      { id: 'g-14', title: 'Battle Royale', coverImage: 'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400', tags: ['動作', '多人'] },
      { id: 'g-15', title: 'Neon Nights', coverImage: 'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400', tags: ['賽博龐克'] },
      { id: 'g-16', title: 'Horror Manor', coverImage: 'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400', tags: ['恐怖'] },
];

// Workshop items
export const mockWorkshopItems: WorkshopItem[] = [
      { id: 'w-1', title: 'Unknown Game 1', coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400', status: 'pending' },
      { id: 'w-2', title: 'Unknown Game 2', coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400', status: 'pending' },
      { id: 'w-3', status: 'pending' },
      { id: 'w-4', title: 'Unknown Game 4', coverImage: 'https://images.unsplash.com/photo-1765706729543-348de9e073b1?w=400', status: 'pending' },
      { id: 'w-5', status: 'pending' },
      { id: 'w-6', title: 'Unknown Game 6', coverImage: 'https://images.unsplash.com/photo-1740390133235-e82eba2c040a?w=400', status: 'pending' },
      { id: 'w-7', status: 'pending' },
      { id: 'w-8', title: 'Unknown Game 8', coverImage: 'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400', status: 'pending' },
      { id: 'w-9', status: 'pending' },
      { id: 'w-10', title: 'Unknown Game 10', coverImage: 'https://images.unsplash.com/photo-1661715328971-83cd2179df82?w=400', status: 'pending' },
      { id: 'w-11', status: 'pending' },
      { id: 'w-12', title: 'Unknown Game 12', coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400', status: 'pending' },
      { id: 'w-13', status: 'pending' },
      { id: 'w-14', status: 'pending' },
      { id: 'w-15', title: 'Unknown Game 15', coverImage: 'https://images.unsplash.com/photo-1587016077756-d34a82b7376c?w=400', status: 'pending' },
      { id: 'w-16', status: 'pending' },
];

// Available tags for focus bar
export const availableTags = [
      '我的最愛', '賽博龐克', 'RPG', '冒險', '單人', '多人',
      '動作', '射擊', '策略', '模擬', '恐怖', '解謎',
      '平台', '競速', '體育', '格鬥', '音樂', '視覺小說',
      '獨立遊戲', 'AAA大作'
];

// ============================================
// Work Details View Mock Data
// ============================================

// Asset tag type for available assets display
export interface AssetTag {
      name: string;
      color: string; // Hex color for inline style
}

// Scenario writer type
export interface ScenarioWriter {
      name: string;
      role: string;
}

// Voice actor display info
export interface VoiceActorInfo {
      id: string;
      name: string;
      image: string;
}

// Mock available assets with color coding
export const mockAvailableAssets: AssetTag[] = [
      { name: 'ISO', color: '#10b981' },           // Emerald green
      { name: 'Soundtrack', color: '#3b82f6' },    // Blue
      { name: 'Manual PDF', color: '#f97316' },    // Orange
      { name: 'Save Data', color: '#a855f7' },     // Purple
      { name: 'H-Patch', color: '#ec4899' },       // Pink
];

// Mock scenario writers
export const mockScenarioWriters: ScenarioWriter[] = [
      { name: 'Kinoko Nasu', role: 'Original Story & Scenario' },
      { name: 'Takashi Takeuchi', role: 'Character Design & Art Direction' },
];

// Mock voice actors info for display (derived from characters)
export const mockVoiceActorInfoList: VoiceActorInfo[] = [
      { id: 'va-1', name: '花澤香菜', image: 'https://images.unsplash.com/photo-1624395213232-ea2bcd36b865?w=400' },
      { id: 'va-2', name: '早見沙織', image: 'https://images.unsplash.com/photo-1649589244330-09ca58e4fa64?w=400' },
      { id: 'va-3', name: '中村悠一', image: 'https://images.unsplash.com/photo-1672685667592-0392f458f46f?w=400' },
];

// Extended description for games with short descriptions
export const mockExtendedDescription = "In the neon-drenched metropolis of Neo-Tokyo, humanity stands on the brink of a digital revolution. 'Neon Genesis' follows the story of Asuka, a cyber-enhanced agent who discovers a conspiracy that threatens to unravel the fabric of reality. Navigate a world of political intrigue, advanced technology, and personal discovery as you uncover the truth behind the city's glittering façade.";

// Related work type for discovery section
export interface RelatedWork {
      id: string;
      title: string;
      coverImage: string;
      relation: 'sequel' | 'prequel' | 'spinoff' | 'adaptation' | 'same_developer';
}

// Mock related works for discovery section
export const mockRelatedWorks: RelatedWork[] = [
      {
            id: 'rel-1',
            title: 'Neon Genesis 2',
            coverImage: 'https://images.unsplash.com/photo-1577388219814-9b75a45cea09?w=400',
            relation: 'sequel'
      },
      {
            id: 'rel-2',
            title: 'Cyber Prelude',
            coverImage: 'https://images.unsplash.com/photo-1656381620321-bddff61435c3?w=400',
            relation: 'prequel'
      },
      {
            id: 'rel-3',
            title: 'Neon Side Stories',
            coverImage: 'https://images.unsplash.com/photo-1633287453177-24823499b02c?w=400',
            relation: 'spinoff'
      },
      {
            id: 'rel-4',
            title: 'Dark Horizon',
            coverImage: 'https://images.unsplash.com/photo-1762219214303-7e198a76d18e?w=400',
            relation: 'same_developer'
      },
];


