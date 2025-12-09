use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs};

use serenity::all::{
    Command, CreateInteractionResponse, CreateInteractionResponseMessage, Interaction, Ready,
};
use serenity::async_trait;
use serenity::prelude::*;

mod commands;
mod database;
mod loops;
mod services;

use database::{establish_connection, BotDatabase, DbPoolWrapper};

const DATABASE_LOCATION: &str = "/usr/local/bin/data/mtg_secret_santa.bin";

struct Handler {
    is_loop_running: AtomicBool,
    config: HashMap<String, String>,
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

            loops::status::start(loop_ctx.clone());
            let db = BotDatabase::new((*pool).clone());
            loops::gotd_loop::start(loop_ctx.clone(), db);
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

                let response = match command.data.name.as_str() {
                    "ping" => commands::ping::run(&command.data.options),
                    "ha" => commands::hidden_ability::run(&command.data.options).await,
                    "secret" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::secret::run(&command.data.options, &command.user, &db)
                    }
                    "poe" => commands::poe::run(&command.data.options, &self.config),
                    "gotd" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::gotd::run(&command.data.options, &command.user, &db).await
                    }
                    "integration_test" => commands::integration_test::run(&command.data.options),
                    _ => Ok(CreateInteractionResponseMessage::new()
                        .content("not implemented :(")
                        .ephemeral(true)),
                };

                let response = match response {
                    Ok(data) => data,
                    Err(why) => CreateInteractionResponseMessage::new()
                        .content(why.to_string())
                        .ephemeral(true),
                };

                if let Err(why) = command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(response))
                    .await
                {
                    println!("Cannot respond to slash command: {}", why);
                }
            }
            Interaction::Component(component) => {
                let data = ctx.data.read().await;
                let pool = data
                    .get::<DbPoolWrapper>()
                    .expect("Expected DbPool in TypeMap");

                let response = match component.data.custom_id.as_str() {
                    "start_new_event" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::secret::start_new_event_interaction(&db).await
                    }
                    "draw_names" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::secret::draw_names_interaction(&ctx, db).await
                    }
                    "toggle_event_participation" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::secret::toggle_event_participation_interaction(
                            &component.user,
                            &db,
                        )
                    }
                    "test_ha_success" | "test_ha_error" | "test_poe_success" | "test_poe_error"
                    | "test_db_error" => {
                        let db = BotDatabase::new((*pool).as_ref().clone());
                        commands::integration_test::button_handler(
                            &component.data.custom_id,
                            &self.config,
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
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // If commands need to be removed
    // use serenity::http::client::Http;
    // let http_client = Http::new_with_application_id(&token, 704782601273213079);
    // let delete_command = http_client.delete_guild_application_command(323928878420590592, 1049455263440191528).await;
    // println!("{:?}", delete_command);

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false),
            config: {
                let config_raw =
                    fs::read_to_string(env::current_dir().unwrap().join("config.json"))
                        .expect("Unable to read config");
                serde_json::from_str(&config_raw).unwrap()
            },
        })
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        let db_pool = establish_connection(DATABASE_LOCATION);
        data.insert::<DbPoolWrapper>(Arc::new(db_pool));
    }

    // Finally, start a single shard, and start listening to events.
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
