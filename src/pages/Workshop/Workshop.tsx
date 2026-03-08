// Workshop — bulk metadata editing, merge duplicates, gap analysis.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { listWorks, type WorkSummary } from '../../hooks/api';
import './Workshop.css';

interface GapReport {
      unmatched_works: number;
      works_without_cover: number;
      posters_with_variants: number;
      works_missing_assets: { work_id: string; title: string; missing: string[] }[];
      enrichment_diagnostics: EnrichmentDiagnostic[];
      ignored_diagnostics: EnrichmentDiagnostic[];
}

interface EnrichmentDiagnostic {
      work_id: string;
      title: string;
      severity: 'critical' | 'warn' | 'info' | string;
      category: string;
      reason: string;
      suggested_action: string;
      details: string[];
      linked_sources: string[];
      preferred_field: string | null;
      preferred_source: string | null;
}

interface BatchWorkshopResult {
      updated: number;
      skipped: number;
}

interface ImportItem {
      id: string;
      file_name: string;
      detected_type: string;
      status: string;
}

interface DuplicateEntry {
      id: string;
      folder_path: string;
      title: string;
      developer: string | null;
      cover_path: string | null;
      enrichment_state: string;
      asset_count: number;
      asset_types: string[];
      has_completion: boolean;
      has_people: boolean;
      is_representative: boolean;
      manual_group_key: string | null;
      manual_representative: boolean;
}

interface DuplicateGroup {
      title: string;
      representative_id: string;
      representative_cover_path: string | null;
      variant_count: number;
      review_flags: string[];
      entries: DuplicateEntry[];
}

const FLAG_LABELS: Record<string, string> = {
      'title-conflict': 'Title conflict',
      'developer-conflict': 'Developer conflict',
      'mixed-assets': 'Mixed assets',
      'manual-review': 'Manual review',
      'needs-enrichment': 'Needs enrichment',
};

const DIAGNOSTIC_LABELS: Record<string, string> = {
      enrichment: 'Enrichment',
      cover: 'Cover',
      assets: 'Assets',
      variants: 'Variants',
      'metadata-depth': 'Metadata Depth',
      'title-quality': 'Title Quality',
};

