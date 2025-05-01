use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, errors::Error, DecodingKey, EncodingKey, Header, Validation};
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
    }

    fn from_token(token: &str, secret: &[u8]) -> Result<Self, Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
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
