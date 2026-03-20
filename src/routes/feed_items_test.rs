use std::{env, sync::Arc};

use poem::{get, http::StatusCode, test::TestClient, Endpoint, EndpointExt, Route};
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    db::init_db, middlewares::auth_middleware::Auth, models::AppState,
    routes::get_feed_items::get_feed_items,
};
use tyange_cms_api::auth::jwt::Claims;

async fn create_test_state() -> Arc<AppState> {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");
    Arc::new(AppState::new(db))
}

fn create_feed_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at("/feed/items", get(get_feed_items).with(Auth))
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

async fn insert_source(state: &Arc<AppState>, source_id: &str, title: &str) {
    query(
        r#"
        INSERT INTO rss_sources (
            source_id,
            feed_url,
            normalized_feed_url,
            title,
            is_active
        )
        VALUES (?, ?, ?, ?, 1)
        "#,
    )
    .bind(source_id)
    .bind(format!("https://example.com/{source_id}.xml"))
    .bind(format!("https://example.com/{source_id}.xml"))
    .bind(title)
    .execute(&state.db)
    .await
    .expect("failed to insert rss source");
}

async fn subscribe_user(state: &Arc<AppState>, user_id: &str, source_id: &str) {
    query(
        r#"
        INSERT INTO user_rss_subscriptions (user_id, source_id)
        VALUES (?, ?)
        "#,
    )
    .bind(user_id)
    .bind(source_id)
    .execute(&state.db)
    .await
    .expect("failed to insert rss subscription");
}

async fn insert_feed_item(
    state: &Arc<AppState>,
    source_id: &str,
    guid_hash: &str,
    title: &str,
    link: &str,
    published_at: &str,
) {
    query(
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
    .bind(guid_hash)
    .bind(link)
    .bind(title)
    .bind(link)
    .bind(published_at)
    .execute(&state.db)
    .await
    .expect("failed to insert rss item");
}

#[tokio::test]
async fn feed_items_requires_authentication() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_feed_app(state));

    cli.get("/feed/items")
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn feed_items_returns_empty_list_for_user_without_subscriptions() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_feed_app(state));

    let response = cli
        .get("/feed/items")
        .header("Authorization", issue_access_token("empty-user", "user"))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("items").array().assert_len(0);
    data.get("summary")
        .object()
        .get("total_count")
        .assert_i64(0);
    data.get("summary")
        .object()
        .get("unread_count")
        .assert_i64(0);
}

#[tokio::test]
async fn feed_items_aggregates_multiple_sources_and_sorts_latest_first() {
    let state = create_test_state().await;
    insert_source(&state, "source-a", "Source A").await;
    insert_source(&state, "source-b", "Source B").await;
    subscribe_user(&state, "reader-1", "source-a").await;
    subscribe_user(&state, "reader-1", "source-b").await;

    insert_feed_item(
        &state,
        "source-a",
        "hash-a1",
        "Older item",
        "https://example.com/a1",
        "2026-03-18T08:00:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-b",
        "hash-b1",
        "Newest item",
        "https://example.com/b1",
        "2026-03-19T09:20:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-a",
        "hash-a2",
        "Middle item",
        "https://example.com/a2",
        "2026-03-19T08:00:00+00:00",
    )
    .await;

    let cli = TestClient::new(create_feed_app(state));
    let response = cli
        .get("/feed/items?limit=10")
        .header("Authorization", issue_access_token("reader-1", "user"))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    let data = json.value().object().get("data").object();
    let items = data.get("items").array();
    items.assert_len(3);
    items
        .get(0)
        .object()
        .get("title")
        .assert_string("Newest item");
    items
        .get(0)
        .object()
        .get("source_title")
        .assert_string("Source B");
    items.get(0).object().get("read").assert_bool(false);
    items.get(0).object().get("saved").assert_bool(false);
    items
        .get(1)
        .object()
        .get("title")
        .assert_string("Middle item");
    items
        .get(2)
        .object()
        .get("title")
        .assert_string("Older item");
    data.get("summary")
        .object()
        .get("total_count")
        .assert_i64(3);
    data.get("summary")
        .object()
        .get("unread_count")
        .assert_i64(3);
}

