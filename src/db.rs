use poem::{error::InternalServerError, Result};
use sqlx::{query, query_scalar, Row, SqlitePool};

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

    migrate_budget_config(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_spending_records(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_api_keys(pool).await.map_err(InternalServerError)?;

    Ok(())
}

async fn migrate_budget_config(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "budget_config").await? {
        create_budget_config_table(pool).await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS budget_config_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE budget_config_new (
            config_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            week_key TEXT NOT NULL,
            weekly_limit INTEGER NOT NULL DEFAULT 500000,
            alert_threshold REAL NOT NULL DEFAULT 0.85,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(owner_user_id, week_key)
        )
        "#,
    )
    .execute(pool)
    .await?;

    if column_exists(pool, "budget_config", "owner_user_id").await? {
        query(
            r#"
            INSERT INTO budget_config_new
                (config_id, owner_user_id, week_key, weekly_limit, alert_threshold, created_at)
            SELECT
                config_id,
                COALESCE(owner_user_id, 'admin'),
                week_key,
                weekly_limit,
                alert_threshold,
                created_at
            FROM budget_config
            "#,
        )
        .execute(pool)
        .await?;
    } else {
        query(
            r#"
            INSERT INTO budget_config_new
                (config_id, owner_user_id, week_key, weekly_limit, alert_threshold, created_at)
            SELECT
                config_id,
                'admin',
                week_key,
                weekly_limit,
                alert_threshold,
                created_at
            FROM budget_config
            "#,
        )
        .execute(pool)
        .await?;
    }

    query("DROP TABLE budget_config").execute(pool).await?;
    query("ALTER TABLE budget_config_new RENAME TO budget_config")
        .execute(pool)
        .await?;

    Ok(())
}

async fn migrate_spending_records(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "spending_records").await? {
        create_spending_records_table(pool).await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS spending_records_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE spending_records_new (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            week_key TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    if column_exists(pool, "spending_records", "owner_user_id").await? {
        query(
            r#"
            INSERT INTO spending_records_new
                (record_id, owner_user_id, amount, merchant, transacted_at, week_key, created_at)
            SELECT
                record_id,
                COALESCE(owner_user_id, 'admin'),
                amount,
                merchant,
                transacted_at,
                week_key,
                created_at
            FROM spending_records
            "#,
        )
        .execute(pool)
        .await?;
    } else {
        query(
            r#"
            INSERT INTO spending_records_new
                (record_id, owner_user_id, amount, merchant, transacted_at, week_key, created_at)
            SELECT
                record_id,
                'admin',
                amount,
                merchant,
                transacted_at,
                week_key,
                created_at
            FROM spending_records
            "#,
        )
        .execute(pool)
        .await?;
    }

    query("DROP TABLE spending_records").execute(pool).await?;
    query("ALTER TABLE spending_records_new RENAME TO spending_records")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_spending_records_owner_week
        ON spending_records(owner_user_id, week_key)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_budget_config_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS budget_config (
            config_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            week_key TEXT NOT NULL,
            weekly_limit INTEGER NOT NULL DEFAULT 500000,
            alert_threshold REAL NOT NULL DEFAULT 0.85,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(owner_user_id, week_key)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_spending_records_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS spending_records (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            week_key TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_spending_records_owner_week
        ON spending_records(owner_user_id, week_key)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_api_keys(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "api_keys").await? {
        create_api_keys_table(pool).await?;
        return Ok(());
    }

    let has_key_lookup = column_exists(pool, "api_keys", "key_lookup").await?;
    let has_user_role = column_exists(pool, "api_keys", "user_role").await?;

    if has_key_lookup && has_user_role {
        query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
            ON api_keys(key_lookup)
            "#,
        )
        .execute(pool)
        .await?;
        query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
            ON api_keys(user_id)
            "#,
        )
        .execute(pool)
        .await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS api_keys_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE api_keys_new (
            api_key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            key_lookup TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            user_role TEXT NOT NULL DEFAULT 'user',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    if has_key_lookup && !has_user_role {
        query(
            r#"
            INSERT INTO api_keys_new
                (api_key_id, user_id, name, key_lookup, key_hash, user_role, created_at, last_used_at, revoked_at)
            SELECT
                api_key_id,
                user_id,
                name,
                key_lookup,
                key_hash,
                'user',
                COALESCE(created_at, CURRENT_TIMESTAMP),
                last_used_at,
                revoked_at
            FROM api_keys
            "#,
        )
        .execute(pool)
        .await?;
    } else {
        query(
            r#"
            INSERT INTO api_keys_new
                (api_key_id, user_id, name, key_lookup, key_hash, user_role, created_at, last_used_at, revoked_at)
            SELECT
                api_key_id,
                user_id,
                name,
                lower(hex(randomblob(16))),
                key_hash,
                'user',
                COALESCE(created_at, CURRENT_TIMESTAMP),
                last_used_at,
                revoked_at
            FROM api_keys
            "#,
        )
        .execute(pool)
        .await?;
    }

    query("DROP TABLE api_keys").execute(pool).await?;
    query("ALTER TABLE api_keys_new RENAME TO api_keys")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
        ON api_keys(key_lookup)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
        ON api_keys(user_id)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_api_keys_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            api_key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            key_lookup TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            user_role TEXT NOT NULL DEFAULT 'user',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
        ON api_keys(key_lookup)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
        ON api_keys(user_id)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn table_exists(
    pool: &SqlitePool,
    table_name: &str,
) -> std::result::Result<bool, sqlx::Error> {
    let exists = query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn column_exists(
    pool: &SqlitePool,
    table_name: &str,
    column_name: &str,
) -> std::result::Result<bool, sqlx::Error> {
    let rows = query(&format!("PRAGMA table_info({})", table_name))
        .fetch_all(pool)
        .await?;

    Ok(rows
        .iter()
        .any(|row| row.get::<String, _>("name") == column_name))
}
