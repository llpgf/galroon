//! People and credit persistence queries.

use sqlx::SqlitePool;

use crate::domain::error::AppResult;

#[derive(Debug, Clone)]
pub struct UpsertPersonInput {
    pub id: String,
    pub name: String,
    pub name_original: Option<String>,
    pub vndb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub roles_json: String,
    pub image_url: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertCharacterInput {
    pub id: String,
    pub vndb_id: Option<String>,
    pub name: String,
    pub name_original: Option<String>,
    pub gender: String,
    pub birthday: Option<String>,
    pub bust: Option<String>,
    pub height: Option<i64>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub role: String,
    pub voice_actor: Option<String>,
    pub traits_json: String,
}

#[derive(Debug, Clone)]
pub struct WorkCharacterLinkInput {
    pub character_id: String,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct WorkCreditInput {
    pub person_id: String,
    pub role: String,
    pub character_name: Option<String>,
    pub notes: Option<String>,
}

pub async fn clear_for_work(pool: &SqlitePool, work_id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM work_characters WHERE work_id = ?")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM work_credits WHERE work_id = ?")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn replace_for_work(
    pool: &SqlitePool,
    work_id: &str,
    persons: &[UpsertPersonInput],
    characters: &[UpsertCharacterInput],
    character_links: &[WorkCharacterLinkInput],
    credits: &[WorkCreditInput],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM work_characters WHERE work_id = ?")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM work_credits WHERE work_id = ?")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;

    for person in persons {
        sqlx::query(
            "INSERT INTO persons (id, name, name_original, vndb_id, bangumi_id, roles, image_url, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET \
               name = excluded.name, \
               name_original = excluded.name_original, \
               vndb_id = excluded.vndb_id, \
               bangumi_id = excluded.bangumi_id, \
               roles = excluded.roles, \
               image_url = excluded.image_url, \
               description = excluded.description",
        )
        .bind(&person.id)
        .bind(&person.name)
        .bind(&person.name_original)
        .bind(&person.vndb_id)
        .bind(&person.bangumi_id)
        .bind(&person.roles_json)
        .bind(&person.image_url)
        .bind(&person.description)
        .execute(&mut *tx)
        .await?;
    }

    for character in characters {
        sqlx::query(
            "INSERT INTO characters (id, vndb_id, name, name_original, gender, birthday, bust, height, description, image_url, role, voice_actor, traits) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT(id) DO UPDATE SET \
               vndb_id = excluded.vndb_id, \
               name = excluded.name, \
               name_original = excluded.name_original, \
               gender = excluded.gender, \
               birthday = excluded.birthday, \
               bust = excluded.bust, \
               height = excluded.height, \
               description = excluded.description, \
               image_url = excluded.image_url, \
               role = excluded.role, \
               voice_actor = excluded.voice_actor, \
               traits = excluded.traits",
        )
        .bind(&character.id)
        .bind(&character.vndb_id)
        .bind(&character.name)
        .bind(&character.name_original)
        .bind(&character.gender)
        .bind(&character.birthday)
        .bind(&character.bust)
        .bind(character.height)
        .bind(&character.description)
        .bind(&character.image_url)
        .bind(&character.role)
        .bind(&character.voice_actor)
        .bind(&character.traits_json)
        .execute(&mut *tx)
        .await?;
    }

    for link in character_links {
        sqlx::query(
            "INSERT INTO work_characters (work_id, character_id, role) VALUES (?1, ?2, ?3) \
             ON CONFLICT(work_id, character_id) DO UPDATE SET role = excluded.role",
        )
        .bind(work_id)
        .bind(&link.character_id)
        .bind(&link.role)
        .execute(&mut *tx)
        .await?;
    }

    for credit in credits {
        sqlx::query(
            "INSERT INTO work_credits (work_id, person_id, role, character_name, notes) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(work_id, person_id, role) DO UPDATE SET \
               character_name = excluded.character_name, \
               notes = excluded.notes",
        )
        .bind(work_id)
        .bind(&credit.person_id)
        .bind(&credit.role)
        .bind(&credit.character_name)
        .bind(&credit.notes)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
