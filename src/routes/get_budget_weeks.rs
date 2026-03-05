use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};
use sqlx::query_scalar;

use crate::models::{AppState, BudgetWeeksResponse};

#[handler]
pub async fn get_budget_weeks(
    data: Data<&Arc<AppState>>,
) -> Result<Json<BudgetWeeksResponse>, Error> {
    let response = build_budget_weeks_response(&data).await?;
    Ok(Json(response))
}

pub(crate) async fn build_budget_weeks_response(
    data: &Data<&Arc<AppState>>,
) -> Result<BudgetWeeksResponse, Error> {
    let weeks = query_scalar::<_, String>(
        "SELECT DISTINCT week_key
         FROM budget_config
         ORDER BY week_key ASC",
    )
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
