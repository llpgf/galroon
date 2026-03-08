//! Enrichment job queue worker — DB-persisted, crash-safe (R7).
//!
//! Processing loop:
//! 1. Claim next job (atomic via DB)
//! 2. Search cached/live metadata candidates
//! 3. Match + resolve fields
//! 4. Update Work + metadata.json
//! 5. Complete/fail job

use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, warn};

use crate::db::queries;
use crate::db::Database;
use crate::domain::work::EnrichmentState;
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::cache;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::people;
use crate::enrichment::provider::{self, LinkedProviderRecords, MetadataSource, ProviderLinkState};
use crate::enrichment::query;
use crate::enrichment::rate_limit::RateLimiter;
use crate::enrichment::resolver;
use crate::enrichment::search::SearchCandidate;
use crate::enrichment::vndb::VndbClient;
use crate::fs::metadata_io;

pub struct EnrichmentWorker {
    db: Arc<Database>,
    vndb: VndbClient,
    bangumi: BangumiClient,
    dlsite: DlsiteClient,
    worker_id: String,
}

impl EnrichmentWorker {
    pub fn new(db: Arc<Database>, rate_limiter: RateLimiter) -> Self {
        Self {
            db,
            vndb: VndbClient::new(rate_limiter.clone()),
            bangumi: BangumiClient::new(rate_limiter.clone(), None, None),
            dlsite: DlsiteClient::new(rate_limiter),
            worker_id: format!("worker-{}", uuid::Uuid::now_v7()),
        }
    }

    pub fn from_clients(
        db: Arc<Database>,
        vndb: VndbClient,
        bangumi: BangumiClient,
        dlsite: DlsiteClient,
    ) -> Self {
        Self {
            db,
            vndb,
            bangumi,
            dlsite,
            worker_id: format!("worker-{}", uuid::Uuid::now_v7()),
        }
    }

    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(worker = %self.worker_id, "Enrichment worker started");

        loop {
            if *shutdown.borrow() {
                info!(worker = %self.worker_id, "Enrichment worker shutting down");
                break;
            }

            let paused = queries::app_jobs::get_runtime_flag(self.db.read_pool(), "enrichment_paused")
                .await
                .ok()
                .flatten()
                .is_some_and(|value| value == "true");
            if paused {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => continue,
                    _ = shutdown.changed() => break,
                }
            }

