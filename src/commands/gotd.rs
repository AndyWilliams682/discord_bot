use async_trait::async_trait;
use reqwest::{
    header::{ToStrError, CONTENT_TYPE},
    Client, Url,
};
use serenity::all::{
    CommandData, CommandDataOptionValue, CommandOptionType, CreateCommand, CreateCommandOption,
    CreateInteractionResponseMessage, User,
};
use thiserror::Error;
use url::ParseError;

use crate::commands::error::CommandError;
use crate::database::DatabaseResult;

#[derive(Debug, Error, PartialEq)]
pub enum UrlValidationError {
    #[error("The URL must use the HTTP or HTTPS scheme")]
    InvalidScheme,

    #[error("{0}")]
    NonSuccessStatus(u16),

    #[error("{0}")]
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

#[async_trait]
pub trait GotdTrait: Send + Sync {
    async fn insert_gif(&self, user_id: u64, url: String) -> DatabaseResult<()>;
    async fn select_random_gif(&self) -> DatabaseResult<(u64, String)>;
}

#[async_trait]
pub trait GifValidator: Send + Sync {
    async fn validate(&self, url: &str) -> Result<(), UrlValidationError>;
}

pub struct RealGifValidator;

#[async_trait]
impl GifValidator for RealGifValidator {
    async fn validate(&self, s: &str) -> Result<(), UrlValidationError> {
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
}

pub async fn run(
    data: &CommandData,
    invoker: &User,
    db: &impl GotdTrait,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let url_option = data
        .options
        .iter()
        .find(|opt| opt.name == "url")
        .map(|opt| &opt.value);

    let attachment_option = data
        .options
        .iter()
        .find(|opt| opt.name == "file")
        .map(|opt| &opt.value);

    let url_string = if let Some(CommandDataOptionValue::String(url)) = url_option {
        Some(url.clone())
    } else if let Some(CommandDataOptionValue::Attachment(attachment_id)) = attachment_option {
        data.resolved
            .attachments
            .get(attachment_id)
            .map(|a| a.url.clone())
    } else {
        None
    };

    let content = if let Some(url) = url_string {
        let validator = RealGifValidator;
        match submit_gif_logic(url, invoker.id.get(), db, &validator).await {
            Ok(()) => "Gif submitted, thank you!".to_string(),
            Err(why) => return Err(why),
        }
    } else {
        return Err(CommandError::InvalidOption(
            "Please provide either a url or a file".to_string(),
        ));
    };

    Ok(CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(true))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("gotd")
        .description("Submit a url for for gif of the day")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "The url of your gif")
                .required(false),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Attachment,
                "file",
                "Upload your gif directly to discord instead!",
            )
            .required(false),
        )
}

pub async fn submit_gif_logic(
    url: String,
    invoker_id: u64,
    db: &impl GotdTrait,
    validator: &impl GifValidator,
) -> Result<(), CommandError> {
    validator.validate(&url).await?;
    Ok(db.insert_gif(invoker_id, url).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseError;

    struct MockGotdDB {
        should_fail: bool,
    }

    #[async_trait]
    impl GotdTrait for MockGotdDB {
        async fn insert_gif(&self, _user_id: u64, _url: String) -> DatabaseResult<()> {
            if self.should_fail {
                Err(DatabaseError::QueryError("Mock DB Error".to_string()))
            } else {
                Ok(())
            }
        }
        async fn select_random_gif(&self) -> DatabaseResult<(u64, String)> {
            Ok((0, "".to_string())) // Not used in submit logic
        }
    }

    struct MockGifValidator {
        should_fail: bool,
    }

    #[async_trait]
    impl GifValidator for MockGifValidator {
        async fn validate(&self, _url: &str) -> Result<(), UrlValidationError> {
            if self.should_fail {
                Err(UrlValidationError::InvalidScheme)
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn submit_gif_success() {
        let db = MockGotdDB { should_fail: false };
        let validator = MockGifValidator { should_fail: false };
        let result = submit_gif_logic(
            "https://example.com/image.gif".to_string(),
            123,
            &db,
            &validator,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn submit_gif_validation_failure() {
        let db = MockGotdDB { should_fail: false };
        let validator = MockGifValidator { should_fail: true };
        let result = submit_gif_logic("bad_url".to_string(), 123, &db, &validator).await;
        // Expecting UrlValidationError converted to CommandError
        assert!(matches!(result, Err(CommandError::UrlValidation(_))));
    }

    #[tokio::test]
    async fn submit_gif_db_failure() {
        let db = MockGotdDB { should_fail: true };
        let validator = MockGifValidator { should_fail: false };
        let result = submit_gif_logic(
            "https://example.com/image.gif".to_string(),
            123,
            &db,
            &validator,
        )
        .await;
        // Expecting DatabaseError converted to CommandError
        assert!(matches!(result, Err(CommandError::Database(_))));
    }
}
