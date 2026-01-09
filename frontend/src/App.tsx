import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Sidebar } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import { GameCard } from './components/GameCard';
import { InboxPage } from './pages/InboxPage';
import { CollectionsPage } from './pages/CollectionsPage';
import { CanonicalDetailPage } from './pages/CanonicalDetailPage';
import { WorkDetailsPage } from './pages/WorkDetailsPage';
import { CharactersPage } from './pages/CharactersPage';
import { ClusterDecisionPage } from './pages/ClusterDecisionPage';
import { SettingsPage } from './pages/SettingsPage';
import { FocusBar, FocusTag } from './components/FocusBar';
import { CreatorsPage } from './pages/CreatorsPage';
import { GameTypesPage } from './pages/GameTypesPage';
import { TagsPage } from './pages/TagsPage';
import { GalleryPage } from './pages/GalleryPage';
import { WorkshopPage } from './pages/WorkshopPage';
import { DashboardPage } from './pages/DashboardPage';
import { SkeletonGrid, EmptyState } from './components/Skeleton';
import { useLibrary } from './hooks/useLibrary';

// Import mock data (used as fallback and for non-library views)
import {
  mockInboxClusters,
  mockCollections,
  mockCanonicalGame,
  mockClusterDecision,
  mockVoiceActors,
  mockArtists,
  mockWriters,
  mockComposers,
  mockSeries,
  mockGameTypes,
  mockTagsWithGames,
  mockHeroSlides,
  mockGalleryItems,
  mockWorkshopItems,
  availableTags,
  // Work Details View specific mock data
  mockAvailableAssets,
  mockScenarioWriters,
  mockExtendedDescription,
  mockRelatedWorks,
} from './mockData';


type View =
  | 'inbox'
  | 'tags'
  | 'game-types'
  | 'later'
  | 'my-games'
  | 'voice-actors'
  | 'artists'
  | 'writers'
  | 'composers'
  | 'series'
  | 'playlist-favorites'
  | 'playlist-playing'
  | 'playlist-completed'
  | 'collections'
  | 'canonical-detail'
  | 'work-details'
  | 'characters'
  | 'cluster-decision'
  | 'settings'
  | 'gallery'
  | 'workshop'
  | 'dashboard';

