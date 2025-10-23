use std::{env, sync::Arc};

use bcrypt::verify;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Body, Response,
};

use sqlx::Row;
use tyange_cms_api::auth::jwt::Claims;

use crate::{
    models::{LoginRequest, LoginResponse},
    AppState,
};

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
        let access_token_secret = match env::var("JWT_ACCESS_SECRET") {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Server configuration error: {:?}", e);
                return Err(poem::Error::from_string(
                    "Server configuration error.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                ));
            }
        };
        let refresh_token_secret = match env::var("JWT_REFRESH_SECRET") {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Server configuration error: {:?}", e);
                return Err(poem::Error::from_string(
                    "Server configuration error.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                ));
            }
        };

        let access_token_secret_bytes = access_token_secret.as_bytes();
        let access_token = Claims::create_access_token(&user_id, &access_token_secret_bytes)
            .map_err(|e| {
                eprintln!("Server configuration error: {:?}", e);
                poem::Error::from_string(
                    "Can not create access token.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

        let refresh_token_secret_bytes = refresh_token_secret.as_bytes();
        let refresh_token = Claims::create_refresh_token(&user_id, &refresh_token_secret_bytes)
            .map_err(|e| {
                eprintln!("Server configuration error: {:?}", e);
                poem::Error::from_string(
                    "Can not create refresh token.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

        let login_response = LoginResponse {
            access_token,
            refresh_token,
        };

        let json_body = serde_json::to_string(&login_response).map_err(|_| {
            poem::Error::from_string(
                "JSON serialization error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        println!("로그인 성공: {}", user_id);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .content_type("application/json")
            .body(Body::from(json_body)))
    } else {
        Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .content_type("application/json")
            .body(Body::from_string(String::from(
                "로그인 실패: 잘못된 비밀번호",
            ))))
    }
}
