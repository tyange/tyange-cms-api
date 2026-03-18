use std::{
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use feed_rs::model::Feed;
use poem::http::StatusCode;
use reqwest::{
    header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED},
    redirect::Policy,
    Client,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{query, query_as, query_scalar, FromRow, SqlitePool};
use tokio::time::interval;
use url::Url;
use uuid::Uuid;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushMessageBuilder,
};

use crate::models::{
    CreateRssSourceResponse, FeedItemResponse, FeedItemsQuery, FeedItemsResponse,
    FeedSummaryResponse, RssSourceResponse, WebPushSubscriptionResponse,
};

const MAX_FEED_BYTES: usize = 1_000_000;
const POLLING_INTERVAL_SECONDS: u64 = 300;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }
}

#[derive(Debug, FromRow)]
struct SourceRow {
    source_id: String,
    feed_url: String,
    normalized_feed_url: String,
    title: Option<String>,
    site_url: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    consecutive_failures: i64,
}

#[derive(Debug, FromRow)]
struct PushTargetRow {
    push_subscription_id: i64,
    endpoint: String,
    p256dh: String,
    auth: String,
}

#[derive(Debug)]
struct FeedFetchResult {
    feed_title: Option<String>,
    site_url: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    items: Vec<ParsedFeedItem>,
}

#[derive(Debug)]
struct ParsedFeedItem {
    item_guid_hash: String,
    guid_or_link: Option<String>,
    title: String,
    link: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug)]
struct InsertedFeedItem {
    item_id: i64,
    title: String,
    link: Option<String>,
}

#[derive(Debug, FromRow)]
struct FeedListRow {
    item_id: i64,
    source_id: String,
    source_title: Option<String>,
    title: String,
    item_url: Option<String>,
    published_at: Option<String>,
    detected_at: String,
}

#[derive(Debug)]
enum FetchOutcome {
    NotModified {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Parsed(FeedFetchResult),
}

#[derive(Debug)]
struct PushConfig {
    private_key: String,
    subject: String,
}

impl PushConfig {
    fn from_env() -> Option<Self> {
        let _public_key = env::var("VAPID_PUBLIC_KEY").ok()?;
        let private_key = env::var("VAPID_PRIVATE_KEY").ok()?;
        let subject = env::var("VAPID_SUBJECT").ok()?;

        Some(Self {
            private_key,
            subject,
        })
    }
}

pub fn push_public_key() -> Result<String, AppError> {
    env::var("VAPID_PUBLIC_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::service_unavailable(
                "푸시 알림이 아직 설정되지 않았습니다. VAPID_PUBLIC_KEY 환경변수를 확인해주세요.",
            )
        })
}

pub fn start_polling_worker(db: SqlitePool) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(POLLING_INTERVAL_SECONDS));

        loop {
            ticker.tick().await;
            if let Err(err) = process_active_sources_once(&db).await {
                eprintln!("rss polling failed: {}", err.message);
            }
        }
    });
}

pub async fn process_active_sources_once(db: &SqlitePool) -> Result<(), AppError> {
    let sources = query_as::<_, SourceRow>(
        r#"
        SELECT
            source_id,
            feed_url,
            normalized_feed_url,
            title,
            site_url,
            etag,
            last_modified,
            consecutive_failures
        FROM rss_sources
        WHERE is_active = 1
        ORDER BY updated_at DESC, source_id DESC
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS source 조회 실패: {}", err)))?;

    let push_config = PushConfig::from_env();
    for source in sources {
        if let Err(err) = process_single_source(db, &source, push_config.as_ref()).await {
            eprintln!(
                "rss source processing failed for {}: {}",
                source.normalized_feed_url, err.message
            );
        }
    }

    Ok(())
}

pub async fn list_user_rss_sources(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Vec<RssSourceResponse>, AppError> {
    let rows = query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            String,
        ),
    >(
        r#"
        SELECT
            s.source_id,
            s.feed_url,
            s.normalized_feed_url,
            s.title,
            s.site_url,
            s.last_polled_at,
            s.last_success_at,
            s.last_error,
            s.consecutive_failures,
            u.created_at
        FROM user_rss_subscriptions u
        INNER JOIN rss_sources s ON s.source_id = u.source_id
        WHERE u.user_id = ?
        ORDER BY u.created_at DESC, u.subscription_id DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS 구독 목록 조회 실패: {}", err)))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                source_id,
                feed_url,
                normalized_feed_url,
                title,
                site_url,
                last_polled_at,
                last_success_at,
                last_error,
                consecutive_failures,
                subscribed_at,
            )| RssSourceResponse {
                source_id,
                feed_url,
                normalized_feed_url,
                title,
                site_url,
                last_polled_at,
                last_success_at,
                last_error,
                consecutive_failures,
                subscribed_at,
            },
        )
        .collect())
}

