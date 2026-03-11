use std::{collections::BTreeSet, sync::Arc};

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Multipart},
    Error, Request,
};
use sqlx::{query, query_scalar};

use crate::{
    budget_periods::{date_in_period, get_active_budget_period, sum_spending_for_period},
    card_excel::{analyze_excel_bytes, ParsedCardTransactionRow},
    models::{
        AppState, CustomResponse, SpendingImportCommitResponse, SpendingImportPreviewResponse,
        SpendingImportPreviewSummary, SpendingImportRow,
    },
};
use tyange_cms_api::auth::authorization::current_user;

const SHINHAN_CARD_SOURCE: &str = "shinhancard_xls";

#[handler]
pub async fn preview_spending_import(
    req: &Request,
    mut multipart: Multipart,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<SpendingImportPreviewResponse>>, Error> {
    let user = current_user(req)?;
    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(internal_error("예산 조회 실패"))?
        .ok_or_else(|| {
            Error::from_string("현재 활성 기간 예산이 없습니다.", StatusCode::NOT_FOUND)
        })?;

    let upload = read_import_upload(&mut multipart).await?;
    let parsed = analyze_excel_bytes(&upload.file_bytes, Some(&upload.file_name))
        .map_err(|e| Error::from_string(format!("엑셀 분석 실패: {e}"), StatusCode::BAD_REQUEST))?;
    let imported_fingerprints = load_imported_fingerprints(&data.db, &user.user_id).await?;

    let preview = build_preview_rows(
        parsed.rows,
        &budget.from_date,
        &budget.to_date,
        &imported_fingerprints,
    );

    Ok(Json(CustomResponse {
        status: true,
        data: Some(SpendingImportPreviewResponse {
            detected_source: SHINHAN_CARD_SOURCE.to_string(),
            file_name: upload.file_name,
            summary: preview.summary,
            rows: preview.rows,
        }),
        message: Some("엑셀 소비내역 미리보기를 생성했습니다.".to_string()),
    }))
}

#[handler]
pub async fn commit_spending_import(
    req: &Request,
    mut multipart: Multipart,
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<SpendingImportCommitResponse>>, Error> {
    let user = current_user(req)?;
    let budget = get_active_budget_period(&data.db, &user.user_id)
        .await
        .map_err(internal_error("예산 조회 실패"))?
        .ok_or_else(|| {
            Error::from_string("현재 활성 기간 예산이 없습니다.", StatusCode::NOT_FOUND)
        })?;

    let upload = read_import_upload(&mut multipart).await?;
    if upload.selected_fingerprints.is_empty() {
        return Err(Error::from_string(
            "selected_fingerprints가 필요합니다.",
            StatusCode::BAD_REQUEST,
        ));
    }

    let parsed = analyze_excel_bytes(&upload.file_bytes, Some(&upload.file_name))
        .map_err(|e| Error::from_string(format!("엑셀 분석 실패: {e}"), StatusCode::BAD_REQUEST))?;
    let imported_fingerprints = load_imported_fingerprints(&data.db, &user.user_id).await?;
    let preview = build_preview_rows(
        parsed.rows,
        &budget.from_date,
        &budget.to_date,
        &imported_fingerprints,
    );
    let selected = upload
        .selected_fingerprints
        .into_iter()
        .collect::<BTreeSet<_>>();
    let selectable = preview
        .rows
        .iter()
        .filter(|row| row.status == "new")
        .map(|row| row.fingerprint.clone())
        .collect::<BTreeSet<_>>();

    let unknown = selected
        .iter()
        .filter(|fingerprint| !selectable.contains(*fingerprint))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(Error::from_string(
            format!(
                "선택한 fingerprint를 반영할 수 없습니다: {}",
                unknown.join(", ")
            ),
            StatusCode::BAD_REQUEST,
        ));
    }

    let mut inserted_count = 0_i64;
    let mut skipped_duplicate_count = 0_i64;
    let mut skipped_out_of_period_count = preview.summary.out_of_period_count;
    let mut skipped_invalid_count = preview.summary.invalid_count;
    let mut inserted_amount_sum = 0_i64;
    let mut inserted_net_amount_sum = 0_i64;

    for row in preview
        .rows
        .iter()
        .filter(|row| selected.contains(&row.fingerprint))
    {
        let transacted_at = row.transacted_at.clone().ok_or_else(|| {
            Error::from_string(
                "유효하지 않은 거래일시는 반영할 수 없습니다.",
                StatusCode::BAD_REQUEST,
            )
        })?;
        let amount = row.amount.ok_or_else(|| {
            Error::from_string(
                "유효하지 않은 금액은 반영할 수 없습니다.",
                StatusCode::BAD_REQUEST,
            )
        })?;

        let inserted = query(
            "INSERT OR IGNORE INTO spending_records (
                 owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint
             )
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&user.user_id)
        .bind(amount)
        .bind(&row.merchant)
        .bind(transacted_at.replace('T', " "))
        .bind(SHINHAN_CARD_SOURCE)
        .bind(&row.fingerprint)
        .execute(&data.db)
        .await
        .map_err(internal_error("소비 기록 저장 실패"))?;

        if inserted.rows_affected() == 0 {
            skipped_duplicate_count += 1;
            continue;
        }

        inserted_count += 1;
        inserted_amount_sum += amount.abs();
        inserted_net_amount_sum += amount;
    }

    let period_total_spent_from_records =
        sum_spending_for_period(&data.db, &user.user_id, &budget.from_date, &budget.to_date)
            .await
            .map_err(internal_error("기간 소비 합계 조회 실패"))?;

    Ok(Json(CustomResponse {
        status: true,
        data: Some(SpendingImportCommitResponse {
            detected_source: SHINHAN_CARD_SOURCE.to_string(),
            file_name: upload.file_name,
            inserted_count,
            skipped_duplicate_count,
            skipped_out_of_period_count,
            skipped_invalid_count,
            inserted_amount_sum,
            inserted_net_amount_sum,
            period_total_spent_from_records,
            budget_snapshot_total_spent_unchanged: true,
        }),
        message: Some("선택한 엑셀 소비내역을 반영했습니다.".to_string()),
    }))
}

