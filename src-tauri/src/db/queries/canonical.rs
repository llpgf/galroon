use std::collections::{HashMap, HashSet};

use sqlx::{FromRow, Row, SqlitePool};

use crate::api::posters;
use crate::db::models::{WorkRow, WorkSummaryRow};
use crate::domain::error::AppResult;
use crate::domain::work::{EnrichmentState, Work};

#[derive(Debug, Clone, FromRow)]
pub struct CanonicalWorkRow {
    pub canonical_key: String,
    pub preferred_work_id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub library_status: String,
    pub enrichment_state: String,
    pub tags: Option<String>,
    pub release_date: Option<String>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub dlsite_id: Option<String>,
    pub description: Option<String>,
    pub variant_count: i64,
    pub asset_count: i64,
    pub asset_types: Option<String>,
    pub primary_asset_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct CanonicalAssetGroupRow {
    pub canonical_key: String,
    pub asset_type: String,
    pub relation_role: String,
    pub parent_asset_type: Option<String>,
    pub asset_count: i64,
    pub variant_count: i64,
    pub representative_work_id: Option<String>,
    pub representative_path: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct VariantOverrideRow {
    work_id: String,
    manual_group_key: String,
    make_representative: i64,
}

#[derive(Debug, Clone)]
struct VariantOverride {
    manual_group_key: String,
    make_representative: bool,
}

pub async fn rebuild(pool: &SqlitePool) -> AppResult<()> {
    let rows: Vec<WorkRow> = sqlx::query_as("SELECT * FROM works ORDER BY title")
        .fetch_all(pool)
        .await?;
    let overrides = load_variant_overrides(pool).await?;
    let groups = group_works_with_overrides(
        rows.into_iter().map(|row| row.into_work()).collect(),
        &overrides,
    );

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM work_variants")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM canonical_works")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM canonical_asset_groups")
        .execute(&mut *tx)
        .await?;

    for group in groups {
        insert_group(&mut tx, group).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn sync_work_ids(pool: &SqlitePool, work_ids: &[String]) -> AppResult<()> {
    let affected_ids: HashSet<String> = work_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if affected_ids.is_empty() {
        return Ok(());
    }

    let overrides = load_variant_overrides(pool).await?;
    let mut affected_keys = HashSet::new();
    for work_id in &affected_ids {
        let old_keys = sqlx::query("SELECT canonical_key FROM work_variants WHERE work_id = ?")
            .bind(work_id)
            .fetch_all(pool)
            .await?;
        for row in old_keys {
            affected_keys.insert(row.get::<String, _>("canonical_key"));
        }

        if let Some(row) = sqlx::query_as::<_, WorkRow>("SELECT * FROM works WHERE id = ?")
            .bind(work_id)
            .fetch_optional(pool)
            .await?
        {
            let work = row.into_work();
            affected_keys.insert(resolved_canonical_key(&work, &overrides));
        }
    }

    if affected_keys.is_empty() {
        return Ok(());
    }

    let rows: Vec<WorkRow> = sqlx::query_as("SELECT * FROM works ORDER BY title")
        .fetch_all(pool)
        .await?;
    let groups = group_works_with_overrides(
        rows.into_iter().map(|row| row.into_work()).collect(),
        &overrides,
    );
    let affected_groups: Vec<_> = groups
        .into_iter()
        .filter(|group| affected_keys.contains(&group.canonical_key))
        .collect();

    let mut tx = pool.begin().await?;
    for work_id in &affected_ids {
        sqlx::query("DELETE FROM work_variants WHERE work_id = ?")
            .bind(work_id)
            .execute(&mut *tx)
            .await?;
    }
    for canonical_key in &affected_keys {
        sqlx::query("DELETE FROM work_variants WHERE canonical_key = ?")
            .bind(canonical_key)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM canonical_works WHERE canonical_key = ?")
            .bind(canonical_key)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM canonical_asset_groups WHERE canonical_key = ?")
            .bind(canonical_key)
            .execute(&mut *tx)
            .await?;
    }

    for group in affected_groups {
        insert_group(&mut tx, group).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn list_canonical_works(
    pool: &SqlitePool,
    sort_by: &str,
    descending: bool,
    asset_type: Option<&str>,
) -> AppResult<Vec<WorkSummaryRow>> {
    let sort_col = match sort_by {
        "title" => "title",
        "developer" => "developer",
        "rating" => "rating",
        "release_date" => "release_date",
        "created_at" => "created_at",
        "updated_at" => "updated_at",
        _ => "title",
    };
    let dir = if descending { "DESC" } else { "ASC" };

    let query = format!(
        "SELECT
            preferred_work_id as id,
            title,
            cover_path,
            developer,
            rating,
            library_status,
            enrichment_state,
            tags,
            release_date,
            vndb_id,
            bangumi_id,
            dlsite_id,
            variant_count,
            asset_count,
            asset_types,
            primary_asset_type
         FROM canonical_works
         ORDER BY {sort_col} {dir} NULLS LAST"
    );

    let mut rows: Vec<WorkSummaryRow> = sqlx::query_as(&query).fetch_all(pool).await?;
    if let Some(filter) = asset_type.map(str::trim).filter(|value| !value.is_empty()) {
        rows.retain(|row| canonical_row_has_asset_type(row, filter));
    }

    Ok(rows)
}

pub async fn list_all_canonical(pool: &SqlitePool) -> AppResult<Vec<CanonicalWorkRow>> {
    let rows = sqlx::query_as(
        "SELECT canonical_key, preferred_work_id, title, cover_path, developer, rating,
                library_status, enrichment_state, tags, release_date, vndb_id, bangumi_id,
                dlsite_id, description, variant_count, asset_count, asset_types, primary_asset_type,
                created_at, updated_at
         FROM canonical_works",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_preferred_work_id(pool: &SqlitePool, work_id: &str) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT cw.preferred_work_id
         FROM work_variants wv
         JOIN canonical_works cw ON cw.canonical_key = wv.canonical_key
         WHERE wv.work_id = ?1",
    )
    .bind(work_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|value| value.0))
}

pub async fn get_canonical_key(pool: &SqlitePool, work_id: &str) -> AppResult<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT canonical_key FROM work_variants WHERE work_id = ?1")
            .bind(work_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|value| value.0))
}

pub async fn list_variant_ids(pool: &SqlitePool, work_id: &str) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT sibling.work_id
         FROM work_variants current
         JOIN work_variants sibling ON sibling.canonical_key = current.canonical_key
         WHERE current.work_id = ?1
         ORDER BY sibling.is_representative DESC, sibling.work_id",
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(vec![work_id.to_string()]);
    }

