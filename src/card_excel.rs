use std::io::Cursor;

use calamine::{Data, Reader, Xls, Xlsx};
use chrono::{NaiveDate, NaiveDateTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCardTransactionRow {
    pub transacted_at: NaiveDateTime,
    pub amount: i64,
    pub merchant: Option<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCandidate {
    pub rows: Vec<ParsedCardTransactionRow>,
}

#[derive(Debug, Clone, Copy)]
struct HeaderMap {
    transacted_at_idx: usize,
    amount_idx: usize,
    merchant_idx: usize,
    card_idx: Option<usize>,
    approval_idx: Option<usize>,
    cancel_status_idx: Option<usize>,
}

pub fn analyze_excel_bytes(
    file_bytes: &[u8],
    file_name: Option<&str>,
) -> Result<SheetCandidate, String> {
    let is_xls = file_name
        .map(|name| name.to_ascii_lowercase().ends_with(".xls"))
        .unwrap_or(false);

    let first_try = if is_xls {
        parse_xls(file_bytes)
    } else {
        parse_xlsx(file_bytes)
    };

    match first_try {
        Ok(v) => Ok(v),
        Err(first_err) => {
            let second_try = if is_xls {
                parse_xlsx(file_bytes)
            } else {
                parse_xls(file_bytes)
            };
            second_try.or(Err(first_err))
        }
    }
}

fn parse_xlsx(file_bytes: &[u8]) -> Result<SheetCandidate, String> {
    let mut workbook: Xlsx<Cursor<Vec<u8>>> =
        Xlsx::new(Cursor::new(file_bytes.to_vec())).map_err(|e| e.to_string())?;
    extract_best_candidate(&mut workbook)
}

fn parse_xls(file_bytes: &[u8]) -> Result<SheetCandidate, String> {
    let mut workbook: Xls<Cursor<Vec<u8>>> =
        Xls::new(Cursor::new(file_bytes.to_vec())).map_err(|e| e.to_string())?;
    extract_best_candidate(&mut workbook)
}

fn extract_best_candidate<R>(workbook: &mut R) -> Result<SheetCandidate, String>
where
    R: Reader<Cursor<Vec<u8>>>,
{
    let mut best_candidate: Option<SheetCandidate> = None;

    for sheet_name in workbook.sheet_names().to_vec() {
        let Ok(range) = workbook.worksheet_range(&sheet_name) else {
            continue;
        };

        let rows = range
            .rows()
            .map(|row| row.iter().map(cell_to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();

        let has_header_map = find_header_map(&rows).is_some();
        let structured_rows = parse_structured_rows(&rows);
        let flat_rows = parse_flat_rows(&rows);

        let candidate_rows = if has_header_map {
            structured_rows
        } else if structured_rows.len() >= flat_rows.len() {
            structured_rows
        } else {
            flat_rows
        };

        if candidate_rows.is_empty() {
            continue;
        }

        let candidate = SheetCandidate {
            rows: dedup_rows(candidate_rows),
        };

        if let Some(current) = &best_candidate {
            if candidate.rows.len() > current.rows.len() {
                best_candidate = Some(candidate);
            }
        } else {
            best_candidate = Some(candidate);
        }
    }

    best_candidate.ok_or_else(|| {
        "거래일시/금액 컬럼을 자동 인식하지 못했습니다. 신한카드 거래내역 파일인지 확인해주세요."
            .to_string()
    })
}

fn parse_structured_rows(rows: &[Vec<String>]) -> Vec<ParsedCardTransactionRow> {
    let Some((header_row_idx, header_map)) = find_header_map(rows) else {
        return rows
            .iter()
            .filter_map(|row| parse_unstructured_row(row))
            .collect::<Vec<_>>();
    };

    rows.iter()
        .skip(header_row_idx + 1)
        .filter_map(|row| parse_header_mapped_row(row, header_map))
        .collect::<Vec<_>>()
}

fn parse_unstructured_row(row: &[String]) -> Option<ParsedCardTransactionRow> {
    let datetime_idx = row
        .iter()
        .position(|cell| parse_excel_datetime(cell).is_some())?;
    let amount_idx = row
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != datetime_idx)
        .filter_map(|(idx, cell)| parse_amount_from_string(cell).map(|amount| (idx, amount)))
        .last()?;

    let transacted_at = parse_excel_datetime(&row[datetime_idx])?;
    let merchant = extract_merchant(row, datetime_idx, amount_idx.0);
    let fingerprint = build_fingerprint(row);

    Some(ParsedCardTransactionRow {
        transacted_at,
        amount: amount_idx.1,
        merchant,
        fingerprint,
    })
}

fn find_header_map(rows: &[Vec<String>]) -> Option<(usize, HeaderMap)> {
    rows.iter()
        .enumerate()
        .find_map(|(idx, row)| parse_header_row(row).map(|header_map| (idx, header_map)))
}

fn parse_header_row(row: &[String]) -> Option<HeaderMap> {
    let mut transacted_at_idx = None;
    let mut amount_idx = None;
    let mut merchant_idx = None;
    let mut card_idx = None;
    let mut approval_idx = None;
    let mut cancel_status_idx = None;

    for (idx, cell) in row.iter().enumerate() {
        let normalized = normalize_header_name(cell);
        match normalized.as_str() {
            "거래일" => transacted_at_idx = Some(idx),
            "금액" => amount_idx = Some(idx),
            "가맹점명" => merchant_idx = Some(idx),
            "이용카드" => card_idx = Some(idx),
            "승인번호" => approval_idx = Some(idx),
            "취소상태" => cancel_status_idx = Some(idx),
            _ => {}
        }
    }

    Some(HeaderMap {
        transacted_at_idx: transacted_at_idx?,
        amount_idx: amount_idx?,
        merchant_idx: merchant_idx?,
        card_idx,
        approval_idx,
        cancel_status_idx,
    })
}

fn parse_header_mapped_row(
    row: &[String],
    header_map: HeaderMap,
) -> Option<ParsedCardTransactionRow> {
    let transacted_at = parse_excel_datetime(row.get(header_map.transacted_at_idx)?)?;
    let amount = parse_amount_from_string(row.get(header_map.amount_idx)?)?;
    let merchant = row
        .get(header_map.merchant_idx)
        .and_then(|value| normalize_merchant(value));

    let mut fingerprint_values = Vec::with_capacity(6);
    fingerprint_values.push(row[header_map.transacted_at_idx].clone());
    fingerprint_values.push(row[header_map.amount_idx].clone());
    if let Some(merchant) = row.get(header_map.merchant_idx) {
        fingerprint_values.push(merchant.clone());
    }
    if let Some(card_idx) = header_map.card_idx {
        if let Some(card) = row.get(card_idx) {
            fingerprint_values.push(card.clone());
        }
    }
    if let Some(approval_idx) = header_map.approval_idx {
        if let Some(approval) = row.get(approval_idx) {
            fingerprint_values.push(approval.clone());
        }
    }
    if let Some(cancel_status_idx) = header_map.cancel_status_idx {
        if let Some(cancel_status) = row.get(cancel_status_idx) {
            fingerprint_values.push(cancel_status.clone());
        }
    }

    Some(ParsedCardTransactionRow {
        transacted_at,
        amount,
        merchant,
        fingerprint: build_fingerprint(&fingerprint_values),
    })
}

fn parse_flat_rows(rows: &[Vec<String>]) -> Vec<ParsedCardTransactionRow> {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().filter(|cell| !cell.is_empty()).cloned())
        .collect::<Vec<_>>();

    let mut parsed = Vec::new();
    let mut idx = 0usize;
    while idx < cells.len() {
        let Some(transacted_at) = parse_excel_datetime(&cells[idx]) else {
            idx += 1;
            continue;
        };

        let amount_match = ((idx + 1)..usize::min(idx + 4, cells.len()))
            .filter_map(|amount_idx| {
                parse_amount_from_string(&cells[amount_idx]).map(|amount| (amount_idx, amount))
            })
            .last();

        let Some((amount_idx, amount)) = amount_match else {
            idx += 1;
            continue;
        };

        let raw_values = cells[idx..=amount_idx].to_vec();
        let merchant = raw_values
            .iter()
            .skip(1)
            .take(raw_values.len().saturating_sub(2))
            .find(|cell| looks_like_merchant(cell))
            .cloned();

        parsed.push(ParsedCardTransactionRow {
            transacted_at,
            amount,
            merchant,
            fingerprint: build_fingerprint(&raw_values),
        });

        idx = amount_idx + 1;
    }

    parsed
}

fn dedup_rows(rows: Vec<ParsedCardTransactionRow>) -> Vec<ParsedCardTransactionRow> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();

    for row in rows {
        if seen.insert(row.fingerprint.clone()) {
            deduped.push(row);
        }
    }

    deduped
}

fn extract_merchant(row: &[String], datetime_idx: usize, amount_idx: usize) -> Option<String> {
    row.iter()
        .enumerate()
        .filter(|(idx, _)| *idx != datetime_idx && *idx != amount_idx)
        .map(|(_, cell)| cell.trim())
        .find(|cell| looks_like_merchant(cell))
        .map(|cell| cell.to_string())
}

fn looks_like_merchant(value: &str) -> bool {
    normalize_merchant(value).is_some()
}

fn build_fingerprint(values: &[String]) -> String {
    values
        .iter()
        .map(|value| normalize_fingerprint_part(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

fn normalize_fingerprint_part(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        _ => cell.to_string().trim().to_string(),
    }
}

fn normalize_header_name(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn normalize_merchant(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || parse_excel_datetime(trimmed).is_some() {
        return None;
    }

    let normalized = trimmed
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_lowercase();

    if normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "신용" | "체크" | "일시불" | "승인" | "결제확정" | "승인취소" | "취소"
        )
    {
        return None;
    }

    if looks_like_card_label(&normalized) || is_numeric_like(trimmed) {
        return None;
    }

    Some(trimmed.to_string())
}

fn looks_like_card_label(value: &str) -> bool {
    let Some(stripped) = value.strip_prefix("본인") else {
        return false;
    };
    let Some(masked) = stripped.strip_suffix('*') else {
        return false;
    };

    !masked.is_empty() && masked.chars().all(|ch| ch.is_ascii_digit())
}

fn is_numeric_like(value: &str) -> bool {
    let filtered = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, ',' | '.' | '(' | ')' | '-' | '+'))
        .collect::<String>();

    !filtered.is_empty() && filtered.chars().all(|ch| ch.is_ascii_digit())
}

pub fn parse_excel_date_string(raw: &str) -> Option<NaiveDate> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    let date_part = s.split_whitespace().next().unwrap_or(s);
    let normalized = date_part.replace(['.', '/'], "-");

    if let Ok(date) = NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
        return Some(date);
    }

    if normalized.len() == 8 {
        if let Ok(date) = NaiveDate::parse_from_str(&normalized, "%Y%m%d") {
            return Some(date);
        }
    }

    None
}

