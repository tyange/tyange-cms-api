use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

use crate::{
    budget_periods::{format_naive_date, parse_naive_date, sum_spending_for_period},
    models::{AppState, BudgetRebalanceRequest, BudgetRebalanceResponse, CustomResponse},
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn rebalance_budget(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<BudgetRebalanceRequest>,
) -> Result<Json<CustomResponse<BudgetRebalanceResponse>>, Error> {
    let user = current_user(req)?;

    if payload.total_budget <= 0 {
        return Err(Error::from_string(
            "total_budget는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let from_date = parse_naive_date(&payload.from_date, "from_date")
        .map_err(|message| Error::from_string(message, StatusCode::BAD_REQUEST))?;
    let to_date = parse_naive_date(&payload.to_date, "to_date")
        .map_err(|message| Error::from_string(message, StatusCode::BAD_REQUEST))?;
    let as_of_date = parse_naive_date(&payload.as_of_date, "as_of_date")
        .map_err(|message| Error::from_string(message, StatusCode::BAD_REQUEST))?;

    if to_date < from_date {
        return Err(Error::from_string(
            "to_date는 from_date보다 빠를 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if as_of_date > to_date {
        return Err(Error::from_string(
            "as_of_date가 기간 종료일을 초과했습니다.",
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
    let as_of_date_text = format_naive_date(as_of_date);
    let spent_so_far = match payload.spent_so_far {
        Some(value) => {
            if value < 0 {
                return Err(Error::from_string(
                    "spent_so_far는 0 이상이어야 합니다.",
                    StatusCode::BAD_REQUEST,
                ));
            }
            value
        }
        None => {
            let spent_end = if as_of_date < from_date {
                from_date
            } else {
                as_of_date
            };
            sum_spending_for_period(
                &data.db,
                &user.user_id,
                &from_date_text,
                &format_naive_date(spent_end),
            )
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("누적 소비 조회 실패: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?
        }
    };

    let remaining_budget = payload.total_budget - spent_so_far;
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
            format!("재계산 예산 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(BudgetRebalanceResponse {
            budget_id: inserted.last_insert_rowid(),
            total_budget: payload.total_budget,
            from_date: from_date_text,
            to_date: to_date_text,
            as_of_date: as_of_date_text,
            spent_so_far,
            remaining_budget,
            alert_threshold,
            is_overspent: remaining_budget < 0,
        }),
        message: Some(String::from("기간 예산의 남은 총액을 다시 계산했습니다.")),
    }))
}
