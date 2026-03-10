use std::{env, sync::Arc};

use bcrypt::{hash, DEFAULT_COST};
use chrono::Local;
use poem::{get, http::StatusCode, post, put, test::TestClient, Endpoint, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    db::init_db,
    middlewares::api_key_middleware::JwtOrApiKeyAuth,
    middlewares::auth_middleware::Auth,
    models::AppState,
    routes::{
        add_user::create_user,
        create_api_key::create_api_key_handler,
        create_budget_plan::create_budget_plan,
        create_spending::create_spending,
        delete_api_key::delete_api_key,
        delete_spending::delete_spending,
        get_api_keys::get_api_keys,
        get_budget_weeks::get_budget_weeks,
        get_spending::get_spending,
        get_weekly_config::get_weekly_config,
        get_weekly_summary::{get_weekly_summary, get_weekly_summary_by_key},
        rebalance_budget::rebalance_budget,
        set_budget::set_budget,
        update_budget::update_budget,
        update_spending::update_spending,
    },
    utils::current_iso_week_key,
};
use tyange_cms_api::auth::jwt::Claims;

async fn create_test_state() -> Arc<AppState> {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");
    Arc::new(AppState { db })
}

fn create_budget_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at(
            "/api-keys",
            post(create_api_key_handler).get(get_api_keys).with(Auth),
        )
        .at(
            "/api-keys/:api_key_id",
            poem::delete(delete_api_key).with(Auth),
        )
        .at("/budget/weekly-config", get(get_weekly_config).with(Auth))
        .at("/budget/set", post(set_budget).with(Auth))
        .at("/budget/plan", post(create_budget_plan).with(Auth))
        .at("/budget/rebalance", post(rebalance_budget).with(Auth))
        .at("/budget/update/:config_id", put(update_budget).with(Auth))
        .at(
            "/budget/spending",
            get(get_spending.with(Auth)).post(create_spending.with(JwtOrApiKeyAuth)),
        )
        .at(
            "/budget/spending/:record_id",
            put(update_spending).delete(delete_spending).with(Auth),
        )
        .at("/budget/weekly", get(get_weekly_summary).with(Auth))
        .at(
            "/budget/weekly/:week_key",
            get(get_weekly_summary_by_key).with(Auth),
        )
        .at("/budget/weeks", get(get_budget_weeks).with(Auth))
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

fn today_transacted_at() -> String {
    Local::now().format("%Y-%m-%dT12:00:00").to_string()
}

async fn weekly_limit_for_owner(state: &Arc<AppState>, owner_user_id: &str, week_key: &str) -> i64 {
    query_scalar("SELECT weekly_limit FROM budget_config WHERE owner_user_id = ? AND week_key = ?")
        .bind(owner_user_id)
        .bind(week_key)
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch weekly_limit")
}

async fn projected_remaining_for_owner(
    state: &Arc<AppState>,
    owner_user_id: &str,
    week_key: &str,
) -> i64 {
    query_scalar(
        "SELECT projected_remaining FROM budget_config WHERE owner_user_id = ? AND week_key = ?",
    )
    .bind(owner_user_id)
    .bind(week_key)
    .fetch_one(&state.db)
    .await
    .expect("failed to fetch projected_remaining")
}

