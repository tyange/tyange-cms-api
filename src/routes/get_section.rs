use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error,
};
use sqlx::{query_as, Sqlite};

use crate::models::{AppState, CustomResponse, Section, SectionResponse};

#[handler]
pub async fn get_section(
    Path(section_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<SectionResponse>>, Error> {
    let result = query_as::<Sqlite, Section>(
        r#"
        SELECT section_id, section_type, content_data, order_index, is_active, created_at, updated_at
        FROM sections
        WHERE section_id = ?
        "#,
    )
    .bind(&section_id)
    .fetch_optional(&data.db)
    .await;

    match result {
        Ok(Some(db_section)) => {
            let content_json: serde_json::Value = serde_json::from_str(&db_section.content_data)
                .map_err(|e| {
                    Error::from_string(
                        format!("JSON 파싱 에러: {}", e),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?;

            let section_response = SectionResponse {
                section_id: db_section.section_id,
                section_type: db_section.section_type,
                content_data: content_json, // JSON 객체로 변환
                order_index: db_section.order_index,
                is_active: db_section.is_active,
                created_at: db_section.created_at,
                updated_at: db_section.updated_at,
            };

            Ok(Json(CustomResponse {
                status: true,
                data: Some(section_response),
                message: None,
            }))
        }
        Ok(None) => {
            println!("sectino data를 찾을 수 없음: {}", section_id);
            Err(Error::from_string(
                "해당 id에 해당하는 section data가 없네요.",
                StatusCode::NOT_FOUND,
            ))
        }
        Err(err) => Err(Error::from_string(
            format!("Error fetching section: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
