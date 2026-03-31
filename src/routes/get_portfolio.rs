use crate::db::merge_portfolio_document_with_defaults;
use crate::models::{AppState, CustomResponse, PortfolioDocument, PortfolioResponse, PortfolioRow};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{Error, handler};
use sqlx::{Sqlite, query_as};
use std::sync::Arc;

#[handler]
pub async fn get_portfolio(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PortfolioResponse>>, Error> {
    let result = query_as::<Sqlite, PortfolioRow>(
        r#"
        SELECT portfolio_id, slug, content, created_at, updated_at
        FROM portfolio
        WHERE slug = ?
        "#,
    )
    .bind("dev")
    .fetch_optional(&data.db)
    .await;

    match result {
        Ok(Some(db_portfolio)) => {
            let content = serde_json::from_str::<PortfolioDocument>(&db_portfolio.content)
                .map_err(|err| {
                    Error::from_string(
                        format!("Error parsing portfolio content: {}", err),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?;

            let portfolio_response = PortfolioResponse {
                portfolio_id: db_portfolio.portfolio_id,
                slug: db_portfolio.slug,
                content: merge_portfolio_document_with_defaults(content),
                created_at: db_portfolio.created_at,
                updated_at: db_portfolio.updated_at,
            };
            Ok(Json(CustomResponse {
                status: true,
                data: Some(portfolio_response),
                message: None,
            }))
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
