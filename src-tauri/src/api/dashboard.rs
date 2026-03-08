//! Dashboard API — aggregate stats for the dashboard page.

use chrono::Datelike;
use serde::Serialize;
use tauri::State;

use crate::config::SharedConfig;
use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;
use crate::domain::work::{EnrichmentState, WorkSummary};

#[derive(Serialize)]
pub struct DashboardStats {
    pub total_works: i64,
    pub total_brands: i64,
    pub total_matched: i64,
    pub total_favorites: i64,
    pub avg_rating: f64,
    pub match_percent: f64,
    pub top_brands: Vec<BrandCount>,
    pub genre_distribution: Vec<GenreCount>,
    pub rating_distribution: Vec<RatingBucket>,
    pub recent_works: Vec<RecentWork>,
    pub yearly_counts: Vec<YearlyCount>,
}

#[derive(Serialize)]
pub struct BrandCount {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct GenreCount {
    pub genre: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct RatingBucket {
    pub bucket: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct RecentWork {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub variant_count: u32,
}

#[derive(Serialize)]
pub struct YearlyCount {
    pub year: String,
    pub count: i64,
}

#[tauri::command]
pub async fn get_dashboard_stats(
    db: State<'_, Database>,
    config: State<'_, SharedConfig>,
) -> Result<DashboardStats, AppError> {
    let pool = db.read_pool();
    let cfg = config.read().await;
    let sfw = cfg.sfw_mode;
    drop(cfg);

    let stats = build_stats(pool, sfw).await?;
    Ok(stats)
}

#[tauri::command]
pub async fn toggle_sfw(config: State<'_, SharedConfig>) -> Result<bool, AppError> {
    let new_val = {
        let cfg = config.read().await;
        !cfg.sfw_mode
    };
    config.update(|cfg| cfg.sfw_mode = new_val).await?;
    Ok(new_val)
}

async fn build_stats(pool: &sqlx::SqlitePool, _sfw: bool) -> Result<DashboardStats, AppError> {
    let works: Vec<WorkSummary> =
        queries::canonical::list_canonical_works(pool, "title", false, None)
            .await?
            .into_iter()
            .map(|row| row.into_summary())
            .collect();
    let recent: Vec<WorkSummary> =
        queries::canonical::list_canonical_works(pool, "created_at", true, None)
            .await?
            .into_iter()
            .map(|row| row.into_summary())
            .collect();

    let total_works = works.len() as i64;
    let total_matched = works
        .iter()
        .filter(|work| matches!(work.enrichment_state, EnrichmentState::Matched))
        .count() as i64;
    let total_favorites = works
        .iter()
        .filter(|work| {
            matches!(
                work.library_status,
                crate::domain::work::LibraryStatus::Completed
            )
        })
        .count() as i64;
    let total_brands = works
        .iter()
        .filter_map(|work| work.developer.clone())
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    let ratings: Vec<f64> = works.iter().filter_map(|work| work.rating).collect();
    let avg_rating = if ratings.is_empty() {
        0.0
    } else {
        ratings.iter().sum::<f64>() / ratings.len() as f64
    };

    let match_percent = if total_works > 0 {
        (total_matched as f64 / total_works as f64) * 100.0
    } else {
        0.0
    };

    // Top brands
    let mut brand_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for work in &works {
        if let Some(developer) = &work.developer {
            *brand_counts.entry(developer.clone()).or_insert(0) += 1;
        }
    }
    let mut top_brands: Vec<BrandCount> = brand_counts
        .into_iter()
        .map(|(name, count)| BrandCount { name, count })
        .collect();
    top_brands.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    top_brands.truncate(10);

    // Rating distribution
    let rating_dist = build_rating_distribution(&works);

    // Recent works
    let recent_works: Vec<RecentWork> = recent
        .iter()
        .take(8)
        .map(|work| RecentWork {
            id: work.id.to_string(),
            title: work.title.clone(),
            cover_path: work.cover_path.clone(),
            developer: work.developer.clone(),
            variant_count: work.variant_count,
        })
        .collect();

    // Yearly counts
    let mut yearly_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for work in &works {
        if let Some(date) = work.release_date {
            *yearly_map.entry(date.year().to_string()).or_insert(0) += 1;
        }
    }
    let mut yearly_counts: Vec<YearlyCount> = yearly_map
        .into_iter()
        .map(|(year, count)| YearlyCount { year, count })
        .collect();
    yearly_counts.sort_by(|left, right| right.year.cmp(&left.year));
    yearly_counts.truncate(10);

    Ok(DashboardStats {
        total_works,
        total_brands,
        total_matched,
        total_favorites,
        avg_rating,
        match_percent,
        top_brands,
        genre_distribution: Vec::new(), // TODO: from auto_tags
        rating_distribution: rating_dist,
        recent_works,
        yearly_counts,
    })
}

fn build_rating_distribution(works: &[WorkSummary]) -> Vec<RatingBucket> {
    let buckets = [
        ("9-10", 9.0, 10.1),
        ("8-9", 8.0, 9.0),
        ("7-8", 7.0, 8.0),
        ("6-7", 6.0, 7.0),
        ("<6", 0.0, 6.0),
    ];

    let mut result = Vec::new();
    for (label, low, high) in buckets {
        result.push(RatingBucket {
            bucket: label.to_string(),
            count: works
                .iter()
                .filter_map(|work| work.rating)
                .filter(|rating| *rating >= low && *rating < high)
                .count() as i64,
        });
    }
    result
}
