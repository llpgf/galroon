use galroon_lib::config::AppConfig;
use galroon_lib::db::queries;
use galroon_lib::db::Database;
use galroon_lib::enrichment::bangumi::BangumiClient;
use galroon_lib::enrichment::cache;
use galroon_lib::enrichment::dlsite::DlsiteClient;
use galroon_lib::enrichment::query;
use galroon_lib::enrichment::rate_limit::RateLimiter;
use galroon_lib::enrichment::vndb::VndbClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = std::path::PathBuf::from(std::env::args().nth(1).expect("workspace"));
    let target = std::env::args().nth(2).expect("title");
    let config = AppConfig::load_from(&workspace)?;
    let db = Database::new(&config.db_path).await?;
    let rate_limiter = RateLimiter::new();
    let vndb = VndbClient::new(rate_limiter.clone());
    let bangumi = BangumiClient::new(rate_limiter.clone(), config.bangumi.clone(), None);
    let dlsite = DlsiteClient::new(rate_limiter);

    let rows = sqlx::query_as::<_, (String, String)>("SELECT id, title FROM works")
        .fetch_all(db.read_pool())
        .await?;
    let work_id = rows
        .into_iter()
        .find(|(_, title)| title == &target)
        .map(|(id, _)| id)
        .expect("work id");
    let row = queries::works::get_work_by_id(db.read_pool(), &work_id)
        .await?
        .expect("work row");
    let work = row.into_work();
    let query_input = query::build_query_input(&work);
    eprintln!("search terms: {:?}", query_input.search_terms);
    let candidates =
        cache::search_candidates(db.read_pool(), &vndb, &bangumi, &dlsite, &query_input, 10).await;
    for candidate in candidates.into_iter().take(10) {
        println!(
            "{}\t{}\t{:.1}\t{:?}\t{}",
            candidate.source.as_str(),
            candidate.id,
            candidate.score,
            candidate.verdict,
            candidate.title
        );
    }
    Ok(())
}
