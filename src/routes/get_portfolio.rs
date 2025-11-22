use crate::models::{AppState, PortfolioResponse};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::{query_as, Sqlite};
use std::sync::Arc;

#[handler]
pub async fn get_portfolio(data: Data<&Arc<AppState>>) -> Result<Json<PortfolioResponse>, Error> {
    let result = query_as::<Sqlite, PortfolioResponse>(
        r#"
        SELECT content, updated_at
        FROM portfolio
        WHERE portfolio_id = ?
        "#,
    )
    .bind(1)
    .fetch_optional(&data.db)
    .await;

    match result {
        Ok(Some(db_portfolio)) => {
            let portfolio_response = PortfolioResponse {
                content: db_portfolio.content,
                updated_at: db_portfolio.updated_at,
            };
            Ok(Json(portfolio_response))
        }
        Ok(None) => Err(Error::from_string(
            "포트폴리오 데이터를 찾지 못했습니다.",
            StatusCode::NOT_FOUND,
        )),
        Err(err) => Err(Error::from_string(
            format!("Error fetching portfolio: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
