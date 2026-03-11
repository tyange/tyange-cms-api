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
