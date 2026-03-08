use std::path::PathBuf;

use galroon_lib::config::{AppConfig, LauncherConfig};
use galroon_lib::db::queries;
use galroon_lib::db::Database;
use galroon_lib::domain::work::EnrichmentState;
use galroon_lib::enrichment::bangumi::BangumiClient;
use galroon_lib::enrichment::cache;
use galroon_lib::enrichment::dlsite::DlsiteClient;
use galroon_lib::enrichment::people;
use galroon_lib::enrichment::provider::{self, MetadataSource};
use galroon_lib::enrichment::query;
use galroon_lib::enrichment::rate_limit::RateLimiter;
use galroon_lib::enrichment::resolver;
use galroon_lib::enrichment::search::SearchCandidate;
use galroon_lib::enrichment::vndb::VndbClient;
use galroon_lib::fs::metadata_io;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| {
            LauncherConfig::load()
                .ok()
                .and_then(|cfg| cfg.last_workspace)
        })
        .ok_or("workspace path required")?;
    let config = AppConfig::load_from(&workspace)?;
    let db = Database::new(&config.db_path).await?;

    let rate_limiter = RateLimiter::new();
    let vndb = VndbClient::new(rate_limiter.clone());
    let bangumi = BangumiClient::new(rate_limiter.clone(), config.bangumi.clone(), None);
    let dlsite = DlsiteClient::new(rate_limiter);

    let works = sqlx::query_as::<_, (String,)>("SELECT id FROM works ORDER BY title")
        .fetch_all(db.read_pool())
        .await?;
    let provider_defaults = queries::provider_rules::list_field_defaults(db.read_pool()).await?;

    let mut matched = 0_u32;
    let mut pending = 0_u32;
    let mut unmatched = 0_u32;

    for (work_id,) in works {
        let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
            .await?
            .expect("work exists");
        let mut work = row.into_work();
        let mut query_input = query::build_query_input(&work);
        let (linked_vndb, linked_bangumi, linked_dlsite) =
            provider::fetch_linked_records(&work, &vndb, &bangumi, &dlsite).await?;
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
                cache::search_candidates(db.read_pool(), &vndb, &bangumi, &dlsite, &query_input, 10)
                    .await
            };

        let best_vndb = best_candidate_for_source(&candidates, MetadataSource::Vndb);
        let best_bangumi = best_candidate_for_source(&candidates, MetadataSource::Bangumi);
        let best_dlsite = best_candidate_for_source(&candidates, MetadataSource::Dlsite);

        let vndb_auto = best_vndb.as_ref().filter(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::AutoMatch
        });
        let bangumi_auto = best_bangumi.as_ref().filter(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::AutoMatch
        });
        let dlsite_auto = best_dlsite.as_ref().filter(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::AutoMatch
        });

        let vndb_record = if linked_vndb.is_some() {
            linked_vndb
        } else if let Some(candidate) = vndb_auto {
            provider::fetch_record(
                MetadataSource::Vndb,
                &candidate.id,
                &vndb,
                &bangumi,
                &dlsite,
            )
            .await?
            .or_else(|| candidate.record.clone())
        } else {
            None
        };
        let bangumi_record = if linked_bangumi.is_some() {
            linked_bangumi
        } else if let Some(candidate) = bangumi_auto {
            provider::fetch_record(
                MetadataSource::Bangumi,
                &candidate.id,
                &vndb,
                &bangumi,
                &dlsite,
            )
            .await?
            .or_else(|| candidate.record.clone())
        } else {
            None
        };
        let dlsite_record = if linked_dlsite.is_some() {
            linked_dlsite
        } else if let Some(candidate) = dlsite_auto {
            provider::fetch_record(
                MetadataSource::Dlsite,
                &candidate.id,
                &vndb,
                &bangumi,
                &dlsite,
            )
            .await?
            .or_else(|| candidate.record.clone())
        } else {
            None
        };

        if vndb_record.is_some() || bangumi_record.is_some() || dlsite_record.is_some() {
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
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None)?;
            sync_related_people(
                db.read_pool(),
                &bangumi,
                &work.id.to_string(),
                bangumi_record
                    .as_ref()
                    .and_then(|record| record.as_bangumi()),
            )
            .await?;
            for record in [
                vndb_record.as_ref(),
                bangumi_record.as_ref(),
                dlsite_record.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                cache::remember_record(db.read_pool(), &query_input, record, 100.0).await?;
            }
            matched += 1;
            println!("MATCHED\t{}\t{}", work.id, work.title);
        } else if best_vndb.as_ref().is_some_and(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::PendingReview
        }) || best_bangumi.as_ref().is_some_and(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::PendingReview
        }) || best_dlsite.as_ref().is_some_and(|candidate| {
            candidate.verdict == galroon_lib::enrichment::matcher::MatchVerdict::PendingReview
        }) {
            work.enrichment_state = EnrichmentState::PendingReview;
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None)?;
            sync_related_people(db.read_pool(), &bangumi, &work.id.to_string(), None).await?;
            pending += 1;
            println!("PENDING\t{}\t{}", work.id, work.title);
        } else {
            work.enrichment_state = EnrichmentState::Unmatched;
            queries::works::upsert_work(db.read_pool(), &work).await?;
            metadata_io::sync_metadata_from_work(&work, None)?;
            sync_related_people(db.read_pool(), &bangumi, &work.id.to_string(), None).await?;
            unmatched += 1;
            println!("UNMATCHED\t{}\t{}", work.id, work.title);
        }
    }

    queries::canonical::rebuild(db.read_pool()).await?;
    println!(
        "SUMMARY\tmatched={}\tpending={}\tunmatched={}",
        matched, pending, unmatched
    );
    Ok(())
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

async fn sync_related_people(
    pool: &sqlx::SqlitePool,
    bangumi: &BangumiClient,
    work_id: &str,
    subject: Option<&galroon_lib::enrichment::bangumi::BangumiSubject>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(subject) = subject {
        let persons = bangumi.get_subject_persons(subject.id).await?;
        let characters = bangumi.get_subject_characters(subject.id).await?;
        let bundle = people::extract_bangumi_people(&persons, &characters);
        queries::people::replace_for_work(
            pool,
            work_id,
            &bundle.persons,
            &bundle.characters,
            &bundle.character_links,
            &bundle.credits,
        )
        .await?;
    } else {
        queries::people::clear_for_work(pool, work_id).await?;
    }

    Ok(())
}
