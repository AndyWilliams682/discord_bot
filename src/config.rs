use serenity::prelude::TypeMapKey;
use std::env;
use std::sync::Arc;

pub struct BotConfigWrapper;

impl TypeMapKey for BotConfigWrapper {
    type Value = Arc<BotConfig>;
}

#[derive(Debug, Clone)]
pub struct BotConfig {
    // Loaded from environment
    pub discord_token: String,

    // Loaded from TOML (overwritten by environment)
    pub status_update_time: u64, // Time in seconds between status updates

    pub data_folder: String, // Where the database and gif folders are located
    pub database_name: String, // Name of the database file

    pub gif_post_hour: u32, // Hour to post the gif of the day
    pub gif_guild_id: u64, // Server ID that bot runs in
    pub gif_channel_name: String, // Name of the channel for posting gifs
    pub gif_base_url: String, // Url used to point to the gif

    pub secret_admin_id: u64, // User ID of the Secret Santa admin
}

impl BotConfig {
    pub fn load() -> Result<Self, ConfigError> {
        dotenv().ok();

        let settings = Config::builder()
            .add_source(File::with_name("config"))
            .add_source(Environment::default())
            .build()?;
        settings.try_deserialize::<Self>()
    }
}



