use crate::models::AppState;
use poem::http::StatusCode;
use poem::web::Data;
use poem::{handler, Error};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn delete_portfolio(data: Data<&Arc<AppState>>) -> Result<StatusCode, Error> {
    let result = query("DELETE FROM portfolio WHERE slug = ?")
        .bind("dev")
        .execute(&data.db)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("포트폴리오 삭제 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    if result.rows_affected() == 0 {
        return Err(Error::from_string(
            "포트폴리오 데이터를 찾지 못했습니다.",
            StatusCode::NOT_FOUND,
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}
