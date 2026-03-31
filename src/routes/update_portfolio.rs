use crate::db::merge_portfolio_document_with_defaults;
use crate::models::{
    AppState, CustomResponse, PortfolioResponse, PortfolioRow, UpdatePortfolioRequest,
};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{Error, handler};
use sqlx::{Sqlite, query, query_as};
use std::sync::Arc;

#[handler]
pub async fn update_portfolio(
    Json(payload): Json<UpdatePortfolioRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PortfolioResponse>>, Error> {
    let normalized_content = merge_portfolio_document_with_defaults(payload.content);

    let serialized = serde_json::to_string(&normalized_content).map_err(|err| {
        Error::from_string(
            format!("포트폴리오 직렬화 실패: {}", err),
            StatusCode::BAD_REQUEST,
        )
    })?;

    let result = query(
        r#"
        INSERT INTO portfolio (portfolio_id, slug, content, created_at, updated_at)
        VALUES (
            COALESCE((SELECT portfolio_id FROM portfolio WHERE slug = ? LIMIT 1), 1),
            ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        ON CONFLICT(portfolio_id) DO UPDATE SET
            slug = excluded.slug,
            content = excluded.content,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&normalized_content.slug)
    .bind(&normalized_content.slug)
    .bind(serialized)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => {
            let saved = query_as::<Sqlite, PortfolioRow>(
                r#"
                SELECT portfolio_id, slug, content, created_at, updated_at
                FROM portfolio
                WHERE slug = ?
                LIMIT 1
                "#,
            )
            .bind(&normalized_content.slug)
            .fetch_one(&data.db)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("포트폴리오 조회 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

            Ok(Json(CustomResponse {
                status: true,
                data: Some(PortfolioResponse {
                    portfolio_id: saved.portfolio_id,
                    slug: saved.slug,
                    content: normalized_content,
                    created_at: saved.created_at,
                    updated_at: saved.updated_at,
                }),
                message: Some(String::from("포트폴리오를 업데이트 했습니다.")),
            }))
        }
        Err(err) => Err(Error::from_string(
            format!("Failed to update portfolio: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
