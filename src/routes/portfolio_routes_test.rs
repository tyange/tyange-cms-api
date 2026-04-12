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
async fn get_portfolio_returns_not_found_initially_and_put_creates_and_updates_it() {
    let state = create_state().await;
    let cli = TestClient::new(
        Route::new()
            .at("/portfolio", get(get_portfolio).put(update_portfolio))
            .data(state),
    );

    let initial = cli.get("/portfolio").send().await;
    initial.assert_status(StatusCode::NOT_FOUND);

    let created = cli
        .put("/portfolio")
        .body_json(&json!({
            "content": {
                "slug": "dev",
                "version": 1,
                "identity": {
                    "name": "TYANGE",
                    "role": "프론트엔드 개발자",
                    "location": "서울, 대한민국",
                    "availability": "브랜딩과 제품 완성도가 중요한 작업을 선별해 진행합니다",
                    "email": "usun16@gmail.com",
                    "github_url": "https://github.com/tyange"
                },
                "featured_projects": [],
                "career": {
                    "summary_label": "경력",
                    "summary_value": "4년",
                    "companies": [
                        {
                            "company": "테스트 회사",
                            "period": "2020.01 - 2022.12",
                            "employment_type": "정규직",
                            "role": "프론트엔드 개발",
                            "position": "사원",
                            "items": [
                                {
                                    "title": "서비스 운영",
                                    "period": "2021.01 - 2022.12",
                                    "bullets": ["React 운영", "TypeScript 전환"]
                                }
                            ]
                        }
                    ]
                }
            }
        }))
        .send()
        .await;

    created.assert_status_is_ok();
    let created_json = created.json().await;
    created_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("identity")
        .object()
        .get("email")
        .assert_string("usun16@gmail.com");
    created_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("career")
        .object()
        .get("summary_value")
        .assert_string("4년");
    created_json
        .value()
        .object()
        .get("data")
        .object()
        .get("content")
        .object()
        .get("career")
        .object()
        .get("companies")
        .array()
        .get(0)
        .object()
        .get("items")
        .array()
        .get(0)
        .object()
        .get("bullets")
        .array()
        .get(1)
        .assert_string("TypeScript 전환");
}

#[tokio::test]
async fn delete_portfolio_removes_document() {
    let state = create_state().await;
    let cli = TestClient::new(
        Route::new()
            .at(
                "/portfolio",
                get(get_portfolio)
                    .put(update_portfolio)
                    .delete(delete_portfolio),
            )
            .data(state.clone()),
    );

    cli.put("/portfolio")
        .body_json(&json!({
            "content": {
                "slug": "dev",
                "version": 1,
                "identity": {
                    "name": "Test",
                    "role": "dev",
                    "location": "Seoul",
                    "availability": "",
                    "email": "",
                    "github_url": ""
                },
                "featured_projects": []
            }
        }))
        .send()
        .await
        .assert_status_is_ok();

    let deleted = cli.delete("/portfolio").send().await;
    deleted.assert_status(StatusCode::NO_CONTENT);

    let remaining: Option<String> = query_scalar("SELECT slug FROM portfolio WHERE slug = ?")
        .bind("dev")
        .fetch_optional(&state.db)
        .await
        .expect("failed to query portfolio after delete");

    assert!(remaining.is_none());
}
