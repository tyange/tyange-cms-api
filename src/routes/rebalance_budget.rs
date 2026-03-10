use std::sync::Arc;

use chrono::NaiveDate;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error, Request,
};
use sqlx::{query, query_scalar};

use crate::{
    budget::{
        allocate_amounts_by_days, allocate_signed_amounts_by_days, collect_iso_week_days,
        iso_week_key_from_date,
    },
    models::{
        AppState, BudgetRebalanceRequest, BudgetRebalanceResponse, BudgetRebalanceWeekItem,
        CustomResponse,
    },
};
use tyange_cms_api::auth::authorization::current_user;

#[handler]
pub async fn rebalance_budget(
    req: &Request,
    data: Data<&Arc<AppState>>,
    Json(payload): Json<BudgetRebalanceRequest>,
) -> Result<Json<CustomResponse<BudgetRebalanceResponse>>, Error> {
    let user = current_user(req)?;

    if payload.total_budget <= 0 {
        return Err(Error::from_string(
            "total_budget는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let from_date = parse_naive_date(&payload.from_date, "from_date")?;
    let to_date = parse_naive_date(&payload.to_date, "to_date")?;
    let as_of_date = parse_naive_date(&payload.as_of_date, "as_of_date")?;

    if to_date < from_date {
        return Err(Error::from_string(
            "to_date는 from_date보다 빠를 수 없습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    if as_of_date > to_date {
        return Err(Error::from_string(
            "as_of_date가 기간 종료일을 초과했습니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let alert_threshold = payload.alert_threshold.unwrap_or(0.85);
    if !(0.0..=1.0).contains(&alert_threshold) {
        return Err(Error::from_string(
            "alert_threshold는 0.0 이상 1.0 이하여야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let spent_so_far = match payload.spent_so_far {
        Some(spent_so_far) => {
            if spent_so_far < 0 {
                return Err(Error::from_string(
                    "spent_so_far는 0 이상이어야 합니다.",
                    StatusCode::BAD_REQUEST,
                ));
            }
            spent_so_far
        }
        None => {
            let spent_range_end = as_of_date.min(to_date);
            query_scalar::<_, i64>(
                "SELECT COALESCE(SUM(amount), 0)
                 FROM spending_records
                 WHERE owner_user_id = ?
                   AND date(transacted_at) >= date(?)
                   AND date(transacted_at) <= date(?)",
            )
            .bind(&user.user_id)
            .bind(from_date.format("%Y-%m-%d").to_string())
            .bind(spent_range_end.format("%Y-%m-%d").to_string())
            .fetch_one(&data.db)
            .await
            .map_err(|e| {
                Error::from_string(
                    format!("누적 소비 조회 실패: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            })?
        }
    };

    let remaining_budget = payload.total_budget - spent_so_far;
    let rebalance_start = if as_of_date < from_date {
        from_date
    } else {
        as_of_date
    };
    let rebalance_from_week = iso_week_key_from_date(rebalance_start);

    let days_by_week = collect_iso_week_days(from_date, to_date, rebalance_start)?;
    let weekly_limits = allocate_amounts_by_days(
        &days_by_week
            .iter()
            .map(|(_, days)| *days)
            .collect::<Vec<u32>>(),
        remaining_budget,
        true,
    );
    let projected_remaining = allocate_signed_amounts_by_days(
        &days_by_week
            .iter()
            .map(|(_, days)| *days)
            .collect::<Vec<u32>>(),
        remaining_budget,
    );

    let mut tx = data.db.begin().await.map_err(|e| {
        Error::from_string(
            format!("트랜잭션 시작 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    // 저장용 한도는 0 이상으로 유지하고, 표시용 잔여 예산은 별도 컬럼에 보존한다.
    for (((week_key, _days), weekly_limit), projected_remaining) in days_by_week
        .iter()
        .zip(weekly_limits.iter())
        .zip(projected_remaining.iter())
    {
        query(
            "INSERT INTO budget_config (
                 owner_user_id,
                 week_key,
                 weekly_limit,
                 projected_remaining,
                 alert_threshold
             )
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(owner_user_id, week_key) DO UPDATE SET
                 weekly_limit = excluded.weekly_limit,
                 projected_remaining = excluded.projected_remaining,
                 alert_threshold = excluded.alert_threshold",
        )
        .bind(&user.user_id)
        .bind(week_key)
        .bind(*weekly_limit)
        .bind(*projected_remaining)
        .bind(alert_threshold)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            Error::from_string(
                format!("재분배 예산 저장 실패: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    }

    tx.commit().await.map_err(|e| {
        Error::from_string(
            format!("트랜잭션 커밋 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let weeks = days_by_week
        .into_iter()
        .zip(weekly_limits.into_iter())
        .zip(projected_remaining.into_iter())
        .map(
            |(((week_key, days), weekly_limit), projected_remaining)| BudgetRebalanceWeekItem {
                week_key,
                days,
                weekly_limit,
                projected_remaining,
            },
        )
        .collect::<Vec<BudgetRebalanceWeekItem>>();

    Ok(Json(CustomResponse {
        status: true,
        data: Some(BudgetRebalanceResponse {
            total_budget: payload.total_budget,
            from_date: payload.from_date,
            to_date: payload.to_date,
            as_of_date: payload.as_of_date,
            spent_so_far,
            remaining_budget,
            rebalance_from_week,
            is_overspent: remaining_budget < 0,
            weeks,
        }),
        message: Some(String::from(
            "실제 소비를 반영해 남은 기간의 주차별 예산을 재분배했습니다.",
        )),
    }))
}

fn parse_naive_date(value: &str, field_name: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").map_err(|_| {
        Error::from_string(
            format!("{field_name} 형식이 올바르지 않습니다. 예: 2026-03-05"),
            StatusCode::BAD_REQUEST,
        )
    })
}
