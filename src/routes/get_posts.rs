use crate::models::{Post, PostResponse, PostResponseDb, PostsResponse};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json};
use poem::{handler, Error};
use sqlx::{query, Row};
use std::sync::Arc;

#[handler]
pub async fn get_posts(data: Data<&Arc<AppState>>) -> Result<Json<PostsResponse>, Error> {
    let result = query(
        r#"
        SELECT * FROM posts
        "#,
    )
    .fetch_all(&data.db)
    .await;

    match result {
        Ok(db_posts) => {
            if (db_posts.len() == 0) {
                return Ok(Json(PostsResponse {
                    posts: Vec::<Post>::new(),
                }));
            }

            Ok(Json(PostsResponse {
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
                        };
                        Post::from(post_response_db)
                    })
                    .collect(),
            }))
        }
        Err(err) => {
            eprintln!("Error fetching posts: {}", err);
            Err(Error::from_string(
                "Failed to fetch posts.",
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
