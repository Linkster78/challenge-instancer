use config::{Config, File};
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl};
use oauth2::basic::BasicClient;

use crate::config::InstancerConfig;
use crate::database::InstancerDatabase;

pub struct InstancerState {
    pub config: InstancerConfig,
    pub database: InstancerDatabase,
    pub oauth2: BasicClient
}

impl InstancerState {
    pub async fn new() -> anyhow::Result<InstancerState> {
        let config: InstancerConfig = Config::builder()
            .add_source(File::with_name("config.toml"))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        let database = InstancerDatabase::new(&config.database).await?;

        let oauth2 = BasicClient::new(
            ClientId::new(config.discord.client_id.clone()),
            Some(ClientSecret::new(config.discord.client_secret.clone())),
            AuthUrl::new("https://discord.com/oauth2/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://discord.com/api/oauth2/token".to_string()).unwrap())
        )
            .set_revocation_uri(RevocationUrl::new("https://discord.com/api/oauth2/token/revoke".to_string()).unwrap())
            .set_redirect_uri(RedirectUrl::new(config.discord.redirect_url.clone()).unwrap());

        Ok(InstancerState {
            config,
            database,
            oauth2
        })
    }
}