//! Brand + Creator API — list brands (from developer field), list creators, detail views.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sqlx::{FromRow, Row};
use tauri::State;

use crate::db::queries;
use crate::db::Database;
use crate::domain::error::AppError;

#[derive(Serialize)]
pub struct BrandSummary {
    pub name: String,
    pub works_count: i64,
}

#[derive(Serialize)]
pub struct BrandDetail {
    pub name: String,
    pub works_count: i64,
    pub works: Vec<BrandWork>,
}

#[derive(Serialize, FromRow)]
pub struct BrandWork {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub rating: Option<f64>,
    pub release_date: Option<String>,
}

#[derive(Serialize)]
pub struct CreatorSummary {
    pub id: String,
    pub name: String,
    pub name_original: Option<String>,
    pub role_type: String,
    pub works_count: i64,
    pub image_url: Option<String>,
}

#[derive(Serialize)]
pub struct CreatorDetail {
    pub id: String,
    pub name: String,
    pub name_original: Option<String>,
    pub role_type: String,
    pub image_url: Option<String>,
    pub description: Option<String>,
    pub roles: Vec<String>,
    pub works: Vec<CreatorWork>,
}

#[derive(Serialize, FromRow)]
pub struct CreatorWork {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub rating: Option<f64>,
    pub release_date: Option<String>,
    pub role: String,
    pub character_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(FromRow)]
struct CreatorBaseRow {
    id: String,
    name: String,
    name_original: Option<String>,
    image_url: Option<String>,
    description: Option<String>,
    roles: Option<String>,
    role_type: String,
}

#[derive(FromRow)]
struct CreditWorkRow {
    id: String,
    title: String,
    cover_path: Option<String>,
    rating: Option<f64>,
    release_date: Option<String>,
    role: String,
    character_name: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
pub async fn list_brands(
    db: State<'_, Database>,
    limit: Option<i64>,
) -> Result<Vec<BrandSummary>, AppError> {
    let limit = limit.unwrap_or(200).min(500);
    let pool = db.read_pool();

    let rows = sqlx::query(
        "SELECT developer as name, COUNT(*) as works_count FROM canonical_works \
         WHERE developer IS NOT NULL AND developer != '' \
         GROUP BY developer ORDER BY works_count DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| BrandSummary {
            name: r.get("name"),
            works_count: r.get("works_count"),
        })
        .collect())
}

#[tauri::command]
pub async fn get_brand_detail(
    db: State<'_, Database>,
    name: String,
) -> Result<BrandDetail, AppError> {
    let pool = db.read_pool();

    let works: Vec<BrandWork> = sqlx::query_as(
        "SELECT preferred_work_id as id, title, cover_path, rating, release_date FROM canonical_works \
         WHERE developer = ? ORDER BY release_date DESC",
    )
    .bind(&name)
    .fetch_all(pool)
    .await?;

    Ok(BrandDetail {
        name: name.clone(),
        works_count: works.len() as i64,
        works,
    })
}

#[tauri::command]
pub async fn list_creators(
    db: State<'_, Database>,
    limit: Option<i64>,
) -> Result<Vec<CreatorSummary>, AppError> {
    let limit = limit.unwrap_or(200).min(500);
    let pool = db.read_pool();

    let creators: Vec<CreatorBaseRow> = sqlx::query_as(
        "SELECT p.id, p.name, p.name_original, p.image_url, p.description, p.roles, \
         COALESCE(json_extract(p.roles, '$[0]'), 'staff') as role_type \
         FROM persons p \
         ORDER BY p.name",
    )
    .fetch_all(pool)
    .await?;

    let credit_pairs = sqlx::query("SELECT person_id, work_id FROM work_credits")
        .fetch_all(pool)
        .await?;

    let representative_by_work = queries::canonical::representative_work_map(pool).await?;

    let mut works_by_person: HashMap<String, HashSet<String>> = HashMap::new();
    for row in credit_pairs {
        let person_id: String = row.get("person_id");
        let work_id: String = row.get("work_id");
        let representative = representative_by_work
            .get(&work_id)
            .cloned()
            .unwrap_or(work_id);
        works_by_person
            .entry(person_id)
            .or_default()
            .insert(representative);
    }

    let mut summaries: Vec<CreatorSummary> = creators
        .into_iter()
        .map(|creator| CreatorSummary {
            id: creator.id.clone(),
            name: creator.name,
            name_original: creator.name_original,
            role_type: creator.role_type,
            works_count: works_by_person
                .get(&creator.id)
                .map(|work_ids| work_ids.len() as i64)
                .unwrap_or_default(),
            image_url: creator.image_url,
        })
        .collect();

    summaries.sort_by(|left, right| {
        right
            .works_count
            .cmp(&left.works_count)
            .then_with(|| left.name.cmp(&right.name))
    });
    summaries.truncate(limit as usize);
    Ok(summaries)
}

#[tauri::command]
pub async fn get_creator_detail(
    db: State<'_, Database>,
    id: String,
) -> Result<Option<CreatorDetail>, AppError> {
    let pool = db.read_pool();

    let creator: Option<CreatorBaseRow> = sqlx::query_as(
        "SELECT p.id, p.name, p.name_original, p.image_url, p.description, p.roles, \
         COALESCE(json_extract(p.roles, '$[0]'), 'staff') as role_type \
         FROM persons p \
         WHERE p.id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = creator else {
        return Ok(None);
    };

    let raw_works: Vec<CreditWorkRow> = sqlx::query_as(
        "SELECT w.id, w.title, w.cover_path, w.rating, w.release_date, \
         wc.role, wc.character_name, wc.notes \
         FROM works w \
         JOIN work_credits wc ON wc.work_id = w.id \
         WHERE wc.person_id = ? \
         ORDER BY w.release_date DESC, w.title, wc.role",
    )
    .bind(&id)
    .fetch_all(pool)
    .await?;

    let representative_by_work = queries::canonical::representative_work_map(pool).await?;

    let mut seen = HashSet::new();
    let mut deduped_works = Vec::new();
    for work in raw_works {
        let representative_id = representative_by_work
            .get(&work.id)
            .cloned()
            .unwrap_or_else(|| work.id.clone());
        let key = (
            representative_id.clone(),
            work.role.clone(),
            work.character_name.clone(),
        );
        if seen.insert(key) {
            deduped_works.push(CreatorWork {
                id: representative_id,
                title: work.title,
                cover_path: work.cover_path,
                rating: work.rating,
                release_date: work.release_date,
                role: work.role,
                character_name: work.character_name,
                notes: work.notes,
            });
        }
    }

    deduped_works.sort_by(|left, right| {
        right
            .release_date
            .cmp(&left.release_date)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.role.cmp(&right.role))
    });

    let roles = row
        .roles
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(Some(CreatorDetail {
        id: row.id,
        name: row.name,
        name_original: row.name_original,
        role_type: row.role_type,
        image_url: row.image_url,
        description: row.description,
        roles,
        works: deduped_works,
    }))
}
