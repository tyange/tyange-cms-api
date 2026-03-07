use std::{env, sync::Arc};

use chrono::Local;
use poem::{get, post, put, http::StatusCode, test::TestClient, Endpoint, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    db::init_db,
    middlewares::auth_middleware::Auth,
    models::AppState,
    routes::{
        create_budget_plan::create_budget_plan, create_spending::create_spending,
        delete_spending::delete_spending, get_budget_weeks::get_budget_weeks,
        get_spending::get_spending, get_weekly_config::get_weekly_config,
        get_weekly_summary::get_weekly_summary, set_budget::set_budget,
        update_budget::update_budget, update_spending::update_spending,
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
        .at("/budget/weekly-config", get(get_weekly_config).with(Auth))
        .at("/budget/set", post(set_budget).with(Auth))
        .at("/budget/plan", post(create_budget_plan).with(Auth))
        .at("/budget/update/:config_id", put(update_budget).with(Auth))
        .at(
            "/budget/spending",
            get(get_spending).post(create_spending).with(Auth),
        )
        .at(
            "/budget/spending/:record_id",
            put(update_spending).delete(delete_spending).with(Auth),
        )
        .at("/budget/weekly", get(get_weekly_summary).with(Auth))
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

    let budget_owner: String = query_scalar(
        "SELECT owner_user_id FROM budget_config WHERE week_key = '2026-W10'",
    )
    .fetch_one(&db)
    .await
    .expect("failed to fetch migrated budget owner");
    let spending_owner: String = query_scalar(
        "SELECT owner_user_id FROM spending_records WHERE week_key = '2026-W10'",
    )
    .fetch_one(&db)
    .await
    .expect("failed to fetch migrated spending owner");

    assert_eq!(budget_owner, "admin");
    assert_eq!(spending_owner, "admin");
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
        .body_json(&json!({"amount": 250, "merchant": "lunch", "transacted_at": today_transacted_at()}))
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
    user1_json.value().object().get("week_key").assert_string(&current_week);
    user1_json.value().object().get("weekly_limit").assert_i64(1000);
    user1_json.value().object().get("total_spent").assert_i64(100);
    user1_json.value().object().get("remaining").assert_i64(900);
    user1_json.value().object().get("record_count").assert_i64(1);

    let user2_summary = cli
        .get("/budget/weekly")
        .header("Authorization", issue_access_token("user-2", "user"))
        .send()
        .await;
    user2_summary.assert_status_is_ok();
    let user2_json = user2_summary.json().await;
    user2_json.value().object().get("weekly_limit").assert_i64(2000);
    user2_json.value().object().get("total_spent").assert_i64(250);
    user2_json.value().object().get("remaining").assert_i64(1750);
    user2_json.value().object().get("record_count").assert_i64(1);

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

    let config_count: i64 = query_scalar(
        "SELECT COUNT(*) FROM budget_config WHERE week_key = ?",
    )
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
