use crate::commands::error::CommandError;
use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage,
};

pub fn run(
    _options: &[CommandDataOption],
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let success_btn = CreateButton::new("test_button_success")
        .style(ButtonStyle::Success)
        .label("Test Success");

    let danger_btn = CreateButton::new("test_button_danger")
        .style(ButtonStyle::Danger)
        .label("Test Danger");

    let row = CreateActionRow::Buttons(vec![success_btn, danger_btn]);

    Ok(CreateInteractionResponseMessage::new()
        .content("Integration Test: Click a button!")
        .components(vec![row])
        .ephemeral(true))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("integration_test").description("Test command for Private Testing Guild only")
}

pub fn button_handler(custom_id: &str) -> Result<CreateInteractionResponseMessage, CommandError> {
    let content = match custom_id {
        "test_button_success" => "✅ Success button clicked!",
        "test_button_danger" => "⚠️ Danger button clicked!",
        _ => "Unknown button",
    };

    Ok(CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(true))
}
