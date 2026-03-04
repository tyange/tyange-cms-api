use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Query},
    Error,
};
use sqlx::query_as;

use crate::{
    models::{AppState, SpendingListResponse, SpendingQueryParams, SpendingRecordResponse},
    utils::{current_iso_week_key, normalize_week_key},
};

#[handler]
pub async fn get_spending(
    Query(params): Query<SpendingQueryParams>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<SpendingListResponse>, Error> {
    let week_key = match params.week {
        Some(value) => normalize_week_key(&value).map_err(|_| {
            Error::from_string(
                "week 형식이 올바르지 않습니다. 예: 2026-W10",
                StatusCode::BAD_REQUEST,
            )
        })?,
        None => current_iso_week_key(),
    };

    let records = query_as::<_, SpendingRecordResponse>(
        "SELECT record_id, amount, merchant, transacted_at, created_at
         FROM spending_records
         WHERE week_key = ?
         ORDER BY transacted_at DESC, record_id DESC",
    )
    .bind(&week_key)
    .fetch_all(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("소비 내역 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(SpendingListResponse { week_key, records }))
}
