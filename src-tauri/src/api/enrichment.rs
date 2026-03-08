//! Enrichment API — Tauri IPC commands for enrichment match review.

use serde::Serialize;
use sqlx::Row;
use tauri::State;
use tracing::warn;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::{FieldSource, Work};
use crate::enrichment::bangumi::BangumiClient;
use crate::enrichment::cache;
use crate::enrichment::dlsite::DlsiteClient;
use crate::enrichment::matcher::MatchVerdict;
use crate::enrichment::provider::{
    self, LinkedProviderRecord, MetadataField, MetadataSource, ProviderLinkState, ProviderRecord,
};
use crate::enrichment::query;
use crate::enrichment::resolver;
use crate::enrichment::vndb::VndbClient;
use crate::fs::metadata_io;

#[derive(Serialize)]
pub struct UnmatchedWorkInfo {
    pub id: String,
    pub title: String,
    pub folder_path: String,
    pub enrichment_state: String,
    pub linked_sources: Vec<String>,
    pub job_state: Option<String>,
    pub attempt_count: Option<i32>,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
    pub suggested_action: String,
}

#[derive(Serialize)]
pub struct MatchCandidate {
    pub id: String,
    pub title: String,
    pub title_original: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub source: String,
    pub similarity: f64,
}

#[derive(Serialize)]
pub struct ReviewWorkInfo {
    pub id: String,
    pub title: String,
    pub folder_path: String,
    pub developer: Option<String>,
    pub release_date: Option<String>,
    pub rating: Option<f64>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub tags: Vec<String>,
    pub enrichment_state: String,
    pub title_source: String,
    pub field_preferences: std::collections::HashMap<String, String>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
}

#[derive(Serialize)]
pub struct ReviewLinkedSource {
    pub source: String,
    pub source_label: String,
    pub external_id: String,
    pub title: String,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub release_date: Option<String>,
    pub cover_url: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Serialize)]
pub struct ReviewFieldDecision {
    pub field: String,
    pub label: String,
    pub current_value: Option<String>,
    pub candidate_value: Option<String>,
    pub action: String,
    pub resolved_value: Option<String>,
    pub preferred_source: Option<String>,
    pub supports_source: bool,
}

#[derive(Serialize)]
pub struct ReviewCandidate {
    pub id: String,
    pub title: String,
    pub title_original: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub source: String,
    pub source_label: String,
    pub similarity: f64,
    pub verdict: String,
    pub release_date: Option<String>,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub field_decisions: Vec<ReviewFieldDecision>,
}

#[derive(Serialize)]
pub struct EnrichmentReviewItem {
    pub work: ReviewWorkInfo,
    pub query_terms: Vec<String>,
    pub diagnostics: Vec<String>,
    pub job_status: Option<ReviewJobStatus>,
    pub provider_refresh: Vec<ReviewProviderRefresh>,
    pub current_sources: Vec<ReviewLinkedSource>,
    pub candidates: Vec<ReviewCandidate>,
}

#[derive(Serialize)]
pub struct ReviewJobStatus {
    pub state: String,
    pub attempt_count: i32,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Serialize)]
pub struct ReviewProviderRefresh {
    pub source: String,
    pub source_label: String,
    pub external_id: String,
    pub status: String,
    pub message: Option<String>,
    pub suggested_action: String,
}

