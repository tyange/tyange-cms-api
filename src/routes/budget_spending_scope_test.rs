use std::{env, sync::Arc};

use bcrypt::{hash, DEFAULT_COST};
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
        get_budget::get_budget,
        get_spending::get_spending,
        update_active_budget::update_active_budget,
        update_spending::update_spending,
    },
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
        .at(
            "/budget",
            get(get_budget.with(Auth)).put(update_active_budget.with(Auth)),
        )
        .at("/budget/plan", post(create_budget_plan).with(Auth))
        .at(
            "/budget/spending",
            get(get_spending.with(Auth)).post(create_spending.with(JwtOrApiKeyAuth)),
        )
        .at(
            "/budget/spending/:record_id",
            put(update_spending).delete(delete_spending).with(Auth),
        )
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

#[tokio::test]
async fn migrates_legacy_spending_rows_to_admin_owner_and_drops_week_key() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");

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
        "INSERT INTO spending_records (amount, merchant, transacted_at, week_key)
         VALUES (12000, 'legacy-store', '2026-03-03 12:00:00', '2026-W10')",
    )
    .execute(&db)
    .await
    .expect("failed to seed legacy spending");

    init_db(&db).await.expect("failed to migrate db");

    let owner: String =
        query_scalar("SELECT owner_user_id FROM spending_records ORDER BY record_id DESC LIMIT 1")
            .fetch_one(&db)
            .await
            .expect("failed to fetch migrated spending owner");
    assert_eq!(owner, "admin");

    let week_column_exists: i64 = query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('spending_records') WHERE name = 'week_key'",
    )
    .fetch_one(&db)
    .await
    .expect("failed to inspect spending_records schema");
    assert_eq!(week_column_exists, 0);
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
async fn migrates_budget_periods_to_add_snapshot_total_spent_column() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");

    query(
        r#"
        CREATE TABLE budget_periods (
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
    .execute(&db)
    .await
    .expect("failed to create legacy budget_periods");

    init_db(&db).await.expect("failed to migrate db");

    let column_exists: i64 = query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('budget_periods') WHERE name = 'snapshot_total_spent'",
    )
    .fetch_one(&db)
    .await
    .expect("failed to inspect budget_periods schema");

    assert_eq!(column_exists, 1);
}

