use crate::models::{CustomResponse, Post, UpdatePostRequest};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json, Path};
use poem::{handler, Error, Request};
use sqlx::query;
use std::sync::Arc;
use tyange_cms_api::auth::permission::permission;

#[handler]
pub async fn update_post(
    req: &Request,
    Path(post_id): Path<String>,
    Json(payload): Json<UpdatePostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<Post>>, Error> {
    if let Some(token) = req.header("Authorization") {
        match permission(&token, &post_id, &data.db).await {
            Ok(is_ok_permission) => {
                if is_ok_permission {
                    let result = query(
                        r"
                            UPDATE posts SET title = $1, description = $2, published_at = $3, tags = $4, content = $5, status = $6 WHERE post_id = $7
                        ",
                    ).bind(&payload.title).bind(&payload.description).bind(&payload.published_at).bind(&payload.tags).bind(&payload.content).bind(&payload.status).bind(&post_id).execute(&data.db).await;

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
                                status: payload.status,
                            }),
                            message: Some(String::from("포스트를 업데이트 했습니다.")),
                        })),
                        Err(_) => Err(Error::from_string(
                            "Failed to update post.",
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )),
                    }
                } else {
                    Err(Error::from_string(
                        "본인이 업로드한 게시글만 수정할 수 있습니다.",
                        StatusCode::FORBIDDEN,
                    ))
                }
            }
            Err(err) => Err(Error::from_string(
                format!("Error update posts: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )),
        }
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
