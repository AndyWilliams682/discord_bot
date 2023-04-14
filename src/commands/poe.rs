use serenity::builder::CreateApplicationCommand;
use serenity::model::application::command::CommandOptionType;
use serenity::model::application::interaction::application_command::{CommandDataOption, CommandDataOptionValue};
use std::collections::HashMap;

pub fn run(options: &[CommandDataOption], config: &HashMap<String, String>) -> String {
    let option = options
        .get(0)
        .expect("Expected user option")
        .resolved
        .as_ref()
        .expect("Expected user object");

    if let CommandDataOptionValue::User(user, _member) = option {
        if let Some(account) =  config.get(&user.id.to_string()) {
            format!("https://www.pathofexile.com/account/view-profile/{}/characters", account)
        } else {
            "This user does not have an account linked".to_string()
        }
    } else {
        "Please provide a valid user".to_string()
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("poe").description("Get a link to the user's poe characters").create_option(|option| {
        option
            .name("id")
            .description("The user to lookup")
            .kind(CommandOptionType::User)
            .required(true)
    })
}