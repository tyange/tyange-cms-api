use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error, Request,
};
use sqlx::query;

use crate::models::{AppState, CustomResponse, WeeklyConfigRequest};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn update_budget(
    req: &Request,
    Path(config_id): Path<u32>,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<WeeklyConfigRequest>,
) -> Result<Json<CustomResponse<()>>, Error> {
    let user = current_user(req)?;
    let updated = query(
        "UPDATE budget_config
         SET weekly_limit = ?, alert_threshold = ?
         WHERE config_id = ? AND owner_user_id = ?",
    )
    .bind(payload.weekly_limit)
    .bind(payload.alert_threshold)
    .bind(config_id)
    .bind(&user.user_id)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 설정 수정 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if updated.rows_affected() == 0 {
        return Err(Error::from_string(
            format!(
                "해당 예산 설정(config_id={})을 찾을 수 없습니다.",
                config_id
            ),
            StatusCode::NOT_FOUND,
        ));
    }

    Ok(Json(CustomResponse {
        status: true,
        data: None,
        message: Some(String::from("예산을 수정했습니다.")),
    }))
}
