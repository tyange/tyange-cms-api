use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::{query, query_as, FromRow};

use crate::{
    models::{AppState, CreateSpendingRequest, CreateSpendingResponse},
    utils::{iso_week_key_from_datetime, parse_transacted_at},
};
use tyange_cms_api::auth::authorization::current_user;

#[derive(FromRow)]
struct ActiveBudget {
    weekly_limit: i64,
    projected_remaining: i64,
    alert_threshold: f64,
}

#[derive(FromRow)]
struct WeeklyTotal {
    weekly_total: i64,
}

#[handler]
pub async fn create_spending(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<CreateSpendingRequest>,
) -> Result<(StatusCode, Json<CreateSpendingResponse>), Error> {
    let user = current_user(req)?;
    if payload.amount <= 0 {
        return Err(Error::from_string(
            "amount는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let transacted_at = parse_transacted_at(&payload.transacted_at).map_err(|_| {
        Error::from_string(
            "transacted_at 형식이 올바르지 않습니다. 예: 2026-03-03T12:20:00",
            StatusCode::BAD_REQUEST,
        )
    })?;
    let week_key = iso_week_key_from_datetime(&transacted_at);

    let budget = query_as::<_, ActiveBudget>(
        "SELECT weekly_limit, projected_remaining, alert_threshold
         FROM budget_config
         WHERE owner_user_id = ? AND week_key = ?
         LIMIT 1",
    )
    .bind(&user.user_id)
    .bind(&week_key)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let budget = budget.ok_or_else(|| {
        Error::from_string(
            "현재 주차에 적용 중인 예산이 없습니다.",
            StatusCode::NOT_FOUND,
        )
    })?;

    let transacted_at_text = transacted_at.format("%Y-%m-%d %H:%M:%S").to_string();

    let inserted = query(
        "INSERT INTO spending_records (owner_user_id, amount, merchant, transacted_at, week_key)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&user.user_id)
    .bind(payload.amount)
    .bind(payload.merchant)
    .bind(&transacted_at_text)
    .bind(&week_key)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("소비 기록 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let weekly_total = query_as::<_, WeeklyTotal>(
        "SELECT COALESCE(SUM(amount), 0) AS weekly_total
         FROM spending_records
         WHERE owner_user_id = ? AND week_key = ?",
    )
    .bind(&user.user_id)
    .bind(&week_key)
    .fetch_one(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("주간 합계 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?
    .weekly_total;

    let remaining = budget.projected_remaining - weekly_total;
    let usage_rate = if budget.weekly_limit > 0 {
        weekly_total as f64 / budget.weekly_limit as f64
    } else {
        0.0
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateSpendingResponse {
            record_id: inserted.last_insert_rowid(),
            weekly_total,
            weekly_limit: budget.weekly_limit,
            remaining,
            alert: usage_rate >= budget.alert_threshold,
        }),
    ))
}
