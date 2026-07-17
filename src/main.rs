use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serenity::all::{
    Command, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
    Interaction, Ready,
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

/// The two response modes a slash command can use.
///
/// - `Immediate` – respond in one shot via `create_response(Message(...))`.
/// - `Deferred`  – acknowledge first with `Defer`, then follow up with
///   `edit_response(...)` once the slow work is done. The inner `String`
///   is the final message content.
enum CommandResponse {
    Immediate(CreateInteractionResponseMessage),
    Deferred(String),
}

struct Handler {
    is_loop_running: AtomicBool,
    poe_accounts: HashMap<String, String>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let _global_command = Command::set_global_commands(
            &ctx.http,
            vec![
                commands::ping::register(),
                commands::hidden_ability::register(),
                commands::secret::register(),
                commands::poe::register(),
                commands::gotd::register(),
            ],
        )
        .await;

        let _guild_command = serenity::all::GuildId::new(704782281578905670)
            .set_commands(&ctx.http, vec![commands::integration_test::register()])
            .await;

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

                let data = ctx.data.read().await;
                let pool = data
                    .get::<DbPoolWrapper>()
                    .expect("Expected DbPool in TypeMap");
                let config = data
                    .get::<BotConfigWrapper>()
                    .expect("Expected BotConfig in TypeMap");

                // Determine upfront if this command needs a deferred response,
                // so we can acknowledge within Discord's 3-second window before
                // any slow processing begins.
                let is_deferred = matches!(command.data.name.as_str(), "gotd" | "ha");

                if is_deferred {
                    if let Err(why) = command
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Defer(
                                CreateInteractionResponseMessage::new().ephemeral(true),
                            ),
                        )
                        .await
                    {
                        println!("Cannot defer interaction: {}", why);
                        return;
                    }
                }

                // Every arm returns Result<CommandResponse, CommandError>.
                // Deferred commands return Deferred(content); all others return Immediate(msg).
                let response = match command.data.name.as_str() {
                    "ping" => commands::ping::run(&command.data.options)
                        .map(CommandResponse::Immediate),
                    "ha" => commands::hidden_ability::run(&command.data.options)
                        .await
                        .map(CommandResponse::Deferred),
                    "secret" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        commands::secret::run(
                            &command.data.options,
                            &command.user,
                            &db,
                            config.secret_admin_id,
                        ).map(CommandResponse::Immediate)
                    }
                    "poe" => commands::poe::run(&command.data.options, &self.poe_accounts)
                        .map(CommandResponse::Immediate),
                    "gotd" => {
                        let db = BotDatabase::new((*pool).as_ref().clone(), config.secret_admin_id);
                        let gif_directory = format!("{}/gifs", config.data_folder);
                        commands::gotd::run(&command.data, &command.user, &db, &gif_directory)
                            .await
                            .map(CommandResponse::Deferred)
                    }
                    "integration_test" => {
                        commands::integration_test::run(&command.data.options)
                            .map(CommandResponse::Immediate)
                    }
                    _ => Ok(CommandResponse::Immediate(
                        CreateInteractionResponseMessage::new()
                            .content("not implemented :(")
                            .ephemeral(true),
                    )),
                };

                match response {
                    Ok(CommandResponse::Immediate(msg)) => {
                        if let Err(why) = command
                            .create_response(&ctx.http, CreateInteractionResponse::Message(msg))
                            .await
                        {
                            println!("Cannot respond to slash command: {}", why);
                        }
                    }
                    Ok(CommandResponse::Deferred(content)) => {
                        // Defer was already sent above; just edit with the result.
                        if let Err(why) = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new().content(content),
                            )
                            .await
                        {
                            println!("Cannot edit deferred response: {}", why);
                        }
                    }
                    Err(why) => {
                        // Send the error back. Use edit_response if we already deferred,
                        // otherwise use create_response.
                        let error_content = why.to_string();
                        if is_deferred {
                            if let Err(e) = command
                                .edit_response(
                                    &ctx.http,
                                    EditInteractionResponse::new().content(error_content),
                                )
                                .await
                            {
                                println!("Cannot send deferred error response: {}", e);
                            }
                        } else {
                            let msg = CreateInteractionResponseMessage::new()
                                .content(error_content)
                                .ephemeral(true);
                            if let Err(e) = command
                                .create_response(&ctx.http, CreateInteractionResponse::Message(msg))
                                .await
                            {
                                println!("Cannot send error response: {}", e);
                            }
                        }
                    }
                }

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