#[tokio::test]
async fn budget_summary_uses_latest_budget_period_and_owner_scope() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-03-01",
            "to_date": "2026-03-31",
            "alert_threshold": 0.8
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({
            "total_budget": 2000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "alert_threshold": 0.75
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("user-2", "user"))
        .body_json(&json!({
            "total_budget": 3000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "alert_threshold": 0.9
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({
            "amount": 400,
            "merchant": "lunch",
            "transacted_at": "2026-04-02T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .get("/budget")
        .header("Authorization", issue_access_token("user-1", "user"))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    json.value().object().get("total_budget").assert_i64(2000);
    json.value().object().get("from_date").assert_string("2026-04-01");
    json.value().object().get("to_date").assert_string("2026-04-30");
    json.value().object().get("total_spent").assert_i64(400);
    json.value().object().get("remaining_budget").assert_i64(1600);
    json.value().object().get("usage_rate").assert_f64(0.2);
    json.value().object().get("alert").assert_bool(false);
    json.value().object().get("alert_threshold").assert_f64(0.75);
    json.value().object().get("is_overspent").assert_bool(false);
}

#[tokio::test]
async fn create_budget_plan_accepts_total_spent_snapshot() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("snapshot-create-user", "user");

    let response = cli
        .post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1500,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "total_spent": 400,
            "alert_threshold": 0.9
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("budget_id").assert_i64(1);
    data.get("total_budget").assert_i64(1500);
    data.get("from_date").assert_string("2026-04-01");
    data.get("to_date").assert_string("2026-04-30");
    data.get("total_spent").assert_i64(400);
    data.get("remaining_budget").assert_i64(1100);
    data.get("usage_rate").assert_f64(0.267);
    data.get("alert").assert_bool(false);
    data.get("alert_threshold").assert_f64(0.9);
    data.get("is_overspent").assert_bool(false);

    let budget = cli
        .get("/budget")
        .header("Authorization", &token)
        .send()
        .await;
    budget.assert_status_is_ok();
    let budget_json = budget.json().await;
    budget_json.value().object().get("total_spent").assert_i64(400);
    budget_json.value().object().get("remaining_budget").assert_i64(1100);
}

#[tokio::test]
async fn create_budget_plan_without_total_spent_uses_spending_sum() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("sum-create-user", "user");

    query(
        "INSERT INTO spending_records (owner_user_id, amount, merchant, transacted_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind("sum-create-user")
    .bind(250_i64)
    .bind("seeded")
    .bind("2026-04-10 09:00:00")
    .execute(&state.db)
    .await
    .expect("failed to seed spending");

    let response = cli
        .post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("total_spent").assert_i64(250);
    data.get("remaining_budget").assert_i64(750);
    data.get("usage_rate").assert_f64(0.25);
}

#[tokio::test]
async fn create_spending_rejects_dates_outside_active_period() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("period-user", "user"))
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("period-user", "user"))
        .body_json(&json!({
            "amount": 100,
            "merchant": "blocked",
            "transacted_at": "2026-05-01T00:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_active_budget_changes_total_budget_and_budget_summary() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("budget-update-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "alert_threshold": 0.8
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", &token)
        .body_json(&json!({
            "amount": 400,
            "merchant": "lunch",
            "transacted_at": "2026-04-02T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .put("/budget")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1500,
            "alert_threshold": 0.9
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("total_budget").assert_i64(1500);
    data.get("total_spent").assert_i64(400);
    data.get("remaining_budget").assert_i64(1100);
    data.get("usage_rate").assert_f64(0.267);
    data.get("alert").assert_bool(false);
    data.get("alert_threshold").assert_f64(0.9);
    data.get("is_overspent").assert_bool(false);

    let budget_response = cli
        .get("/budget")
        .header("Authorization", &token)
        .send()
        .await;
    budget_response.assert_status_is_ok();
    let budget = budget_response.json().await;
    budget.value().object().get("total_budget").assert_i64(1500);
    budget.value().object().get("total_spent").assert_i64(400);
    budget.value().object().get("remaining_budget").assert_i64(1100);
    budget.value().object().get("usage_rate").assert_f64(0.267);
    budget.value().object().get("alert").assert_bool(false);
    budget.value().object().get("alert_threshold").assert_f64(0.9);
    budget.value().object().get("is_overspent").assert_bool(false);
}

#[tokio::test]
async fn update_active_budget_accepts_total_spent_snapshot() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("snapshot-update-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
        .send()
        .await
        .assert_status_is_ok();

    let response = cli
        .put("/budget")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1800,
            "total_spent": 400,
            "alert_threshold": 0.9
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("total_spent").assert_i64(400);
    data.get("remaining_budget").assert_i64(1400);
    data.get("usage_rate").assert_f64(0.222);
    data.get("alert").assert_bool(false);
    data.get("is_overspent").assert_bool(false);

    let budget = cli
        .get("/budget")
        .header("Authorization", &token)
        .send()
        .await;
    budget.assert_status_is_ok();
    let budget_json = budget.json().await;
    budget_json.value().object().get("total_spent").assert_i64(400);
    budget_json.value().object().get("remaining_budget").assert_i64(1400);
}

#[tokio::test]
async fn update_active_budget_without_total_spent_keeps_spending_sum() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("sum-update-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", &token)
        .body_json(&json!({
            "amount": 300,
            "merchant": "dinner",
            "transacted_at": "2026-04-03T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::CREATED);

    let response = cli
        .put("/budget")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1200
        }))
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("total_spent").assert_i64(300);
    data.get("remaining_budget").assert_i64(900);
}

#[tokio::test]
async fn budget_summary_marks_overspent_when_total_spent_exceeds_total_budget() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));
    let token = issue_access_token("overspent-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 500,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "total_spent": 700,
            "alert_threshold": 0.9
        }))
        .send()
        .await
        .assert_status_is_ok();

    let response = cli
        .get("/budget")
        .header("Authorization", &token)
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    json.value().object().get("total_spent").assert_i64(700);
    json.value().object().get("remaining_budget").assert_i64(-200);
    json.value().object().get("usage_rate").assert_f64(1.4);
    json.value().object().get("alert").assert_bool(true);
    json.value().object().get("is_overspent").assert_bool(true);
}

#[tokio::test]
async fn update_active_budget_returns_not_found_without_active_period() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state));

    cli.put("/budget")
        .header("Authorization", issue_access_token("no-budget-user", "user"))
        .body_json(&json!({
            "total_budget": 1500
        }))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn spending_groups_records_by_iso_week_and_matches_totals() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    let token = issue_access_token("group-user", "user");

    cli.post("/budget/plan")
        .header("Authorization", &token)
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-03-30",
            "to_date": "2026-04-12"
        }))
        .send()
        .await
        .assert_status_is_ok();

    for (amount, merchant, transacted_at) in [
        (100, "a", "2026-03-30T12:00:00"),
        (200, "b", "2026-04-01T09:00:00"),
        (50, "c", "2026-04-08T10:00:00"),
    ] {
        cli.post("/budget/spending")
            .header("Authorization", &token)
            .body_json(&json!({
                "amount": amount,
                "merchant": merchant,
                "transacted_at": transacted_at
            }))
            .send()
            .await
            .assert_status(StatusCode::CREATED);
    }

    let response = cli
        .get("/budget/spending")
        .header("Authorization", &token)
        .send()
        .await;
    response.assert_status_is_ok();

    let json = response.json().await;
    json.value().object().get("total_spent").assert_i64(350);
    json.value().object().get("remaining_budget").assert_i64(650);
    let weeks = json.value().object().get("weeks").array();
    assert_eq!(weeks.len(), 2);
    weeks.get(0).object().get("week_key").assert_string("2026-W15");
    weeks.get(0).object().get("weekly_total").assert_i64(50);
    weeks.get(0).object().get("record_count").assert_i64(1);
    weeks.get(1).object().get("week_key").assert_string("2026-W14");
    weeks.get(1).object().get("weekly_total").assert_i64(300);
    weeks.get(1).object().get("record_count").assert_i64(2);
}

