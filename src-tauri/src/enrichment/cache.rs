//! Cached enrichment lookup built on top of provider search.

use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;

use crate::db::queries;
use crate::domain::error::AppResult;
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::matcher::{self, MatchInput, MatchVerdict};
use crate::enrichment::provider::{self, MetadataSource, ProviderRecord};
use crate::enrichment::query::{self, EnrichmentQueryInput};
use crate::enrichment::search::{self, SearchCandidate};
use crate::enrichment::vndb::VndbClient;

pub async fn search_candidates(
    pool: &SqlitePool,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    input: &EnrichmentQueryInput,
    per_query_limit: u32,
) -> Vec<SearchCandidate> {
    let match_input = MatchInput {
        titles: input.search_terms.clone(),
        bonuses: matcher::MatchBonuses {
            known_brand: input.known_brand.clone(),
            expected_year: input.expected_year,
        },
    };

    let mut merged: HashMap<String, SearchCandidate> = HashMap::new();

    for query_term in input.search_terms.iter().take(5) {
        let normalized = query::canonicalize_query(query_term);
        let mut satisfied_sources = HashSet::new();

        if !normalized.is_empty() {
            if let Ok(rows) =
                queries::enrichment_mappings::find_mappings_for_title(pool, &normalized).await
            {
                for row in rows {
                    let Some(source) = MetadataSource::from_str(&row.source) else {
                        continue;
                    };
                    let scored = matcher::score_candidate(
                        &match_input,
                        &row.resolved_title,
                        &row.external_id,
                    );
                    let score = scored.score.max(row.confidence);
                    let verdict = verdict_for_score(score);
                    upsert_candidate(
                        &mut merged,
                        SearchCandidate {
                            id: row.external_id,
                            title: row.resolved_title,
                            title_original: row.title_original,
                            developer: row.developer,
                            rating: row.rating,
                            source,
                            score,
                            verdict,
                            record: None,
                        },
                    );
                    if row.confidence >= 85.0 {
                        satisfied_sources.insert(source);
                    }
                }
            }
        }

        for source in [
            MetadataSource::Vndb,
            MetadataSource::Bangumi,
            MetadataSource::Dlsite,
        ] {
            if satisfied_sources.contains(&source) {
                continue;
            }

            if let Ok(results) = provider::search_provider(
                source,
                vndb,
                bangumi,
                dlsite,
                query_term,
                per_query_limit,
            )
            .await
            {
                search::merge_provider_candidates(&mut merged, &match_input, results);
            }
        }
    }

    let mut candidates: Vec<SearchCandidate> = merged.into_values().collect();
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.source.as_str().cmp(b.source.as_str()))
    });
    candidates
}

pub async fn remember_candidate(
    pool: &SqlitePool,
    input: &EnrichmentQueryInput,
    candidate: &SearchCandidate,
) -> AppResult<()> {
    if candidate.verdict == MatchVerdict::Rejected {
        return Ok(());
    }

    for term in input.search_terms.iter().take(5) {
        let normalized = query::canonicalize_query(term);
        if normalized.is_empty() {
            continue;
        }

        queries::enrichment_mappings::upsert_mapping(
            pool,
            &normalized,
            candidate.source.as_str(),
            &candidate.id,
            &candidate.title,
            candidate.title_original.as_deref(),
            candidate.developer.as_deref(),
            candidate.rating,
            candidate.score,
        )
        .await?;
    }

    Ok(())
}

pub async fn remember_record(
    pool: &SqlitePool,
    input: &EnrichmentQueryInput,
    record: &ProviderRecord,
    confidence: f64,
) -> AppResult<()> {
    let candidate = SearchCandidate {
        id: record.id(),
        title: record.title(),
        title_original: record.title_original(),
        developer: record.developer(),
        rating: record.rating(),
        source: record.source(),
        score: confidence,
        verdict: verdict_for_score(confidence),
        record: Some(record.clone()),
    };

    remember_candidate(pool, input, &candidate).await
}

fn upsert_candidate(merged: &mut HashMap<String, SearchCandidate>, candidate: SearchCandidate) {
    let key = format!("{}:{}", candidate.source.as_str(), candidate.id);
    match merged.get(&key) {
        Some(existing) if existing.score >= candidate.score => {}
        _ => {
            merged.insert(key, candidate);
        }
    }
}

fn verdict_for_score(score: f64) -> MatchVerdict {
    if score >= 85.0 {
        MatchVerdict::AutoMatch
    } else if score >= 75.0 {
        MatchVerdict::PendingReview
    } else {
        MatchVerdict::Rejected
    }
}
