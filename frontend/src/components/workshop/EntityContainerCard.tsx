/**
 * Entity Container Card Component
 * 
 * Displays an aggregated game entity with visual stacking effect.
 * Shows detected game name with confidence, or "Unknown Cluster" fallback.
 */

import { useTranslation } from 'react-i18next';
import { ImageWithFallback } from '../figma/ImageWithFallback';
import { cn } from '../ui/utils';

export interface EntityFile {
      id: string;
      filename: string;
      type: 'GAME' | 'PATCH' | 'DLC' | 'EXTRA';
      size?: number;
}

export interface EntityContainer {
      id: string;
      detectedTitle?: string;
      confidence: 'high' | 'medium' | 'low' | 'none';
      coverImage?: string;
      files: EntityFile[];
      isOrphan?: boolean;
}

interface EntityContainerCardProps {
      entity: EntityContainer;
      isSelected?: boolean;
      onClick?: () => void;
      onDragOver?: (e: React.DragEvent) => void;
      onDrop?: (e: React.DragEvent) => void;
      displayTitle?: string;
}

export function EntityContainerCard({
      entity,
      isSelected,
      onClick,
      onDragOver,
      onDrop,
      displayTitle
}: EntityContainerCardProps) {
      const { t } = useTranslation();

      const confidenceColors = {
            high: 'bg-green-500/20 text-green-400 border-green-400/30',
            medium: 'bg-yellow-500/20 text-yellow-400 border-yellow-400/30',
            low: 'bg-orange-500/20 text-orange-400 border-orange-400/30',
            none: 'bg-gray-500/20 text-gray-400 border-gray-400/30'
      };

      const hasMultipleFiles = entity.files.length > 1;

      return (
            <div
                  onClick={onClick}
                  onDragOver={onDragOver}
                  onDrop={onDrop}
                  className={cn(
                        "group relative cursor-pointer transition-all duration-200",
                        isSelected
                              ? "ring-2 ring-[#6366f1] ring-offset-2 ring-offset-[#0a0a0a]"
                              : "hover:translate-x-1"
                  )}
            >
                  {/* Visual Stacking Effect - 3 cards layered */}
                  {hasMultipleFiles && (
                        <>
                              <div className="absolute -right-1 -bottom-1 w-full h-full bg-[#1a1a1a] rounded-lg border border-[#2a2a2a] transform translate-x-2 translate-y-2" />
                              <div className="absolute -right-0.5 -bottom-0.5 w-full h-full bg-[#1e1e1e] rounded-lg border border-[#2a2a2a] transform translate-x-1 translate-y-1" />
                        </>
                  )}

                  {/* Main Card */}
                  <div className={cn(
                        "relative bg-[#1a1a1a] rounded-lg border border-[#2a2a2a] overflow-hidden",
                        "group-hover:border-[#3a3a3a] transition-colors"
                  )}>
                        {/* Cover Section */}
                        <div className="relative aspect-[4/3] bg-[#0e0e0e]">
                              {entity.coverImage ? (
                                    <ImageWithFallback
                                          src={entity.coverImage}
                                          alt={entity.detectedTitle || 'Unknown'}
                                          className="w-full h-full object-cover"
                                    />
                              ) : (
                                    <div className="w-full h-full flex items-center justify-center">
                                          <span className="text-4xl text-[#3a3a3a]">?</span>
                                    </div>
                              )}

                              {/* File Count Badge */}
                              <div className="absolute top-2 right-2 bg-black/70 backdrop-blur-sm text-white text-xs px-2 py-1 rounded-full">
                                    {entity.files.length} {t('workshop.files')}
                              </div>
                        </div>

                        {/* Info Section */}
                        <div className="p-3 space-y-2">
                              {/* Title */}
                              <h3 className="text-white text-sm font-medium truncate">
                                    {displayTitle || (entity.detectedTitle
                                          ? `${t('workshop.detected')}: ${entity.detectedTitle}`
                                          : `${t('workshop.unknownCluster')} (${entity.files.length} ${t('workshop.files')})`
                                    )}
                              </h3>

                              {/* Confidence Badge */}
                              {entity.detectedTitle && (
                                    <span className={cn(
                                          "inline-block text-xs px-2 py-0.5 rounded-full border",
                                          confidenceColors[entity.confidence]
                                    )}>
                                          {t(`workshop.confidence.${entity.confidence}`)}
                                    </span>
                              )}

                              {/* File Type Summary */}
                              <div className="flex gap-1 flex-wrap">
                                    {['GAME', 'PATCH', 'DLC', 'EXTRA'].map(type => {
                                          const count = entity.files.filter(f => f.type === type).length;
                                          if (count === 0) return null;

                                          const colors: Record<string, string> = {
                                                GAME: 'bg-blue-500/20 text-blue-400',
                                                PATCH: 'bg-green-500/20 text-green-400',
                                                DLC: 'bg-purple-500/20 text-purple-400',
                                                EXTRA: 'bg-yellow-500/20 text-yellow-400'
                                          };

                                          return (
                                                <span
                                                      key={type}
                                                      className={cn("text-[10px] px-1.5 py-0.5 rounded", colors[type])}
                                                >
                                                      {count} {type}
                                                </span>
                                          );
                                    })}
                              </div>
                        </div>
                  </div>
            </div>
      );
}
