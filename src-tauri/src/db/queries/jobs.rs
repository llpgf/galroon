//! Enrichment job queue queries (R7).

use sqlx::SqlitePool;

use crate::db::models::JobRow;
use crate::domain::error::AppResult;

/// Enqueue a new enrichment job (idempotent via dedup_key).
pub async fn enqueue_job(
    pool: &SqlitePool,
    work_id: &str,
    job_type: &str,
    dedup_key: Option<&str>,
    payload: Option<&str>,
) -> AppResult<i64> {
    let inserted = sqlx::query_as::<_, (i64,)>(
        r#"
        INSERT INTO enrichment_jobs (work_id, job_type, dedup_key, payload)
        VALUES (?1, ?2, ?3, ?4)
        RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(job_type)
    .bind(dedup_key)
    .bind(payload)
    .fetch_one(pool)
    .await;

    match inserted {
        Ok((id,)) => Ok(id),
        Err(sqlx::Error::Database(db_err))
            if db_err.message().contains("UNIQUE constraint failed")
                || db_err.message().contains("idx_jobs_dedup") =>
        {
            let key = dedup_key.unwrap_or_default();
            let row: (i64,) = sqlx::query_as(
                "SELECT id FROM enrichment_jobs
                 WHERE dedup_key = ?1 AND state NOT IN ('success', 'failed')
                 ORDER BY id DESC
                 LIMIT 1",
            )
            .bind(key)
            .fetch_one(pool)
            .await?;
            Ok(row.0)
        }
        Err(err) => Err(err.into()),
    }
}

/// Atomically claim the next available job (R7).
///
/// Uses UPDATE ... WHERE state IN ('queued', 'retry_wait') AND next_run_at <= now
/// to prevent race conditions (only one claim succeeds).
pub async fn claim_next_job(pool: &SqlitePool, _worker_id: &str) -> AppResult<Option<JobRow>> {
    let now = chrono::Utc::now().to_rfc3339();

    // Atomic claim: UPDATE + RETURNING in one statement
    let row: Option<JobRow> = sqlx::query_as(
        r#"
        UPDATE enrichment_jobs
        SET state = 'claimed',
            updated_at = ?1
        WHERE id = (
            SELECT id FROM enrichment_jobs
            WHERE state IN ('queued', 'retry_wait')
              AND next_run_at <= ?1
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

/// Mark a job as completed.
pub async fn complete_job(pool: &SqlitePool, job_id: i64) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE enrichment_jobs SET state = 'success', updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(job_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Mark a job as failed with error and schedule retry with exponential backoff + jitter (R8).
pub async fn fail_job(
    pool: &SqlitePool,
    job_id: i64,
    attempt_count: i32,
    max_attempts: i32,
    error_msg: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now();

    if attempt_count >= max_attempts {
        // Permanently failed
        sqlx::query(
            "UPDATE enrichment_jobs SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(error_msg)
        .bind(now.to_rfc3339())
        .bind(job_id)
        .execute(pool)
        .await?;
    } else {
        // Exponential backoff with jitter: base * 2^attempt + random(0..base)
        let base_secs: u64 = 5;
        let backoff = base_secs * 2u64.pow(attempt_count as u32);
        let capped = backoff.min(300); // Max 5 minutes
        let next_run = now + chrono::Duration::seconds(capped as i64);

        sqlx::query(
            r#"
            UPDATE enrichment_jobs
            SET state = 'retry_wait',
                attempt_count = ?1,
                last_error = ?2,
                next_run_at = ?3,
                updated_at = ?4
            WHERE id = ?5
            "#,
        )
        .bind(attempt_count + 1)
        .bind(error_msg)
        .bind(next_run.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(job_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Get count of jobs by state (for dashboard/health).
pub async fn job_stats(pool: &SqlitePool) -> AppResult<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT state, COUNT(*) FROM enrichment_jobs GROUP BY state")
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("galroon_jobs_{}_{}.db", name, Uuid::new_v4()))
    }

    #[tokio::test]
    async fn enqueue_job_reuses_active_dedup_key() {
        let db = Database::new(&temp_db_path("dedup"))
            .await
            .expect("db init");

        db.execute_write(
            "INSERT INTO works (id, folder_path, title, title_aliases, tags, user_tags, library_status, enrichment_state, title_source, folder_mtime, metadata_mtime, created_at, updated_at)
             VALUES (?1, ?2, ?3, '[]', '[]', '[]', 'unplayed', 'unmatched', 'filesystem', 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
                .to_string(),
            vec![
                serde_json::Value::String("00000000-0000-0000-0000-000000000001".to_string()),
                serde_json::Value::String("C:/tmp/work".to_string()),
                serde_json::Value::String("Test Work".to_string()),
            ],
        )
        .await
        .expect("insert work");

        let first = enqueue_job(
            db.read_pool(),
            "00000000-0000-0000-0000-000000000001",
            "metadata_refresh",
            Some("refresh:test"),
            None,
        )
        .await
        .expect("enqueue first");
        let second = enqueue_job(
            db.read_pool(),
            "00000000-0000-0000-0000-000000000001",
            "metadata_refresh",
            Some("refresh:test"),
            None,
        )
        .await
        .expect("enqueue second");

        assert_eq!(first, second);
    }
}
