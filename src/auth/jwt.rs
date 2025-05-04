use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use poem::{http::StatusCode, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
    token_type: String,
}

impl Claims {
    pub fn new(user_id: &str, token_type: &str, expires_in_minutes: i64) -> Self {
        let expiration = Utc::now()
            .checked_add_signed(Duration::minutes(expires_in_minutes))
            .expect("유효한 타임 스탬프를 생성할 수 없습니다.")
            .timestamp() as usize;

        let iat = Utc::now().timestamp() as usize;

        Self {
            sub: user_id.to_owned(),
            exp: expiration,
            iat,
            token_type: token_type.to_owned(),
        }
    }

    fn to_token(&self, secret: &[u8]) -> Result<String, Error> {
        encode(&Header::default(), &self, &EncodingKey::from_secret(secret))
            .map_err(|e| Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))
    }

    pub fn validate_access_token(token: &str, secret: &[u8]) -> Result<bool, Error> {
        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret),
            &Validation::default(),
        ) {
            Ok(token_data) => match usize::try_from(Utc::now().timestamp()) {
                Ok(converted) => {
                    let exp = token_data.claims.exp;
                    return Ok(converted > exp);
                }
                Err(e) => Err(Error::from_string(
                    e.to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )),
            },
            Err(e) => Err(Error::from_string(e.to_string(), StatusCode::UNAUTHORIZED)),
        }
    }

    pub fn create_access_token(user_id: &str, secret: &[u8]) -> Result<String, Error> {
        let claims = Self::new(user_id, "access", 15);
        claims.to_token(secret)
    }

    pub fn create_refresh_token(user_id: &str, secret: &[u8]) -> Result<String, Error> {
        let claims = Self::new(user_id, "refresh", 7 * 24 * 60);
        claims.to_token(secret)
    }
}
