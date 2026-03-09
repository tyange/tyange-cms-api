use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_as;

use crate::models::{ApiKeyListResponse, ApiKeyResponse, AppState};
use tyange_cms_api::auth::{api_key::ApiKeyListItem, authorization::current_user};

#[handler]
pub async fn get_api_keys(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<ApiKeyListResponse>, Error> {
    let user = current_user(req)?;
    let rows = query_as::<_, ApiKeyListItem>(
        r#"
        SELECT api_key_id AS id, name, created_at, last_used_at, revoked_at
        FROM api_keys
        WHERE user_id = ?
        ORDER BY created_at DESC, api_key_id DESC
        "#,
    )
    .bind(&user.user_id)
    .fetch_all(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("API key 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(ApiKeyListResponse {
        api_keys: rows
            .into_iter()
            .map(|row| ApiKeyResponse {
                id: row.id,
                name: row.name,
                created_at: row.created_at,
                last_used_at: row.last_used_at,
                revoked_at: row.revoked_at,
            })
            .collect(),
    }))
}
