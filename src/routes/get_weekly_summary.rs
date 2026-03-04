use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Error,
};
use sqlx::{query_as, FromRow};

use crate::{
    models::{AppState, WeeklySummaryResponse},
    utils::{current_iso_week_key, normalize_week_key},
};

#[derive(FromRow)]
struct ActiveBudget {
    weekly_limit: i64,
    alert_threshold: f64,
}

#[derive(FromRow)]
struct WeeklyAggregate {
    total_spent: i64,
    record_count: i64,
}

#[handler]
pub async fn get_weekly_summary(
    data: Data<&Arc<AppState>>,
) -> Result<Json<WeeklySummaryResponse>, Error> {
    build_weekly_summary(&data, &current_iso_week_key()).await
}

#[handler]
pub async fn get_weekly_summary_by_key(
    Path(week_key): Path<String>,
    data: Data<&Arc<AppState>>,
) -> Result<Json<WeeklySummaryResponse>, Error> {
    let normalized = normalize_week_key(&week_key).map_err(|_| {
        Error::from_string(
            "week_key 형식이 올바르지 않습니다. 예: 2026-W10",
            StatusCode::BAD_REQUEST,
        )
    })?;
    build_weekly_summary(&data, &normalized).await
}

async fn build_weekly_summary(
    data: &Data<&Arc<AppState>>,
    week_key: &str,
) -> Result<Json<WeeklySummaryResponse>, Error> {
    let budget = query_as::<_, ActiveBudget>(
        "SELECT weekly_limit, alert_threshold
         FROM budget_config
         WHERE week_key = ?
         LIMIT 1",
    )
    .bind(week_key)
    .fetch_optional(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("예산 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let budget = budget.ok_or_else(|| {
        Error::from_string(
            "해당 주차에 적용 중인 예산이 없습니다.",
            StatusCode::NOT_FOUND,
        )
    })?;

    let aggregate = query_as::<_, WeeklyAggregate>(
        "SELECT COALESCE(SUM(amount), 0) AS total_spent,
                COUNT(record_id) AS record_count
         FROM spending_records
         WHERE week_key = ?",
    )
    .bind(week_key)
    .fetch_one(&data.db)
    .await
    .map_err(|e| {
        Error::from_string(
            format!("주간 합계 조회 실패: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let remaining = budget.weekly_limit - aggregate.total_spent;
    let usage_rate_raw = if budget.weekly_limit > 0 {
        aggregate.total_spent as f64 / budget.weekly_limit as f64
    } else {
        0.0
    };
    let usage_rate = (usage_rate_raw * 1000.0).round() / 1000.0;

    Ok(Json(WeeklySummaryResponse {
        week_key: week_key.to_string(),
        weekly_limit: budget.weekly_limit,
        total_spent: aggregate.total_spent,
        remaining,
        usage_rate,
        alert: usage_rate_raw >= budget.alert_threshold,
        record_count: aggregate.record_count,
    }))
}
