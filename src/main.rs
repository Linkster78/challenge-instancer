extern crate alloc;
extern crate core;

use std::sync::Arc;

use crate::config::InstancerConfig;
use crate::database::Database;
use crate::deployment_worker::DeploymentWorker;
use crate::state::InstancerState;
use axum::routing::get;
use axum::Router;
use ::config::{Config, File};
use sd_notify::NotifyState;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::ConnectOptions;
use tokio::net::TcpListener;
use tokio::{signal};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tower_http::services::ServeDir;
use tower_sessions::cookie::time::Duration;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::{sqlx::SqlitePool, SqliteStore};
use tracing::log::LevelFilter;

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
    tracing_subscriber::fmt()
        .event_format(
            tracing_subscriber::fmt::format()
                .with_file(true)
                .with_line_number(true)
        )
        .init();

    let config: InstancerConfig = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()?
        .try_deserialize()?;

    let sqlite_pool = SqlitePool::connect_with(SqliteConnectOptions::new()
        .create_if_missing(true)
        .log_statements(LevelFilter::Trace)
        .filename(config.database.file_path.clone()))
        .await.expect("failed to setup sqlite pool for session store");
    let database = Database::new(sqlite_pool.clone()).await?;

    let shutdown_token = CancellationToken::new();
    let deployer = DeploymentWorker::new(&config, database.clone(), shutdown_token.clone());

    deployer.prepare().await?;

    let session_store = SqliteStore::new(sqlite_pool);
    session_store.migrate().await.expect("failed to migrate session store");

    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(3)))
        .with_http_only(false)
        .with_secure(false);

    let state = Arc::new(InstancerState::new(config, database, deployer, session_store, shutdown_token.clone()));

    let mut workers = JoinSet::new();
    for _ in 1..=state.config.settings.worker_count {
        let state = Arc::clone(&state);
        workers.spawn(async move { state.deployer.do_work().await });
    }

    let app = Router::new()
        .route("/", get(router::dashboard))
        .route("/help", get(router::help))
        .route("/login", get(router::login))
        .route("/logout", get(router::logout))
        .route("/ws", get(router::dashboard_ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(Arc::clone(&state))
        .layer(session_layer);

    tracing::info!("started instancer on {}", state.config.settings.listen_on);

    let listener = TcpListener::bind(&state.config.settings.listen_on).await?;
    let _ = sd_notify::notify(true, &[NotifyState::Ready]);
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;

    tracing::info!("shutdown requested, draining pending deployment requests...");

    shutdown_token.cancel();
    workers.join_all().await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}