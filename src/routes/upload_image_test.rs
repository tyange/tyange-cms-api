use std::{env, sync::Arc};

use poem::{
    post,
    test::{TestClient, TestForm, TestFormField},
    Endpoint, EndpointExt, Route,
};
use sqlx::{query_as, query_scalar, SqlitePool};
use tokio::fs;

use crate::{
    db::init_db, middlewares::auth_middleware::Auth, models::AppState,
    routes::upload_image::upload_image,
};
use tyange_cms_api::auth::jwt::Claims;

const TEST_UPLOAD_PATH: &str = "/tmp/tyange-cms-upload-image-tests";

#[derive(sqlx::FromRow)]
struct StoredImageRow {
    post_id: Option<String>,
    origin_name: String,
    mime_type: String,
    image_type: String,
    file_name: String,
}

async fn create_test_state() -> Arc<AppState> {
    env::set_var("JWT_ACCESS_SECRET", "test-access-secret");
    env::set_var("UPLOAD_PATH", TEST_UPLOAD_PATH);

    let _ = fs::remove_dir_all(TEST_UPLOAD_PATH).await;
    fs::create_dir_all(TEST_UPLOAD_PATH)
        .await
        .expect("failed to create upload dir");

    let db = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to connect sqlite");
    init_db(&db).await.expect("failed to init db");

    Arc::new(AppState::new(db))
}

fn create_upload_app(state: Arc<AppState>) -> impl Endpoint {
    Route::new()
        .at("/upload-image", post(upload_image).with(Auth))
        .at("/images/upload", post(upload_image).with(Auth))
        .data(state)
}

fn issue_access_token(user_id: &str, role: &str) -> String {
    Claims::create_access_token(user_id, role, b"test-access-secret")
        .expect("failed to create access token")
}

#[tokio::test]
async fn upload_image_accepts_image_without_post_id() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_upload_app(state.clone()));

    let response = cli
        .post("/images/upload")
        .header("Authorization", issue_access_token("writer-1", "user"))
        .multipart(
            TestForm::new().field(
                TestFormField::bytes(b"fake png bytes")
                    .name("file")
                    .filename("avatar.png")
                    .content_type("image/png"),
            ),
        )
        .send()
        .await;

    response.assert_status_is_ok();
    let json = response.json().await;
    let image_path = json
        .value()
        .object()
        .get("data")
        .object()
        .get("image_path")
        .string()
        .to_string();

    assert!(image_path.starts_with("/images/"));

    let stored: StoredImageRow = query_as(
        "SELECT post_id, origin_name, mime_type, image_type, file_name FROM images LIMIT 1",
    )
    .fetch_one(&state.db)
    .await
    .expect("failed to fetch stored image");

    assert_eq!(stored.post_id, None);
    assert_eq!(stored.origin_name, "avatar.png");
    assert_eq!(stored.mime_type, "image/png");
    assert_eq!(stored.image_type, "in_post");
    assert_eq!(image_path, format!("/images/{}", stored.file_name));

    let saved_bytes = fs::read(format!("{}/{}", TEST_UPLOAD_PATH, stored.file_name))
        .await
        .expect("failed to read uploaded file");
    assert_eq!(saved_bytes, b"fake png bytes");
}

#[tokio::test]
async fn upload_image_rejects_non_image_content_type() {
    let state = create_test_state().await;
    let cli = TestClient::new(create_upload_app(state.clone()));

    let response = cli
        .post("/upload-image")
        .header("Authorization", issue_access_token("writer-1", "user"))
        .multipart(
            TestForm::new().field(
                TestFormField::bytes(b"plain text")
                    .name("file")
                    .filename("note.txt")
                    .content_type("text/plain"),
            ),
        )
        .send()
        .await;

    response.assert_status(poem::http::StatusCode::BAD_REQUEST);

    let saved_count: i64 = query_scalar("SELECT COUNT(*) FROM images")
        .fetch_one(&state.db)
        .await
        .expect("failed to count images");
    assert_eq!(saved_count, 0);
}
