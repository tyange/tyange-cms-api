use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_scalar;

use crate::models::{AppState, BudgetWeeksResponse};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_budget_weeks(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<BudgetWeeksResponse>, Error> {
    let user = current_user(req)?;
    let response = build_budget_weeks_response(&data, &user.user_id).await?;
    Ok(Json(response))
}

pub(crate) async fn build_budget_weeks_response(
    data: &Data<&Arc<AppState>>,
    owner_user_id: &str,
) -> Result<BudgetWeeksResponse, Error> {
    let weeks = query_scalar::<_, String>(
        "SELECT DISTINCT week_key
         FROM budget_config
         WHERE owner_user_id = ?
         ORDER BY week_key ASC",
    )
    .bind(owner_user_id)
    .fetch_all(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("주차 목록 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let min_week = weeks.first().cloned();
    let max_week = weeks.last().cloned();

    Ok(BudgetWeeksResponse {
        weeks,
        min_week,
        max_week,
    })
}
