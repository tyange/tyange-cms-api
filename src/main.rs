#[cfg(test)]
mod budget;
mod budget_periods;
mod card_excel;
mod db;
mod middlewares;
mod models;
mod routes;
mod utils;

use dotenv::dotenv;
use middlewares::admin_middleware::AdminOnly;
use middlewares::api_key_middleware::JwtOrApiKeyAuth;
use middlewares::auth_middleware::Auth;
use std::{env, fs, sync::Arc};

use crate::routes::analyze_card_excel::calculate_remaining_weekly_budget;
use crate::routes::create_api_key::create_api_key_handler;
use crate::routes::create_budget_plan::create_budget_plan;
use crate::routes::create_spending::create_spending;
use crate::routes::delete_api_key::delete_api_key;
use crate::routes::delete_post::delete_post;
use crate::routes::delete_spending::delete_spending;
use crate::routes::get_all_posts::get_all_posts;
use crate::routes::get_api_keys::get_api_keys;
use crate::routes::get_budget::get_budget;
use crate::routes::get_count_with_tags::get_count_with_tags;
use crate::routes::get_portfolio::get_portfolio;
use crate::routes::get_posts_with_tags::get_posts_with_tags;
use crate::routes::get_spending::get_spending;
use crate::routes::get_tags_with_category::get_tags_with_category;
use crate::routes::import_spending_excel::{commit_spending_import, preview_spending_import};
use crate::routes::me::me;
use crate::routes::signup::signup;
use crate::routes::update_active_budget::update_active_budget;
use crate::routes::update_portfolio::update_portfolio;
use crate::routes::update_post::update_post;
use crate::routes::update_spending::update_spending;
use crate::routes::upload_image::upload_image;
use crate::{models::AppState, routes::add_user::add_user};
use db::init_db;
use poem::{
    delete, endpoint::StaticFilesEndpoint, get, handler, http::StatusCode, listener::TcpListener,
    middleware::Cors, options, post, put, EndpointExt, Response, Route, Server,
};
use routes::{get_post::get_post, get_posts::get_posts, login::login, upload_post::upload_post};
use sqlx::SqlitePool;

#[handler]
fn return_str() -> &'static str {
    "hello"
}

#[handler]
async fn options_handler() -> Response {
    Response::builder().status(StatusCode::OK).finish()
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/database.db".to_string());
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        fs::create_dir_all(parent)?;
    }

    let db_url = format!("sqlite:{}?mode=rwc", db_path);
    println!("Database URL: {}", db_url);

    let db = SqlitePool::connect(&db_url).await.map_err(|e| {
        eprintln!("Connection with Database: {:?}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e)
    })?;

    init_db(&db)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let state = Arc::new(AppState { db });

    fn configure_routes() -> Route {
        let upload_base_path = env::var("UPLOAD_PATH").unwrap_or(String::from(".uploads/images"));

        Route::new()
            .at("/health", get(return_str))
            .at("/health-check", get(return_str))
            .at("/posts", get(get_posts))
            .at("/posts/search-with-tags", get(get_posts_with_tags))
            .at("/post/:post_id", get(get_post))
            .at("/post/upload", post(upload_post).with(Auth))
            .at("/post/update/:post_id", put(update_post).with(Auth))
            .at("/post/delete/:post_id", delete(delete_post).with(Auth))
            .at("/tags", get(get_count_with_tags))
            .at("/tags-with-category", get(get_tags_with_category))
            .at("/portfolio", get(get_portfolio))
            .at(
                "/portfolio/update",
                put(update_portfolio).with(AdminOnly).with(Auth),
            )
            .at("/upload-image", post(upload_image).with(Auth))
            .at("/login", post(login))
            .at("/signup", post(signup))
            .at("/me", get(me).with(Auth))
            .at("/admin/add-user", post(add_user).with(AdminOnly).with(Auth))
            .at(
                "/admin/posts",
                get(get_all_posts).with(AdminOnly).with(Auth),
            )
            .at(
                "/api-keys",
                post(create_api_key_handler).get(get_api_keys).with(Auth),
            )
            .at("/api-keys/:api_key_id", delete(delete_api_key).with(Auth))
            .at(
                "/budget",
                get(get_budget.with(Auth)).put(update_active_budget.with(Auth)),
            )
            .at("/budget/plan", post(create_budget_plan).with(Auth))
            .at(
                "/budget/card-excel/remaining-weekly-budget",
                post(calculate_remaining_weekly_budget).with(Auth),
            )
            .at(
                "/budget/spending",
                get(get_spending.with(Auth)).post(create_spending.with(JwtOrApiKeyAuth)),
            )
            .at(
                "/budget/spending/import-preview",
                post(preview_spending_import).with(Auth),
            )
            .at(
                "/budget/spending/import-commit",
                post(commit_spending_import).with(Auth),
            )
            .at(
                "/budget/spending/:record_id",
                put(update_spending.with(Auth)).delete(delete_spending.with(Auth)),
            )
            .nest("/images", StaticFilesEndpoint::new(upload_base_path))
            .at("/*path", options(options_handler))
    }

    let app = configure_routes().data(state).with(
        Cors::new()
            .allow_origin("http://localhost:3001")
            .allow_origin("http://localhost:3000")
            .allow_origin("http://localhost:4000")
            .allow_origin("https://cms.tyange.com")
            .allow_origin("https://blog.tyange.com")
            .allow_origin("https://dashboard.tyange.com")
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allow_credentials(true)
            .allow_headers(vec!["authorization", "x-api-key", "content-type", "accept"]),
    );

    Server::new(TcpListener::bind("0.0.0.0:8080"))
        .run(app)
        .await
}
