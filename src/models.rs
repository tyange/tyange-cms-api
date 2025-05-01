use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub post_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadPostRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
}
