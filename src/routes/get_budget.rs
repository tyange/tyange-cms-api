use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};

use crate::{
    budget_periods::{compute_budget_summary, get_active_budget_period, resolve_budget_total_spent},
    models::{AppState, BudgetSummaryResponse},
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_budget(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<BudgetSummaryResponse>, Error> {
    let user = current_user(req)?;
    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("예산 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?
        .ok_or_else(|| {
            Error::from_string(
                "현재 활성 기간 예산이 없습니다.",
                StatusCode::NOT_FOUND,
            )
        })?;

    let total_spent = resolve_budget_total_spent(&data.db, &user.user_id, &budget)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("소비 합계 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    let summary = compute_budget_summary(budget.total_budget, total_spent, budget.alert_threshold);

    Ok(Json(BudgetSummaryResponse {
        budget_id: budget.budget_id,
        total_budget: budget.total_budget,
        from_date: budget.from_date,
        to_date: budget.to_date,
        total_spent,
        remaining_budget: summary.remaining_budget,
        usage_rate: summary.usage_rate,
        alert: summary.alert,
        alert_threshold: budget.alert_threshold,
        is_overspent: summary.is_overspent,
    }))
}
