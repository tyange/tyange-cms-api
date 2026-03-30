use crate::models::{AppState, CustomResponse, PortfolioResponse, UpdatePortfolioRequest};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::{query, query_as, Sqlite};
use std::sync::Arc;

#[handler]
pub async fn update_portfolio(
    Json(payload): Json<UpdatePortfolioRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PortfolioResponse>>, Error> {
    let serialized = serde_json::to_string(&payload.content).map_err(|err| {
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
    .bind(&payload.content.slug)
    .bind(&payload.content.slug)
    .bind(serialized)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => {
            let saved = query_as::<Sqlite, crate::models::PortfolioRow>(
                r#"
                SELECT portfolio_id, slug, content, created_at, updated_at
                FROM portfolio
                WHERE slug = ?
                LIMIT 1
                "#,
            )
            .bind(&payload.content.slug)
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
                    content: payload.content,
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
