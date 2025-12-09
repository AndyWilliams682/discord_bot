use std::collections::HashMap;

use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage,
};

use crate::commands::error::CommandError;
use crate::commands::gotd::GotdTrait;
use crate::commands::hidden_ability::get_hidden_abilities;
use crate::commands::poe;
use crate::database::BotDatabase;
use crate::services::pokeapi::RealPokeAPIService;

pub fn run(
    _options: &[CommandDataOption],
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let ha_success_btn = CreateButton::new("test_ha_success")
        .style(ButtonStyle::Success)
        .label("HA Success Test");

    let ha_error_btn = CreateButton::new("test_ha_error")
        .style(ButtonStyle::Danger)
        .label("HA Error Test");

    let poe_success_btn = CreateButton::new("test_poe_success")
        .style(ButtonStyle::Success)
        .label("POE Success Test");

    let poe_error_btn = CreateButton::new("test_poe_error")
        .style(ButtonStyle::Danger)
        .label("POE Error Test");

    let db_error_btn = CreateButton::new("test_db_error")
        .style(ButtonStyle::Danger)
        .label("DB Error Test");

    let row = CreateActionRow::Buttons(vec![
        ha_success_btn,
        ha_error_btn,
        poe_success_btn,
        poe_error_btn,
        db_error_btn,
    ]);

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
        "test_ha_success" => output == "porygon: analytic\n",
        "test_ha_error" => output == "missingno: Non-success status code: 404\n",
        "test_poe_success" => {
            output == "https://www.pathofexile.com/account/view-profile/flyingrhino/characters"
        }
        "test_poe_error" => output == "This user does not have an account linked",
        "test_db_error" => {
            output == "Database error: Failed to run query: UNIQUE constraint failed: gifs.url"
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
    config: &HashMap<String, String>,
    db: &BotDatabase,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let content = match custom_id {
        "test_ha_success" => {
            get_hidden_abilities(vec!["porygon"], &RealPokeAPIService::new()).await
        }
        "test_ha_error" => {
            get_hidden_abilities(vec!["missingno"], &RealPokeAPIService::new()).await
        }
        "test_poe_success" => poe::get_response_content(255117530253754378, config),
        "test_poe_error" => poe::get_response_content(255117530253754377, config),
        "test_db_error" => match db
            .insert_gif(0, "https://images-ext-1.discordapp.net/external/ZPuxUmy38GY5RVuMbVVXjp4tmKYMJ65CPP0JU8G_7TI/https/media.tenor.com/OMeh_em3sH0AAAPo/jonah-jameson-mouth.mp4".to_string())
            .await
        {
            Ok(_) => "THIS SHOULD'VE FAILED".to_string(),
            Err(e) => format!("Database error: {}", e),
        },
        _ => "Unknown button".to_string(),
    };

    Ok(CreateInteractionResponseMessage::new()
        .content(verify_output(custom_id, content))
        .ephemeral(true))
}
