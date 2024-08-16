extern crate alloc;

use std::sync::Arc;

use ::config::{Config, File};
use axum::Router;
use axum::routing::get;
use sqlx::sqlite::SqliteConnectOptions;
use tokio::net::TcpListener;
use tokio::task;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration;
use tower_sessions_sqlx_store::{SqliteStore, sqlx::SqlitePool};

use crate::config::InstancerConfig;
use crate::database::Database;
use crate::deployment_worker::DeploymentWorker;
use crate::state::InstancerState;

mod router;
mod templating;
mod config;
mod state;
mod discord;
mod database;
mod models;
mod deployment_worker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config: InstancerConfig = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()?
        .try_deserialize()?;

    let sqlite_pool = SqlitePool::connect_with(SqliteConnectOptions::new()
        .create_if_missing(true)
        .filename(config.database.file_path.clone()))
        .await.expect("failed to setup sqlite pool for session store");
    let database = Database::new(sqlite_pool.clone()).await?;

    let deployer = DeploymentWorker::new(&config, database.clone());

    let session_store = SqliteStore::new(sqlite_pool);
    session_store.migrate().await.expect("failed to migrate session store");

    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(3)))
        .with_http_only(false)
        .with_secure(false);

    let state = Arc::new(InstancerState::new(config, database, deployer, session_store));
    let state_c = Arc::clone(&state);

    task::spawn(async move { state_c.deployer.do_work().await });

    let app = Router::new()
        .route("/", get(router::dashboard))
        .route("/login", get(router::login))
        .route("/logout", get(router::logout))
        .route("/ws", get(router::dashboard_ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
        .layer(session_layer);

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}