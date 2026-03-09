use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use sqlx::{query, query_as, FromRow, Pool, Sqlite};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyRecord {
    pub id: i64,
    pub user_id: String,
    pub role: String,
    pub key_hash: String,
}

#[derive(Debug, FromRow)]
pub struct ApiKeyListItem {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

pub fn generate_raw_api_key(key_lookup: &str) -> String {
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    format!("tk_{}.{}", key_lookup, secret)
}

pub fn parse_lookup_from_raw_key(raw_key: &str) -> Option<&str> {
    let rest = raw_key.strip_prefix("tk_")?;
    let (lookup, secret) = rest.split_once('.')?;
    if lookup.is_empty() || secret.is_empty() {
        return None;
    }
    Some(lookup)
}

pub async fn create_api_key(
    db: &Pool<Sqlite>,
    user_id: &str,
    name: &str,
    role: &str,
) -> Result<(i64, String), sqlx::Error> {
    let lookup = Uuid::new_v4().simple().to_string();
    let raw_key = generate_raw_api_key(&lookup);
    let key_hash =
        hash(&raw_key, DEFAULT_COST).map_err(|err| sqlx::Error::Protocol(err.to_string()))?;

    let result = query(
        r#"
        INSERT INTO api_keys (user_id, name, key_lookup, key_hash, user_role)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(&lookup)
    .bind(&key_hash)
    .bind(role)
    .execute(db)
    .await?;

    Ok((result.last_insert_rowid(), raw_key))
}

pub async fn find_active_api_key_by_raw_key(
    db: &Pool<Sqlite>,
    raw_key: &str,
) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
    let Some(lookup) = parse_lookup_from_raw_key(raw_key) else {
        return Ok(None);
    };

    let record = query_as::<_, ApiKeyRecord>(
        r#"
        SELECT api_key_id AS id, user_id, user_role AS role, key_hash
        FROM api_keys
        WHERE key_lookup = ? AND revoked_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(lookup)
    .fetch_optional(db)
    .await?;

    let Some(record) = record else {
        return Ok(None);
    };

    let matches =
        verify(raw_key, &record.key_hash).map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
    if matches {
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

pub async fn touch_api_key_last_used(
    db: &Pool<Sqlite>,
    api_key_id: i64,
) -> Result<(), sqlx::Error> {
    query(
        r#"
        UPDATE api_keys
        SET last_used_at = ?
        WHERE api_key_id = ?
        "#,
    )
    .bind(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(api_key_id)
    .execute(db)
    .await?;

    Ok(())
}