export default function Workshop() {
      const navigate = useNavigate();
      const [tab, setTab] = useState<'gap' | 'dedupe' | 'import' | 'matcher'>('gap');
      const [gap, setGap] = useState<GapReport | null>(null);
      const [imports, setImports] = useState<ImportItem[]>([]);
      const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
      const [posters, setPosters] = useState<WorkSummary[]>([]);
      const [matchResults, setMatchResults] = useState<string[]>([]);
      const [isMatching, setIsMatching] = useState(false);
      const [mergingId, setMergingId] = useState<string | null>(null);
      const [workingAction, setWorkingAction] = useState<string | null>(null);
      const [selectedDiagnostics, setSelectedDiagnostics] = useState<string[]>([]);
      const [selectedIgnoredDiagnostics, setSelectedIgnoredDiagnostics] = useState<string[]>([]);
      const [providerDefaults, setProviderDefaults] = useState<Record<string, string>>({});
      const [sourcePosterId, setSourcePosterId] = useState<string>('');
      const [targetPosterId, setTargetPosterId] = useState<string>('');
      const [isPosterMerging, setIsPosterMerging] = useState(false);

      useEffect(() => {
            void refreshAll();
      }, []);

      async function refreshAll() {
            await Promise.all([loadGap(), loadImports(), loadDuplicates(), loadPosters(), loadProviderDefaults()]);
      }

      async function loadGap() {
            try {
                  setGap(await invoke<GapReport>('get_gap_analysis'));
            } catch {
            }
      }

      async function loadImports() {
            try {
                  setImports(await invoke<ImportItem[]>('list_import_queue'));
            } catch {
            }
      }

      async function loadDuplicates() {
            try {
                  setDuplicates(await invoke<DuplicateGroup[]>('find_duplicates'));
            } catch {
            }
      }

      async function loadPosters() {
            try {
                  setPosters(await listWorks(1, 200));
            } catch {
            }
      }

      async function loadProviderDefaults() {
            try {
                  setProviderDefaults(await invoke<Record<string, string>>('list_provider_field_defaults'));
            } catch {
                  setProviderDefaults({});
            }
      }

      async function clearDone() {
            try {
                  await invoke('clear_import_queue');
                  await loadImports();
            } catch {
            }
      }

      async function runBatchMatch() {
            setIsMatching(true);
            try {
                  const results = await invoke<string[]>('batch_multi_source_match');
                  setMatchResults(results);
                  await refreshAll();
            } catch {
                  setMatchResults(['Error running batch match']);
            }
            setIsMatching(false);
      }

      async function mergeGroup(group: DuplicateGroup) {
            const sources = group.entries.filter((entry) => !entry.is_representative);
            if (sources.length === 0) return;
            setMergingId(group.representative_id);
            try {
                  for (const entry of sources) {
                        await invoke('merge_works', {
                              targetId: group.representative_id,
                              sourceId: entry.id,
                        });
                  }
                  await refreshAll();
            } finally {
                  setMergingId(null);
            }
      }

      async function runRowAction(actionKey: string, task: () => Promise<void>) {
            setWorkingAction(actionKey);
            try {
                  await task();
                  await refreshAll();
            } finally {
                  setWorkingAction(null);
            }
      }

      async function runDiagnosticAction(actionKey: string, task: () => Promise<void>) {
            setWorkingAction(actionKey);
            try {
                  await task();
                  await loadGap();
            } finally {
                  setWorkingAction(null);
            }
      }

      async function runBatchDiagnosticAction(actionKey: string, task: () => Promise<void>) {
            setWorkingAction(actionKey);
            try {
                  await task();
                  await Promise.all([loadGap(), loadProviderDefaults()]);
            } finally {
                  setWorkingAction(null);
            }
      }

      function diagnosticKey(item: EnrichmentDiagnostic) {
            return `${item.work_id}:${item.category}`;
      }

      function toggleDiagnosticSelection(item: EnrichmentDiagnostic, ignored: boolean) {
            const key = diagnosticKey(item);
            const setter = ignored ? setSelectedIgnoredDiagnostics : setSelectedDiagnostics;
            setter((current) => current.includes(key) ? current.filter((value) => value !== key) : [...current, key]);
      }

      const selectedActiveItems = gap?.enrichment_diagnostics.filter((item) => selectedDiagnostics.includes(diagnosticKey(item))) ?? [];
      const selectedIgnoredItems = gap?.ignored_diagnostics.filter((item) => selectedIgnoredDiagnostics.includes(diagnosticKey(item))) ?? [];

      async function setProviderDefault(field: string, source: string | null) {
            await runBatchDiagnosticAction(`defaults:${field}:${source ?? 'auto'}`, async () => {
                  await invoke('set_provider_field_default', { field, source });
            });
      }

      async function batchIgnoreSelected() {
            if (selectedActiveItems.length === 0) return;
            await runBatchDiagnosticAction('batch:ignore', async () => {
                  await invoke<BatchWorkshopResult>('batch_ignore_workshop_diagnostics', {
                        items: selectedActiveItems,
                  });
                  setSelectedDiagnostics([]);
            });
      }

      async function batchRestoreSelected() {
            if (selectedIgnoredItems.length === 0) return;
            await runBatchDiagnosticAction('batch:restore', async () => {
                  await invoke<BatchWorkshopResult>('batch_restore_workshop_diagnostics', {
                        items: selectedIgnoredItems,
                  });
                  setSelectedIgnoredDiagnostics([]);
            });
      }

      async function batchApplyPreference(source: string | null) {
            if (selectedActiveItems.length === 0) return;
            await runBatchDiagnosticAction(`batch:pref:${source ?? 'auto'}`, async () => {
                  await invoke<BatchWorkshopResult>('batch_apply_diagnostic_preferences', {
                        items: selectedActiveItems,
                        source,
                  });
            });
      }

      async function batchRefreshSource(source: string) {
            if (selectedActiveItems.length === 0) return;
            await runBatchDiagnosticAction(`batch:refresh:${source}`, async () => {
                  await invoke<BatchWorkshopResult>('batch_refresh_work_provider_links', {
                        workIds: selectedActiveItems.map((item) => item.work_id),
                        source,
                  });
            });
      }

      async function mergePosterGroups() {
            if (!sourcePosterId || !targetPosterId || sourcePosterId === targetPosterId) return;
            setIsPosterMerging(true);
            try {
                  await invoke('merge_poster_groups', {
                        targetId: targetPosterId,
                        sourceId: sourcePosterId,
                  });
                  await refreshAll();
                  setSourcePosterId('');
            } finally {
                  setIsPosterMerging(false);
            }
      }

      return (
            <div className="workshop-page">
                  <h1>🔧 Workshop</h1>

                  <div className="ws-tabs">
                        <button className={tab === 'gap' ? 'active' : ''} onClick={() => setTab('gap')}>📊 Health Check</button>
                        <button className={tab === 'dedupe' ? 'active' : ''} onClick={() => setTab('dedupe')}>🧬 Review Ops ({duplicates.length})</button>
                        <button className={tab === 'import' ? 'active' : ''} onClick={() => setTab('import')}>📥 Import Queue ({imports.length})</button>
                        <button className={tab === 'matcher' ? 'active' : ''} onClick={() => setTab('matcher')}>🔗 Auto Matcher</button>
                  </div>

                  {tab === 'gap' && gap && (
                        <div className="gap-section">
                              <div className="gap-stats">
                                    <div className="gap-card warn"><span className="gap-num">{gap.unmatched_works}</span><span>Unmatched</span></div>
                                    <div className="gap-card warn"><span className="gap-num">{gap.works_without_cover}</span><span>No Cover</span></div>
                                    <div className="gap-card info"><span className="gap-num">{gap.works_missing_assets.length}</span><span>Missing Core Assets</span></div>
                                    <div className="gap-card info"><span className="gap-num">{gap.posters_with_variants}</span><span>Multi-Source Posters</span></div>
                              </div>

                              <section className="poster-merge-studio provider-rules-panel">
                                    <div>
                                          <h3>Provider Rules</h3>
                                          <p>Workspace-wide defaults used by the resolver before per-poster overrides.</p>
                                    </div>
                                    <div className="provider-rule-list">
                                          {[
                                                { field: 'title', label: 'Title' },
                                                { field: 'developer', label: 'Developer' },
                                                { field: 'release_date', label: 'Release Date' },
                                                { field: 'rating', label: 'Rating' },
                                                { field: 'description', label: 'Description' },
                                                { field: 'tags', label: 'Tags' },
                                                { field: 'cover_path', label: 'Cover' },
                                          ].map((rule) => (
                                                <div key={rule.field} className="provider-rule-row">
                                                      <span>{rule.label}</span>
                                                      <div className="diagnostic-source-actions">
                                                            <button
                                                                  className={`row-action ghost ${!providerDefaults[rule.field] ? 'active' : ''}`}
                                                                  disabled={workingAction === `defaults:${rule.field}:auto`}
                                                                  onClick={() => void setProviderDefault(rule.field, null)}
                                                            >
                                                                  Auto
                                                            </button>
                                                            {['vndb', 'bangumi', 'dlsite'].map((source) => (
                                                                  <button
                                                                        key={`${rule.field}:${source}`}
                                                                        className={`row-action ghost ${providerDefaults[rule.field] === source ? 'active' : ''}`}
                                                                        disabled={workingAction === `defaults:${rule.field}:${source}`}
                                                                        onClick={() => void setProviderDefault(rule.field, source)}
                                                                  >
                                                                        {source.toUpperCase()}
                                                                  </button>
                                                            ))}
                                                      </div>
                                                </div>
                                          ))}
                                    </div>
                              </section>

                              {gap.enrichment_diagnostics.length > 0 && (
                                    <div className="gap-list gap-diagnostics">
                                          <div className="gap-list-head">
                                                <div>
                                                      <h3>Priority Diagnostics</h3>
                                                      <p>Poster-level issues ordered by severity, with a suggested next action.</p>
                                                </div>
                                                <div className="gap-head-actions">
                                                      <span className="gap-count">{gap.enrichment_diagnostics.length} items</span>
                                                      <button className="row-action ghost" onClick={() => setSelectedDiagnostics(gap.enrichment_diagnostics.map((item) => diagnosticKey(item)))}>
                                                            Select All
                                                      </button>
                                                      <button className="row-action ghost" onClick={() => setSelectedDiagnostics([])}>
                                                            Clear
                                                      </button>
                                                </div>
                                          </div>
                                          <div className={`diagnostic-batch-bar ${selectedActiveItems.length === 0 ? 'is-disabled' : ''}`}>
                                                      <span>{selectedActiveItems.length} selected</span>
                                                      <button className="row-action ghost" disabled={selectedActiveItems.length === 0 || workingAction === 'batch:ignore'} onClick={() => void batchIgnoreSelected()}>
                                                            Ignore Selected
                                                      </button>
                                                      <button className="row-action ghost" disabled={selectedActiveItems.length === 0 || workingAction === 'batch:pref:auto'} onClick={() => void batchApplyPreference(null)}>
                                                            Prefer Auto
                                                      </button>
                                                      {['vndb', 'bangumi', 'dlsite'].map((source) => (
                                                            <button key={`pref-${source}`} className="row-action ghost" disabled={selectedActiveItems.length === 0 || workingAction === `batch:pref:${source}`} onClick={() => void batchApplyPreference(source)}>
                                                                  Prefer {source.toUpperCase()}
                                                            </button>
                                                      ))}
                                                      {['vndb', 'bangumi', 'dlsite'].map((source) => (
                                                            <button key={`refresh-${source}`} className="row-action" disabled={selectedActiveItems.length === 0 || workingAction === `batch:refresh:${source}`} onClick={() => void batchRefreshSource(source)}>
                                                                  Refresh {source.toUpperCase()}
                                                            </button>
                                                      ))}
                                          </div>
                                          {gap.enrichment_diagnostics.map((item) => (
                                                <article key={`${item.category}:${item.work_id}:${item.reason}`} className={`diagnostic-card ${item.severity}`}>
                                                      <div className="diagnostic-head">
                                                            <label className="diagnostic-select">
                                                                  <input
                                                                        type="checkbox"
                                                                        checked={selectedDiagnostics.includes(diagnosticKey(item))}
                                                                        onChange={() => toggleDiagnosticSelection(item, false)}
                                                                  />
                                                            </label>
                                                            <div>
                                                                  <div className="diagnostic-title-row">
                                                                        <strong>{item.title}</strong>
                                                                        <span className={`diagnostic-severity ${item.severity}`}>{item.severity}</span>
                                                                        <span className="diagnostic-category">{DIAGNOSTIC_LABELS[item.category] ?? item.category}</span>
                                                                  </div>
                                                                  <p>{item.reason}</p>
                                                            </div>
                                                      </div>
                                                      <div className="diagnostic-action">{item.suggested_action}</div>
                                                      {item.details.length > 0 && (
                                                            <div className="diagnostic-details">
                                                                  {item.details.map((detail) => (
                                                                        <span key={detail} className="diagnostic-detail">{detail}</span>
                                                                  ))}
                                                            </div>
                                                      )}
                                                      <div className="diagnostic-controls">
                                                            <div className="diagnostic-nav-actions">
                                                                  <button className="row-action ghost" onClick={() => navigate(`/work/${item.work_id}`)}>
                                                                        Open Poster
                                                                  </button>
                                                                  {item.category === 'enrichment' && (
                                                                        <button className="row-action ghost" onClick={() => navigate(`/enrichment?work=${encodeURIComponent(item.work_id)}`)}>
                                                                              Open Review
                                                                        </button>
                                                                  )}
                                                            </div>
                                                            {item.preferred_field && item.linked_sources.length > 0 && (
                                                                  <div className="diagnostic-source-actions">
                                                                        <span className="diagnostic-control-label">
                                                                              Prefer for {item.preferred_field === 'cover_path' ? 'cover' : item.preferred_field}
                                                                        </span>
                                                                        <button
                                                                              className={`row-action ghost ${!item.preferred_source ? 'active' : ''}`}
                                                                              disabled={workingAction === `pref:${item.work_id}:${item.preferred_field}:auto`}
                                                                              onClick={() => void runDiagnosticAction(`pref:${item.work_id}:${item.preferred_field}:auto`, async () => {
                                                                                    await invoke('set_work_field_preference', {
                                                                                          workId: item.work_id,
                                                                                          field: item.preferred_field,
                                                                                          source: null,
                                                                                    });
                                                                              })}
                                                                        >
                                                                              Auto
                                                                        </button>
                                                                        {item.linked_sources.map((source) => (
                                                                              <button
                                                                                    key={`${item.work_id}:${item.category}:pref:${source}`}
                                                                                    className={`row-action ghost ${item.preferred_source === source ? 'active' : ''}`}
                                                                                    disabled={workingAction === `pref:${item.work_id}:${item.preferred_field}:${source}`}
                                                                                    onClick={() => void runDiagnosticAction(`pref:${item.work_id}:${item.preferred_field}:${source}`, async () => {
                                                                                          await invoke('set_work_field_preference', {
                                                                                                workId: item.work_id,
                                                                                                field: item.preferred_field,
                                                                                                source,
                                                                                          });
                                                                                    })}
                                                                              >
                                                                                    Prefer {source.toUpperCase()}
                                                                              </button>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                            {item.linked_sources.length > 0 && (
                                                                  <div className="diagnostic-source-actions">
                                                                        <span className="diagnostic-control-label">Refresh source</span>
                                                                        {item.linked_sources.map((source) => (
                                                                              <button
                                                                                    key={`${item.work_id}:${item.category}:refresh:${source}`}
                                                                                    className="row-action"
                                                                                    disabled={workingAction === `refresh:${item.work_id}:${source}`}
                                                                                    onClick={() => void runDiagnosticAction(`refresh:${item.work_id}:${source}`, async () => {
                                                                                          await invoke('refresh_work_provider_link', {
                                                                                                workId: item.work_id,
                                                                                                source,
                                                                                          });
                                                                                    })}
                                                                              >
                                                                                    {workingAction === `refresh:${item.work_id}:${source}` ? 'Refreshing…' : `Refresh ${source.toUpperCase()}`}
                                                                              </button>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                            <div className="diagnostic-nav-actions">
                                                                  <button
                                                                        className="row-action ghost"
                                                                        disabled={workingAction === `ignore:${item.work_id}:${item.category}`}
                                                                        onClick={() => void runDiagnosticAction(`ignore:${item.work_id}:${item.category}`, async () => {
                                                                              await invoke('ignore_workshop_diagnostic', {
                                                                                    workId: item.work_id,
                                                                                    category: item.category,
                                                                              });
                                                                        })}
                                                                  >
                                                                        {workingAction === `ignore:${item.work_id}:${item.category}` ? 'Ignoring…' : 'Ignore Issue'}
                                                                  </button>
                                                            </div>
                                                      </div>
                                                </article>
                                          ))}
                                    </div>
                              )}

                              <div className="gap-list gap-diagnostics">
                                          <div className="gap-list-head">
                                                <div>
                                                      <h3>Ignored Diagnostics</h3>
                                                      <p>Dismissed workshop issues stay hidden until you restore them.</p>
                                                </div>
                                                <div className="gap-head-actions">
                                                      <span className="gap-count">{gap.ignored_diagnostics.length} ignored</span>
                                                      <button className="row-action ghost" onClick={() => setSelectedIgnoredDiagnostics(gap.ignored_diagnostics.map((item) => diagnosticKey(item)))}>
                                                            Select All
                                                      </button>
                                                      <button className="row-action ghost" onClick={() => setSelectedIgnoredDiagnostics([])}>
                                                            Clear
                                                      </button>
                                                </div>
                                          </div>
                                          <div className={`diagnostic-batch-bar ${selectedIgnoredItems.length === 0 ? 'is-disabled' : ''}`}>
                                                      <span>{selectedIgnoredItems.length} selected</span>
                                                      <button className="row-action ghost" disabled={selectedIgnoredItems.length === 0 || workingAction === 'batch:restore'} onClick={() => void batchRestoreSelected()}>
                                                            Restore Selected
                                                      </button>
                                          </div>
                                          {gap.ignored_diagnostics.length === 0 ? (
                                                <div className="detail-empty-note">No ignored diagnostics are stored for this workspace.</div>
                                          ) : (
                                                gap.ignored_diagnostics.map((item) => (
                                                      <article key={`ignored:${item.category}:${item.work_id}:${item.reason}`} className={`diagnostic-card ${item.severity}`}>
                                                            <div className="diagnostic-head">
                                                                  <label className="diagnostic-select">
                                                                        <input
                                                                              type="checkbox"
                                                                              checked={selectedIgnoredDiagnostics.includes(diagnosticKey(item))}
                                                                              onChange={() => toggleDiagnosticSelection(item, true)}
                                                                        />
                                                                  </label>
                                                                  <div>
                                                                        <div className="diagnostic-title-row">
                                                                              <strong>{item.title}</strong>
                                                                              <span className={`diagnostic-severity ${item.severity}`}>{item.severity}</span>
                                                                              <span className="diagnostic-category">{DIAGNOSTIC_LABELS[item.category] ?? item.category}</span>
                                                                        </div>
                                                                        <p>{item.reason}</p>
                                                                  </div>
                                                            </div>
                                                            <div className="diagnostic-controls">
                                                                  <button
                                                                        className="row-action ghost"
                                                                        disabled={workingAction === `restore:${item.work_id}:${item.category}`}
                                                                        onClick={() => void runDiagnosticAction(`restore:${item.work_id}:${item.category}`, async () => {
                                                                              await invoke('restore_workshop_diagnostic', {
                                                                                    workId: item.work_id,
                                                                                    category: item.category,
                                                                              });
                                                                        })}
                                                                  >
                                                                        {workingAction === `restore:${item.work_id}:${item.category}` ? 'Restoring…' : 'Restore Issue'}
                                                                  </button>
                                                            </div>
                                                      </article>
                                                ))
                                          )}
                                    </div>

                              {gap.works_missing_assets.length > 0 && (
                                    <div className="gap-list">
                                          <div className="gap-list-head">
                                                <div>
                                                      <h3>Works Missing Core Assets</h3>
                                                      <p>Posters with no classified assets or no detected game package.</p>
                                                </div>
                                                <span className="gap-count">{gap.works_missing_assets.length} posters</span>
                                          </div>
                                          {gap.works_missing_assets.slice(0, 20).map((w) => (
                                                <div key={w.work_id} className="gap-row">
                                                      <span className="gap-title">{w.title}</span>
                                                      <span className="gap-missing">{w.missing.join(', ')}</span>
                                                      <button className="row-action ghost" onClick={() => navigate(`/work/${w.work_id}`)}>
                                                            Open Poster
                                                      </button>
                                                </div>
                                          ))}
                                    </div>
                              )}
                        </div>
                  )}

                  {tab === 'dedupe' && (
                        <div className="dedupe-section">
                              <section className="poster-merge-studio">
                                    <div>
                                          <h3>Poster Merge Studio</h3>
                                          <p>Attach one poster's variants under another canonical poster without deleting works.</p>
                                    </div>
                                    <div className="poster-merge-grid">
                                          <label>
                                                <span>Source Poster</span>
                                                <select value={sourcePosterId} onChange={(event) => setSourcePosterId(event.target.value)}>
                                                      <option value="">Select poster…</option>
                                                      {posters.map((poster) => (
                                                            <option key={`source-${poster.id}`} value={poster.id}>
                                                                  {poster.title} {poster.variant_count > 1 ? `×${poster.variant_count}` : ''}
                                                            </option>
                                                      ))}
                                                </select>
                                          </label>
                                          <label>
                                                <span>Target Poster</span>
                                                <select value={targetPosterId} onChange={(event) => setTargetPosterId(event.target.value)}>
                                                      <option value="">Select poster…</option>
                                                      {posters.map((poster) => (
                                                            <option key={`target-${poster.id}`} value={poster.id}>
                                                                  {poster.title} {poster.variant_count > 1 ? `×${poster.variant_count}` : ''}
                                                            </option>
                                                      ))}
                                                </select>
                                          </label>
                                    </div>
                                    <button
                                          className="merge-btn"
                                          disabled={!sourcePosterId || !targetPosterId || sourcePosterId === targetPosterId || isPosterMerging}
                                          onClick={() => void mergePosterGroups()}
                                    >
                                          {isPosterMerging ? 'Merging Posters…' : 'Merge Posters'}
                                    </button>
                              </section>

                              {duplicates.length === 0 ? (
                                    <div className="ws-empty">No poster-level duplicates detected. Manual merge is still available above.</div>
                              ) : (
                                    <div className="duplicate-groups">
                                          {duplicates.map((group) => (
                                                <article key={group.representative_id} className="duplicate-group">
                                                      <div className="duplicate-head">
                                                            <div>
                                                                  <h3>{group.title}</h3>
                                                                  <p>{group.variant_count} source folders collapse into one poster</p>
                                                                  {group.review_flags.length > 0 && (
                                                                        <div className="duplicate-flags">
                                                                              {group.review_flags.map((flag) => (
                                                                                    <span key={flag} className="duplicate-flag">{FLAG_LABELS[flag] ?? flag}</span>
                                                                              ))}
                                                                        </div>
                                                                  )}
                                                            </div>
                                                            <button
                                                                  className="merge-btn destructive"
                                                                  onClick={() => void mergeGroup(group)}
                                                                  disabled={mergingId === group.representative_id}
                                                            >
                                                                  {mergingId === group.representative_id ? 'Collapsing…' : 'Collapse Rows'}
                                                            </button>
                                                      </div>
                                                      <div className="duplicate-list">
                                                            {group.entries.map((entry) => {
                                                                  const actionPrefix = `${group.representative_id}:${entry.id}`;
                                                                  return (
                                                                        <div key={entry.id} className={`duplicate-row ${entry.is_representative ? 'canonical' : ''}`}>
                                                                              <div className="duplicate-main">
                                                                                    <div className="duplicate-title-row">
                                                                                          <strong>{entry.title}</strong>
                                                                                          {entry.is_representative && <span className="duplicate-badge">Canonical</span>}
                                                                                          {entry.manual_group_key && <span className="duplicate-badge subtle">Manual</span>}
                                                                                    </div>
                                                                                    <span className="duplicate-path">{entry.folder_path}</span>
                                                                              </div>
                                                                              <div className="duplicate-side">
                                                                                    <div className="duplicate-meta">
                                                                                          <span>{entry.asset_count} assets</span>
                                                                                          {entry.asset_types.slice(0, 3).map((assetType) => <span key={assetType}>{assetType}</span>)}
                                                                                          {entry.has_completion && <span>completion</span>}
                                                                                          {entry.has_people && <span>credits</span>}
                                                                                          <span>{entry.enrichment_state}</span>
                                                                                    </div>
                                                                                    <div className="duplicate-actions">
                                                                                          {!entry.is_representative && (
                                                                                                <button
                                                                                                      className="row-action"
                                                                                                      disabled={workingAction === `${actionPrefix}:canonical`}
                                                                                                      onClick={() => void runRowAction(`${actionPrefix}:canonical`, async () => {
                                                                                                            await invoke('set_canonical_representative', { workId: entry.id });
                                                                                                      })}
                                                                                                >
                                                                                                      {workingAction === `${actionPrefix}:canonical` ? 'Working…' : 'Make Canonical'}
                                                                                                </button>
                                                                                          )}
                                                                                          <button
                                                                                                className="row-action"
                                                                                                disabled={workingAction === `${actionPrefix}:split`}
                                                                                                onClick={() => void runRowAction(`${actionPrefix}:split`, async () => {
                                                                                                      await invoke('split_work_variant', { workId: entry.id });
                                                                                                })}
                                                                                          >
                                                                                                {workingAction === `${actionPrefix}:split` ? 'Working…' : 'Split Poster'}
                                                                                          </button>
                                                                                          {entry.manual_group_key && (
                                                                                                <button
                                                                                                      className="row-action ghost"
                                                                                                      disabled={workingAction === `${actionPrefix}:restore`}
                                                                                                      onClick={() => void runRowAction(`${actionPrefix}:restore`, async () => {
                                                                                                            await invoke('clear_work_variant_override', { workId: entry.id });
                                                                                                      })}
                                                                                                >
                                                                                                      {workingAction === `${actionPrefix}:restore` ? 'Working…' : 'Restore Auto'}
                                                                                                </button>
                                                                                          )}
                                                                                    </div>
                                                                              </div>
                                                                        </div>
                                                                  );
                                                            })}
                                                      </div>
                                                </article>
                                          ))}
                                    </div>
                              )}
                        </div>
                  )}

                  {tab === 'import' && (
                        <div className="import-section">
                              {imports.length === 0 ? (
                                    <div className="ws-empty">Import queue is empty. Drag files here to import.</div>
                              ) : (
                                    <>
                                          <div className="import-list">
                                                {imports.map((item) => (
                                                      <div key={item.id} className="import-row">
                                                            <span className={`import-status ${item.status}`}>{item.status}</span>
                                                            <span className="import-name">{item.file_name}</span>
                                                            <span className="import-type">{item.detected_type}</span>
                                                      </div>
                                                ))}
                                          </div>
                                          <button className="ws-clear-btn" onClick={() => void clearDone()}>Clear Done/Error</button>
                                    </>
                              )}
                        </div>
                  )}

                  {tab === 'matcher' && (
                        <div className="matcher-section">
                              <p className="matcher-desc">Run cascading multi-source matching (VNDB → DLsite → Bangumi) on all unmatched works.</p>
                              <button className="ws-match-btn" onClick={() => void runBatchMatch()} disabled={isMatching}>
                                    {isMatching ? '⟳ Matching...' : '🚀 Run Batch Match (up to 50 works)'}
                              </button>
                              {matchResults.length > 0 && (
                                    <div className="match-results">
                                          <h3>Results ({matchResults.length})</h3>
                                          {matchResults.map((result, index) => (
                                                <div key={index} className="match-result-row">{result}</div>
                                          ))}
                                    </div>
                              )}
                        </div>
                  )}
            </div>
      );
}
