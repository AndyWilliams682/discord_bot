use serenity::all::{CreateCommand, CreateCommandOption, CommandOptionType, CommandDataOption, CommandDataOptionValue};
use std::collections::HashMap;

pub fn run(options: &[CommandDataOption], config: &HashMap<String, String>) -> String {
    let option = &options
        .get(0)
        .expect("Expected user option")
        .value;

    if let CommandDataOptionValue::User(user_id) = option {
        if let Some(account) =  config.get(&user_id.to_string()) {
            format!("https://www.pathofexile.com/account/view-profile/{}/characters", account)
        } else {
            "This user does not have an account linked".to_string()
        }
    } else {
        "Please provide a valid user".to_string()
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("poe")
        .description("Get a link to the user's poe characters")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "id", "The user to lookup")
                .required(true)
        )
}