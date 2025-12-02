use serenity::all::{CreateCommand, CreateCommandOption, CommandOptionType, CommandDataOption, CommandDataOptionValue, CreateInteractionResponseMessage};
use std::collections::HashMap;

pub fn run(options: &[CommandDataOption], config: &HashMap<String, String>) -> CreateInteractionResponseMessage {
    let requested_user = &options
        .get(0)
        .expect("Expected user option")
        .value;

    let content = if let CommandDataOptionValue::User(user_id) = requested_user {
        get_response_content(user_id.get(), config)
    } else {
        "Please provide a valid user".to_string()
    };
    CreateInteractionResponseMessage::new().content(content)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("poe")
        .description("Get a link to the user's poe characters")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "id", "The user to lookup")
                .required(true)
        )
}

pub fn get_response_content(user_id: u64, config: &HashMap<String, String>) -> String {
    if let Some(account) =  config.get(&user_id.to_string()) {
        format!("https://www.pathofexile.com/account/view-profile/{}/characters", account)
    } else {
        "This user does not have an account linked".to_string()
    }
}
