use crate::models::{AppState, CustomResponse, Portfolio, UpdatePortfolioRequest};
use chrono::Utc;
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn update_portfolio(
    Json(payload): Json<UpdatePortfolioRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<Portfolio>>, Error> {
    let result = query(
        r"
        UPDATE portfolio SET content = $1
        ",
    )
    .bind(&payload.content)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => Ok(Json(CustomResponse {
            status: true,
            data: Some(Portfolio {
                portfolio_id: 1,
                content: payload.content,
                updated_at: Utc::now().to_string(),
            }),
            message: Some(String::from("포트폴리오를 업데이트 했습니다.")),
        })),
        Err(_) => Err(Error::from_string(
            "Failed to update post.",
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
