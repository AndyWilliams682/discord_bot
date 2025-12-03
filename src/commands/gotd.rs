use async_trait::async_trait;
use reqwest::{
    header::{ToStrError, CONTENT_TYPE},
    Client, Url,
};
use serenity::all::{
    CommandDataOption, CommandDataOptionValue, CommandOptionType, CreateCommand,
    CreateCommandOption, CreateInteractionResponseMessage, User,
};
use thiserror::Error;
use url::ParseError;

#[derive(Debug, Error, PartialEq)]
pub enum UrlValidationError {
    #[error("The URL format is invalid.")]
    InvalidFormat,

    #[error("The URL must use the HTTP or HTTPS scheme")]
    InvalidScheme,

    #[error("The network request returned a non-success status code: {0}")]
    NonSuccessStatus(u16),

    #[error("The content type is invalid or missing: {0}")]
    InvalidContentType(String),

    #[error("Could not parse url")]
    Parse(#[from] ParseError),
}

impl From<reqwest::Error> for UrlValidationError {
    fn from(_e: reqwest::Error) -> Self {
        UrlValidationError::InvalidContentType("The header is missing".to_string())
    }
}

impl From<ToStrError> for UrlValidationError {
    fn from(_e: ToStrError) -> Self {
        UrlValidationError::InvalidContentType(
            "The header could not be interpreted as string".to_string(),
        )
    }
}

async fn is_valid_url(s: &str) -> Result<(), UrlValidationError> {
    let url = Url::parse(s)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(UrlValidationError::InvalidScheme);
    }

    let client = Client::new();

    let response = client.head(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(UrlValidationError::NonSuccessStatus(status.as_u16()));
    }
    if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let content_type_str = content_type.to_str()?.to_lowercase();
        if content_type_str.starts_with("image/gif")
            || content_type_str.starts_with("video/webm")
            || content_type_str.starts_with("video/mp4")
        {
            return Ok(());
        } else {
            return Err(UrlValidationError::InvalidContentType(content_type_str));
        }
    }
    Err(UrlValidationError::InvalidContentType(
        "Header Missing".to_string(),
    ))
}

#[derive(Debug, PartialEq)]
pub enum GotdError {
    Validation(UrlValidationError),
    Database(String),
    Internal(String),
}

#[async_trait]
pub trait InsertGif: Send + Sync {
    async fn insert_gif(&self, user_id: u64, username: String, url: String) -> Result<(), String>;
}

#[async_trait] // Might want to combine with InsertGif
pub trait SelectRandomGif: Send + Sync {
    async fn select_random_gif(&self) -> Result<(u64, String), String>;
}

pub async fn submit_gif_logic(
    url: String,
    invoker_id: u64,
    invoker_name: String,
    repo: &impl InsertGif,
) -> String {
    if is_valid_url(&url).await.is_err() {
        return "Invalid URL".to_string();
    }

    match repo.insert_gif(invoker_id, invoker_name, url).await {
        Ok(_) => "Gif added, thank you!".to_string(),
        Err(e) => {
            println!("Database error during gif submission: {}", e);
            "Database operation failed.".to_string()
        }
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("gotd")
        .description("Submit a url for for gif of the day")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "The url of your gif")
                .required(true),
        )
}

pub async fn run(
    options: &[CommandDataOption],
    invoker: &User,
    repo: &impl InsertGif,
) -> CreateInteractionResponseMessage {
    let url_option = &options.get(0).expect("Expected string option").value;
    let content = if let CommandDataOptionValue::String(url) = url_option {
        submit_gif_logic(url.clone(), invoker.id.get(), invoker.name.clone(), repo).await
    } else {
        "How did you input a non-string?".to_string()
    };
    CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(true)
}
