use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{db::init_db, models::AppState, routes::get_budget_weeks::build_budget_weeks_response};

#[tokio::test]
async fn returns_empty_payload_when_no_budget_week_exists() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");

    let state = Arc::new(AppState::new(db));
    let response = build_budget_weeks_response(&poem::web::Data(&state), "user-1")
        .await
        .expect("handler should succeed");

    assert!(response.weeks.is_empty());
    assert!(response.min_week.is_none());
    assert!(response.max_week.is_none());
}

#[tokio::test]
async fn returns_sorted_weeks_with_min_and_max() {
    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");

    sqlx::query(
        "INSERT INTO budget_config (
             owner_user_id,
             week_key,
             weekly_limit,
             projected_remaining,
             alert_threshold
         )
         VALUES ('user-1', '2026-W12', 500000, 500000, 0.85),
                ('user-1', '2026-W10', 500000, 500000, 0.85),
                ('user-1', '2025-W52', 500000, 500000, 0.85),
                ('user-2', '2026-W09', 500000, 500000, 0.85)",
    )
    .execute(&db)
    .await
    .expect("failed to seed budget_config");

    let state = Arc::new(AppState::new(db));
    let response = build_budget_weeks_response(&poem::web::Data(&state), "user-1")
        .await
        .expect("handler should succeed");

    assert_eq!(
        response.weeks,
        vec![
            "2025-W52".to_string(),
            "2026-W10".to_string(),
            "2026-W12".to_string()
        ]
    );
    assert_eq!(response.min_week.as_deref(), Some("2025-W52"));
    assert_eq!(response.max_week.as_deref(), Some("2026-W12"));
}