pub async fn create_or_subscribe_rss_source(
    db: &SqlitePool,
    user_id: &str,
    feed_url: &str,
) -> Result<CreateRssSourceResponse, AppError> {
    let normalized_feed_url = normalize_feed_url(feed_url)?;
    validate_feed_url_safety(&normalized_feed_url)?;

    let existing = query_as::<_, SourceRow>(
        r#"
        SELECT
            source_id,
            feed_url,
            normalized_feed_url,
            title,
            site_url,
            etag,
            last_modified,
            consecutive_failures
        FROM rss_sources
        WHERE normalized_feed_url = ?
        LIMIT 1
        "#,
    )
    .bind(&normalized_feed_url)
    .fetch_optional(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS source 조회 실패: {}", err)))?;

    let source = if let Some(existing) = existing {
        query(
            r#"
            UPDATE rss_sources
            SET is_active = 1, updated_at = CURRENT_TIMESTAMP
            WHERE source_id = ?
            "#,
        )
        .bind(&existing.source_id)
        .execute(db)
        .await
        .map_err(|err| AppError::internal(format!("RSS source 갱신 실패: {}", err)))?;
        existing
    } else {
        let fetched = fetch_feed(&normalized_feed_url, None, None).await?;
        let FetchOutcome::Parsed(fetched) = fetched else {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "RSS source 생성 시 304 응답은 허용되지 않습니다.",
            ));
        };

        let source_id = Uuid::new_v4().to_string();
        query(
            r#"
            INSERT INTO rss_sources (
                source_id,
                feed_url,
                normalized_feed_url,
                title,
                site_url,
                etag,
                last_modified,
                last_polled_at,
                last_success_at,
                last_error,
                consecutive_failures,
                is_active
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL, 0, 1)
            "#,
        )
        .bind(&source_id)
        .bind(feed_url.trim())
        .bind(&normalized_feed_url)
        .bind(&fetched.feed_title)
        .bind(&fetched.site_url)
        .bind(&fetched.etag)
        .bind(&fetched.last_modified)
        .execute(db)
        .await
        .map_err(|err| AppError::internal(format!("RSS source 저장 실패: {}", err)))?;

        insert_feed_items(db, &source_id, &fetched.items)
            .await
            .map_err(|err| AppError::internal(format!("RSS item 초기 저장 실패: {}", err)))?;

        query_as::<_, SourceRow>(
            r#"
            SELECT
                source_id,
                feed_url,
                normalized_feed_url,
                title,
                site_url,
                etag,
                last_modified,
                consecutive_failures
            FROM rss_sources
            WHERE source_id = ?
            LIMIT 1
            "#,
        )
        .bind(&source_id)
        .fetch_one(db)
        .await
        .map_err(|err| AppError::internal(format!("저장된 RSS source 조회 실패: {}", err)))?
    };

    query(
        r#"
        INSERT OR IGNORE INTO user_rss_subscriptions (user_id, source_id)
        VALUES (?, ?)
        "#,
    )
    .bind(user_id)
    .bind(&source.source_id)
    .execute(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS 구독 저장 실패: {}", err)))?;

    sync_source_active_flag(db, &source.source_id).await?;

    Ok(CreateRssSourceResponse {
        source_id: source.source_id,
        feed_url: source.feed_url,
        normalized_feed_url: source.normalized_feed_url,
        title: source.title,
        site_url: source.site_url,
    })
}

pub async fn delete_user_rss_subscription(
    db: &SqlitePool,
    user_id: &str,
    source_id: &str,
) -> Result<bool, AppError> {
    let result = query(
        r#"
        DELETE FROM user_rss_subscriptions
        WHERE user_id = ? AND source_id = ?
        "#,
    )
    .bind(user_id)
    .bind(source_id)
    .execute(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS 구독 해지 실패: {}", err)))?;

    if result.rows_affected() > 0 {
        sync_source_active_flag(db, source_id).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub async fn list_push_subscriptions(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Vec<WebPushSubscriptionResponse>, AppError> {
    let rows = query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            i64,
            Option<String>,
        ),
    >(
        r#"
        SELECT
            push_subscription_id,
            endpoint,
            user_agent,
            created_at,
            last_success_at,
            last_failure_at,
            failure_count,
            revoked_at
        FROM web_push_subscriptions
        WHERE user_id = ?
        ORDER BY created_at DESC, push_subscription_id DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|err| AppError::internal(format!("Push 구독 목록 조회 실패: {}", err)))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                push_subscription_id,
                endpoint,
                user_agent,
                created_at,
                last_success_at,
                last_failure_at,
                failure_count,
                revoked_at,
            )| WebPushSubscriptionResponse {
                push_subscription_id,
                endpoint,
                user_agent,
                created_at,
                last_success_at,
                last_failure_at,
                failure_count,
                revoked_at,
            },
        )
        .collect())
}