    Ok(rows.into_iter().map(|row| row.get("work_id")).collect())
}

pub async fn representative_work_map(pool: &SqlitePool) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query(
        "SELECT wv.work_id, cw.preferred_work_id
         FROM work_variants wv
         JOIN canonical_works cw ON cw.canonical_key = wv.canonical_key",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.get("work_id"), row.get("preferred_work_id")))
        .collect())
}

pub async fn duplicate_groups(pool: &SqlitePool) -> AppResult<Vec<CanonicalWorkRow>> {
    let rows = sqlx::query_as(
        "SELECT canonical_key, preferred_work_id, title, cover_path, developer, rating,
                library_status, enrichment_state, tags, release_date, vndb_id, bangumi_id,
                dlsite_id, description, variant_count, asset_count, asset_types, primary_asset_type,
                created_at, updated_at
         FROM canonical_works
         WHERE variant_count > 1
         ORDER BY variant_count DESC, title",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_asset_groups(
    pool: &SqlitePool,
    work_id: &str,
) -> AppResult<Vec<CanonicalAssetGroupRow>> {
    let canonical_key = get_canonical_key(pool, work_id)
        .await?
        .ok_or_else(|| crate::domain::error::AppError::WorkNotFound(work_id.to_string()))?;

    let rows = sqlx::query_as(
        "SELECT canonical_key, asset_type, asset_count, variant_count, representative_work_id, representative_path
                , relation_role, parent_asset_type
         FROM canonical_asset_groups
         WHERE canonical_key = ?
         ORDER BY asset_count DESC, asset_type",
    )
    .bind(canonical_key)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn insert_group(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    group: PosterGroupRecord,
) -> AppResult<()> {
    let summary = group.group.clone().into_summary();
    let representative = group.group.representative.clone();
    let earliest_created_at = group
        .group
        .variants
        .iter()
        .map(|variant| variant.created_at)
        .min()
        .unwrap_or(representative.created_at)
        .to_rfc3339();
    let variant_ids: Vec<String> = group
        .group
        .variants
        .iter()
        .map(|variant| variant.id.to_string())
        .collect();
    let (asset_count, asset_types, primary_asset_type) =
        aggregate_group_assets(tx, &variant_ids).await?;

    sqlx::query(
        "INSERT INTO canonical_works (
            canonical_key, preferred_work_id, title, cover_path, developer, rating,
            library_status, enrichment_state, tags, release_date, vndb_id, bangumi_id,
            dlsite_id, description, variant_count, asset_count, asset_types, primary_asset_type,
            created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6,
            ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20
        )",
    )
    .bind(&group.canonical_key)
    .bind(representative.id.to_string())
    .bind(&summary.title)
    .bind(&summary.cover_path)
    .bind(&summary.developer)
    .bind(summary.rating)
    .bind(
        serde_json::to_string(&summary.library_status)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(
        serde_json::to_string(&summary.enrichment_state)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .bind(serde_json::to_string(&summary.tags)?)
    .bind(summary.release_date.map(|date| date.to_string()))
    .bind(&summary.vndb_id)
    .bind(&summary.bangumi_id)
    .bind(&summary.dlsite_id)
    .bind(&representative.description)
    .bind(summary.variant_count as i64)
    .bind(asset_count)
    .bind(serde_json::to_string(&asset_types)?)
    .bind(primary_asset_type)
    .bind(earliest_created_at)
    .bind(group.group.latest_created_at().to_rfc3339())
    .execute(&mut **tx)
    .await?;

    for variant in group.group.variants {
        sqlx::query(
            "INSERT INTO work_variants (work_id, canonical_key, is_representative) VALUES (?1, ?2, ?3)"
        )
        .bind(variant.id.to_string())
        .bind(&group.canonical_key)
        .bind((variant.id == representative.id) as i64)
        .execute(&mut **tx)
        .await?;
    }

    insert_asset_groups(tx, &group.canonical_key, &variant_ids).await?;
    normalize_asset_group_relationships(tx, &group.canonical_key).await?;

    Ok(())
}

async fn insert_asset_groups(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    canonical_key: &str,
    variant_ids: &[String],
) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT
            a.asset_type,
            COUNT(*) as asset_count,
            COUNT(DISTINCT a.work_id) as variant_count
         FROM assets a
         WHERE a.work_id IN (SELECT work_id FROM work_variants WHERE canonical_key = ?1)
         GROUP BY a.asset_type
         ORDER BY asset_count DESC, a.asset_type",
    )
    .bind(canonical_key)
    .fetch_all(&mut **tx)
    .await?;

    let ordered_asset_types = rows
        .iter()
        .map(|row| row.get::<String, _>("asset_type"))
        .collect::<Vec<_>>();
    let primary_parent = ordered_asset_types
        .iter()
        .find(|value| value.eq_ignore_ascii_case("game"))
        .cloned();

    for row in rows {
        let asset_type: String = row.get("asset_type");
        let asset_count: i64 = row.get("asset_count");
        let variant_count: i64 = row.get("variant_count");
        let (relation_role, parent_asset_type) =
            classify_asset_relationship(&asset_type, primary_parent.as_deref());

        let representative = sqlx::query(
            "SELECT a.work_id, a.path, COUNT(*) as count
             FROM assets a
             WHERE a.asset_type = ?1
               AND a.work_id IN (SELECT work_id FROM work_variants WHERE canonical_key = ?2)
             GROUP BY a.work_id, a.path
             ORDER BY count DESC, a.work_id, a.path
             LIMIT 1",
        )
        .bind(&asset_type)
        .bind(canonical_key)
        .fetch_optional(&mut **tx)
        .await?;

        let (representative_work_id, representative_path) = representative
            .map(|value| {
                (
                    value.get::<String, _>("work_id"),
                    value.get::<String, _>("path"),
                )
            })
            .map(|(work_id, path)| (Some(work_id), Some(path)))
            .unwrap_or((variant_ids.first().cloned(), None));

        sqlx::query(
            "INSERT INTO canonical_asset_groups (
                canonical_key, asset_type, relation_role, parent_asset_type, asset_count, variant_count,
                representative_work_id, representative_path, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
        )
        .bind(canonical_key)
        .bind(asset_type)
        .bind(relation_role)
        .bind(parent_asset_type)
        .bind(asset_count)
        .bind(variant_count)
        .bind(representative_work_id)
        .bind(representative_path)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

fn classify_asset_relationship<'a>(
    asset_type: &str,
    primary_parent: Option<&'a str>,
) -> (&'static str, Option<&'a str>) {
    if asset_type.eq_ignore_ascii_case("game") {
        return ("primary", None);
    }
    if asset_type.eq_ignore_ascii_case("ost") || asset_type.eq_ignore_ascii_case("voice_drama") {
        return ("companion", primary_parent);
    }
    if asset_type.eq_ignore_ascii_case("update")
        || asset_type.eq_ignore_ascii_case("dlc")
        || asset_type.eq_ignore_ascii_case("crack")
        || asset_type.eq_ignore_ascii_case("bonus")
        || asset_type.eq_ignore_ascii_case("guide")
        || asset_type.eq_ignore_ascii_case("save")
    {
        return ("supplemental", primary_parent);
    }
    ("supplemental", primary_parent)
}

async fn normalize_asset_group_relationships(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    canonical_key: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE canonical_asset_groups
         SET relation_role = CASE
                WHEN lower(asset_type) = 'game' THEN 'primary'
                WHEN lower(asset_type) IN ('ost', 'voice_drama') THEN 'companion'
                ELSE 'supplemental'
             END,
             parent_asset_type = CASE
                WHEN lower(asset_type) = 'game' THEN NULL
                WHEN EXISTS (
                    SELECT 1 FROM canonical_asset_groups cg
                    WHERE cg.canonical_key = canonical_asset_groups.canonical_key
                      AND lower(cg.asset_type) = 'game'
                ) THEN 'game'
                ELSE NULL
             END
         WHERE canonical_key = ?1",
    )
    .bind(canonical_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn aggregate_group_assets(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    variant_ids: &[String],
) -> AppResult<(i64, Vec<String>, Option<String>)> {
    let mut counts: HashMap<String, i64> = HashMap::new();

    for variant_id in variant_ids {
        let rows = sqlx::query(
            "SELECT asset_type, COUNT(*) as count FROM assets WHERE work_id = ? GROUP BY asset_type"
        )
        .bind(variant_id)
        .fetch_all(&mut **tx)
        .await?;

        for row in rows {
            let asset_type: String = row.get("asset_type");
            let count: i64 = row.get("count");
            *counts.entry(asset_type).or_default() += count;
        }
    }

    let total = counts.values().copied().sum();
    let mut ordered: Vec<(String, i64)> = counts.into_iter().collect();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let asset_types = ordered
        .iter()
        .map(|(asset_type, _)| asset_type.clone())
        .collect::<Vec<_>>();
    let primary_asset_type = ordered.first().map(|(asset_type, _)| asset_type.clone());

    Ok((total, asset_types, primary_asset_type))
}

fn canonical_row_has_asset_type(row: &WorkSummaryRow, asset_type: &str) -> bool {
    row.asset_types
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .any(|value| value.eq_ignore_ascii_case(asset_type))
}

async fn load_variant_overrides(pool: &SqlitePool) -> AppResult<HashMap<String, VariantOverride>> {
    let rows: Vec<VariantOverrideRow> = sqlx::query_as(
        "SELECT work_id, manual_group_key, make_representative FROM canonical_variant_overrides",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.work_id,
                VariantOverride {
                    manual_group_key: row.manual_group_key,
                    make_representative: row.make_representative == 1,
                },
            )
        })
        .collect())
}

