use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_as;
use tyange_cms_api::auth::authorization::current_user;

use crate::{
    models::{CustomResponse, MatchMessagesResponse},
    routes::match_utils::{
        find_confirmed_match_for_user, to_match_message_response, to_match_summary,
    },
    AppState,
};

#[handler]
pub async fn get_match_messages(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<MatchMessagesResponse>>, Error> {
    let user = current_user(req)?;
    let matched = find_confirmed_match_for_user(&data.db, &user.user_id)
        .await?
        .ok_or_else(|| {
            Error::from_string(
                "확정된 매칭 상대가 있어야 메시지를 볼 수 있습니다.",
                StatusCode::FORBIDDEN,
            )
        })?;

    let summary = to_match_summary(matched.clone(), &user.user_id);
    let rows = query_as::<_, (i64, i64, String, String, String, String)>(
        r#"
        SELECT
            message_id,
            match_id,
            sender_user_id,
            receiver_user_id,
            content,
            created_at
        FROM match_messages
        WHERE match_id = ?
        ORDER BY created_at ASC, message_id ASC
        "#,
    )
    .bind(matched.match_id)
    .fetch_all(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("메시지 목록 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(MatchMessagesResponse {
            match_id: matched.match_id,
            counterpart_user_id: summary.counterpart_user_id,
            messages: rows.into_iter().map(to_match_message_response).collect(),
        }),
        message: None,
    }))
}
