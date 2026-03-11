use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

use crate::{
    budget_periods::{date_in_period, get_active_budget_period, sum_spending_for_period},
    models::{AppState, CreateSpendingRequest, CreateSpendingResponse},
    utils::parse_transacted_at,
};
use tyange_cms_api::auth::authorization::current_user;

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

    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("예산 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?
        .ok_or_else(|| {
            Error::from_string("현재 활성 기간 예산이 없습니다.", StatusCode::NOT_FOUND)
        })?;

    if !date_in_period(&transacted_at, &budget.from_date, &budget.to_date) {
        return Err(Error::from_string(
            "transacted_at가 현재 활성 예산 기간 밖에 있습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let transacted_at_text = transacted_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let inserted = query(
        "INSERT INTO spending_records (owner_user_id, amount, merchant, transacted_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&user.user_id)
    .bind(payload.amount)
    .bind(payload.merchant)
    .bind(&transacted_at_text)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("소비 기록 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let period_total_spent =
        sum_spending_for_period(&data.db, &user.user_id, &budget.from_date, &budget.to_date)
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("기간 소비 합계 조회 실패: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

    let remaining = budget.total_budget - period_total_spent;
    let usage_rate = if budget.total_budget > 0 {
        period_total_spent as f64 / budget.total_budget as f64
    } else {
        0.0
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateSpendingResponse {
            record_id: inserted.last_insert_rowid(),
            budget_id: budget.budget_id,
            period_total_spent,
            total_budget: budget.total_budget,
            remaining,
            alert: usage_rate >= budget.alert_threshold,
        }),
    ))
}
