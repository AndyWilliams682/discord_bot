use serenity::{builder::CreateApplicationCommand};
use rusqlite::{Connection, Result, params};
use serenity::model::user::User;
use serenity::model::prelude::interaction::application_command::{CommandDataOption, CommandDataOptionValue};
use serenity::model::prelude::command::CommandOptionType;
use url::Url;


fn is_valid_url(s: &str) -> bool {
    match Url::parse(s) {
        Ok(url) => {
            matches!(url.scheme(), "http" | "https")
        },
        Err(_) => false,
    }
}


pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("gotd")
        .description("Submit a url for for gif of the day")
        .create_option(|option| {
            option
                .name("url")
                .description("The url of your gif")
                .kind(CommandOptionType::String)
                .required(true)
        })
}


fn run_wrapped(url: &str, invoker: &User) -> Result<String> {
    let db_file_path = "/usr/local/bin/data/mtg_secret_santa.bin";
    let conn = Connection::open(db_file_path)?;

    conn.execute("
        INSERT OR IGNORE INTO users (user_id, username)
        VALUES (?1, ?2);
    ", params![invoker.id.as_u64(), invoker.name])?;

    return match is_valid_url(&url) {
        true => {
            conn.execute("
                INSERT INTO gifs (submitted_by, url, posts)
                VALUES (?1, ?2, 0);
            ", params![invoker.id.as_u64(), url])?;
            Ok("Gif added, thank you!".to_string())
        },
        false => Ok("Your url does not appear to be valid".to_string())
    }
}


pub fn run(options: &[CommandDataOption], invoker: &User) -> String {
    let first_option = options
        .get(0)
        .expect("Expected string option")
        .resolved
        .as_ref()
        .expect("Expected string object");
    if let CommandDataOptionValue::String(url) = first_option {
        match run_wrapped(url, invoker) {
            Ok(reply) => reply,
            Err(e) => format!("{}", e)
        }
    } else {
        "How did you input a non-string?".to_string()
    }
}
