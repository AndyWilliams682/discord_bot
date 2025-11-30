use std::{sync::Arc, time::Duration};
use serenity::all::{Context, UserId, GuildId, Mentionable, CreateMessage};
use chrono::Local;
use rusqlite::{Connection, Result, params};

const HOUR_TO_RUN: u32 = 17;
const GUILD_ID: u64 = 323928878420590592; // 704782281578905670;
const CHANNEL_NAME: &str = "gif-of-the-day"; // "test";

pub fn start(ctx: Arc<Context>) {
    let gotd_context = Arc::clone(&ctx);
    tokio::spawn(async move {
        loop {
            let now = Local::now();
            let next = next_nine_am(now);
            let wait_dur = next.signed_duration_since(now).to_std().unwrap_or_else(|_| Duration::from_secs(0));
            tokio::time::sleep(wait_dur).await;
            post_gotd(Arc::clone(&gotd_context)).await;
        }
    });
}


fn next_nine_am(now: chrono::DateTime<Local>) -> chrono::DateTime<Local> {
    let today_nine = now.date_naive().and_hms_opt(HOUR_TO_RUN, 0, 0)
        .unwrap_or_else(|| now.date_naive().and_hms_opt(HOUR_TO_RUN, 0, 0).unwrap())
        .and_local_timezone(Local).unwrap();
    if now < today_nine {
        today_nine
    } else {
        let next_day = now.date_naive().succ_opt().unwrap_or_else(|| now.date_naive() + chrono::Duration::days(1));
        next_day.and_hms_opt(HOUR_TO_RUN, 0, 0).unwrap().and_local_timezone(Local).unwrap()
    }
}


fn select_random_gif() -> Result<(u64, String)> {
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;

    let gif_stmt = "
        SELECT submitted_by, url
        FROM gifs
        WHERE gifs.posts = (SELECT MIN(posts) FROM gifs)
        ORDER BY RANDOM()
        LIMIT 1;
    ";

    conn.query_row(gif_stmt, params![], |row| {
        let gif_submitter: u64 = row.get(0)?;
        let gif_url: String = row.get(1)?;

        conn.execute("
            UPDATE gifs
            SET posts = posts + 1
            WHERE url = ?1;
        ", params![gif_url.clone()])?;
        
        Ok((gif_submitter, gif_url))
    })
}

async fn post_gotd(ctx: Arc<Context>) {
    let content = match select_random_gif() {
        Ok((submitter, url)) => format!("{} Submitted by {}", url, UserId::new(submitter).mention().to_string()),
        Err(e) => format!("Error posting GotD: {}", e)
    };
    if let Ok(channels) = GuildId::new(GUILD_ID).channels(&ctx.http).await {
        if let Some((id, _channel)) = channels.into_iter().find(|(_id, channel)| channel.name == CHANNEL_NAME) {
            if let Err(err) = id.send_message(&ctx.http, CreateMessage::new().content(&content)).await {
                println!("Failed to send GOTD message to channel {}: {:?}", id.get(), err);
            }
        } else {
            println!("Channel {} not found in guild {}", CHANNEL_NAME, GUILD_ID);
        }
    } else {
        println!("Failed to fetch channels for guild {}", GUILD_ID);
    }
}