#[tauri::command]
pub async fn get_unmatched_works(
    db: State<'_, Database>,
) -> Result<Vec<UnmatchedWorkInfo>, AppError> {
    let rows = sqlx::query(
        "SELECT cw.preferred_work_id as id, cw.title, w.folder_path, cw.enrichment_state,
                cw.vndb_id, cw.bangumi_id, cw.dlsite_id,
                job.state as job_state, job.attempt_count, job.next_run_at, job.last_error
         FROM canonical_works cw
         JOIN works w ON w.id = cw.preferred_work_id
         LEFT JOIN enrichment_jobs job ON job.id = (
             SELECT ej.id FROM enrichment_jobs ej
             WHERE ej.work_id = cw.preferred_work_id
             ORDER BY ej.id DESC
             LIMIT 1
         )
         WHERE cw.enrichment_state IN ('unmatched', 'pending_review', 'rejected')
         ORDER BY CASE cw.enrichment_state
             WHEN 'pending_review' THEN 0
             WHEN 'unmatched' THEN 1
             ELSE 2
         END, cw.updated_at DESC, cw.title",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| UnmatchedWorkInfo {
            id: r.get("id"),
            title: r.get("title"),
            folder_path: r.get("folder_path"),
            enrichment_state: r.get("enrichment_state"),
            linked_sources: [
                ("vndb", r.get::<Option<String>, _>("vndb_id")),
                ("bangumi", r.get::<Option<String>, _>("bangumi_id")),
                ("dlsite", r.get::<Option<String>, _>("dlsite_id")),
            ]
            .into_iter()
            .filter_map(|(source, value)| value.map(|_| source.to_string()))
            .collect(),
            job_state: r.get("job_state"),
            attempt_count: r.get("attempt_count"),
            next_run_at: r.get("next_run_at"),
            last_error: r.get("last_error"),
            suggested_action: queue_suggested_action(
                r.get::<String, _>("enrichment_state").as_str(),
                r.get::<Option<String>, _>("job_state").as_deref(),
                r.get::<Option<String>, _>("last_error").as_deref(),
            )
            .to_string(),
        })
        .collect())
}

#[tauri::command]
pub async fn get_enrichment_review_item(
    work_id: String,
    db: State<'_, Database>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<EnrichmentReviewItem, AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let work_row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(work_id.clone()))?;
    let work = work_row.into_work();

    let mut query_input = query::build_query_input(&work);
    let linked = provider::fetch_linked_records_detailed(&work, &vndb, &bangumi, &dlsite).await;
    let linked_vndb = linked.vndb.record.clone();
    let linked_bangumi = linked.bangumi.record.clone();
    let linked_dlsite = linked.dlsite.record.clone();

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

    let current_sources = [
        linked_vndb.as_ref(),
        linked_bangumi.as_ref(),
        linked_dlsite.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(build_linked_source)
    .collect::<Vec<_>>();
    let provider_refresh = [&linked.vndb, &linked.bangumi, &linked.dlsite]
        .into_iter()
        .filter(|item| item.state != ProviderLinkState::NotLinked)
        .map(build_provider_refresh)
        .collect::<Vec<_>>();
    let job_status = latest_job_status(db.read_pool(), &work.id.to_string()).await?;

    let search_candidates =
        cache::search_candidates(db.read_pool(), &vndb, &bangumi, &dlsite, &query_input, 10).await;

    let mut candidates = Vec::new();
    for candidate in search_candidates.into_iter().take(10) {
        let record = match candidate.record.clone() {
            Some(record) => Some(record),
            None => {
                provider::fetch_record(candidate.source, &candidate.id, &vndb, &bangumi, &dlsite)
                    .await
                    .map_err(AppError::Internal)?
            }
        };

        candidates.push(build_review_candidate(&work, &candidate, record.as_ref()));
    }

    Ok(EnrichmentReviewItem {
        work: build_review_work(&work),
        query_terms: query_input.search_terms,
        diagnostics: build_review_diagnostics(&work, &provider_refresh, &current_sources),
        job_status,
        provider_refresh,
        current_sources,
        candidates,
    })
}

fn build_review_diagnostics(
    work: &Work,
    provider_refresh: &[ReviewProviderRefresh],
    current_sources: &[ReviewLinkedSource],
) -> Vec<String> {
    let mut diagnostics = Vec::new();

    match work.enrichment_state {
        crate::domain::work::EnrichmentState::PendingReview => {
            diagnostics.push(
                "This poster already has candidate matches, but still needs a review decision."
                    .to_string(),
            );
        }
        crate::domain::work::EnrichmentState::Unmatched => {
            diagnostics.push("No provider match is confirmed yet. Try broader queries or manually attach a source ID.".to_string());
        }
        crate::domain::work::EnrichmentState::Rejected => {
            diagnostics.push("This poster was manually rejected earlier. Re-run matching only if the source title is now cleaner.".to_string());
        }
        crate::domain::work::EnrichmentState::Matched => {}
    }

    if current_sources.is_empty() {
        diagnostics.push("No provider is currently linked to this poster.".to_string());
    }

    if work
        .cover_path
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        diagnostics.push("No cover is currently resolved for this poster.".to_string());
    }

    for refresh in provider_refresh {
        if refresh.status != "ready" {
            diagnostics.push(format!(
                "{} refresh status: {}{}",
                refresh.source_label,
                refresh.status,
                refresh
                    .message
                    .as_ref()
                    .map(|message| format!(" ({message})"))
                    .unwrap_or_default()
            ));
        }
    }

    diagnostics
}

