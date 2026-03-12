use std::env;

use poem::{http::StatusCode, Error};
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
pub struct GoogleTokenVerifier {
    http_client: Client,
    tokeninfo_url: String,
    allow_fake_tokens_for_tests: bool,
}

#[derive(Debug, Clone)]
pub struct VerifiedGoogleUser {
    pub email: String,
    pub google_sub: String,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenInfoResponse {
    aud: Option<String>,
    iss: Option<String>,
    sub: Option<String>,
    email: Option<String>,
    #[serde(default, deserialize_with = "deserialize_google_bool")]
    email_verified: bool,
    exp: Option<String>,
}

impl GoogleTokenVerifier {
    pub fn from_env() -> Self {
        let tokeninfo_url = env::var("GOOGLE_TOKENINFO_URL")
            .unwrap_or_else(|_| "https://oauth2.googleapis.com/tokeninfo".to_string());

        Self {
            http_client: Client::new(),
            tokeninfo_url,
            allow_fake_tokens_for_tests: env::var("ALLOW_FAKE_GOOGLE_ID_TOKEN_FOR_TESTS")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }

    pub async fn verify_id_token(
        &self,
        id_token: &str,
        client_id: &str,
    ) -> Result<VerifiedGoogleUser, Error> {
        let token_info =
            if self.allow_fake_tokens_for_tests && id_token.trim_start().starts_with('{') {
                serde_json::from_str::<GoogleTokenInfoResponse>(id_token).map_err(|_| {
                    Error::from_string("Invalid Google ID token.", StatusCode::UNAUTHORIZED)
                })?
            } else {
                let response = self
                    .http_client
                    .get(&self.tokeninfo_url)
                    .query(&[("id_token", id_token)])
                    .send()
                    .await
                    .map_err(|e| {
                        eprintln!("Google token verification request failed: {}", e);
                        Error::from_string(
                            "Failed to verify Google ID token.",
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    })?;

                if !response.status().is_success() {
                    return Err(Error::from_string(
                        "Invalid Google ID token.",
                        StatusCode::UNAUTHORIZED,
                    ));
                }

                response
                    .json::<GoogleTokenInfoResponse>()
                    .await
                    .map_err(|e| {
                        eprintln!("Google token verification response parse failed: {}", e);
                        Error::from_string(
                            "Failed to verify Google ID token.",
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    })?
            };

        validate_google_token_info(&token_info, client_id)
    }
}

fn validate_google_token_info(
    token_info: &GoogleTokenInfoResponse,
    client_id: &str,
) -> Result<VerifiedGoogleUser, Error> {
    let audience = token_info.aud.as_deref().unwrap_or_default();
    if audience != client_id {
        return Err(Error::from_string(
            "Invalid Google ID token audience.",
            StatusCode::UNAUTHORIZED,
        ));
    }

    let issuer = token_info.iss.as_deref().unwrap_or_default();
    if issuer != "accounts.google.com" && issuer != "https://accounts.google.com" {
        return Err(Error::from_string(
            "Invalid Google ID token issuer.",
            StatusCode::UNAUTHORIZED,
        ));
    }

    if !token_info.email_verified {
        return Err(Error::from_string(
            "Google account email is not verified.",
            StatusCode::UNAUTHORIZED,
        ));
    }

    let email = token_info
        .email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::from_string("Google account email is missing.", StatusCode::UNAUTHORIZED)
        })?;

    let google_sub = token_info
        .sub
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::from_string(
                "Google account subject is missing.",
                StatusCode::UNAUTHORIZED,
            )
        })?;

    if let Some(exp) = token_info.exp.as_deref() {
        let expires_at = exp.parse::<i64>().map_err(|_| {
            Error::from_string(
                "Invalid Google ID token expiration.",
                StatusCode::UNAUTHORIZED,
            )
        })?;
        if chrono::Utc::now().timestamp() >= expires_at {
            return Err(Error::from_string(
                "Google ID token has expired.",
                StatusCode::UNAUTHORIZED,
            ));
        }
    }

    Ok(VerifiedGoogleUser {
        email: email.to_ascii_lowercase(),
        google_sub: google_sub.to_string(),
    })
}

fn deserialize_google_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolValue {
        Bool(bool),
        String(String),
    }

    let value = Option::<BoolValue>::deserialize(deserializer)?;
    Ok(match value {
        Some(BoolValue::Bool(value)) => value,
        Some(BoolValue::String(value)) => value.eq_ignore_ascii_case("true"),
        None => false,
    })
}
