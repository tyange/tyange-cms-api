use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};

use crate::{
    models::{CreateRssSourceRequest, CreateRssSourceResponse, CustomResponse},
    rss_push::create_or_subscribe_rss_source,
    AppState,
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn create_rss_source(
    req: &Request,
    Json(payload): Json<CreateRssSourceRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<CreateRssSourceResponse>>, Error> {
    let user = current_user(req)?;
    let source = create_or_subscribe_rss_source(&data.db, &user.user_id, &payload.feed_url)
        .await
        .map_err(|err| Error::from_string(err.message, err.status))?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(source),
        message: Some("RSS 구독이 저장되었습니다.".to_string()),
    }))
}
