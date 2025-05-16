use crate::models::{CustomResponse, DeletePostResponse};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json, Path};
use poem::{handler, Error, Request};
use sqlx::query;
use std::sync::Arc;
use tyange_cms_backend::auth::permission::permission;

#[handler]
pub async fn delete_post(
    req: &Request,
    Path(post_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<DeletePostResponse>>, Error> {
    if let Some(token) = req.header("Authorization") {
        match permission(&token, &post_id, &data.db).await {
            Ok(is_ok_permission) => {
                if is_ok_permission {
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
                            eprintln!("Error delete post: {}", e);
                            Err(Error::from_string(
                                "Failed to delete post",
                                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
                            ))
                        }
                    }
                } else {
                    Err(Error::from_string(
                        "본인이 업로드한 게시글만 삭제할 수 있습니다.",
                        StatusCode::FORBIDDEN,
                    ))
                }
            }
            Err(err) => Err(Error::from_string(
                err.to_string(),
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
