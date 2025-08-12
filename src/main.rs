mod db;
mod middlewares;
mod models;
mod routes;
mod utils;

use dotenv::dotenv;
use middlewares::auth_middleware::Auth;
use std::{env, fs, path::PathBuf, sync::Arc};

use crate::models::AppState;
use crate::routes::delete_post::delete_post;
use crate::routes::update_post::update_post;
use crate::routes::upload_image::upload_image;
use db::init_db;
use poem::{
    delete, endpoint::StaticFilesEndpoint, get, handler, http::StatusCode, listener::TcpListener,
    middleware::Cors, options, post, put, EndpointExt, Response, Route, Server,
};
use routes::{get_post::get_post, get_posts::get_posts, login::login, upload_post::upload_post};
use sqlx::SqlitePool;

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

    let state = Arc::new(AppState {
        db,
        upload_dir: PathBuf::from("./uploads"),
    });

    fn configure_routes() -> Route {
        let upload_base_path = env::var("UPLOAD_PATH").unwrap_or(String::from(".uploads/images"));

        Route::new()
            .at("/posts", get(get_posts))
            .at("/post/:post_id", get(get_post))
            .at("/post/upload", post(upload_post).with(Auth))
            .at("/post/update/:post_id", put(update_post).with(Auth))
            .at("/post/delete/:post_id", delete(delete_post).with(Auth))
            .at("/upload-image", post(upload_image).with(Auth))
            .at("/login", post(login))
            .nest("/images", StaticFilesEndpoint::new(upload_base_path))
            .at("/*path", options(options_handler))
    }

    let app = configure_routes().data(state).with(
        Cors::new()
            .allow_origin("http://localhost:3001")
            .allow_origin("http://localhost:3000")
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allow_credentials(true)
            .allow_headers(vec!["authorization", "content-type", "accept"]),
    );

    Server::new(TcpListener::bind("0.0.0.0:8080"))
        .run(app)
        .await
}
