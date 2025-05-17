use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error,
};
use sqlx::{query_as, Sqlite};

use crate::models::{Post, PostResponseDb};
use crate::{AppState};

#[handler]
pub async fn get_post(
    Path(post_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<Post>, Error> {
    let result = query_as::<Sqlite, PostResponseDb>(
        r#"
        SELECT post_id, title, description, published_at, tags, content
        FROM posts
        WHERE post_id = ?
        "#,
    )
    .bind(&post_id)
    .fetch_optional(&data.db)
    .await;
    
    println!("쿼리 결과: {:?}", result);

    match result {
        Ok(Some(db_post)) => {
            let post_response = Post::from(db_post);
            Ok(Json(post_response))
        }
        Ok(None) => {
            println!("포스트를 찾을 수 없음: {}", post_id); // 디버깅 로그
            Err(Error::from_string(
                "해당 id에 해당하는 포스트가 없네요.",
                StatusCode::NOT_FOUND,
            ))
        },
        Err(err) => {
            Err(Error::from_string(
                format!("Error fetching post: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