fn group_works_with_overrides(
    works: Vec<Work>,
    overrides: &HashMap<String, VariantOverride>,
) -> Vec<PosterGroupRecord> {
    let mut grouped: HashMap<String, Vec<Work>> = HashMap::new();
    for work in works {
        let canonical_key = resolved_canonical_key(&work, overrides);
        grouped.entry(canonical_key).or_default().push(work);
    }

    grouped
        .into_iter()
        .map(|(canonical_key, mut variants)| {
            variants.sort_by(compare_work_quality);
            variants.reverse();
            let representative = choose_representative(&variants, overrides, &canonical_key);
            PosterGroupRecord {
                canonical_key,
                group: posters::PosterGroup {
                    representative,
                    variants,
                },
            }
        })
        .collect()
}

fn resolved_canonical_key(work: &Work, overrides: &HashMap<String, VariantOverride>) -> String {
    overrides
        .get(&work.id.to_string())
        .map(|value| value.manual_group_key.clone())
        .unwrap_or_else(|| posters::canonical_key(work))
}

fn choose_representative(
    variants: &[Work],
    overrides: &HashMap<String, VariantOverride>,
    canonical_key: &str,
) -> Work {
    if let Some(work) = variants.iter().find(|variant| {
        overrides.get(&variant.id.to_string()).is_some_and(|value| {
            value.make_representative && value.manual_group_key == canonical_key
        })
    }) {
        return work.clone();
    }

    variants
        .first()
        .cloned()
        .expect("canonical group should contain at least one work")
}

