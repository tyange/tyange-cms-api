use crate::models::{CustomResponse, Post, PostResponseDb, PostsResponse};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::{query, Row};
use std::sync::Arc;

#[handler]
pub async fn get_all_posts(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PostsResponse>>, Error> {
    let result = query(
        r#"
        SELECT * FROM posts ORDER BY published_at DESC
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
                        .map(|db_post| {
                            let post_response_db = PostResponseDb {
                                post_id: db_post.get("post_id"),
                                title: db_post.get("title"),
                                description: db_post.get("description"),
                                published_at: db_post.get("published_at"),
                                tags: db_post.get("tags"),
                                content: db_post.get("content"),
                                status: db_post.get("status"),
                            };
                            Post::from(post_response_db)
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
