use regex::Regex;
use serde_json::Value;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum HiddenAbilityError {
    #[error("The network request returned a non-success status code: {0}")]
    NonSuccessStatus(u16),

    #[error("Pokemon not found: {0}")]
    InvalidContentType(String),

    #[error("Pokemon name is not valid for PokeAPI: {0}")]
    InvalidPokeAPIName(String),
}

impl From<reqwest::Error> for HiddenAbilityError {
    fn from(e: reqwest::Error) -> Self {
        HiddenAbilityError::InvalidContentType(e.to_string())
    }
}

pub type HiddenAbilityResult = Result<String, HiddenAbilityError>;

const MIN_CHARS: usize = 3; // Shortest name is "Mew"
const MAX_CHARS: usize = 25; // arbitrary maximum
const NO_HIDDEN_ABILITY: &str = "No Hidden Ability";

#[async_trait]
pub trait PokeAPIService {
    async fn get_hidden_ability(&self, api_name: &str) -> HiddenAbilityResult;
}

pub struct RealPokeAPIService;

impl RealPokeAPIService {
    pub fn new() -> Self {
        Self {}
    }

    fn extract_hidden_ability(parsed_json: &Value) -> String {
        let abilities: &Vec<Value> = parsed_json["abilities"].as_array().unwrap();
        let mut hidden_ability: String = NO_HIDDEN_ABILITY.to_string();
        for ability in abilities {
            if ability["is_hidden"] == true {
                hidden_ability = ability["ability"]["name"].as_str().unwrap().to_string();
            }
        }
        hidden_ability
    }
}

#[async_trait]
impl PokeAPIService for RealPokeAPIService {
    async fn get_hidden_ability(&self, api_name: &str) -> HiddenAbilityResult {
        let url = format!("https://pokeapi.co/api/v2/pokemon/{}", api_name);
        let response = reqwest::get(&url).await?;
        let status = response.status();

        if !status.is_success() {
            return Err(HiddenAbilityError::NonSuccessStatus(status.as_u16()))
        }
        let parsed = response.json::<Value>().await?; // TODO Add this error to the internal error type
        Ok(RealPokeAPIService::extract_hidden_ability(&parsed))
    }
}

pub fn convert_to_pokeapi_name(s: String) -> HiddenAbilityResult {
    let chars_to_null = Regex::new(r"[':.]").unwrap();
    let forbidden_chars = Regex::new(r"[^a-z2-]").unwrap();

    let lowercase_s = s
        .to_lowercase()
        .replace(" ", "-")
        .replace("♀", "f")
        .replace("♂", "m");
    let no_punctuation_s = chars_to_null.replace_all(&lowercase_s, "");

    let is_empty_or_whitespace = no_punctuation_s.trim().is_empty();
    let is_too_short = no_punctuation_s.len() < MIN_CHARS;
    let is_too_long = no_punctuation_s.len() > MAX_CHARS;
    let contains_forbidden_chars = forbidden_chars.is_match(&no_punctuation_s);

    if is_empty_or_whitespace || is_too_short || is_too_long || contains_forbidden_chars {
        Err(HiddenAbilityError::InvalidPokeAPIName(s))
    } else {
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_valid_name_is_accepted() {
        let name = "Jigglypuff from the top".to_string();
        assert_ok!(convert_to_pokeapi_name(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "".to_string();
        assert_err!(convert_to_pokeapi_name(name));
    }

    #[test]
    fn a_name_with_forbidden_characters_is_rejected() {
        let name = "Test/".to_string();
        assert_err!(convert_to_pokeapi_name(name));
    }

    #[test]
    fn a_name_that_is_too_short_is_rejected() {
        let name = "a".repeat(MIN_CHARS - 1).to_string();
        assert_err!(convert_to_pokeapi_name(name));
    }

    #[test]
    fn a_name_that_is_too_long_is_rejected() {
        let name = "a".repeat(MAX_CHARS + 1).to_string();
        assert_err!(convert_to_pokeapi_name(name));
    }
}
