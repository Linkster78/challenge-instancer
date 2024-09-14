use const_format::concatcp;
use serde::Deserialize;

const HOST: &'static str = "https://discord.com/api/v10";

pub const SCOPES: [&'static str; 2] = ["identify", "guilds"];

pub struct Discord {
    access_token: String,
    client: reqwest::Client
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>
}

#[derive(Deserialize, Debug)]
pub struct Guild {
    pub id: String
}

impl Discord {
    pub fn new(access_token: String) -> Self {
        Discord {
            access_token,
            client: reqwest::Client::new()
        }
    }

    pub async fn current_user(&self) -> anyhow::Result<User> {
        Ok(self.client.get(concatcp!(HOST, "/users/@me"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await?
            .json().await?)
    }

    pub async fn current_guilds(&self) -> anyhow::Result<Vec<Guild>> {
        Ok(self.client.get(concatcp!(HOST, "/users/@me/guilds"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await?
            .json().await?)
    }

    pub fn avatar_url(id: &str, avatar: &Option<String>) -> String {
        match avatar {
            None => String::from("https://discordapp.com/assets/a0180771ce23344c2a95.png"),
            Some(avatar_hash) => format!("https://cdn.discordapp.com/avatars/{}/{}.png", id, avatar_hash)
        }
    }
}