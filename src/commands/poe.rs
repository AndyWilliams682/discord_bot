use crate::commands::{error::CommandError, BotCommand, CommandContext, CommandResponse};
use serenity::all::{
    CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType,
    CreateCommand, CreateCommandOption,
};
use serenity::async_trait;
use std::collections::HashMap;

pub struct PoeCommand;

#[async_trait]
impl BotCommand for PoeCommand {
    fn name(&self) -> &'static str {
        "poe"
    }

    fn register(&self) -> CreateCommand {
        register()
    }

    async fn execute(
        &self,
        interaction: &CommandInteraction,
        context: CommandContext<'_>,
    ) -> Result<CommandResponse, CommandError> {
        run(&interaction.data.options, context.poe_accounts)
    }
}

pub fn run(
    options: &[CommandDataOption],
    config: &HashMap<String, String>,
) -> Result<CommandResponse, CommandError> {
    let requested_user = &options.get(0).expect("Expected user option").value;

    let content = if let CommandDataOptionValue::User(user_id) = requested_user {
        get_response_content(user_id.get(), config)
    } else {
        return Err(CommandError::InvalidOption(
            "Please provide a valid user".to_string(),
        ));
    };
    Ok(CommandResponse::new().content(content))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("poe")
        .description("Get a link to the user's poe characters")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "id", "The user to lookup")
                .required(true),
        )
}

pub fn get_response_content(user_id: u64, config: &HashMap<String, String>) -> String {
    if let Some(account) = config.get(&user_id.to_string()) {
        format!(
            "https://www.pathofexile.com/account/view-profile/{}/characters",
            account
        )
    } else {
        "This user does not have an account linked".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_response_content_found() {
        let mut config = HashMap::new();
        config.insert("12345".to_string(), "MyAccountName".to_string());
        let res = get_response_content(12345, &config);
        assert_eq!(
            res,
            "https://www.pathofexile.com/account/view-profile/MyAccountName/characters"
        );
    }

    #[test]
    fn test_get_response_content_not_found() {
        let config = HashMap::new();
        let res = get_response_content(12345, &config);
        assert_eq!(res, "This user does not have an account linked");
    }
}