pub fn parse_excel_datetime(raw: &str) -> Option<NaiveDateTime> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let normalized = value.replace(['.', '/'], "-");
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ];

    for format in formats {
        if let Ok(transacted_at) = NaiveDateTime::parse_from_str(&normalized, format) {
            return Some(transacted_at);
        }
    }

    parse_excel_date_string(value)?.and_hms_opt(0, 0, 0)
}

pub fn parse_amount_from_string(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_negative =
        (trimmed.starts_with('(') && trimmed.ends_with(')')) || trimmed.starts_with('-');

    let filtered = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == ',')
        .collect::<String>()
        .replace(',', "");

    if filtered.is_empty() {
        return None;
    }

    let parsed = if filtered.contains('.') {
        filtered.parse::<f64>().ok().map(|v| v.round() as i64)
    } else {
        filtered.parse::<i64>().ok()
    }?;

    Some(match is_negative {
        true => -parsed,
        false => parsed,
    })
}

#[cfg(test)]
mod tests {
    use super::{analyze_excel_bytes, parse_flat_rows, parse_structured_rows};

    #[test]
    fn parses_shinhancard_style_flat_rows() {
        let rows = vec![
            vec!["2026.03.05 09:45".to_string()],
            vec!["43782818".to_string()],
            vec!["000000000007000".to_string()],
            vec!["2026.03.02 19:09".to_string()],
            vec!["13103861".to_string()],
            vec!["-000000000019000".to_string()],
        ];

        let parsed = parse_flat_rows(&rows);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].amount, 7000);
        assert_eq!(parsed[1].amount, -19000);
        assert!(parsed[0].merchant.is_none());
    }

    #[test]
    fn parses_structured_rows_with_merchant() {
        let rows = vec![vec![
            "2026.03.02 07:10".to_string(),
            "Amazon_AWS".to_string(),
            "05526357".to_string(),
            "000000000006565".to_string(),
        ]];

        let parsed = parse_structured_rows(&rows);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].amount, 6565);
        assert_eq!(parsed[0].merchant.as_deref(), Some("Amazon_AWS"));
    }

    #[test]
    fn parses_shinhancard_header_rows_without_card_digits_becoming_amount() {
        let rows = vec![
            vec![
                "거래일".to_string(),
                "카드구분".to_string(),
                "이용카드".to_string(),
                "가맹점명".to_string(),
                "승인번호".to_string(),
                "금액".to_string(),
                "매입구분".to_string(),
                "이용구분".to_string(),
                "거래통화".to_string(),
                "해외이용금액".to_string(),
                "취소상태".to_string(),
            ],
            vec![
                "2026.03.03 12:20".to_string(),
                "신용".to_string(),
                "본인996*".to_string(),
                "씨유 역삼신웅점".to_string(),
                "19939500".to_string(),
                "8800".to_string(),
                "결제확정".to_string(),
                "일시불".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ],
            vec![
                "2026.03.02 19:09".to_string(),
                "신용".to_string(),
                "본인996*".to_string(),
                "㈜우아한형제들".to_string(),
                "13103861".to_string(),
                "-19000".to_string(),
                "승인취소".to_string(),
                "일시불".to_string(),
                "".to_string(),
                "".to_string(),
                "취소".to_string(),
            ],
        ];

        let parsed = parse_structured_rows(&rows);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].amount, 8800);
        assert_eq!(parsed[0].merchant.as_deref(), Some("씨유 역삼신웅점"));
        assert!(parsed[0].fingerprint.contains("본인996*"));
        assert!(!parsed[0].fingerprint.ends_with("|996"));
        assert_eq!(parsed[1].amount, -19000);
        assert_eq!(parsed[1].merchant.as_deref(), Some("㈜우아한형제들"));
    }

    #[test]
    fn parses_fixture_excel_with_header_based_amounts() {
        let fixture = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test_samples/shinhancard_sample.xls"
        ))
        .expect("expected shinhancard fixture");

        let parsed = analyze_excel_bytes(&fixture, Some("shinhancard_sample.xls"))
            .expect("expected fixture to parse");

        assert!(parsed.rows.iter().any(|row| {
            row.transacted_at.format("%Y-%m-%dT%H:%M:%S").to_string() == "2026-03-03T12:20:00"
                && row.amount == 8800
                && row.merchant.as_deref() == Some("씨유 역삼신웅점")
                && row.fingerprint.contains("본인996*")
        }));
        assert!(parsed.rows.iter().any(|row| {
            row.transacted_at.format("%Y-%m-%dT%H:%M:%S").to_string() == "2026-03-02T19:09:00"
                && row.amount == -19000
                && row.merchant.as_deref() == Some("㈜우아한형제들")
        }));
    }
}
