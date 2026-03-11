use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;

use crate::{
    budget_periods::{compute_budget_summary, get_active_budget_period, sum_spending_for_period},
    models::{AppState, CustomResponse, UpdateActiveBudgetRequest, UpdateActiveBudgetResponse},
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn update_active_budget(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<UpdateActiveBudgetRequest>,
) -> Result<Json<CustomResponse<UpdateActiveBudgetResponse>>, Error> {
    let user = current_user(req)?;
    reject_unsupported_budget_fields(&payload.extra_fields)?;

    if payload.total_budget <= 0 {
        return Err(Error::from_string(
            "total_budget는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

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

    let alert_threshold = payload.alert_threshold.unwrap_or(budget.alert_threshold);
    if !(0.0..=1.0).contains(&alert_threshold) {
        return Err(Error::from_string(
            "alert_threshold는 0.0 이상 1.0 이하여야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }
    let total_spent =
        sum_spending_for_period(&data.db, &user.user_id, &budget.from_date, &budget.to_date)
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("소비 합계 조회 실패: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;
    let summary = compute_budget_summary(payload.total_budget, total_spent, alert_threshold);

    query(
        "UPDATE budget_periods
         SET total_budget = ?, alert_threshold = ?, updated_at = CURRENT_TIMESTAMP
         WHERE budget_id = ? AND owner_user_id = ?",
    )
    .bind(payload.total_budget)
    .bind(alert_threshold)
    .bind(budget.budget_id)
    .bind(&user.user_id)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 수정 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(UpdateActiveBudgetResponse {
            budget_id: budget.budget_id,
            total_budget: payload.total_budget,
            from_date: budget.from_date,
            to_date: budget.to_date,
            total_spent,
            remaining_budget: summary.remaining_budget,
            usage_rate: summary.usage_rate,
            alert: summary.alert,
            alert_threshold,
            is_overspent: summary.is_overspent,
        }),
        message: Some(String::from("현재 활성 기간 예산을 수정했습니다.")),
    }))
}

fn reject_unsupported_budget_fields(
    extra_fields: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), Error> {
    if extra_fields.contains_key("total_spent") || extra_fields.contains_key("spent_so_far") {
        return Err(Error::from_string(
            "total_spent와 spent_so_far는 더 이상 지원되지 않습니다. 소비 합계는 거래원장에서 자동 계산됩니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if !extra_fields.is_empty() {
        return Err(Error::from_string(
            format!(
                "알 수 없는 요청 필드가 있습니다: {}",
                extra_fields.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
            StatusCode::BAD_REQUEST,
        ));
    }

    Ok(())
}
