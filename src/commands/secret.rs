use async_trait::async_trait;
use chrono::Datelike;
use rand::seq::SliceRandom;
use rand::{random, thread_rng};
use rusqlite::{params, Connection, Error, Result};
use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage, User, UserId,
};
use serenity::prelude::*;
use tokio::task;

use crate::database::DbPool;

#[async_trait]
pub trait SecretSantaTrait: Send + Sync {
    fn get_latest_giftee(&self, user_id: u64) -> Result<String>;
    fn is_event_open(&self) -> Result<bool>;
    fn toggle_event_participation(&self, user_id: u64, username: String) -> Result<String>;
    fn get_drawn_names(&self) -> Result<Vec<(u64, u64)>>;
}

fn get_button_label(button_id: &str) -> &str {
    match button_id {
        "start_new_event" => "Create New Secret Santa Event",
        "draw_names" => "Draw Names",
        "toggle_event_participation" => "Join (or Leave) Secret Santa",
        _ => "How did you conjure this??",
    }
}

// const ADMIN_ID: u64 = 255117530253754378; // My ID
const ADMIN_ID: u64 = 248966803139723264; // Grif's ID
const WEIGHTS: [f32; 3] = [0.0, 0.0, 0.5];
pub const PREV_RELEVANT_EVENTS: usize = WEIGHTS.len();

#[derive(Debug)]
pub struct GifteeHistory {
    pub event: i32,
    pub user: u64,
    pub user_giftee: u64,
}

pub fn check_assignment_validation(
    permutation: &Vec<usize>,
    restrictions: &Vec<[usize; 3]>,
) -> bool {
    for elem in 0..permutation.len() {
        if permutation[elem] == elem {
            // Ensures the permutation is a derangement
            return false;
        }
        for prev_event in 0..3 {
            if permutation[elem] == restrictions[elem][prev_event] {
                if WEIGHTS[prev_event] == 0.0 {
                    // The most recent events prevent repeat pairings
                    return false;
                } else if random::<f32>() > WEIGHTS[prev_event] {
                    // Some previous events allow a chance of repeated pairings
                    return false;
                }
            }
        }
    }
    return true;
}

fn get_latest_giftee(user_id: UserId, conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare(
        "
        SELECT user_giftee
        FROM participation
        WHERE user = (?1) AND event = (
            SELECT MAX(event_id) FROM events
        )
    ",
    )?;

    let result: Result<i64, Error> = stmt.query_row(params![user_id.get()], |row| row.get(0));

    match result {
        Ok(giftee_id) => {
            let uid = UserId::new(giftee_id as u64);
            Ok(format!("Your giftee is: {}", uid.mention().to_string()))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Ok("No giftee found - are you sure you're a participant for this event?".to_string())
        }
        Err(e) => Err(e),
    }
}

pub fn run_wrapped(
    _options: &[CommandDataOption],
    invoker: &User,
    pool: &DbPool,
) -> Result<(String, Vec<String>)> {
    let conn = pool.get().map_err(|_e| Error::QueryReturnedNoRows)?; // Map r2d2 error to rusqlite error or handle differently
    if invoker.id == ADMIN_ID {
        // Griffin's ID, runs hosting command WORK ON THIS NEXT
        return Ok((
            "Hello admin!".to_string(),
            vec!["start_new_event".to_string(), "draw_names".to_string()],
        ));
    } else {
        // Other ids will return their currently assigned giftee
        return Ok((format!("{}", get_latest_giftee(invoker.id, &conn)?), vec![]));
    }
}

pub fn run(
    options: &[CommandDataOption],
    invoker: &User,
    pool: &DbPool,
) -> CreateInteractionResponseMessage {
    match run_wrapped(options, invoker, pool) {
        Ok((content, buttons)) => {
            let mut response = CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true);
            if !buttons.is_empty() {
                let mut row_buttons = Vec::new();
                for button_id in buttons {
                    row_buttons.push(
                        CreateButton::new(button_id.clone())
                            .style(ButtonStyle::Success)
                            .label(get_button_label(&button_id)),
                    );
                }
                response = response.components(vec![CreateActionRow::Buttons(row_buttons)]);
            }
            response
        }
        Err(e) => CreateInteractionResponseMessage::new()
            .content(format!("{}", e))
            .ephemeral(true),
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("secret").description("See your recipient for secret santa!")
}

