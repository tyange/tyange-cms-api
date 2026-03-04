use std::env;

use poem::{http::StatusCode, Endpoint, Error, Middleware, Request};

pub struct ApiKeyAuth;

impl<E: Endpoint> Middleware<E> for ApiKeyAuth {
    type Output = ApiKeyAuthImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        ApiKeyAuthImpl { ep }
    }
}

pub struct ApiKeyAuthImpl<E> {
    ep: E,
}

impl<E: Endpoint> Endpoint for ApiKeyAuthImpl<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> Result<Self::Output, Error> {
        let header_value = req.headers().get("X-API-Key").ok_or_else(|| {
            Error::from_string("X-API-Key header is required", StatusCode::UNAUTHORIZED)
        })?;

        let incoming_key = header_value.to_str().map_err(|_| {
            Error::from_string("Invalid X-API-Key header", StatusCode::UNAUTHORIZED)
        })?;

        let expected_key = env::var("MACRODROID_API_KEY").map_err(|e| {
            Error::from_string(
                format!("Server configuration error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        if incoming_key != expected_key {
            return Err(Error::from_string(
                "유효하지 않은 API Key입니다.",
                StatusCode::UNAUTHORIZED,
            ));
        }

        self.ep.call(req).await
    }
}
