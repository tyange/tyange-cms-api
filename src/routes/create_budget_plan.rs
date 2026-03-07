use std::{collections::BTreeMap, sync::Arc};

use chrono::{Datelike, NaiveDate};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

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

    let mut days_by_week: BTreeMap<String, u32> = BTreeMap::new();
    let mut cursor = from_date;
    loop {
        let iso = cursor.iso_week();
        let week_key = format!("{}-W{:02}", iso.year(), iso.week());
        let entry = days_by_week.entry(week_key).or_insert(0);
        *entry += 1;

        if cursor == to_date {
            break;
        }
        cursor = cursor.succ_opt().ok_or_else(|| {
            Error::from_string(
                "날짜 계산 중 오류가 발생했습니다.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    }

    let total_days: u32 = days_by_week.values().sum();
    let daily_budget = payload.total_budget as f64 / total_days as f64;

    let mut week_items = days_by_week
        .iter()
        .map(|(week_key, days)| {
            let exact = payload.total_budget as f64 * (*days as f64) / (total_days as f64);
            let base = exact.floor() as i64;
            (week_key.clone(), *days, base, exact - base as f64)
        })
        .collect::<Vec<(String, u32, i64, f64)>>();

    let allocated_base_sum = week_items.iter().map(|(_, _, base, _)| *base).sum::<i64>();
    let mut remainder = payload.total_budget - allocated_base_sum;

    let mut order = (0..week_items.len()).collect::<Vec<usize>>();
    order.sort_by(|a, b| week_items[*b].3.total_cmp(&week_items[*a].3));
    for idx in order {
        if remainder <= 0 {
            break;
        }
        week_items[idx].2 += 1;
        remainder -= 1;
    }

    let mut tx = data.db.begin().await.map_err(|e| {
        Error::from_string(
            format!("트랜잭션 시작 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    for (week_key, _days, weekly_limit, _frac) in &week_items {
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

    let weeks = week_items
        .into_iter()
        .map(|(week_key, days, weekly_limit, _frac)| BudgetPlanWeekItem {
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
