use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};
use tyange_cms_api::auth::authorization::current_user;

use crate::{
    models::{CustomResponse, MatchSummaryResponse},
    routes::match_utils::{find_active_match_for_user, to_match_summary},
    AppState,
};

#[handler]
pub async fn get_my_match(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<MatchSummaryResponse>>, Error> {
    let user = current_user(req)?;
    let match_summary = find_active_match_for_user(&data.db, &user.user_id)
        .await?
        .map(|row| to_match_summary(row, &user.user_id));
    let has_match = match_summary.is_some();

    Ok(Json(CustomResponse {
        status: true,
        data: match_summary,
        message: if has_match {
            None
        } else {
            Some("현재 활성 매칭이 없습니다.".to_string())
        },
    }))
}
