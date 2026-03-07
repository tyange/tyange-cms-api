use poem::http::StatusCode;
use poem::{Error, Request};
use sqlx::{query_scalar, Error as SqlxError, Pool, Sqlite};

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub role: String,
}

pub fn current_user(req: &Request) -> Result<&AuthenticatedUser, Error> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| Error::from_string("인증된 사용자 정보가 없습니다.", StatusCode::UNAUTHORIZED))
}

pub async fn ensure_post_owner(
    user: &AuthenticatedUser,
    post_id: &str,
    db: &Pool<Sqlite>,
) -> Result<(), Error> {
    let writer_id: String = query_scalar(
        r#"
        SELECT writer_id FROM posts WHERE post_id = ?
        "#,
    )
    .bind(post_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        eprintln!("Error fetching post owner: {}", err);
        match err {
            SqlxError::RowNotFound => {
                Error::from_string("게시글을 찾을 수 없습니다.", StatusCode::NOT_FOUND)
            }
            _ => Error::from_string("게시글 작성자 조회에 실패했습니다.", StatusCode::INTERNAL_SERVER_ERROR),
        }
    })?;

    if user.role == "admin" || user.user_id == writer_id {
        Ok(())
    } else {
        Err(Error::from_string(
            "본인이 업로드한 게시글만 수정 또는 삭제할 수 있습니다.",
            StatusCode::FORBIDDEN,
        ))
    }
}

pub fn ensure_admin(user: &AuthenticatedUser) -> Result<(), Error> {
    if user.role == "admin" {
        Ok(())
    } else {
        Err(Error::from_string(
            "관리자만 접근할 수 있습니다.",
            StatusCode::FORBIDDEN,
        ))
    }
}
