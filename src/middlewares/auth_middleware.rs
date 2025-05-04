use std::env;

use poem::{http::StatusCode, Endpoint, Error, Middleware, Response};
use tyange_cms_backend::auth::jwt::Claims;

pub struct Auth;

impl<E: Endpoint> Middleware<E> for Auth {
    type Output = AuthImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        AuthImpl(ep)
    }
}

pub struct AuthImpl<E>(E);

impl<E: Endpoint> Endpoint for AuthImpl<E> {
    type Output = Response;

    async fn call(&self, req: poem::Request) -> poem::Result<Self::Output, Error> {
        let token = req.headers().get("Authorization").ok_or_else(|| {
            Error::from_string("Authorization header is required", StatusCode::UNAUTHORIZED)
        })?;

        let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
            Error::from_string(
                format!("Server configuration error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        let secret_bytes = secret.as_bytes();
        let is_valid = Claims::validate_access_token(
            token.to_str().map_err(|e| {
                Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
            })?,
            &secret_bytes,
        )
        .map_err(|e| Error::from_string(e.to_string(), StatusCode::UNAUTHORIZED))?;

        if is_valid {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body("허용된 유저입니다."))
        } else {
            Err(Error::from_string(
                "인증되지 않은 유저는 접근할 수 없습니다.",
                StatusCode::UNAUTHORIZED,
            ))
        }
    }
}
