use crate::models::{CustomResponse, Post, PostResponse, UpdatePostRequest};
use crate::AppState;
use poem::{handler, Error};
use poem::http::{StatusCode};
use poem::web::{Data, Json, Path};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn update_post(
    Path(post_id): Path<String>,
    Json(payload): Json<UpdatePostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<Post>>, Error> {
    let result = query(
        r"
        UPDATE posts SET title = $1, description = $2, published_at = $3, tags = $4, content = $5 WHERE post_id = $6
        ",
    ).bind(&payload.title).bind(&payload.description).bind(&payload.published_at).bind(&payload.tags).bind(&payload.content).bind(&post_id).execute(&data.db).await;

    match result {
        Ok(_) => Ok(Json(CustomResponse {
            status: true,
            data: Some(Post {
                post_id,
                title: payload.title,
                description: payload.description,
                published_at: payload.published_at,
                tags: payload
                    .tags
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                content: payload.content
            }),
            message: Some(String::from("포스트를 업데이트 했습니다.")),
        })),
        Err(err) => {
            eprintln!("Error update posts: {}", err);
            Err(Error::from_string(
                "Failed to update post.",
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