struct ImportUpload {
    file_bytes: Vec<u8>,
    file_name: String,
    selected_fingerprints: Vec<String>,
}

struct PreviewBuildResult {
    summary: SpendingImportPreviewSummary,
    rows: Vec<SpendingImportRow>,
}

async fn read_import_upload(multipart: &mut Multipart) -> Result<ImportUpload, Error> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut selected_fingerprints = Vec::new();

    while let Some(field) = multipart.next_field().await? {
        let field_name = field.name().map(|name| name.to_string());

        if let Some(upload_name) = field.file_name().map(|value| value.to_string()) {
            let bytes = field.bytes().await.map_err(|e| {
                Error::from_string(format!("엑셀 파일 읽기 실패: {e}"), StatusCode::BAD_REQUEST)
            })?;
            file_bytes = Some(bytes.to_vec());
            file_name = Some(upload_name);
            continue;
        }

        let Some(field_name) = field_name else {
            continue;
        };
        if field_name != "selected_fingerprints" {
            continue;
        }

        let value = field.text().await.map_err(|e| {
            Error::from_string(
                format!("요청 파라미터 읽기 실패: {e}"),
                StatusCode::BAD_REQUEST,
            )
        })?;
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            selected_fingerprints.push(trimmed.to_string());
        }
    }

    let file_bytes = file_bytes.ok_or_else(|| {
        Error::from_string("업로드할 엑셀 파일이 없습니다.", StatusCode::BAD_REQUEST)
    })?;
    let file_name = file_name.ok_or_else(|| {
        Error::from_string("파일명을 확인할 수 없습니다.", StatusCode::BAD_REQUEST)
    })?;

    Ok(ImportUpload {
        file_bytes,
        file_name,
        selected_fingerprints,
    })
}

fn build_preview_rows(
    rows: Vec<ParsedCardTransactionRow>,
    from_date: &str,
    to_date: &str,
    imported_fingerprints: &BTreeSet<String>,
) -> PreviewBuildResult {
    let mut parsed_count = 0_i64;
    let mut in_period_count = 0_i64;
    let mut duplicate_count = 0_i64;
    let mut new_count = 0_i64;
    let mut out_of_period_count = 0_i64;
    let invalid_count = 0_i64;
    let mut new_amount_sum = 0_i64;
    let mut new_net_amount_sum = 0_i64;
    let mut preview_rows = Vec::new();

    for row in rows {
        parsed_count += 1;

        let transacted_at = row.transacted_at.format("%Y-%m-%dT%H:%M:%S").to_string();
        let status;
        let reason;

        if !date_in_period(&row.transacted_at, from_date, to_date) {
            out_of_period_count += 1;
            status = "out_of_period";
            reason = Some("현재 활성 예산 기간 밖의 거래입니다.".to_string());
        } else if imported_fingerprints.contains(&row.fingerprint) {
            in_period_count += 1;
            duplicate_count += 1;
            status = "duplicate";
            reason = Some("이미 가져온 거래입니다.".to_string());
        } else {
            in_period_count += 1;
            new_count += 1;
            new_amount_sum += row.amount.abs();
            new_net_amount_sum += row.amount;
            status = "new";
            reason = None;
        }

        preview_rows.push(SpendingImportRow {
            fingerprint: row.fingerprint,
            transacted_at: Some(transacted_at),
            amount: Some(row.amount),
            merchant: row.merchant,
            status: status.to_string(),
            reason,
        });
    }

    PreviewBuildResult {
        summary: SpendingImportPreviewSummary {
            parsed_count,
            in_period_count,
            duplicate_count,
            new_count,
            out_of_period_count,
            invalid_count,
            new_amount_sum,
            new_net_amount_sum,
        },
        rows: preview_rows,
    }
}

async fn load_imported_fingerprints(
    db: &sqlx::SqlitePool,
    user_id: &str,
) -> Result<BTreeSet<String>, Error> {
    let fingerprints = query_scalar::<_, String>(
        "SELECT source_fingerprint
         FROM spending_records
         WHERE owner_user_id = ?
           AND source_type = ?
           AND source_fingerprint IS NOT NULL",
    )
    .bind(user_id)
    .bind(SHINHAN_CARD_SOURCE)
    .fetch_all(db)
    .await
    .map_err(internal_error("기존 가져오기 이력 조회 실패"))?;

    Ok(fingerprints.into_iter().collect::<BTreeSet<_>>())
}

fn internal_error(prefix: &'static str) -> impl FnOnce(sqlx::Error) -> Error {
    move |e| Error::from_string(format!("{prefix}: {e}"), StatusCode::INTERNAL_SERVER_ERROR)
}
