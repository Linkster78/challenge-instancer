use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration;

use crate::state::InstancerState;

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

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)))
        .with_secure(false);

    let state = InstancerState::new().await.expect("failed to initialize state");
    let state = Arc::new(state);

    let app = Router::new()
        .route("/", get(router::dashboard))
        .route("/login", get(router::login))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
        .layer(session_layer);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}