use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};
use tyange_cms_api::auth::authorization::current_user;

use crate::{
    models::{CustomResponse, MatchSummaryResponse},
    routes::match_utils::close_active_match,
    AppState,
};

#[handler]
pub async fn delete_my_match(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<MatchSummaryResponse>>, Error> {
    let user = current_user(req)?;
    let existing =
        crate::routes::match_utils::find_active_match_for_user(&data.db, &user.user_id).await?;
    let existing_for_message = existing.clone();

    let response = match existing {
        Some(row) => {
            let next_status = if row.status == "pending" {
                "cancelled"
            } else {
                "unmatched"
            };

            close_active_match(&data.db, &user.user_id, next_status).await?
        }
        None => None,
    };

    Ok(Json(CustomResponse {
        status: true,
        data: response,
        message: Some(match existing_for_message {
            Some(row) if row.status == "pending" => "매칭 요청을 취소했습니다.".to_string(),
            Some(_) => "현재 매칭을 해제했습니다.".to_string(),
            None => "현재 활성 매칭이 없습니다.".to_string(),
        }),
    }))
}