#[tokio::test]
async fn migrates_legacy_budget_and_spending_rows_to_admin_owner() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");

    query(
        r#"
        CREATE TABLE budget_config (
            config_id INTEGER PRIMARY KEY AUTOINCREMENT,
            week_key TEXT NOT NULL UNIQUE,
            weekly_limit INTEGER NOT NULL DEFAULT 500000,
            alert_threshold REAL NOT NULL DEFAULT 0.85,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create legacy budget_config");

    query(
        r#"
        CREATE TABLE spending_records (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            week_key TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create legacy spending_records");

    query(
        "INSERT INTO budget_config (week_key, weekly_limit, alert_threshold)
         VALUES ('2026-W10', 700000, 0.9)",
    )
    .execute(&db)
    .await
    .expect("failed to seed legacy budget");

    query(
        "INSERT INTO spending_records (amount, merchant, transacted_at, week_key)
         VALUES (12000, 'legacy-store', '2026-03-03 12:00:00', '2026-W10')",
    )
    .execute(&db)
    .await
    .expect("failed to seed legacy spending");

    init_db(&db).await.expect("failed to migrate db");

    let budget_owner: String =
        query_scalar("SELECT owner_user_id FROM budget_config WHERE week_key = '2026-W10'")
            .fetch_one(&db)
            .await
            .expect("failed to fetch migrated budget owner");
    let spending_owner: String =
        query_scalar("SELECT owner_user_id FROM spending_records WHERE week_key = '2026-W10'")
            .fetch_one(&db)
            .await
            .expect("failed to fetch migrated spending owner");

    assert_eq!(budget_owner, "admin");
    assert_eq!(spending_owner, "admin");
}

#[tokio::test]
async fn migrates_legacy_api_keys_to_lookup_and_default_role() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");

    query(
        r#"
        CREATE TABLE api_keys (
            api_key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create legacy api_keys");

    query(
        r#"
        INSERT INTO api_keys (user_id, name, key_hash)
        VALUES (?, ?, ?)
        "#,
    )
    .bind("legacy-user")
    .bind("old phone")
    .bind(hash("legacy-plain-key", DEFAULT_COST).expect("failed to hash legacy key"))
    .execute(&db)
    .await
    .expect("failed to seed legacy api_keys");

    init_db(&db).await.expect("failed to migrate db");

    let migrated: (String, String) =
        sqlx::query_as("SELECT key_lookup, user_role FROM api_keys WHERE user_id = ? LIMIT 1")
            .bind("legacy-user")
            .fetch_one(&db)
            .await
            .expect("failed to fetch migrated api key");

    assert!(!migrated.0.is_empty());
    assert_eq!(migrated.1, "user");
}

#[tokio::test]
async fn budget_and_spending_are_scoped_per_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let transacted_at = today_transacted_at();
    let current_week = current_iso_week_key();

    cli.post("/budget/set")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({"weekly_limit": 1000, "alert_threshold": 0.8}))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/set")
        .header("Authorization", issue_access_token("user-2", "user"))
        .body_json(&json!({"weekly_limit": 2000, "alert_threshold": 0.9}))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({"amount": 100, "merchant": "coffee", "transacted_at": transacted_at}))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("user-2", "user"))
        .body_json(
            &json!({"amount": 250, "merchant": "lunch", "transacted_at": today_transacted_at()}),
        )
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let user1_summary = cli
        .get("/budget/weekly")
        .header("Authorization", issue_access_token("user-1", "user"))
        .send()
        .await;
    user1_summary.assert_status_is_ok();
    let user1_json = user1_summary.json().await;
    user1_json
        .value()
        .object()
        .get("week_key")
        .assert_string(&current_week);
    user1_json
        .value()
        .object()
        .get("weekly_limit")
        .assert_i64(1000);
    user1_json
        .value()
        .object()
        .get("total_spent")
        .assert_i64(100);
    user1_json.value().object().get("remaining").assert_i64(900);
    user1_json
        .value()
        .object()
        .get("projected_remaining")
        .assert_i64(1000);
    user1_json
        .value()
        .object()
        .get("record_count")
        .assert_i64(1);

    let user2_summary = cli
        .get("/budget/weekly")
        .header("Authorization", issue_access_token("user-2", "user"))
        .send()
        .await;
    user2_summary.assert_status_is_ok();
    let user2_json = user2_summary.json().await;
    user2_json
        .value()
        .object()
        .get("weekly_limit")
        .assert_i64(2000);
    user2_json
        .value()
        .object()
        .get("total_spent")
        .assert_i64(250);
    user2_json
        .value()
        .object()
        .get("remaining")
        .assert_i64(1750);
    user2_json
        .value()
        .object()
        .get("projected_remaining")
        .assert_i64(2000);
    user2_json
        .value()
        .object()
        .get("record_count")
        .assert_i64(1);

    let user1_spending = cli
        .get("/budget/spending")
        .header("Authorization", issue_access_token("user-1", "user"))
        .send()
        .await;
    user1_spending.assert_status_is_ok();
    let user1_spending_count: i64 = query_scalar(
        "SELECT COUNT(*) FROM spending_records WHERE owner_user_id = ? AND week_key = ?",
    )
    .bind("user-1")
    .bind(&current_week)
    .fetch_one(&state.db)
    .await
    .expect("failed to count user-1 spending records");
    assert_eq!(user1_spending_count, 1);

    let config_count: i64 = query_scalar("SELECT COUNT(*) FROM budget_config WHERE week_key = ?")
        .bind(&current_week)
        .fetch_one(&state.db)
        .await
        .expect("failed to count budget configs");

    assert_eq!(config_count, 2);
}

