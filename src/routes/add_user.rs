use crate::models::{AddUserRequest, AppState, CustomResponse};
use bcrypt::{hash, DEFAULT_COST};
use poem::{
    handler,
    web::{Data, Json},
    Error,
};
use sqlx::query;
use std::sync::Arc;

#[handler]
pub async fn add_user(
    Json(payload): Json<AddUserRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<()>>, Error> {
    // 패스워드 해싱
    let hashed_password = hash(&payload.password, DEFAULT_COST).map_err(|e| {
        Error::from_string(
            format!("Password hashing failed: {}", e),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let result = query(
        r#"
        INSERT INTO users (user_id, password, user_role)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&payload.user_id)
    .bind(&hashed_password) // 해싱된 패스워드 사용
    .bind(&payload.user_role)
    .execute(&data.db)
    .await;

    match result {
        Ok(_) => {
            println!("User added successfully: {}", payload.user_id);
            Ok(Json(CustomResponse {
                status: true,
                data: None,
                message: Some(String::from("사용자를 추가했습니다.")),
            }))
        }
        Err(err) => {
            eprintln!("Error adding user: {}", err);
            Err(Error::from_string(
                format!("Error adding user: {}", err),
                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
