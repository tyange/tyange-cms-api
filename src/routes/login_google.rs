use std::{env, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};
use sqlx::{query, Row};
use tyange_cms_api::auth::google::GoogleTokenVerifier;

use crate::{
    models::{AppState, GoogleLoginRequest, LoginResponse},
    routes::login::issue_login_response,
};

#[handler]
pub async fn login_google(
    Json(payload): Json<GoogleLoginRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<LoginResponse>, Error> {
    let google_client_id = env::var("GOOGLE_CLIENT_ID").map_err(|e| {
        eprintln!("Server configuration error: {:?}", e);
        Error::from_string(
            "Server configuration error.",
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let verified_user = GoogleTokenVerifier::from_env()
        .verify_id_token(&payload.id_token, &google_client_id)
        .await?;

    let existing_google_owner = query(
        r#"
        SELECT user_id FROM users WHERE google_sub = ?
        "#,
    )
    .bind(&verified_user.google_sub)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        eprintln!("Database error: {:?}", e);
        Error::from_string("Database error", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    if let Some(row) = existing_google_owner {
        let owner_user_id: String = row.try_get("user_id").unwrap_or_default();
        if !owner_user_id.eq_ignore_ascii_case(&verified_user.email) {
            return Err(Error::from_string(
                "Google account is already linked to another user.",
                StatusCode::CONFLICT,
            ));
        }
    }

    let existing_user = query(
        r#"
        SELECT user_id, user_role, auth_provider, google_sub
        FROM users
        WHERE lower(user_id) = lower(?)
        "#,
    )
    .bind(&verified_user.email)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        eprintln!("Database error: {:?}", e);
        Error::from_string("Database error", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    let (user_id, user_role) = match existing_user {
        Some(row) => {
            let user_id: String = row.try_get("user_id").unwrap_or_default();
            let user_role: String = row
                .try_get("user_role")
                .unwrap_or_else(|_| "user".to_string());
            let linked_google_sub: Option<String> = row.try_get("google_sub").unwrap_or(None);

            if let Some(linked_google_sub) = linked_google_sub.as_deref() {
                if linked_google_sub != verified_user.google_sub {
                    return Err(Error::from_string(
                        "Google account is already linked to another user.",
                        StatusCode::CONFLICT,
                    ));
                }
            } else {
                query(
                    r#"
                    UPDATE users
                    SET google_sub = ?
                    WHERE user_id = ?
                    "#,
                )
                .bind(&verified_user.google_sub)
                .bind(&user_id)
                .execute(&data.db)
                .await
                .map_err(|e| {
                    eprintln!("Database error: {:?}", e);
                    Error::from_string("Database error", StatusCode::INTERNAL_SERVER_ERROR)
                })?;
            }

            (user_id, user_role)
        }
        None => {
            query(
                r#"
                INSERT INTO users (user_id, password, user_role, auth_provider, google_sub)
                VALUES (?, NULL, 'user', 'google', ?)
                "#,
            )
            .bind(&verified_user.email)
            .bind(&verified_user.google_sub)
            .execute(&data.db)
            .await
            .map_err(|e| {
                eprintln!("Database error: {:?}", e);
                Error::from_string("Database error", StatusCode::INTERNAL_SERVER_ERROR)
            })?;

            (verified_user.email, "user".to_string())
        }
    };

    let response = issue_login_response(&user_id, &user_role)?;
    Ok(Json(response))
}
