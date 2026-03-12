use std::{env, sync::Arc};

use poem::{delete, http::StatusCode, post, put, test::TestClient, Endpoint, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    blog_redeploy::{BlogContentEvent, BlogRedeployService, BlogVisibility, MockDispatchFailure},
    db::init_db,
    middlewares::admin_middleware::AdminOnly,
    middlewares::auth_middleware::Auth,
    models::AppState,
    routes::{
        add_user::add_user, delete_post::delete_post, get_all_posts::get_all_posts,
        update_post::update_post, upload_post::upload_post,
    },
};
use tyange_cms_api::auth::jwt::Claims;

async fn create_test_state() -> Arc<AppState> {
    let (blog_redeploy, _) = BlogRedeployService::mock();
    create_test_state_with_redeploy(blog_redeploy).await
}

async fn create_test_state_with_redeploy(blog_redeploy: BlogRedeployService) -> Arc<AppState> {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");

    query(
        r#"
        CREATE TABLE IF NOT EXISTS tags (
            tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create tags");

    query(
        r#"
        CREATE TABLE IF NOT EXISTS post_tags (
            post_id TEXT NOT NULL,
            tag_id INTEGER NOT NULL,
            FOREIGN KEY (post_id) REFERENCES posts(post_id),
            FOREIGN KEY (tag_id) REFERENCES tags(tag_id)
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("failed to create post_tags");

    Arc::new(AppState::new_with_blog_redeploy(db, blog_redeploy))
}

fn create_test_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at("/post/upload", post(upload_post).with(Auth))
        .at("/post/update/:post_id", put(update_post).with(Auth))
        .at("/post/delete/:post_id", delete(delete_post).with(Auth))
        .at("/admin/add-user", post(add_user).with(AdminOnly).with(Auth))
        .at(
            "/admin/posts",
            poem::get(get_all_posts).with(AdminOnly).with(Auth),
        )
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

#[tokio::test]
async fn upload_post_saves_writer_id_from_authenticated_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_test_app(state.clone()));

    let response = cli
        .post("/post/upload")
        .header("Authorization", issue_access_token("writer-1", "user"))
        .body_json(&json!({
            "title": "first post",
            "description": "desc",
            "published_at": "2026-03-07T00:00:00Z",
            "tags": [],
            "content": "body",
            "status": "draft"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let writer_id: String = query_scalar("SELECT writer_id FROM posts WHERE title = ?")
        .bind("first post")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch writer_id");

    assert_eq!(writer_id, "writer-1");
}

#[tokio::test]
async fn update_post_rejects_authenticated_user_who_is_not_owner() {
    let state = create_test_state().await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-1")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-1")
    .bind("draft")
    .execute(&state.db)
    .await
    .expect("failed to seed post");

    let cli = TestClient::new(create_test_app(state));
    let response = cli
        .put("/post/update/post-1")
        .header("Authorization", issue_access_token("other-user", "user"))
        .body_json(&json!({
            "title": "updated",
            "description": "updated desc",
            "published_at": "2026-03-08T00:00:00Z",
            "tags": [],
            "content": "updated content",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn update_post_allows_owner_and_persists_changes() {
    let state = create_test_state().await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-2")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-2")
    .bind("draft")
    .execute(&state.db)
    .await
    .expect("failed to seed post");

    let cli = TestClient::new(create_test_app(state.clone()));
    let response = cli
        .put("/post/update/post-2")
        .header("Authorization", issue_access_token("owner-2", "user"))
        .body_json(&json!({
            "title": "updated",
            "description": "updated desc",
            "published_at": "2026-03-08T00:00:00Z",
            "tags": [{ "tag": "rust", "category": "tech" }],
            "content": "updated content",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let updated_title: String = query_scalar("SELECT title FROM posts WHERE post_id = ?")
        .bind("post-2")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch updated title");
    let tag_count: i64 = query_scalar("SELECT COUNT(*) FROM post_tags WHERE post_id = ?")
        .bind("post-2")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch tag count");

    assert_eq!(updated_title, "updated");
    assert_eq!(tag_count, 1);
}

#[tokio::test]
async fn delete_post_returns_not_found_when_post_does_not_exist() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_test_app(state));

    let response = cli
        .delete("/post/delete/missing-post")
        .header("Authorization", issue_access_token("writer-1", "user"))
        .send()
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_post_allows_admin_to_manage_other_users_post() {
    let state = create_test_state().await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-3")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-3")
    .bind("draft")
    .execute(&state.db)
    .await
    .expect("failed to seed post");

    let cli = TestClient::new(create_test_app(state.clone()));
    let response = cli
        .put("/post/update/post-3")
        .header("Authorization", issue_access_token("admin-1", "admin"))
        .body_json(&json!({
            "title": "updated by admin",
            "description": "updated desc",
            "published_at": "2026-03-08T00:00:00Z",
            "tags": [],
            "content": "updated content",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status_is_ok();
}

#[tokio::test]
async fn admin_route_rejects_non_admin_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_test_app(state));

    let response = cli
        .post("/admin/add-user")
        .header("Authorization", issue_access_token("user-1", "user"))
        .body_json(&json!({
            "user_id": "new-user",
            "password": "password",
            "user_role": "user"
        }))
        .send()
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_route_allows_admin_user() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_test_app(state.clone()));

    let response = cli
        .post("/admin/add-user")
        .header("Authorization", issue_access_token("admin-1", "admin"))
        .body_json(&json!({
            "user_id": "new-admin-user",
            "password": "password",
            "user_role": "user"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let created_role: String = query_scalar("SELECT user_role FROM users WHERE user_id = ?")
        .bind("new-admin-user")
        .fetch_one(&state.db)
        .await
        .expect("failed to fetch created user");

    assert_eq!(created_role, "user");
}

#[tokio::test]
async fn visible_publish_triggers_dispatch_once() {
    let (blog_redeploy, mock_handle) = BlogRedeployService::mock();
    let state = create_test_state_with_redeploy(blog_redeploy).await;
    let cli = TestClient::new(create_test_app(state));

    let response = cli
        .post("/post/upload")
        .header(
            "Authorization",
            issue_access_token("writer-visible", "user"),
        )
        .body_json(&json!({
            "title": "published post",
            "description": "desc",
            "published_at": "2026-03-12T00:00:00Z",
            "tags": [],
            "content": "body",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let calls = mock_handle.take_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].content_event, BlogContentEvent::Publish);
    assert_eq!(calls[0].visibility, BlogVisibility::Visible);
    assert!(!calls[0].post_id.is_empty());
}

#[tokio::test]
async fn visible_update_triggers_dispatch_once() {
    let (blog_redeploy, mock_handle) = BlogRedeployService::mock();
    let state = create_test_state_with_redeploy(blog_redeploy).await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-visible-update")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-visible")
    .bind("published")
    .execute(&state.db)
    .await
    .expect("failed to seed published post");

    let cli = TestClient::new(create_test_app(state));
    let response = cli
        .put("/post/update/post-visible-update")
        .header("Authorization", issue_access_token("owner-visible", "user"))
        .body_json(&json!({
            "title": "title updated",
            "description": "description",
            "published_at": "2026-03-07T00:00:00Z",
            "tags": [],
            "content": "content",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let calls = mock_handle.take_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].content_event, BlogContentEvent::Update);
    assert_eq!(calls[0].post_id, "post-visible-update");
    assert_eq!(calls[0].visibility, BlogVisibility::Visible);
}

#[tokio::test]
async fn draft_only_update_does_not_trigger_dispatch() {
    let (blog_redeploy, mock_handle) = BlogRedeployService::mock();
    let state = create_test_state_with_redeploy(blog_redeploy).await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-draft-update")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-draft")
    .bind("draft")
    .execute(&state.db)
    .await
    .expect("failed to seed draft post");

    let cli = TestClient::new(create_test_app(state));
    let response = cli
        .put("/post/update/post-draft-update")
        .header("Authorization", issue_access_token("owner-draft", "user"))
        .body_json(&json!({
            "title": "title updated",
            "description": "description updated",
            "published_at": "2026-03-08T00:00:00Z",
            "tags": [],
            "content": "content updated",
            "status": "draft"
        }))
        .send()
        .await;

    response.assert_status_is_ok();
    assert!(mock_handle.take_calls().await.is_empty());
}

#[tokio::test]
async fn dispatch_failure_does_not_rollback_visible_publish() {
    let (blog_redeploy, mock_handle) = BlogRedeployService::mock();
    mock_handle
        .fail_next(MockDispatchFailure {
            content_event: BlogContentEvent::Publish,
            post_id: "post-will-be-created".to_string(),
            visibility: BlogVisibility::Visible,
            status: Some(500),
            message: "mock github failure".to_string(),
        })
        .await;

    let state = create_test_state_with_redeploy(blog_redeploy).await;
    let cli = TestClient::new(create_test_app(state.clone()));

    let response = cli
        .post("/post/upload")
        .header(
            "Authorization",
            issue_access_token("writer-failure", "user"),
        )
        .body_json(&json!({
            "title": "publish despite dispatch failure",
            "description": "desc",
            "published_at": "2026-03-12T00:00:00Z",
            "tags": [],
            "content": "body",
            "status": "published"
        }))
        .send()
        .await;

    response.assert_status_is_ok();

    let saved_count: i64 = query_scalar("SELECT COUNT(*) FROM posts WHERE title = ?")
        .bind("publish despite dispatch failure")
        .fetch_one(&state.db)
        .await
        .expect("failed to count saved posts");
    assert_eq!(saved_count, 1);

    let calls = mock_handle.take_calls().await;
    let failures = mock_handle.take_failures().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].content_event, BlogContentEvent::Publish);
    assert_eq!(failures[0].visibility, BlogVisibility::Visible);
}

#[tokio::test]
async fn visible_delete_triggers_dispatch_once() {
    let (blog_redeploy, mock_handle) = BlogRedeployService::mock();
    let state = create_test_state_with_redeploy(blog_redeploy).await;
    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-visible-delete")
    .bind("title")
    .bind("description")
    .bind("2026-03-07T00:00:00Z")
    .bind("content")
    .bind("owner-delete")
    .bind("published")
    .execute(&state.db)
    .await
    .expect("failed to seed published post");

    let cli = TestClient::new(create_test_app(state.clone()));
    let response = cli
        .delete("/post/delete/post-visible-delete")
        .header("Authorization", issue_access_token("owner-delete", "user"))
        .send()
        .await;

    response.assert_status_is_ok();

    let calls = mock_handle.take_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].content_event, BlogContentEvent::Delete);
    assert_eq!(calls[0].post_id, "post-visible-delete");
    assert_eq!(calls[0].visibility, BlogVisibility::Hidden);
}