            let job =
                match queries::jobs::claim_next_job(self.db.read_pool(), &self.worker_id).await {
                    Ok(Some(job)) => job,
                    Ok(None) => {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(5)) => continue,
                            _ = shutdown.changed() => break,
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to claim job");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        continue;
                    }
                };

            info!(
                job_id = job.id,
                work_id = %job.work_id,
                job_type = %job.job_type,
                attempt = job.attempt_count,
                "Processing enrichment job"
            );

            let result = self.process_job(&job).await;

            match result {
                Ok(()) => {
                    if let Err(e) = queries::jobs::complete_job(self.db.read_pool(), job.id).await {
                        error!(job_id = job.id, error = %e, "Failed to mark job complete");
                    }
                }
                Err(err_msg) => {
                    warn!(
                        job_id = job.id,
                        error = %err_msg,
                        attempt = job.attempt_count,
                        "Enrichment job failed"
                    );
                    if let Err(e) = queries::jobs::fail_job(
                        self.db.read_pool(),
                        job.id,
                        job.attempt_count,
                        job.max_attempts,
                        &err_msg,
                    )
                    .await
                    {
                        error!(job_id = job.id, error = %e, "Failed to mark job as failed");
                    }
                }
            }
        }
    }

    async fn process_job(&self, job: &crate::db::models::JobRow) -> Result<(), String> {
        let work_row = queries::works::get_work_by_id(self.db.read_pool(), &job.work_id)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("Work {} not found", job.work_id))?;

        let mut work = work_row.into_work();
        let mut query_input = query::build_query_input(&work);
        let linked =
            provider::fetch_linked_records_detailed(&work, &self.vndb, &self.bangumi, &self.dlsite)
                .await;
        clear_stale_links(&mut work, &linked);
        let linked_vndb = linked.vndb.record.clone();
        let linked_bangumi = linked.bangumi.record.clone();
        let linked_dlsite = linked.dlsite.record.clone();
        let refresh_warnings = collect_refresh_warnings(&linked);

        for record in [
            linked_vndb.as_ref(),
            linked_bangumi.as_ref(),
            linked_dlsite.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            query::extend_query_input(&mut query_input, record.search_titles());
        }

        let candidates =
            if linked_vndb.is_some() && linked_bangumi.is_some() && linked_dlsite.is_some() {
                Vec::new()
            } else {
                cache::search_candidates(
                    self.db.read_pool(),
                    &self.vndb,
                    &self.bangumi,
                    &self.dlsite,
                    &query_input,
                    5,
                )
                .await
            };

        let best_vndb = best_candidate_for_source(&candidates, MetadataSource::Vndb);
        let best_bangumi = best_candidate_for_source(&candidates, MetadataSource::Bangumi);
        let best_dlsite = best_candidate_for_source(&candidates, MetadataSource::Dlsite);

        let has_pending = best_vndb.as_ref().is_some_and(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::PendingReview
        }) || best_bangumi.as_ref().is_some_and(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::PendingReview
        }) || best_dlsite.as_ref().is_some_and(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::PendingReview
        });

        let vndb_record = if linked_vndb.is_some() {
            linked_vndb
        } else if let Some(candidate) = best_vndb.as_ref().filter(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::AutoMatch
        }) {
            provider::fetch_record(
                MetadataSource::Vndb,
                &candidate.id,
                &self.vndb,
                &self.bangumi,
                &self.dlsite,
            )
            .await
            .map_err(|err| {
                if candidate.record.is_some() {
                    warn!(error = %err, source = "vndb", id = %candidate.id, "Falling back to cached provider record");
                    err
                } else {
                    err
                }
            })
            .ok()
            .flatten()
            .or_else(|| candidate.record.clone())
        } else {
            None
        };

        let bangumi_record = if linked_bangumi.is_some() {
            linked_bangumi
        } else if let Some(candidate) = best_bangumi.as_ref().filter(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::AutoMatch
        }) {
            provider::fetch_record(
                MetadataSource::Bangumi,
                &candidate.id,
                &self.vndb,
                &self.bangumi,
                &self.dlsite,
            )
            .await
            .map_err(|err| {
                if candidate.record.is_some() {
                    warn!(error = %err, source = "bangumi", id = %candidate.id, "Falling back to cached provider record");
                    err
                } else {
                    err
                }
            })
            .ok()
            .flatten()
            .or_else(|| candidate.record.clone())
        } else {
            None
        };

        let dlsite_record = if linked_dlsite.is_some() {
            linked_dlsite
        } else if let Some(candidate) = best_dlsite.as_ref().filter(|candidate| {
            candidate.verdict == crate::enrichment::matcher::MatchVerdict::AutoMatch
        }) {
            provider::fetch_record(
                MetadataSource::Dlsite,
                &candidate.id,
                &self.vndb,
                &self.bangumi,
                &self.dlsite,
            )
            .await
            .map_err(|err| {
                if candidate.record.is_some() {
                    warn!(error = %err, source = "dlsite", id = %candidate.id, "Falling back to cached provider record");
                    err
                } else {
                    err
                }
            })
            .ok()
            .flatten()
            .or_else(|| candidate.record.clone())
        } else {
            None
        };

        for record in [
            vndb_record.as_ref(),
            bangumi_record.as_ref(),
            dlsite_record.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            cache::remember_record(self.db.read_pool(), &query_input, record, 100.0)
                .await
                .map_err(|e| format!("Failed to store mapping: {}", e))?;
        }

        if vndb_record.is_some() || bangumi_record.is_some() || dlsite_record.is_some() {
            let provider_defaults =
                queries::provider_rules::list_field_defaults(self.db.read_pool())
                    .await
                    .map_err(|e| format!("Provider default load error: {}", e))?;
            resolver::resolve_with_defaults(
                &mut work,
                vndb_record.as_ref().and_then(|record| record.as_vndb()),
                bangumi_record
                    .as_ref()
                    .and_then(|record| record.as_bangumi()),
                dlsite_record.as_ref().and_then(|record| record.as_dlsite()),
                &provider_defaults,
            );
            work.enrichment_state = EnrichmentState::Matched;
        } else if has_pending {
            work.enrichment_state = EnrichmentState::PendingReview;
        } else {
            if refresh_warnings
                .iter()
                .any(|(_, state, _)| state.should_retry())
            {
                return Err(format_refresh_error(&refresh_warnings));
            }
            work.enrichment_state = EnrichmentState::Unmatched;
        }

        queries::works::upsert_work(self.db.read_pool(), &work)
            .await
            .map_err(|e| format!("DB update error: {}", e))?;
        metadata_io::sync_metadata_from_work(&work, None)
            .map_err(|e| format!("Metadata sync error: {}", e))?;
        queries::canonical::sync_work_ids(self.db.read_pool(), &[work.id.to_string()])
            .await
            .map_err(|e| format!("Canonical sync error: {}", e))?;
        self.sync_related_people(
            &work.id.to_string(),
            bangumi_record
                .as_ref()
                .and_then(|record| record.as_bangumi()),
        )
        .await?;

        info!(
            work_id = %work.id,
            title = %work.title,
            primary_title = %query_input.primary_title,
            search_terms = query_input.search_terms.len(),
            vndb = vndb_record.is_some(),
            bangumi = bangumi_record.is_some(),
            dlsite = dlsite_record.is_some(),
            "Enrichment complete"
        );

        Ok(())
    }

    async fn sync_related_people(
        &self,
        work_id: &str,
        bangumi_record: Option<&crate::enrichment::bangumi::BangumiSubject>,
    ) -> Result<(), String> {
        if let Some(subject) = bangumi_record {
            let persons = self
                .bangumi
                .get_subject_persons(subject.id)
                .await
                .map_err(|e| format!("Failed to fetch Bangumi persons: {}", e))?;
            let characters = self
                .bangumi
                .get_subject_characters(subject.id)
                .await
                .map_err(|e| format!("Failed to fetch Bangumi characters: {}", e))?;
            let bundle = people::extract_bangumi_people(&persons, &characters);
            queries::people::replace_for_work(
                self.db.read_pool(),
                work_id,
                &bundle.persons,
                &bundle.characters,
                &bundle.character_links,
                &bundle.credits,
            )
            .await
            .map_err(|e| format!("Failed to persist related people: {}", e))?;
        } else {
            queries::people::clear_for_work(self.db.read_pool(), work_id)
                .await
                .map_err(|e| format!("Failed to clear related people: {}", e))?;
        }

        Ok(())
    }
}

