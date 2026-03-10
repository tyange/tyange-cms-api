use poem::{http::StatusCode, Endpoint, Error, Middleware, Request};
use std::sync::Arc;
use tyange_cms_api::auth::{
    api_key::{find_active_api_key_by_raw_key, touch_api_key_last_used},
    authorization::AuthenticatedUser,
};

use crate::{middlewares::auth_middleware::require_jwt_user, models::AppState};

pub struct JwtOrApiKeyAuth;

impl<E: Endpoint> Middleware<E> for JwtOrApiKeyAuth {
    type Output = JwtOrApiKeyAuthImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        JwtOrApiKeyAuthImpl { ep }
    }
}

pub struct JwtOrApiKeyAuthImpl<E> {
    ep: E,
}

async fn authenticated_user_from_api_key(req: &Request) -> Result<AuthenticatedUser, Error> {
    let header_value = req.headers().get("X-API-Key").ok_or_else(|| {
        Error::from_string("X-API-Key header is required", StatusCode::UNAUTHORIZED)
    })?;

    let raw_key = header_value
        .to_str()
        .map_err(|_| Error::from_string("Invalid X-API-Key header", StatusCode::UNAUTHORIZED))?;

    let state = req.data::<Arc<AppState>>().ok_or_else(|| {
        Error::from_string(
            "AppState is not configured",
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let record = find_active_api_key_by_raw_key(&state.db, raw_key)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("API key lookup failed: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?
        .ok_or_else(|| {
            Error::from_string("유효하지 않은 API Key입니다.", StatusCode::UNAUTHORIZED)
        })?;

    touch_api_key_last_used(&state.db, record.id)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("API key update failed: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    Ok(AuthenticatedUser {
        user_id: record.user_id,
        role: record.role,
    })
}

impl<E: Endpoint> Endpoint for JwtOrApiKeyAuthImpl<E> {
    type Output = E::Output;

    async fn call(&self, mut req: Request) -> Result<Self::Output, Error> {
        let user = if req.headers().contains_key("Authorization") {
            require_jwt_user(&req)?
        } else {
            authenticated_user_from_api_key(&req).await?
        };

        req.extensions_mut().insert(user);
        self.ep.call(req).await
    }
}