#[tauri::command]
pub async fn search_enrichment_candidates(
    work_id: String,
    title: String,
    db: State<'_, Database>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<Vec<MatchCandidate>, AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let mut query_input = match queries::works::get_work_by_id(db.read_pool(), &work_id).await? {
        Some(row) => query::build_query_input(&row.into_work()),
        None => query::build_query_input_from_title(&title),
    };

    if let Some(row) = queries::works::get_work_by_id(db.read_pool(), &work_id).await? {
        let work = row.into_work();
        let linked = provider::fetch_linked_records_detailed(&work, &vndb, &bangumi, &dlsite).await;
        let linked_vndb = linked.vndb.record;
        let linked_bangumi = linked.bangumi.record;
        let linked_dlsite = linked.dlsite.record;
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
    }

    let candidates =
        cache::search_candidates(db.read_pool(), &vndb, &bangumi, &dlsite, &query_input, 10).await;

    Ok(candidates
        .into_iter()
        .take(10)
        .map(|candidate| MatchCandidate {
            id: candidate.id,
            title: candidate.title,
            title_original: candidate.title_original,
            developer: candidate.developer,
            rating: candidate.rating,
            source: candidate.source.as_str().to_string(),
            similarity: candidate.score / 100.0,
        })
        .collect())
}

#[tauri::command]
pub async fn confirm_enrichment_match(
    db: State<'_, Database>,
    work_id: String,
    external_id: String,
    source: String,
    selected_fields: Option<Vec<String>>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<(), AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let source_kind = MetadataSource::from_str(&source)
        .ok_or_else(|| AppError::Internal(format!("Unknown source: {}", source)))?;
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(work_id.clone()))?;
    let mut work = row.into_work();
    let previous = work.clone();
    let selected_fields = normalize_selected_fields(selected_fields);
    match source_kind {
        MetadataSource::Vndb => work.vndb_id = Some(external_id.clone()),
        MetadataSource::Bangumi => work.bangumi_id = Some(external_id.clone()),
        MetadataSource::Dlsite => work.dlsite_id = Some(external_id.clone()),
    }

    let query_input = query::build_query_input(&work);
    match provider::fetch_record(source_kind, &external_id, &vndb, &bangumi, &dlsite).await {
        Ok(Some(record)) => {
            let linked = provider::fetch_linked_records(&work, &vndb, &bangumi, &dlsite)
                .await
                .map_err(AppError::Internal)?;
            let vndb_record = if source_kind == MetadataSource::Vndb {
                Some(record.clone())
            } else {
                linked.0
            };
            let bangumi_record = if source_kind == MetadataSource::Bangumi {
                Some(record.clone())
            } else {
                linked.1
            };
            let dlsite_record = if source_kind == MetadataSource::Dlsite {
                Some(record.clone())
            } else {
                linked.2
            };

            let provider_defaults =
                queries::provider_rules::list_field_defaults(db.read_pool()).await?;
            resolver::resolve_with_defaults(
                &mut work,
                vndb_record.as_ref().and_then(|value| value.as_vndb()),
                bangumi_record.as_ref().and_then(|value| value.as_bangumi()),
                dlsite_record.as_ref().and_then(|value| value.as_dlsite()),
                &provider_defaults,
            );
            apply_selected_field_scope(&mut work, &previous, source_kind, &selected_fields);
            work.enrichment_state = crate::domain::work::EnrichmentState::Matched;
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;

            if let Err(err) =
                cache::remember_record(db.read_pool(), &query_input, &record, 100.0).await
            {
                warn!(error = %err, work_id = %work_id, source = %source, "Failed to persist confirmed enrichment mapping");
            }
        }
        Ok(None) => {
            work.enrichment_state = crate::domain::work::EnrichmentState::Matched;
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        }
        Err(err) => {
            warn!(error = %err, work_id = %work_id, source = %source, "Failed to fetch confirmed enrichment record");
            work.enrichment_state = crate::domain::work::EnrichmentState::Matched;
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
        }
    }

    queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;

    Ok(())
}

