use std::sync::Arc;

use chrono::Utc;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Path},
    Error, Request,
};
use sqlx::query;

use crate::models::AppState;
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn delete_api_key(
    req: &Request,
    Path(api_key_id): Path<i64>,
    data: Data<&Arc<AppState>>,
) -> Result<StatusCode, Error> {
    let user = current_user(req)?;
    let result = query(
        r#"
        UPDATE api_keys
        SET revoked_at = COALESCE(revoked_at, ?)
        WHERE api_key_id = ? AND user_id = ?
        "#,
    )
    .bind(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(api_key_id)
    .bind(&user.user_id)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("API key 폐기 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if result.rows_affected() == 0 {
        return Err(Error::from_string(
            "해당 API key를 찾을 수 없습니다.",
            StatusCode::NOT_FOUND,
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}
