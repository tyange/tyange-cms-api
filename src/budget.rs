use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use poem::{http::StatusCode, Error};

pub fn iso_week_key_from_date(date: NaiveDate) -> String {
    let iso = date.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

pub fn collect_iso_week_days(
    from_date: NaiveDate,
    to_date: NaiveDate,
    start_date: NaiveDate,
) -> Result<Vec<(String, u32)>, Error> {
    if start_date > to_date {
        return Ok(vec![]);
    }

    let mut days_by_week = BTreeMap::<String, u32>::new();
    let mut cursor = from_date.max(start_date);

    loop {
        let week_key = iso_week_key_from_date(cursor);
        *days_by_week.entry(week_key).or_insert(0) += 1;

        if cursor == to_date {
            break;
        }

        cursor = cursor.succ_opt().ok_or_else(|| {
            Error::from_string(
                "날짜 계산 중 오류가 발생했습니다.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
    }

    Ok(days_by_week.into_iter().collect())
}

pub fn allocate_amounts_by_days(
    day_counts: &[u32],
    total_budget: i64,
    clamp_negative_to_zero: bool,
) -> Vec<i64> {
    if day_counts.is_empty() {
        return vec![];
    }

    let allocatable_budget = if clamp_negative_to_zero && total_budget < 0 {
        0
    } else {
        total_budget
    };

    if allocatable_budget <= 0 {
        return vec![0; day_counts.len()];
    }

    let total_days = day_counts.iter().sum::<u32>();
    if total_days == 0 {
        return vec![0; day_counts.len()];
    }

    let mut items = day_counts
        .iter()
        .map(|days| {
            let exact = allocatable_budget as f64 * (*days as f64) / total_days as f64;
            let base = exact.floor() as i64;
            (base, exact - base as f64)
        })
        .collect::<Vec<(i64, f64)>>();

    let allocated_base_sum = items.iter().map(|(base, _)| *base).sum::<i64>();
    let mut remainder = allocatable_budget - allocated_base_sum;

    let mut order = (0..items.len()).collect::<Vec<usize>>();
    order.sort_by(|a, b| items[*b].1.total_cmp(&items[*a].1).then_with(|| a.cmp(b)));

    for idx in order {
        if remainder <= 0 {
            break;
        }
        items[idx].0 += 1;
        remainder -= 1;
    }

    items.into_iter().map(|(amount, _)| amount).collect()
}

pub fn allocate_signed_amounts_by_days(day_counts: &[u32], total_budget: i64) -> Vec<i64> {
    if total_budget == 0 {
        return vec![0; day_counts.len()];
    }

    let allocated = allocate_amounts_by_days(day_counts, total_budget.saturating_abs(), false);
    if total_budget < 0 {
        allocated.into_iter().map(|amount| -amount).collect()
    } else {
        allocated
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{allocate_amounts_by_days, allocate_signed_amounts_by_days, collect_iso_week_days};

    #[test]
    fn allocates_remainder_by_fractional_days() {
        let allocated = allocate_amounts_by_days(&[5, 7, 7, 2], 2_050_000, false);

        assert_eq!(allocated, vec![488_095, 683_334, 683_333, 195_238]);
    }

    #[test]
    fn clamps_negative_budget_to_zero_when_requested() {
        let allocated = allocate_amounts_by_days(&[3, 7], -50_000, true);

        assert_eq!(allocated, vec![0, 0]);
    }

    #[test]
    fn allocates_negative_amounts_by_days() {
        let allocated = allocate_signed_amounts_by_days(&[3, 7], -50_000);

        assert_eq!(allocated, vec![-15_000, -35_000]);
    }

    #[test]
    fn groups_days_by_iso_week() {
        let days = collect_iso_week_days(
            NaiveDate::from_ymd_opt(2026, 3, 22).unwrap(),
            NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        )
        .unwrap();

        assert_eq!(
            days,
            vec![
                ("2026-W14".to_string(), 5),
                ("2026-W15".to_string(), 7),
                ("2026-W16".to_string(), 7),
                ("2026-W17".to_string(), 2),
            ]
        );
    }
}
