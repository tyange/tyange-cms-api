use poem::{handler, web::Json, Error};

use crate::models::{UploadPostRequest, UploadResponse};

#[handler]
pub async fn upload_post(
    Json(payload): Json<UploadPostRequest>,
) -> Result<Json<UploadResponse>, Error> {
    println!("Received title: {}", payload.title);
    println!("Received content: {}", payload.content);

    Ok(Json(UploadResponse {
        post_id: String::from("HIROO"),
    }))
}
