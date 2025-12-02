use serenity::all::{CreateCommand, CreateCommandOption, CommandDataOption, CommandDataOptionValue, CommandOptionType, User, CreateInteractionResponseMessage};
use rusqlite::{Result, params};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use async_trait::async_trait;

use crate::database::DbPool;


#[async_trait]
pub trait GotdRepository: Send + Sync {
    async fn submit_gif(&self, user_id: u64, username: String, url: String) -> Result<(), String>;
}

pub struct GotdRepositoryImpl {
    pool: DbPool,
}

impl GotdRepositoryImpl {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GotdRepository for GotdRepositoryImpl {
    async fn submit_gif(&self, user_id: u64, username: String, url: String) -> Result<(), String> {
        let pool_clone = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool_clone.get().map_err(|e| e.to_string())?;
            
            conn.execute("
                INSERT OR IGNORE INTO users (user_id, username)
                VALUES (?1, ?2);
            ", params![user_id, username]).map_err(|e| e.to_string())?;

            conn.execute("
                INSERT INTO gifs (submitted_by, url, posts)
                VALUES (?1, ?2, 0);
            ", params![user_id, url]).map_err(|e| e.to_string())?;
            
            Ok(())
        }).await.map_err(|e| e.to_string())?
    }
}

pub async fn submit_gif_logic(
    url: String, 
    invoker_id: u64, 
    invoker_name: String, 
    repo: &impl GotdRepository
) -> String {
    
    if !is_valid_url(&url).await {
        return "Invalid URL".to_string();
    }

    match repo.submit_gif(invoker_id, invoker_name, url).await {
        Ok(_) => "Gif added, thank you!".to_string(),
        Err(e) => {
            println!("Database error during gif submission: {}", e);
            "Database operation failed.".to_string()
        }
    }
}


async fn is_valid_url(s: &str) -> bool {
    let url = match Url::parse(s) {
        Ok(url) => {
            if !matches!(url.scheme(), "http" | "https") {
                return false
            }
            url
        },
        Err(_) => return false,
    };

    let client = Client::new();
    match client.head(url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                return false;
            }

            if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
                if let Ok(content_type_str) = content_type.to_str() {
                    let content_type_lower = content_type_str.to_lowercase();

                    return content_type_lower.starts_with("image/gif") ||
                           content_type_lower.starts_with("video/webm") ||
                           content_type_lower.starts_with("video/mp4");
                }
            }
            false
        }
        Err(e) => {
            eprintln!("Network error verifying URL: {}", e);
            false
        }
    }
}


pub fn register() -> CreateCommand {
    CreateCommand::new("gotd")
        .description("Submit a url for for gif of the day")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "The url of your gif")
                .required(true)
        )
}

pub async fn run(options: &[CommandDataOption], invoker: &User, pool: &DbPool) -> CreateInteractionResponseMessage {
    let url_option = &options.get(0).expect("Expected string option").value;
    let repo = GotdRepositoryImpl::new(pool.clone());
    let content = if let CommandDataOptionValue::String(url) = url_option {
        submit_gif_logic(url.clone(), invoker.id.get(), invoker.name.clone(), &repo).await
    } else {
        "How did you input a non-string?".to_string()
    };
    CreateInteractionResponseMessage::new().content(content).ephemeral(true)
}
