use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_scalar;

use crate::models::{AppState, CreateApiKeyRequest, CreateApiKeyResponse};
use tyange_cms_api::auth::{api_key::create_api_key, authorization::current_user};

#[handler]
pub async fn create_api_key_handler(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), Error> {
    let user = current_user(req)?;
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(Error::from_string(
            "name은 비어 있을 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let (id, raw_key) = create_api_key(&data.db, &user.user_id, name, &user.role)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("API key 생성 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    let created = query_scalar::<_, String>(
        r#"
        SELECT created_at FROM api_keys WHERE api_key_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("생성된 API key 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id,
            name: name.to_string(),
            api_key: raw_key,
            created_at: created,
            last_used_at: None,
            revoked_at: None,
        }),
    ))
}