#[tokio::test]
async fn update_and_delete_require_record_owner() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("owner-1", "user"))
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/budget/spending")
        .header("Authorization", issue_access_token("owner-1", "user"))
        .body_json(&json!({
            "amount": 100,
            "merchant": "groceries",
            "transacted_at": "2026-04-05T12:00:00"
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

    cli.put(&format!("/budget/spending/{}", record_id))
        .header("Authorization", issue_access_token("other-user", "user"))
        .body_json(&json!({
            "amount": 300,
            "merchant": "blocked",
            "transacted_at": "2026-04-06T12:00:00"
        }))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);

    cli.delete(&format!("/budget/spending/{}", record_id))
        .header("Authorization", issue_access_token("other-user", "user"))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_key_can_create_spending_for_its_owner_and_updates_last_used_at() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_budget_app(state.clone()));
    create_user(&state.db, "macro-user", "password", "user")
        .await
        .expect("failed to create user");

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("macro-user", "user"))
        .body_json(&json!({
            "total_budget": 1000,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
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
            "transacted_at": "2026-04-02T12:00:00"
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

    cli.post("/budget/plan")
        .header("Authorization", issue_access_token("owner", "user"))
        .body_json(&json!({
            "total_budget": 1500,
            "from_date": "2026-04-01",
            "to_date": "2026-04-30"
        }))
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
            "transacted_at": "2026-04-02T12:00:00"
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
