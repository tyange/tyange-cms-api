use std::{env, sync::Arc};

use poem::{http::StatusCode, post, test::TestClient, Endpoint, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    db::init_db,
    middlewares::auth_middleware::Auth,
    models::AppState,
    routes::{
        add_user::create_user, create_match::create_match,
        create_match_message::create_match_message, delete_my_match::delete_my_match,
        get_match_messages::get_match_messages, get_my_match::get_my_match,
        respond_match::respond_match,
    },
};
use tyange_cms_api::auth::jwt::Claims;

async fn create_test_state() -> Arc<AppState> {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");
    Arc::new(AppState::new(db))
}

fn create_match_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at("/match/request", post(create_match).with(Auth))
        .at(
            "/match/me",
            poem::get(get_my_match).delete(delete_my_match).with(Auth),
        )
        .at(
            "/match/messages",
            poem::get(get_match_messages)
                .post(create_match_message)
                .with(Auth),
        )
        .at("/match/:match_id/respond", post(respond_match).with(Auth))
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

async fn seed_user(state: &Arc<AppState>, user_id: &str) {
    create_user(&state.db, user_id, "password123", "user")
        .await
        .expect("failed to create user");
}

#[tokio::test]
async fn create_match_succeeds_for_idle_users() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    let response = cli
        .post("/match/request")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "bob@example.com"
        }))
        .send()
        .await;

    response.assert_status(StatusCode::CREATED);
    let json = response.json().await;
    let data = json.value().object().get("data").object();
    data.get("status").assert_string("pending");
    data.get("requester_user_id")
        .assert_string("alice@example.com");
    data.get("target_user_id").assert_string("bob@example.com");

    let stored_status: String = query_scalar("SELECT status FROM user_matches WHERE match_id = 1")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch stored status");
    assert_eq!(stored_status, "pending");
}

#[tokio::test]
async fn create_match_rejects_self_request() {
    let state = create_test_state().await;
    seed_user(&state, "solo@example.com").await;
    let cli = TestClient::new(create_match_app(state));

    cli.post("/match/request")
        .header(
            "Authorization",
            issue_access_token("solo@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "solo@example.com"
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_match_rejects_missing_target_user() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    let cli = TestClient::new(create_match_app(state));

    cli.post("/match/request")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "missing@example.com"
        }))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_match_rejects_when_either_user_already_has_active_match() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    seed_user(&state, "charlie@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed active match");

    cli.post("/match/request")
        .header(
            "Authorization",
            issue_access_token("charlie@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "bob@example.com"
        }))
        .send()
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn reverse_duplicate_request_is_rejected() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed active match");

    cli.post("/match/request")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "alice@example.com"
        }))
        .send()
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn only_target_user_can_respond_to_pending_match() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    seed_user(&state, "charlie@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed pending match");

    cli.post("/match/1/respond")
        .header(
            "Authorization",
            issue_access_token("charlie@example.com", "user"),
        )
        .body_json(&json!({ "action": "accept" }))
        .send()
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn accepting_match_updates_status_and_responded_at() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed pending match");

    let response = cli
        .post("/match/1/respond")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .body_json(&json!({ "action": "accept" }))
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    json.value()
        .object()
        .get("data")
        .object()
        .get("status")
        .assert_string("matched");

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, responded_at FROM user_matches WHERE match_id = 1")
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch updated match");
    assert_eq!(row.0, "matched");
    assert!(row.1.is_some());
}

#[tokio::test]
async fn get_my_match_returns_same_active_match_for_both_participants() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status, responded_at) VALUES (?, ?, 'matched', ?)",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .bind("2026-03-19 10:00:00")
    .execute(&state.db)
    .await
    .expect("failed to seed matched row");

    let alice_response = cli
        .get("/match/me")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .send()
        .await;
    alice_response.assert_status_is_ok();
    let alice_json = alice_response.json().await;
    let alice_data = alice_json.value().object().get("data").object();
    alice_data.get("match_id").assert_i64(1);
    alice_data
        .get("counterpart_user_id")
        .assert_string("bob@example.com");

    let bob_response = cli
        .get("/match/me")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .send()
        .await;
    bob_response.assert_status_is_ok();
    let bob_json = bob_response.json().await;
    let bob_data = bob_json.value().object().get("data").object();
    bob_data.get("match_id").assert_i64(1);
    bob_data
        .get("counterpart_user_id")
        .assert_string("alice@example.com");
}

