// WorkDetail page — full details with completion tracking + characters + translate.

import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { Work, getWork, toAssetUrl, formatRating, statusLabel, statusColor, enrichmentLabel } from '../../hooks/api';
import './WorkDetail.css';

interface CharacterRow {
      id: string;
      name: string;
      name_original: string | null;
      role: string | null;
      image_url: string | null;
}

interface CompletionInfo {
      status: string;
      progress_pct: number;
      playtime_min: number;
      notes: string;
}

interface TagInfo {
      id: string;
      name: string;
      tag_type: string;
}

interface WorkCreditRow {
      person_id: string;
      name: string;
      name_original: string | null;
      image_url: string | null;
      description: string | null;
      role: string;
      character_name: string | null;
      notes: string | null;
}

interface WorkVariantRow {
      id: string;
      folder_path: string;
      title: string;
      developer: string | null;
      enrichment_state: string;
      asset_count: number;
      asset_types: string[];
      has_completion: boolean;
      is_representative: boolean;
}

interface WorkAssetGroupVariantRow {
      work_id: string;
      folder_path: string;
      asset_count: number;
}

interface WorkAssetGroupRow {
      asset_type: string;
      relation_role: string;
      parent_asset_type: string | null;
      asset_count: number;
      variant_count: number;
      representative_work_id: string | null;
      representative_path: string | null;
      variants: WorkAssetGroupVariantRow[];
}

interface ProvenanceEntry {
      key: string;
      label: string;
      source: string;
      availableSources: string[];
}

const COMPLETION_STATUSES = [
      { value: 'not_started', label: 'Not Started' },
      { value: 'in_progress', label: 'In Progress' },
      { value: 'completed', label: 'Completed' },
      { value: 'on_hold', label: 'On Hold' },
      { value: 'dropped', label: 'Dropped' },
];

function sanitizeDescription(text: string): string {
      return text
            .replace(/\[url=[^\]]+\]([\s\S]*?)\[\/url\]/gi, '$1')
            .replace(/\[(?:i|b|u|spoiler|quote)\]([\s\S]*?)\[\/(?:i|b|u|spoiler|quote)\]/gi, '$1')
            .replace(/\[\/?[a-z]+(?:=[^\]]+)?\]/gi, '')
            .replace(/\r\n/g, '\n')
            .trim();
}

function sourceLabel(source: string): string {
      const labels: Record<string, string> = {
            filesystem: 'Filesystem',
            vndb: 'VNDB',
            bangumi: 'Bangumi',
            dlsite: 'DLsite',
            user_override: 'Manual',
      };
      return labels[source] || source;
}

function editableFieldValue(work: Work, field: string): string {
      switch (field) {
            case 'title': return work.title;
            case 'title_aliases': return work.title_aliases.join('\n');
            case 'developer': return work.developer || '';
            case 'publisher': return work.publisher || '';
            case 'release_date': return work.release_date || '';
            case 'description': return work.description || '';
            case 'cover_path': return work.cover_path || '';
            case 'rating': return work.rating !== null ? String(work.rating) : '';
            default: return '';
      }
}

