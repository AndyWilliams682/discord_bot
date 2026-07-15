use ::config::{Config, ConfigError, Environment, File};
use dotenvy::dotenv;
use serde::Deserialize;
use serenity::prelude::TypeMapKey;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

pub struct BotConfigWrapper;

impl TypeMapKey for BotConfigWrapper {
    type Value = Arc<BotConfig>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    // Loaded from environment
    pub discord_token: String,

    // Loaded from TOML (overwritten by environment)
    pub status_update_time: u64, // Time in seconds between status updates

    pub data_folder: String, // Where the database and gif folders are located
    pub database_name: String, // Name of the database file

    pub gif_post_hour: u32,       // Hour to post the gif of the day
    pub gif_guild_id: u64,        // Server ID that bot runs in
    pub gif_channel_name: String, // Name of the channel for posting gifs
    pub gif_base_url: String,     // Url used to point to the gif

    pub secret_admin_id: u64, // User ID of the Secret Santa admin

    #[serde(default)]
    pub poe_accounts: HashMap<String, String>, // Discord user ID -> Path of Exile account name
}

impl BotConfig {
    pub fn load() -> Result<Self, ConfigError> {
        dotenv().ok();

        let mut builder = Config::builder()
            .add_source(File::with_name("config").required(true))
            .add_source(Environment::default().separator("__").try_parsing(true));

        if let Ok(token) = env::var("DISCORD_TOKEN") {
            builder = builder.set_override("discord_token", token)?;
        }

        let config = builder.build()?.try_deserialize::<Self>()?;
        if config.discord_token.trim().is_empty() {
            return Err(ConfigError::Message(
                "DISCORD_TOKEN must be set in .env or the environment".to_string(),
            ));
        }

        Ok(config)
    }
}
