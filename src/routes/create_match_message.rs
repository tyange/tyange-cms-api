use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;
use tyange_cms_api::auth::authorization::current_user;

use crate::{
    models::{CreateMatchMessageRequest, CustomResponse, MatchMessageResponse},
    routes::match_utils::{
        current_timestamp, find_confirmed_match_for_user, to_match_message_response,
    },
    AppState,
};

#[handler]
pub async fn create_match_message(
    req: &Request,
    Json(payload): Json<CreateMatchMessageRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<(StatusCode, Json<CustomResponse<MatchMessageResponse>>), Error> {
    let user = current_user(req)?;
    let matched = find_confirmed_match_for_user(&data.db, &user.user_id)
        .await?
        .ok_or_else(|| {
            Error::from_string(
                "확정된 매칭 상대가 있어야 메시지를 작성할 수 있습니다.",
                StatusCode::FORBIDDEN,
            )
        })?;

    let content = payload.content.trim();
    if content.is_empty() {
        return Err(Error::from_string(
            "메시지 내용은 비어 있을 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let receiver_user_id = if matched.requester_user_id == user.user_id {
        matched.target_user_id.clone()
    } else {
        matched.requester_user_id.clone()
    };

    let result = query(
        r#"
        INSERT INTO match_messages (match_id, sender_user_id, receiver_user_id, content)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(matched.match_id)
    .bind(&user.user_id)
    .bind(&receiver_user_id)
    .bind(content)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("메시지 저장 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CustomResponse {
            status: true,
            data: Some(to_match_message_response((
                result.last_insert_rowid(),
                matched.match_id,
                user.user_id.clone(),
                receiver_user_id,
                content.to_string(),
                current_timestamp(),
            ))),
            message: Some("메시지를 저장했습니다.".to_string()),
        }),
    ))
}
