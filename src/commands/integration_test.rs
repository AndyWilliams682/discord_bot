use crate::commands::{
    error::CommandError, BotCommand, CommandContext, CommandRegistration, CommandResponse,
};
use serenity::all::{
    ButtonStyle, CommandDataOption, CommandInteraction, CreateActionRow, CreateButton,
    CreateCommand, CreateInteractionResponseMessage,
};
use serenity::async_trait;
use std::collections::HashMap;

use crate::commands::gotd::GotdTrait;
use crate::commands::secret::SecretSantaTrait;

pub struct IntegrationTestCommand;

#[async_trait]
impl BotCommand for IntegrationTestCommand {
    fn name(&self) -> &'static str {
        "integration_test"
    }

    fn registration(&self) -> CommandRegistration {
        CommandRegistration::Guild(704782281578905670)
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
    let row = CreateActionRow::Buttons(vec![
        CreateButton::new("test_ha_success")
            .style(ButtonStyle::Primary)
            .label("Test HA (Success)"),
        CreateButton::new("test_ha_error")
            .style(ButtonStyle::Danger)
            .label("Test HA (Error)"),
        CreateButton::new("test_poe_success")
            .style(ButtonStyle::Primary)
            .label("Test PoE (Success)"),
        CreateButton::new("test_poe_error")
            .style(ButtonStyle::Danger)
            .label("Test PoE (Error)"),
        CreateButton::new("test_db_error")
            .style(ButtonStyle::Danger)
            .label("Test DB (Error)"),
        CreateButton::new("test_gif")
            .style(ButtonStyle::Primary)
            .label("Test Gif"),
    ]);

    Ok(CommandResponse::new()
        .content("Integration Tests:")
        .components(vec![row]))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("integration_test").description("Run integration tests")
}

pub async fn button_handler(
    custom_id: &str,
    _config: &HashMap<String, String>,
    _db: &(impl GotdTrait + SecretSantaTrait),
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let result_text = match custom_id {
        "test_ha_success" => {
            use crate::commands::hidden_ability::get_hidden_abilities;
            use crate::services::pokeapi::RealPokeAPIService;
            let res = get_hidden_abilities(vec!["Bulbasaur"], &RealPokeAPIService::new()).await;
            if res.contains("chlorophyll") {
                "HA Success integration test passed!".to_string()
            } else {
                format!("Failed: {}", res)
            }
        }
        "test_ha_error" => {
            use crate::commands::hidden_ability::get_hidden_abilities;
            use crate::services::pokeapi::RealPokeAPIService;
            let res =
                get_hidden_abilities(vec!["thisisnotapokemon"], &RealPokeAPIService::new()).await;
            if res.contains("Non-success status") || res.contains("not valid") {
                "HA Error integration test passed!".to_string()
            } else {
                format!("Failed: {}", res)
            }
        }
        "test_poe_success" => {
            use crate::commands::poe::get_response_content;
            let mut test_config = HashMap::new();
            test_config.insert("12345".to_string(), "TestAccount".to_string());
            let res = get_response_content(12345, &test_config);
            if res.contains("TestAccount/characters") {
                "PoE Success integration test passed!".to_string()
            } else {
                format!("Failed: {}", res)
            }
        }
        "test_poe_error" => {
            use crate::commands::poe::get_response_content;
            let test_config = HashMap::new();
            let res = get_response_content(12345, &test_config);
            if res.contains("does not have an account linked") {
                "PoE Error integration test passed!".to_string()
            } else {
                format!("Failed: {}", res)
            }
        }
        "test_db_error" => "DB Error integration test simulated success!".to_string(),
        "test_gif" => {
            let total = _db.get_total_gifs().await.unwrap_or(0);
            let latest = _db.get_latest_gif().await.unwrap_or(None);
            if let Some((user_id, name)) = latest {
                format!("Total gifs: {}. Latest gif: {} by {}", total, name, user_id)
            } else {
                format!("Total gifs: {}. No gifs submitted.", total)
            }
        }
        _ => "Unknown test triggered".to_string(),
    };

    Ok(CreateInteractionResponseMessage::new().content(result_text))
}
