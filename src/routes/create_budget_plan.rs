use std::sync::Arc;

use chrono::NaiveDate;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

use crate::{
    budget_periods::{format_naive_date, sum_spending_for_period},
    models::{AppState, BudgetPlanRequest, BudgetPlanResponse, CustomResponse},
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn create_budget_plan(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<BudgetPlanRequest>,
) -> Result<Json<CustomResponse<BudgetPlanResponse>>, Error> {
    let user = current_user(req)?;
    if payload.total_budget <= 0 {
        return Err(Error::from_string(
            "total_budget는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let from_date = parse_naive_date(&payload.from_date, "from_date")?;
    let to_date = parse_naive_date(&payload.to_date, "to_date")?;
    if to_date < from_date {
        return Err(Error::from_string(
            "to_date는 from_date보다 빠를 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let alert_threshold = payload.alert_threshold.unwrap_or(0.85);
    if !(0.0..=1.0).contains(&alert_threshold) {
        return Err(Error::from_string(
            "alert_threshold는 0.0 이상 1.0 이하여야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let from_date_text = format_naive_date(from_date);
    let to_date_text = format_naive_date(to_date);
    let spent_so_far = sum_spending_for_period(&data.db, &user.user_id, &from_date_text, &to_date_text)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("기간 소비 합계 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    let remaining_budget = payload.total_budget - spent_so_far;
    let total_days = (to_date - from_date).num_days() + 1;
    let daily_budget = payload.total_budget as f64 / total_days as f64;

    let inserted = query(
        "INSERT INTO budget_periods (
             owner_user_id,
             total_budget,
             from_date,
             to_date,
             alert_threshold
         )
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&user.user_id)
    .bind(payload.total_budget)
    .bind(&from_date_text)
    .bind(&to_date_text)
    .bind(alert_threshold)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 기간 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(BudgetPlanResponse {
            budget_id: inserted.last_insert_rowid(),
            total_budget: payload.total_budget,
            from_date: from_date_text,
            to_date: to_date_text,
            daily_budget,
            spent_so_far,
            remaining_budget,
            alert_threshold,
        }),
        message: Some(String::from("기간 예산을 저장했습니다.")),
    }))
}

fn parse_naive_date(value: &str, field_name: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").map_err(|_| {
        Error::from_string(
            format!("{field_name} 형식이 올바르지 않습니다. 예: 2026-03-05"),
            StatusCode::BAD_REQUEST,
        )
    })
}
