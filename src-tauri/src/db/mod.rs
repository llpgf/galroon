//! Database module — SQLite with WAL mode, DbWriter actor, and read pool.
//!
//! Architecture (R1):
//! - ALL writes go through the DbWriter actor (single-writer, serialized via tokio mpsc)
//! - Reads use a separate connection pool (multi-reader)
//! - WAL mode + busy_timeout on every connection
//! - This eliminates SQLITE_BUSY under concurrent enrichment + UI edits

pub mod models;
pub mod queries;

use serde_json::Value;
use sqlx::query::Query;
use sqlx::sqlite::{
    SqliteArguments, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, Sqlite, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::domain::error::{AppError, AppResult};

/// A write operation sent to the DbWriter actor.
type _WriteOp = Box<
    dyn FnOnce(&SqlitePool) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>
        + Send,
>;

/// The database handle shared across the application.
///
/// Contains a read pool for concurrent reads and a write channel
/// for serialized writes through the DbWriter actor.
#[derive(Clone)]
pub struct Database {
    /// Multi-connection pool for concurrent reads
    read_pool: SqlitePool,
    /// Channel to send write operations to the DbWriter actor
    write_tx: mpsc::Sender<WriteRequest>,
}

/// A write request sent to the DbWriter actor.
struct WriteRequest {
    /// SQL statement(s) to execute
    sql: String,
    /// Parameters as JSON-encoded values
    params: Vec<Value>,
    /// Response channel
    reply: oneshot::Sender<AppResult<u64>>,
}

impl Database {
    /// Create a new Database with WAL mode, DbWriter actor, and read pool.
    pub async fn new(db_path: &Path) -> AppResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db_url = format!("sqlite:{}", db_path.display());

        // Connection options: WAL mode + busy_timeout (R1)
        let connect_options = SqliteConnectOptions::from_str(&db_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .create_if_missing(true);

        // Read pool: multiple connections for concurrent reads
        let read_pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(connect_options.clone())
            .await?;

        // Write pool: single connection for serialized writes
        let write_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await?;

        // Run migrations
        Self::run_migrations(&write_pool).await?;

        // Start the DbWriter actor
        let (write_tx, write_rx) = mpsc::channel::<WriteRequest>(256);
        tokio::spawn(Self::db_writer_loop(write_pool, write_rx));

        info!("Database initialized with WAL mode + DbWriter actor");

        Ok(Self {
            read_pool,
            write_tx,
        })
    }

    /// The DbWriter actor loop — serializes all writes through a single task (R1).
    async fn db_writer_loop(pool: SqlitePool, mut rx: mpsc::Receiver<WriteRequest>) {
        while let Some(req) = rx.recv().await {
            let prepared = req
                .params
                .iter()
                .try_fold(sqlx::query(&req.sql), |query, param| {
                    bind_json_param(query, param)
                });

            let reply_result = match prepared {
                Ok(query) => match query.execute(&pool).await {
                    Ok(r) => Ok(r.rows_affected()),
                    Err(e) => Err(AppError::Database(e)),
                },
                Err(e) => Err(e),
            };

            if req.reply.send(reply_result).is_err() {
                warn!("DbWriter: caller dropped before receiving response");
            }
        }
        info!("DbWriter actor stopped");
    }

    /// Run database migrations.
    async fn run_migrations(pool: &SqlitePool) -> AppResult<()> {
        sqlx::query(include_str!("../../migrations/001_works.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/002_tags.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/003_characters.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/004_jobs.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/005_fts.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/006_assets.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/007_work_texts.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/008_dual_tags.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/009_collections.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/010_completion.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/011_enrichment_mappings.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/012_canonical_works.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/013_work_persistence.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!("../../migrations/014_field_preferences.sql"))
            .execute(pool)
            .await?;
        sqlx::query(include_str!(
            "../../migrations/015_workshop_diagnostics.sql"
        ))
        .execute(pool)
        .await?;
        sqlx::query(include_str!(
            "../../migrations/016_provider_rules_and_asset_groups.sql"
        ))
        .execute(pool)
        .await?;
        sqlx::query(include_str!("../../migrations/017_app_jobs.sql"))
            .execute(pool)
            .await?;

        Self::ensure_works_compat(pool).await?;
        Self::ensure_canonical_works_compat(pool).await?;

        info!("Database migrations complete");
        Ok(())
    }

    async fn ensure_works_compat(pool: &SqlitePool) -> AppResult<()> {
        let columns = sqlx::query("PRAGMA table_info(works)")
            .fetch_all(pool)
            .await?;
        let has_dlsite_id = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "dlsite_id");
        let has_field_sources = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "field_sources");
        let has_field_preferences = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "field_preferences");
        let has_user_overrides = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "user_overrides");
        let has_content_signature = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "content_signature");

        if !has_dlsite_id {
            sqlx::query("ALTER TABLE works ADD COLUMN dlsite_id TEXT")
                .execute(pool)
                .await?;
        }
        if !has_field_sources {
            sqlx::query("ALTER TABLE works ADD COLUMN field_sources TEXT")
                .execute(pool)
                .await?;
        }
        if !has_field_preferences {
            sqlx::query("ALTER TABLE works ADD COLUMN field_preferences TEXT")
                .execute(pool)
                .await?;
        }
        if !has_user_overrides {
            sqlx::query("ALTER TABLE works ADD COLUMN user_overrides TEXT")
                .execute(pool)
                .await?;
        }
        if !has_content_signature {
            sqlx::query("ALTER TABLE works ADD COLUMN content_signature TEXT")
                .execute(pool)
                .await?;
        }

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_works_dlsite_id ON works(dlsite_id)")
            .execute(pool)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_works_content_signature ON works(content_signature)",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn ensure_canonical_works_compat(pool: &SqlitePool) -> AppResult<()> {
        let columns = sqlx::query("PRAGMA table_info(canonical_works)")
            .fetch_all(pool)
            .await?;
        let has_asset_count = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "asset_count");
        let has_asset_types = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "asset_types");
        let has_primary_asset_type = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "primary_asset_type");

        if !has_asset_count {
            sqlx::query(
                "ALTER TABLE canonical_works ADD COLUMN asset_count INTEGER NOT NULL DEFAULT 0",
            )
            .execute(pool)
            .await?;
        }

        if !has_asset_types {
            sqlx::query("ALTER TABLE canonical_works ADD COLUMN asset_types TEXT")
                .execute(pool)
                .await?;
        }

        if !has_primary_asset_type {
            sqlx::query("ALTER TABLE canonical_works ADD COLUMN primary_asset_type TEXT")
                .execute(pool)
                .await?;
        }

        let asset_group_columns = sqlx::query("PRAGMA table_info(canonical_asset_groups)")
            .fetch_all(pool)
            .await?;
        let has_relation_role = asset_group_columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "relation_role");
        let has_parent_asset_type = asset_group_columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "parent_asset_type");

        if !has_relation_role {
            sqlx::query(
                "ALTER TABLE canonical_asset_groups ADD COLUMN relation_role TEXT NOT NULL DEFAULT 'supplemental'",
            )
            .execute(pool)
            .await?;
        }

        if !has_parent_asset_type {
            sqlx::query("ALTER TABLE canonical_asset_groups ADD COLUMN parent_asset_type TEXT")
                .execute(pool)
                .await?;
        }

        Ok(())
    }

    /// Get a reference to the read pool for queries.
    pub fn read_pool(&self) -> &SqlitePool {
        &self.read_pool
    }

    /// Execute a write operation through the DbWriter actor.
    ///
    /// Returns the number of rows affected.
    pub async fn execute_write(&self, sql: String, params: Vec<Value>) -> AppResult<u64> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let request = WriteRequest {
            sql,
            params,
            reply: reply_tx,
        };

        self.write_tx
            .send(request)
            .await
            .map_err(|_| AppError::DbWriterClosed)?;

        reply_rx.await.map_err(|_| AppError::DbWriterClosed)?
    }

    /// Get the current database version (for health checks).
    pub async fn version(&self) -> AppResult<String> {
        let row = sqlx::query("SELECT sqlite_version()")
            .fetch_one(&self.read_pool)
            .await?;
        let version: String = row.get(0);
        Ok(version)
    }
}

