use serde::Deserialize;

#[derive(Deserialize)]
pub struct InstancerConfig {
    pub discord: Discord
}

#[derive(Deserialize)]
pub struct Discord {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String
}