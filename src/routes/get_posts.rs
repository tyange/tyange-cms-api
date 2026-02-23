use crate::models::{CustomResponse, Post, PostResponseDb, PostsResponse};
use crate::utils::parse_tags;
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::query_as;
use std::sync::Arc;

#[handler]
pub async fn get_posts(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PostsResponse>>, Error> {
    let db_posts = query_as::<_, PostResponseDb>(
        r#"
        SELECT p.post_id, p.title, p.description, p.published_at,
        p.content, p.status,
        IFNULL(GROUP_CONCAT(t.category || '::' || t.name, ','), '') AS tags
        FROM posts p
        LEFT JOIN post_tags pt ON p.post_id = pt.post_id
        LEFT JOIN tags t ON pt.tag_id = t.tag_id
        WHERE p.status != 'draft'
        GROUP BY p.post_id
        ORDER BY p.published_at DESC, p.created_at DESC
        "#,
    )
    .fetch_all(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("Error fetching posts: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if db_posts.is_empty() {
        return Ok(Json(CustomResponse {
            status: true,
            data: Some(PostsResponse { posts: vec![] }),
            message: Some(String::from("포스트가 하나도 없네요.")),
        }));
    }

    let posts = db_posts
        .into_iter()
        .map(|db_post| Post {
            post_id: db_post.post_id,
            title: db_post.title,
            description: db_post.description,
            published_at: db_post.published_at,
            content: db_post.content,
            status: db_post.status,
            tags: parse_tags(&db_post.tags),
        })
        .collect();

    Ok(Json(CustomResponse {
        status: true,
        data: Some(PostsResponse { posts }),
        message: None,
    }))
}