pub async fn list_user_feed_items(
    db: &SqlitePool,
    user_id: &str,
    params: FeedItemsQuery,
) -> Result<FeedItemsResponse, AppError> {
    let limit = params.limit.unwrap_or(50).clamp(1, 100) as i64;
    let source_id = params.source_id.map(|value| value.trim().to_string());
    let _unread_only = params.unread_only.unwrap_or(false);

    let total_count = query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM user_rss_subscriptions u
        INNER JOIN rss_feed_items i ON i.source_id = u.source_id
        WHERE u.user_id = ?
          AND (? IS NULL OR u.source_id = ?)
        "#,
    )
    .bind(user_id)
    .bind(source_id.as_deref())
    .bind(source_id.as_deref())
    .fetch_one(db)
    .await
    .map_err(|err| AppError::internal(format!("Feed 요약 조회 실패: {}", err)))?;

    let unread_count = total_count;

    let rows = query_as::<_, FeedListRow>(
        r#"
        SELECT
            i.item_id,
            i.source_id,
            COALESCE(NULLIF(s.title, ''), NULLIF(s.site_url, ''), s.normalized_feed_url) AS source_title,
            i.title,
            i.link AS item_url,
            i.published_at,
            i.detected_at
        FROM user_rss_subscriptions u
        INNER JOIN rss_sources s ON s.source_id = u.source_id
        INNER JOIN rss_feed_items i ON i.source_id = u.source_id
        WHERE u.user_id = ?
          AND (? IS NULL OR u.source_id = ?)
        ORDER BY
            COALESCE(i.published_at, i.detected_at) DESC,
            i.item_id DESC
        LIMIT ?
        "#,
    )
    .bind(user_id)
    .bind(source_id.as_deref())
    .bind(source_id.as_deref())
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|err| AppError::internal(format!("Feed 목록 조회 실패: {}", err)))?;

    let items = rows
        .into_iter()
        .map(|row| FeedItemResponse {
            item_id: row.item_id.to_string(),
            source_id: row.source_id,
            source_title: row
                .source_title
                .unwrap_or_else(|| "unknown-source".to_string()),
            title: row.title,
            published_at: row.published_at.unwrap_or(row.detected_at),
            item_url: row.item_url,
            read: false,
            saved: false,
        })
        .collect();

    Ok(FeedItemsResponse {
        items,
        summary: FeedSummaryResponse {
            total_count,
            unread_count,
        },
    })
}

pub async fn upsert_push_subscription(
    db: &SqlitePool,
    user_id: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: Option<&str>,
) -> Result<WebPushSubscriptionResponse, AppError> {
    if endpoint.trim().is_empty() || p256dh.trim().is_empty() || auth.trim().is_empty() {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "endpoint와 keys 정보는 비어 있을 수 없습니다.",
        ));
    }

    query(
        r#"
        INSERT INTO web_push_subscriptions (user_id, endpoint, p256dh, auth, user_agent, revoked_at, failure_count)
        VALUES (?, ?, ?, ?, ?, NULL, 0)
        ON CONFLICT(endpoint) DO UPDATE SET
            user_id = excluded.user_id,
            p256dh = excluded.p256dh,
            auth = excluded.auth,
            user_agent = excluded.user_agent,
            revoked_at = NULL,
            failure_count = 0
        "#,
    )
    .bind(user_id)
    .bind(endpoint.trim())
    .bind(p256dh.trim())
    .bind(auth.trim())
    .bind(user_agent.map(str::trim))
    .execute(db)
    .await
    .map_err(|err| AppError::internal(format!("Push 구독 저장 실패: {}", err)))?;

    let row = query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            i64,
            Option<String>,
        ),
    >(
        r#"
        SELECT
            push_subscription_id,
            endpoint,
            user_agent,
            created_at,
            last_success_at,
            last_failure_at,
            failure_count,
            revoked_at
        FROM web_push_subscriptions
        WHERE endpoint = ?
        LIMIT 1
        "#,
    )
    .bind(endpoint.trim())
    .fetch_one(db)
    .await
    .map_err(|err| AppError::internal(format!("저장된 Push 구독 조회 실패: {}", err)))?;

    Ok(WebPushSubscriptionResponse {
        push_subscription_id: row.0,
        endpoint: row.1,
        user_agent: row.2,
        created_at: row.3,
        last_success_at: row.4,
        last_failure_at: row.5,
        failure_count: row.6,
        revoked_at: row.7,
    })
}

pub async fn revoke_push_subscription(
    db: &SqlitePool,
    user_id: &str,
    endpoint: &str,
) -> Result<bool, AppError> {
    let result = query(
        r#"
        UPDATE web_push_subscriptions
        SET revoked_at = COALESCE(revoked_at, CURRENT_TIMESTAMP)
        WHERE user_id = ? AND endpoint = ?
        "#,
    )
    .bind(user_id)
    .bind(endpoint.trim())
    .execute(db)
    .await
    .map_err(|err| AppError::internal(format!("Push 구독 해지 실패: {}", err)))?;

    Ok(result.rows_affected() > 0)
}

