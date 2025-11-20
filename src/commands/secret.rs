use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::model::id::UserId;
use serenity::model::user::User;
use serenity::prelude::Mentionable;
use rusqlite::{Connection, Result, params, Error};
use chrono::Datelike;
use rand::{random, thread_rng};
use rand::seq::SliceRandom;


// const ADMIN_ID: u64 = 255117530253754378;
const ADMIN_ID: u64 = 248966803139723264; // Grif's ID
const WEIGHTS: [f32; 3] = [0.0, 0.0, 0.5];
const PREV_RELEVANT_EVENTS: usize = WEIGHTS.len();


#[derive(Debug)]
struct GifteeHistory {
    event: i32,
    user: u64,
    user_giftee: u64,
}


fn check_assignment_validation(permutation: &Vec<usize>, restrictions: &Vec<[usize; 3]>) -> bool {
    for elem in 0..permutation.len() {
        if permutation[elem] == elem { // Ensures the permutation is a derangement
            return false
        }
        for prev_event in 0..3 {
            if permutation[elem] == restrictions[elem][prev_event] {
                if WEIGHTS[prev_event] == 0.0 { // The most recent events prevent repeat pairings
                    return false
                } else if random::<f32>() > WEIGHTS[prev_event] { // Some previous events allow a chance of repeated pairings
                    return false
                }
            }
        }
    }
    return true
}


fn get_latest_giftee(user_id: UserId, conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare("
        SELECT user_giftee
        FROM participation
        WHERE user = (?1) AND event = (
            SELECT MAX(event_id) FROM events
        )
    ")?;

    let result: Result<i64, Error> = stmt.query_row(params![user_id.as_u64()], |row| {
        row.get(0)
    });

    match result {
        Ok(giftee_id) => {
            let uid = UserId(giftee_id as u64);
            Ok(format!("Your giftee is: {}", uid.mention().to_string()))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Ok("No giftee found - are you sure you're a participant for this event?".to_string())
        },
        Err(e) => Err(e)
    }
}


pub fn run_wrapped(_options: &[CommandDataOption], invoker: &User) -> Result<(String, Vec<String>)> {
    // Move this to the main, shouldn't need to re-open every time?
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;
    if invoker.id == ADMIN_ID { // Griffin's ID, runs hosting command WORK ON THIS NEXT
        return Ok((
            "You are admin! Let's add this!".to_string(),
            vec!["start_new_event".to_string(), "draw_names".to_string()]
        ));
    } else { // Other ids will return their currently assigned giftee
        return Ok((format!("{}", get_latest_giftee(invoker.id, &conn)?), vec![]));
    }
}


pub fn run(_options: &[CommandDataOption], invoker: &User) -> (String, bool, Vec<String>) {
    match run_wrapped(_options, invoker) {
        Ok(reply) => (reply.0, true, reply.1),
        Err(e) => (format!("{}", e), true, vec![])
    }
}


pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("secret").description("See your recipient for secret santa!")
}


fn current_year() -> i32 {
    chrono::Local::now().year()
}


pub async fn start_new_event() -> Result<String> {
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;
    let current_year = chrono::Local::now().year();
    conn.execute("
        INSERT INTO events (event_id) 
        VALUES (?1)
    ", params![current_year])?; // TODO: Better error handling?
    conn.execute("
        INSERT INTO participation (event, user)
        VALUES (?1, ?2);
    ", params![current_year, ADMIN_ID])?;
    return Ok("New event has begun!".to_string()) // TODO: This might needs to also create a message with a button
}


pub fn is_event_open(conn: &Connection) -> Result<bool> {
    let mut stmt = conn.prepare("
        SELECT 1
        FROM participation
        WHERE event = ?1 and user_giftee IS NULL
        LIMIT 1
    ")?;
    let mut iter = stmt.query_map(params![current_year()], |_row| Ok(()))?;
    Ok(iter.next().transpose()?.is_some())
}


pub fn join_event(invoker: &User) -> Result<String> {
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;
    if !is_event_open(&conn)? {
        return Ok("Unable to join - the names have already been drawn for this event.".to_string())
    }
    conn.execute("
        INSERT OR IGNORE INTO users (user_id, username)
        VALUES (?1, ?2);
    ", params![invoker.id.as_u64(), invoker.name])?;
    conn.execute("
        INSERT INTO participation (event, user, user_giftee)
        VALUES (?1, ?2, NULL);
    ", params![current_year(), invoker.id.as_u64()])?;
    Ok(format!("{} has joined the event!", invoker.mention()))
}


pub fn draw_names() -> Result<String> {
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;
    let mut prev_years_stmt = conn.prepare("
        SELECT event_id
        FROM events
        WHERE event_id != ?1
        ORDER BY event_id DESC
        LIMIT ?2
    ")?;
    let prev_years = prev_years_stmt
        .query_map(params![current_year(), PREV_RELEVANT_EVENTS], |row| row.get::<_, i32>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut current_participants_stmt = conn.prepare("
        SELECT user
        FROM participation
        WHERE event = ?1
    ")?;
    let current_participants = current_participants_stmt
        .query_map(params![current_year()], |row| row.get::<_, u64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let num_participants = current_participants.len();

    let mut giftee_history: Vec<[usize; PREV_RELEVANT_EVENTS]> = vec![[num_participants; PREV_RELEVANT_EVENTS]; num_participants];
    let mut giftee_history_stmt = conn.prepare("
        SELECT user, user_giftee, event
        FROM participation
        WHERE event < ?1 AND event >= ?2
        ORDER BY event DESC
    ")?;
    let giftee_history_iter = giftee_history_stmt
        .query_map(params![current_year(), prev_years[prev_years.len() - 1]], |row| {
            Ok(GifteeHistory { user: row.get(0)?, user_giftee: row.get(1)?, event: row.get(2)? })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for previous_participation in giftee_history_iter {
        let event_idx = (prev_years[0] - previous_participation.event) as usize;
        if let Some(user_idx) = current_participants.iter().position(|&x| x == previous_participation.user) {
            if let Some(giftee_idx) = current_participants.iter().position(|&x| x == previous_participation.user_giftee) {
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
    };

    for participant_idx in 0..current_participants.len() {
        let participant_id = current_participants[participant_idx];
        let giftee_id = current_participants[solution[participant_idx]];
        conn.execute("
            UPDATE participation
            SET user_giftee = ?1
            WHERE event = ?2 AND user = ?3;
        ", params![giftee_id, current_year(), participant_id])?;
    }

    Ok("Names have been drawn! Check your DMs".to_string())
}
