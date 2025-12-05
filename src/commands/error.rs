use thiserror::Error;

use crate::commands::gotd::UrlValidationError;
use crate::database::DatabaseError;
use crate::services::pokeapi::PokeAPIError;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Invalid option: {0}")]
    InvalidOption(String),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Discord API error: {0}")]
    Discord(#[from] serenity::Error),

    #[error("URL Validation error: {0}")]
    UrlValidation(#[from] UrlValidationError),

    #[error("PokeAPI error: {0}")]
    PokeAPI(#[from] PokeAPIError),

    #[error("{0}")]
    Generic(String),
}

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        CommandError::Generic(s)
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        CommandError::Generic(s.to_string())
    }
}