#[tokio::test]
async fn feed_items_dedupes_reingested_items_from_same_source() {
    let state = create_test_state().await;
    insert_source(&state, "source-dedupe", "Source Dedupe").await;
    subscribe_user(&state, "reader-2", "source-dedupe").await;

    insert_feed_item(
        &state,
        "source-dedupe",
        "same-hash",
        "Only once",
        "https://example.com/dedupe",
        "2026-03-19T10:00:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-dedupe",
        "same-hash",
        "Only once",
        "https://example.com/dedupe",
        "2026-03-19T10:00:00+00:00",
    )
    .await;

    let stored_count: i64 = query_scalar(
        "SELECT COUNT(*) FROM rss_feed_items WHERE source_id = ? AND item_guid_hash = ?",
    )
    .bind("source-dedupe")
    .bind("same-hash")
    .fetch_one(&state.db)
    .await
    .expect("failed to count deduped rss items");
    assert_eq!(stored_count, 1);

    let cli = TestClient::new(create_feed_app(state));
    let response = cli
        .get("/feed/items")
        .header("Authorization", issue_access_token("reader-2", "user"))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    json.value()
        .object()
        .get("data")
        .object()
        .get("items")
        .array()
        .assert_len(1);
}

#[tokio::test]
async fn feed_items_can_filter_by_source_id() {
    let state = create_test_state().await;
    insert_source(&state, "source-1", "Source 1").await;
    insert_source(&state, "source-2", "Source 2").await;
    subscribe_user(&state, "reader-3", "source-1").await;
    subscribe_user(&state, "reader-3", "source-2").await;

    insert_feed_item(
        &state,
        "source-1",
        "hash-1",
        "Source one item",
        "https://example.com/1",
        "2026-03-19T09:00:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-2",
        "hash-2",
        "Source two item",
        "https://example.com/2",
        "2026-03-19T10:00:00+00:00",
    )
    .await;

    let cli = TestClient::new(create_feed_app(state));
    let response = cli
        .get("/feed/items?source_id=source-1")
        .header("Authorization", issue_access_token("reader-3", "user"))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    let data = json.value().object().get("data").object();
    let items = data.get("items").array();
    items.assert_len(1);
    items
        .get(0)
        .object()
        .get("source_id")
        .assert_string("source-1");
    items
        .get(0)
        .object()
        .get("title")
        .assert_string("Source one item");
    data.get("summary")
        .object()
        .get("total_count")
        .assert_i64(1);
}

#[tokio::test]
async fn feed_items_can_paginate_with_offset() {
    let state = create_test_state().await;
    insert_source(&state, "source-page", "Source Page").await;
    subscribe_user(&state, "reader-4", "source-page").await;

    insert_feed_item(
        &state,
        "source-page",
        "hash-1",
        "Item 1",
        "https://example.com/1",
        "2026-03-19T12:00:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-page",
        "hash-2",
        "Item 2",
        "https://example.com/2",
        "2026-03-19T11:00:00+00:00",
    )
    .await;
    insert_feed_item(
        &state,
        "source-page",
        "hash-3",
        "Item 3",
        "https://example.com/3",
        "2026-03-19T10:00:00+00:00",
    )
    .await;

    let cli = TestClient::new(create_feed_app(state));
    let response = cli
        .get("/feed/items?limit=2&offset=1")
        .header("Authorization", issue_access_token("reader-4", "user"))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    let data = json.value().object().get("data").object();
    let items = data.get("items").array();
    items.assert_len(2);
    items.get(0).object().get("title").assert_string("Item 2");
    items.get(1).object().get("title").assert_string("Item 3");
    data.get("summary").object().get("total_count").assert_i64(3);
}
