use crate::models::AppState;
use poem::http::StatusCode;
use poem::web::Data;
use poem::{handler, Error};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn delete_portfolio(data: Data<&Arc<AppState>>) -> Result<StatusCode, Error> {
    let portfolio_id: Option<i32> =
        sqlx::query_scalar("SELECT portfolio_id FROM portfolio WHERE slug = ?")
            .bind("dev")
            .fetch_optional(&data.db)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("포트폴리오 조회 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

    let portfolio_id = match portfolio_id {
        Some(id) => id,
        None => {
            return Err(Error::from_string(
                "포트폴리오 데이터를 찾지 못했습니다.",
                StatusCode::NOT_FOUND,
            ));
        }
    };

    query("DELETE FROM portfolio_section WHERE portfolio_id = ?")
        .bind(portfolio_id)
        .execute(&data.db)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("포트폴리오 섹션 삭제 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    query("DELETE FROM portfolio WHERE portfolio_id = ?")
        .bind(portfolio_id)
        .execute(&data.db)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("포트폴리오 삭제 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}
