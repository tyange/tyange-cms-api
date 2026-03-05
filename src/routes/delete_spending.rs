use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Path},
    Error,
};
use sqlx::query;

use crate::models::AppState;

#[handler]
pub async fn delete_spending(
    Path(record_id): Path<i64>,
    data: Data<&Arc<AppState>>,
) -> Result<StatusCode, Error> {
    let result = query("DELETE FROM spending_records WHERE record_id = ?")
        .bind(record_id)
        .execute(&data.db)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("소비 기록 삭제 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    if result.rows_affected() == 0 {
        return Err(Error::from_string(
            format!(
                "해당 소비 기록(record_id={})을 찾을 수 없습니다.",
                record_id
            ),
            StatusCode::NOT_FOUND,
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}
