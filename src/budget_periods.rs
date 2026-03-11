use chrono::{NaiveDate, NaiveDateTime};
use sqlx::{query_as, query_scalar, FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub struct BudgetPeriodRow {
    pub budget_id: i64,
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub alert_threshold: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetSummaryMetrics {
    pub remaining_budget: i64,
    pub usage_rate: f64,
    pub alert: bool,
    pub is_overspent: bool,
}

pub async fn get_active_budget_period(
    pool: &SqlitePool,
    owner_user_id: &str,
) -> Result<Option<BudgetPeriodRow>, sqlx::Error> {
    query_as::<_, BudgetPeriodRow>(
        "SELECT budget_id, total_budget, from_date, to_date, alert_threshold
         FROM budget_periods
         WHERE owner_user_id = ?
         ORDER BY updated_at DESC, budget_id DESC
         LIMIT 1",
    )
    .bind(owner_user_id)
    .fetch_optional(pool)
    .await
}

pub fn compute_budget_summary(
    total_budget: i64,
    total_spent: i64,
    alert_threshold: f64,
) -> BudgetSummaryMetrics {
    let remaining_budget = total_budget - total_spent;
    let usage_rate_raw = if total_budget > 0 {
        total_spent as f64 / total_budget as f64
    } else {
        0.0
    };

    BudgetSummaryMetrics {
        remaining_budget,
        usage_rate: (usage_rate_raw * 1000.0).round() / 1000.0,
        alert: usage_rate_raw >= alert_threshold,
        is_overspent: total_spent > total_budget,
    }
}

pub async fn sum_spending_for_period(
    pool: &SqlitePool,
    owner_user_id: &str,
    from_date: &str,
    to_date: &str,
) -> Result<i64, sqlx::Error> {
    query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(amount), 0)
         FROM spending_records
         WHERE owner_user_id = ?
           AND date(transacted_at) >= date(?)
           AND date(transacted_at) <= date(?)",
    )
    .bind(owner_user_id)
    .bind(from_date)
    .bind(to_date)
    .fetch_one(pool)
    .await
}

pub fn format_naive_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn date_in_period(transacted_at: &NaiveDateTime, from_date: &str, to_date: &str) -> bool {
    let date = transacted_at.date();
    let from = NaiveDate::parse_from_str(from_date, "%Y-%m-%d");
    let to = NaiveDate::parse_from_str(to_date, "%Y-%m-%d");

    matches!((from, to), (Ok(from), Ok(to)) if date >= from && date <= to)
}