async fn process_single_source(
    db: &SqlitePool,
    source: &SourceRow,
    push_config: Option<&PushConfig>,
) -> Result<(), AppError> {
    match fetch_feed(
        &source.normalized_feed_url,
        source.etag.as_deref(),
        source.last_modified.as_deref(),
    )
    .await
    {
        Ok(FetchOutcome::NotModified {
            etag,
            last_modified,
        }) => {
            query(
                r#"
                UPDATE rss_sources
                SET
                    etag = COALESCE(?, etag),
                    last_modified = COALESCE(?, last_modified),
                    last_polled_at = CURRENT_TIMESTAMP,
                    last_error = NULL,
                    consecutive_failures = 0,
                    updated_at = CURRENT_TIMESTAMP
                WHERE source_id = ?
                "#,
            )
            .bind(etag)
            .bind(last_modified)
            .bind(&source.source_id)
            .execute(db)
            .await
            .map_err(|err| AppError::internal(format!("RSS source 304 갱신 실패: {}", err)))?;
            Ok(())
        }
        Ok(FetchOutcome::Parsed(fetched)) => {
            let inserted_items = insert_feed_items(db, &source.source_id, &fetched.items)
                .await
                .map_err(|err| AppError::internal(format!("RSS item 저장 실패: {}", err)))?;

            query(
                r#"
                UPDATE rss_sources
                SET
                    title = ?,
                    site_url = ?,
                    etag = ?,
                    last_modified = ?,
                    last_polled_at = CURRENT_TIMESTAMP,
                    last_success_at = CURRENT_TIMESTAMP,
                    last_error = NULL,
                    consecutive_failures = 0,
                    updated_at = CURRENT_TIMESTAMP
                WHERE source_id = ?
                "#,
            )
            .bind(&fetched.feed_title)
            .bind(&fetched.site_url)
            .bind(&fetched.etag)
            .bind(&fetched.last_modified)
            .bind(&source.source_id)
            .execute(db)
            .await
            .map_err(|err| {
                AppError::internal(format!("RSS source 성공 상태 갱신 실패: {}", err))
            })?;

            if inserted_items.is_empty() || push_config.is_none() {
                return Ok(());
            }

            deliver_inserted_items(
                db,
                &source.source_id,
                fetched
                    .feed_title
                    .or(source.title.clone())
                    .or_else(|| feed_domain(&source.feed_url)),
                inserted_items,
                push_config.expect("checked is_some"),
            )
            .await
        }
        Err(err) => {
            let previous_count = source.consecutive_failures;
            query(
                r#"
                UPDATE rss_sources
                SET
                    last_polled_at = CURRENT_TIMESTAMP,
                    last_error = ?,
                    consecutive_failures = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE source_id = ?
                "#,
            )
            .bind(&err.message)
            .bind(previous_count + 1)
            .bind(&source.source_id)
            .execute(db)
            .await
            .map_err(|update_err| {
                AppError::internal(format!("RSS 실패 상태 갱신 실패: {}", update_err))
            })?;

            Err(err)
        }
    }
}

async fn deliver_inserted_items(
    db: &SqlitePool,
    source_id: &str,
    source_label: Option<String>,
    inserted_items: Vec<InsertedFeedItem>,
    push_config: &PushConfig,
) -> Result<(), AppError> {
    let targets = query_as::<_, PushTargetRow>(
        r#"
        SELECT DISTINCT
            w.push_subscription_id,
            w.endpoint,
            w.p256dh,
            w.auth
        FROM user_rss_subscriptions u
        INNER JOIN web_push_subscriptions w ON w.user_id = u.user_id
        WHERE u.source_id = ? AND w.revoked_at IS NULL
        "#,
    )
    .bind(source_id)
    .fetch_all(db)
    .await
    .map_err(|err| AppError::internal(format!("Push 대상 조회 실패: {}", err)))?;

    if targets.is_empty() {
        return Ok(());
    }

    for item in inserted_items {
        let body = source_label
            .clone()
            .unwrap_or_else(|| "새 RSS 글이 등록되었습니다.".to_string());
        for target in &targets {
            let already_sent: i64 = query_scalar(
                r#"
                SELECT COUNT(*)
                FROM push_delivery_logs
                WHERE push_subscription_id = ? AND item_id = ?
                "#,
            )
            .bind(target.push_subscription_id)
            .bind(item.item_id)
            .fetch_one(db)
            .await
            .map_err(|err| AppError::internal(format!("Push 중복 검사 실패: {}", err)))?;

            if already_sent > 0 {
                continue;
            }

            let payload = json!({
                "title": item.title,
                "body": body,
                "url": item.link,
                "sourceId": source_id,
                "itemId": item.item_id
            })
            .to_string();

            match send_push_message(target, &payload, push_config).await {
                Ok(()) => {
                    query(
                        r#"
                        UPDATE web_push_subscriptions
                        SET last_success_at = CURRENT_TIMESTAMP, failure_count = 0
                        WHERE push_subscription_id = ?
                        "#,
                    )
                    .bind(target.push_subscription_id)
                    .execute(db)
                    .await
                    .map_err(|err| {
                        AppError::internal(format!("Push 성공 상태 저장 실패: {}", err))
                    })?;

                    query(
                        r#"
                        INSERT OR IGNORE INTO push_delivery_logs (push_subscription_id, item_id, status, error_message)
                        VALUES (?, ?, 'sent', NULL)
                        "#,
                    )
                    .bind(target.push_subscription_id)
                    .bind(item.item_id)
                    .execute(db)
                    .await
                    .map_err(|err| AppError::internal(format!("Push delivery log 저장 실패: {}", err)))?;
                }
                Err(err) => {
                    query(
                        r#"
                        UPDATE web_push_subscriptions
                        SET
                            last_failure_at = CURRENT_TIMESTAMP,
                            failure_count = failure_count + 1,
                            revoked_at = CASE WHEN ? THEN COALESCE(revoked_at, CURRENT_TIMESTAMP) ELSE revoked_at END
                        WHERE push_subscription_id = ?
                        "#,
                    )
                    .bind(is_gone_push_error(&err.message))
                    .bind(target.push_subscription_id)
                    .execute(db)
                    .await
                    .map_err(|db_err| AppError::internal(format!("Push 실패 상태 저장 실패: {}", db_err)))?;

                    query(
                        r#"
                        INSERT OR IGNORE INTO push_delivery_logs (push_subscription_id, item_id, status, error_message)
                        VALUES (?, ?, 'failed', ?)
                        "#,
                    )
                    .bind(target.push_subscription_id)
                    .bind(item.item_id)
                    .bind(&err.message)
                    .execute(db)
                    .await
                    .map_err(|db_err| AppError::internal(format!("Push 실패 로그 저장 실패: {}", db_err)))?;
                }
            }
        }
    }

    Ok(())
}

