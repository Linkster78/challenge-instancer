use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use sqlx::sqlite::SqliteConnectOptions;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration;
use crate::state::InstancerState;
use tower_sessions_sqlx_store::{sqlx::SqlitePool, SqliteStore};

mod router;
mod templating;
mod config;
mod state;
mod discord;
mod database;
mod models;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = InstancerState::new().await.expect("failed to initialize state");
    let state = Arc::new(state);

    let pool = SqlitePool::connect_with(SqliteConnectOptions::new()
        .create_if_missing(true)
        .filename(state.config.database.file_path.clone()))
        .await.expect("failed to setup sqlite pool for session store");
    let session_store = SqliteStore::new(pool);
    session_store.migrate().await.expect("failed to migrate session store");

    let session_layer = SessionManagerLayer::new(session_store)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(3)))
        .with_secure(false);

    let app = Router::new()
        .route("/", get(router::dashboard))
        .route("/login", get(router::login))
        .route("/logout", get(router::logout))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
        .layer(session_layer);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}