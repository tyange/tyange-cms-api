use std::sync::Arc;

use chrono::{Duration, Local, NaiveDate};
use poem::{
    handler,
    http::StatusCode,
    web::{Data as PoemData, Json, Multipart},
    Error,
};

use crate::card_excel::analyze_excel_bytes;
use crate::models::{
    AppState, CustomResponse, RemainingWeeklyBudgetBucket, RemainingWeeklyBudgetResponse,
};

#[handler]
pub async fn calculate_remaining_weekly_budget(
    mut multipart: Multipart,
    _data: PoemData<&Arc<AppState>>,
) -> Result<Json<CustomResponse<RemainingWeeklyBudgetResponse>>, Error> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut total_budget: Option<i64> = None;
    let mut from_date: Option<NaiveDate> = None;
    let mut to_date: Option<NaiveDate> = None;
    let mut as_of_date: Option<NaiveDate> = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().map(|v| v.to_string());
        if let Some(upload_name) = field.file_name().map(|v| v.to_string()) {
            let bytes = field.bytes().await.map_err(|e| {
                Error::from_string(
                    format!("엑셀 파일 읽기 실패: {}", e),
                    StatusCode::BAD_REQUEST,
                )
            })?;
            file_bytes = Some(bytes.to_vec());
            file_name = Some(upload_name);
            continue;
        }

        let Some(field_name) = name else {
            continue;
        };

        let value = field.text().await.map_err(|e| {
            Error::from_string(
                format!("요청 파라미터 읽기 실패: {}", e),
                StatusCode::BAD_REQUEST,
            )
        })?;

        match field_name.as_str() {
            "total_budget" => {
                total_budget = Some(parse_i64_strict(&value, "total_budget")?);
            }
            "from_date" => {
                from_date = Some(parse_naive_date(&value, "from_date")?);
            }
            "to_date" => {
                to_date = Some(parse_naive_date(&value, "to_date")?);
            }
            "as_of_date" => {
                as_of_date = Some(parse_naive_date(&value, "as_of_date")?);
            }
            _ => {}
        }
    }

    let total_budget = total_budget
        .ok_or_else(|| Error::from_string("total_budget가 필요합니다.", StatusCode::BAD_REQUEST))?;
    let from_date = from_date
        .ok_or_else(|| Error::from_string("from_date가 필요합니다.", StatusCode::BAD_REQUEST))?;
    let to_date = to_date
        .ok_or_else(|| Error::from_string("to_date가 필요합니다.", StatusCode::BAD_REQUEST))?;
    let as_of_date = as_of_date.unwrap_or_else(|| Local::now().date_naive());

    if total_budget <= 0 {
        return Err(Error::from_string(
            "total_budget는 0보다 커야 합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

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

    let file_bytes = file_bytes.ok_or_else(|| {
        Error::from_string("업로드할 엑셀 파일이 없습니다.", StatusCode::BAD_REQUEST)
    })?;

    let candidate = analyze_excel_bytes(&file_bytes, file_name.as_deref()).map_err(|e| {
        Error::from_string(format!("엑셀 분석 실패: {}", e), StatusCode::BAD_REQUEST)
    })?;

    let spent_net = candidate
        .rows
        .iter()
        .filter(|row| {
            let date = row.transacted_at.date();
            date >= from_date && date <= as_of_date
        })
        .map(|row| row.amount)
        .sum::<i64>();
    let remaining_budget = total_budget - spent_net;

    let buckets = allocate_remaining_buckets(from_date, to_date, as_of_date, remaining_budget)?;
    let remaining_days = buckets.iter().map(|item| item.days).sum::<u32>();

    let response = RemainingWeeklyBudgetResponse {
        total_budget,
        period_start: from_date.format("%Y-%m-%d").to_string(),
        period_end: to_date.format("%Y-%m-%d").to_string(),
        as_of_date: as_of_date.format("%Y-%m-%d").to_string(),
        spent_net,
        remaining_budget,
        remaining_days,
        is_overspent: remaining_budget < 0,
        buckets,
    };

    Ok(Json(CustomResponse {
        status: true,
        data: Some(response),
        message: Some("순지출 기준 남은 주간 예산을 계산했습니다.".to_string()),
    }))
}

fn parse_i64_strict(value: &str, field_name: &str) -> Result<i64, Error> {
    value.trim().parse::<i64>().map_err(|_| {
        Error::from_string(
            format!("{field_name} 형식이 올바르지 않습니다. 예: 2400000"),
            StatusCode::BAD_REQUEST,
        )
    })
}

fn parse_naive_date(value: &str, field_name: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").map_err(|_| {
        Error::from_string(
            format!("{field_name} 형식이 올바르지 않습니다. 예: 2026-03-05"),
            StatusCode::BAD_REQUEST,
        )
    })
}

