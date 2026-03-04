use std::sync::Arc;

use chrono::{Datelike, Local};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};
use sqlx::query;

use crate::models::{AppState, CustomResponse, WeeklyConfigRequest};

#[handler]
pub async fn set_budget(
    data: Data<&Arc<AppState>>,
    Json(payload): Json<WeeklyConfigRequest>,
) -> Result<Json<CustomResponse<()>>, Error> {
    let today = Local::now().date_naive();
    let week_key = format!(
        "{}-W{:02}",
        today.iso_week().year(),
        today.iso_week().week()
    );

    query(
        "INSERT INTO budget_config (week_key, weekly_limit, alert_threshold)
         VALUES (?, ?, ?)
         ON CONFLICT(week_key) DO UPDATE SET
             weekly_limit = excluded.weekly_limit,
             alert_threshold = excluded.alert_threshold",
    )
    .bind(&week_key)
    .bind(payload.weekly_limit)
    .bind(payload.alert_threshold)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 설정 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: None,
        message: Some(String::from("예산을 업로드 했습니다.")),
    }))
}
