use std::env;

use serenity::async_trait;
use serenity::model::application::command::{CommandOptionType};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;

mod commands;
pub use crate::commands::hidden_ability;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = GuildId(
            env::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let _commands = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command.name("ping").description("A ping command")
                })
                .create_application_command(|command| {
                    command
                        .name("ha")
                        .description("Outputs the hidden abilities of all pokemon provided")
                        .create_option(|option| {
                            option
                                .name("pokemon_list")
                                .description(
                                    "List of Pokemon (eg: unown, vulpix-alola, nidoran-f, falinks)",
                                )
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
        })
        .await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                "ping" => "Hey, I'm alive!".to_string(),
                "ha" => {
                    let options = command
                        .data
                        .options
                        .get(0)
                        .expect("Expected string option")
                        .resolved
                        .as_ref()
                        .expect("Expected string object");
                    hidden_ability::get_pokemon_ha_from_api(options).await
                },
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
    /*
    To Do:
    Error responses should be ethereal to the command user, if possible
    Clean up the code probably
        Utilize Rustemon library for API calls
        IDK what else
    */

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(Handler)
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