use crate::commands::{error::CommandError, BotCommand, CommandContext, CommandResponse};
use serenity::all::{CommandDataOption, CommandInteraction, CreateCommand};
use serenity::async_trait;

pub struct PingCommand;

#[async_trait]
impl BotCommand for PingCommand {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn register(&self) -> CreateCommand {
        register()
    }

    async fn execute(
        &self,
        interaction: &CommandInteraction,
        _context: CommandContext<'_>,
    ) -> Result<CommandResponse, CommandError> {
        run(&interaction.data.options)
    }
}

pub fn run(_options: &[CommandDataOption]) -> Result<CommandResponse, CommandError> {
    Ok(CommandResponse::new().content(get_response_content()))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("A ping command")
}

fn get_response_content() -> String {
    "Hey, I'm alive!".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_response_content() {
        assert_eq!(get_response_content(), "Hey, I'm alive!");
    }

    #[test]
    fn test_run() {
        let result = run(&[]);
        assert!(result.is_ok());
    }
}