pub fn current_year() -> i32 {
    chrono::Local::now().year()
}

pub async fn start_new_event(pool: &DbPool) -> CreateInteractionResponseMessage {
    let res: Result<String, String> = match pool.get() {
        Ok(conn) => {
            let current_year = chrono::Local::now().year();
            if let Err(e) = conn.execute(
                "INSERT INTO events (event_id) VALUES (?1)",
                params![current_year],
            ) {
                Err(e.to_string())
            } else if let Err(e) = conn.execute(
                "INSERT INTO participation (event, user) VALUES (?1, ?2);",
                params![current_year, ADMIN_ID],
            ) {
                Err(e.to_string())
            } else {
                Ok("New event has begun!".to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    };

    match res {
        Ok(content) => {
            let buttons = vec!["toggle_event_participation"];
            let mut row_buttons = Vec::new();
            for button_id in buttons {
                row_buttons.push(
                    CreateButton::new(button_id)
                        .style(ButtonStyle::Success)
                        .label(get_button_label(button_id)),
                );
            }
            CreateInteractionResponseMessage::new()
                .content(content)
                .components(vec![CreateActionRow::Buttons(row_buttons)])
        }
        Err(e) => CreateInteractionResponseMessage::new()
            .content(format!("{}", e))
            .ephemeral(true),
    }
}

pub fn is_event_open(conn: &Connection) -> Result<bool> {
    let mut stmt = conn.prepare(
        "
        SELECT 1
        FROM participation
        WHERE event = ?1 and user_giftee IS NULL
        LIMIT 1
    ",
    )?;
    let mut iter = stmt.query_map(params![current_year()], |_row| Ok(()))?;
    Ok(iter.next().transpose()?.is_some())
}

pub fn toggle_event_participation(
    invoker: &User,
    pool: &DbPool,
) -> CreateInteractionResponseMessage {
    let res: Result<String> = (|| {
        let conn = pool.get().map_err(|_| Error::QueryReturnedNoRows)?; // TODO: Better error handling
        if !is_event_open(&conn)? {
            return Ok(
                "Unable to join - the names have already been drawn for this event.".to_string(),
            );
        }

        conn.execute(
            "
            INSERT OR IGNORE INTO users (user_id, username)
            VALUES (?1, ?2);
        ",
            params![invoker.id.get(), invoker.name],
        )?;

        let mut stmt = conn.prepare(
            "
            SELECT 1
            FROM participation
            WHERE user = ?1
            LIMIT 1;
        ",
        )?;
        let mut iter = stmt.query_map(params![invoker.id.get()], |_row| Ok(()))?;

        let mut count_participants_stmt = conn.prepare(
            "
            SELECT COUNT(*)
            FROM participation
            WHERE event = ?1;
        ",
        )?;
        let participant_count: i64 =
            count_participants_stmt.query_row(params![current_year()], |row| row.get(0))?;

        match iter.next().transpose()?.is_some() {
            // Checking if user is already in the event
            true => {
                conn.execute(
                    "
                    DELETE FROM participation
                    WHERE user = ?1 AND event = ?2
                ",
                    params![invoker.id.get(), current_year()],
                )?;
                Ok(format!(
                    "{} has left the event. {} has {} participants",
                    invoker.mention(),
                    current_year(),
                    participant_count
                ))
            }
            false => {
                conn.execute(
                    "
                    INSERT INTO participation (event, user, user_giftee)
                    VALUES (?1, ?2, NULL);
                ",
                    params![current_year(), invoker.id.get()],
                )?;
                Ok(format!(
                    "{} has joined the event! {} has {} participants",
                    invoker.mention(),
                    current_year(),
                    participant_count
                ))
            }
        }
    })();

    match res {
        Ok(content) => CreateInteractionResponseMessage::new().content(content),
        Err(e) => CreateInteractionResponseMessage::new()
            .content(format!("{}", e))
            .ephemeral(true),
    }
}

// This exists to make the tokio code cleaner
// Break this into smaller functions
fn get_drawn_names(pool: &DbPool) -> Result<Vec<(u64, u64)>> {
    let conn = pool.get().map_err(|_| Error::QueryReturnedNoRows)?;
    let mut prev_years_stmt = conn.prepare(
        "
        SELECT event_id
        FROM events
        WHERE event_id != ?1
        ORDER BY event_id DESC
        LIMIT ?2
    ",
    )?;
    let prev_years = prev_years_stmt
        .query_map(params![current_year(), PREV_RELEVANT_EVENTS], |row| {
            row.get::<_, i32>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut current_participants_stmt = conn.prepare(
        "
        SELECT user
        FROM participation
        WHERE event = ?1
    ",
    )?;
    let current_participants = current_participants_stmt
        .query_map(params![current_year()], |row| row.get::<_, u64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let num_participants = current_participants.len();

    let mut giftee_history: Vec<[usize; PREV_RELEVANT_EVENTS]> =
        vec![[num_participants; PREV_RELEVANT_EVENTS]; num_participants];
    let mut giftee_history_stmt = conn.prepare(
        "
        SELECT user, user_giftee, event
        FROM participation
        WHERE event < ?1 AND event >= ?2
        ORDER BY event DESC
    ",
    )?;
    let giftee_history_iter = giftee_history_stmt
        .query_map(
            params![current_year(), prev_years[prev_years.len() - 1]],
            |row| {
                Ok(GifteeHistory {
                    user: row.get(0)?,
                    user_giftee: row.get(1)?,
                    event: row.get(2)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    for previous_participation in giftee_history_iter {
        let event_idx = (prev_years[0] - previous_participation.event) as usize;
        if let Some(user_idx) = current_participants
            .iter()
            .position(|&x| x == previous_participation.user)
        {
            if let Some(giftee_idx) = current_participants
                .iter()
                .position(|&x| x == previous_participation.user_giftee)
            {
                giftee_history[user_idx][event_idx] = giftee_idx;
            }
        }
    }

    let mut solution: Vec<usize> = (0..current_participants.len()).collect();
    let mut viable_solution = false;
    let mut rng = thread_rng();
    while !viable_solution {
        solution.shuffle(&mut rng);
        viable_solution = check_assignment_validation(&solution, &giftee_history);
    }

    let assignments: Vec<(u64, u64)> = solution
        .iter()
        .enumerate()
        .map(|(participant_idx, &giftee_idx)| {
            (
                current_participants[participant_idx],
                current_participants[giftee_idx],
            )
        })
        .collect();

    for &(participant_id, giftee_id) in assignments.iter() {
        conn.execute(
            "
            UPDATE participation
            SET user_giftee = ?1
            WHERE event = ?2 AND user = ?3;
        ",
            params![giftee_id, current_year(), participant_id],
        )?;
    }
    Ok(assignments)
}

pub async fn draw_names(ctx: &Context, pool: &DbPool) -> CreateInteractionResponseMessage {
    let pool = pool.clone();
    let assignments_res = task::spawn_blocking(move || get_drawn_names(&pool))
        .await
        .expect("Failed to run database tasks");

    match assignments_res {
        Ok(assignments) => {
            for &(participant_id, giftee_id) in assignments.iter() {
                if let Ok(participant_user) = UserId::new(participant_id).to_user(&ctx.http).await {
                    let giftee_mention = UserId::new(giftee_id).mention().to_string();
                    let dm_message = format!(
                        "🎉 Your Secret Santa assignment for the {} event is {}! 🎉",
                        current_year(),
                        giftee_mention
                    );
                    if let Ok(dm_channel) = participant_user.create_dm_channel(&ctx.http).await {
                        if let Err(why) = dm_channel.say(&ctx.http, dm_message).await {
                            println!(
                                "Could not fetch Discord user object for ID {}: {}",
                                participant_id, why
                            );
                        }
                    }
                } else {
                    println!(
                        "Could not fetch Discord user object for ID {}",
                        participant_id
                    );
                }
            }
            CreateInteractionResponseMessage::new().content("Names have been drawn! Check your DMs")
        }
        Err(e) => CreateInteractionResponseMessage::new()
            .content(format!("{}", e))
            .ephemeral(true),
    }
}
