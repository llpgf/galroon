/**
 * Dual Track Editor Component
 * 
 * Side-by-side comparison: API data (read-only) vs User edits (editable).
 * Highlights differences between the two panels.
 */

import { useTranslation } from 'react-i18next';
import { Lock, Pencil } from 'lucide-react';
import { cn } from '../ui/utils';

interface MetadataField {
      key: string;
      label: string;
      apiValue?: string;
      userValue?: string;
}

interface DualTrackEditorProps {
      title: string;
      fields: MetadataField[];
      onFieldChange?: (key: string, value: string) => void;
}

export function DualTrackEditor({ title, fields, onFieldChange }: DualTrackEditorProps) {
      const { t } = useTranslation();

      return (
            <div className="bg-[#0e0e0e] rounded-lg border border-[#2a2a2a] overflow-hidden">
                  {/* Header */}
                  <div className="px-4 py-3 border-b border-[#2a2a2a] bg-[#1a1a1a]">
                        <h3 className="text-white text-sm font-medium">{title}</h3>
                  </div>

                  {/* Dual Track Layout */}
                  <div className="grid grid-cols-2 divide-x divide-[#2a2a2a]">
                        {/* Left Panel: API Data (Read-Only) */}
                        <div className="p-4 space-y-4 opacity-60">
                              <div className="flex items-center gap-2 text-[#6b6b6b] text-xs mb-4">
                                    <Lock className="w-3 h-3" />
                                    <span>{t('workshop.apiData')}</span>
                              </div>

                              {fields.map(field => (
                                    <div key={field.key} className="space-y-1">
                                          <label className="text-[#6b6b6b] text-xs">{field.label}</label>
                                          <div className="text-[#888] text-sm bg-[#1a1a1a] px-3 py-2 rounded border border-[#2a2a2a]">
                                                {field.apiValue || t('common.unknown')}
                                          </div>
                                    </div>
                              ))}
                        </div>

                        {/* Right Panel: User Edits (Editable) */}
                        <div className="p-4 space-y-4">
                              <div className="flex items-center gap-2 text-[#6366f1] text-xs mb-4">
                                    <Pencil className="w-3 h-3" />
                                    <span>{t('workshop.userEdits')}</span>
                              </div>

                              {fields.map(field => {
                                    const hasChange = field.apiValue !== field.userValue && field.userValue;

                                    return (
                                          <div key={field.key} className="space-y-1">
                                                <label className="text-[#b3b3b3] text-xs">{field.label}</label>
                                                <input
                                                      type="text"
                                                      value={field.userValue || ''}
                                                      onChange={(e) => onFieldChange?.(field.key, e.target.value)}
                                                      placeholder={field.apiValue || t('common.unknown')}
                                                      className={cn(
                                                            "w-full text-sm bg-[#1a1a1a] px-3 py-2 rounded border transition-colors",
                                                            "focus:outline-none focus:ring-1 focus:ring-[#6366f1]",
                                                            hasChange
                                                                  ? "text-[#6366f1] border-[#6366f1]/50 bg-[#6366f1]/10"
                                                                  : "text-white border-[#2a2a2a]"
                                                      )}
                                                />
                                          </div>
                                    );
                              })}
                        </div>
                  </div>
            </div>
      );
}
