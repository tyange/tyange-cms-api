use std::{env, sync::Arc};

use bcrypt::verify;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Response,
};

use sqlx::Row;
use tyange_cms_backend::auth::jwt::Claims;

use crate::{models::LoginRequest, AppState};

#[handler]
pub async fn login(
    Json(payload): Json<LoginRequest>,
    data: Data<&Arc<AppState>>,
) -> poem::Result<Response> {
    let user = sqlx::query(
        r#"
        SELECT user_id, password FROM users WHERE user_id = ?
        "#,
    )
    .bind(&payload.user_id)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        eprintln!("Database error: {:?}", e);
        poem::Error::from_string("Database error", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    let row = match user {
        Some(row) => row,
        None => {
            return Err(poem::Error::from_string(
                "Invalid credentials",
                StatusCode::UNAUTHORIZED,
            ))
        }
    };

    let user_id: String = row.try_get("user_id").unwrap_or_default();
    let stored_hash: String = row.try_get("password").unwrap_or_default();

    let password_matches = verify(&payload.password, &stored_hash).unwrap_or(false);

    if password_matches {
        println!("로그인 성공: {}", user_id);

        let access_token_secret = match env::var("JWT_ACCESS_SECRET") {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Server configuration error: {:?}", e);
                return Err(Error::from_string(
                    "Server configuration error.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                ));
            }
        };
        let refresh_token_secret = match env::var("JWT_REFRESH_SECRET") {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Server configuration error: {:?}", e);
                return 
            }
        }

        let access_secret = access_token_secret.as_bytes();
        let access_token = Claims::create_access_token(&user_id, &access_secret);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .body("Login successful"))
    } else {
        println!("로그인 실패: 잘못된 비밀번호");
        Err(poem::Error::from_string(
            "Invalid credentials",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
