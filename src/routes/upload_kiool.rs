use std::{env, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query;
use tyange_cms_backend::auth::jwt::Claims;
use uuid::Uuid;

use crate::models::{AppState, CustomResponse, UploadKioolRequest, UploadKioolResponse};

#[handler]
pub async fn upload_kiool(
    req: &Request,
    Json(payload): Json<UploadKioolRequest>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<UploadKioolResponse>>, Error> {
    if let Some(token) = req.header("Authorization") {
        let secret = env::var("JWT_ACCESS_SECRET").map_err(|e| {
            Error::from_string(
                format!("Server configuration error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
        let secret_bytes = secret.as_bytes();
        let decoded_token = Claims::from_token(&token, &secret_bytes)?;

        let user_id = decoded_token.claims.sub;

        let kiool_id = Uuid::new_v4().to_string();

        let result = query(
            r#"
        INSERT INTO kiools (kiool_id, title, description, published_at, tags, content, writer_id, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(&kiool_id)
        .bind(&payload.title)
        .bind(&payload.description)
        .bind(&payload.published_at)
        .bind(&payload.tags)
        .bind(&payload.content)
        .bind(&user_id)
        .bind(&payload.status)
        .execute(&data.db)
        .await;

        match result {
            Ok(_) => {
                println!("Kiool saved successfully with ID: {}", kiool_id);
                Ok(Json(CustomResponse {
                    status: true,
                    data: Some(UploadKioolResponse { kiool_id }),
                    message: Some(String::from("포스트를 업로드 했습니다.")),
                }))
            }
            Err(err) => {
                eprintln!("Error saving kiool: {}", err);
                Err(Error::from_string(
                    format!("Error upload kiools: {}", err),
                    poem::http::StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    } else {
        Err(Error::from_string(
            "토큰을 받지 못했어요.",
            StatusCode::UNAUTHORIZED,
        ))
    }
}