fn allocate_remaining_buckets(
    from_date: NaiveDate,
    to_date: NaiveDate,
    as_of_date: NaiveDate,
    remaining_budget: i64,
) -> Result<Vec<RemainingWeeklyBudgetBucket>, Error> {
    let mut windows = Vec::<(NaiveDate, NaiveDate, u32)>::new();
    let mut cursor = from_date;

    while cursor <= to_date {
        let week_end = (cursor + Duration::days(6)).min(to_date);
        if week_end >= as_of_date {
            let start = if cursor < as_of_date {
                as_of_date
            } else {
                cursor
            };
            let days = (week_end - start).num_days() as u32 + 1;
            windows.push((start, week_end, days));
        }
        cursor += Duration::days(7);
    }

    if windows.is_empty() {
        return Ok(vec![]);
    }

    if remaining_budget <= 0 {
        return Ok(windows
            .iter()
            .enumerate()
            .map(|(i, (start, end, days))| RemainingWeeklyBudgetBucket {
                bucket_index: i as u32 + 1,
                from_date: start.format("%Y-%m-%d").to_string(),
                to_date: end.format("%Y-%m-%d").to_string(),
                days: *days,
                amount: 0,
            })
            .collect::<Vec<_>>());
    }

    let total_days = windows.iter().map(|(_, _, days)| *days).sum::<u32>();
    if total_days == 0 {
        return Err(Error::from_string(
            "남은 일수가 0입니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let mut temp = windows
        .iter()
        .map(|(start, end, days)| {
            let exact = remaining_budget as f64 * (*days as f64) / (total_days as f64);
            let base = exact.floor() as i64;
            (*start, *end, *days, base, exact - base as f64)
        })
        .collect::<Vec<(NaiveDate, NaiveDate, u32, i64, f64)>>();

    let allocated_base = temp.iter().map(|(_, _, _, base, _)| *base).sum::<i64>();
    let mut remainder = remaining_budget - allocated_base;

    let mut order = (0..temp.len()).collect::<Vec<usize>>();
    order.sort_by(|a, b| temp[*b].4.total_cmp(&temp[*a].4));
    for idx in order {
        if remainder <= 0 {
            break;
        }
        temp[idx].3 += 1;
        remainder -= 1;
    }

    Ok(temp
        .into_iter()
        .enumerate()
        .map(
            |(i, (start, end, days, amount, _))| RemainingWeeklyBudgetBucket {
                bucket_index: i as u32 + 1,
                from_date: start.format("%Y-%m-%d").to_string(),
                to_date: end.format("%Y-%m-%d").to_string(),
                days,
                amount,
            },
        )
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::card_excel::{parse_amount_from_string, parse_excel_date_string};

    use super::allocate_remaining_buckets;

    #[test]
    fn parse_date_formats() {
        assert_eq!(
            parse_excel_date_string("2026-03-03").unwrap().to_string(),
            "2026-03-03"
        );
        assert_eq!(
            parse_excel_date_string("2026.03.03 12:20:00")
                .unwrap()
                .to_string(),
            "2026-03-03"
        );
        assert_eq!(
            parse_excel_date_string("20260303").unwrap().to_string(),
            "2026-03-03"
        );
    }

    #[test]
    fn parse_amount_formats() {
        assert_eq!(parse_amount_from_string("12,300원"), Some(12300));
        assert_eq!(parse_amount_from_string("(3,000)"), Some(-3000));
        assert_eq!(parse_amount_from_string("-1500"), Some(-1500));
    }

    #[test]
    fn allocate_remaining_budget_by_days() {
        let buckets = allocate_remaining_buckets(
            NaiveDate::from_ymd_opt(2026, 2, 22).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 21).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 5).unwrap(),
            1_158_650,
        )
        .unwrap();

        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].days, 3);
        assert_eq!(buckets[1].days, 7);
        assert_eq!(buckets[2].days, 7);
        assert_eq!(buckets[0].amount, 204_468);
        assert_eq!(buckets[1].amount, 477_091);
        assert_eq!(buckets[2].amount, 477_091);
    }
}
