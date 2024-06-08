use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct InstancerConfig {
    pub discord: DiscordConfig,
    pub database: DatabaseConfig
}

#[derive(Deserialize, Debug)]
pub struct DiscordConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub server_id: String
}

#[derive(Deserialize, Debug)]
pub struct DatabaseConfig {
    pub file_path: PathBuf
}