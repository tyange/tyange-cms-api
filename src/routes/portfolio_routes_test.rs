use std::sync::Arc;

use poem::{get, http::StatusCode, test::TestClient, EndpointExt, Route};
use serde_json::json;
use sqlx::{query, query_scalar, SqlitePool};

use crate::{
    db::init_db,
    models::AppState,
    routes::{
        delete_portfolio::delete_portfolio, get_portfolio::get_portfolio,
        get_posts_with_tags::get_posts_with_tags, update_portfolio::update_portfolio,
    },
};

async fn create_state() -> Arc<AppState> {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");
    Arc::new(AppState::new(db))
}

#[tokio::test]
async fn get_portfolio_returns_seeded_document_and_put_updates_it() {
    let state = create_state().await;
    let cli = TestClient::new(
        Route::new()
            .at("/portfolio", get(get_portfolio).put(update_portfolio))
            .data(state),
    );

    let initial = cli.get("/portfolio").send().await;
    initial.assert_status_is_ok();

    let initial_json = initial.json().await;
    initial_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("identity")
        .object()
        .get("name")
        .assert_string("TYANGE");
    initial_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("featured_projects")
        .array()
        .get(0)
        .object()
        .get("title")
        .assert_string("tyange-blog");

    let updated = cli
        .put("/portfolio")
        .body_json(&json!({
            "content": {
                "slug": "dev",
                "version": 1,
                "identity": {
                    "name": "TYANGE",
                    "role": "Frontend developer",
                    "location": "Seoul",
                    "availability": "Open",
                    "email": "hello@tyange.dev",
                    "github_url": "https://github.com/tyange",
                    "blog_url": "https://blog.tyange.com",
                    "velog_url": "https://velog.io/@tyange"
                },
                "hero": {
                    "eyebrow": "eyebrow",
                    "headline": "updated headline",
                    "summary": "updated summary",
                    "primary_cta": { "label": "GitHub", "url": "https://github.com/tyange" },
                    "secondary_cta": { "label": "Blog", "url": "https://blog.tyange.com" }
                },
                "highlight_cards": [
                    { "label": "Focus", "title": "Interfaces" },
                    { "label": "Stack", "title": "Next.js" }
                ],
                "guiding_principle": "Every element earns its place.",
                "featured_projects": [],
                "about": {
                    "eyebrow": "About",
                    "headline": "About headline",
                    "paragraphs": ["A", "B"],
                    "services": ["UI"],
                    "strengths": ["Systems"]
                },
                "writing": {
                    "eyebrow": "Writing",
                    "title": "Dev posts",
                    "description": "Notes"
                }
            }
        }))
        .send()
        .await;

    updated.assert_status_is_ok();
    let updated_json = updated.json().await;
    updated_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("hero")
        .object()
        .get("headline")
        .assert_string("updated headline");
}

#[tokio::test]
async fn get_posts_with_tags_filters_dev_posts() {
    let state = create_state().await;

    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, tags, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-dev")
    .bind("Dev post")
    .bind("desc")
    .bind("2026-03-30T00:00:00Z")
    .bind(r#"[{"tag":"dev","category":"topic"}]"#)
    .bind("body")
    .bind("writer")
    .bind("published")
    .execute(&state.db)
    .await
    .expect("failed to seed dev post");

    query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, tags, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("post-life")
    .bind("Life post")
    .bind("desc")
    .bind("2026-03-29T00:00:00Z")
    .bind(r#"[{"tag":"life","category":"topic"}]"#)
    .bind("body")
    .bind("writer")
    .bind("published")
    .execute(&state.db)
    .await
    .expect("failed to seed life post");

    query("INSERT OR IGNORE INTO tags (name, category) VALUES (?, ?)")
        .bind("dev")
        .bind("topic")
        .execute(&state.db)
        .await
        .expect("failed to seed dev tag");
    query("INSERT OR IGNORE INTO tags (name, category) VALUES (?, ?)")
        .bind("life")
        .bind("topic")
        .execute(&state.db)
        .await
        .expect("failed to seed life tag");
    query(
        "INSERT INTO post_tags (post_id, tag_id) SELECT ?, tag_id FROM tags WHERE name = ? AND category = ?",
    )
    .bind("post-dev")
    .bind("dev")
    .bind("topic")
    .execute(&state.db)
    .await
    .expect("failed to seed dev post tag");
    query(
        "INSERT INTO post_tags (post_id, tag_id) SELECT ?, tag_id FROM tags WHERE name = ? AND category = ?",
    )
    .bind("post-life")
    .bind("life")
    .bind("topic")
    .execute(&state.db)
    .await
    .expect("failed to seed life post tag");

    let cli = TestClient::new(
        Route::new()
            .at("/posts/search-with-tags", get(get_posts_with_tags))
            .data(state),
    );

    let response = cli.get("/posts/search-with-tags?include=dev").send().await;
    response.assert_status_is_ok();

    let payload = response.json().await;
    let posts = payload
        .value()
        .object()
        .get("data")
        .object()
        .get("posts")
        .array();
    posts.assert_len(1);
    posts.get(0).object().get("title").assert_string("Dev post");
}

#[tokio::test]
async fn delete_portfolio_removes_document() {
    let state = create_state().await;
    let cli = TestClient::new(
        Route::new()
            .at("/portfolio", get(get_portfolio).delete(delete_portfolio))
            .data(state.clone()),
    );

    let deleted = cli.delete("/portfolio").send().await;
    deleted.assert_status(StatusCode::NO_CONTENT);

    let remaining: Option<String> = query_scalar("SELECT slug FROM portfolio WHERE slug = ?")
        .bind("dev")
        .fetch_optional(&state.db)
        .await
        .expect("failed to query portfolio after delete");

    assert!(remaining.is_none());
}
