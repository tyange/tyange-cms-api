use crate::models::{CustomResponse, Post, PostResponseDb, PostsResponse, SearchParams};
use crate::AppState;
use poem::http::StatusCode;
use poem::web::{Data, Json, Query};
use poem::{handler, Error};
use sqlx::{QueryBuilder, Row};
use std::sync::Arc;

#[handler]
pub async fn get_posts_with_search_params(
    Query(search_params): Query<SearchParams>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<PostsResponse>>, Error> {
    let mut builder = QueryBuilder::new("SELECT * FROM posts WHERE status != 'draft'");

    if let Some(tag) = search_params.tag {
        builder.push(" AND tags LIKE ");
        builder.push_bind(format!("%{}%", tag));
    }

    builder.push(" ORDER BY published_at DESC, ROWID DESC");

    let result = builder.build().fetch_all(&data.db).await;

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
