use const_format::concatcp;
use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

const HOST: &'static str = "https://discord.com/api/v10";

pub struct Discord {
    access_token: String,
    client: reqwest::Client
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    pub global_name: String,
    pub avatar: String
}

#[derive(Deserialize, Debug)]
pub struct Guild {
    pub id: String
}

#[derive(Error, Debug)]
pub enum DiscordError {
    #[error("oauth2 scope missing")]
    MissingScope,
    #[error("api error `{0}`\n{1}")]
    Unknown(StatusCode, String)
}

impl Discord {
    pub fn new(access_token: String) -> Self {
        Discord {
            access_token,
            client: reqwest::Client::new()
        }
    }

    pub async fn current_user(&self) -> anyhow::Result<User> {
        let response = self.client.get(concatcp!(HOST, "/users/@me"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::UNAUTHORIZED => Err(DiscordError::MissingScope.into()),
            _ => Err(DiscordError::Unknown(response.status(), response.text().await?).into())
        }
    }

    pub async fn current_guilds(&self) -> anyhow::Result<Vec<Guild>> {
        let response = self.client.get(concatcp!(HOST, "/users/@me/guilds"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::UNAUTHORIZED => Err(DiscordError::MissingScope.into()),
            _ => Err(DiscordError::Unknown(response.status(), response.text().await?).into())
        }
    }
}