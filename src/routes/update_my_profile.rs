use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;
use tyange_cms_api::auth::authorization::current_user;

use crate::models::{AppState, MeResponse, UpdateMyProfileRequest};

const DISPLAY_NAME_LIMIT: usize = 32;
const AVATAR_URL_LIMIT: usize = 512;
const BIO_LIMIT: usize = 160;

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

#[handler]
pub async fn update_my_profile(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<UpdateMyProfileRequest>,
) -> Result<Json<MeResponse>, Error> {
    let user = current_user(req)?;
    let display_name = trim_to_option(&payload.display_name);
    let avatar_url = trim_to_option(&payload.avatar_url);
    let bio = trim_to_option(&payload.bio);

    if display_name.as_deref().map_or(0, str::len) > DISPLAY_NAME_LIMIT {
        return Err(Error::from_string(
            "표시 이름은 32자를 넘길 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if avatar_url.as_deref().map_or(0, str::len) > AVATAR_URL_LIMIT {
        return Err(Error::from_string(
            "프로필 사진 URL은 512자를 넘길 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if bio.as_deref().map_or(0, str::len) > BIO_LIMIT {
        return Err(Error::from_string(
            "소개는 160자를 넘길 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    query(
        r#"
        UPDATE users
        SET display_name = ?, avatar_url = ?, bio = ?
        WHERE user_id = ?
        "#,
    )
    .bind(&display_name)
    .bind(&avatar_url)
    .bind(&bio)
    .bind(&user.user_id)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("사용자 프로필 저장 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(MeResponse {
        user_id: user.user_id.clone(),
        user_role: user.role.clone(),
        display_name,
        avatar_url,
        bio,
    }))
}
