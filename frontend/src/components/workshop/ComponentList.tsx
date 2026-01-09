/**
 * Component List
 * 
 * Displays files within an entity container with auto-tagged color badges.
 * Supports drag-drop target for orphan file assignment.
 */

import { useTranslation } from 'react-i18next';
import { File, Folder, Package, Gift, Sparkles, GripVertical } from 'lucide-react';
import { cn } from '../ui/utils';
import type { EntityFile } from './EntityContainerCard';

interface ComponentListProps {
      files: EntityFile[];
      onReorder?: (files: EntityFile[]) => void;
      onRemove?: (fileId: string) => void;
      onDragOver?: (e: React.DragEvent) => void;
      onDrop?: (e: React.DragEvent) => void;
      title?: string;
}

const FILE_TYPE_CONFIG = {
      GAME: {
            color: 'bg-blue-500/20 text-blue-400 border-blue-400/30',
            icon: Folder,
            label: 'Game'
      },
      PATCH: {
            color: 'bg-green-500/20 text-green-400 border-green-400/30',
            icon: Package,
            label: 'Patch'
      },
      DLC: {
            color: 'bg-purple-500/20 text-purple-400 border-purple-400/30',
            icon: Gift,
            label: 'DLC'
      },
      EXTRA: {
            color: 'bg-yellow-500/20 text-yellow-400 border-yellow-400/30',
            icon: Sparkles,
            label: 'Extra'
      }
};

export function ComponentList({
      files,
      onReorder,
      onRemove,
      onDragOver,
      onDrop,
      title
}: ComponentListProps) {
      const { t } = useTranslation();

      return (
            <div
                  className="bg-[#0e0e0e] rounded-lg border border-[#2a2a2a] overflow-hidden"
                  onDragOver={onDragOver}
                  onDrop={onDrop}
            >
                  {/* Header */}
                  <div className="px-4 py-3 border-b border-[#2a2a2a] bg-[#1a1a1a] flex items-center justify-between">
                        <h3 className="text-white text-sm font-medium">{title || t('workshop.components')}</h3>
                        <span className="text-[#6b6b6b] text-xs">{files.length} {t('workshop.files')}</span>
                  </div>

                  {/* File List */}
                  <div className="divide-y divide-[#1a1a1a]">
                        {files.map((file, index) => {
                              const config = FILE_TYPE_CONFIG[file.type];
                              const Icon = config.icon;

                              return (
                                    <div
                                          key={file.id}
                                          className="group px-4 py-3 flex items-center gap-3 hover:bg-[#1a1a1a] transition-colors"
                                    >
                                          {/* Drag Handle */}
                                          <GripVertical className="w-4 h-4 text-[#3a3a3a] cursor-grab active:cursor-grabbing opacity-0 group-hover:opacity-100 transition-opacity" />

                                          {/* Type Icon */}
                                          <div className={cn(
                                                "w-8 h-8 rounded flex items-center justify-center flex-shrink-0",
                                                config.color
                                          )}>
                                                <Icon className="w-4 h-4" />
                                          </div>

                                          {/* File Info */}
                                          <div className="flex-1 min-w-0">
                                                <p className="text-white text-sm truncate">{file.filename}</p>
                                                {file.size && (
                                                      <p className="text-[#6b6b6b] text-xs">
                                                            {(file.size / 1024 / 1024).toFixed(1)} MB
                                                      </p>
                                                )}
                                          </div>

                                          {/* Type Badge */}
                                          <span className={cn(
                                                "text-[10px] px-2 py-0.5 rounded-full border",
                                                config.color
                                          )}>
                                                {config.label}
                                          </span>

                                          {/* Remove Button */}
                                          <button
                                                onClick={() => onRemove?.(file.id)}
                                                className="text-[#4a4a4a] hover:text-red-400 text-xs opacity-0 group-hover:opacity-100 transition-all"
                                          >
                                                ✕
                                          </button>
                                    </div>
                              );
                        })}
                  </div>

                  {/* Drop Zone Hint */}
                  {files.length === 0 && (
                        <div className="px-4 py-8 text-center">
                              <File className="w-8 h-8 text-[#3a3a3a] mx-auto mb-2" />
                              <p className="text-[#6b6b6b] text-sm">{t('workshop.dropFilesHere')}</p>
                        </div>
                  )}
            </div>
      );
}
