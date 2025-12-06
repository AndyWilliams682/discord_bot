use crate::commands::error::CommandError;
use serenity::all::{
    CommandDataOption, CommandDataOptionValue, CommandOptionType, CreateCommand,
    CreateCommandOption, CreateInteractionResponseMessage,
};
use std::collections::HashMap;

pub fn run(
    options: &[CommandDataOption],
    config: &HashMap<String, String>,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let requested_user = &options.get(0).expect("Expected user option").value;

    let content = if let CommandDataOptionValue::User(user_id) = requested_user {
        get_response_content(user_id.get(), config)
    } else {
        return Err(CommandError::InvalidOption(
            "Please provide a valid user".to_string(),
        ));
    };
    Ok(CreateInteractionResponseMessage::new().content(content))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("poe")
        .description("Get a link to the user's poe characters")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "id", "The user to lookup")
                .required(true),
        )
}

fn get_response_content(user_id: u64, config: &HashMap<String, String>) -> String {
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
        config.insert("123".to_string(), "exile_account".to_string());

        let result = get_response_content(123, &config);
        assert_eq!(
            result,
            "https://www.pathofexile.com/account/view-profile/exile_account/characters"
        );
    }

    #[test]
    fn test_get_response_content_not_found() {
        let config = HashMap::new();
        let result = get_response_content(456, &config);
        assert_eq!(result, "This user does not have an account linked");
    }
}
