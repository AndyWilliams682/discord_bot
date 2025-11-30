use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs};

use serenity::async_trait;
use serenity::all::{
    Command, Interaction, Ready, ButtonStyle,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateActionRow, CreateButton
};
use serenity::prelude::*;

mod commands;
mod loops;

fn get_button_label(button_id: &str) -> &str {
    match button_id {
        "start_new_event" => "Create New Secret Santa Event",
        "draw_names" => "Draw Names",
        "toggle_event_participation" => "Join (or Leave) Secret Santa",
        _ => "How did you conjure this??"
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

        let _global_command = Command::set_global_commands(&ctx.http, vec![
            commands::ping::register(),
            commands::hidden_ability::register(),
            commands::secret::register(),
            commands::poe::register(),
            commands::gotd::register(),
        ])
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
            Interaction::Command(command) => {
                println!("Received command interaction: {:#?}", command);

                let (content, is_ephemeral, shown_button_ids) = match command.data.name.as_str() {
                    "ping" => (commands::ping::run(&command.data.options), false, vec![]),
                    "ha" => (commands::hidden_ability::run(&command.data.options).await, false, vec![]),
                    "secret" => commands::secret::run(&command.data.options, &command.user),
                    "poe" => (commands::poe::run(&command.data.options, &self.config), false, vec![]),
                    "gotd" => (commands::gotd::run(&command.data.options, &command.user).await, true, vec![]),
                    _ => ("not implemented :(".to_string(), true, vec![]),
                };

                let mut response_data = CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(is_ephemeral);

                if shown_button_ids.len() > 0 {
                    let mut buttons = Vec::new();
                    for button_id in &shown_button_ids {
                        buttons.push(
                            CreateButton::new(button_id.clone())
                                .style(ButtonStyle::Success)
                                .label(get_button_label(button_id))
                        );
                    }
                    let row = CreateActionRow::Buttons(buttons);
                    response_data = response_data.components(vec![row]);
                }

                if let Err(why) = command
                    .create_response(&ctx.http, 
                        CreateInteractionResponse::Message(response_data)
                    )
                    .await
                {
                    println!("Cannot respond to slash command: {}", why);
                }
            },
            Interaction::Component(component) => {
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
    
                let mut response_data = CreateInteractionResponseMessage::new()
                    .content(follow_up_response)
                    .ephemeral(is_ephemeral);

                if follow_up_buttons.len() > 0 {
                    let mut buttons = Vec::new();
                    for button_id in &follow_up_buttons {
                        buttons.push(
                            CreateButton::new(*button_id)
                                .style(ButtonStyle::Success)
                                .label(get_button_label(button_id))
                        );
                    }
                    let row = CreateActionRow::Buttons(buttons);
                    response_data = response_data.components(vec![row]);
                }

                // Respond to the button press interaction
                if let Err(why) = component
                    .create_response(&ctx.http, 
                        CreateInteractionResponse::Message(response_data)
                    )
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