fn normalize_selected_fields(selected_fields: Option<Vec<String>>) -> Option<Vec<String>> {
    let normalized = selected_fields.map(|fields| {
        let mut values = fields
            .into_iter()
            .filter_map(|field| normalize_field_key(&field).map(str::to_string))
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values
    });
    match normalized {
        Some(values) if values.is_empty() => None,
        other => other,
    }
}

fn apply_selected_field_scope(
    work: &mut Work,
    previous: &Work,
    source: MetadataSource,
    selected_fields: &Option<Vec<String>>,
) {
    let Some(selected_fields) = selected_fields else {
        return;
    };

    for field in selected_fields {
        work.field_preferences
            .insert(field.clone(), source.as_str().to_string());
    }

    for field in [
        "title",
        "title_aliases",
        "developer",
        "release_date",
        "rating",
        "description",
        "tags",
        "cover_path",
    ] {
        if !selected_fields.iter().any(|selected| selected == field) {
            restore_field(work, previous, field);
        }
    }
}

fn restore_field(work: &mut Work, previous: &Work, field: &str) {
    match field {
        "title" => {
            work.title = previous.title.clone();
            work.title_original = previous.title_original.clone();
            work.title_source = previous.title_source.clone();
            restore_source_entry(work, previous, "title");
        }
        "title_aliases" => {
            work.title_aliases = previous.title_aliases.clone();
            restore_source_entry(work, previous, "title_aliases");
        }
        "developer" => {
            work.developer = previous.developer.clone();
            restore_source_entry(work, previous, "developer");
        }
        "release_date" => {
            work.release_date = previous.release_date;
            restore_source_entry(work, previous, "release_date");
        }
        "rating" => {
            work.rating = previous.rating;
            work.vote_count = previous.vote_count;
            restore_source_entry(work, previous, "rating");
        }
        "description" => {
            work.description = previous.description.clone();
            restore_source_entry(work, previous, "description");
        }
        "tags" => {
            work.tags = previous.tags.clone();
            restore_source_entry(work, previous, "tags");
        }
        "cover_path" => {
            work.cover_path = previous.cover_path.clone();
            restore_source_entry(work, previous, "cover_path");
        }
        _ => {}
    }
}

fn restore_source_entry(work: &mut Work, previous: &Work, field: &str) {
    if let Some(value) = previous.field_sources.get(field) {
        work.field_sources.insert(field.to_string(), value.clone());
    } else {
        work.field_sources.remove(field);
    }
}

#[tauri::command]
pub async fn reject_enrichment(db: State<'_, Database>, work_id: String) -> Result<(), AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(work_id.clone()))?;
    let mut work = row.into_work();
    work.enrichment_state = crate::domain::work::EnrichmentState::Rejected;
    queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;

    queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;

    Ok(())
}

