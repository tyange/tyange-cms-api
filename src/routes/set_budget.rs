use std::sync::Arc;

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
    query(
        r#"
        INSERT INTO budget_config (weekly_limit, alert_threshold, started_at)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&payload.weekly_limit)
    .bind(&payload.alert_threshold)
    .bind(&payload.started_at)
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
