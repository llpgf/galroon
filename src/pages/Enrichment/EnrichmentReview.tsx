import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSearchParams } from 'react-router-dom';
import { toAssetUrl } from '../../hooks/api';
import './EnrichmentReview.css';

interface UnmatchedWork {
      id: string;
      title: string;
      folder_path: string;
      enrichment_state: string;
      linked_sources: string[];
      job_state: string | null;
      attempt_count: number | null;
      next_run_at: string | null;
      last_error: string | null;
      suggested_action: string;
}

interface ReviewWorkInfo {
      id: string;
      title: string;
      folder_path: string;
      developer: string | null;
      release_date: string | null;
      rating: number | null;
      description: string | null;
      cover_path: string | null;
      tags: string[];
      enrichment_state: string;
      title_source: string;
      field_preferences: Record<string, string>;
      vndb_id: string | null;
      bangumi_id: string | null;
      dlsite_id: string | null;
}

interface ReviewLinkedSource {
      source: string;
      source_label: string;
      external_id: string;
      title: string;
      developer: string | null;
      rating: number | null;
      release_date: string | null;
      cover_url: string | null;
      capabilities: string[];
}

interface ReviewFieldDecision {
      field: string;
      label: string;
      current_value: string | null;
      candidate_value: string | null;
      action: string;
      resolved_value: string | null;
      preferred_source: string | null;
      supports_source: boolean;
}

interface ReviewCandidate {
      id: string;
      title: string;
      title_original: string | null;
      developer: string | null;
      rating: number | null;
      source: string;
      source_label: string;
      similarity: number;
      verdict: string;
      release_date: string | null;
      description: string | null;
      cover_url: string | null;
      tags: string[];
      capabilities: string[];
      field_decisions: ReviewFieldDecision[];
}

interface EnrichmentReviewItem {
      work: ReviewWorkInfo;
      query_terms: string[];
      diagnostics: string[];
      job_status: ReviewJobStatus | null;
      provider_refresh: ReviewProviderRefresh[];
      current_sources: ReviewLinkedSource[];
      candidates: ReviewCandidate[];
}

interface ReviewJobStatus {
      state: string;
      attempt_count: number;
      next_run_at: string | null;
      last_error: string | null;
}

interface ReviewProviderRefresh {
      source: string;
      source_label: string;
      external_id: string;
      status: string;
      message: string | null;
      suggested_action: string;
}

function actionLabel(action: string): string {
      switch (action) {
            case 'fill': return 'Fill';
            case 'override': return 'Override';
            case 'keep_current': return 'Keep';
            default: return 'No Data';
      }
}

function sourceLabel(source: string): string {
      const labels: Record<string, string> = {
            auto: 'Auto',
            vndb: 'VNDB',
            bangumi: 'Bangumi',
            dlsite: 'DLsite',
      };
      return labels[source] || source;
}

