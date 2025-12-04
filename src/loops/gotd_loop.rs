use chrono::Local;
use serenity::all::{Context, CreateMessage, GuildId, Mentionable, UserId};
use std::{sync::Arc, time::Duration};

use crate::commands::gotd::GotdTrait;
use crate::database::BotDatabase;

const HOUR_TO_RUN: u32 = 17;
const GUILD_ID: u64 = 323928878420590592; // 704782281578905670;
const CHANNEL_NAME: &str = "gif-of-the-day"; // "test";

pub fn start(ctx: Arc<Context>, db: BotDatabase) {
    let gotd_context = Arc::clone(&ctx);
    let db = db.clone();
    tokio::spawn(async move {
        loop {
            let now = Local::now();
            let next = next_nine_am(now);
            let wait_dur = next
                .signed_duration_since(now)
                .to_std()
                .unwrap_or_else(|_| Duration::from_secs(0));
            tokio::time::sleep(wait_dur).await;
            post_gotd(Arc::clone(&gotd_context), &db.clone()).await;
        }
    });
}

async fn post_gotd(ctx: Arc<Context>, db: &impl GotdTrait) {
    let content = match db.select_random_gif().await {
        Ok((submitter, url)) => format!(
            "{} Submitted by {}",
            url,
            UserId::new(submitter).mention().to_string()
        ),
        Err(why) => format!("Error posting GotD: {}", why),
    };
    if let Ok(channels) = GuildId::new(GUILD_ID).channels(&ctx.http).await {
        if let Some((id, _channel)) = channels
            .into_iter()
            .find(|(_id, channel)| channel.name == CHANNEL_NAME)
        {
            if let Err(why) = id
                .send_message(&ctx.http, CreateMessage::new().content(&content))
                .await
            {
                println!(
                    "Failed to send GOTD message to channel {}: {:?}",
                    id.get(),
                    why
                );
            }
        } else {
            println!("Channel {} not found in guild {}", CHANNEL_NAME, GUILD_ID);
        }
    } else {
        println!("Failed to fetch channels for guild {}", GUILD_ID);
    }
}

fn next_nine_am(now: chrono::DateTime<Local>) -> chrono::DateTime<Local> {
    let today = now.date_naive();
    let today_nine = today
        .and_hms_opt(HOUR_TO_RUN, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();

    if now < today_nine {
        today_nine
    } else {
        let next_day = today
            .succ_opt()
            .unwrap_or_else(|| today + chrono::Duration::days(1));
        next_day
            .and_hms_opt(HOUR_TO_RUN, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
    }
}
