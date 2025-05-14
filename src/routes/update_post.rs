use crate::models::{CustomResponse, Post, PostResponse, PostResponseDb, UpdatePostRequest};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json, Path};
use poem::{handler, Error, Request};
use sqlx::{query, query_scalar};
use std::env;
use std::sync::Arc;
use tyange_cms_backend::auth::jwt::Claims;

#[handler]
pub async fn update_post(
    req: &Request,
    Path(post_id): Path<String>,
    Json(payload): Json<UpdatePostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<Post>>, Error> {
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
        let writer_id: String = query_scalar(
            r#"
        SELECT writer_id FROM posts WHERE post_id = ?
        "#,
        )
        .bind(&post_id)
        .fetch_one(&data.db)
        .await
        .map_err(|err| {
            eprintln!("Error update posts: {}", err);
            Error::from_string("Failed to get post id.", StatusCode::INTERNAL_SERVER_ERROR)
        })?;

        if user_id != writer_id {
            return Err(Error::from_string(
                "본인이 업로드한 게시글만 수정할 수 있습니다.",
                StatusCode::FORBIDDEN,
            ));
        }

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
                    content: payload.content,
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
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
