use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};

use crate::{
    models::{AppState, CustomResponse, SignupRequest},
    routes::add_user::create_user,
};

const MIN_PASSWORD_LENGTH: usize = 8;

#[handler]
pub async fn signup(
    Json(payload): Json<SignupRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<()>>, Error> {
    validate_email(&payload.email)?;
    validate_password(&payload.password)?;

    create_user(&data.db, &payload.email, &payload.password, "user").await?;

    Ok(Json(CustomResponse {
        status: true,
        data: None,
        message: Some(String::from("회원가입이 완료되었습니다.")),
    }))
}

fn validate_email(email: &str) -> Result<(), Error> {
    let Some((local, domain)) = email.split_once('@') else {
        return Err(Error::from_string(
            "올바른 이메일 형식이 아닙니다.",
            StatusCode::BAD_REQUEST,
        ));
    };

    if local.is_empty()
        || domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
        || email.matches('@').count() != 1
    {
        return Err(Error::from_string(
            "올바른 이메일 형식이 아닙니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    Ok(())
}

fn validate_password(password: &str) -> Result<(), Error> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(Error::from_string(
            format!(
                "비밀번호는 최소 {}자 이상이어야 합니다.",
                MIN_PASSWORD_LENGTH
            ),
            StatusCode::BAD_REQUEST,
        ));
    }

    Ok(())
}
