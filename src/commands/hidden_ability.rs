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
        Err(why) => return format!("{}\n", why),
    };

    match api_service.get_hidden_ability(&api_name).await {
        Ok(api_output) => format!("{}: {}\n", api_name, api_output),
        Err(why) => format!("{}\n", why),
    }
}

pub async fn get_hidden_abilities(
    pokemon_list: Vec<&str>,
    api_service: &impl PokeAPIService,
) -> String {
    let mut output: String = "".to_string();
    for input_name in pokemon_list {
        output.push_str(&format_hidden_ability(input_name, api_service).await);
    }
    output.to_string()
}
