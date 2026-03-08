use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::models::AppJobRow;
use crate::domain::error::AppResult;

pub async fn enqueue_job(
    pool: &SqlitePool,
    kind: &str,
    title: &str,
    payload: Option<&Value>,
    dedup_key: Option<&str>,
    can_pause: bool,
    can_resume: bool,
    can_cancel: bool,
) -> AppResult<i64> {
    let payload_json = payload.map(serde_json::to_string).transpose()?;
    let inserted = sqlx::query_as::<_, (i64,)>(
        r#"
        INSERT INTO app_jobs (kind, title, payload, dedup_key, can_pause, can_resume, can_cancel)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        RETURNING id
        "#,
    )
    .bind(kind)
    .bind(title)
    .bind(payload_json)
    .bind(dedup_key)
    .bind(if can_pause { 1_i64 } else { 0 })
    .bind(if can_resume { 1_i64 } else { 0 })
    .bind(if can_cancel { 1_i64 } else { 0 })
    .fetch_one(pool)
    .await;

    match inserted {
        Ok((id,)) => Ok(id),
        Err(sqlx::Error::Database(db_err))
            if db_err.message().contains("UNIQUE constraint failed")
                || db_err.message().contains("idx_app_jobs_dedup") =>
        {
            let row: (i64,) = sqlx::query_as(
                "SELECT id FROM app_jobs
                 WHERE dedup_key = ?1 AND state IN ('queued', 'running', 'paused')
                 ORDER BY id DESC
                 LIMIT 1",
            )
            .bind(dedup_key.unwrap_or_default())
            .fetch_one(pool)
            .await?;
            Ok(row.0)
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn claim_next_job(pool: &SqlitePool) -> AppResult<Option<AppJobRow>> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: Option<AppJobRow> = sqlx::query_as(
        r#"
        UPDATE app_jobs
        SET state = 'running',
            started_at = COALESCE(started_at, ?1),
            updated_at = ?1
        WHERE id = (
            SELECT id
            FROM app_jobs
            WHERE state = 'queued'
            ORDER BY id ASC
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn recover_interrupted_jobs(pool: &SqlitePool) -> AppResult<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE app_jobs
         SET state = 'queued',
             current_step = COALESCE(current_step, 'Recovered after app restart'),
             updated_at = ?1
         WHERE state = 'running'",
    )
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_jobs(pool: &SqlitePool, limit: i64) -> AppResult<Vec<AppJobRow>> {
    let rows = sqlx::query_as::<_, AppJobRow>(
        "SELECT * FROM app_jobs ORDER BY id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_job(pool: &SqlitePool, job_id: i64) -> AppResult<Option<AppJobRow>> {
    let row = sqlx::query_as::<_, AppJobRow>("SELECT * FROM app_jobs WHERE id = ?1")
        .bind(job_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn update_progress(
    pool: &SqlitePool,
    job_id: i64,
    progress_pct: f64,
    current_step: Option<&str>,
    checkpoint: Option<&Value>,
) -> AppResult<()> {
    let checkpoint_json = checkpoint.map(serde_json::to_string).transpose()?;
    sqlx::query(
        "UPDATE app_jobs
         SET progress_pct = ?1,
             current_step = COALESCE(?2, current_step),
             checkpoint_json = COALESCE(?3, checkpoint_json),
             updated_at = ?4
         WHERE id = ?5",
    )
    .bind(progress_pct)
    .bind(current_step)
    .bind(checkpoint_json)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete_job(pool: &SqlitePool, job_id: i64, result: Option<&Value>) -> AppResult<()> {
    let result_json = result.map(serde_json::to_string).transpose()?;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE app_jobs
         SET state = 'completed',
             progress_pct = 100,
             result_json = ?1,
             finished_at = ?2,
             updated_at = ?2
         WHERE id = ?3",
    )
    .bind(result_json)
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_job(pool: &SqlitePool, job_id: i64, message: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE app_jobs
         SET state = 'failed',
             last_error = ?1,
             finished_at = ?2,
             updated_at = ?2
         WHERE id = ?3",
    )
    .bind(message)
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn pause_job(pool: &SqlitePool, job_id: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE app_jobs
         SET state = 'paused',
             updated_at = ?1
         WHERE id = ?2 AND state IN ('queued', 'running')",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn resume_job(pool: &SqlitePool, job_id: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE app_jobs
         SET state = 'queued',
             current_step = COALESCE(current_step, 'Resumed'),
             updated_at = ?1
         WHERE id = ?2 AND state = 'paused'",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn cancel_job(pool: &SqlitePool, job_id: i64) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE app_jobs
         SET state = 'cancelled',
             finished_at = ?1,
             updated_at = ?1
         WHERE id = ?2 AND state IN ('queued', 'running', 'paused')",
    )
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_runtime_flag(pool: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO app_runtime_flags (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
         value = excluded.value,
         updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_runtime_flag(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_runtime_flags WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|tuple| tuple.0))
}
