use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::path::PathBuf;

pub struct AppState {
    pub db: Pool<Sqlite>,
    pub upload_dir: PathBuf,
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
        }
    }
}

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

#[derive(Debug, FromRow)]
pub struct PostResponseDb {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: String,
    pub content: String,
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
}

#[derive(Debug, Serialize)]
pub struct DeletePostResponse {
    pub post_id: String,
}
