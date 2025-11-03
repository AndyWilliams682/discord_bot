use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::interaction::application_command::CommandDataOption;

pub fn run(options: &[CommandDataOption]) -> String {
    "Hey this command worked".to_string()
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("secret").description("A secret message command").create_option(|option| {
        option
            .name("id")
            .description("The user to message via the bot")
            .kind(CommandOptionType::User)
            .required(true)
    }).create_option(|option| {
        option
            .name("message")
            .description("The message to send")
            .kind(CommandOptionType::String)
            .required(true)
    })
}
