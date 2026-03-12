use std::{env, sync::Arc};

use bcrypt::verify;
use poem::{get, http::StatusCode, post, test::TestClient, Endpoint, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, Row, SqlitePool};

use crate::{
    db::init_db,
    middlewares::admin_middleware::AdminOnly,
    middlewares::auth_middleware::Auth,
    models::AppState,
    routes::{
        add_user::add_user, login::login, login_google::login_google, me::me, signup::signup,
    },
};
use tyange_cms_api::auth::jwt::Claims;

async fn create_test_state() -> Arc<AppState> {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    env::set_var("JWT_REFRESH_SECRET", "test-refresh-secret");
    env::set_var("GOOGLE_CLIENT_ID", "test-google-client-id");
    env::set_var("ALLOW_FAKE_GOOGLE_ID_TOKEN_FOR_TESTS", "true");

    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");
    Arc::new(AppState::new(db))
}

fn create_auth_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at("/signup", post(signup))
        .at("/login", post(login))
        .at("/login/google", post(login_google))
        .at("/me", get(me).with(Auth))
        .at("/admin/add-user", post(add_user).with(AdminOnly).with(Auth))
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

fn fake_google_id_token(
    email: &str,
    google_sub: &str,
    overrides: serde_json::Value,
) -> serde_json::Value {
    let mut token = json!({
        "aud": "test-google-client-id",
        "iss": "https://accounts.google.com",
        "sub": google_sub,
        "email": email,
        "email_verified": true,
        "exp": (chrono::Utc::now().timestamp() + 3600).to_string()
    });

    if let Some(object) = overrides.as_object() {
        for (key, value) in object {
            token[key] = value.clone();
        }
    }

    token
}

#[tokio::test]
async fn signup_creates_user_role_account_with_hashed_password() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state.clone()));

    let response = cli
        .post("/signup")
        .body_json(&json!({
            "email": "user@example.com",
            "password": "password123"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let stored_hash: String = query_scalar("SELECT password FROM users WHERE user_id = ?")
        .bind("user@example.com")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch stored password");
    let stored_role: String = query_scalar("SELECT user_role FROM users WHERE user_id = ?")
        .bind("user@example.com")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch stored role");

    assert_ne!(stored_hash, "password123");
    assert!(verify("password123", &stored_hash).expect("password verify should work"));
    assert_eq!(stored_role, "user");
}

#[tokio::test]
async fn signup_rejects_duplicate_email() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/signup")
        .body_json(&json!({
            "email": "duplicate@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/signup")
        .body_json(&json!({
            "email": "duplicate@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn signup_rejects_invalid_email_and_short_password() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/signup")
        .body_json(&json!({
            "email": "invalid-email",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);

    cli.post("/signup")
        .body_json(&json!({
            "email": "valid@example.com",
            "password": "short"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn signup_user_can_login_with_email_as_user_id() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/signup")
        .body_json(&json!({
            "email": "login@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status_is_ok();

    let response = cli
        .post("/login")
        .body_json(&json!({
            "user_id": "login@example.com",
            "password": "password123"
        }))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    json.value().object().get("user_role").assert_string("user");
    json.value().object().get("access_token").assert_not_null();
    json.value().object().get("refresh_token").assert_not_null();
}

#[tokio::test]
async fn login_and_me_return_current_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/signup")
        .body_json(&json!({
            "email": "me@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status_is_ok();

    let login_response = cli
        .post("/login")
        .body_json(&json!({
            "user_id": "me@example.com",
            "password": "password123"
        }))
        .send()
        .await;

    login_response.assert_status_is_ok();
    let login_json = login_response.json().await;
    let access_token = login_json
        .value()
        .object()
        .get("access_token")
        .string()
        .to_string();

    let me_response = cli
        .get("/me")
        .header("Authorization", access_token)
        .send()
        .await;

    me_response.assert_status_is_ok();
    let me_json = me_response.json().await;
    me_json
        .value()
        .object()
        .get("user_id")
        .assert_string("me@example.com");
    me_json
        .value()
        .object()
        .get("user_role")
        .assert_string("user");
}

#[tokio::test]
async fn google_login_creates_new_user_and_returns_tokens() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state.clone()));

    let response = cli
        .post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("google-user@example.com", "google-sub-1", json!({})).to_string()
        }))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    json.value().object().get("user_role").assert_string("user");
    json.value().object().get("access_token").assert_not_null();
    json.value().object().get("refresh_token").assert_not_null();

    let row =
        query("SELECT user_id, password, auth_provider, google_sub FROM users WHERE user_id = ?")
            .bind("google-user@example.com")
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch google user");

    let password: Option<String> = row
        .try_get("password")
        .expect("password column should exist");
    let auth_provider: String = row
        .try_get("auth_provider")
        .expect("auth_provider column should exist");
    let google_sub: String = row.try_get("google_sub").expect("google_sub should exist");

    assert_eq!(password, None);
    assert_eq!(auth_provider, "google");
    assert_eq!(google_sub, "google-sub-1");
}

#[tokio::test]
async fn google_login_links_existing_local_user_without_duplicate() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state.clone()));

    cli.post("/signup")
        .body_json(&json!({
            "email": "linked@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("linked@example.com", "google-sub-2", json!({})).to_string()
        }))
        .send()
        .await
        .assert_status_is_ok();

    let user_count: i64 =
        query_scalar("SELECT COUNT(*) FROM users WHERE lower(user_id) = lower(?)")
            .bind("linked@example.com")
            .fetch_one(&state.db)
            .await
            .expect("failed to count users");
    let linked_google_sub: String = query_scalar("SELECT google_sub FROM users WHERE user_id = ?")
        .bind("linked@example.com")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch linked google_sub");
    let auth_provider: String = query_scalar("SELECT auth_provider FROM users WHERE user_id = ?")
        .bind("linked@example.com")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch auth_provider");

    assert_eq!(user_count, 1);
    assert_eq!(linked_google_sub, "google-sub-2");
    assert_eq!(auth_provider, "local");
}

#[tokio::test]
async fn google_login_succeeds_for_existing_linked_google_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state.clone()));

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("repeat@example.com", "google-sub-repeat", json!({})).to_string()
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("repeat@example.com", "google-sub-repeat", json!({})).to_string()
        }))
        .send()
        .await
        .assert_status_is_ok();

    let user_count: i64 = query_scalar("SELECT COUNT(*) FROM users WHERE user_id = ?")
        .bind("repeat@example.com")
        .fetch_one(&state.db)
        .await
        .expect("failed to count repeat users");

    assert_eq!(user_count, 1);
}