export default function EnrichmentReview() {
      const [searchParams, setSearchParams] = useSearchParams();
      const [works, setWorks] = useState<UnmatchedWork[]>([]);
      const [selectedId, setSelectedId] = useState<string | null>(null);
      const [reviewItem, setReviewItem] = useState<EnrichmentReviewItem | null>(null);
      const [isLoading, setIsLoading] = useState(true);
      const [isReviewLoading, setIsReviewLoading] = useState(false);
      const [workingAction, setWorkingAction] = useState<string | null>(null);
      const [candidateFieldSelection, setCandidateFieldSelection] = useState<Record<string, string[]>>({});
      const [manualSearchQuery, setManualSearchQuery] = useState('');
      const [manualSource, setManualSource] = useState('vndb');
      const [manualExternalId, setManualExternalId] = useState('');
      const [manualResults, setManualResults] = useState<ReviewCandidate[]>([]);
      const requestedWorkId = searchParams.get('work');
      const hasActiveReview = Boolean(reviewItem);
      const activeQueryTerms = reviewItem?.query_terms ?? [];

      const loadReviewItem = useCallback(async (workId: string) => {
            setIsReviewLoading(true);
            try {
                  const item = await invoke<EnrichmentReviewItem>('get_enrichment_review_item', { workId });
                  setReviewItem(item);
                  setSelectedId(workId);
                  setCandidateFieldSelection((current) => {
                        const next = { ...current };
                        for (const candidate of item.candidates) {
                              const key = `${candidate.source}:${candidate.id}`;
                              next[key] = candidate.field_decisions
                                    .filter((decision) =>
                                          decision.supports_source &&
                                          decision.candidate_value &&
                                          decision.action !== 'keep_current' &&
                                          decision.action !== 'no_data'
                                    )
                                    .map((decision) => decision.field);
                        }
                        return next;
                  });
                  setManualSearchQuery(item.query_terms[0] || item.work.title);
                  setManualResults([]);
                  setManualExternalId('');
                  setSearchParams((prev) => {
                        const next = new URLSearchParams(prev);
                        next.set('work', workId);
                        return next;
                  }, { replace: true });
            } catch {
                  setReviewItem(null);
            } finally {
                  setIsReviewLoading(false);
            }
      }, [setSearchParams]);

      const loadQueue = useCallback(async () => {
            setIsLoading(true);
            try {
                  const data = await invoke<UnmatchedWork[]>('get_unmatched_works');
                  const nextWorks = Array.isArray(data) ? data : [];
                  setWorks(nextWorks);
                  if (requestedWorkId) {
                        setSelectedId(requestedWorkId);
                        await loadReviewItem(requestedWorkId);
                  } else if (nextWorks.length === 0) {
                        setSelectedId(null);
                        setReviewItem(null);
                  } else if (!selectedId || !nextWorks.some((work) => work.id === selectedId)) {
                        const nextId = nextWorks[0].id;
                        setSelectedId(nextId);
                        await loadReviewItem(nextId);
                  }
            } catch {
                  setWorks([]);
                  setSelectedId(null);
                  setReviewItem(null);
            } finally {
                  setIsLoading(false);
            }
      }, [loadReviewItem, requestedWorkId, selectedId]);

      useEffect(() => {
            void loadQueue();
      }, [loadQueue]);

      const confirmMatch = useCallback(async (candidate: ReviewCandidate) => {
            if (!reviewItem) return;
            setWorkingAction(`confirm:${candidate.source}:${candidate.id}`);
            try {
                  const selectionKey = `${candidate.source}:${candidate.id}`;
                  await invoke('confirm_enrichment_match', {
                        workId: reviewItem.work.id,
                        externalId: candidate.id,
                        source: candidate.source,
                        selectedFields: candidateFieldSelection[selectionKey] ?? [],
                  });
                  await loadQueue();
            } finally {
                  setWorkingAction(null);
            }
      }, [candidateFieldSelection, loadQueue, reviewItem]);

      const rejectWork = useCallback(async () => {
            if (!reviewItem) return;
            setWorkingAction('reject');
            try {
                  await invoke('reject_enrichment', { workId: reviewItem.work.id });
                  await loadQueue();
            } finally {
                  setWorkingAction(null);
            }
      }, [loadQueue, reviewItem]);

      const resetWork = useCallback(async () => {
            if (!reviewItem) return;
            setWorkingAction('reset');
            try {
                  await invoke('reset_enrichment', { workId: reviewItem.work.id });
                  await loadQueue();
                  await loadReviewItem(reviewItem.work.id);
            } finally {
                  setWorkingAction(null);
            }
      }, [loadQueue, loadReviewItem, reviewItem]);

      const setFieldPreference = useCallback(async (field: string, source: string | null) => {
            if (!reviewItem) return;
            setWorkingAction(`pref:${field}:${source ?? 'auto'}`);
            try {
                  await invoke('set_work_field_preference', {
                        workId: reviewItem.work.id,
                        field,
                        source,
                  });
                  await loadReviewItem(reviewItem.work.id);
            } finally {
                  setWorkingAction(null);
            }
      }, [loadReviewItem, reviewItem]);

      const toggleCandidateField = useCallback((candidate: ReviewCandidate, field: string) => {
            const selectionKey = `${candidate.source}:${candidate.id}`;
            setCandidateFieldSelection((current) => {
                  const currentFields = current[selectionKey] ?? [];
                  const nextFields = currentFields.includes(field)
                        ? currentFields.filter((value) => value !== field)
                        : [...currentFields, field];
                  return { ...current, [selectionKey]: nextFields };
            });
      }, []);

      const runManualSearch = useCallback(async () => {
            if (!reviewItem || !manualSearchQuery.trim()) return;
            setWorkingAction('manual-search');
            try {
                  const results = await invoke<ReviewCandidate[]>('search_enrichment_candidates', {
                        workId: reviewItem.work.id,
                        title: manualSearchQuery.trim(),
                  });
                  setManualResults(Array.isArray(results) ? results.map((candidate) => ({
                        ...candidate,
                        source_label: sourceLabel(candidate.source),
                        verdict: 'Manual Search',
                        release_date: null,
                        description: null,
                        cover_url: null,
                        tags: [],
                        capabilities: [],
                        field_decisions: [],
                  } as ReviewCandidate)) : []);
            } finally {
                  setWorkingAction(null);
            }
      }, [manualSearchQuery, reviewItem]);

      const applyManualExternalId = useCallback(async () => {
            if (!reviewItem || !manualExternalId.trim()) return;
            setWorkingAction('manual-link');
            try {
                  await invoke('confirm_enrichment_match', {
                        workId: reviewItem.work.id,
                        externalId: manualExternalId.trim(),
                        source: manualSource,
                  });
                  await loadQueue();
            } finally {
                  setWorkingAction(null);
            }
      }, [loadQueue, manualExternalId, manualSource, reviewItem]);

      return (
            <div className="enrichment-page">
                  <header className="enrichment-header">
                        <div>
                              <h1>Enrichment Review</h1>
                              <p className="enrichment-subtitle">Resolve weak matches, compare source fields, and push poster metadata toward canonical quality.</p>
                        </div>
                        <span className="enrichment-count">{works.length} in queue</span>
                  </header>

                  {isLoading ? (
                        <div className="enrichment-loading">Loading review queue...</div>
                  ) : (
                        <div className="enrichment-layout">
                              <aside className="work-list">
                                    {works.length === 0 ? (
                                          <div className="work-list-empty">
                                                <span className="empty-icon">✅</span>
                                                <p>Review queue is empty.</p>
                                          </div>
                                    ) : works.map((work) => (
                                          <button
                                                key={work.id}
                                                className={`work-list-item ${selectedId === work.id ? 'active' : ''}`}
                                                onClick={() => void loadReviewItem(work.id)}
                                          >
                                                <span className="work-item-topline">
                                                      <span className="work-item-title">{work.title}</span>
                                                      <span className={`work-item-state ${work.enrichment_state}`}>{work.enrichment_state}</span>
                                                </span>
                                                <span className="work-item-path">{work.folder_path}</span>
                                                {work.linked_sources.length > 0 && (
                                                      <span className="work-item-links">Linked: {work.linked_sources.join(', ')}</span>
                                                )}
                                                {work.job_state && (
                                                      <span className="work-item-job">
                                                            Job: {work.job_state}
                                                            {work.attempt_count !== null ? ` · attempt ${work.attempt_count + 1}` : ''}
                                                      </span>
                                                )}
                                                {work.last_error && (
                                                      <span className="work-item-error">{work.last_error}</span>
                                                )}
                                                <span className="work-item-action">{work.suggested_action}</span>
                                          </button>
                                    ))}
                              </aside>

                              <main className="match-panel">
                                    {!reviewItem && (
                                          <>
                                                <div className="match-panel-empty">
                                                      <p>Select a work to inspect provider candidates.</p>
                                                </div>
                                                <section className="candidate-section">
                                                      <div className="section-heading">
                                                            <h3>Candidate Comparison</h3>
                                                            <p>Manual retry tools stay available even when the review queue is empty.</p>
                                                      </div>
                                                      <div className="job-status-panel">
                                                            <div className="job-status-topline">
                                                                  <strong>Review Diagnostics</strong>
                                                            </div>
                                                            <ul className="review-diagnostic-list">
                                                                  <li>No review item selected yet.</li>
                                                                  <li>Choose a work from the queue or use a direct deep link from Workshop.</li>
                                                            </ul>
                                                      </div>
                                                      <div className="manual-review-tools">
                                                            <div className="manual-tool-block">
                                                                  <label>Retry Query</label>
                                                                  <div className="manual-tool-row">
                                                                        <input
                                                                              value={manualSearchQuery}
                                                                              onChange={(event) => setManualSearchQuery(event.target.value)}
                                                                              placeholder="Select a work or open a deep link first"
                                                                              disabled
                                                                        />
                                                                        <button className="ghost-btn" disabled>Search</button>
                                                                  </div>
                                                            </div>
                                                            <div className="manual-tool-block">
                                                                  <label>Manual Provider Link</label>
                                                                  <div className="manual-tool-row">
                                                                        <select value={manualSource} onChange={(event) => setManualSource(event.target.value)} disabled>
                                                                              <option value="vndb">VNDB</option>
                                                                              <option value="bangumi">Bangumi</option>
                                                                              <option value="dlsite">DLsite</option>
                                                                        </select>
                                                                        <input
                                                                              value={manualExternalId}
                                                                              onChange={(event) => setManualExternalId(event.target.value)}
                                                                              placeholder="Paste an external ID after selecting a work"
                                                                              disabled
                                                                        />
                                                                        <button className="confirm-btn" disabled>Link ID</button>
                                                                  </div>
                                                            </div>
                                                      </div>
                                                </section>
                                          </>
                                    )}

                                    {reviewItem && (
                                          <>
                                                <section className="review-hero">
                                                      <div className="review-cover-shell">
                                                            {reviewItem.work.cover_path ? (
                                                                  <img
                                                                        className="review-cover"
                                                                        src={toAssetUrl(reviewItem.work.cover_path) ?? reviewItem.work.cover_path}
                                                                        alt={reviewItem.work.title}
                                                                  />
                                                            ) : (
                                                                  <div className="review-cover placeholder">No Cover</div>
                                                            )}
                                                      </div>
                                                      <div className="review-hero-body">
                                                            <div className="review-hero-topline">
                                                                  <h2>{reviewItem.work.title}</h2>
                                                                  <div className="hero-actions">
                                                                        <button
                                                                              className="ghost-btn"
                                                                              onClick={() => void resetWork()}
                                                                              disabled={workingAction === 'reset'}
                                                                        >
                                                                              {workingAction === 'reset' ? 'Resetting…' : 'Reset'}
                                                                        </button>
                                                                        <button
                                                                              className="ghost-btn danger"
                                                                              onClick={() => void rejectWork()}
                                                                              disabled={workingAction === 'reject'}
                                                                        >
                                                                              {workingAction === 'reject' ? 'Rejecting…' : 'Reject'}
                                                                        </button>
                                                                  </div>
                                                            </div>
                                                            <code className="match-path">{reviewItem.work.folder_path}</code>
                                                            <div className="review-stats">
                                                                  <span>Title source: {reviewItem.work.title_source}</span>
                                                                  {reviewItem.work.developer && <span>Developer: {reviewItem.work.developer}</span>}
                                                                  {reviewItem.work.release_date && <span>Release: {reviewItem.work.release_date}</span>}
                                                                  {reviewItem.work.rating !== null && <span>Rating: {reviewItem.work.rating.toFixed(1)}</span>}
                                                                  {reviewItem.work.vndb_id && <span>VNDB: {reviewItem.work.vndb_id}</span>}
                                                                  {reviewItem.work.bangumi_id && <span>Bangumi: {reviewItem.work.bangumi_id}</span>}
                                                                  {reviewItem.work.dlsite_id && <span>DLsite: {reviewItem.work.dlsite_id}</span>}
                                                            </div>
                                                            {activeQueryTerms.length > 0 && (
                                                                  <div className="query-chip-row">
                                                                        {activeQueryTerms.slice(0, 8).map((term) => (
                                                                              <button key={term} className="query-chip" onClick={() => setManualSearchQuery(term)}>{term}</button>
                                                                        ))}
                                                                  </div>
                                                            )}
                                                            {reviewItem.diagnostics.length > 0 && (
                                                                  <div className="job-status-panel">
                                                                        <div className="job-status-topline">
                                                                              <strong>Review Diagnostics</strong>
                                                                        </div>
                                                                        <ul className="review-diagnostic-list">
                                                                              {reviewItem.diagnostics.map((item) => (
                                                                                    <li key={item}>{item}</li>
                                                                              ))}
                                                                        </ul>
                                                                  </div>
                                                            )}
                                                            {reviewItem.work.description && (
                                                                  <p className="review-description">{reviewItem.work.description}</p>
                                                            )}
                                                            {reviewItem.job_status && (
                                                                  <div className="job-status-panel">
                                                                        <div className="job-status-topline">
                                                                              <strong>Latest Job</strong>
                                                                              <span className={`job-state ${reviewItem.job_status.state}`}>{reviewItem.job_status.state}</span>
                                                                        </div>
                                                                        <div className="job-status-meta">
                                                                              <span>Attempt {reviewItem.job_status.attempt_count + 1}</span>
                                                                              {reviewItem.job_status.next_run_at && <span>Next retry: {reviewItem.job_status.next_run_at}</span>}
                                                                        </div>
                                                                        {reviewItem.job_status.last_error && (
                                                                              <p className="job-status-error">{reviewItem.job_status.last_error}</p>
                                                                        )}
                                                                  </div>
                                                            )}
                                                      </div>
                                                </section>

                                                {reviewItem.provider_refresh.length > 0 && (
                                                      <section className="current-sources-section">
                                                            <div className="section-heading">
                                                                  <h3>Provider Refresh Status</h3>
                                                                  <p>Linked provider IDs are refreshed independently so one bad source does not block the whole poster.</p>
                                                            </div>
                                                            <div className="source-grid">
                                                                  {reviewItem.provider_refresh.map((item) => (
                                                                        <article key={`${item.source}:${item.external_id}`} className="source-card refresh-card">
                                                                              <div className="source-card-topline">
                                                                                    <strong>{item.source_label}</strong>
                                                                                    <span>{item.external_id}</span>
                                                                              </div>
                                                                              <div className={`refresh-state ${item.status}`}>{item.status}</div>
                                                                              {item.message && <p className="refresh-message">{item.message}</p>}
                                                                              <p className="refresh-action">{item.suggested_action}</p>
                                                                        </article>
                                                                  ))}
                                                            </div>
                                                      </section>
                                                )}

                                                <section className="current-sources-section">
                                                      <div className="section-heading">
                                                            <h3>Current Sources</h3>
                                                            <p>Linked metadata providers already attached to this poster.</p>
                                                      </div>
                                                      {reviewItem.current_sources.length === 0 ? (
                                                            <div className="source-empty">No provider is currently linked.</div>
                                                      ) : (
                                                            <div className="source-grid">
                                                                  {reviewItem.current_sources.map((source) => (
                                                                        <article key={`${source.source}:${source.external_id}`} className="source-card">
                                                                              <div className="source-card-topline">
                                                                                    <strong>{source.source_label}</strong>
                                                                                    <span>{source.external_id}</span>
                                                                              </div>
                                                                              <div className="source-title">{source.title}</div>
                                                                              <div className="source-meta">
                                                                                    {source.developer && <span>{source.developer}</span>}
                                                                                    {source.release_date && <span>{source.release_date}</span>}
                                                                                    {source.rating !== null && <span>{source.rating.toFixed(1)}</span>}
                                                                              </div>
                                                                              <div className="capability-row">
                                                                                    {source.capabilities.map((capability) => (
                                                                                          <span key={capability} className="capability-chip linked">{capability}</span>
                                                                                    ))}
                                                                              </div>
                                                                        </article>
                                                                  ))}
                                                            </div>
                                                      )}
                                                </section>

                                                <section className="candidate-section">
                                                      <div className="section-heading">
                                                            <h3>Candidate Comparison</h3>
                                                            <p>{isReviewLoading ? 'Refreshing provider details…' : `${reviewItem.candidates.length} candidates ranked across VNDB, Bangumi, and DLsite.`}</p>
                                                      </div>
                                                      <div className="manual-review-tools">
                                                            <div className="manual-tool-block">
                                                                  <label>Retry Query</label>
                                                                  <div className="manual-tool-row">
                                                                        <input
                                                                              value={manualSearchQuery}
                                                                              onChange={(event) => setManualSearchQuery(event.target.value)}
                                                                              placeholder="Try a shorter or alternate title"
                                                                              disabled={!hasActiveReview}
                                                                        />
                                                                        <button
                                                                              className="ghost-btn"
                                                                              onClick={() => void runManualSearch()}
                                                                              disabled={!hasActiveReview || workingAction === 'manual-search'}
                                                                        >
                                                                              {workingAction === 'manual-search' ? 'Searching…' : 'Search'}
                                                                        </button>
                                                                  </div>
                                                                  {manualResults.length > 0 && (
                                                                        <div className="manual-result-list">
                                                                              {manualResults.map((candidate) => (
                                                                                    <button
                                                                                          key={`manual:${candidate.source}:${candidate.id}`}
                                                                                          className="manual-result-chip"
                                                                                          onClick={() => {
                                                                                                setManualSource(candidate.source);
                                                                                                setManualExternalId(candidate.id);
                                                                                          }}
                                                                                    >
                                                                                          {candidate.source_label} · {candidate.title}
                                                                                    </button>
                                                                              ))}
                                                                        </div>
                                                                  )}
                                                            </div>
                                                            <div className="manual-tool-block">
                                                                  <label>Manual Provider Link</label>
                                                                  <div className="manual-tool-row">
                                                                        <select value={manualSource} onChange={(event) => setManualSource(event.target.value)} disabled={!hasActiveReview}>
                                                                              <option value="vndb">VNDB</option>
                                                                              <option value="bangumi">Bangumi</option>
                                                                              <option value="dlsite">DLsite</option>
                                                                        </select>
                                                                        <input
                                                                              value={manualExternalId}
                                                                              onChange={(event) => setManualExternalId(event.target.value)}
                                                                              placeholder="e.g. v2346 / 558749 / RJ123456"
                                                                              disabled={!hasActiveReview}
                                                                        />
                                                                        <button
                                                                              className="confirm-btn"
                                                                              onClick={() => void applyManualExternalId()}
                                                                              disabled={!hasActiveReview || workingAction === 'manual-link'}
                                                                        >
                                                                              {workingAction === 'manual-link' ? 'Linking…' : 'Link ID'}
                                                                        </button>
                                                                  </div>
                                                            </div>
                                                      </div>
                                                      {reviewItem.candidates.length === 0 ? (
                                                            <div className="match-no-results">No candidates found for the current query set.</div>
                                                      ) : (
                                                            <div className="candidate-list detailed">
                                                                  {reviewItem.candidates.map((candidate) => {
                                                                        const previewUrl = toAssetUrl(candidate.cover_url) ?? candidate.cover_url;
                                                                        const actionKey = `confirm:${candidate.source}:${candidate.id}`;
                                                                        const selectionKey = `${candidate.source}:${candidate.id}`;
                                                                        const selectedFields = candidateFieldSelection[selectionKey] ?? [];
                                                                        return (
                                                                              <article key={`${candidate.source}:${candidate.id}`} className="candidate-card detailed">
                                                                                    <div className="candidate-hero">
                                                                                          <div className="candidate-cover-shell">
                                                                                                {previewUrl ? (
                                                                                                      <img className="candidate-cover" src={previewUrl} alt={candidate.title} />
                                                                                                ) : (
                                                                                                      <div className="candidate-cover placeholder">No Cover</div>
                                                                                                )}
                                                                                          </div>
                                                                                          <div className="candidate-info">
                                                                                                <div className="candidate-title-row">
                                                                                                      <span className="candidate-title">{candidate.title}</span>
                                                                                                      <span className={`candidate-verdict ${candidate.verdict.toLowerCase().replace(/\s+/g, '-')}`}>{candidate.verdict}</span>
                                                                                                </div>
                                                                                                {candidate.title_original && (
                                                                                                      <span className="candidate-original">{candidate.title_original}</span>
                                                                                                )}
                                                                                                <div className="candidate-meta">
                                                                                                      <span className="candidate-source">{candidate.source_label}</span>
                                                                                                      <span>{Math.round(candidate.similarity * 100)}% match</span>
                                                                                                      {candidate.developer && <span>{candidate.developer}</span>}
                                                                                                      {candidate.release_date && <span>{candidate.release_date}</span>}
                                                                                                      {candidate.rating !== null && <span>{candidate.rating.toFixed(1)}</span>}
                                                                                                </div>
                                                                                                <div className="capability-row">
                                                                                                      {candidate.capabilities.map((capability) => (
                                                                                                            <span key={capability} className="capability-chip">{capability}</span>
                                                                                                      ))}
                                                                                                </div>
                                                                                                {candidate.tags.length > 0 && (
                                                                                                      <div className="tag-row">
                                                                                                            {candidate.tags.slice(0, 8).map((tag) => (
                                                                                                                  <span key={tag} className="tag-chip">{tag}</span>
                                                                                                            ))}
                                                                                                      </div>
                                                                                                )}
                                                                                          </div>
                                                                                          <button
                                                                                                className="confirm-btn"
                                                                                                onClick={() => void confirmMatch(candidate)}
                                                                                                disabled={workingAction === actionKey}
                                                                                          >
                                                                                                {workingAction === actionKey ? 'Applying…' : `Apply ${selectedFields.length || 'All'} Fields`}
                                                                                          </button>
                                                                                    </div>
                                                                                    <div className="field-decision-grid">
                                                                                          {candidate.field_decisions.map((decision) => (
                                                                                                <div key={`${candidate.id}:${decision.field}`} className="field-card">
                                                                                                      <div className="field-card-topline">
                                                                                                            <div className="field-card-heading">
                                                                                                                  <strong>{decision.label}</strong>
                                                                                                                  {decision.supports_source && decision.candidate_value && decision.action !== 'no_data' && (
                                                                                                                        <label className="field-select-toggle">
                                                                                                                              <input
                                                                                                                                    type="checkbox"
                                                                                                                                    checked={selectedFields.includes(decision.field)}
                                                                                                                                    onChange={() => toggleCandidateField(candidate, decision.field)}
                                                                                                                              />
                                                                                                                              <span>Apply</span>
                                                                                                                        </label>
                                                                                                                  )}
                                                                                                            </div>
                                                                                                            <span className={`field-action ${decision.action}`}>{actionLabel(decision.action)}</span>
                                                                                                      </div>
                                                                                                      <div className="field-preference-row">
                                                                                                            <span className="field-preference-label">
                                                                                                                  Preferred: {sourceLabel(decision.preferred_source || 'auto')}
                                                                                                            </span>
                                                                                                            <div className="field-preference-actions">
                                                                                                                  <button
                                                                                                                        className={`pref-btn ${!decision.preferred_source ? 'active' : ''}`}
                                                                                                                        onClick={() => void setFieldPreference(decision.field, null)}
                                                                                                                        disabled={workingAction === `pref:${decision.field}:auto`}
                                                                                                                  >
                                                                                                                        Auto
                                                                                                                  </button>
                                                                                                                  {decision.supports_source && (
                                                                                                                        <button
                                                                                                                              className={`pref-btn ${decision.preferred_source === candidate.source ? 'active' : ''}`}
                                                                                                                              onClick={() => void setFieldPreference(decision.field, candidate.source)}
                                                                                                                              disabled={workingAction === `pref:${decision.field}:${candidate.source}`}
                                                                                                                        >
                                                                                                                              Prefer {candidate.source_label}
                                                                                                                        </button>
                                                                                                                  )}
                                                                                                            </div>
                                                                                                      </div>
                                                                                                      <div className="field-compare">
                                                                                                            <div>
                                                                                                                  <label>Current</label>
                                                                                                                  <p>{decision.current_value ?? '—'}</p>
                                                                                                            </div>
                                                                                                            <div>
                                                                                                                  <label>Candidate</label>
                                                                                                                  <p>{decision.candidate_value ?? '—'}</p>
                                                                                                            </div>
                                                                                                            <div>
                                                                                                                  <label>Resolved</label>
                                                                                                                  <p>{decision.resolved_value ?? '—'}</p>
                                                                                                            </div>
                                                                                                      </div>
                                                                                                </div>
                                                                                          ))}
                                                                                    </div>
                                                                                    {candidate.description && (
                                                                                          <p className="candidate-description">{candidate.description}</p>
                                                                                    )}
                                                                              </article>
                                                                        );
                                                                  })}
                                                            </div>
                                                      )}
                                                </section>
                                          </>
                                    )}
                              </main>
                        </div>
                  )}
            </div>
      );
}
