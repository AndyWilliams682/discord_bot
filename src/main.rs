use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serenity::all::{
    Command, CommandInteraction, CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage, Interaction, Ready,
};
use serenity::async_trait;
use serenity::prelude::*;

mod commands;
mod config;
mod database;
mod loops;
mod services;

use config::{BotConfig, BotConfigWrapper};
use database::{establish_connection, BotDatabase, DbPoolWrapper};

struct Handler {
    is_loop_running: AtomicBool,
    poe_accounts: HashMap<String, String>,
}

async fn send_command_response(
    ctx: &Context,
    command: &CommandInteraction,
    response: commands::CommandResponse,
    deferred: bool,
) {
    let result = if deferred {
        command
            .edit_response(&ctx.http, response.into_edit_response())
            .await
            .map(|_| ())
    } else {
        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(response.into_initial_response()),
            )
            .await
    };

    if let Err(why) = result {
        println!("Cannot respond to slash command: {}", why);
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let registered_commands = commands::all();
        let mut global_commands = Vec::new();
        let mut guild_commands: HashMap<u64, Vec<CreateCommand>> = HashMap::new();

        for command in registered_commands.iter() {
            match command.registration() {
                commands::CommandRegistration::Global => global_commands.push(command.register()),
                commands::CommandRegistration::Guild(guild_id) => guild_commands
                    .entry(guild_id)
                    .or_default()
                    .push(command.register()),
            }
        }

        let _global_command = Command::set_global_commands(&ctx.http, global_commands).await;

        for (guild_id, commands) in guild_commands {
            let _guild_command = serenity::all::GuildId::new(guild_id)
                .set_commands(&ctx.http, commands)
                .await;
        }

        let loop_ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let data = loop_ctx.data.read().await;
            let pool = data
                .get::<DbPoolWrapper>()
                .expect("Expected DbPool in TypeMap")
                .clone();
            let config = data
                .get::<BotConfigWrapper>()
                .expect("Expected BotConfig in TypeMap")
                .clone();

            loops::status::start(loop_ctx.clone(), config.clone());
            let db = BotDatabase::new((*pool).clone(), config.secret_admin_id);
            loops::gotd_loop::start(loop_ctx.clone(), db, config);
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                println!("Received command interaction: {:#?}", command);

                let (pool, config) = {
                    let data = ctx.data.read().await;
                    let pool = data
                        .get::<DbPoolWrapper>()
                        .expect("Expected DbPool in TypeMap")
                        .clone();
                    let config = data
                        .get::<BotConfigWrapper>()
                        .expect("Expected BotConfig in TypeMap")
                        .clone();
                    (pool, config)
                };

                let registered_commands = commands::all();
                let bot_command = registered_commands
                    .iter()
                    .find(|candidate| candidate.name() == command.data.name.as_str());

                let Some(bot_command) = bot_command else {
                    let response = commands::CommandResponse::new()
                        .content("not implemented :(")
                        .ephemeral(true);
                    send_command_response(&ctx, &command, response, false).await;
                    return;
                };

                let deferred = bot_command.should_defer();
                if deferred {
                    if let Err(why) = command
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Defer(
                                CreateInteractionResponseMessage::new().ephemeral(true),
                            ),
                        )
                        .await
                    {
                        println!("Cannot defer slash command: {}", why);
                        return;
                    }
                }

                let command_context = commands::CommandContext {
                    pool: pool.as_ref(),
                    config: config.as_ref(),
                    poe_accounts: &self.poe_accounts,
                };

                let response = bot_command.execute(&command, command_context).await;

                let response = match response {
                    Ok(data) => data,
                    Err(why) => commands::CommandResponse::new()
                        .content(why.to_string())
                        .ephemeral(true),
                };

                send_command_response(&ctx, &command, response, deferred).await;
            }
            Interaction::Component(component) => {
                let data = ctx.data.read().await;
                let pool = data
                    .get::<DbPoolWrapper>()
                    .expect("Expected DbPool in TypeMap");
                let config = data
                    .get::<BotConfigWrapper>()
                    .expect("Expected BotConfig in TypeMap");

                let response = match component.data.custom_id.as_str() {
                    "start_new_event" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        commands::secret::start_new_event_interaction(&db).await
                    }
                    "draw_names" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        commands::secret::draw_names_interaction(&ctx, db).await
                    }
                    "toggle_event_participation" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        commands::secret::toggle_event_participation_interaction(
                            &component.user,
                            &db,
                        )
                    }
                    "test_ha_success" | "test_ha_error" | "test_poe_success" | "test_poe_error"
                    | "test_db_error" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        commands::integration_test::button_handler(
                            &component.data.custom_id,
                            &self.poe_accounts,
                            &db,
                        )
                        .await
                    }
                    _ => Ok(CreateInteractionResponseMessage::new()
                        .content("How did you even invoke this?")
                        .ephemeral(true)),
                };

                let response = match response {
                    Ok(data) => data,
                    Err(why) => CreateInteractionResponseMessage::new()
                        .content(why.to_string())
                        .ephemeral(true),
                };

                // Respond to the button press interaction
                if let Err(why) = component
                    .create_response(&ctx.http, CreateInteractionResponse::Message(response))
                    .await
                {
                    println!("Cannot respond to secret button press: {}", why);
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Arc::new(BotConfig::load().unwrap_or_else(|e| {
        eprintln!("Configuration Error: {}", e);
        std::process::exit(1);
    }));

    
    
    // If commands need to be removed
    // use serenity::http::client::Http;
    // let http_client = Http::new_with_application_id(&token, 704782601273213079);
    // let delete_command = http_client.delete_guild_application_command(323928878420590592, 1049455263440191528).await;
    // println!("{:?}", delete_command);
    
    // Build our client.
    let mut client = Client::builder(&config.discord_token, GatewayIntents::empty())
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false),
            poe_accounts: config.poe_accounts.clone(),
        })
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        let db_pool = establish_connection(
            env::current_dir()
                .unwrap()
                .join(&config.data_folder)
                .join(format!("{}.bin", config.database_name))
                .to_str()
                .unwrap(),
        );
        let db = BotDatabase::new(db_pool.clone(), config.secret_admin_id);
        db.initialize()
            .expect("Failed to initialize database schema");
        data.insert::<DbPoolWrapper>(Arc::new(db_pool));
        data.insert::<BotConfigWrapper>(config.clone());
    }

    // Finally, start a single shard, and start listening to events.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
