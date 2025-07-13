use std::env;
use std::sync::Arc;

use crate::{
    models::{CustomResponse, UploadPostRequest, UploadResponse},
    AppState,
};
use poem::http::StatusCode;
use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;
use tyange_cms_backend::auth::jwt::Claims;
use uuid::Uuid;

#[handler]
pub async fn upload_post(
    req: &Request,
    Json(payload): Json<UploadPostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UploadResponse>>, Error> {
    if let Some(token) = req.header("Authorization") {
        let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
            Error::from_string(
                format!("Server configuration error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
        let secret_bytes = secret.as_bytes();
        let decoded_token = Claims::from_token(&token, &secret_bytes)?;

        let user_id = decoded_token.claims.sub;

        let post_id = Uuid::new_v4().to_string();

        let result = query(
            r#"
        INSERT INTO posts (post_id, title, description, published_at, tags, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(&post_id)
        .bind(&payload.title)
        .bind(&payload.description)
        .bind(&payload.published_at)
        .bind(&payload.tags)
        .bind(&payload.content)
        .bind(&user_id)
        .bind(&payload.status)
        .execute(&data.db)
        .await;

        match result {
            Ok(_) => {
                println!("Post saved successfully with ID: {}", post_id);
                Ok(Json(CustomResponse {
                    status: true,
                    data: Some(UploadResponse { post_id }),
                    message: Some(String::from("포스트를 업로드 했습니다.")),
                }))
            }
            Err(err) => {
                eprintln!("Error saving post: {}", err);
                Err(Error::from_string(
                    format!("Error upload posts: {}", err),
                    poem::http::StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
