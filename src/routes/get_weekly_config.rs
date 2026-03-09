use std::sync::Arc;

use chrono::{Datelike, Local};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_as;

use crate::models::{AppState, WeeklyConfigResponse};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_weekly_config(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<WeeklyConfigResponse>, Error> {
    let user = current_user(req)?;
    let today = Local::now().date_naive();
    let week_key = format!(
        "{}-W{:02}",
        today.iso_week().year(),
        today.iso_week().week()
    );

    let existing = query_as::<_, WeeklyConfigResponse>(
        "SELECT config_id, week_key, weekly_limit, alert_threshold
         FROM budget_config
         WHERE owner_user_id = ? AND week_key = ?",
    )
    .bind(&user.user_id)
    .bind(&week_key)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("DB 조회 오류: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if let Some(config) = existing {
        return Ok(Json(config));
    }

    sqlx::query("INSERT INTO budget_config (owner_user_id, week_key) VALUES (?, ?)")
        .bind(&user.user_id)
        .bind(&week_key)
        .execute(&data.db)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("DB 생성 오류: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    let config = query_as::<_, WeeklyConfigResponse>(
        "SELECT config_id, week_key, weekly_limit, alert_threshold
         FROM budget_config
         WHERE owner_user_id = ? AND week_key = ?",
    )
    .bind(&user.user_id)
    .bind(&week_key)
    .fetch_one(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("DB 조회 오류: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(config))
}
