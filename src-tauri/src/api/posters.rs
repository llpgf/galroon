use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use unicode_normalization::UnicodeNormalization;

use crate::domain::work::{EnrichmentState, Work, WorkSummary};

#[derive(Debug, Clone)]
pub struct PosterGroup {
    pub representative: Work,
    pub variants: Vec<Work>,
}

impl PosterGroup {
    pub fn variant_count(&self) -> u32 {
        self.variants.len() as u32
    }

    pub fn latest_created_at(&self) -> DateTime<Utc> {
        self.variants
            .iter()
            .map(|work| work.created_at)
            .max()
            .unwrap_or(self.representative.created_at)
    }

    pub fn into_summary(self) -> WorkSummary {
        let variant_count = self.variant_count();
        let work = self.representative;
        WorkSummary {
            id: work.id,
            title: work.title,
            cover_path: work.cover_path,
            developer: work.developer,
            rating: work.rating,
            library_status: work.library_status,
            enrichment_state: work.enrichment_state,
            tags: work.tags,
            release_date: work.release_date,
            vndb_id: work.vndb_id,
            bangumi_id: work.bangumi_id,
            dlsite_id: work.dlsite_id,
            variant_count,
            asset_count: 0,
            asset_types: Vec::new(),
            primary_asset_type: None,
        }
    }
}

pub fn group_works_by_poster(works: Vec<Work>) -> Vec<PosterGroup> {
    let mut grouped: HashMap<String, Vec<Work>> = HashMap::new();
    for work in works {
        grouped.entry(canonical_key(&work)).or_default().push(work);
    }

    grouped
        .into_values()
        .map(|mut variants| {
            variants.sort_by(compare_work_quality);
            variants.reverse();
            let representative = variants
                .first()
                .cloned()
                .unwrap_or_else(|| Work::from_discovery("".into(), String::new(), 0.0));
            PosterGroup {
                representative,
                variants,
            }
        })
        .collect()
}

pub fn variant_ids_for_work(works: &[Work], work_id: &str) -> Vec<String> {
    for group in group_works_by_poster(works.to_vec()) {
        if group
            .variants
            .iter()
            .any(|variant| variant.id.to_string() == work_id)
        {
            return group
                .variants
                .into_iter()
                .map(|variant| variant.id.to_string())
                .collect();
        }
    }

    vec![work_id.to_string()]
}

pub fn canonical_key(work: &Work) -> String {
    if let Some(id) = &work.vndb_id {
        return format!("vndb:{}", id.to_lowercase());
    }
    if let Some(id) = &work.bangumi_id {
        return format!("bangumi:{}", id.to_lowercase());
    }
    if let Some(id) = &work.dlsite_id {
        return format!("dlsite:{}", id.to_lowercase());
    }

    let developer = work
        .developer
        .as_deref()
        .map(normalize_key)
        .unwrap_or_default();
    format!("title:{}::dev:{}", normalize_key(&work.title), developer)
}

fn normalize_key(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || ('\u{3040}'..='\u{30ff}').contains(c)
                || ('\u{4e00}'..='\u{9fff}').contains(c)
        })
        .collect()
}

fn compare_work_quality(left: &Work, right: &Work) -> Ordering {
    work_quality_tuple(left).cmp(&work_quality_tuple(right))
}

fn work_quality_tuple(work: &Work) -> (u8, u8, u8, u8, i64, String) {
    let matched = matches!(work.enrichment_state, EnrichmentState::Matched) as u8;
    let has_cover = work.cover_path.is_some() as u8;
    let has_description = work
        .description
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty()) as u8;
    let has_developer = work.developer.is_some() as u8;
    let votes = work.vote_count.unwrap_or_default() as i64;
    (
        matched,
        has_cover,
        has_description,
        has_developer,
        votes,
        work.updated_at.to_rfc3339(),
    )
}
