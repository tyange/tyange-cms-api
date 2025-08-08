use std::{env, path::PathBuf, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Multipart, Query},
    Error, Request,
};
use sqlx::query;
use tokio::fs;
use tyange_cms_backend::auth::jwt::Claims;
use uuid::Uuid;

use crate::models::{AppState, CustomResponse, UploadImageQueryParmas, UploadImageResponse};

#[handler]
pub async fn upload_image(
    req: &Request,
    mut multipart: Multipart,
    Query(params): Query<UploadImageQueryParmas>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UploadImageResponse>>, Error> {
    if let Some(token) = req.header("Authorization") {
        let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
            Error::from_string(
                format!("Server configuration error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        let secret_bytes = secret.as_bytes();

        match Claims::validate_token(&token, &secret_bytes) {
            Ok(_) => {
                while let Some(field) = multipart.next_field().await? {
                    let origin_filename = &field.file_name().unwrap_or("unknown").to_owned();

                    let extension = std::path::Path::new(origin_filename)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("jpg");

                    let file_bytes = field.bytes().await.map_err(|e| {
                        Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                    })?;

                    let upload_base_path =
                        env::var("UPLOAD_PATH").unwrap_or_else(|_| ".uploads/images".to_string());

                    let file_name = format!("{}.{}", Uuid::new_v4(), extension);
                    let mut file_path = PathBuf::from(upload_base_path);
                    file_path.push(file_name.clone());

                    fs::create_dir_all(file_path.parent().unwrap())
                        .await
                        .map_err(|e| {
                            Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                        })?;

                    fs::write(&file_path, &file_bytes).await.map_err(|e| {
                        Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                    })?;

                    let image_id = Uuid::new_v4().to_string();

                    let post_id = params.post_id;

                    let image_type = params.image_type.unwrap_or(String::from("in_post"));

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
                        .bind(&extension)
                        .bind(&image_type).execute(&data.db)
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
                    "업로드할 파일이 없습니다.",
                    StatusCode::BAD_REQUEST,
                ))
            }
            Err(e) => Err(e),
        }
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