export default function WorkDetail() {
      const { id } = useParams<{ id: string }>();
      const navigate = useNavigate();
      const [work, setWork] = useState<Work | null>(null);
      const [isLoading, setIsLoading] = useState(true);
      const [characters, setCharacters] = useState<CharacterRow[]>([]);
      const [completion, setCompletion] = useState<CompletionInfo | null>(null);
      const [translatedDesc, setTranslatedDesc] = useState<string | null>(null);
      const [isTranslating, setIsTranslating] = useState(false);
      const [workTags, setWorkTags] = useState<TagInfo[]>([]);
      const [tagInput, setTagInput] = useState('');
      const [tagSuggestions, setTagSuggestions] = useState<TagInfo[]>([]);
      const [credits, setCredits] = useState<WorkCreditRow[]>([]);
      const [variants, setVariants] = useState<WorkVariantRow[]>([]);
      const [assetGroups, setAssetGroups] = useState<WorkAssetGroupRow[]>([]);
      const [fieldDrafts, setFieldDrafts] = useState<Record<string, string>>({});
      const [fieldAction, setFieldAction] = useState<string | null>(null);

      useEffect(() => {
            if (!id) return;
            loadWork(id);
            loadCharacters(id);
            loadCredits(id);
            loadVariants(id);
            loadAssetGroups(id);
            loadCompletion(id);
            loadWorkTags(id);
            void loadTranslatedDescription(id);
      }, [id]);

      useEffect(() => {
            if (!work) return;
            setFieldDrafts({
                  title: editableFieldValue(work, 'title'),
                  title_aliases: editableFieldValue(work, 'title_aliases'),
                  developer: editableFieldValue(work, 'developer'),
                  publisher: editableFieldValue(work, 'publisher'),
                  release_date: editableFieldValue(work, 'release_date'),
                  description: editableFieldValue(work, 'description'),
                  cover_path: editableFieldValue(work, 'cover_path'),
                  rating: editableFieldValue(work, 'rating'),
            });
      }, [work]);

      async function loadWorkTags(workId: string) {
            try { setWorkTags(await invoke<TagInfo[]>('get_work_tags', { workId })); } catch { }
      }

      async function loadTranslatedDescription(workId: string) {
            try {
                  const text = await invoke<string | null>('get_localized_text', {
                        workId,
                        field: 'description',
                        locale: 'en',
                  });
                  setTranslatedDesc(text || null);
            } catch {
                  setTranslatedDesc(null);
            }
      }

      async function handleTagSearch(q: string) {
            setTagInput(q);
            if (q.length < 1) { setTagSuggestions([]); return; }
            try { setTagSuggestions(await invoke<TagInfo[]>('search_tags', { query: q })); } catch { }
      }

      async function handleAddTag(tag: TagInfo) {
            if (!id) return;
            await invoke('tag_work', { workId: id, tagId: tag.id });
            loadWorkTags(id);
            setTagInput('');
            setTagSuggestions([]);
      }

      async function handleCreateTag() {
            if (!id || !tagInput.trim()) return;
            const newId = await invoke<string>('add_user_tag', { name: tagInput.trim() });
            await invoke('tag_work', { workId: id, tagId: newId });
            loadWorkTags(id);
            setTagInput('');
            setTagSuggestions([]);
      }

      async function handleRemoveTag(tagId: string) {
            if (!id) return;
            await invoke('untag_work', { workId: id, tagId });
            loadWorkTags(id);
      }

      const loadWork = useCallback(async (workId: string) => {
            setIsLoading(true);
            try {
                  setWork(await getWork(workId) as Work);
            } catch (err) {
                  console.error('Failed to load work:', err);
            } finally {
                  setIsLoading(false);
            }
      }, []);

      async function loadCharacters(workId: string) {
            try {
                  const data = await invoke<CharacterRow[]>('list_characters', { workId });
                  setCharacters(data || []);
            } catch { setCharacters([]); }
      }

      async function loadCompletion(workId: string) {
            try {
                  const data = await invoke<CompletionInfo | null>('get_completion', { workId });
                  setCompletion(data);
            } catch { }
      }

      async function loadCredits(workId: string) {
            try {
                  const data = await invoke<WorkCreditRow[]>('list_work_credits', { workId });
                  setCredits(data || []);
            } catch {
                  setCredits([]);
            }
      }

      async function loadVariants(workId: string) {
            try {
                  const data = await invoke<WorkVariantRow[]>('list_work_variants', { workId });
                  setVariants(data || []);
            } catch {
                  setVariants([]);
            }
      }

      async function loadAssetGroups(workId: string) {
            try {
                  const data = await invoke<WorkAssetGroupRow[]>('list_work_asset_groups', { workId });
                  setAssetGroups(data || []);
            } catch {
                  setAssetGroups([]);
            }
      }

      async function handleCompletionChange(status: string) {
            if (!id) return;
            try {
                  await invoke('update_completion', {
                        workId: id,
                        status,
                        progressPct: status === 'completed' ? 100 : completion?.progress_pct || 0,
                        playtimeMin: completion?.playtime_min || 0,
                  });
                  loadCompletion(id);
            } catch { }
      }

      async function handleTranslate() {
            if (!id || !work?.description) return;
            setIsTranslating(true);
            try {
                  const result = await invoke<{ translated: string }>('translate_text', {
                        workId: id,
                        fieldName: 'description',
                        text: work.description,
                        sourceLang: 'ja',
                        targetLang: 'en',
                  });
                  setTranslatedDesc(result.translated);
            } catch (e) {
                  alert('Translation failed. Configure your AI gateway in Settings first. ' + e);
            } finally {
                  setIsTranslating(false);
            }
      }

      async function handleFieldSave(field: string) {
            if (!id) return;
            const rawValue = fieldDrafts[field] ?? '';
            const value = field === 'rating'
                  ? Number(rawValue)
                  : field === 'title_aliases'
                        ? rawValue
                              .split(/\r?\n|,/)
                              .map((entry) => entry.trim())
                              .filter(Boolean)
                        : rawValue;
            setFieldAction(`save:${field}`);
            try {
                  await invoke('update_work_field', {
                        id,
                        field,
                        value,
                  });
                  await loadWork(id);
            } finally {
                  setFieldAction(null);
            }
      }

      async function handleFieldReset(field: string) {
            if (!id) return;
            setFieldAction(`reset:${field}`);
            try {
                  await invoke('reset_work_field_override', { id, field });
                  await loadWork(id);
            } finally {
                  setFieldAction(null);
            }
      }

      async function handleFieldPreference(field: string, source: string | null) {
            if (!id) return;
            setFieldAction(`pref:${field}:${source ?? 'auto'}`);
            try {
                  await invoke('set_work_field_preference', {
                        workId: id,
                        field,
                        source,
                  });
                  await loadWork(id);
            } finally {
                  setFieldAction(null);
            }
      }

      if (isLoading) {
            return (
                  <div className="detail-loading">
                        <div className="loading-spinner" />
                        Loading...
                  </div>
            );
      }

      if (!work) {
            return (
                  <div className="detail-empty">
                        <h2>Work not found</h2>
                        <button onClick={() => navigate('/library')}>Back to Library</button>
                  </div>
            );
      }

      const coverUrl = toAssetUrl(work.cover_path);
      const descriptionText = work.description ? sanitizeDescription(work.description) : null;
      const descriptionParagraphs = descriptionText
            ? descriptionText.split(/\n{2,}/).map((paragraph) => paragraph.trim()).filter(Boolean)
            : [];
      const creditSections = [
            { key: 'writer', label: 'Writing' },
            { key: 'artist', label: 'Art' },
            { key: 'composer', label: 'Music' },
            { key: 'director', label: 'Direction' },
            { key: 'voice_actor', label: 'Voice Cast' },
            { key: 'staff', label: 'Staff' },
      ]
            .map((section) => ({
                  ...section,
                  items: credits.filter((credit) => credit.role === section.key),
            }))
            .filter((section) => section.items.length > 0);
      const provenanceEntries: ProvenanceEntry[] = [
            {
                  key: 'title',
                  label: 'Title',
                  source: work.field_sources?.title || work.title_source || 'filesystem',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.bangumi_id ? 'bangumi' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'developer',
                  label: 'Developer',
                  source: work.field_sources?.developer || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'publisher',
                  label: 'Publisher',
                  source: work.field_sources?.publisher || '',
                  availableSources: [],
            },
            {
                  key: 'release_date',
                  label: 'Release Date',
                  source: work.field_sources?.release_date || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.bangumi_id ? 'bangumi' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'rating',
                  label: 'Rating',
                  source: work.field_sources?.rating || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.bangumi_id ? 'bangumi' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'description',
                  label: 'Description',
                  source: work.field_sources?.description || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.bangumi_id ? 'bangumi' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'cover_path',
                  label: 'Cover',
                  source: work.field_sources?.cover_path || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.bangumi_id ? 'bangumi' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'tags',
                  label: 'Auto Tags',
                  source: work.field_sources?.tags || '',
                  availableSources: [work.vndb_id ? 'vndb' : '', work.dlsite_id ? 'dlsite' : ''].filter(Boolean),
            },
            {
                  key: 'library_status',
                  label: 'Library Status',
                  source: work.field_sources?.library_status || '',
                  availableSources: [],
            },
      ].filter((entry) => entry.source);
      const editableFields = [
            { key: 'title', label: 'Title', multiline: false, numeric: false, inputType: 'text' },
            { key: 'title_aliases', label: 'Aliases', multiline: true, numeric: false, inputType: 'text' },
            { key: 'developer', label: 'Developer', multiline: false, numeric: false, inputType: 'text' },
            { key: 'publisher', label: 'Publisher', multiline: false, numeric: false, inputType: 'text' },
            { key: 'release_date', label: 'Release Date', multiline: false, numeric: false, inputType: 'date' },
            { key: 'cover_path', label: 'Cover Path / URL', multiline: false, numeric: false, inputType: 'text' },
            { key: 'rating', label: 'Rating', multiline: false, numeric: true, inputType: 'number' },
            { key: 'description', label: 'Description', multiline: true, numeric: false, inputType: 'text' },
      ];

      return (
            <div className="work-detail">
                  <header className="detail-header">
                        <button className="back-btn" onClick={() => navigate(-1)}>← Back</button>
                        <div className="detail-status-badge" style={{ color: statusColor(work.library_status) }}>
                              {statusLabel(work.library_status)}
                        </div>
                  </header>

                  <div className="detail-content">
                        {/* Left: Cover */}
                        <div className="detail-cover">
                              {coverUrl ? (
                                    <img src={coverUrl} alt={work.title} draggable={false} />
                              ) : (
                                    <div className="detail-cover-placeholder"><span>🎮</span></div>
                              )}
                        </div>

                        {/* Right: Info */}
                        <div className="detail-info">
                              <h1 className="detail-title">{work.title}</h1>
                              {work.title_original && <p className="detail-original-title">{work.title_original}</p>}

                              {/* Metadata grid */}
                              <div className="detail-meta">
                                    {work.developer && (
                                          <div className="meta-row">
                                                <span className="meta-label">Developer</span>
                                                <span className="meta-value meta-link-btn" onClick={() => navigate(`/brand/${encodeURIComponent(work.developer!)}`)}>
                                                      {work.developer}
                                                </span>
                                          </div>
                                    )}
                                    {work.publisher && (
                                          <div className="meta-row">
                                                <span className="meta-label">Publisher</span>
                                                <span className="meta-value">{work.publisher}</span>
                                          </div>
                                    )}
                                    {work.release_date && (
                                          <div className="meta-row">
                                                <span className="meta-label">Release Date</span>
                                                <span className="meta-value">{work.release_date}</span>
                                          </div>
                                    )}
                                    {work.rating !== null && (
                                          <div className="meta-row">
                                                <span className="meta-label">Rating</span>
                                                <span className="meta-value rating-value">
                                                      ★ {formatRating(work.rating)}
                                                      {work.vote_count !== null && <span className="vote-count">({work.vote_count} votes)</span>}
                                                </span>
                                          </div>
                                    )}
                                    <div className="meta-row">
                                          <span className="meta-label">Enrichment</span>
                                          <span className="meta-value">{enrichmentLabel(work.enrichment_state)}</span>
                                    </div>
                                    {work.vndb_id && (
                                          <div className="meta-row">
                                                <span className="meta-label">VNDB</span>
                                                <a className="meta-link" href={`https://vndb.org/${work.vndb_id}`} target="_blank" rel="noreferrer">
                                                      {work.vndb_id}
                                                </a>
                                          </div>
                                    )}
                                    {work.bangumi_id && (
                                          <div className="meta-row">
                                                <span className="meta-label">Bangumi</span>
                                                <a className="meta-link" href={`https://bgm.tv/subject/${work.bangumi_id}`} target="_blank" rel="noreferrer">
                                                      {work.bangumi_id}
                                                </a>
                                          </div>
                                    )}
                                    {work.dlsite_id && (
                                          <div className="meta-row">
                                                <span className="meta-label">DLsite</span>
                                                <a className="meta-link" href={`https://www.dlsite.com/pro/work/=/product_id/${work.dlsite_id}.html`} target="_blank" rel="noreferrer">
                                                      {work.dlsite_id}
                                                </a>
                                          </div>
                                    )}
                              </div>

                              {provenanceEntries.length > 0 && (
                                    <div className="detail-provenance">
                                          <div className="detail-section-header">
                                                <h3>Metadata Provenance</h3>
                                                <span className="detail-section-count">{provenanceEntries.length} tracked</span>
                                          </div>
                                          <div className="provenance-grid">
                                                {provenanceEntries.map((entry) => (
                                                      <div key={entry.key} className="provenance-row">
                                                            <div className="provenance-main">
                                                                  <span className="provenance-label">{entry.label}</span>
                                                                  <span className={`provenance-badge source-${entry.source}`}>{sourceLabel(entry.source)}</span>
                                                            </div>
                                                            {(entry.availableSources.length > 0 || work.field_preferences?.[entry.key]) && (
                                                                  <div className="provenance-actions">
                                                                        <button
                                                                              className={`provenance-btn ${!work.field_preferences?.[entry.key] ? 'active' : ''}`}
                                                                              onClick={() => handleFieldPreference(entry.key, null)}
                                                                              disabled={fieldAction === `pref:${entry.key}:auto`}
                                                                        >
                                                                              Auto
                                                                        </button>
                                                                        {entry.availableSources.map((source) => (
                                                                              <button
                                                                                    key={`${entry.key}:${source}`}
                                                                                    className={`provenance-btn ${work.field_preferences?.[entry.key] === source ? 'active' : ''}`}
                                                                                    onClick={() => handleFieldPreference(entry.key, source)}
                                                                                    disabled={fieldAction === `pref:${entry.key}:${source}`}
                                                                              >
                                                                                    {sourceLabel(source)}
                                                                              </button>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                      </div>
                                                ))}
                                          </div>
                                    </div>
                              )}

                              <div className="detail-editor">
                                    <div className="detail-section-header">
                                          <h3>Field Controls</h3>
                                          <span className="detail-section-count">{editableFields.length} editable</span>
                                    </div>
                                    <div className="editor-grid">
                                          {editableFields.map((field) => (
                                                <div key={field.key} className="editor-card">
                                                      <div className="editor-topline">
                                                            <label htmlFor={`edit-${field.key}`}>{field.label}</label>
                                                            <span className="editor-source">
                                                                  {sourceLabel(work.field_sources?.[field.key] || (field.key === 'title' ? work.title_source : 'filesystem'))}
                                                            </span>
                                                      </div>
                                                      {field.multiline ? (
                                                            <textarea
                                                                  id={`edit-${field.key}`}
                                                                  className="editor-input editor-textarea"
                                                                  value={fieldDrafts[field.key] ?? ''}
                                                                  onChange={(e) => setFieldDrafts((current) => ({ ...current, [field.key]: e.target.value }))}
                                                            />
                                                      ) : (
                                                            <input
                                                                  id={`edit-${field.key}`}
                                                                  className="editor-input"
                                                                  type={field.inputType}
                                                                  step={field.numeric ? '0.1' : undefined}
                                                                  value={fieldDrafts[field.key] ?? ''}
                                                                  onChange={(e) => setFieldDrafts((current) => ({ ...current, [field.key]: e.target.value }))}
                                                            />
                                                      )}
                                                      <div className="editor-actions">
                                                            <button
                                                                  className="editor-btn primary"
                                                                  onClick={() => handleFieldSave(field.key)}
                                                                  disabled={fieldAction === `save:${field.key}` || !(fieldDrafts[field.key] ?? '').trim()}
                                                            >
                                                                  {fieldAction === `save:${field.key}` ? 'Saving…' : 'Save'}
                                                            </button>
                                                            <button
                                                                  className="editor-btn"
                                                                  onClick={() => handleFieldReset(field.key)}
                                                                  disabled={fieldAction === `reset:${field.key}`}
                                                            >
                                                                  {fieldAction === `reset:${field.key}` ? 'Resetting…' : 'Reset to Auto'}
                                                            </button>
                                                      </div>
                                                </div>
                                          ))}
                                    </div>
                              </div>

                              {/* ── Completion Tracking ── */}
                              <div className="detail-completion">
                                    <h3>Progress</h3>
                                    <div className="completion-controls">
                                          <select
                                                id="completion-status"
                                                name="completion-status"
                                                value={completion?.status || 'not_started'}
                                                onChange={(e) => handleCompletionChange(e.target.value)}
                                                className="completion-select"
                                          >
                                                {COMPLETION_STATUSES.map(s => (
                                                      <option key={s.value} value={s.value}>{s.label}</option>
                                                ))}
                                          </select>
                                          {completion && completion.playtime_min > 0 && (
                                                <span className="playtime">⏱ {Math.round(completion.playtime_min / 60)}h</span>
                                          )}
                                    </div>
                                    {completion && completion.progress_pct > 0 && (
                                          <div className="progress-bar">
                                                <div className="progress-fill" style={{ width: `${completion.progress_pct}%` }} />
                                          </div>
                                    )}
                              </div>

                              {/* Auto Tags (read-only) */}
                              {work.tags.length > 0 && (
                                    <div className="detail-tags">
                                          <h3>Auto Tags</h3>
                                          <div className="tag-list">
                                                {work.tags.map(tag => <span key={tag} className="tag-chip">{tag}</span>)}
                                          </div>
                                    </div>
                              )}

                              {/* Interactive Tag Editor */}
                              <div className="detail-tags user-tags">
                                    <h3>My Tags</h3>
                                    <div className="tag-list">
                                          {workTags.filter(t => t.tag_type === 'user').map(tag => (
                                                <span key={tag.id} className="tag-chip user editable">
                                                      {tag.name}
                                                      <button className="tag-remove" onClick={() => handleRemoveTag(tag.id)}>✕</button>
                                                </span>
                                          ))}
                                    </div>
                                    <div className="tag-input-row">
                                          <input
                                                id="work-tag-input"
                                                name="work-tag-input"
                                                type="text"
                                                className="tag-input"
                                                placeholder="Add tag..."
                                                value={tagInput}
                                                onChange={(e) => handleTagSearch(e.target.value)}
                                                onKeyDown={(e) => e.key === 'Enter' && handleCreateTag()}
                                          />
                                          {tagInput && (
                                                <button className="tag-create-btn" onClick={handleCreateTag}>+ Create</button>
                                          )}
                                    </div>
                                    {tagSuggestions.length > 0 && (
                                          <div className="tag-suggestions">
                                                {tagSuggestions.map(s => (
                                                      <button key={s.id} className="tag-suggestion" onClick={() => handleAddTag(s)}>
                                                            <span className={`sug-type ${s.tag_type}`}>{s.tag_type}</span>
                                                            {s.name}
                                                      </button>
                                                ))}
                                          </div>
                                    )}
                              </div>

                              {creditSections.length > 0 && (
                                    <div className="detail-credits">
                                          <h3>Staff & Cast</h3>
                                          <div className="credit-sections">
                                                {creditSections.map((section) => (
                                                      <section key={section.key} className="credit-section">
                                                            <h4>{section.label}</h4>
                                                            <div className="credit-list">
                                                                  {section.items.map((credit) => (
                                                                        <button
                                                                              key={`${credit.person_id}-${credit.role}-${credit.character_name ?? ''}`}
                                                                              className="credit-card"
                                                                              onClick={() => navigate(`/person/${credit.person_id}`)}
                                                                        >
                                                                              <div className="credit-avatar">
                                                                                    {credit.image_url ? (
                                                                                          <img src={toAssetUrl(credit.image_url) ?? credit.image_url} alt={credit.name} loading="lazy" />
                                                                                    ) : (
                                                                                          credit.name.charAt(0)
                                                                                    )}
                                                                              </div>
                                                                              <div className="credit-info">
                                                                                    <span className="credit-name">{credit.name}</span>
                                                                                    {credit.name_original && <span className="credit-original">{credit.name_original}</span>}
                                                                                    {credit.character_name && <span className="credit-meta">as {credit.character_name}</span>}
                                                                                    {credit.notes && <span className="credit-meta">{credit.notes}</span>}
                                                                              </div>
                                                                        </button>
                                                                  ))}
                                                            </div>
                                                      </section>
                                                ))}
                                          </div>
                                    </div>
                              )}

                              <div className="detail-variants">
                                    <div className="detail-section-header">
                                          <h3>Variants & Sources</h3>
                                          <span className="detail-section-count">{variants.length} folder{variants.length !== 1 ? 's' : ''}</span>
                                    </div>
                                    {variants.length === 0 ? (
                                          <div className="detail-empty-note">No grouped source folders are attached to this poster yet.</div>
                                    ) : (
                                          <div className="variant-list">
                                                {variants.map((variant) => (
                                                      <div key={variant.id} className={`variant-card ${variant.is_representative ? 'representative' : ''}`}>
                                                            <div className="variant-main">
                                                                  <div className="variant-title-row">
                                                                        <strong>{variant.title}</strong>
                                                                        {variant.is_representative && <span className="variant-badge">Canonical</span>}
                                                                  </div>
                                                                  <span className="variant-path">{variant.folder_path}</span>
                                                            </div>
                                                            <div className="variant-meta">
                                                                  {variant.developer && <span>{variant.developer}</span>}
                                                                  <span>{variant.asset_count} assets</span>
                                                                  {variant.has_completion && <span>progress</span>}
                                                                  <span>{enrichmentLabel(variant.enrichment_state)}</span>
                                                            </div>
                                                            {variant.asset_types.length > 0 && (
                                                                  <div className="variant-assets">
                                                                        {variant.asset_types.map((assetType) => (
                                                                              <span key={`${variant.id}-${assetType}`} className="variant-asset-chip">{assetType}</span>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                      </div>
                                                ))}
                                          </div>
                                    )}
                              </div>

                              <div className="detail-variants">
                                    <div className="detail-section-header">
                                          <h3>Asset Groups</h3>
                                          <span className="detail-section-count">{assetGroups.length} group{assetGroups.length !== 1 ? 's' : ''}</span>
                                    </div>
                                    {assetGroups.length === 0 ? (
                                          <div className="detail-empty-note">No asset relationships have been grouped for this poster yet.</div>
                                    ) : (
                                          <div className="variant-list">
                                                {assetGroups.map((group) => (
                                                      <div key={group.asset_type} className="variant-card">
                                                            <div className="variant-main">
                                                                  <div className="variant-title-row">
                                                                        <strong>{group.asset_type}</strong>
                                                                        <span className="variant-badge subtle">{group.asset_count} assets</span>
                                                                        <span className="variant-badge subtle">{group.variant_count} variants</span>
                                                                        <span className="variant-badge subtle">{group.relation_role}</span>
                                                                  </div>
                                                                  {group.representative_path && (
                                                                        <span className="variant-path">{group.representative_path}</span>
                                                                  )}
                                                                  {group.parent_asset_type && (
                                                                        <span className="variant-path">Attached to {group.parent_asset_type}</span>
                                                                  )}
                                                            </div>
                                                            {group.variants.length > 0 && (
                                                                  <div className="variant-assets">
                                                                        {group.variants.slice(0, 4).map((variant) => (
                                                                              <span key={`${group.asset_type}:${variant.work_id}`} className="variant-asset-chip">
                                                                                    {variant.asset_count} · {variant.folder_path.split(/[\\/]/).pop()}
                                                                              </span>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                      </div>
                                                ))}
                                          </div>
                                    )}
                              </div>

                              {/* Description + Translate */}
                              {descriptionParagraphs.length > 0 && (
                                    <div className="detail-description">
                                          <div className="desc-header">
                                                <h3>Description</h3>
                                                <button className="translate-btn" onClick={handleTranslate} disabled={isTranslating}>
                                                      {isTranslating ? '⟳ Translating...' : '🌐 Translate'}
                                                </button>
                                          </div>
                                          {translatedDesc && <p className="translated-text">{translatedDesc}</p>}
                                          {descriptionParagraphs.map((paragraph, index) => (
                                                <p key={`${work.id}-desc-${index}`}>{paragraph}</p>
                                          ))}
                                    </div>
                              )}

                              {/* ── Characters ── */}
                              {characters.length > 0 && (
                                    <div className="detail-characters">
                                          <h3>Characters ({characters.length})</h3>
                                          <div className="char-grid">
                                                {characters.map(c => (
                                                      <div key={c.id} className="char-card">
                                                            <div className="char-avatar">
                                                                  {c.image_url ? (
                                                                        <img src={toAssetUrl(c.image_url) ?? c.image_url} alt={c.name} loading="lazy" />
                                                                  ) : (
                                                                        c.name.charAt(0)
                                                                  )}
                                                            </div>
                                                            <div className="char-info">
                                                                  <span className="char-name">{c.name}</span>
                                                                  {c.name_original && <span className="char-jp">{c.name_original}</span>}
                                                                  {c.role && <span className="char-role">{c.role}</span>}
                                                            </div>
                                                      </div>
                                                ))}
                                          </div>
                                    </div>
                              )}

                              {/* Folder path */}
                              <div className="detail-path">
                                    <span className="path-label">Folder</span>
                                    <code>{work.folder_path}</code>
                              </div>
                        </div>
                  </div>
            </div>
      );
}
