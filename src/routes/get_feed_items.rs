use std::sync::Arc;

use poem::{
    handler,
    web::{Data, Json, Query},
    Error, Request,
};

use crate::{
    models::{CustomResponse, FeedItemsQuery, FeedItemsResponse},
    rss_push::list_user_feed_items,
    AppState,
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_feed_items(
    req: &Request,
    Query(params): Query<FeedItemsQuery>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<FeedItemsResponse>>, Error> {
    let user = current_user(req)?;
    let response = list_user_feed_items(&data.db, &user.user_id, params)
        .await
        .map_err(|err| Error::from_string(err.message, err.status))?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(response),
        message: None,
    }))
}