#[tokio::test]
async fn delete_my_match_cancels_pending_request() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed pending match");

    cli.delete("/match/me")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .send()
        .await
        .assert_status_is_ok();

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, closed_at FROM user_matches WHERE match_id = 1")
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch cancelled row");
    assert_eq!(row.0, "cancelled");
    assert!(row.1.is_some());
}

#[tokio::test]
async fn delete_my_match_unmatches_confirmed_pair_and_allows_rematch() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status, responded_at) VALUES (?, ?, 'matched', ?)",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .bind("2026-03-19 10:00:00")
    .execute(&state.db)
    .await
    .expect("failed to seed matched row");

    cli.delete("/match/me")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .send()
        .await
        .assert_status_is_ok();

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, closed_at FROM user_matches WHERE match_id = 1")
            .fetch_one(&state.db)
            .await
            .expect("failed to fetch unmatched row");
    assert_eq!(row.0, "unmatched");
    assert!(row.1.is_some());

    let second = cli
        .post("/match/request")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "target_user_id": "bob@example.com"
        }))
        .send()
        .await;
    second.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn protected_match_routes_require_authentication() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_match_app(state));

    cli.get("/match/me")
        .send()
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn matched_user_can_create_and_list_messages() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status, responded_at) VALUES (?, ?, 'matched', ?)",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .bind("2026-03-19 10:00:00")
    .execute(&state.db)
    .await
    .expect("failed to seed matched row");

    let create_response = cli
        .post("/match/messages")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "content": "hello bob"
        }))
        .send()
        .await;
    create_response.assert_status(StatusCode::CREATED);

    let stored: (String, String) = sqlx::query_as(
        "SELECT sender_user_id, receiver_user_id FROM match_messages WHERE match_id = 1 LIMIT 1",
    )
    .fetch_one(&state.db)
    .await
    .expect("failed to fetch stored message");
    assert_eq!(stored.0, "alice@example.com");
    assert_eq!(stored.1, "bob@example.com");

    let list_response = cli
        .get("/match/messages")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .send()
        .await;
    list_response.assert_status_is_ok();

    let list_json = list_response.json().await;
    let data = list_json.value().object().get("data").object();
    data.get("match_id").assert_i64(1);
    data.get("counterpart_user_id")
        .assert_string("alice@example.com");
    data.get("messages").array().assert_contains(|value| {
        let object = value.object();
        object.get("sender_user_id").string() == "alice@example.com"
            && object.get("receiver_user_id").string() == "bob@example.com"
            && object.get("content").string() == "hello bob"
    });
}

#[tokio::test]
async fn users_without_confirmed_match_cannot_create_or_view_messages() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status) VALUES (?, ?, 'pending')",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .execute(&state.db)
    .await
    .expect("failed to seed pending row");

    cli.post("/match/messages")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "content": "not allowed yet"
        }))
        .send()
        .await
        .assert_status(StatusCode::FORBIDDEN);

    cli.get("/match/messages")
        .header(
            "Authorization",
            issue_access_token("bob@example.com", "user"),
        )
        .send()
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn empty_message_content_is_rejected() {
    let state = create_test_state().await;
    seed_user(&state, "alice@example.com").await;
    seed_user(&state, "bob@example.com").await;
    let cli = TestClient::new(create_match_app(state.clone()));

    query(
        "INSERT INTO user_matches (requester_user_id, target_user_id, status, responded_at) VALUES (?, ?, 'matched', ?)",
    )
    .bind("alice@example.com")
    .bind("bob@example.com")
    .bind("2026-03-19 10:00:00")
    .execute(&state.db)
    .await
    .expect("failed to seed matched row");

    cli.post("/match/messages")
        .header(
            "Authorization",
            issue_access_token("alice@example.com", "user"),
        )
        .body_json(&json!({
            "content": "   "
        }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}