fn build_review_work(work: &Work) -> ReviewWorkInfo {
    ReviewWorkInfo {
        id: work.id.to_string(),
        title: work.title.clone(),
        folder_path: work.folder_path.to_string_lossy().to_string(),
        developer: work.developer.clone(),
        release_date: work.release_date.map(|value| value.to_string()),
        rating: work.rating,
        description: work.description.clone(),
        cover_path: work.cover_path.clone(),
        tags: work.tags.clone(),
        enrichment_state: serde_json::to_string(&work.enrichment_state)
            .unwrap_or_else(|_| "unmatched".to_string())
            .trim_matches('"')
            .to_string(),
        title_source: field_source_label(work.title_source.clone()).to_string(),
        field_preferences: work.field_preferences.clone(),
        vndb_id: work.vndb_id.clone(),
        bangumi_id: work.bangumi_id.clone(),
        dlsite_id: work.dlsite_id.clone(),
    }
}

fn build_linked_source(record: &ProviderRecord) -> ReviewLinkedSource {
    ReviewLinkedSource {
        source: record.source().as_str().to_string(),
        source_label: record.source().display_name().to_string(),
        external_id: record.id(),
        title: record.title(),
        developer: record.developer(),
        rating: record.rating(),
        release_date: record.release_date(),
        cover_url: record.cover_url(),
        capabilities: record
            .source()
            .supported_fields()
            .iter()
            .map(|field| field.display_name().to_string())
            .collect(),
    }
}

fn build_provider_refresh(linked: &LinkedProviderRecord) -> ReviewProviderRefresh {
    ReviewProviderRefresh {
        source: linked.source.as_str().to_string(),
        source_label: linked.source.display_name().to_string(),
        external_id: linked.external_id.clone().unwrap_or_default(),
        status: linked.state.as_str().to_string(),
        message: linked.message.clone(),
        suggested_action: provider_action(linked.state).to_string(),
    }
}

fn build_review_candidate(
    work: &Work,
    candidate: &crate::enrichment::search::SearchCandidate,
    record: Option<&ProviderRecord>,
) -> ReviewCandidate {
    let source = candidate.source;
    let field_decisions = record
        .map(|record| build_field_decisions(work, source, record))
        .unwrap_or_default();

    ReviewCandidate {
        id: candidate.id.clone(),
        title: candidate.title.clone(),
        title_original: candidate.title_original.clone(),
        developer: candidate.developer.clone(),
        rating: candidate.rating,
        source: source.as_str().to_string(),
        source_label: source.display_name().to_string(),
        similarity: candidate.score / 100.0,
        verdict: verdict_label(candidate.verdict.clone()).to_string(),
        release_date: record.and_then(|value| value.release_date()),
        description: record.and_then(|value| value.description()),
        cover_url: record.and_then(|value| value.cover_url()),
        tags: record.map(|value| value.tags()).unwrap_or_default(),
        capabilities: source
            .supported_fields()
            .iter()
            .map(|field| field.display_name().to_string())
            .collect(),
        field_decisions,
    }
}

fn build_field_decisions(
    work: &Work,
    source: MetadataSource,
    record: &ProviderRecord,
) -> Vec<ReviewFieldDecision> {
    [
        MetadataField::Title,
        MetadataField::TitleAliases,
        MetadataField::Developer,
        MetadataField::ReleaseDate,
        MetadataField::Rating,
        MetadataField::Description,
        MetadataField::Tags,
        MetadataField::CoverImage,
    ]
    .into_iter()
    .map(|field| {
        let current_value = current_field_value(work, field);
        let candidate_value = record.field_value(field);
        let action = decide_field_action(
            work,
            field,
            source,
            current_value.as_deref(),
            candidate_value.as_deref(),
        );
        let resolved_value = match action {
            "fill" | "override" => candidate_value.clone().or(current_value.clone()),
            _ => current_value.clone().or(candidate_value.clone()),
        };
        ReviewFieldDecision {
            field: field.as_str().to_string(),
            label: field.display_name().to_string(),
            current_value,
            candidate_value,
            action: action.to_string(),
            resolved_value,
            preferred_source: work.field_preferences.get(field_key(field)).cloned(),
            supports_source: source.supported_fields().contains(&field),
        }
    })
    .collect()
}

