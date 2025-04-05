use poem::{error::InternalServerError, Result};
use sqlx::SqlitePool;

pub async fn init_db(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            content TEXT
        )",
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    Ok(())
}
