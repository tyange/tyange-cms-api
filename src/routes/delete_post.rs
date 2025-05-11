use crate::models::{CustomResponse, DeletePostResponse};
use crate::AppState;
use poem::web::{Data, Json, Path};
use poem::{handler, Error};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn delete_post(
    Path(post_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<DeletePostResponse>>, Error> {
    let result = query(
        r#"
        DELETE FROM posts WHERE post_id = ?
        "#,
    )
    .bind(&post_id)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => Ok(Json(CustomResponse {
            status: true,
            data: Some(DeletePostResponse { post_id }),
            message: Some(String::from("포스트가 삭제되었습니다.")),
        })),
        Err(e) => {
            eprintln!("Error saving post: {}", e);
            Err(Error::from_string(
                "Failed to save post",
                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
