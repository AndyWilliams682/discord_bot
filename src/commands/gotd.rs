use async_trait::async_trait;
use reqwest::{
    header::{ToStrError, CONTENT_TYPE},
    Client, Url,
};
use serenity::all::{
    CommandData, CommandDataOptionValue, CommandOptionType, CreateCommand, CreateCommandOption,
    User,
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
    async fn insert_gif(&self, user_id: u64, name: String) -> DatabaseResult<()>;
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

#[async_trait]
pub trait FileDownloader: Send + Sync {
    async fn download(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub struct RealFileDownloader;

#[async_trait]
impl FileDownloader for RealFileDownloader {
    async fn download(&self, url: &str) -> Result<Vec<u8>, String> {
        let client = Client::new();
        let bytes = client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    }
}

#[derive(Debug)]
pub enum GifSubmission {
    Url(String),
    Attachment { url: String, filename: String },
}

impl GifSubmission {
    pub async fn new(
        url_opt: Option<String>,
        attachment_opt: Option<(String, String)>,
        validator: &impl GifValidator,
    ) -> Result<Self, CommandError> {
        let submission = if let Some(url) = url_opt {
            validator.validate(&url).await?;
            GifSubmission::Url(url)
        } else if let Some((url, filename)) = attachment_opt {
            validator.validate(&url).await?;
            GifSubmission::Attachment { url, filename }
        } else {
            return Err(CommandError::InvalidOption(
                "Please provide either a url or a file".to_string(),
            ));
        };
        Ok(submission)
    }

    pub async fn save_to_file(
        &self,
        custom_name: Option<String>,
        downloader: &impl FileDownloader,
        gif_dir: &str,
    ) -> Result<std::path::PathBuf, CommandError> {
        let (source_url, original_filename) = match self {
            GifSubmission::Url(url) => (url, None),
            GifSubmission::Attachment { url, filename } => (url, Some(filename.as_str())),
        };

        // Determine extension
        let extension = if let Some(orig_name) = original_filename {
            std::path::Path::new(orig_name)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("gif")
                .to_string()
        } else {
            // Try to extract from URL path
            if let Ok(parsed_url) = Url::parse(source_url) {
                std::path::Path::new(parsed_url.path())
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("gif")
                    .to_string()
            } else {
                "gif".to_string()
            }
        };

        // Determine final filename
        let dest_filename = if let Some(custom) = custom_name {
            let has_ext = std::path::Path::new(&custom).extension().is_some();
            if has_ext {
                custom
            } else {
                format!("{}.{}", custom, extension)
            }
        } else if let Some(orig_name) = original_filename {
            orig_name.to_string()
        } else {
            format!("gif_{}.{}", rand::random::<u32>(), extension)
        };

        let save_dir = std::path::Path::new(gif_dir);
        std::fs::create_dir_all(save_dir)
            .map_err(|e| CommandError::Generic(format!("Failed to create directories: {}", e)))?;

        let dest_path = save_dir.join(dest_filename);

        let bytes = downloader
            .download(source_url)
            .await
            .map_err(|e| CommandError::Generic(format!("Failed to download file: {}", e)))?;

        std::fs::write(&dest_path, bytes)
            .map_err(|e| CommandError::Generic(format!("Failed to write file: {}", e)))?;

        Ok(dest_path)
    }
}

pub async fn run(
    data: &CommandData,
    invoker: &User,
    db: &impl GotdTrait,
    gif_dir: &str,
) -> Result<String, CommandError> {
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

    let name_option = data
        .options
        .iter()
        .find(|opt| opt.name == "name")
        .and_then(|opt| {
            if let CommandDataOptionValue::String(ref name) = opt.value {
                Some(name.clone())
            } else {
                None
            }
        });

    let url_opt = url_option.and_then(|val| {
        if let CommandDataOptionValue::String(ref url) = val {
            Some(url.clone())
        } else {
            None
        }
    });

    let attachment_opt = attachment_option.and_then(|val| {
        if let CommandDataOptionValue::Attachment(attachment_id) = val {
            data.resolved
                .attachments
                .get(attachment_id)
                .map(|a| (a.url.clone(), a.filename.clone()))
        } else {
            None
        }
    });

    let validator = RealGifValidator;
    let submission = GifSubmission::new(url_opt, attachment_opt, &validator).await?;

    let downloader = RealFileDownloader;
    match submit_gif_logic(
        submission,
        name_option,
        invoker.id.get(),
        db,
        &downloader,
        gif_dir,
    )
    .await
    {
        Ok(()) => Ok("Gif submitted, thank you!".to_string()),
        Err(why) => Err(why),
    }
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
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "name",
                "A custom name to save the gif as",
            )
            .required(false),
        )
}

pub async fn submit_gif_logic(
    submission: GifSubmission,
    custom_name: Option<String>,
    invoker_id: u64,
    db: &impl GotdTrait,
    downloader: &impl FileDownloader,
    gif_dir: &str,
) -> Result<(), CommandError> {
    let saved_path = submission
        .save_to_file(custom_name, downloader, gif_dir)
        .await?;
    let stem = saved_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CommandError::Generic("Invalid file name".to_string()))?
        .to_string();
    Ok(db.insert_gif(invoker_id, stem).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockGotdDB {
        inserted: Mutex<Option<(u64, String)>>,
        random_res: Option<(u64, String)>,
    }

    #[async_trait]
    impl GotdTrait for MockGotdDB {
        async fn insert_gif(&self, user_id: u64, name: String) -> DatabaseResult<()> {
            *self.inserted.lock().unwrap() = Some((user_id, name));
            Ok(())
        }
        async fn select_random_gif(&self) -> DatabaseResult<(u64, String)> {
            Ok(self.random_res.clone().unwrap())
        }
    }

    struct MockGifValidator {
        is_valid: bool,
    }

    #[async_trait]
    impl GifValidator for MockGifValidator {
        async fn validate(&self, _url: &str) -> Result<(), UrlValidationError> {
            if self.is_valid {
                Ok(())
            } else {
                Err(UrlValidationError::InvalidScheme)
            }
        }
    }

    struct MockFileDownloader;

    #[async_trait]
    impl FileDownloader for MockFileDownloader {
        async fn download(&self, _url: &str) -> Result<Vec<u8>, String> {
            Ok(b"mock_gif_data".to_vec())
        }
    }

    #[tokio::test]
    async fn test_submit_gif_logic_success() {
        let db = MockGotdDB {
            inserted: Mutex::new(None),
            random_res: None,
        };
        let validator = MockGifValidator { is_valid: true };
        let downloader = MockFileDownloader;

        let submission = GifSubmission::new(
            Some("http://example.com/test.gif".to_string()),
            None,
            &validator,
        )
        .await
        .unwrap();

        let temp_dir = std::env::temp_dir();
        let temp_dir_str = temp_dir.to_str().unwrap();

        let res = submit_gif_logic(
            submission,
            Some("my_test_gif".to_string()),
            123,
            &db,
            &downloader,
            temp_dir_str,
        )
        .await;
        assert!(res.is_ok());

        let inserted = db.inserted.lock().unwrap().clone().unwrap();
        assert_eq!(inserted.0, 123);
        assert_eq!(inserted.1, "my_test_gif");

        // Clean up
        let path = temp_dir.join("my_test_gif.gif");
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    #[tokio::test]
    async fn test_submit_gif_logic_attachment() {
        let db = MockGotdDB {
            inserted: Mutex::new(None),
            random_res: None,
        };
        let validator = MockGifValidator { is_valid: true };
        let downloader = MockFileDownloader;

        let submission = GifSubmission::new(
            None,
            Some((
                "http://example.com/attachment.gif".to_string(),
                "original.gif".to_string(),
            )),
            &validator,
        )
        .await
        .unwrap();

        let temp_dir = std::env::temp_dir();
        let temp_dir_str = temp_dir.to_str().unwrap();

        let res = submit_gif_logic(
            submission,
            None,
            123,
            &db,
            &downloader,
            temp_dir_str
        ).await;
        assert!(res.is_ok());

        let inserted = db.inserted.lock().unwrap().clone().unwrap();
        assert_eq!(inserted.0, 123);
        assert_eq!(inserted.1, "original");

        // Clean up
        let path = temp_dir.join("original.gif");
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    #[tokio::test]
    async fn test_submit_gif_logic_invalid_url() {
        let validator = MockGifValidator { is_valid: false };

        let res = GifSubmission::new(
            Some("ftp://example.com/test.gif".to_string()),
            None,
            &validator,
        )
        .await;

        assert!(res.is_err());
        match res.unwrap_err() {
            CommandError::UrlValidation(UrlValidationError::InvalidScheme) => (),
            _ => panic!("Expected InvalidScheme error"),
        }
    }
}