#[tokio::test]
async fn google_login_rejects_invalid_audience() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token(
                "wrong-aud@example.com",
                "google-sub-wrong-aud",
                json!({"aud": "another-client-id"})
            ).to_string()
        }))
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn google_login_rejects_unverified_email() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token(
                "not-verified@example.com",
                "google-sub-not-verified",
                json!({"email_verified": false})
            ).to_string()
        }))
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn google_login_rejects_conflicting_google_subject() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("first@example.com", "shared-google-sub", json!({})).to_string()
        }))
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/login/google")
        .body_json(&json!({
            "id_token": fake_google_id_token("second@example.com", "shared-google-sub", json!({})).to_string()
        }))
        .send()
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn init_db_migrates_existing_users_table_for_google_login() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");

    query(
        r#"
        CREATE TABLE users (
            user_id TEXT PRIMARY KEY,
            password TEXT NOT NULL,
            user_role TEXT NOT NULL
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create legacy users table");

    query(
        r#"
        INSERT INTO users (user_id, password, user_role)
        VALUES ('legacy@example.com', 'hashed-password', 'user')
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to seed legacy user");

    init_db(&db).await.expect("failed to migrate database");

    let row = query("SELECT password, auth_provider, google_sub FROM users WHERE user_id = ?")
        .bind("legacy@example.com")
        .fetch_one(&db)
        .await
        .expect("failed to fetch migrated user");

    let password: Option<String> = row
        .try_get("password")
        .expect("password column should exist");
    let auth_provider: String = row
        .try_get("auth_provider")
        .expect("auth_provider column should exist");
    let google_sub: Option<String> = row
        .try_get("google_sub")
        .expect("google_sub column should exist");

    assert_eq!(password.as_deref(), Some("hashed-password"));
    assert_eq!(auth_provider, "local");
    assert_eq!(google_sub, None);
}

#[tokio::test]
async fn non_admin_still_cannot_use_admin_add_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_auth_app(state));

    let response = cli
        .post("/admin/add-user")
        .header(
            "Authorization",
            issue_access_token("member@example.com", "user"),
        )
        .body_json(&json!({
            "user_id": "created-by-user@example.com",
            "password": "password123",
            "user_role": "user"
        }))
        .send()
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}
