use crate::models::{
    AppState, CustomResponse, PortfolioMeta, PortfolioResponse, UpdatePortfolioRequest,
};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{Error, handler};
use sqlx::query;
use std::sync::Arc;

async fn upsert_section(
    pool: &sqlx::SqlitePool,
    portfolio_id: i32,
    section_key: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    query(
        r#"
        INSERT INTO portfolio_section (portfolio_id, section_key, content, created_at, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(portfolio_id, section_key) DO UPDATE SET
            content = excluded.content,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(portfolio_id)
    .bind(section_key)
    .bind(content)
    .execute(pool)
    .await?;
    Ok(())
}

#[handler]
pub async fn update_portfolio(
    Json(payload): Json<UpdatePortfolioRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PortfolioResponse>>, Error> {
    let content = payload.content;
    let slug = content.slug.trim();
    let slug = if slug.is_empty() { "dev" } else { slug };

    // Upsert master row
    query(
        r#"
        INSERT INTO portfolio (slug, created_at)
        VALUES (?, CURRENT_TIMESTAMP)
        ON CONFLICT(slug) DO UPDATE SET slug = excluded.slug
        "#,
    )
    .bind(slug)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("포트폴리오 마스터 행 생성 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let portfolio_id: i32 =
        sqlx::query_scalar("SELECT portfolio_id FROM portfolio WHERE slug = ?")
            .bind(slug)
            .fetch_one(&data.db)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("포트폴리오 조회 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

    // Serialize each section
    let meta = PortfolioMeta {
        slug: slug.to_string(),
        version: content.version,
    };
    let meta_json = serde_json::to_string(&meta).map_err(|err| {
        Error::from_string(format!("meta 직렬화 실패: {}", err), StatusCode::BAD_REQUEST)
    })?;
    let identity_json = serde_json::to_string(&content.identity).map_err(|err| {
        Error::from_string(
            format!("identity 직렬화 실패: {}", err),
            StatusCode::BAD_REQUEST,
        )
    })?;
    let projects_json = serde_json::to_string(&content.featured_projects).map_err(|err| {
        Error::from_string(
            format!("featured_projects 직렬화 실패: {}", err),
            StatusCode::BAD_REQUEST,
        )
    })?;
    let career_json = content
        .career
        .as_ref()
        .map(|c| serde_json::to_string(c))
        .transpose()
        .map_err(|err| {
            Error::from_string(
                format!("career 직렬화 실패: {}", err),
                StatusCode::BAD_REQUEST,
            )
        })?;

    // Upsert all sections
    let pool = &data.db;

    upsert_section(pool, portfolio_id, "meta", &meta_json)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("meta 섹션 저장 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    upsert_section(pool, portfolio_id, "identity", &identity_json)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("identity 섹션 저장 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    upsert_section(pool, portfolio_id, "featured_projects", &projects_json)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("featured_projects 섹션 저장 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    if let Some(career_str) = &career_json {
        upsert_section(pool, portfolio_id, "career", career_str)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("career 섹션 저장 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;
    } else {
        // career가 None이면 기존 섹션 삭제
        query("DELETE FROM portfolio_section WHERE portfolio_id = ? AND section_key = 'career'")
            .bind(portfolio_id)
            .execute(pool)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("career 섹션 삭제 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;
    }

    // Fetch updated_at
    let updated_at: String = sqlx::query_scalar(
        "SELECT MAX(updated_at) FROM portfolio_section WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .fetch_one(pool)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("updated_at 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let created_at: String =
        sqlx::query_scalar("SELECT created_at FROM portfolio WHERE portfolio_id = ?")
            .bind(portfolio_id)
            .fetch_one(pool)
            .await
            .map_err(|err| {
                Error::from_string(
                    format!("created_at 조회 실패: {}", err),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(PortfolioResponse {
            portfolio_id,
            slug: slug.to_string(),
            content,
            created_at,
            updated_at,
        }),
        message: Some(String::from("포트폴리오를 업데이트 했습니다.")),
    }))
}
