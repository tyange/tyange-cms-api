use poem::http::StatusCode;
use poem::Error;
use sqlx::{query, query_as, query_scalar, SqlitePool};

use crate::models::MatchSummaryResponse;

const ACTIVE_MATCH_STATUSES: [&str; 2] = ["pending", "matched"];

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct MatchRow {
    pub match_id: i64,
    pub requester_user_id: String,
    pub target_user_id: String,
    pub status: String,
    pub created_at: String,
    pub responded_at: Option<String>,
}

pub fn active_match_statuses() -> &'static [&'static str; 2] {
    &ACTIVE_MATCH_STATUSES
}

pub fn current_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub async fn ensure_user_exists(db: &SqlitePool, user_id: &str) -> Result<(), Error> {
    let exists: i64 = query_scalar("SELECT COUNT(*) FROM users WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(db)
        .await
        .map_err(|err| {
            Error::from_string(
                format!("사용자 조회 실패: {}", err),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

    if exists == 0 {
        return Err(Error::from_string(
            "대상 사용자를 찾을 수 없습니다.",
            StatusCode::NOT_FOUND,
        ));
    }

    Ok(())
}

pub async fn find_active_match_for_user(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Option<MatchRow>, Error> {
    query_as::<_, MatchRow>(
        r#"
        SELECT
            match_id,
            requester_user_id,
            target_user_id,
            status,
            created_at,
            responded_at
        FROM user_matches
        WHERE (requester_user_id = ? OR target_user_id = ?)
          AND status IN (?, ?)
        ORDER BY created_at DESC, match_id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(active_match_statuses()[0])
    .bind(active_match_statuses()[1])
    .fetch_optional(db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("매칭 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })
}

pub async fn find_pending_match_by_id(
    db: &SqlitePool,
    match_id: i64,
) -> Result<Option<MatchRow>, Error> {
    query_as::<_, MatchRow>(
        r#"
        SELECT
            match_id,
            requester_user_id,
            target_user_id,
            status,
            created_at,
            responded_at
        FROM user_matches
        WHERE match_id = ?
          AND status = 'pending'
        LIMIT 1
        "#,
    )
    .bind(match_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("매칭 조회 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })
}

pub fn to_match_summary(row: MatchRow, current_user_id: &str) -> MatchSummaryResponse {
    let counterpart_user_id = if row.requester_user_id == current_user_id {
        row.target_user_id.clone()
    } else {
        row.requester_user_id.clone()
    };

    MatchSummaryResponse {
        match_id: row.match_id,
        status: row.status,
        requester_user_id: row.requester_user_id,
        target_user_id: row.target_user_id,
        counterpart_user_id,
        created_at: row.created_at,
        responded_at: row.responded_at,
    }
}

pub async fn close_active_match(
    db: &SqlitePool,
    user_id: &str,
    next_status: &str,
) -> Result<Option<MatchSummaryResponse>, Error> {
    let existing = find_active_match_for_user(db, user_id).await?;
    let Some(row) = existing else {
        return Ok(None);
    };

    let now = current_timestamp();
    query(
        r#"
        UPDATE user_matches
        SET status = ?, closed_at = ?
        WHERE match_id = ?
        "#,
    )
    .bind(next_status)
    .bind(&now)
    .bind(row.match_id)
    .execute(db)
    .await
    .map_err(|err| {
        Error::from_string(
            format!("매칭 종료 실패: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Some(to_match_summary(
        MatchRow {
            status: next_status.to_string(),
            ..row
        },
        user_id,
    )))
}