#[tokio::test]
async fn update_and_delete_require_record_owner_and_budget_owner() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));

    cli.post("/budget/set")
        .header("Authorization", issue_access_token("owner-1", "user"))
        .body_json(&json!({"weekly_limit": 1000, "alert_threshold": 0.8}))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("owner-1", "user"))
        .body_json(&json!({
            "amount": 100,
            "merchant": "groceries",
            "transacted_at": today_transacted_at()
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let record_id: i64 = query_scalar(
        "SELECT record_id FROM spending_records WHERE owner_user_id = ? ORDER BY record_id DESC LIMIT 1",
    )
    .bind("owner-1")
    .fetch_one(&state.db)
    .await
    .expect("failed to fetch record_id");

    let config_id: i64 = query_scalar(
        "SELECT config_id FROM budget_config WHERE owner_user_id = ? ORDER BY config_id DESC LIMIT 1",
    )
    .bind("owner-1")
    .fetch_one(&state.db)
    .await
    .expect("failed to fetch config_id");

    cli.put(&format!("/budget/spending/{}", record_id))
        .header("Authorization", issue_access_token("other-user", "user"))
        .body_json(&json!({
            "amount": 300,
            "merchant": "blocked",
            "transacted_at": today_transacted_at()
        }))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);

    cli.delete(&format!("/budget/spending/{}", record_id))
        .header("Authorization", issue_access_token("other-user", "user"))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);

    cli.put(&format!("/budget/update/{}", config_id))
        .header("Authorization", issue_access_token("other-user", "user"))
        .body_json(&json!({"weekly_limit": 3000, "alert_threshold": 0.7}))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rebalance_updates_remaining_weeks_from_as_of_week() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("rebalance-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 2_500_000,
            "from_date": "2026-03-22",
            "to_date": "2026-04-21",
            "alert_threshold": 0.85
        }))
        .send()
        .await
        .assert_status_is_ok();

    for (amount, transacted_at) in [
        (100_000, "2026-03-25T12:00:00"),
        (350_000, "2026-04-01T09:00:00"),
    ] {
        cli.post("/budget/spending")
            .header("Authorization", &token)
            .body_json(&json!({
                "amount": amount,
                "merchant": "seed",
                "transacted_at": transacted_at
            }))
            .send()
            .await
            .assert_status(StatusCode::CREATED);
    }

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 2_500_000,
            "from_date": "2026-03-22",
            "to_date": "2026-04-21",
            "as_of_date": "2026-04-01",
            "alert_threshold": 0.9
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    json.value().object().get("status").assert_bool(true);
    let data = json.value().object().get("data").object();
    data.get("spent_so_far").assert_i64(450_000);
    data.get("remaining_budget").assert_i64(2_050_000);
    data.get("rebalance_from_week").assert_string("2026-W14");
    data.get("is_overspent").assert_bool(false);
    let weeks = data.get("weeks").array();
    assert_eq!(weeks.len(), 4);
    weeks
        .get(0)
        .object()
        .get("week_key")
        .assert_string("2026-W14");
    weeks.get(0).object().get("days").assert_i64(5);
    weeks
        .get(0)
        .object()
        .get("weekly_limit")
        .assert_i64(488_095);
    weeks
        .get(0)
        .object()
        .get("projected_remaining")
        .assert_i64(488_095);
    weeks
        .get(1)
        .object()
        .get("week_key")
        .assert_string("2026-W15");
    weeks
        .get(1)
        .object()
        .get("weekly_limit")
        .assert_i64(683_334);
    weeks
        .get(1)
        .object()
        .get("projected_remaining")
        .assert_i64(683_334);
    weeks
        .get(2)
        .object()
        .get("week_key")
        .assert_string("2026-W16");
    weeks
        .get(2)
        .object()
        .get("weekly_limit")
        .assert_i64(683_333);
    weeks
        .get(2)
        .object()
        .get("projected_remaining")
        .assert_i64(683_333);
    weeks
        .get(3)
        .object()
        .get("week_key")
        .assert_string("2026-W17");
    weeks
        .get(3)
        .object()
        .get("weekly_limit")
        .assert_i64(195_238);
    weeks
        .get(3)
        .object()
        .get("projected_remaining")
        .assert_i64(195_238);

    assert_eq!(
        weekly_limit_for_owner(&state, "rebalance-user", "2026-W12").await,
        80_645
    );
    assert_eq!(
        weekly_limit_for_owner(&state, "rebalance-user", "2026-W13").await,
        564_516
    );
    assert_eq!(
        weekly_limit_for_owner(&state, "rebalance-user", "2026-W14").await,
        488_095
    );
}

