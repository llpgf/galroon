/**
 * Workshop View (Revamped)
 * 
 * 3-column layout with collapsible left sidebar:
 * - Left: Entity Container List (collapsible)
 * - Center: Dual Track Editor (API vs User edits)
 * - Right: Component List (now "List of Candidates")
 * + Bottom: Orphan Drawer
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Sparkles, Save, Eye, ChevronLeft, ChevronRight } from 'lucide-react';
import { EntityContainerCard, EntityContainer, EntityFile } from './workshop/EntityContainerCard';
import { OrphanDrawer } from './workshop/OrphanDrawer';
import { DualTrackEditor } from './workshop/DualTrackEditor';
import { ComponentList } from './workshop/ComponentList';

// Mock data for development
const mockEntities: EntityContainer[] = [
      {
            id: '1',
            detectedTitle: 'Fate/stay night',
            confidence: 'high',
            coverImage: 'https://images.unsplash.com/photo-1511512578047-dfb367046420?w=400',
            files: [
                  { id: 'f1', filename: 'FateSN_Full.iso', type: 'GAME', size: 4500000000 },
                  { id: 'f2', filename: 'Update_v1.2.zip', type: 'PATCH', size: 150000000 },
                  { id: 'f3', filename: 'Voice_Pack.rar', type: 'DLC', size: 2000000000 }
            ]
      },
      {
            id: '2',
            detectedTitle: 'Steins;Gate',
            confidence: 'medium',
            files: [
                  { id: 'f4', filename: 'SG_Setup.exe', type: 'GAME', size: 3200000000 }
            ]
      },
      {
            id: '3',
            confidence: 'none',
            files: [
                  { id: 'f5', filename: 'unknown_folder', type: 'GAME', size: 1500000000 },
                  { id: 'f6', filename: 'readme.txt', type: 'EXTRA', size: 5000 }
            ]
      }
];

const mockOrphans: EntityFile[] = [
      { id: 'o1', filename: 'fate.crack.zip', type: 'EXTRA' },
      { id: 'o2', filename: 'patch_v3.rar', type: 'PATCH' }
];

export function WorkshopViewRevamped() {
      const { t } = useTranslation();
      const [entities, setEntities] = useState<EntityContainer[]>(mockEntities);
      const [orphans, setOrphans] = useState<EntityFile[]>(mockOrphans);
      const [selectedEntityId, setSelectedEntityId] = useState<string | null>(mockEntities[0]?.id || null);
      const [draggedFile, setDraggedFile] = useState<EntityFile | null>(null);
      const [leftSidebarCollapsed, setLeftSidebarCollapsed] = useState(false);

      // Editable metadata state
      const [userEdits, setUserEdits] = useState<Record<string, string>>({
            title: '',
            developer: '',
            releaseDate: '',
            genre: ''
      });

      const selectedEntity = entities.find(e => e.id === selectedEntityId);

      // Smart entity title logic
      const getEntityTitle = (entity: EntityContainer) => {
            if (entity.detectedTitle) {
                  // VNDB match → "Potential Game: [name]"
                  return `Potential Game: ${entity.detectedTitle}`;
            }
            // No match → "Unknown Cluster (X files)"
            return `Unknown Cluster (${entity.files.length} files)`;
      };

      const handleDragStart = useCallback((file: EntityFile, e: React.DragEvent) => {
            setDraggedFile(file);
            e.dataTransfer.effectAllowed = 'move';
      }, []);

      const handleDragOver = useCallback((e: React.DragEvent) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
      }, []);

      const handleDropOnEntity = useCallback((entityId: string, e: React.DragEvent) => {
            e.preventDefault();
            if (!draggedFile) return;

            // Remove from orphans
            setOrphans(prev => prev.filter(o => o.id !== draggedFile.id));

            // Add to target entity
            setEntities(prev => prev.map(entity => {
                  if (entity.id === entityId) {
                        return { ...entity, files: [...entity.files, draggedFile] };
                  }
                  return entity;
            }));

            setDraggedFile(null);
      }, [draggedFile]);

      const handleFieldChange = useCallback((key: string, value: string) => {
            setUserEdits(prev => ({ ...prev, [key]: value }));
      }, []);

      const handlePreviewRitual = useCallback(() => {
            alert(t('workshop.previewRitual'));
      }, [t]);

      const handleSaveDraft = useCallback(() => {
            alert(t('workshop.draftSaved'));
      }, [t]);

      // Build metadata fields for editor
      const metadataFields = [
            { key: 'title', label: t('workshop.field.title'), apiValue: selectedEntity?.detectedTitle, userValue: userEdits.title },
            { key: 'developer', label: t('workshop.field.developer'), apiValue: 'TYPE-MOON', userValue: userEdits.developer },
            { key: 'releaseDate', label: t('workshop.field.releaseDate'), apiValue: '2004-01-30', userValue: userEdits.releaseDate },
            { key: 'genre', label: t('workshop.field.genre'), apiValue: 'Visual Novel', userValue: userEdits.genre }
      ];

      return (
            <div className="h-screen flex flex-col bg-[#0a0a0a]">
                  {/* Header */}
                  <div className="sticky top-0 z-40 bg-[#0a0a0a]/90 backdrop-blur-xl border-b border-[#1a1a1a] px-6 py-4">
                        <div className="flex items-center justify-between">
                              <div>
                                    <h1 className="text-white text-xl font-light tracking-tight">
                                          {t('nav.workshop')}: <span className="text-[#6b6b6b]">{entities.length} {t('workshop.entities')}</span>
                                    </h1>
                                    {orphans.length > 0 && (
                                          <p className="text-orange-400 text-sm mt-1">
                                                {orphans.length} {t('workshop.orphanFiles')}
                                          </p>
                                    )}
                              </div>

                              {selectedEntity && (
                                    <div className="flex gap-3">
                                          <button
                                                onClick={handleSaveDraft}
                                                className="px-4 py-2 bg-[#2a2a2a] hover:bg-[#3a3a3a] text-white rounded-lg text-sm flex items-center gap-2 transition-colors"
                                          >
                                                <Save className="w-4 h-4" />
                                                {t('workshop.saveDraft')}
                                          </button>
                                          <button
                                                onClick={handlePreviewRitual}
                                                className="px-4 py-2 bg-[#6366f1] hover:bg-[#5558e3] text-white rounded-lg text-sm flex items-center gap-2 transition-colors"
                                          >
                                                <Eye className="w-4 h-4" />
                                                {t('workshop.previewRitual')}
                                          </button>
                                    </div>
                              )}
                        </div>
                  </div>

                  {/* Main Content - 3 Column Layout */}
                  <div className="flex-1 flex overflow-hidden">
                        {/* Left: Entity List (Collapsible) */}
                        <div className={`relative border-r border-[#1a1a1a] transition-all duration-300 ${leftSidebarCollapsed ? 'w-0' : 'w-72'}`}>
                              {!leftSidebarCollapsed && (
                                    <div className="h-full overflow-y-auto p-4 space-y-4">
                                          {entities.map(entity => (
                                                <EntityContainerCard
                                                      key={entity.id}
                                                      entity={entity}
                                                      isSelected={entity.id === selectedEntityId}
                                                      onClick={() => setSelectedEntityId(entity.id)}
                                                      onDragOver={handleDragOver}
                                                      onDrop={(e) => handleDropOnEntity(entity.id, e)}
                                                      displayTitle={getEntityTitle(entity)}
                                                />
                                          ))}
                                    </div>
                              )}

                              {/* Collapse/Expand Button */}
                              <button
                                    onClick={() => setLeftSidebarCollapsed(!leftSidebarCollapsed)}
                                    className="absolute -right-4 top-1/2 -translate-y-1/2 z-10 w-8 h-8 bg-[#2a2a2a] hover:bg-[#3a3a3a] border border-[#3a3a3a] rounded-full flex items-center justify-center text-white/60 hover:text-white transition-colors shadow-lg"
                              >
                                    {leftSidebarCollapsed ? (
                                          <ChevronRight className="w-4 h-4" />
                                    ) : (
                                          <ChevronLeft className="w-4 h-4" />
                                    )}
                              </button>
                        </div>

                        {/* Center: Dual Track Editor */}
                        <div className="flex-1 overflow-y-auto p-6 space-y-6">
                              {selectedEntity ? (
                                    <>
                                          {/* Entity Header with Smart Title */}
                                          <div className="flex items-center gap-4">
                                                <Sparkles className="w-6 h-6 text-[#6366f1]" />
                                                <div>
                                                      <h2 className="text-white text-lg">
                                                            {getEntityTitle(selectedEntity)}
                                                      </h2>
                                                      <p className="text-[#6b6b6b] text-sm">
                                                            {selectedEntity.files.length} {t('workshop.files')}
                                                      </p>
                                                </div>
                                          </div>

                                          {/* Dual Track Editor */}
                                          <DualTrackEditor
                                                title={t('workshop.metadata')}
                                                fields={metadataFields}
                                                onFieldChange={handleFieldChange}
                                          />
                                    </>
                              ) : (
                                    <div className="flex items-center justify-center h-full">
                                          <p className="text-[#6b6b6b]">{t('workshop.selectEntity')}</p>
                                    </div>
                              )}
                        </div>

                        {/* Right: Component List (Now "List of Candidates") */}
                        <div className="w-80 border-l border-[#1a1a1a] overflow-y-auto p-4">
                              {selectedEntity && (
                                    <ComponentList
                                          files={selectedEntity.files}
                                          onDragOver={handleDragOver}
                                          onDrop={(e) => handleDropOnEntity(selectedEntity.id, e)}
                                          title={t('workshop.listOfCandidates')}
                                    />
                              )}
                        </div>
                  </div>

                  {/* Bottom: Orphan Drawer */}
                  <OrphanDrawer
                        orphans={orphans}
                        onDragStart={handleDragStart}
                  />
            </div>
      );
}
