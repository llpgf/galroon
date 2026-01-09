/**
 * Orphan Drawer Component
 * 
 * Collapsible bottom drawer for unmatched files.
 * Supports drag-and-drop to entity containers.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronUp, FileQuestion } from 'lucide-react';
import { cn } from '../ui/utils';
import type { EntityFile } from './EntityContainerCard';

interface OrphanDrawerProps {
      orphans: EntityFile[];
      onDragStart?: (file: EntityFile, e: React.DragEvent) => void;
}

export function OrphanDrawer({ orphans, onDragStart }: OrphanDrawerProps) {
      const { t } = useTranslation();
      const [isExpanded, setIsExpanded] = useState(false);

      if (orphans.length === 0) return null;

      return (
            <div className={cn(
                  "border-t border-[#2a2a2a] bg-[#0e0e0e] transition-all duration-300",
                  isExpanded ? "max-h-64" : "max-h-12"
            )}>
                  {/* Header */}
                  <button
                        onClick={() => setIsExpanded(!isExpanded)}
                        className="w-full px-4 py-3 flex items-center justify-between text-[#6b6b6b] hover:text-white transition-colors"
                  >
                        <div className="flex items-center gap-2">
                              <FileQuestion className="w-4 h-4" />
                              <span className="text-sm">
                                    {t('workshop.orphans')} ({orphans.length})
                              </span>
                        </div>
                        {isExpanded ? (
                              <ChevronDown className="w-4 h-4" />
                        ) : (
                              <ChevronUp className="w-4 h-4" />
                        )}
                  </button>

                  {/* Orphan Files Grid */}
                  {isExpanded && (
                        <div className="px-4 pb-4 overflow-y-auto max-h-48">
                              <div className="grid grid-cols-2 gap-2">
                                    {orphans.map(file => (
                                          <div
                                                key={file.id}
                                                draggable
                                                onDragStart={(e) => onDragStart?.(file, e)}
                                                className={cn(
                                                      "bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg p-3",
                                                      "cursor-grab active:cursor-grabbing",
                                                      "hover:border-[#3a3a3a] transition-colors"
                                                )}
                                          >
                                                <div className="flex items-center gap-2">
                                                      <FileQuestion className="w-4 h-4 text-[#6b6b6b] flex-shrink-0" />
                                                      <span className="text-white text-xs truncate">{file.filename}</span>
                                                </div>
                                                <p className="text-[#4a4a4a] text-[10px] mt-1">
                                                      {t('workshop.dragToAssign')}
                                                </p>
                                          </div>
                                    ))}
                              </div>
                        </div>
                  )}
            </div>
      );
}
