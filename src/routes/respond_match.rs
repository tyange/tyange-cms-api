use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error, Request,
};
use sqlx::query;
use tyange_cms_api::auth::authorization::current_user;

use crate::{
    models::{CustomResponse, MatchSummaryResponse, RespondMatchRequest},
    routes::match_utils::{current_timestamp, find_pending_match_by_id, to_match_summary},
    AppState,
};

#[handler]
pub async fn respond_match(
    req: &Request,
    Path(match_id): Path<i64>,
    Json(payload): Json<RespondMatchRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<MatchSummaryResponse>>, Error> {
    let user = current_user(req)?;
    let action = payload.action.trim().to_lowercase();
    let next_status = match action.as_str() {
        "accept" => "matched",
        "reject" => "rejected",
        _ => {
            return Err(Error::from_string(
                "action은 accept 또는 reject만 허용됩니다.",
                StatusCode::BAD_REQUEST,
            ))
        }
    };

    let row = find_pending_match_by_id(&data.db, match_id)
        .await?
        .ok_or_else(|| {
            Error::from_string(
                "대기 중인 매칭 요청을 찾을 수 없습니다.",
                StatusCode::NOT_FOUND,
            )
        })?;

    if row.target_user_id != user.user_id {
        return Err(Error::from_string(
            "대상 사용자만 이 요청에 응답할 수 있습니다.",
            StatusCode::FORBIDDEN,
        ));
    }

    let responded_at = current_timestamp();
    query(
        r#"
        UPDATE user_matches
        SET status = ?, responded_at = ?, closed_at = CASE WHEN ? = 'rejected' THEN ? ELSE closed_at END
        WHERE match_id = ? AND status = 'pending'
        "#,
    )
    .bind(next_status)
    .bind(&responded_at)
    .bind(next_status)
    .bind(&responded_at)
    .bind(match_id)
    .execute(&data.db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("매칭 응답 저장 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(to_match_summary(
            crate::routes::match_utils::MatchRow {
                status: next_status.to_string(),
                responded_at: Some(responded_at),
                ..row
            },
            &user.user_id,
        )),
        message: Some(if next_status == "matched" {
            "매칭을 수락했습니다.".to_string()
        } else {
            "매칭을 거절했습니다.".to_string()
        }),
    }))
}
