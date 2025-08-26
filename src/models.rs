use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};

pub struct AppState {
    pub db: Pool<Sqlite>,
}

#[derive(Debug, Serialize)]
pub struct CustomResponse<T> {
    pub status: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Post {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<String>,
    pub content: String,
    pub status: String,
}

impl From<PostResponseDb> for Post {
    fn from(db: PostResponseDb) -> Self {
        let tags = if db.tags.is_empty() {
            Vec::new()
        } else {
            db.tags.split(',').map(|s| s.trim().to_string()).collect()
        };
        Self {
            post_id: db.post_id,
            title: db.title,
            description: db.description,
            published_at: db.published_at,
            tags,
            content: db.content,
            status: db.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UploadPostResponse {
    pub post_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadPostRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadKioolResponse {
    pub kiool_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadKioolRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
    pub status: String,
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

#[derive(Debug, FromRow)]
pub struct PostResponseDb {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<String>,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PostsResponse {
    pub posts: Vec<Post>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePostRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct DeletePostResponse {
    pub post_id: String,
}

#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
    pub image_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadImageQueryParmas {
    pub post_id: Option<String>,
    pub image_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddUserRequest {
    pub user_id: String,
    pub password: String,
    pub user_role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Section {
    pub section_id: i32,
    pub section_type: String,
    pub content_data: String,
    pub order_index: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct SectionResponse {
    pub section_id: i32,
    pub section_type: String,
    pub content_data: serde_json::Value,
    pub order_index: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSectionRequest {
    pub section_type: String,
    pub content_data: String,
    pub order_index: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSectionRequest {
    pub section_type: Option<String>,
    pub content_data: Option<String>,
    pub order_index: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateSectionResponse {
    pub section_id: i32,
}

#[derive(Debug, Serialize)]
pub struct DeleteSectionResponse {
    pub section_id: i32,
}
