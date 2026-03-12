use std::{env, path::PathBuf, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Multipart, Query},
    Error, Request,
};
use sqlx::query;
use tokio::fs;
use tyange_cms_api::auth::authorization::current_user;
use uuid::Uuid;

use crate::models::{AppState, CustomResponse, UploadImageQueryParmas, UploadImageResponse};

#[handler]
pub async fn upload_image(
    req: &Request,
    mut multipart: Multipart,
    Query(params): Query<UploadImageQueryParmas>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UploadImageResponse>>, Error> {
    let _user = current_user(req)?;

    while let Some(field) = multipart.next_field().await? {
        let Some(origin_filename) = field.file_name().map(|name| name.to_owned()) else {
            continue;
        };

        let content_type = field
            .content_type()
            .map(|mime| mime.to_string())
            .unwrap_or_default();

        if !content_type.starts_with("image/") {
            return Err(Error::from_string(
                format!("이미지 파일만 업로드할 수 있습니다: {}", content_type),
                StatusCode::BAD_REQUEST,
            ));
        }

        let extension = std::path::Path::new(&origin_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.trim().is_empty())
            .unwrap_or_else(|| match content_type.as_str() {
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                "image/svg+xml" => "svg",
                _ => "jpg",
            });

        let file_bytes = field
            .bytes()
            .await
            .map_err(|e| Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?;

        if file_bytes.is_empty() {
            return Err(Error::from_string(
                "비어 있는 파일은 업로드할 수 없습니다.",
                StatusCode::BAD_REQUEST,
            ));
        }

        let upload_base_path =
            env::var("UPLOAD_PATH").unwrap_or_else(|_| ".uploads/images".to_string());

        let file_name = format!("{}.{}", Uuid::new_v4(), extension);
        let mut file_path = PathBuf::from(upload_base_path);
        file_path.push(file_name.clone());

        fs::create_dir_all(file_path.parent().unwrap())
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("디렉토리 생성 실패 ({}): {}", file_path.display(), e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?;

        fs::write(&file_path, &file_bytes).await.map_err(|e| {
            Error::from_string(
                format!("파일 생성 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        let image_id = Uuid::new_v4().to_string();

        let post_id = params.post_id.clone();

        let image_type = params.image_type.clone().unwrap_or(String::from("in_post"));

        let result = query(
            r#"
            INSERT INTO images (image_id, post_id, file_name, origin_name, file_path, mime_type, image_type)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&image_id)
        .bind(&post_id)
        .bind(&file_name)
        .bind(&origin_filename)
        .bind(file_path.to_str())
        .bind(&content_type)
        .bind(&image_type)
        .execute(&data.db)
        .await;

        result.map_err(|err| {
            eprintln!("Error saving image: {}", err);
            Error::from_string(
                format!("Error upload image: {}", err),
                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        println!("이미지 저장 완료: {}", file_path.display());

        let web_accessible_path = format!("/images/{}", file_name);

        return Ok(Json(CustomResponse {
            status: true,
            data: Some(UploadImageResponse {
                image_path: web_accessible_path,
            }),
            message: Some(String::from("이미지 업로드에 성공했습니다.")),
        }));
    }

    Err(Error::from_string(
        "업로드할 이미지 파일이 없습니다.",
        StatusCode::BAD_REQUEST,
    ))
}
