/**
 * MetadataEditor - Roon-style Metadata Editor
 *
 * Phase 24.0: The Curator
 *
 * Features:
 * - Field locking: Every input has a 🔒 toggle
 * - Smart inputs: Show original title as suggestion
 * - Tabs: General, Images, System
 * - Auto-lock on edit
 */

import React, { useState, useEffect } from 'react';
import { Lock, Unlock, Save, X, Image as ImageIcon, FolderOpen } from 'lucide-react';
import { api } from '../../api/client';

interface MetadataField {
  value: any;
  locked: boolean;
  source?: string;
}

interface GameMetadata {
  title: MetadataField;
  developer: MetadataField;
  release_date?: MetadataField;
  cover_url?: MetadataField;
  cover_path?: MetadataField;
  library_status: MetadataField;
  rating?: MetadataField;
  description?: MetadataField;
  tags?: MetadataField;
  user_tags?: string[];
  vndb_id?: string;
  exe_path?: string;
}

interface MetadataEditorProps {
  gameId: string;
  metadata: GameMetadata;
  onSave: (updatedMetadata: GameMetadata) => void;
  onCancel: () => void;
  onReidentify: () => void;
}

type Tab = 'general' | 'images' | 'system';

export const MetadataEditor: React.FC<MetadataEditorProps> = ({
  gameId,
  metadata,
  onSave,
  onCancel,
  onReidentify,
}) => {
  const [activeTab, setActiveTab] = useState<Tab>('general');
  const [editedMetadata, setEditedMetadata] = useState<GameMetadata>(metadata);
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'success' | 'error'>('idle');

  /**
   * Handle field value change
   * Auto-locks the field when user edits it
   */
  const handleFieldChange = (fieldName: string, value: any) => {
    setEditedMetadata((prev) => {
      const field = fieldName as keyof GameMetadata;
      const fieldData = prev[field];

      // Auto-lock on edit
      if (fieldData && typeof fieldData === 'object' && 'value' in fieldData) {
        return {
          ...prev,
          [field]: {
            ...fieldData,
            value,
            locked: true,
          },
        };
      }

      return prev;
    });
    setSaveStatus('idle');
  };

  /**
   * Toggle field lock status
   */
  const toggleFieldLock = (fieldName: string) => {
    setEditedMetadata((prev) => {
      const field = fieldName as keyof GameMetadata;
      const fieldData = prev[field];

      if (fieldData && typeof fieldData === 'object' && 'value' in fieldData) {
        return {
          ...prev,
          [field]: {
            ...fieldData,
            locked: !fieldData.locked,
          },
        };
      }

      return prev;
    });
    setSaveStatus('idle');
  };

  /**
   * Revert field to original value (when unlocking)
   */
  const revertField = (fieldName: string) => {
    const field = fieldName as keyof GameMetadata;
    const originalField = metadata[field];

    if (originalField && typeof originalField === 'object' && 'value' in originalField) {
      setEditedMetadata((prev) => ({
        ...prev,
        [field]: { ...originalField },
      }));
    }
  };

  /**
   * Save changes to backend
   */
  const handleSave = async () => {
    setIsSaving(true);
    setSaveStatus('idle');

    try {
      // Call backend to update metadata
      await api.updateField(gameId, 'metadata', editedMetadata);

      setSaveStatus('success');
      onSave(editedMetadata);

      // Reset success message after 2 seconds
      setTimeout(() => setSaveStatus('idle'), 2000);
    } catch (err: any) {
      setSaveStatus('error');
      console.error('Failed to save metadata:', err);
    } finally {
      setIsSaving(false);
    }
  };

  /**
   * Render a locked/unlocked input field
   */
  const renderField = (
    label: string,
    fieldName: string,
    placeholder: string = '',
    type: 'text' | 'textarea' | 'date' = 'text'
  ) => {
    const fieldData = editedMetadata[fieldName as keyof GameMetadata] as MetadataField;
    if (!fieldData) return null;

    const { value, locked, source } = fieldData;

    return (
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium text-zinc-300">{label}</label>
          <div className="flex items-center gap-2">
            {source && (
              <span className="text-xs text-zinc-500">Source: {source}</span>
            )}
            <button
              onClick={() => {
                if (locked) {
                  toggleFieldLock(fieldName);
                  revertField(fieldName);
                } else {
                  toggleFieldLock(fieldName);
                }
              }}
              className={`p-1.5 rounded transition-colors ${
                locked
                  ? 'bg-amber-600/20 text-amber-400 hover:bg-amber-600/30'
                  : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
              }`}
              title={locked ? 'Unlock and revert' : 'Lock field'}
            >
              {locked ? <Lock size={16} /> : <Unlock size={16} />}
            </button>
          </div>
        </div>

        {type === 'textarea' ? (
          <textarea
            value={value || ''}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder={placeholder}
            disabled={locked}
            className={`w-full px-4 py-3 bg-zinc-800 text-white rounded-lg border focus:outline-none focus:ring-2 focus:ring-blue-500 resize-none ${
              locked
                ? 'border-zinc-700 opacity-60 cursor-not-allowed'
                : 'border-zinc-700'
            }`}
            rows={4}
          />
        ) : (
          <input
            type={type}
            value={value || ''}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder={placeholder}
            disabled={locked}
            className={`w-full px-4 py-3 bg-zinc-800 text-white rounded-lg border focus:outline-none focus:ring-2 focus:ring-blue-500 ${
              locked
                ? 'border-zinc-700 opacity-60 cursor-not-allowed'
                : 'border-zinc-700'
            }`}
          />
        )}

        {locked && (
          <div className="text-xs text-amber-400 mt-1 flex items-center gap-1">
            <Lock size={12} />
            This field is locked and won't be updated by scanner/cloud sync
          </div>
        )}
      </div>
    );
  };

  /**
   * Render tag input field
   */
  const renderTagsField = () => {
    const fieldData = editedMetadata.tags;
    if (!fieldData) return null;

    const { value, locked } = fieldData as MetadataField;
    const tags = Array.isArray(value) ? value : [];

    return (
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <label className="text-sm font-medium text-zinc-300">Tags</label>
          <button
            onClick={() => toggleFieldLock('tags')}
            className={`p-1.5 rounded transition-colors ${
              locked
                ? 'bg-amber-600/20 text-amber-400 hover:bg-amber-600/30'
                : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
            }`}
          >
            {locked ? <Lock size={16} /> : <Unlock size={16} />}
          </button>
        </div>

        <div className="flex flex-wrap gap-2">
          {tags.map((tag: string, index: number) => (
            <span
              key={index}
              className="px-3 py-1 bg-zinc-800 text-zinc-300 rounded-full text-sm"
            >
              {tag}
            </span>
          ))}
        </div>

        {locked && (
          <div className="text-xs text-amber-400 mt-1">Tags are locked</div>
        )}
      </div>
    );
  };

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
      <div className="bg-zinc-900 rounded-lg max-w-4xl w-full max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-6 border-b border-zinc-800">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold text-white">Edit Metadata</h2>
            <button
              onClick={onCancel}
              className="p-2 hover:bg-zinc-800 rounded-lg transition-colors"
            >
              <X size={20} className="text-zinc-400" />
            </button>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-zinc-800">
          <button
            onClick={() => setActiveTab('general')}
            className={`px-6 py-3 font-medium transition-colors ${
              activeTab === 'general'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-zinc-400 hover:text-white'
            }`}
          >
            General
          </button>
          <button
            onClick={() => setActiveTab('images')}
            className={`px-6 py-3 font-medium transition-colors ${
              activeTab === 'images'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-zinc-400 hover:text-white'
            }`}
          >
            Images
          </button>
          <button
            onClick={() => setActiveTab('system')}
            className={`px-6 py-3 font-medium transition-colors ${
              activeTab === 'system'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-zinc-400 hover:text-white'
            }`}
          >
            System
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {activeTab === 'general' && (
            <div className="space-y-4">
              {/* Title */}
              {renderField('Title', 'title', 'Game title', 'text')}

              {/* Display Title (for multilingual titles) */}
              <div className="mb-4">
                <label className="text-sm font-medium text-zinc-300 block mb-2">
                  Original Title (Japanese)
                </label>
                <div className="text-sm text-zinc-500 italic">
                  Suggestion: Use for official Japanese title
                </div>
              </div>

              {/* Developer */}
              {renderField('Developer', 'developer', 'Developer/Studio name')}

              {/* Release Date */}
              {renderField('Release Date', 'release_date', 'YYYY-MM-DD', 'date')}

              {/* Description */}
              {renderField('Description', 'description', 'Game description...', 'textarea')}

              {/* Tags */}
              {renderTagsField()}

              {/* Rating */}
              {editedMetadata.rating && renderField('Rating', 'rating', '0-10', 'text')}
            </div>
          )}

          {activeTab === 'images' && (
            <div className="space-y-4">
              <div className="text-center py-12 bg-zinc-800 rounded-lg">
                <ImageIcon size={48} className="text-zinc-600 mx-auto mb-3" />
                <p className="text-zinc-400 mb-2">Image Curator</p>
                <p className="text-sm text-zinc-500">
                  Select cover and background images from your game folder
                </p>
                <button className="mt-4 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors">
                  Open Image Selector
                </button>
              </div>
            </div>
          )}

          {activeTab === 'system' && (
            <div className="space-y-4">
              {/* EXE Path */}
              <div className="mb-4">
                <label className="text-sm font-medium text-zinc-300 block mb-2">
                  Executable Path
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={editedMetadata.exe_path || ''}
                    onChange={(e) => handleFieldChange('exe_path', e.target.value)}
                    placeholder="Path to game executable..."
                    className="flex-1 px-4 py-3 bg-zinc-800 text-white rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                  <button className="px-4 py-3 bg-zinc-800 text-zinc-400 rounded-lg hover:bg-zinc-700 transition-colors">
                    <FolderOpen size={20} />
                  </button>
                </div>
                <p className="text-xs text-zinc-500 mt-1">
                  Override the auto-detected executable path
                </p>
              </div>

              {/* VNDB ID */}
              <div className="mb-4">
                <label className="text-sm font-medium text-zinc-300 block mb-2">
                  VNDB ID
                </label>
                <input
                  type="text"
                  value={editedMetadata.vndb_id || ''}
                  onChange={(e) => handleFieldChange('vndb_id', e.target.value)}
                  placeholder="v12345"
                  className="w-full px-4 py-3 bg-zinc-800 text-white rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
              </div>

              {/* Library Status */}
              <div className="mb-4">
                <label className="text-sm font-medium text-zinc-300 block mb-2">
                  Library Status
                </label>
                <select
                  value={editedMetadata.library_status?.value || 'unstarted'}
                  onChange={(e) => handleFieldChange('library_status', e.target.value)}
                  className="w-full px-4 py-3 bg-zinc-800 text-white rounded-lg border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  <option value="unstarted">Unstarted</option>
                  <option value="playing">Playing</option>
                  <option value="completed">Completed</option>
                  <option value="dropped">Dropped</option>
                </select>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-zinc-800 flex justify-between items-center">
          <div className="flex gap-3">
            <button
              onClick={onReidentify}
              className="px-4 py-2 bg-zinc-800 text-zinc-300 rounded-lg hover:bg-zinc-700 transition-colors"
            >
              Re-identify
            </button>
            <button
              onClick={onCancel}
              className="px-4 py-2 text-zinc-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
          </div>

          <div className="flex items-center gap-3">
            {saveStatus === 'success' && (
              <span className="text-green-400 text-sm">Saved successfully!</span>
            )}
            {saveStatus === 'error' && (
              <span className="text-red-400 text-sm">Failed to save</span>
            )}

            <button
              onClick={handleSave}
              disabled={isSaving}
              className="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
            >
              {isSaving ? 'Saving...' : (
                <>
                  <Save size={20} />
                  Save Changes
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default MetadataEditor;
