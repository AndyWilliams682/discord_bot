use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs};

use serenity::async_trait;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod commands;
mod loops;

struct Handler {
    is_loop_running: AtomicBool,
    config: HashMap<String, String>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let _global_command = Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|command| commands::ping::register(command))
                .create_application_command(|command| commands::hidden_ability::register(command))
                .create_application_command(|command| commands::secret::register(command))
                .create_application_command(|command| commands::poe::register(command))
                .create_application_command(|command| commands::test_feature::register(command))
        })
        .await;

        let loop_ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            loops::status::start(loop_ctx);

            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                "ping" => commands::ping::run(&command.data.options),
                "ha" => commands::hidden_ability::run(&command.data.options).await,
                "secret" => commands::secret::run(&command.data.options),
                "poe" => commands::poe::run(&command.data.options, &self.config),
                "test_feature" => commands::test_feature::run().await,
                _ => "not implemented :(".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                println!("Cannot respond to slash command: {}", why);
            }
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

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
