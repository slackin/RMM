mod db;
mod ffmpeg;
mod routes;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

pub struct AppState {
    pub db: db::Database,
    pub job_processor: Mutex<()>, // serializes encoding jobs
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db = db::Database::open("media_manager.db").expect("Failed to open database");
    db.migrate().expect("Failed to run migrations");

    let state = Arc::new(AppState {
        db,
        job_processor: Mutex::new(()),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(routes::health))
        .route("/api/scan", post(routes::scan_directory))
        .route("/api/files", get(routes::list_files))
        .route("/api/files/{id}", get(routes::get_file))
        .route("/api/files/{id}", axum::routing::delete(routes::delete_file))
        .route("/api/encode", post(routes::start_encode))
        .route("/api/jobs", get(routes::list_jobs))
        .route("/api/jobs/{id}", get(routes::get_job))
        .layer(cors)
        .with_state(state);

    let bind_addr = "0.0.0.0:9090";
    tracing::info!("Media Manager server listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
