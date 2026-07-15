pub mod error;
pub mod gotd;
pub mod hidden_ability;
pub mod integration_test;
pub mod ping;
pub mod poe;
pub mod secret;

use std::collections::HashMap;

use crate::config::BotConfig;
use crate::database::DbPool;
use error::CommandError;
use serenity::all::{CommandInteraction, CreateCommand};
use serenity::all::{CreateActionRow, CreateInteractionResponseMessage, EditInteractionResponse};
use serenity::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRegistration {
    Global,
    Guild(u64),
}

#[derive(Clone, Copy)]
pub struct CommandContext<'a> {
    pub pool: &'a DbPool,
    pub config: &'a BotConfig,
    pub poe_accounts: &'a HashMap<String, String>,
}

#[async_trait]
pub trait BotCommand: Send + Sync {
    fn name(&self) -> &'static str;

    fn registration(&self) -> CommandRegistration {
        CommandRegistration::Global
    }

    fn should_defer(&self) -> bool {
        false
    }

    fn register(&self) -> CreateCommand;

    async fn execute(
        &self,
        interaction: &CommandInteraction,
        context: CommandContext<'_>,
    ) -> Result<CommandResponse, CommandError>;
}

#[derive(Debug, Clone, Default)]
pub struct CommandResponse {
    content: String,
    ephemeral: bool,
    components: Vec<CreateActionRow>,
}

impl CommandResponse {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = ephemeral;
        self
    }

    pub fn components(mut self, components: Vec<CreateActionRow>) -> Self {
        self.components = components;
        self
    }

    pub fn into_initial_response(self) -> CreateInteractionResponseMessage {
        let mut response = CreateInteractionResponseMessage::new()
            .content(self.content)
            .ephemeral(self.ephemeral);

        if !self.components.is_empty() {
            response = response.components(self.components);
        }

        response
    }

    pub fn into_edit_response(self) -> EditInteractionResponse {
        let mut response = EditInteractionResponse::new().content(self.content);

        if !self.components.is_empty() {
            response = response.components(self.components);
        }

        response
    }
}

pub fn all() -> Vec<Box<dyn BotCommand>> {
    vec![
        Box::new(ping::PingCommand),
        Box::new(hidden_ability::HiddenAbilityCommand),
        Box::new(secret::SecretCommand),
        Box::new(poe::PoeCommand),
        Box::new(gotd::GotdCommand),
        Box::new(integration_test::IntegrationTestCommand),
    ]
}
