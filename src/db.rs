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
              password TEXT,
              user_role TEXT NOT NULL,
              auth_provider TEXT NOT NULL DEFAULT 'local',
              google_sub TEXT
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

    migrate_budget_periods(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_spending_records(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_users(pool).await.map_err(InternalServerError)?;
    migrate_user_matches(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_match_messages(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_api_keys(pool).await.map_err(InternalServerError)?;
    migrate_rss_push(pool).await.map_err(InternalServerError)?;

    Ok(())
}

async fn migrate_budget_periods(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "budget_periods").await? {
        create_budget_periods_table(pool).await?;
        return Ok(());
    }

    if !column_exists(pool, "budget_periods", "snapshot_total_spent").await? {
        ensure_budget_period_indexes(pool).await?;
        return Ok(());
    }

    if table_exists(pool, "budget_periods_new").await? {
        query("DROP TABLE budget_periods_new").execute(pool).await?;
    }

    if column_exists(pool, "budget_periods", "snapshot_total_spent").await? {
        query(
            r#"
            CREATE TABLE budget_periods_new (
                budget_id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner_user_id TEXT NOT NULL,
                total_budget INTEGER NOT NULL,
                from_date DATE NOT NULL,
                to_date DATE NOT NULL,
                alert_threshold REAL NOT NULL DEFAULT 0.85,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;
        query(
            r#"
            INSERT INTO budget_periods_new (
                budget_id,
                owner_user_id,
                total_budget,
                from_date,
                to_date,
                alert_threshold,
                created_at,
                updated_at
            )
            SELECT
                budget_id,
                owner_user_id,
                total_budget,
                from_date,
                to_date,
                alert_threshold,
                created_at,
                updated_at
            FROM budget_periods
            "#,
        )
        .execute(pool)
        .await?;
        query("DROP TABLE budget_periods").execute(pool).await?;
        query("ALTER TABLE budget_periods_new RENAME TO budget_periods")
            .execute(pool)
            .await?;
    }

    ensure_budget_period_indexes(pool).await?;

    Ok(())
}

async fn migrate_spending_records(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "spending_records").await? {
        create_spending_records_table(pool).await?;
        return Ok(());
    }

    let has_owner_user_id = column_exists(pool, "spending_records", "owner_user_id").await?;
    let has_source_type = column_exists(pool, "spending_records", "source_type").await?;
    let has_source_fingerprint =
        column_exists(pool, "spending_records", "source_fingerprint").await?;
    let has_week_key = column_exists(pool, "spending_records", "week_key").await?;

    if has_owner_user_id && has_source_type && has_source_fingerprint && !has_week_key {
        ensure_spending_record_indexes(pool).await?;
        return Ok(());
    }

    if table_exists(pool, "spending_records_new").await? {
        query("DROP TABLE spending_records_new")
            .execute(pool)
            .await?;
    }
    query(
        r#"
        CREATE TABLE spending_records_new (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            source_type TEXT,
            source_fingerprint TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    if has_owner_user_id {
        let copy_sql = match (has_source_type, has_source_fingerprint) {
            (true, true) => {
                r#"
                INSERT INTO spending_records_new
                    (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
                SELECT
                    record_id,
                    COALESCE(owner_user_id, 'admin'),
                    amount,
                    merchant,
                    transacted_at,
                    source_type,
                    source_fingerprint,
                    created_at
                FROM spending_records
                "#
            }
            _ => {
                r#"
                INSERT INTO spending_records_new
                    (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
                SELECT
                    record_id,
                    COALESCE(owner_user_id, 'admin'),
                    amount,
                    merchant,
                    transacted_at,
                    NULL,
                    NULL,
                    created_at
                FROM spending_records
                "#
            }
        };
        query(copy_sql).execute(pool).await?;
    } else {
        query(
            r#"
            INSERT INTO spending_records_new
                (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
            SELECT
                record_id,
                'admin',
                amount,
                merchant,
                transacted_at,
                NULL,
                NULL,
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
    ensure_spending_record_indexes(pool).await?;

    Ok(())
}

async fn create_budget_periods_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS budget_periods (
            budget_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            total_budget INTEGER NOT NULL,
            from_date DATE NOT NULL,
            to_date DATE NOT NULL,
            alert_threshold REAL NOT NULL DEFAULT 0.85,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_budget_period_indexes(pool).await?;

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
            source_type TEXT,
            source_fingerprint TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_spending_record_indexes(pool).await?;

    Ok(())
}

async fn ensure_budget_period_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_budget_periods_owner_updated_at
        ON budget_periods(owner_user_id, updated_at DESC, budget_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_spending_record_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_spending_records_owner_transacted_at
        ON spending_records(owner_user_id, transacted_at)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_spending_records_import_fingerprint
        ON spending_records(owner_user_id, source_type, source_fingerprint)
        WHERE source_type IS NOT NULL AND source_fingerprint IS NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_users(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "users").await? {
        create_users_table(pool).await?;
        return Ok(());
    }

    let has_auth_provider = column_exists(pool, "users", "auth_provider").await?;
    let has_google_sub = column_exists(pool, "users", "google_sub").await?;

    if has_auth_provider && has_google_sub {
        ensure_user_indexes(pool).await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS users_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE users_new (
            user_id TEXT PRIMARY KEY,
            password TEXT,
            user_role TEXT NOT NULL,
            auth_provider TEXT NOT NULL DEFAULT 'local',
            google_sub TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        INSERT INTO users_new (user_id, password, user_role, auth_provider, google_sub)
        SELECT user_id, password, user_role, 'local', NULL
        FROM users
        "#,
    )
    .execute(pool)
    .await?;

    query("DROP TABLE users").execute(pool).await?;
    query("ALTER TABLE users_new RENAME TO users")
        .execute(pool)
        .await?;
    ensure_user_indexes(pool).await?;

    Ok(())
}

async fn create_users_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id TEXT PRIMARY KEY,
            password TEXT,
            user_role TEXT NOT NULL,
            auth_provider TEXT NOT NULL DEFAULT 'local',
            google_sub TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;
    ensure_user_indexes(pool).await?;

    Ok(())
}

async fn ensure_user_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_google_sub
        ON users(google_sub)
        WHERE google_sub IS NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_user_matches(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS user_matches (
            match_id INTEGER PRIMARY KEY AUTOINCREMENT,
            requester_user_id TEXT NOT NULL,
            target_user_id TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            responded_at DATETIME,
            closed_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_matches_requester_status
        ON user_matches(requester_user_id, status, created_at DESC, match_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_matches_target_status
        ON user_matches(target_user_id, status, created_at DESC, match_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_match_messages(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS match_messages (
            message_id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            sender_user_id TEXT NOT NULL,
            receiver_user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES user_matches(match_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_match_messages_match_created_at
        ON match_messages(match_id, created_at ASC, message_id ASC)
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

async fn migrate_rss_push(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS rss_sources (
            source_id TEXT PRIMARY KEY,
            feed_url TEXT NOT NULL,
            normalized_feed_url TEXT NOT NULL UNIQUE,
            title TEXT,
            site_url TEXT,
            etag TEXT,
            last_modified TEXT,
            last_polled_at DATETIME,
            last_success_at DATETIME,
            last_error TEXT,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            is_active BOOLEAN NOT NULL DEFAULT true,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_rss_sources_active
        ON rss_sources(is_active, updated_at DESC)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS rss_feed_items (
            item_id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id TEXT NOT NULL,
            item_guid_hash TEXT NOT NULL,
            guid_or_link TEXT,
            title TEXT NOT NULL,
            link TEXT,
            published_at TEXT,
            detected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_id) REFERENCES rss_sources(source_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_rss_feed_items_source_guid_hash
        ON rss_feed_items(source_id, item_guid_hash)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS user_rss_subscriptions (
            subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_id) REFERENCES rss_sources(source_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_user_rss_subscriptions_user_source
        ON user_rss_subscriptions(user_id, source_id)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_rss_subscriptions_source
        ON user_rss_subscriptions(source_id)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS web_push_subscriptions (
            push_subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL UNIQUE,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            user_agent TEXT,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_success_at DATETIME,
            last_failure_at DATETIME,
            failure_count INTEGER NOT NULL DEFAULT 0,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_web_push_subscriptions_user_revoked
        ON web_push_subscriptions(user_id, revoked_at)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS push_delivery_logs (
            delivery_id INTEGER PRIMARY KEY AUTOINCREMENT,
            push_subscription_id INTEGER NOT NULL,
            item_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            error_message TEXT,
            sent_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (push_subscription_id) REFERENCES web_push_subscriptions(push_subscription_id),
            FOREIGN KEY (item_id) REFERENCES rss_feed_items(item_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_push_delivery_logs_subscription_item
        ON push_delivery_logs(push_subscription_id, item_id)
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
