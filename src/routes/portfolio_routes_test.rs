use std::sync::Arc;

use poem::{EndpointExt, Route, get, http::StatusCode, test::TestClient};
use serde_json::json;
use sqlx::{SqlitePool, query_scalar};

use crate::{
    db::init_db,
    models::AppState,
    routes::{
        delete_portfolio::delete_portfolio, get_portfolio::get_portfolio,
        update_portfolio::update_portfolio,
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
        .get("metrics")
        .array()
        .get(0)
        .object()
        .get("value")
        .assert_string("2");

    let updated = cli
        .put("/portfolio")
        .body_json(&json!({
            "content": {
                "slug": "dev",
                "version": 1,
                "identity": {
                    "name": "TYANGE",
                    "role": "프론트엔드 개발자",
                    "location": "서울",
                    "availability": "가능",
                    "email": "usun16@gmail.com",
                    "github_url": "https://github.com/tyange",
                    "blog_url": "https://blog.tyange.com",
                    "velog_url": "https://velog.io/@tyange"
                },
                "hero": {
                    "eyebrow": "아이브로우",
                    "headline": "업데이트된 헤드라인",
                    "summary": "요약",
                    "primary_cta": { "label": "깃허브", "url": "https://github.com/tyange" },
                    "secondary_cta": { "label": "블로그", "url": "https://blog.tyange.com" }
                },
                "highlight_cards": [
                    { "label": "집중", "title": "인터페이스" },
                    { "label": "스택", "title": "Next.js" }
                ],
                "metrics": [
                    { "value": "2", "unit": "개사", "description": "프론트엔드 개발자로 재직한 이력" }
                ],
                "guiding_principle": "모든 요소는 이유가 있어야 한다.",
                "featured_projects": [],
                "about": {
                    "eyebrow": "소개",
                    "headline": "소개 헤드라인",
                    "paragraphs": ["A", "B"],
                    "services": ["UI"],
                    "strengths": ["구조"]
                },
                "writing": {
                    "eyebrow": "기록",
                    "title": "dev 글",
                    "description": "설명"
                },
                "currently_building": [
                    {
                        "name": "포트폴리오 개편",
                        "summary": "API와 프론트 데이터를 맞추는 중",
                        "stack": ["Next.js", "Rust"]
                    }
                ]
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
        .assert_string("업데이트된 헤드라인");
    updated_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("currently_building")
        .array()
        .get(0)
        .object()
        .get("name")
        .assert_string("포트폴리오 개편");
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
