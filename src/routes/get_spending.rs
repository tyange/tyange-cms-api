use std::{collections::BTreeMap, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::query_as;

use crate::{
    budget_periods::{get_active_budget_period, sum_spending_for_period},
    models::{AppState, SpendingListResponse, SpendingRecordResponse, SpendingWeekGroup},
    utils::{iso_week_key_from_datetime, parse_transacted_at},
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn get_spending(
    req: &Request,
    data: Data<&Arc<AppState>>,
) -> Result<Json<SpendingListResponse>, Error> {
    let user = current_user(req)?;
    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("예산 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?
        .ok_or_else(|| {
            Error::from_string(
                "현재 활성 기간 예산이 없습니다.",
                StatusCode::NOT_FOUND,
            )
        })?;

    let records = query_as::<_, SpendingRecordResponse>(
        "SELECT record_id, amount, merchant, transacted_at, created_at
         FROM spending_records
         WHERE owner_user_id = ?
           AND date(transacted_at) >= date(?)
           AND date(transacted_at) <= date(?)
         ORDER BY transacted_at DESC, record_id DESC",
    )
    .bind(&user.user_id)
    .bind(&budget.from_date)
    .bind(&budget.to_date)
    .fetch_all(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("소비 내역 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let total_spent = sum_spending_for_period(&data.db, &user.user_id, &budget.from_date, &budget.to_date)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("기간 소비 합계 조회 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    let mut grouped = BTreeMap::<String, Vec<SpendingRecordResponse>>::new();
    for record in records {
        let transacted_at = parse_transacted_at(&record.transacted_at).map_err(|_| {
            Error::from_string(
                "저장된 transacted_at 형식이 올바르지 않습니다.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
        grouped
            .entry(iso_week_key_from_datetime(&transacted_at))
            .or_default()
            .push(record);
    }

    let mut weeks = grouped
        .into_iter()
        .map(|(week_key, records)| {
            let weekly_total = records.iter().map(|record| record.amount).sum::<i64>();
            let record_count = records.len() as i64;
            SpendingWeekGroup {
                week_key,
                weekly_total,
                record_count,
                records,
            }
        })
        .collect::<Vec<_>>();
    weeks.sort_by(|a, b| b.week_key.cmp(&a.week_key));

    Ok(Json(SpendingListResponse {
        budget_id: budget.budget_id,
        from_date: budget.from_date,
        to_date: budget.to_date,
        total_spent,
        remaining_budget: budget.total_budget - total_spent,
        weeks,
    }))
}