async fn send_push_message(
    target: &PushTargetRow,
    payload: &str,
    push_config: &PushConfig,
) -> Result<(), AppError> {
    let subscription_info = SubscriptionInfo::new(
        target.endpoint.clone(),
        target.p256dh.clone(),
        target.auth.clone(),
    );
    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());

    let mut signature_builder = VapidSignatureBuilder::from_base64(
        &push_config.private_key,
        base64::URL_SAFE_NO_PAD,
        &subscription_info,
    )
    .map_err(|err| AppError::internal(format!("VAPID 서명 생성 실패: {}", err)))?;
    signature_builder.add_claim("sub", push_config.subject.as_str());

    let signature = signature_builder
        .build()
        .map_err(|err| AppError::internal(format!("VAPID 서명 build 실패: {}", err)))?;
    builder.set_vapid_signature(signature);

    let client = IsahcWebPushClient::new()
        .map_err(|err| AppError::internal(format!("Push client 초기화 실패: {}", err)))?;
    let message = builder
        .build()
        .map_err(|err| AppError::internal(format!("Push 메시지 build 실패: {}", err)))?;

    client
        .send(message)
        .await
        .map_err(|err| AppError::internal(format!("Push 전송 실패: {}", err)))?;

    Ok(())
}

fn is_gone_push_error(message: &str) -> bool {
    message.contains("404") || message.contains("410")
}

async fn fetch_feed(
    feed_url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchOutcome, AppError> {
    let client = http_client()?;
    let mut request = client.get(feed_url);

    if let Some(etag) = etag {
        request = request.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = last_modified {
        request = request.header(IF_MODIFIED_SINCE, last_modified);
    }

    let response = request.send().await.map_err(|err| {
        AppError::new(StatusCode::BAD_GATEWAY, format!("RSS fetch 실패: {}", err))
    })?;

    let status = response.status();
    let new_etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let new_last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(FetchOutcome::NotModified {
            etag: new_etag,
            last_modified: new_last_modified,
        });
    }

    if !status.is_success() {
        return Err(AppError::new(
            StatusCode::BAD_GATEWAY,
            format!("RSS fetch 응답이 비정상입니다: {}", status),
        ));
    }

    if let Some(content_length) = response.content_length() {
        if content_length as usize > MAX_FEED_BYTES {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "RSS 문서 크기가 허용 한도를 초과했습니다.",
            ));
        }
    }

    let body = response.bytes().await.map_err(|err| {
        AppError::new(
            StatusCode::BAD_GATEWAY,
            format!("RSS body 읽기 실패: {}", err),
        )
    })?;
    if body.len() > MAX_FEED_BYTES {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "RSS 문서 크기가 허용 한도를 초과했습니다.",
        ));
    }

    let feed = feed_rs::parser::parse(&body[..]).map_err(|err| {
        AppError::new(
            StatusCode::BAD_REQUEST,
            format!("RSS/Atom 파싱 실패: {}", err),
        )
    })?;

    Ok(FetchOutcome::Parsed(convert_feed(
        feed,
        new_etag,
        new_last_modified,
    )))
}

