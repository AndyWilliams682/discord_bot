use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs};

use serenity::async_trait;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use serenity::model::application::component::ButtonStyle;

mod commands;
mod loops;

fn get_button_label(button_id: &str) -> &str {
    match button_id {
        "start_new_event" => "Create New Secret Santa Event",
        "draw_names" => "Draw Names",
        "toggle_event_participation" => "Join (or Leave) Secret Santa",
        _ => "How did you conjure this?"
    }
}


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
                .create_application_command(|command| commands::gotd::register(command))
        })
        .await;

        let loop_ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            loops::status::start(loop_ctx.clone());
            loops::gotd_loop::start(loop_ctx.clone());
            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }


    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {
                println!("Received command interaction: {:#?}", command);

                let (content, is_ephemeral, shown_button_ids) = match command.data.name.as_str() {
                    "ping" => (commands::ping::run(&command.data.options), false, vec![]),
                    "ha" => (commands::hidden_ability::run(&command.data.options).await, false, vec![]),
                    "secret" => commands::secret::run(&command.data.options, &command.user),
                    "poe" => (commands::poe::run(&command.data.options, &self.config), false, vec![]),
                    "gotd" => (commands::gotd::run(&command.data.options, &command.user), true, vec![]),
                    _ => ("not implemented :(".to_string(), true, vec![]),
                };

                if let Err(why) = command
                    .create_interaction_response(&ctx.http, |response| {
                        response
                            .kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|message| {
                                message.content(content).ephemeral(is_ephemeral);

                                if shown_button_ids.len() > 0 {
                                    message.components(|components| {
                                        components.create_action_row(|row| {
                                            for button_id in &shown_button_ids {
                                                row.create_button(|button| {
                                                    button.style(ButtonStyle::Success)
                                                        .label(get_button_label(button_id))
                                                        .custom_id(button_id)
                                                });
                                            }
                                            row
                                        });
                                        components
                                    });
                                }
                                message
                            })
                    })
                    .await
                {
                    println!("Cannot respond to slash command: {}", why);
                }
            },
            Interaction::MessageComponent(component) => {
                let (follow_up_result, is_ephemeral, follow_up_buttons) = match component.data.custom_id.as_str() {
                    "start_new_event" => (commands::secret::start_new_event().await, false, vec!["toggle_event_participation"]),
                    "draw_names" => (commands::secret::draw_names(&ctx).await, false, vec![]),
                    "toggle_event_participation" => (commands::secret::toggle_event_participation(&component.user), false, vec![]),
                    _ => (Ok("How did you even invoke this?".to_string()), true, vec![])
                };

                let (follow_up_response, is_ephemeral, follow_up_buttons) = match follow_up_result {
                    Ok(resp) => (resp, is_ephemeral, follow_up_buttons),
                    Err(e) => (format!("{}", e), true, vec![])
                };
    
                // Respond to the button press interaction
                if let Err(why) = component
                    .create_interaction_response(&ctx.http, |response| {
                        response
                            .kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|message| {
                                // Use an ephemeral response for a "secret" follow-up
                                message.content(follow_up_response).ephemeral(is_ephemeral);
                                if follow_up_buttons.len() > 0 {
                                    message.components(|components| {
                                        components.create_action_row(|row| {
                                            for button_id in &follow_up_buttons {
                                                row.create_button(|button| {
                                                    button.style(ButtonStyle::Success)
                                                        .label(get_button_label(button_id))
                                                        .custom_id(button_id)
                                                });
                                            }
                                            row
                                        });
                                        components
                                    });
                                }
                                message
                            })
                    })
                    .await
                {
                    println!("Cannot respond to secret button press: {}", why);
                }
            },
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

    // Finally, start a single shard, and start listening to events.
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
