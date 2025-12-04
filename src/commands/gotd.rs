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

use crate::database::{DatabaseError, DatabaseResult};

#[derive(Debug, Error, PartialEq)]
pub enum UrlValidationError {
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

#[derive(Debug, Error, PartialEq)]
pub enum GotdError {
    #[error("The url provided is not valid: {0}")]
    Validation(#[from] UrlValidationError),

    #[error("The database encountered an error: {0}")]
    Database(#[from] DatabaseError),
}

#[async_trait]
pub trait GotdTrait: Send + Sync {
    async fn insert_gif(&self, user_id: u64, username: String, url: String) -> DatabaseResult<()>;
    async fn select_random_gif(&self) -> DatabaseResult<(u64, String)>;
}

pub async fn run(
    options: &[CommandDataOption],
    invoker: &User,
    db: &impl GotdTrait,
) -> CreateInteractionResponseMessage {
    let url_option = &options.get(0).expect("Expected string option").value;
    let content = if let CommandDataOptionValue::String(url) = url_option {
        match submit_gif_logic(url.clone(), invoker.id.get(), invoker.name.clone(), db).await {
            Ok(()) => "Gif submitted, thank you!".to_string(),
            Err(why) => why.to_string(),
        }
    } else {
        "How did you input a non-string?".to_string()
    };
    CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(true)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("gotd")
        .description("Submit a url for for gif of the day")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "The url of your gif")
                .required(true),
        )
}

async fn is_valid_gif_url(s: &str) -> Result<(), UrlValidationError> {
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

pub async fn submit_gif_logic(
    url: String,
    invoker_id: u64,
    invoker_name: String,
    db: &impl GotdTrait,
) -> Result<(), GotdError> {
    is_valid_gif_url(&url).await?;
    Ok(db.insert_gif(invoker_id, invoker_name, url).await?)
}
