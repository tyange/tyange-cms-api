use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error, Request,
};
use sqlx::{query, query_as};

use crate::{
    budget_periods::{date_in_period, get_active_budget_period},
    models::{AppState, SpendingRecordResponse, UpdateSpendingRequest},
    utils::parse_transacted_at,
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn update_spending(
    req: &Request,
    Path(record_id): Path<i64>,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<UpdateSpendingRequest>,
) -> Result<Json<SpendingRecordResponse>, Error> {
    let user = current_user(req)?;
    if payload.amount <= 0 {
        return Err(Error::from_string(
            "amount는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let transacted_at = parse_transacted_at(&payload.transacted_at).map_err(|_| {
        Error::from_string(
            "transacted_at 형식이 올바르지 않습니다. 예: 2026-03-03T12:20:00",
            StatusCode::BAD_REQUEST,
        )
    })?;

    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("예산 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?
        .ok_or_else(|| {
            Error::from_string("현재 활성 기간 예산이 없습니다.", StatusCode::NOT_FOUND)
        })?;

    if !date_in_period(&transacted_at, &budget.from_date, &budget.to_date) {
        return Err(Error::from_string(
            "transacted_at가 현재 활성 예산 기간 밖에 있습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let transacted_at_text = transacted_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let updated = query(
        "UPDATE spending_records
         SET amount = ?, merchant = ?, transacted_at = ?
         WHERE record_id = ? AND owner_user_id = ?",
    )
    .bind(payload.amount)
    .bind(payload.merchant)
    .bind(&transacted_at_text)
    .bind(record_id)
    .bind(&user.user_id)
    .execute(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("소비 기록 수정 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    if updated.rows_affected() == 0 {
        return Err(Error::from_string(
            format!(
                "해당 소비 기록(record_id={})을 찾을 수 없습니다.",
                record_id
            ),
            StatusCode::NOT_FOUND,
        ));
    }

    let record = query_as::<_, SpendingRecordResponse>(
        "SELECT record_id, amount, merchant, transacted_at, created_at
         FROM spending_records
         WHERE record_id = ? AND owner_user_id = ?",
    )
    .bind(record_id)
    .bind(&user.user_id)
    .fetch_one(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("수정된 소비 기록 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(record))
}
