//! Asset queries — persist classified folder contents.

use sqlx::SqlitePool;

use crate::domain::asset::AssetEntry;
use crate::domain::error::AppResult;

pub async fn replace_assets_for_work(
    pool: &SqlitePool,
    work_id: &str,
    assets: &[AssetEntry],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM assets WHERE work_id = ?")
        .bind(work_id)
        .execute(&mut *tx)
        .await?;

    for asset in assets {
        sqlx::query(
            r#"
            INSERT INTO assets (id, work_id, path, filename, asset_type, size_bytes, is_dir)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(work_id)
        .bind(asset.path.to_string_lossy().to_string())
        .bind(&asset.filename)
        .bind(
            serde_json::to_string(&asset.asset_type)
                .unwrap_or_default()
                .trim_matches('"'),
        )
        .bind(i64::try_from(asset.size_bytes).unwrap_or(i64::MAX))
        .bind(if asset.is_dir { 1_i64 } else { 0_i64 })
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
