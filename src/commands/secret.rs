use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::model::id::UserId;
use serenity::model::user::User;
use serenity::prelude::Mentionable;
use rusqlite::{Connection, Result, params, Error};


#[derive(Debug)]
struct Participation {
    event: i32,
    user: i32,
    user_giftee: i32,
}

fn check_assignment_validation(assignment: &Vec<usize>) -> bool {
    for idx in 0..assignment.len() {
        if assignment[idx] == idx {
            return false
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


pub fn run_wrapped(_options: &[CommandDataOption], invoker: &User) -> Result<String> {
    // Move this to the main, shouldn't need to re-open every time?
    let db_file_path = "mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;
    if invoker.id == 248966803139723264 { // Griffin's ID, runs hosting command WORK ON THIS NEXT
        return Ok("You are admin! Let's add this!".to_string());
    } else { // Other ids will return their currently assigned giftee
        return Ok(format!("{}", get_latest_giftee(invoker.id, &conn)?));
    }
}


pub fn run(_options: &[CommandDataOption], invoker: &User) -> String {
    match run_wrapped(_options, invoker) {
        Ok(reply) => reply,
        Err(e) => format!("{}", e)
    }
}


pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("secret").description("See your recipient for secret santa!")
}


pub async fn run_secret_button_logic(user: &serenity::model::user::User) -> String {
    format!("🤫 You pressed the secret button! Your user ID is {}.", user.id.as_u64())
}
