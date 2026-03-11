use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Path},
    Error, Request,
};

use crate::{rss_push::delete_user_rss_subscription, AppState};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn delete_rss_subscription(
    req: &Request,
    Path(source_id): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<StatusCode, Error> {
    let user = current_user(req)?;
    let deleted = delete_user_rss_subscription(&data.db, &user.user_id, &source_id)
        .await
        .map_err(|err| Error::from_string(err.message, err.status))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(Error::from_string(
            "해당 RSS 구독을 찾을 수 없습니다.",
            StatusCode::NOT_FOUND,
        ))
    }
}
