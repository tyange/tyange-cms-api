use crate::models::{
    AppState, CustomResponse, PortfolioCareerSection, PortfolioDocument, PortfolioIdentity,
    PortfolioIntroSection, PortfolioMasterRow, PortfolioMeta, PortfolioProject,
    PortfolioResponse, PortfolioSectionRow,
};
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{Error, handler};
use sqlx::{Sqlite, query_as};
use std::sync::Arc;

#[handler]
pub async fn get_portfolio(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PortfolioResponse>>, Error> {
    let master = query_as::<Sqlite, PortfolioMasterRow>(
        "SELECT portfolio_id, slug, created_at FROM portfolio WHERE slug = ?",
    )
    .bind("dev")
    .fetch_optional(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("Error fetching portfolio: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let master = match master {
        Some(m) => m,
        None => {
            return Err(Error::from_string(
                "포트폴리오 데이터를 찾지 못했습니다.",
                StatusCode::NOT_FOUND,
            ));
        }
    };

    let sections = query_as::<Sqlite, PortfolioSectionRow>(
        r#"
        SELECT section_id, portfolio_id, section_key, content, created_at, updated_at
        FROM portfolio_section
        WHERE portfolio_id = ?
        "#,
    )
    .bind(master.portfolio_id)
    .fetch_all(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("Error fetching portfolio sections: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if sections.is_empty() {
        return Err(Error::from_string(
            "포트폴리오 데이터를 찾지 못했습니다.",
            StatusCode::NOT_FOUND,
        ));
    }

    let mut meta: Option<PortfolioMeta> = None;
    let mut identity: Option<PortfolioIdentity> = None;
    let mut featured_projects: Vec<PortfolioProject> = Vec::new();
    let mut career: Option<PortfolioCareerSection> = None;
    let mut intro: Option<PortfolioIntroSection> = None;
    let mut latest_updated_at = String::new();

    for section in &sections {
        if section.updated_at > latest_updated_at {
            latest_updated_at = section.updated_at.clone();
        }

        match section.section_key.as_str() {
            "meta" => {
                meta = Some(serde_json::from_str(&section.content).map_err(|err| {
                    Error::from_string(
                        format!("Error parsing meta section: {}", err),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?);
            }
            "identity" => {
                identity = Some(serde_json::from_str(&section.content).map_err(|err| {
                    Error::from_string(
                        format!("Error parsing identity section: {}", err),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?);
            }
            "featured_projects" => {
                featured_projects =
                    serde_json::from_str(&section.content).map_err(|err| {
                        Error::from_string(
                            format!("Error parsing featured_projects section: {}", err),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    })?;
            }
            "career" => {
                career = Some(serde_json::from_str(&section.content).map_err(|err| {
                    Error::from_string(
                        format!("Error parsing career section: {}", err),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?);
            }
            "intro" => {
                intro = Some(serde_json::from_str(&section.content).map_err(|err| {
                    Error::from_string(
                        format!("Error parsing intro section: {}", err),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?);
            }
            _ => {}
        }
    }

    let meta = meta.ok_or_else(|| {
        Error::from_string(
            "포트폴리오 meta 섹션을 찾지 못했습니다.",
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let identity = identity.ok_or_else(|| {
        Error::from_string(
            "포트폴리오 identity 섹션을 찾지 못했습니다.",
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let document = PortfolioDocument {
        slug: meta.slug,
        version: meta.version,
        identity,
        featured_projects,
        career,
        intro,
    };

    Ok(Json(CustomResponse {
        status: true,
        data: Some(PortfolioResponse {
            portfolio_id: master.portfolio_id,
            slug: master.slug,
            content: document,
            created_at: master.created_at,
            updated_at: latest_updated_at,
        }),
        message: None,
    }))
}