export default function App() {
  const { t } = useTranslation();
  const [currentView, setCurrentView] = useState<View>('gallery');
  const [searchQuery, setSearchQuery] = useState('');
  const [focusTags, setFocusTags] = useState<FocusTag[]>([]);

  // Use API hook for library data (with mock fallback)
  const { data: libraryData, loading: libraryLoading } = useLibrary();


  const handleAddTag = (tagName: string) => {
    if (!focusTags.find(t => t.name === tagName)) {
      setFocusTags([...focusTags, { name: tagName, state: 'include' }]);
    }
  };

  const handleRemoveTag = (tagName: string) => {
    setFocusTags(focusTags.filter(t => t.name !== tagName));
  };

  const handleToggleTagState = (tagName: string) => {
    setFocusTags(focusTags.map(tag => {
      if (tag.name === tagName) {
        return { ...tag, state: tag.state === 'include' ? 'exclude' : 'include' };
      }
      return tag;
    }));
  };

  const handleNavigate = (view: View) => {
    setCurrentView(view);
  };

  const handleSelectCluster = (id: string) => {
    setCurrentView('cluster-decision');
  };

  const handleGameClick = (id: string) => {
    // For mock/demo consistency, treat gallery items as valid games to open details
    // In production, this would look up the game in libraryData
    setCurrentView('work-details');
  };

  const handleBackToLibrary = () => {
    setCurrentView('gallery');
  };

  const handleBackToInbox = () => {
    setCurrentView('inbox');
  };

  const handleAcceptMatch = () => {
    alert('Match accepted! This would create a canonical entry.');
    setCurrentView('inbox');
  };

  const handleRejectMatch = () => {
    alert('Match rejected. Files will remain in orphan state.');
    setCurrentView('inbox');
  };

  const handleQuickAccept = (id: string) => {
    alert(`Quick accepted cluster: ${id}`);
  };

  const handleQuickReject = (id: string) => {
    alert(`Quick rejected cluster: ${id}`);
  };

  const handleLaunchGame = (path: string) => {
    alert(`Launching game from: ${path}`);
  };

  const handleOpenFolder = (path: string) => {
    alert(`Opening folder: ${path}`);
  };

  const handleEditMetadata = () => {
    alert('Edit Metadata dialog would open here');
  };

  const handleSelectCollection = (id: string) => {
    alert(`Opening collection: ${id}`);
  };

  const handleCreateCollection = () => {
    alert('Create new collection dialog would open here');
  };

  const [libraryPaths] = useState([
    { path: 'D:/Games', usage: 75, label: 'Game Drive' },
    { path: 'C:/Program Files/Steam/steamapps/common', usage: 92, label: 'System SSD' },
    { path: 'E:/Epic Games', usage: 45, label: 'External HDD' }
  ]);
  const [scanningPaths, setScanningPaths] = useState<Set<string>>(new Set());

  const handleScanPath = (path: string) => {
    setScanningPaths(prev => new Set(prev).add(path));
    // Simulate scan duration
    setTimeout(() => {
      setScanningPaths(prev => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    }, 2000);
  };

  const handleScanAll = () => {
    // Mark all as scanning
    const allPaths = libraryPaths.map(p => p.path);
    setScanningPaths(new Set(allPaths));

    setTimeout(() => {
      setScanningPaths(new Set());
    }, 3000);
  };

  const filteredLibrary = searchQuery
    ? libraryData.filter(item =>
      item.display_title.toLowerCase().includes(searchQuery.toLowerCase())
    )
    : libraryData;

  return (
    <div className="flex bg-[var(--color-background)] w-full h-full text-[var(--color-text-primary)] overflow-hidden select-none">
      <Sidebar
        currentView={currentView}
        onNavigate={(view) => setCurrentView(view as View)}
        inboxCount={mockInboxClusters.length}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 bg-[var(--color-background)]">
        <TopBar
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          libraryPaths={libraryPaths}
          onScanPath={handleScanPath}
        />

        <div className="flex-1 overflow-y-auto relative scrollbar-hide">

          {currentView === 'inbox' && (
            <InboxPage
              clusters={mockInboxClusters}
              onSelectCluster={handleSelectCluster}
              onAcceptCluster={handleQuickAccept}
              onRejectCluster={handleQuickReject}
            />
          )}

          {/* Tags View */}
          {currentView === 'tags' && (
            <TagsPage
              tags={mockTagsWithGames}
              onSelectTag={(id) => alert(`Opening tag: ${id}`)}
            />
          )}

          {/* Game Types View */}
          {currentView === 'game-types' && (
            <GameTypesPage
              types={mockGameTypes}
              onSelectType={(id) => alert(`Opening type: ${id}`)}
            />
          )}

          {/* Voice Actors View */}
          {currentView === 'voice-actors' && (
            <CreatorsPage
              type="voice-actors"
              creators={mockVoiceActors}
              onSelectCreator={(id) => alert(`Opening voice actor: ${id}`)}
            />
          )}

          {/* Artists View */}
          {currentView === 'artists' && (
            <CreatorsPage
              type="artists"
              creators={mockArtists}
              onSelectCreator={(id) => alert(`Opening artist: ${id}`)}
            />
          )}

          {/* Writers View */}
          {currentView === 'writers' && (
            <CreatorsPage
              type="writers"
              creators={mockWriters}
              onSelectCreator={(id) => alert(`Opening writer: ${id}`)}
            />
          )}

          {/* Composers View */}
          {currentView === 'composers' && (
            <CreatorsPage
              type="composers"
              creators={mockComposers}
              onSelectCreator={(id) => alert(`Opening composer: ${id}`)}
            />
          )}

          {/* Series View */}
          {currentView === 'series' && (
            <CreatorsPage
              type="series"
              creators={mockSeries}
              onSelectCreator={(id) => alert(`Opening series: ${id}`)}
            />
          )}

          {/* Placeholder views for remaining sections */}
          {(currentView === 'later' ||
            currentView === 'my-games' ||
            currentView === 'playlist-favorites' ||
            currentView === 'playlist-playing' ||
            currentView === 'playlist-completed') && (
              <div className="flex items-center justify-center h-full">
                <div className="text-center">
                  <h2 className="text-white text-2xl mb-2">
                    {currentView === 'later' && t('nav.later')}
                    {currentView === 'my-games' && t('nav.myGames')}
                    {currentView === 'playlist-favorites' && t('nav.favorites')}
                    {currentView === 'playlist-playing' && t('nav.playing')}
                    {currentView === 'playlist-completed' && t('nav.completed')}
                  </h2>
                  <p className="text-[var(--color-text-tertiary)]">{t('common.underConstruction')}</p>
                </div>
              </div>
            )}

          {currentView === 'collections' && (
            <CollectionsPage
              collections={mockCollections}
              onSelectCollection={handleSelectCollection}
              onCreateCollection={handleCreateCollection}
            />
          )}

          {currentView === 'work-details' && (
            <WorkDetailsPage
              game={mockCanonicalGame}
              availableAssets={mockAvailableAssets}
              scenarioWriters={mockScenarioWriters}
              relatedWorks={mockRelatedWorks}
              extendedDescription={mockExtendedDescription}
              onBack={handleBackToLibrary}
              onLaunchGame={handleLaunchGame}
              onOpenFolder={handleOpenFolder}
              onEditMetadata={handleEditMetadata}
              onViewAllCharacters={() => setCurrentView('characters')}
            />
          )}

          {currentView === 'characters' && (
            <CharactersPage
              game={mockCanonicalGame}
              onBack={() => setCurrentView('work-details')}
            />
          )}

          {currentView === 'cluster-decision' && (
            <ClusterDecisionPage
              cluster={mockClusterDecision}
              onBack={handleBackToInbox}
              onAccept={handleAcceptMatch}
              onReject={handleRejectMatch}
            />
          )}

          {currentView === 'settings' && (
            <SettingsPage />
          )}

          {currentView === 'gallery' && (
            <GalleryPage
              heroSlides={mockHeroSlides}
              items={mockGalleryItems}
              onItemClick={handleGameClick}
            />
          )}

          {currentView === 'workshop' && (
            <WorkshopPage />
          )}

          {currentView === 'dashboard' && (
            <DashboardPage
              onNavigateToGallery={(filter) => {
                setCurrentView('gallery');
                if (filter) alert(`Filter: ${filter}`);
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
