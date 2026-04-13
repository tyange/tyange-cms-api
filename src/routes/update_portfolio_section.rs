use crate::models::{
    AppState, CustomResponse, PortfolioCareerSection, PortfolioIdentity, PortfolioMeta,
    PortfolioProject,
};
use poem::http::StatusCode;
use poem::web::{Data, Json, Path};
use poem::{Error, handler};
use sqlx::query;
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
pub struct UpdateSectionRequest {
    pub content: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
pub struct UpdateSectionResponse {
    pub section_key: String,
    pub updated_at: String,
}

fn validate_section(section_key: &str, value: &serde_json::Value) -> Result<String, String> {
    match section_key {
        "meta" => {
            let _: PortfolioMeta =
                serde_json::from_value(value.clone()).map_err(|e| format!("meta 검증 실패: {}", e))?;
            serde_json::to_string(value).map_err(|e| e.to_string())
        }
        "identity" => {
            let _: PortfolioIdentity = serde_json::from_value(value.clone())
                .map_err(|e| format!("identity 검증 실패: {}", e))?;
            serde_json::to_string(value).map_err(|e| e.to_string())
        }
        "featured_projects" => {
            let _: Vec<PortfolioProject> = serde_json::from_value(value.clone())
                .map_err(|e| format!("featured_projects 검증 실패: {}", e))?;
            serde_json::to_string(value).map_err(|e| e.to_string())
        }
        "career" => {
            let _: PortfolioCareerSection = serde_json::from_value(value.clone())
                .map_err(|e| format!("career 검증 실패: {}", e))?;
            serde_json::to_string(value).map_err(|e| e.to_string())
        }
        _ => Err(format!("알 수 없는 섹션: {}", section_key)),
    }
}

#[handler]
pub async fn update_portfolio_section(
    Path(section_key): Path<String>,
    Json(payload): Json<UpdateSectionRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UpdateSectionResponse>>, Error> {
    let serialized = validate_section(&section_key, &payload.content).map_err(|err| {
        Error::from_string(err, StatusCode::BAD_REQUEST)
    })?;

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

    let portfolio_id = portfolio_id.ok_or_else(|| {
        Error::from_string(
            "포트폴리오 데이터를 찾지 못했습니다.",
            StatusCode::NOT_FOUND,
        )
    })?;

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
    .bind(&section_key)
    .bind(&serialized)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("{} 섹션 저장 실패: {}", section_key, err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let updated_at: String = sqlx::query_scalar(
        "SELECT updated_at FROM portfolio_section WHERE portfolio_id = ? AND section_key = ?",
    )
    .bind(portfolio_id)
    .bind(&section_key)
    .fetch_one(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("updated_at 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(UpdateSectionResponse {
            section_key,
            updated_at,
        }),
        message: Some(String::from("섹션을 업데이트 했습니다.")),
    }))
}