fn best_candidate_for_source(
    candidates: &[SearchCandidate],
    source: MetadataSource,
) -> Option<SearchCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.source == source)
        .max_by(|left, right| {
            left.score
                .partial_cmp(&right.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn clear_stale_links(work: &mut crate::domain::work::Work, linked: &LinkedProviderRecords) {
    for outcome in [&linked.vndb, &linked.bangumi, &linked.dlsite] {
        if outcome.state != ProviderLinkState::Missing {
            continue;
        }
        match outcome.source {
            MetadataSource::Vndb => work.vndb_id = None,
            MetadataSource::Bangumi => work.bangumi_id = None,
            MetadataSource::Dlsite => work.dlsite_id = None,
        }
    }
}

fn collect_refresh_warnings(
    linked: &LinkedProviderRecords,
) -> Vec<(MetadataSource, ProviderLinkState, String)> {
    [&linked.vndb, &linked.bangumi, &linked.dlsite]
        .into_iter()
        .filter_map(|outcome| match outcome.state {
            ProviderLinkState::Ready | ProviderLinkState::NotLinked => None,
            state => Some((
                outcome.source,
                state,
                outcome
                    .message
                    .clone()
                    .unwrap_or_else(|| "Provider refresh failed".to_string()),
            )),
        })
        .collect()
}

fn format_refresh_error(warnings: &[(MetadataSource, ProviderLinkState, String)]) -> String {
    warnings
        .iter()
        .filter(|(_, state, _)| state.should_retry())
        .map(|(source, state, message)| {
            format!(
                "{} [{}]: {}",
                source.display_name(),
                state.as_str(),
                message
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}
