use std::env;
use std::sync::Arc;

use crate::{
    models::{CustomResponse, UploadPostRequest, UploadPostResponse},
    AppState,
};
use poem::http::StatusCode;
use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;
use tyange_cms_api::auth::jwt::Claims;
use uuid::Uuid;

#[handler]
pub async fn upload_post(
    req: &Request,
    Json(payload): Json<UploadPostRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UploadPostResponse>>, Error> {
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

        let mut tx = data.db.begin().await.map_err(|e| {
            Error::from_string(
                format!("트랜잭션 시작 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        query(
            r#"
            INSERT INTO posts (post_id, title, description, published_at, content, writer_id, status)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&post_id)
        .bind(&payload.title)
        .bind(&payload.description)
        .bind(&payload.published_at)
        .bind(&payload.content)
        .bind(&user_id)
        .bind(&payload.status)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("포스트 저장 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        for tag in &payload.tags {
            let tag_name = &tag.name;
            if tag_name.is_empty() {
                continue;
            }

            query("INSERT OR IGNORE INTO tags (name, category) VALUES (?, ?)")
                .bind(tag_name)
                .bind(&tag.category)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    Error::from_string(
                        format!("태그 저장 실패: {}", e),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                })?;

            query(
                r#"
                INSERT INTO post_tags (post_id, tag_id)
                SELECT ?, tag_id FROM tags WHERE name = ?
                "#,
            )
            .bind(&post_id)
            .bind(tag_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("포스트-태그 관계 저장 실패: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;
        }

        tx.commit().await.map_err(|e| {
            Error::from_string(
                format!("트랜잭션 커밋 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        println!("Post saved successfully with ID: {}", post_id);
        Ok(Json(CustomResponse {
            status: true,
            data: Some(UploadPostResponse { post_id }),
            message: Some(String::from("포스트를 업로드 했습니다.")),
        }))
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
