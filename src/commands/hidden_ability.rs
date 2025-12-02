use std::fmt;
use regex::Regex;
use serenity::all::{CreateCommand, CreateCommandOption, CommandOptionType, CommandDataOptionValue, CommandDataOption, CreateInteractionResponseMessage};
use serde_json::Value;

const MIN_CHARS: usize = 3; // Shortest name is "Mew"
const MAX_CHARS: usize = 25; // arbitrary maximum
const NO_HIDDEN_ABILITY: &str = "No Hidden Ability";


use async_trait::async_trait;

#[async_trait]
pub trait PokeAPIService {
    async fn get_hidden_ability(&self, api_name: &str) -> Result<String, String>;
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
    async fn get_hidden_ability(&self, api_name: &str) -> Result<String, String> {
        let url = format!("https://pokeapi.co/api/v2/pokemon/{}", api_name);
        let response = match reqwest::get(&url).await {
            Ok(res) => res,
            Err(e) => return Err(format!("{:?}", e))
        };

        match response.status() {
            reqwest::StatusCode::OK => {
                match response.json::<Value>().await {
                    Ok(parsed) => Ok(RealPokeAPIService::extract_hidden_ability(&parsed)),
                    Err(e) => Err(format!("{:?}", e))
                }
            }
            status_code => Err(format!("{:?}", status_code))
        }
    }
}


pub async fn get_hidden_abilities(pokemon_list: Vec<&str>, api_service: &impl PokeAPIService) -> String {
    let mut output: String = "".to_string();
    for input_name in pokemon_list {
        let api_name = match PokeAPIName::parse(input_name.to_string()) {
            Ok(api_name) => api_name,
            Err(why) => {
                output.push_str(&format!("{}: {}\n", input_name, why));
                continue
            }
        };
        match api_service.get_hidden_ability(api_name.as_ref()).await {
            Ok(api_output) => output.push_str(&format!("{}: {}\n", api_name, &api_output)),
            Err(err) => output.push_str(&format!("{}: {:?}\n", api_name, err))
        }
    }
    output.to_string()
}


pub async fn run(options: &[CommandDataOption]) -> CreateInteractionResponseMessage {
    if let CommandDataOptionValue::String(raw_input) = &options
        .get(0)
        .expect("Expected string option")
        .value {
            let pokemon_list = raw_input.split(", ").collect::<Vec<&str>>();
            let api_service = RealPokeAPIService::new();
            let mut content = get_hidden_abilities(pokemon_list, &api_service).await;
            if content.len() == 0 {
                content = format!("Your input \"{}\" has no valid pokemon", raw_input)
            }
            CreateInteractionResponseMessage::new().content(content)
        } else {
            CreateInteractionResponseMessage::new().content("How did you input a non-string?")
        }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ha")
        .description("Outputs the hidden abilities of all pokemon provided")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "pokemon_list", "List of Pokemon (eg: unown, vulpix-alola, nidoran-f, falinks)")
                .required(true)
        )
}

#[derive(Debug)]
pub struct PokeAPIName(String);

impl PokeAPIName {
    pub fn parse(s: String) -> Result<PokeAPIName, String> {
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
            Err(format!("{} is not a valid PokeAPI Name\n", s))
        } else {
            Ok(Self(s))
        }
    }
}

impl AsRef<str> for PokeAPIName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PokeAPIName {
    fn fmt (&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use serenity::model::application::interaction::application_command::CommandDataOptionValue;
    use crate::commands::hidden_ability;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_valid_name_is_accepted() {
        let name = "Jigglypuff from the top".to_string();
        assert_ok!(hidden_ability::PokeAPIName::parse(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "".to_string();
        assert_err!(hidden_ability::PokeAPIName::parse(name));
    }

    #[test]
    fn a_name_with_forbidden_characters_is_rejected() {
        let name = "Test/".to_string();
        assert_err!(hidden_ability::PokeAPIName::parse(name));
    }

    #[test]
    fn a_name_that_is_too_short_is_rejected() {
        let name = "a".repeat(hidden_ability::MIN_CHARS - 1).to_string();
        assert_err!(hidden_ability::PokeAPIName::parse(name));
    }

    #[test]
    fn a_name_that_is_too_long_is_rejected() {
        let name = "a".repeat(hidden_ability::MAX_CHARS + 1).to_string();
        assert_err!(hidden_ability::PokeAPIName::parse(name));
    }

    #[tokio::test]
    async fn porygons_ha_is_analytic() {
        let porygon_option = CommandDataOptionValue::String("porygon".to_string());
        assert_eq!("porygon: analytic\n".to_string(),
                   hidden_ability::get_pokemon_ha_from_api(&porygon_option).await)
    }

    #[tokio::test]
    async fn porygon_family_ha_is_analytic() {
        let porygon_family_option = CommandDataOptionValue::String("porygon, porygon2, porygon-z".to_string());
        assert_eq!("porygon: analytic\nporygon2: analytic\nporygon-z: analytic\n",
                   hidden_ability::get_pokemon_ha_from_api(&porygon_family_option).await)
    }

    #[tokio::test]
    async fn rotom_has_no_ha() {
        let rotom_option = CommandDataOptionValue::String("rotom".to_string());
        assert_eq!(format!("rotom: {}\n", hidden_ability::NO_HIDDEN_ABILITY),
                   hidden_ability::get_pokemon_ha_from_api(&rotom_option).await)
    }

    #[tokio::test]
    async fn golem_alola_ha_is_galvanize() {
        let golem_alola_option = CommandDataOptionValue::String("golem-alola".to_string());
        assert_eq!("golem-alola: galvanize\n",
                   hidden_ability::get_pokemon_ha_from_api(&golem_alola_option).await)
    }

    #[tokio::test]
    async fn invalid_name_is_rejected() {
        let invalid_option = CommandDataOptionValue::String("./:-".to_string());
        assert_eq!("./:- is not a valid PokeAPI Name\n",
                   hidden_ability::get_pokemon_ha_from_api(&invalid_option).await)
    }

    #[tokio::test]
    async fn bad_name_returns_404() {
        let bad_name_option = CommandDataOptionValue::String("MissingNo.".to_string());
        assert_eq!("MissingNo.: 404 Not Found\n",
                   hidden_ability::get_pokemon_ha_from_api(&bad_name_option).await)
    }
}
