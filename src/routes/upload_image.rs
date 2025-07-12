use std::{env, path::PathBuf, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Multipart},
    Error, Request,
};
use tokio::fs;
use tyange_cms_backend::auth::jwt::Claims;
use uuid::Uuid;

use crate::{
    models::{AppState, CustomResponse, UploadImageResponse},
    utils::token::get_user_id_from_token,
};

#[handler]
pub async fn upload_image(
    req: &Request,
    mut multipart: Multipart,
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
                    let original_filename = &field.file_name().unwrap_or("unknown").to_owned();

                    let extension = std::path::Path::new(original_filename)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("jpg");

                    let data = field.bytes().await.map_err(|e| {
                        Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                    })?;

                    let upload_base_path =
                        env::var("UPLOAD_PATH").unwrap_or_else(|_| "uploads/images".to_string());

                    let file_name = format!("{}.{}", Uuid::new_v4(), extension);
                    let mut file_path = PathBuf::from(upload_base_path);
                    file_path.push(file_name);

                    fs::create_dir_all(file_path.parent().unwrap())
                        .await
                        .map_err(|e| {
                            Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                        })?;

                    fs::write(&file_path, &data).await.map_err(|e| {
                        Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                    })?;

                    println!("이미지 저장 완료: {}", file_path.display());

                    return Ok(Json(CustomResponse {
                        status: true,
                        data: Some(UploadImageResponse {
                            image_path: file_path.to_string_lossy().to_string(),
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
