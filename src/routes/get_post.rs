use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error,
};
use sqlx::{query, query_as, Row, Sqlite};

use crate::models::{Post, PostResponseDb, TagWithCategory};
use crate::AppState;

#[handler]
pub async fn get_post(
    Path(post_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<Post>, Error> {
    let result = query_as::<Sqlite, PostResponseDb>(
        r#"
        SELECT post_id, title, description, published_at, content, status
        FROM posts
        WHERE post_id = ?
        "#,
    )
    .bind(&post_id)
    .fetch_optional(&data.db)
    .await;

    let tags = query(
        r#"
        SELECT t.name, t.category
        FROM post_tags pt
        JOIN tags t ON pt.tag_id = t.tag_id
        WHERE pt.post_id = ?;
        "#,
    )
    .bind(&post_id)
    .fetch_all(&data.db)
    .await;

    match result {
        Ok(Some(db_post)) => {
            let post_response = Post {
                post_id: db_post.post_id,
                title: db_post.title,
                description: db_post.description,
                published_at: db_post.published_at,
                tags: tags
                    .unwrap_or(Vec::new())
                    .iter()
                    .map(|row| TagWithCategory {
                        tag: row.get("tag"),
                        category: row.get("category"),
                    })
                    .collect(),
                content: db_post.content,
                status: db_post.status,
            };
            Ok(Json(post_response))
        }
        Ok(None) => {
            println!("포스트를 찾을 수 없음: {}", post_id);
            Err(Error::from_string(
                "해당 id에 해당하는 포스트가 없네요.",
                StatusCode::NOT_FOUND,
            ))
        }
        Err(err) => Err(Error::from_string(
            format!("Error fetching post: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