#[tokio::test]
async fn rebalance_only_counts_current_owner_spending() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));

    for owner in ["owner-a", "owner-b"] {
        cli.post("/budget/plan")
            .header("Authorization", issue_access_token(owner, "user"))
            .body_json(&json!({
                "total_budget": 700_000,
                "from_date": "2026-03-22",
                "to_date": "2026-04-04",
                "alert_threshold": 0.85
            }))
            .send()
            .await
            .assert_status_is_ok();
    }

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("owner-a", "user"))
        .body_json(&json!({
            "amount": 100_000,
            "merchant": "a",
            "transacted_at": "2026-03-25T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("owner-b", "user"))
        .body_json(&json!({
            "amount": 250_000,
            "merchant": "b",
            "transacted_at": "2026-03-26T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", issue_access_token("owner-a", "user"))
        .body_json(&json!({
            "total_budget": 700_000,
            "from_date": "2026-03-22",
            "to_date": "2026-04-04",
            "as_of_date": "2026-03-26"
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("spent_so_far").assert_i64(100_000);
    data.get("remaining_budget").assert_i64(600_000);
}

#[tokio::test]
async fn rebalance_uses_request_spent_so_far_when_provided() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("manual-spent-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1_000_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "alert_threshold": 0.85
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", &token)
        .body_json(&json!({
            "amount": 100_000,
            "merchant": "db-spending",
            "transacted_at": "2026-03-25T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1_000_000,
            "spent_so_far": 400_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-03-30"
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("spent_so_far").assert_i64(400_000);
    data.get("remaining_budget").assert_i64(600_000);

    assert_eq!(
        weekly_limit_for_owner(&state, "manual-spent-user", "2026-W14").await,
        600_000
    );
}

#[tokio::test]
async fn api_key_can_create_spending_for_its_owner_and_updates_last_used_at() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    create_user(&state.db, "macro-user", "password", "user")
        .await
        .expect("failed to create user");

    cli.post("/budget/set")
        .header("Authorization", issue_access_token("macro-user", "user"))
        .body_json(&json!({"weekly_limit": 1000, "alert_threshold": 0.8}))
        .send()
        .await
        .assert_status_is_ok();

    let create_key = cli
        .post("/api-keys")
        .header("Authorization", issue_access_token("macro-user", "user"))
        .body_json(&json!({"name": "macrodroid phone"}))
        .send()
        .await;
    create_key.assert_status(StatusCode::CREATED);
    let create_json = create_key.json().await;
    let api_key = create_json
        .value()
        .object()
        .get("api_key")
        .string()
        .to_string();
    let api_key_id = create_json.value().object().get("id").i64();

    cli.post("/budget/spending")
        .header("X-API-Key", &api_key)
        .body_json(&json!({
            "amount": 200,
            "merchant": "coffee",
            "transacted_at": today_transacted_at()
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let owner: String =
        query_scalar("SELECT owner_user_id FROM spending_records ORDER BY record_id DESC LIMIT 1")
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch spending owner");
    assert_eq!(owner, "macro-user");

    let last_used_at: Option<String> =
        query_scalar("SELECT last_used_at FROM api_keys WHERE api_key_id = ?")
            .bind(api_key_id)
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch api key last_used_at");
    assert!(last_used_at.is_some());
}

#[tokio::test]
async fn api_keys_are_listed_without_plaintext_and_revoked_keys_stop_working() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    create_user(&state.db, "owner", "password", "user")
        .await
        .expect("failed to create owner");
    create_user(&state.db, "other", "password", "user")
        .await
        .expect("failed to create other");

    cli.post("/budget/set")
        .header("Authorization", issue_access_token("owner", "user"))
        .body_json(&json!({"weekly_limit": 1500, "alert_threshold": 0.8}))
        .send()
        .await
        .assert_status_is_ok();

    let create_response = cli
        .post("/api-keys")
        .header("Authorization", issue_access_token("owner", "user"))
        .body_json(&json!({"name": "android"}))
        .send()
        .await;
    create_response.assert_status(StatusCode::CREATED);
    let create_json = create_response.json().await;
    let api_key = create_json
        .value()
        .object()
        .get("api_key")
        .string()
        .to_string();
    let api_key_id = create_json.value().object().get("id").i64();

    let list_response = cli
        .get("/api-keys")
        .header("Authorization", issue_access_token("owner", "user"))
        .send()
        .await;
    list_response.assert_status_is_ok();
    let list_body = list_response
        .0
        .into_body()
        .into_string()
        .await
        .expect("failed to read list response body");
    assert!(!list_body.contains(&api_key));
    let list_json: serde_json::Value =
        serde_json::from_str(&list_body).expect("failed to parse list response json");
    let items = list_json["api_keys"]
        .as_array()
        .expect("api_keys should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "android");
    assert_eq!(items[0]["id"], api_key_id);

    cli.delete(&format!("/api-keys/{}", api_key_id))
        .header("Authorization", issue_access_token("other", "user"))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);

    cli.delete(&format!("/api-keys/{}", api_key_id))
        .header("Authorization", issue_access_token("owner", "user"))
        .send()
        .await
        .assert_status(StatusCode::NO_CONTENT);

    cli.post("/budget/spending")
        .header("X-API-Key", &api_key)
        .body_json(&json!({
            "amount": 100,
            "merchant": "blocked",
            "transacted_at": today_transacted_at()
        }))
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);

    let revoked_at: Option<String> =
        query_scalar("SELECT revoked_at FROM api_keys WHERE api_key_id = ?")
            .bind(api_key_id)
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch revoked_at");
    assert!(revoked_at.is_some());
}

#[tokio::test]
async fn rebalance_rejects_negative_request_spent_so_far() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));

    cli.post("/budget/rebalance")
        .header(
            "Authorization",
            issue_access_token("negative-spent-user", "user"),
        )
        .body_json(&json!({
            "total_budget": 700_000,
            "spent_so_far": -1,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-03-30"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rebalance_preserves_past_weeks_and_updates_remaining_weeks_only() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("history-user", "user");

    query(
        "INSERT INTO budget_config (
             owner_user_id,
             week_key,
             weekly_limit,
             projected_remaining,
             alert_threshold
         )
         VALUES (?, '2026-W13', 123456, 123456, 0.8)",
    )
    .bind("history-user")
    .execute(&state.db)
    .await
    .expect("failed to insert historical budget");

    query(
        "INSERT INTO budget_config (
             owner_user_id,
             week_key,
             weekly_limit,
             projected_remaining,
             alert_threshold
         )
         VALUES (?, '2026-W14', 1, 1, 0.8),
                (?, '2026-W15', 1, 1, 0.8)",
    )
    .bind("history-user")
    .bind("history-user")
    .execute(&state.db)
    .await
    .expect("failed to insert future budget");

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 700_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-12",
            "as_of_date": "2026-03-30",
            "alert_threshold": 0.9
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    assert_eq!(
        weekly_limit_for_owner(&state, "history-user", "2026-W13").await,
        123_456
    );
    assert_eq!(
        weekly_limit_for_owner(&state, "history-user", "2026-W14").await,
        350_000
    );
    assert_eq!(
        weekly_limit_for_owner(&state, "history-user", "2026-W15").await,
        350_000
    );
}

#[tokio::test]
async fn rebalance_before_period_starts_behaves_like_full_plan() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", issue_access_token("before-user", "user"))
        .body_json(&json!({
            "total_budget": 700_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-03-20"
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("spent_so_far").assert_i64(0);
    data.get("remaining_budget").assert_i64(700_000);
    data.get("rebalance_from_week").assert_string("2026-W13");
    let weeks = data.get("weeks").array();
    assert_eq!(weeks.len(), 2);
    weeks
        .get(0)
        .object()
        .get("week_key")
        .assert_string("2026-W13");
    weeks
        .get(0)
        .object()
        .get("weekly_limit")
        .assert_i64(350_000);
    weeks
        .get(0)
        .object()
        .get("projected_remaining")
        .assert_i64(350_000);
    weeks
        .get(1)
        .object()
        .get("week_key")
        .assert_string("2026-W14");
    weeks
        .get(1)
        .object()
        .get("weekly_limit")
        .assert_i64(350_000);
    weeks
        .get(1)
        .object()
        .get("projected_remaining")
        .assert_i64(350_000);
}

#[tokio::test]
async fn rebalance_rejects_as_of_after_period_end() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));

    cli.post("/budget/rebalance")
        .header("Authorization", issue_access_token("after-user", "user"))
        .body_json(&json!({
            "total_budget": 700_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-04-06"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rebalance_clamps_saved_limits_to_zero_when_remaining_budget_is_negative() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("overspent-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 100_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "alert_threshold": 0.85
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", &token)
        .body_json(&json!({
            "amount": 150_000,
            "merchant": "overspent",
            "transacted_at": "2026-03-25T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .post("/budget/rebalance")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 100_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-03-26"
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("remaining_budget").assert_i64(-50_000);
    data.get("is_overspent").assert_bool(true);
    let weeks = data.get("weeks").array();
    weeks.get(0).object().get("weekly_limit").assert_i64(0);
    weeks.get(1).object().get("weekly_limit").assert_i64(0);
    weeks
        .get(0)
        .object()
        .get("projected_remaining")
        .assert_i64(-18_182);
    weeks
        .get(1)
        .object()
        .get("projected_remaining")
        .assert_i64(-31_818);

    assert_eq!(
        weekly_limit_for_owner(&state, "overspent-user", "2026-W13").await,
        0
    );
    assert_eq!(
        weekly_limit_for_owner(&state, "overspent-user", "2026-W14").await,
        0
    );
    assert_eq!(
        projected_remaining_for_owner(&state, "overspent-user", "2026-W13").await,
        -18_182
    );
    assert_eq!(
        projected_remaining_for_owner(&state, "overspent-user", "2026-W14").await,
        -31_818
    );
}

#[tokio::test]
async fn weekly_summary_by_key_returns_projected_remaining_after_rebalance() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));
    let token = issue_access_token("weekly-summary-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 100_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "alert_threshold": 0.85
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", &token)
        .body_json(&json!({
            "amount": 150_000,
            "merchant": "overspent",
            "transacted_at": "2026-03-25T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    cli.post("/budget/rebalance")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 100_000,
            "from_date": "2026-03-23",
            "to_date": "2026-04-05",
            "as_of_date": "2026-03-26"
        }))
        .send()
        .await
        .assert_status_is_ok();

    let response = cli
        .get("/budget/weekly/2026-W13")
        .header("Authorization", &token)
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    json.value()
        .object()
        .get("week_key")
        .assert_string("2026-W13");
    json.value().object().get("weekly_limit").assert_i64(0);
    json.value().object().get("total_spent").assert_i64(150_000);
    json.value().object().get("remaining").assert_i64(-150_000);
    json.value()
        .object()
        .get("projected_remaining")
        .assert_i64(-18_182);
}