fn convert_feed(
    feed: Feed,
    etag: Option<String>,
    last_modified: Option<String>,
) -> FeedFetchResult {
    let feed_title = feed.title.map(|title| title.content);
    let site_url = feed.links.first().map(|link| link.href.clone());
    let items = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title.map(|value| value.content).unwrap_or_default();
            if title.trim().is_empty() {
                return None;
            }

            let guid_or_link = if !entry.id.trim().is_empty() {
                Some(entry.id.clone())
            } else {
                entry.links.first().map(|link| link.href.clone())
            };
            let link = entry.links.first().map(|value| value.href.clone());
            let published_at = entry
                .published
                .or(entry.updated)
                .map(|value| value.to_rfc3339());

            Some(ParsedFeedItem {
                item_guid_hash: build_item_hash(
                    guid_or_link.as_deref(),
                    link.as_deref(),
                    &title,
                    published_at.as_deref(),
                ),
                guid_or_link,
                title,
                link,
                published_at,
            })
        })
        .collect();

    FeedFetchResult {
        feed_title,
        site_url,
        etag,
        last_modified,
        items,
    }
}

async fn insert_feed_items(
    db: &SqlitePool,
    source_id: &str,
    items: &[ParsedFeedItem],
) -> Result<Vec<InsertedFeedItem>, sqlx::Error> {
    let mut inserted = Vec::new();
    for item in items {
        let result = query(
            r#"
            INSERT OR IGNORE INTO rss_feed_items (
                source_id,
                item_guid_hash,
                guid_or_link,
                title,
                link,
                published_at
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(source_id)
        .bind(&item.item_guid_hash)
        .bind(&item.guid_or_link)
        .bind(&item.title)
        .bind(&item.link)
        .bind(&item.published_at)
        .execute(db)
        .await?;

        if result.rows_affected() == 1 {
            inserted.push(InsertedFeedItem {
                item_id: result.last_insert_rowid(),
                title: item.title.clone(),
                link: item.link.clone(),
            });
        }
    }

    Ok(inserted)
}

async fn sync_source_active_flag(db: &SqlitePool, source_id: &str) -> Result<(), AppError> {
    let subscription_count: i64 = query_scalar(
        r#"
        SELECT COUNT(*)
        FROM user_rss_subscriptions
        WHERE source_id = ?
        "#,
    )
    .bind(source_id)
    .fetch_one(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS 구독 수 조회 실패: {}", err)))?;

    query(
        r#"
        UPDATE rss_sources
        SET is_active = ?, updated_at = CURRENT_TIMESTAMP
        WHERE source_id = ?
        "#,
    )
    .bind(subscription_count > 0)
    .bind(source_id)
    .execute(db)
    .await
    .map_err(|err| AppError::internal(format!("RSS 활성 상태 갱신 실패: {}", err)))?;

    Ok(())
}

fn normalize_feed_url(feed_url: &str) -> Result<String, AppError> {
    let trimmed = feed_url.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "feed_url은 비어 있을 수 없습니다.",
        ));
    }

    let mut url = Url::parse(trimmed)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "유효한 feed_url 형식이 아닙니다."))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "feed_url은 http 또는 https만 허용됩니다.",
            ))
        }
    }

    url.set_fragment(None);
    if (url.scheme() == "http" && url.port() == Some(80))
        || (url.scheme() == "https" && url.port() == Some(443))
    {
        let _ = url.set_port(None);
    }

    Ok(url.to_string())
}

fn validate_feed_url_safety(feed_url: &str) -> Result<(), AppError> {
    let url = Url::parse(feed_url)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "유효한 feed_url 형식이 아닙니다."))?;
    let host = url.host_str().ok_or_else(|| {
        AppError::new(
            StatusCode::BAD_REQUEST,
            "host가 없는 URL은 허용되지 않습니다.",
        )
    })?;

    if env::var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return Ok(());
    }

    if is_blocked_host(host) {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "사설망, localhost, 메타데이터 주소는 RSS source로 사용할 수 없습니다.",
        ));
    }

    Ok(())
}

fn is_blocked_host(host: &str) -> bool {
    let lowered = host.to_ascii_lowercase();
    if lowered == "localhost" || lowered.ends_with(".localhost") {
        return true;
    }
    if lowered == "metadata.google.internal" {
        return true;
    }

    if let Ok(ip) = lowered.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
            IpAddr::V6(ipv6) => is_private_ipv6(ipv6),
        };
    }

    false
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.octets() == [169, 254, 169, 254]
}

fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local() || ip.is_unicast_link_local()
}

fn build_item_hash(
    guid_or_link: Option<&str>,
    link: Option<&str>,
    title: &str,
    published_at: Option<&str>,
) -> String {
    let identity = guid_or_link
        .or(link)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{}::{}", title.trim(), published_at.unwrap_or_default()));
    let mut hasher = Sha256::new();
    hasher.update(identity.as_bytes());
    hex::encode(hasher.finalize())
}

