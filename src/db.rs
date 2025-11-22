use poem::{error::InternalServerError, Result};
use sqlx::SqlitePool;

pub async fn init_db(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS posts (
            post_id TEXT PRIMARY KEY,
            title TEXT,
            description TEXT,
            published_at DATETIME,
            tags TEXT,
            content TEXT,
            writer_id TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            image_id TEXT PRIMARY KEY,
            post_id TEXT,
            file_name TEXT NOT NULL,
            origin_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            image_type TEXT NOT NULL,
            uploaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (post_id) REFERENCES posts(post_id)
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id TEXT PRIMARY KEY,
            password TEXT NOT NULL,
            user_role TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sections (
            section_id INTEGER PRIMARY KEY,
            section_type TEXT NOT NULL,
            content_data TEXT NOT NULL,
            order_index INTEGER NOT NULL,
            is_active BOOLEAN DEFAULT true,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS portfolio (
            portfolio_id INTEGER PRIMARY KEY,
            content TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    Ok(())
}
