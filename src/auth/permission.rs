use crate::auth::jwt::Claims;
use poem::http::StatusCode;
use poem::Error;
use sqlx::{query_scalar, Pool, Sqlite};
use std::env;

pub async fn permission(token: &str, post_id: &str, db: &Pool<Sqlite>) -> Result<bool, Error> {
    let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
        Error::from_string(
            format!("Server configuration error: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;
    let secret_bytes = secret.as_bytes();
    let decoded_token = Claims::from_token(&token, &secret_bytes)?;

    let user_id = decoded_token.claims.sub;
    let writer_id: String = query_scalar(
        r#"
        SELECT writer_id FROM posts WHERE post_id = ?
        "#,
    )
    .bind(post_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        eprintln!("Error update posts: {}", err);
        Error::from_string("Failed to get post id.", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(user_id == writer_id)
}
