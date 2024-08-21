use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl};
use tokio_util::sync::CancellationToken;
use tower_sessions_sqlx_store::SqliteStore;

use crate::config::InstancerConfig;
use crate::database::Database;
use crate::deployment_worker::DeploymentWorker;

pub struct InstancerState {
    pub config: InstancerConfig,
    pub database: Database,
    pub deployer: DeploymentWorker,
    pub session_store: SqliteStore,
    pub shutdown_token: CancellationToken,
    pub oauth2: BasicClient,
}

impl InstancerState {
    pub fn new(config: InstancerConfig, database: Database, deployer: DeploymentWorker, session_store: SqliteStore, shutdown_token: CancellationToken) -> InstancerState {
        let oauth2 = BasicClient::new(
            ClientId::new(config.discord.client_id.clone()),
            Some(ClientSecret::new(config.discord.client_secret.clone())),
            AuthUrl::new("https://discord.com/oauth2/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://discord.com/api/oauth2/token".to_string()).unwrap())
        )
            .set_revocation_uri(RevocationUrl::new("https://discord.com/api/oauth2/token/revoke".to_string()).unwrap())
            .set_redirect_uri(RedirectUrl::new(config.discord.redirect_url.clone()).unwrap());

        InstancerState {
            config,
            database,
            deployer,
            session_store,
            shutdown_token,
            oauth2,
        }
    }
}