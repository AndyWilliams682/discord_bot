use serenity::all::{CommandDataOption, CreateCommand, CreateInteractionResponseMessage};

pub fn run(_options: &[CommandDataOption]) -> CreateInteractionResponseMessage {
    CreateInteractionResponseMessage::new().content(get_response_content())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("A ping command")
}

fn get_response_content() -> String {
    "Hey, I'm alive!".to_string()
}
