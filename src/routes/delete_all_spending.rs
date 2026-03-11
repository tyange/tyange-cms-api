use std::sync::Arc;

use poem::{handler, http::StatusCode, web::Data, Error, Request};
use sqlx::query;

use crate::models::AppState;
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn delete_all_spending(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<StatusCode, Error> {
    let user = current_user(req)?;

    query("DELETE FROM spending_records WHERE owner_user_id = ?")
        .bind(&user.user_id)
        .execute(&data.db)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("소비 기록 초기화 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}
