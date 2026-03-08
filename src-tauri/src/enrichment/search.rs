//! Aggregate live metadata candidates from enrichment providers.

use std::collections::HashMap;

use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::matcher::{self, MatchBonuses, MatchInput, MatchVerdict};
use crate::enrichment::provider::{self, MetadataSource, ProviderRecord, ProviderSearchResult};
use crate::enrichment::query::EnrichmentQueryInput;
use crate::enrichment::vndb::VndbClient;

#[derive(Debug, Clone)]
pub struct SearchCandidate {
    pub id: String,
    pub title: String,
    pub title_original: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub source: MetadataSource,
    pub score: f64,
    pub verdict: MatchVerdict,
    pub record: Option<ProviderRecord>,
}

pub async fn search_live_sources(
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    input: &EnrichmentQueryInput,
    per_query_limit: u32,
) -> Vec<SearchCandidate> {
    let match_input = MatchInput {
        titles: input.search_terms.clone(),
        bonuses: MatchBonuses {
            known_brand: input.known_brand.clone(),
            expected_year: input.expected_year,
        },
    };

    let mut merged: HashMap<String, SearchCandidate> = HashMap::new();

    for query in input.search_terms.iter().take(5) {
        for source in [
            MetadataSource::Vndb,
            MetadataSource::Bangumi,
            MetadataSource::Dlsite,
        ] {
            if let Ok(results) =
                provider::search_provider(source, vndb, bangumi, dlsite, query, per_query_limit)
                    .await
            {
                merge_provider_candidates(&mut merged, &match_input, results);
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

pub fn merge_provider_candidates(
    merged: &mut HashMap<String, SearchCandidate>,
    input: &MatchInput,
    results: Vec<ProviderSearchResult>,
) {
    for result in results {
        let scored = matcher::score_candidate_with_titles(input, &result.search_titles, &result.id);
        let key = format!("{}:{}", result.source.as_str(), result.id);
        upsert_candidate(
            merged,
            key,
            SearchCandidate {
                id: result.id,
                title: result.title,
                title_original: result.title_original,
                developer: result.developer,
                rating: result.rating,
                source: result.source,
                score: scored.score,
                verdict: scored.verdict,
                record: result.record,
            },
        );
    }
}

fn upsert_candidate(
    merged: &mut HashMap<String, SearchCandidate>,
    key: String,
    candidate: SearchCandidate,
) {
    match merged.get(&key) {
        Some(existing) if existing.score >= candidate.score => {}
        _ => {
            merged.insert(key, candidate);
        }
    }
}
