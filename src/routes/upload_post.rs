use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json},
    Error,
};
use uuid::Uuid;

use crate::{
    models::{UploadPostRequest, UploadResponse},
    AppState,
};

#[handler]
pub async fn upload_post(
    Json(payload): Json<UploadPostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<UploadResponse>, Error> {
    let post_id = Uuid::new_v4().to_string();

    let result = sqlx::query(
        r#"
        INSERT INTO posts (post_id, title, description, published_at, tags, content)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&post_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.published_at)
    .bind(&payload.tags)
    .bind(&payload.content)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => {
            println!("Post saved successfully with ID: {}", post_id);
            Ok(Json(UploadResponse { post_id }))
        }
        Err(err) => {
            eprintln!("Error saving post: {}", err);
            Err(Error::from_string(
                "Failed to save post",
                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
