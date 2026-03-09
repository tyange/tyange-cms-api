use std::sync::Arc;

use chrono::NaiveDate;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

use crate::budget::{allocate_amounts_by_days, collect_iso_week_days};
use crate::models::{
    AppState, BudgetPlanRequest, BudgetPlanResponse, BudgetPlanWeekItem, CustomResponse,
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

    let from_date = NaiveDate::parse_from_str(&payload.from_date, "%Y-%m-%d").map_err(|_| {
        Error::from_string(
            "from_date 형식이 올바르지 않습니다. 예: 2026-03-05",
            StatusCode::BAD_REQUEST,
        )
    })?;
    let to_date = NaiveDate::parse_from_str(&payload.to_date, "%Y-%m-%d").map_err(|_| {
        Error::from_string(
            "to_date 형식이 올바르지 않습니다. 예: 2026-03-21",
            StatusCode::BAD_REQUEST,
        )
    })?;

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

    let days_by_week = collect_iso_week_days(from_date, to_date, from_date)?;

    let total_days: u32 = days_by_week.iter().map(|(_, days)| *days).sum();
    let daily_budget = payload.total_budget as f64 / total_days as f64;
    let weekly_limits = allocate_amounts_by_days(
        &days_by_week
            .iter()
            .map(|(_, days)| *days)
            .collect::<Vec<u32>>(),
        payload.total_budget,
        false,
    );

    let mut tx = data.db.begin().await.map_err(|e| {
        Error::from_string(
            format!("트랜잭션 시작 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    for ((week_key, _days), weekly_limit) in days_by_week.iter().zip(weekly_limits.iter()) {
        query(
            "INSERT INTO budget_config (owner_user_id, week_key, weekly_limit, alert_threshold)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(owner_user_id, week_key) DO UPDATE SET
                 weekly_limit = excluded.weekly_limit,
                 alert_threshold = excluded.alert_threshold",
        )
        .bind(&user.user_id)
        .bind(week_key)
        .bind(*weekly_limit)
        .bind(alert_threshold)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("주차 예산 저장 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    }

    tx.commit().await.map_err(|e| {
        Error::from_string(
            format!("트랜잭션 커밋 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let weeks = days_by_week
        .into_iter()
        .zip(weekly_limits.into_iter())
        .map(|((week_key, days), weekly_limit)| BudgetPlanWeekItem {
            week_key,
            days,
            weekly_limit,
        })
        .collect::<Vec<BudgetPlanWeekItem>>();

    Ok(Json(CustomResponse {
        status: true,
        data: Some(BudgetPlanResponse {
            total_budget: payload.total_budget,
            from_date: payload.from_date,
            to_date: payload.to_date,
            daily_budget,
            weeks,
        }),
        message: Some(String::from("주차별 예산 계획을 저장했습니다.")),
    }))
}
