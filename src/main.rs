mod db;
mod models;
mod routes;

use std::{path::PathBuf, sync::Arc};

use db::init_db;
use poem::{listener::TcpListener, EndpointExt, Route, Server};
use routes::upload_post::upload_post;
use sqlx::{Pool, Sqlite, SqlitePool};

pub struct AppState {
    pub db: Pool<Sqlite>,
    pub upload_dir: PathBuf,
}

fn configure_routes() -> Route {
    Route::new().at("/upload-post", poem::post(upload_post))
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let db_path = "./.db/database.db";
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

    let app = configure_routes().data(state);

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
