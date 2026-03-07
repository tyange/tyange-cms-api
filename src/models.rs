use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};

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

#[derive(Deserialize)]
pub struct WeeklyConfigRequest {
    pub weekly_limit: u32,
    pub alert_threshold: f64,
}
#[derive(FromRow, Serialize)]
pub struct WeeklyConfigResponse {
    pub config_id: u32,
    pub week_key: String,
    pub weekly_limit: u32,
    pub alert_threshold: f64,
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
    pub weekly_total: i64,
    pub weekly_limit: i64,
    pub remaining: i64,
    pub alert: bool,
}

#[derive(Debug, Deserialize)]
pub struct SpendingQueryParams {
    pub week: Option<String>,
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
pub struct SpendingListResponse {
    pub week_key: String,
    pub records: Vec<SpendingRecordResponse>,
}

#[derive(Debug, Serialize)]
pub struct WeeklySummaryResponse {
    pub week_key: String,
    pub weekly_limit: i64,
    pub total_spent: i64,
    pub remaining: i64,
    pub usage_rate: f64,
    pub alert: bool,
    pub record_count: i64,
}

#[derive(Debug, Serialize)]
pub struct BudgetWeeksResponse {
    pub weeks: Vec<String>,
    pub min_week: Option<String>,
    pub max_week: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BudgetPlanRequest {
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub alert_threshold: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct BudgetPlanWeekItem {
    pub week_key: String,
    pub days: u32,
    pub weekly_limit: i64,
}

#[derive(Debug, Serialize)]
pub struct BudgetPlanResponse {
    pub total_budget: i64,
    pub from_date: String,
    pub to_date: String,
    pub daily_budget: f64,
    pub weeks: Vec<BudgetPlanWeekItem>,
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
