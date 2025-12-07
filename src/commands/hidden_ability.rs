use crate::commands::error::CommandError;
use crate::services::pokeapi::{convert_to_pokeapi_name, PokeAPIService, RealPokeAPIService};
use serenity::all::{
    CommandDataOption, CommandDataOptionValue, CommandOptionType, CreateCommand,
    CreateCommandOption, CreateInteractionResponseMessage,
};

pub async fn run(
    options: &[CommandDataOption],
) -> Result<CreateInteractionResponseMessage, CommandError> {
    if let CommandDataOptionValue::String(raw_input) =
        &options.get(0).expect("Expected string option").value
    {
        let pokemon_list = raw_input.split(", ").collect::<Vec<&str>>();
        let api_service = RealPokeAPIService::new();
        let mut content = get_hidden_abilities(pokemon_list, &api_service).await;
        if content.len() == 0 {
            content = format!("Your input \"{}\" has no valid pokemon", raw_input)
        }
        Ok(CreateInteractionResponseMessage::new().content(content))
    } else {
        Err(CommandError::InvalidOption(
            "How did you input a non-string?".to_string(),
        ))
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ha")
        .description("Outputs the hidden abilities of all pokemon provided")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "pokemon_list",
                "List of Pokemon (eg: unown, vulpix-alola, nidoran-f, falinks)",
            )
            .required(true),
        )
}

async fn format_hidden_ability(input_name: &str, api_service: &impl PokeAPIService) -> String {
    let api_name = match convert_to_pokeapi_name(input_name.to_string()) {
        Ok(name) => name,
        Err(why) => return why.to_string(),
    };

    match api_service.get_hidden_ability(&api_name).await {
        Ok(api_output) => format!("{}: {}\n", api_name, api_output),
        Err(why) => why.to_string(),
    }
}

async fn get_hidden_abilities(
    pokemon_list: Vec<&str>,
    api_service: &impl PokeAPIService,
) -> String {
    let mut output: String = "".to_string();
    for input_name in pokemon_list {
        output.push_str(&format_hidden_ability(input_name, api_service).await);
    }
    output.to_string()
}

#[cfg(test)]
mod tests {
    use crate::commands::hidden_ability;
    use crate::services::pokeapi::{
        PokeAPIError, PokeAPIResult, PokeAPIService, NO_HIDDEN_ABILITY,
    };
    use async_trait::async_trait;

    struct MockPokeAPIService {
        response: PokeAPIResult,
    }

    #[async_trait]
    impl PokeAPIService for MockPokeAPIService {
        async fn get_hidden_ability(&self, api_name: &str) -> PokeAPIResult {
            match api_name {
                "porygon" | "porygon2" | "porygon-z" => Ok("analytic".to_string()),
                "rotom" => Ok(NO_HIDDEN_ABILITY.to_string()),
                "golem-alola" => Ok("galvanize".to_string()),
                "missingno" => Err(PokeAPIError::NonSuccessStatus("missingno", 404)),
                _ => self.response.clone(),
            }
        }
    }

    #[tokio::test]
    async fn porygons_ha_is_analytic() {
        let service = MockPokeAPIService {
            response: Ok("".to_string()),
        };
        let result = hidden_ability::get_hidden_abilities(vec!["porygon"], &service).await;
        assert_eq!("porygon: analytic\n", result);
    }

    #[tokio::test]
    async fn porygon_family_ha_is_analytic() {
        let service = MockPokeAPIService {
            response: Ok("".to_string()),
        };
        let result = hidden_ability::get_hidden_abilities(
            vec!["porygon", "porygon2", "porygon-z"],
            &service,
        )
        .await;
        assert_eq!(
            "porygon: analytic\nporygon2: analytic\nporygon-z: analytic\n",
            result
        );
    }

    #[tokio::test]
    async fn rotom_has_no_ha() {
        let service = MockPokeAPIService {
            response: Ok("".to_string()),
        };
        let result = hidden_ability::get_hidden_abilities(vec!["rotom"], &service).await;
        assert_eq!(format!("rotom: {}\n", NO_HIDDEN_ABILITY), result);
    }

    #[tokio::test]
    async fn golem_alola_ha_is_galvanize() {
        let service = MockPokeAPIService {
            response: Ok("".to_string()),
        };
        let result = hidden_ability::get_hidden_abilities(vec!["golem-alola"], &service).await;
        assert_eq!("golem-alola: galvanize\n", result);
    }

    #[tokio::test]
    async fn invalid_name_is_rejected() {
        let service = MockPokeAPIService {
            response: Ok("".to_string()),
        };
        let result = hidden_ability::get_hidden_abilities(vec!["./:-"], &service).await;
        assert!(result.contains("Name is not valid for PokeAPI"));
    }

    #[tokio::test]
    async fn bad_name_returns_404() {
        let service = MockPokeAPIService {
            response: Err(PokeAPIError::NonSuccessStatus("missingno", 404)),
        };
        let result = hidden_ability::get_hidden_abilities(vec!["MissingNo."], &service).await;
        assert!(result.contains("non-success status code: 404"));
    }
}
