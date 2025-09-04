use std::env;

use poem::{http::StatusCode, Error};
use tyange_cms_api::auth::jwt::Claims;

pub fn get_user_id_from_token(token: &str) -> Result<String, Error> {
    let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
        Error::from_string(
            format!("Server configuration error: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;
    let secret_bytes = secret.as_bytes();
    let decoded_token = Claims::from_token(&token, &secret_bytes)?;

    Ok(decoded_token.claims.sub)
}
