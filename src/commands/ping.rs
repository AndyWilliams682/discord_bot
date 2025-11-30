use serenity::all::{CreateCommand, CommandDataOption, CreateInteractionResponseMessage};

pub fn run(_options: &[CommandDataOption]) -> CreateInteractionResponseMessage {
    CreateInteractionResponseMessage::new().content("Hey, I'm alive!")
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("A ping command")
}