fn feed_domain(feed_url: &str) -> Option<String> {
    Url::parse(feed_url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
}

fn http_client() -> Result<Client, AppError> {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::limited(5))
        .user_agent("tyange-cms-api/rss-poller")
        .build()
        .map_err(|err| AppError::internal(format!("HTTP client 생성 실패: {}", err)))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        sync::{Arc, Mutex, OnceLock},
    };

    use poem::{delete, get, test::TestClient, Endpoint, EndpointExt, Route};
    use serde_json::json;
    use sqlx::{query, query_scalar, SqlitePool};
    use tokio::net::TcpListener;

    use crate::{
        db::init_db,
        middlewares::auth_middleware::Auth,
        models::AppState,
        routes::{
            create_rss_source::create_rss_source,
            delete_push_subscription::delete_push_subscription,
            delete_rss_subscription::delete_rss_subscription,
            get_push_public_key::get_push_public_key,
            get_push_subscriptions::get_push_subscriptions, get_rss_sources::get_rss_sources,
            upsert_push_subscription::upsert_push_subscription,
        },
    };
    use tyange_cms_api::auth::jwt::Claims;

    use super::{
        build_item_hash, is_blocked_host, normalize_feed_url, process_active_sources_once,
    };

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    async fn create_test_state() -> Arc<AppState> {
        env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
        env::set_var("JWT_REFRESH_SECRET", "test-refresh-secret");
        let db = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("failed to connect sqlite");
        init_db(&db).await.expect("failed to init db");
        Arc::new(AppState::new(db))
    }

    fn issue_access_token(user_id: &str) -> String {
        Claims::create_access_token(user_id, "user", b"test-access-secret")
            .expect("failed to create access token")
    }

    fn create_rss_app(state: Arc<AppState>) -> impl Endpoint {
        Route::new()
            .at(
                "/rss-sources",
                get(get_rss_sources).post(create_rss_source).with(Auth),
            )
            .at(
                "/rss-sources/:source_id/subscription",
                delete(delete_rss_subscription).with(Auth),
            )
            .at(
                "/push/subscriptions",
                get(get_push_subscriptions)
                    .post(upsert_push_subscription)
                    .delete(delete_push_subscription)
                    .with(Auth),
            )
            .at("/push/public-key", get(get_push_public_key))
            .data(state)
    }

    async fn spawn_feed_server(responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind tcp listener");
        let addr = listener.local_addr().expect("listener addr");

        tokio::spawn(async move {
            for body in responses {
                let (mut socket, _) = listener.accept().await.expect("failed to accept");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                use tokio::io::AsyncWriteExt;
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("failed to write response");
            }
        });

        format!("http://127.0.0.1:{}/feed.xml", addr.port())
    }

    fn sample_feed(title: &str, item_title: &str, item_link: &str, pub_date: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>{title}</title>
    <link>https://example.com</link>
    <description>feed</description>
    <item>
      <guid>{item_link}</guid>
      <title>{item_title}</title>
      <link>{item_link}</link>
      <pubDate>{pub_date}</pubDate>
    </item>
  </channel>
</rss>"#
        )
    }

    #[test]
    fn normalize_feed_url_drops_fragment_and_default_port() {
        let normalized = normalize_feed_url("https://Example.com:443/feed.xml#section")
            .expect("normalize should work");
        assert_eq!(normalized, "https://example.com/feed.xml");
    }

    #[test]
    fn blocked_host_catches_local_and_metadata_hosts() {
        assert!(is_blocked_host("localhost"));
        assert!(is_blocked_host("127.0.0.1"));
        assert!(is_blocked_host("169.254.169.254"));
        assert!(!is_blocked_host("example.com"));
    }

    #[test]
    fn item_hash_falls_back_to_title_and_published_at() {
        let first = build_item_hash(None, None, "hello", Some("2026-03-12T00:00:00Z"));
        let second = build_item_hash(None, None, "hello", Some("2026-03-12T00:00:00Z"));
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn create_rss_source_registers_subscription_and_seeds_items() {
        let _guard = env_lock().lock().expect("env lock");
        env::set_var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS", "1");
        let state = create_test_state().await;
        let cli = TestClient::new(create_rss_app(state.clone()));
        let feed_url = spawn_feed_server(vec![sample_feed(
            "Test Feed",
            "First",
            "https://example.com/posts/1",
            "Wed, 12 Mar 2026 09:00:00 GMT",
        )])
        .await;

        let response = cli
            .post("/rss-sources")
            .header("Authorization", issue_access_token("rss-user@example.com"))
            .body_json(&json!({ "feed_url": feed_url }))
            .send()
            .await;

        response.assert_status_is_ok();

        let source_count: i64 = query_scalar("SELECT COUNT(*) FROM rss_sources")
            .fetch_one(&state.db)
            .await
            .expect("failed to count rss_sources");
        let item_count: i64 = query_scalar("SELECT COUNT(*) FROM rss_feed_items")
            .fetch_one(&state.db)
            .await
            .expect("failed to count rss_feed_items");
        let subscription_count: i64 = query_scalar("SELECT COUNT(*) FROM user_rss_subscriptions")
            .fetch_one(&state.db)
            .await
            .expect("failed to count user_rss_subscriptions");

        assert_eq!(source_count, 1);
        assert_eq!(item_count, 1);
        assert_eq!(subscription_count, 1);
        env::remove_var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS");
    }

    #[tokio::test]
    async fn create_rss_source_rejects_localhost_without_override() {
        let _guard = env_lock().lock().expect("env lock");
        env::remove_var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS");
        let state = create_test_state().await;
        let cli = TestClient::new(create_rss_app(state));

        cli.post("/rss-sources")
            .header("Authorization", issue_access_token("rss-user@example.com"))
            .body_json(&json!({ "feed_url": "http://127.0.0.1/feed.xml" }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn process_active_sources_inserts_only_new_items() {
        let _guard = env_lock().lock().expect("env lock");
        env::set_var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS", "1");
        let state = create_test_state().await;
        let feed_url = spawn_feed_server(vec![
            sample_feed(
                "Test Feed",
                "First",
                "https://example.com/posts/1",
                "Wed, 12 Mar 2026 09:00:00 GMT",
            ),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <link>https://example.com</link>
    <description>feed</description>
    <item>
      <guid>https://example.com/posts/1</guid>
      <title>First</title>
      <link>https://example.com/posts/1</link>
      <pubDate>Wed, 12 Mar 2026 09:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>https://example.com/posts/2</guid>
      <title>Second</title>
      <link>https://example.com/posts/2</link>
      <pubDate>Wed, 12 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#
            ),
        ])
        .await;

        query(
            r#"
            INSERT INTO rss_sources (
                source_id,
                feed_url,
                normalized_feed_url,
                is_active
            )
            VALUES (?, ?, ?, 1)
            "#,
        )
        .bind("source-1")
        .bind(&feed_url)
        .bind(&feed_url)
        .execute(&state.db)
        .await
        .expect("failed to seed rss source");

        process_active_sources_once(&state.db)
            .await
            .expect("first poll should succeed");
        process_active_sources_once(&state.db)
            .await
            .expect("second poll should succeed");

        let item_count: i64 = query_scalar("SELECT COUNT(*) FROM rss_feed_items")
            .fetch_one(&state.db)
            .await
            .expect("failed to count rss_feed_items");
        assert_eq!(item_count, 2);
        env::remove_var("ALLOW_PRIVATE_FEED_URLS_FOR_TESTS");
    }

    #[tokio::test]
    async fn push_subscription_endpoints_support_upsert_list_and_revoke() {
        let _guard = env_lock().lock().expect("env lock");
        env::set_var("VAPID_PUBLIC_KEY", "test-public-key");
        let state = create_test_state().await;
        let cli = TestClient::new(create_rss_app(state.clone()));

        cli.post("/push/subscriptions")
            .header("Authorization", issue_access_token("push-user@example.com"))
            .header("User-Agent", "Mozilla/Test")
            .body_json(&json!({
                "endpoint": "https://push.example.com/subscriptions/1",
                "keys": {
                    "p256dh": "key-p256dh",
                    "auth": "key-auth"
                }
            }))
            .send()
            .await
            .assert_status_is_ok();

        let list_response = cli
            .get("/push/subscriptions")
            .header("Authorization", issue_access_token("push-user@example.com"))
            .send()
            .await;
        list_response.assert_status_is_ok();

        let public_key_response = cli.get("/push/public-key").send().await;
        public_key_response.assert_status_is_ok();
        let public_key_json = public_key_response.json().await;
        public_key_json
            .value()
            .object()
            .get("status")
            .assert_bool(true);
        public_key_json
            .value()
            .object()
            .get("data")
            .object()
            .get("public_key")
            .assert_string("test-public-key");
        public_key_json
            .value()
            .object()
            .get("message")
            .assert_null();

        cli.delete("/push/subscriptions")
            .header("Authorization", issue_access_token("push-user@example.com"))
            .body_json(&json!({
                "endpoint": "https://push.example.com/subscriptions/1"
            }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::NO_CONTENT);

        let revoked_at: Option<String> =
            query_scalar("SELECT revoked_at FROM web_push_subscriptions WHERE endpoint = ?")
                .bind("https://push.example.com/subscriptions/1")
                .fetch_one(&state.db)
                .await
                .expect("failed to fetch revoked_at");
        assert!(revoked_at.is_some());
        env::remove_var("VAPID_PUBLIC_KEY");
    }

    #[tokio::test]
    async fn push_public_key_endpoint_returns_503_when_vapid_key_is_missing() {
        let _guard = env_lock().lock().expect("env lock");
        env::remove_var("VAPID_PUBLIC_KEY");

        let state = create_test_state().await;
        let cli = TestClient::new(create_rss_app(state));

        cli.get("/push/public-key")
            .send()
            .await
            .assert_status(poem::http::StatusCode::SERVICE_UNAVAILABLE);
    }
}
