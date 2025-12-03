use crate::commands::gotd::SelectRandomGif;
use crate::database::BotDatabase;
use chrono::Local;
use serenity::all::{Context, CreateMessage, GuildId, Mentionable, UserId};
use std::{sync::Arc, time::Duration};

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
            post_gotd(Arc::clone(&gotd_context), db.clone()).await;
        }
    });
}

fn next_nine_am(now: chrono::DateTime<Local>) -> chrono::DateTime<Local> {
    let today_nine = now
        .date_naive()
        .and_hms_opt(HOUR_TO_RUN, 0, 0)
        .unwrap_or_else(|| now.date_naive().and_hms_opt(HOUR_TO_RUN, 0, 0).unwrap())
        .and_local_timezone(Local)
        .unwrap();
    if now < today_nine {
        today_nine
    } else {
        let next_day = now
            .date_naive()
            .succ_opt()
            .unwrap_or_else(|| now.date_naive() + chrono::Duration::days(1));
        next_day
            .and_hms_opt(HOUR_TO_RUN, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
    }
}

async fn post_gotd(ctx: Arc<Context>, db: BotDatabase) {
    let content = match db.select_random_gif().await {
        Ok((submitter, url)) => format!(
            "{} Submitted by {}",
            url,
            UserId::new(submitter).mention().to_string()
        ),
        Err(e) => format!("Error posting GotD: {}", e),
    };
    if let Ok(channels) = GuildId::new(GUILD_ID).channels(&ctx.http).await {
        if let Some((id, _channel)) = channels
            .into_iter()
            .find(|(_id, channel)| channel.name == CHANNEL_NAME)
        {
            if let Err(err) = id
                .send_message(&ctx.http, CreateMessage::new().content(&content))
                .await
            {
                println!(
                    "Failed to send GOTD message to channel {}: {:?}",
                    id.get(),
                    err
                );
            }
        } else {
            println!("Channel {} not found in guild {}", CHANNEL_NAME, GUILD_ID);
        }
    } else {
        println!("Failed to fetch channels for guild {}", GUILD_ID);
    }
}