fn compare_work_quality(left: &Work, right: &Work) -> std::cmp::Ordering {
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

#[derive(Debug, Clone)]
struct PosterGroupRecord {
    canonical_key: String,
    group: posters::PosterGroup,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("galroon_canonical_{}_{}.db", name, Uuid::new_v4()))
    }

    async fn insert_work(
        db: &Database,
        id: &str,
        folder_path: &str,
        title: &str,
        cover_path: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO works (
                id, folder_path, title, title_aliases, cover_path, tags, user_tags,
                library_status, enrichment_state, title_source, folder_mtime, metadata_mtime,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, '[]', ?4, '[]', '[]',
                'unplayed', 'matched', 'filesystem', 0, 0,
                '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
            )",
        )
        .bind(id)
        .bind(folder_path)
        .bind(title)
        .bind(cover_path)
        .execute(db.read_pool())
        .await
        .expect("insert work");
    }

    #[tokio::test]
    async fn split_override_creates_separate_canonical_poster() {
        let db = Database::new(&temp_db_path("split_override"))
            .await
            .expect("db init");
        insert_work(
            &db,
            "00000000-0000-0000-0000-000000000001",
            "C:/tmp/w1",
            "Same Title",
            Some("cover-a"),
        )
        .await;
        insert_work(
            &db,
            "00000000-0000-0000-0000-000000000002",
            "C:/tmp/w2",
            "Same Title",
            None,
        )
        .await;

        rebuild(db.read_pool())
            .await
            .expect("rebuild canonical works");
        let groups = duplicate_groups(db.read_pool())
            .await
            .expect("duplicate groups before split");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].variant_count, 2);

        sqlx::query(
            "INSERT INTO canonical_variant_overrides (work_id, manual_group_key, make_representative)
             VALUES (?1, ?2, 1)"
        )
        .bind("00000000-0000-0000-0000-000000000002")
        .bind("manual:split:00000000-0000-0000-0000-000000000002")
        .execute(db.read_pool())
        .await
        .expect("insert split override");

        sync_work_ids(
            db.read_pool(),
            &[
                "00000000-0000-0000-0000-000000000001".to_string(),
                "00000000-0000-0000-0000-000000000002".to_string(),
            ],
        )
        .await
        .expect("sync split override");

        let all = list_all_canonical(db.read_pool())
            .await
            .expect("list canonical after split");
        assert_eq!(all.len(), 2);
        let groups = duplicate_groups(db.read_pool())
            .await
            .expect("duplicate groups after split");
        assert!(groups.is_empty());
        let preferred =
            get_preferred_work_id(db.read_pool(), "00000000-0000-0000-0000-000000000002")
                .await
                .expect("preferred work id")
                .expect("preferred work exists");
        assert_eq!(preferred, "00000000-0000-0000-0000-000000000002");
    }

    #[tokio::test]
    async fn representative_override_changes_preferred_work() {
        let db = Database::new(&temp_db_path("representative_override"))
            .await
            .expect("db init");
        insert_work(
            &db,
            "00000000-0000-0000-0000-000000000001",
            "C:/tmp/rep1",
            "Rep Title",
            Some("cover-a"),
        )
        .await;
        insert_work(
            &db,
            "00000000-0000-0000-0000-000000000002",
            "C:/tmp/rep2",
            "Rep Title",
            Some("cover-b"),
        )
        .await;

        rebuild(db.read_pool())
            .await
            .expect("rebuild canonical works");
        let original_key =
            get_canonical_key(db.read_pool(), "00000000-0000-0000-0000-000000000001")
                .await
                .expect("get canonical key")
                .expect("key exists");

        sqlx::query(
            "INSERT INTO canonical_variant_overrides (work_id, manual_group_key, make_representative)
             VALUES (?1, ?2, 1)"
        )
        .bind("00000000-0000-0000-0000-000000000002")
        .bind(&original_key)
        .execute(db.read_pool())
        .await
        .expect("insert representative override");

        sync_work_ids(
            db.read_pool(),
            &[
                "00000000-0000-0000-0000-000000000001".to_string(),
                "00000000-0000-0000-0000-000000000002".to_string(),
            ],
        )
        .await
        .expect("sync representative override");

        let preferred =
            get_preferred_work_id(db.read_pool(), "00000000-0000-0000-0000-000000000001")
                .await
                .expect("preferred work id")
                .expect("preferred work exists");
        assert_eq!(preferred, "00000000-0000-0000-0000-000000000002");
    }
}