fn field_key(field: MetadataField) -> &'static str {
    match field {
        MetadataField::CoverImage => "cover_path",
        _ => field.as_str(),
    }
}

fn current_field_value(work: &Work, field: MetadataField) -> Option<String> {
    match field {
        MetadataField::Title => Some(work.title.clone()),
        MetadataField::TitleAliases => {
            if work.title_aliases.is_empty() {
                None
            } else {
                Some(work.title_aliases.join(" | "))
            }
        }
        MetadataField::Developer => work.developer.clone(),
        MetadataField::ReleaseDate => work.release_date.map(|value| value.to_string()),
        MetadataField::Rating => work.rating.map(|value| format!("{value:.1}")),
        MetadataField::Description => work.description.clone(),
        MetadataField::Tags => {
            if work.tags.is_empty() {
                None
            } else {
                Some(work.tags.join(", "))
            }
        }
        MetadataField::CoverImage => work.cover_path.clone(),
    }
}

#[tauri::command]
pub async fn set_work_field_preference(
    work_id: String,
    field: String,
    source: Option<String>,
    db: State<'_, Database>,
    vndb: State<'_, VndbClient>,
    bangumi: State<'_, BangumiClient>,
    dlsite: State<'_, DlsiteClient>,
) -> Result<(), AppError> {
    let work_id = queries::canonical::get_preferred_work_id(db.read_pool(), &work_id)
        .await?
        .unwrap_or(work_id);
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .ok_or_else(|| AppError::WorkNotFound(work_id.clone()))?;
    let mut work = row.into_work();

    let normalized_field = normalize_field_key(&field)
        .ok_or_else(|| AppError::Validation(format!("Unsupported field '{}'", field)))?;

    match source.as_deref() {
        None | Some("auto") => {
            work.field_preferences.remove(normalized_field);
        }
        Some(preferred @ ("vndb" | "bangumi" | "dlsite")) => {
            work.field_preferences
                .insert(normalized_field.to_string(), preferred.to_string());
        }
        Some(other) => {
            return Err(AppError::Validation(format!(
                "Unsupported source preference '{}'",
                other
            )))
        }
    }

    let linked = provider::fetch_linked_records(&work, &vndb, &bangumi, &dlsite)
        .await
        .map_err(AppError::Internal)?;
    let provider_defaults = queries::provider_rules::list_field_defaults(db.read_pool()).await?;
    resolver::resolve_with_defaults(
        &mut work,
        linked.0.as_ref().and_then(|record| record.as_vndb()),
        linked.1.as_ref().and_then(|record| record.as_bangumi()),
        linked.2.as_ref().and_then(|record| record.as_dlsite()),
        &provider_defaults,
    );
    queries::works::upsert_work(db.read_pool(), &work).await?;
    metadata_io::sync_metadata_from_work(&work, None).map_err(AppError::Io)?;
    queries::canonical::sync_work_ids(db.read_pool(), std::slice::from_ref(&work_id)).await?;
    Ok(())
}

fn normalize_field_key(field: &str) -> Option<&'static str> {
    match field {
        "title" => Some("title"),
        "title_aliases" => Some("title_aliases"),
        "developer" => Some("developer"),
        "release_date" => Some("release_date"),
        "rating" => Some("rating"),
        "description" => Some("description"),
        "tags" => Some("tags"),
        "cover_image" | "cover_path" => Some("cover_path"),
        _ => None,
    }
}

