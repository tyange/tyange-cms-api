use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Pool, Sqlite};
use std::collections::HashMap;

pub struct AppState {
    pub db: Pool<Sqlite>,
}

#[derive(Debug, Serialize)]
pub struct CustomResponse<T> {
    pub status: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Post {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<TagWithCategory>,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct PostItem {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<TagWithCategory>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct UploadPostResponse {
    pub post_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Tag {
    pub tag: String,
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadPostRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<Tag>,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MeResponse {
    pub user_id: String,
    pub user_role: String,
}

#[derive(Debug, FromRow)]
pub struct PostResponseDb {
    pub post_id: String,
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub content: String,
    pub status: String,
    pub tags: String,
}

#[derive(Debug, Serialize)]
pub struct PostsResponse {
    pub posts: Vec<PostItem>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<Tag>,
    pub content: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct DeletePostResponse {
    pub post_id: String,
}

#[derive(Debug, Serialize)]
pub struct UploadImageResponse {
    pub image_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadImageQueryParmas {
    pub post_id: Option<String>,
    pub image_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddUserRequest {
    pub user_id: String,
    pub password: String,
    pub user_role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct Portfolio {
    pub portfolio_id: i32,
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PortfolioResponse {
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePortfolioRequest {
    pub content: String,
}

#[derive(Deserialize)]
pub struct SearchPostsWithTag {
    pub include: Option<String>,
    pub exclude: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchParamsWithTags {
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CountWithTag {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct TagsWithCategory {
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TagWithCategory {
    pub tag: String,
    pub category: String,
}

#[derive(Deserialize)]
pub struct SearchPostsWithWriter {
    pub writer_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSpendingRequest {
    pub amount: i64,
    pub merchant: Option<String>,
    pub transacted_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSpendingRequest {
    pub amount: i64,
    pub merchant: Option<String>,
    pub transacted_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSpendingResponse {
    pub record_id: i64,
    pub budget_id: i64,
    pub period_total_spent: i64,
    pub total_budget: i64,
    pub remaining: i64,
    pub alert: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SpendingRecordResponse {
    pub record_id: i64,
    pub amount: i64,
    pub merchant: Option<String>,
    pub transacted_at: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct SpendingWeekGroup {
    pub week_key: String,
    pub weekly_total: i64,
    pub record_count: i64,
    pub records: Vec<SpendingRecordResponse>,
}

#[derive(Debug, Serialize)]
pub struct SpendingListResponse {
    pub budget_id: i64,
    pub from_date: String,
    pub to_date: String,
    pub total_spent: i64,
    pub remaining: i64,
    pub weeks: Vec<SpendingWeekGroup>,
}

#[derive(Debug, Serialize)]
pub struct SpendingImportPreviewSummary {
    pub parsed_count: i64,
    pub in_period_count: i64,
    pub duplicate_count: i64,
    pub new_count: i64,
    pub out_of_period_count: i64,
    pub invalid_count: i64,
    pub new_amount_sum: i64,
    pub new_net_amount_sum: i64,
}

#[derive(Debug, Serialize)]
pub struct SpendingImportRow {
    pub fingerprint: String,
    pub transacted_at: Option<String>,
    pub amount: Option<i64>,
    pub merchant: Option<String>,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SpendingImportPreviewResponse {
    pub detected_source: String,
    pub file_name: String,
    pub summary: SpendingImportPreviewSummary,
    pub rows: Vec<SpendingImportRow>,
}

#[derive(Debug, Serialize)]
pub struct SpendingImportCommitResponse {
    pub detected_source: String,
    pub file_name: String,
    pub inserted_count: i64,
    pub skipped_duplicate_count: i64,
    pub skipped_out_of_period_count: i64,
    pub skipped_invalid_count: i64,
    pub inserted_amount_sum: i64,
    pub inserted_net_amount_sum: i64,
    pub period_total_spent_from_records: i64,
    pub remaining: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BudgetSummaryResponse {
    pub budget_id: i64,
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub total_spent: i64,
    pub remaining_budget: i64,
    pub usage_rate: f64,
    pub alert: bool,
    pub alert_threshold: f64,
    pub is_overspent: bool,
}

#[derive(Debug, Deserialize)]
pub struct BudgetPlanRequest {
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub alert_threshold: Option<f64>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct BudgetPlanResponse {
    pub budget_id: i64,
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub daily_budget: f64,
    pub total_spent: i64,
    pub remaining_budget: i64,
    pub usage_rate: f64,
    pub alert: bool,
    pub alert_threshold: f64,
    pub is_overspent: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActiveBudgetRequest {
    pub total_budget: i64,
    pub alert_threshold: Option<f64>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdateActiveBudgetResponse {
    pub budget_id: i64,
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub total_spent: i64,
    pub remaining_budget: i64,
    pub usage_rate: f64,
    pub alert: bool,
    pub alert_threshold: f64,
    pub is_overspent: bool,
}

#[derive(Debug, Serialize)]
pub struct RemainingWeeklyBudgetBucket {
    pub bucket_index: u32,
    pub from_date: String,
    pub to_date: String,
    pub days: u32,
    pub amount: i64,
}

#[derive(Debug, Serialize)]
pub struct RemainingWeeklyBudgetResponse {
    pub total_budget: i64,
    pub period_start: String,
    pub period_end: String,
    pub as_of_date: String,
    pub spent_net: i64,
    pub remaining_budget: i64,
    pub remaining_days: u32,
    pub is_overspent: bool,
    pub buckets: Vec<RemainingWeeklyBudgetBucket>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub api_key: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyListResponse {
    pub api_keys: Vec<ApiKeyResponse>,
}
