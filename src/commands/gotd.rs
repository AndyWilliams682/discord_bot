use serenity::all::{CreateCommand, CreateCommandOption, CommandDataOption, CommandDataOptionValue, CommandOptionType, User, CreateInteractionResponseMessage};
use rusqlite::{Result, params};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use crate::database::DbPool;


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


async fn run_wrapped(url: &str, invoker: &User, pool: &DbPool) -> Result<String> {
    let conn = pool.get().map_err(|_| rusqlite::Error::QueryReturnedNoRows)?; // TODO: Better error handling

    conn.execute("
        INSERT OR IGNORE INTO users (user_id, username)
        VALUES (?1, ?2);
    ", params![invoker.id.get(), invoker.name])?;

    return match is_valid_url(&url).await {
        true => {
            conn.execute("
                INSERT INTO gifs (submitted_by, url, posts)
                VALUES (?1, ?2, 0);
            ", params![invoker.id.get(), url])?;
            Ok("Gif added, thank you!".to_string())
        },
        false => Ok("Your url does not appear to be valid".to_string())
    }
}


pub async fn run(options: &[CommandDataOption], invoker: &User, pool: &DbPool) -> CreateInteractionResponseMessage {
    let first_option = &options
        .get(0)
        .expect("Expected string option")
        .value;
    let content = if let CommandDataOptionValue::String(url) = first_option {
        match run_wrapped(&url, invoker, pool).await {
            Ok(reply) => reply,
            Err(e) => format!("{}", e)
        }
    } else {
        "How did you input a non-string?".to_string()
    };
    CreateInteractionResponseMessage::new().content(content).ephemeral(true)
}
