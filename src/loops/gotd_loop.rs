use chrono::Local;
use serenity::all::{Context, CreateMessage, GuildId, Mentionable, UserId};
use std::{sync::Arc, time::Duration};

use crate::commands::gotd::GotdTrait;
use crate::config::BotConfig;
use crate::database::BotDatabase;

pub fn start(ctx: Arc<Context>, db: BotDatabase, config: Arc<BotConfig>) {
    let gotd_context = Arc::clone(&ctx);
    let db = db.clone();
    tokio::spawn(async move {
        loop {
            let now = Local::now();
            let next = next_post_hour(now, config.gif_post_hour);
            let wait_dur = next
                .signed_duration_since(now)
                .to_std()
                .unwrap_or_else(|_| Duration::from_secs(0));
            tokio::time::sleep(wait_dur).await;
            post_gotd(Arc::clone(&gotd_context), &db, &config).await;
        }
    });
}

async fn post_gotd(ctx: Arc<Context>, db: &impl GotdTrait, config: &BotConfig) {
    let content = match db.select_random_gif().await {
        Ok((submitter, name)) => format!(
            "{}/{} Submitted by {}",
            config.gif_base_url,
            name,
            UserId::new(submitter).mention().to_string()
        ),
        Err(why) => format!("Error posting GotD: {}", why),
    };
    let guild_id = config.gif_guild_id;
    let channel_name = &config.gif_channel_name;
    if let Ok(channels) = GuildId::new(guild_id).channels(&ctx.http).await {
        if let Some((id, _channel)) = channels
            .into_iter()
            .find(|(_id, channel)| &channel.name == channel_name)
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
            println!("Channel {} not found in guild {}", channel_name, guild_id);
        }
    } else {
        println!("Failed to fetch channels for guild {}", guild_id);
    }
}

fn next_post_hour(now: chrono::DateTime<Local>, daily_gif_hour: u32) -> chrono::DateTime<Local> {
    let today = now.date_naive();
    let today_post_hour = today
        .and_hms_opt(daily_gif_hour, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();

    if now < today_post_hour {
        today_post_hour
    } else {
        let next_day = today
            .succ_opt()
            .unwrap_or_else(|| today + chrono::Duration::days(1));
        next_day
            .and_hms_opt(daily_gif_hour, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    #[test]
    fn next_post_hour_before() {
        // Today at 08:00 (daily_gif_hour is 9)
        let now = Local.with_ymd_and_hms(2023, 10, 27, 8, 0, 0).unwrap();
        let next = next_post_hour(now, 9);

        // Should be Today at 09:00
        assert_eq!(next.day(), 27);
        assert_eq!(next.hour(), 9);
    }

    #[test]
    fn next_post_hour_after() {
        // Today at 20:00 (daily_gif_hour is 9)
        let now = Local.with_ymd_and_hms(2023, 10, 27, 20, 0, 0).unwrap();
        let next = next_post_hour(now, 9);

        // Should be Tomorrow (28th) at 09:00
        assert_eq!(next.day(), 28);
        assert_eq!(next.hour(), 9);
    }

    #[test]
    fn next_post_hour_midnight() {
        // Today at 00:00
        let now = Local.with_ymd_and_hms(2023, 10, 27, 0, 0, 0).unwrap();
        let next = next_post_hour(now, 9);

        // Should be Today at 09:00
        assert_eq!(next.day(), 27);
        assert_eq!(next.hour(), 9);
    }
}
