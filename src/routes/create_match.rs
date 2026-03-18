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
    models::{CreateMatchRequest, CustomResponse, MatchSummaryResponse},
    routes::match_utils::{
        ensure_user_exists, find_active_match_for_user, to_match_summary, MatchRow,
    },
    AppState,
};

#[handler]
pub async fn create_match(
    req: &Request,
    Json(payload): Json<CreateMatchRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<(StatusCode, Json<CustomResponse<MatchSummaryResponse>>), Error> {
    let user = current_user(req)?;
    let target_user_id = payload.target_user_id.trim();

    if target_user_id.is_empty() {
        return Err(Error::from_string(
            "대상 사용자 ID는 비어 있을 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if target_user_id == user.user_id {
        return Err(Error::from_string(
            "자기 자신에게는 매칭을 신청할 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    ensure_user_exists(&data.db, &user.user_id).await?;
    ensure_user_exists(&data.db, target_user_id).await?;

    if find_active_match_for_user(&data.db, &user.user_id)
        .await?
        .is_some()
        || find_active_match_for_user(&data.db, target_user_id)
            .await?
            .is_some()
    {
        return Err(Error::from_string(
            "이미 활성 매칭 또는 대기 중인 요청이 있습니다.",
            StatusCode::CONFLICT,
        ));
    }

    let result = query(
        r#"
        INSERT INTO user_matches (requester_user_id, target_user_id, status)
        VALUES (?, ?, 'pending')
        "#,
    )
    .bind(&user.user_id)
    .bind(target_user_id)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("매칭 신청 저장 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let summary = to_match_summary(
        MatchRow {
            match_id: result.last_insert_rowid(),
            requester_user_id: user.user_id.clone(),
            target_user_id: target_user_id.to_string(),
            status: "pending".to_string(),
            created_at: crate::routes::match_utils::current_timestamp(),
            responded_at: None,
        },
        &user.user_id,
    );

    Ok((
        StatusCode::CREATED,
        Json(CustomResponse {
            status: true,
            data: Some(summary),
            message: Some("매칭 신청을 보냈습니다.".to_string()),
        }),
    ))
}
