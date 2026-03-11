use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json},
    Error, Request,
};

use crate::{
    models::{CustomResponse, RssSourceListResponse},
    rss_push::list_user_rss_sources,
    AppState,
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_rss_sources(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<RssSourceListResponse>>, Error> {
    let user = current_user(req)?;
    let sources = list_user_rss_sources(&data.db, &user.user_id)
        .await
        .map_err(|err| Error::from_string(err.message, err.status))?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(RssSourceListResponse { sources }),
        message: None,
    }))
}
