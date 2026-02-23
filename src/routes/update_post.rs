use crate::models::{CustomResponse, Post, TagWithCategory, UpdatePostRequest};
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
                    let mut tx = data.db.begin().await.map_err(|e| {
                        Error::from_string(
                            format!("트랜잭션 시작 실패: {}", e),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    })?;

                    query(
                        r#"
                        UPDATE posts SET title = ?, description = ?, published_at = ?,
                        content = ?, status = ? WHERE post_id = ?
                        "#,
                    )
                    .bind(&payload.title)
                    .bind(&payload.description)
                    .bind(&payload.published_at)
                    .bind(&payload.content)
                    .bind(&payload.status)
                    .bind(&post_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        Error::from_string(
                            format!("포스트 업데이트 실패: {}", e),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    })?;

                    // 2. 기존 태그 관계 삭제
                    query("DELETE FROM post_tags WHERE post_id = ?")
                        .bind(&post_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| {
                            Error::from_string(
                                format!("태그 관계 삭제 실패: {}", e),
                                StatusCode::INTERNAL_SERVER_ERROR,
                            )
                        })?;

                    for tag in &payload.tags {
                        let tag_name = &tag.tag;
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

                    Ok(Json(CustomResponse {
                        status: true,
                        data: Some(Post {
                            post_id,
                            title: payload.title,
                            description: payload.description,
                            published_at: payload.published_at,
                            tags: payload
                                .tags
                                .iter()
                                .map(|tag| TagWithCategory {
                                    tag: String::from(&tag.tag),
                                    category: String::from(&tag.category),
                                })
                                .collect(),
                            content: payload.content,
                            status: payload.status,
                        }),
                        message: Some(String::from("포스트를 업데이트 했습니다.")),
                    }))
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
