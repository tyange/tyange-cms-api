use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error,
};
use sqlx::{query_as, Sqlite};

use crate::models::{AppState, Section};

#[handler]
pub async fn get_section(
    Path(section_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<Section>, Error> {
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
            let section_response = Section::from(db_section);
            Ok(Json(section_response))
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
