use crate::models::{CustomResponse, Post, PostsResponse};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::{query, Row};
use std::sync::Arc;

#[handler]
pub async fn get_posts(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PostsResponse>>, Error> {
    let result = query(
        r#"
        SELECT p.post_id, p.title, p.description, p.published_at,
        p.content, p.status,
        IFNULL(GROUP_CONCAT(t.name, ','), '') AS tags
        FROM posts p
        LEFT JOIN post_tags pt ON p.post_id = pt.post_id
        LEFT JOIN tags t ON pt.tag_id = t.tag_id
        WHERE p.status != 'draft'
        GROUP BY p.post_id
        ORDER BY p.published_at DESC, p.created_at DESC
        "#,
    )
    .fetch_all(&data.db)
    .await;

    match result {
        Ok(db_posts) => {
            if db_posts.len() == 0 {
                return Ok(Json(CustomResponse {
                    status: true,
                    data: Some(PostsResponse {
                        posts: Vec::<Post>::new(),
                    }),
                    message: Some(String::from("포스트가 하나도 없네요.")),
                }));
            }

            Ok(Json(CustomResponse {
                status: true,
                data: Some(PostsResponse {
                    posts: db_posts
                        .iter()
                        .map(|db_post| Post {
                            post_id: db_post.get("post_id"),
                            title: db_post.get("title"),
                            description: db_post.get("description"),
                            published_at: db_post.get("published_at"),
                            tags: {
                                let tags_str: String = db_post.get("tags");
                                if tags_str.is_empty() {
                                    Vec::new()
                                } else {
                                    tags_str.split(',').map(|s| s.trim().to_string()).collect()
                                }
                            },
                            content: db_post.get("content"),
                            status: db_post.get("status"),
                        })
                        .collect(),
                }),
                message: None,
            }))
        }
        Err(err) => Err(Error::from_string(
            format!("Error fetching posts: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
