use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage,
};

use crate::commands::error::CommandError;
use crate::commands::hidden_ability::get_hidden_abilities;
use crate::services::pokeapi::RealPokeAPIService;

pub fn run(
    _options: &[CommandDataOption],
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let success_btn = CreateButton::new("test_ha_success")
        .style(ButtonStyle::Success)
        .label("HA Success Test");

    let danger_btn = CreateButton::new("test_ha_error")
        .style(ButtonStyle::Danger)
        .label("HA Error Test");

    let row = CreateActionRow::Buttons(vec![success_btn, danger_btn]);

    Ok(CreateInteractionResponseMessage::new()
        .content("Integration Test: Click a button!")
        .components(vec![row])
        .ephemeral(true))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("integration_test")
        .description("Test command for Private Testing Guild only")
}

fn verify_output(content: &str, output: String) -> String {
    let test_result = match content {
        "test_ha_success" => output == "porygon: analytic\nunown: No hidden ability",
        "test_ha_error" => {
            output == "missingno: Non-success status code: 404\na: Name is not valid for PokeAPI"
        }
        _ => false,
    };
    match test_result {
        true => format!("{}: Test Passed", content),
        false => format!("{}: Test Failed: {}", content, output),
    }
}

pub async fn button_handler(
    custom_id: &str,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let content = match custom_id {
        "test_ha_success" => {
            get_hidden_abilities(vec!["porygon", "unown"], &RealPokeAPIService::new()).await
        }
        "test_ha_error" => {
            get_hidden_abilities(vec!["missingno", "a"], &RealPokeAPIService::new()).await
        }
        _ => "Unknown button".to_string(),
    };

    Ok(CreateInteractionResponseMessage::new()
        .content(verify_output(custom_id, content))
        .ephemeral(true))
}
