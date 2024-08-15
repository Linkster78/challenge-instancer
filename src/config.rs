use std::collections::HashMap;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

#[derive(Deserialize, Debug)]
pub struct InstancerConfig {
    pub settings: SettingsConfig,
    pub discord: DiscordConfig,
    pub database: DatabaseConfig,
    pub deployers: HashMap<String, DeployerConfig>,
    pub challenges: HashMap<String, ChallengeConfig>
}

#[derive(Deserialize, Debug)]
pub struct SettingsConfig {
    pub max_concurrent_challenges: u32
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

#[derive(Deserialize, Debug)]
pub struct DeployerConfig {
    pub path: PathBuf
}

#[derive(Deserialize, Debug)]
pub struct ChallengeConfig {
    pub name: String,
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_duration")]
    pub ttl: u32,
    pub deployer: String
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<u32, D::Error>
where D: Deserializer<'de>
{
    static DURATION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[1-9]\d*[smhd]$").unwrap());

    let s: String = Deserialize::deserialize(deserializer)?;
    if !DURATION_RE.is_match(&s) {
        return Err(Error::custom(format!("value \"{}\" didn't match duration regex", s)))
    }

    let multiplier = match s.chars().last().unwrap() {
        's' => 1,
        'm' => 60,
        'h' => 60 * 60,
        'd' => 60 * 60 * 24,
        _ => panic!("this should never happen")
    };

    let value: u32 = s[..s.len() - 1].parse::<u32>().unwrap();
    return Ok(value * multiplier);
}