fn bind_json_param<'q>(
    query: Query<'q, Sqlite, SqliteArguments<'q>>,
    value: &'q Value,
) -> AppResult<Query<'q, Sqlite, SqliteArguments<'q>>> {
    Ok(match value {
        Value::Null => query.bind(Option::<String>::None),
        Value::Bool(v) => query.bind(if *v { 1_i64 } else { 0_i64 }),
        Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                query.bind(i)
            } else if let Some(u) = v.as_u64() {
                if let Ok(i) = i64::try_from(u) {
                    query.bind(i)
                } else {
                    query.bind(u as f64)
                }
            } else if let Some(f) = v.as_f64() {
                query.bind(f)
            } else {
                return Err(AppError::Validation(
                    "Unsupported JSON number parameter".to_string(),
                ));
            }
        }
        Value::String(v) => query.bind(v),
        Value::Array(_) | Value::Object(_) => query.bind(serde_json::to_string(value)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("galroon_{}_{}.db", name, uuid::Uuid::new_v4()))
    }

    #[tokio::test]
    async fn execute_write_binds_scalars_and_nulls() {
        let db_path = temp_db_path("db_writer_params");
        let db = Database::new(&db_path).await.expect("db init");

        db.execute_write(
            "CREATE TABLE IF NOT EXISTS param_test (id TEXT PRIMARY KEY, text_value TEXT, num_value REAL, bool_value INTEGER, optional_value TEXT)".to_string(),
            vec![],
        )
        .await
        .expect("create param_test");

        db.execute_write(
            "INSERT INTO param_test (id, text_value, num_value, bool_value, optional_value) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
            vec![
                json!("row-1"),
                json!("O'Brien"),
                json!(42.5),
                json!(true),
                Value::Null,
            ],
        )
        .await
        .expect("insert bound row");

        let row = sqlx::query(
            "SELECT text_value, num_value, bool_value, optional_value FROM param_test WHERE id = ?",
        )
        .bind("row-1")
        .fetch_one(db.read_pool())
        .await
        .expect("fetch row");

        let text_value: String = row.get("text_value");
        let num_value: f64 = row.get("num_value");
        let bool_value: i64 = row.get("bool_value");
        let optional_value: Option<String> = row.get("optional_value");

        assert_eq!(text_value, "O'Brien");
        assert!((num_value - 42.5).abs() < f64::EPSILON);
        assert_eq!(bool_value, 1);
        assert_eq!(optional_value, None);
    }
}