fn decide_field_action(
    work: &Work,
    field: MetadataField,
    source: MetadataSource,
    current_value: Option<&str>,
    candidate_value: Option<&str>,
) -> &'static str {
    let has_current = current_value.is_some_and(|value| !value.trim().is_empty());
    let has_candidate = candidate_value.is_some_and(|value| !value.trim().is_empty());

    if !has_current && !has_candidate {
        return "no_data";
    }
    if !has_candidate {
        return if has_current {
            "keep_current"
        } else {
            "no_data"
        };
    }
    if !has_current {
        return "fill";
    }

    match field {
        MetadataField::Title => {
            if should_override_title(work.title_source.clone(), source)
                && current_value != candidate_value
            {
                "override"
            } else {
                "keep_current"
            }
        }
        _ => "keep_current",
    }
}

fn should_override_title(current_source: FieldSource, candidate_source: MetadataSource) -> bool {
    if current_source == FieldSource::UserOverride {
        return false;
    }
    source_priority(candidate_source) >= field_source_priority(current_source)
}

fn source_priority(source: MetadataSource) -> i32 {
    match source {
        MetadataSource::Vndb => 3,
        MetadataSource::Dlsite => 2,
        MetadataSource::Bangumi => 1,
    }
}

fn field_source_priority(source: FieldSource) -> i32 {
    match source {
        FieldSource::UserOverride => 99,
        FieldSource::Vndb => 3,
        FieldSource::Dlsite => 2,
        FieldSource::Bangumi => 1,
        FieldSource::Filesystem => 0,
    }
}

fn field_source_label(source: FieldSource) -> &'static str {
    match source {
        FieldSource::Filesystem => "filesystem",
        FieldSource::Vndb => "vndb",
        FieldSource::Bangumi => "bangumi",
        FieldSource::Dlsite => "dlsite",
        FieldSource::UserOverride => "user_override",
    }
}

fn verdict_label(verdict: MatchVerdict) -> &'static str {
    match verdict {
        MatchVerdict::AutoMatch => "Auto Match",
        MatchVerdict::PendingReview => "Review",
        MatchVerdict::Rejected => "Weak",
    }
}

fn provider_action(state: ProviderLinkState) -> &'static str {
    match state {
        ProviderLinkState::NotLinked | ProviderLinkState::Ready => "No action needed.",
        ProviderLinkState::Missing => {
            "Clear or replace the stale source link, then refresh metadata."
        }
        ProviderLinkState::AuthError => {
            "Reconnect the provider account or refresh the token, then retry."
        }
        ProviderLinkState::RateLimited => "Wait for the provider cooldown, then retry enrichment.",
        ProviderLinkState::TransientError => {
            "Retry enrichment; if it persists, inspect provider connectivity."
        }
    }
}

fn queue_suggested_action(
    enrichment_state: &str,
    job_state: Option<&str>,
    last_error: Option<&str>,
) -> &'static str {
    match (enrichment_state, job_state) {
        (_, Some("retry_wait")) => "Waiting for automatic retry.",
        (_, Some("failed")) => "Reset this poster to enqueue a fresh metadata refresh.",
        ("pending_review", _) => "Open the review item and confirm the best candidate.",
        ("rejected", _) => "Reset enrichment if you want to search again.",
        ("unmatched", _) if last_error.is_some() => "Inspect the last provider error, then retry.",
        _ => "Run refresh or inspect provider candidates.",
    }
}

async fn latest_job_status(
    pool: &sqlx::SqlitePool,
    work_id: &str,
) -> Result<Option<ReviewJobStatus>, AppError> {
    let row = sqlx::query(
        "SELECT state, attempt_count, next_run_at, last_error
         FROM enrichment_jobs
         WHERE work_id = ?1
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(work_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| ReviewJobStatus {
        state: row.get("state"),
        attempt_count: row.get("attempt_count"),
        next_run_at: row.get("next_run_at"),
        last_error: row.get("last_error"),
    }))
}
