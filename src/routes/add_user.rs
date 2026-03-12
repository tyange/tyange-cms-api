use crate::models::{AddUserRequest, AppState, CustomResponse};
use bcrypt::{hash, DEFAULT_COST};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};
use sqlx::{query, Error as SqlxError, Pool, Sqlite};
use std::sync::Arc;

pub async fn create_user(
    db: &Pool<Sqlite>,
    user_id: &str,
    password: &str,
    user_role: &str,
) -> Result<(), Error> {
    let hashed_password = hash(password, DEFAULT_COST).map_err(|e| {
        Error::from_string(
            format!("Password hashing failed: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    query(
        r#"
        INSERT INTO users (user_id, password, user_role, auth_provider)
        VALUES (?, ?, ?, 'local')
        "#,
    )
    .bind(user_id)
    .bind(&hashed_password)
    .bind(user_role)
    .execute(db)
    .await
    .map_err(|err| match err {
        SqlxError::Database(db_err) if db_err.is_unique_violation() => {
            Error::from_string("이미 존재하는 사용자입니다.", StatusCode::CONFLICT)
        }
        _ => {
            eprintln!("Error adding user: {}", err);
            Error::from_string(
                format!("Error adding user: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    })?;

    Ok(())
}

#[handler]
pub async fn add_user(
    Json(payload): Json<AddUserRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<()>>, Error> {
    create_user(
        &data.db,
        &payload.user_id,
        &payload.password,
        &payload.user_role,
    )
    .await?;

    println!("User added successfully: {}", payload.user_id);
    Ok(Json(CustomResponse {
        status: true,
        data: None,
        message: Some(String::from("사용자를 추가했습니다.")),
    }))
}
