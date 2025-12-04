use async_trait::async_trait;
use chrono::Datelike;
use rand::random;
use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage, User, UserId,
};
use serenity::prelude::*;
use tokio::task;

use crate::database::{DatabaseError, DatabaseResult};

// const SECRET_ADMIN_ID: u64 = 255117530253754378; // My ID
pub const SECRET_ADMIN_ID: u64 = 248966803139723264; // Grif's ID
const WEIGHTS: [f32; 3] = [0.0, 0.0, 0.5];
pub const PREV_RELEVANT_EVENTS: usize = WEIGHTS.len();

pub type SecretResult<T> = Result<T, DatabaseError>;

#[derive(Debug)]
pub struct SecretResponse {
    content: String,
    buttons: Vec<String>,
}

pub type Assignee = Option<u64>;
pub type Assignments = Vec<(u64, u64)>;

#[derive(Debug)]
pub struct GifteeHistory {
    pub event: i32,
    pub user: u64,
    pub user_giftee: u64,
}

pub struct ParticipantUpdate {
    total_participants: u64,
    latest_change: ToggledParticipation,
}

impl ParticipantUpdate {
    pub fn new(total_participants: u64, latest_change: ToggledParticipation) -> Self {
        Self {
            total_participants,
            latest_change,
        }
    }
}

impl ParticipantUpdate {
    fn to_string(&self) -> String {
        match &self.latest_change {
            ToggledParticipation::UserJoined(user_id) => format!(
                "{} has joined the event! {} has {} participants",
                UserId::new(*user_id).mention(),
                current_year(),
                self.total_participants
            ),
            ToggledParticipation::UserLeft(user_id) => format!(
                "{} has left the event. {} has {} participants",
                UserId::new(*user_id).mention(),
                current_year(),
                self.total_participants
            ),
        }
    }
}

pub enum ToggledParticipation {
    UserJoined(u64),
    UserLeft(u64),
}

#[async_trait]
pub trait SecretSantaTrait: Send + Sync {
    fn get_latest_giftee(&self, user_id: u64) -> DatabaseResult<Assignee>;
    fn start_new_event(&self) -> DatabaseResult<()>;
    fn is_event_open(&self) -> DatabaseResult<bool>;
    fn toggle_event_participation(
        &self,
        user_id: u64,
        username: String,
    ) -> DatabaseResult<ParticipantUpdate>;
    fn get_drawn_names(&self) -> DatabaseResult<Assignments>;
}

pub fn run(
    _options: &[CommandDataOption],
    invoker: &User,
    db: &impl SecretSantaTrait,
) -> CreateInteractionResponseMessage {
    let response_data = match invoker.id.get() {
        SECRET_ADMIN_ID => admin_response(),
        _ => user_response(invoker.id.get(), db),
    };
    if let Err(why) = response_data {
        return CreateInteractionResponseMessage::new()
            .content(why.to_string())
            .ephemeral(true);
    }
    let response_data = response_data.unwrap();
    let mut response = CreateInteractionResponseMessage::new()
        .content(response_data.content)
        .ephemeral(true);
    if !response_data.buttons.is_empty() {
        let mut row_buttons = Vec::new();
        for button_id in response_data.buttons {
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

fn admin_response() -> SecretResult<SecretResponse> {
    Ok(SecretResponse {
        content: "Hello admin!".to_string(),
        buttons: vec!["start_new_event".to_string(), "draw_names".to_string()],
    })
}

fn user_response(user_id: u64, db: &impl SecretSantaTrait) -> SecretResult<SecretResponse> {
    let latest_giftee = db.get_latest_giftee(user_id)?;
    let content = match latest_giftee {
        Some(giftee_id) => {
            let giftee_mention = UserId::new(giftee_id).mention();
            format!("Your giftee is {}", giftee_mention)
        }
        None => "No giftee found - are you a participant for this event?".to_string(),
    };
    Ok(SecretResponse {
        content,
        buttons: vec![],
    })
}

pub fn register() -> CreateCommand {
    CreateCommand::new("secret").description("See your recipient for secret santa!")
}

fn get_button_label(button_id: &str) -> &str {
    match button_id {
        "start_new_event" => "Create New Secret Santa Event",
        "draw_names" => "Draw Names",
        "toggle_event_participation" => "Join (or Leave) Secret Santa",
        _ => "How did you conjure this??",
    }
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

pub fn current_year() -> i32 {
    chrono::Local::now().year()
}

pub async fn start_new_event(db: &impl SecretSantaTrait) -> CreateInteractionResponseMessage {
    let res: Result<String, String> = match db.start_new_event() {
        Ok(_) => Ok("New event has begun!".to_string()),
        Err(why) => Err(why.to_string()),
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
        Err(why) => CreateInteractionResponseMessage::new()
            .content(why.to_string())
            .ephemeral(true),
    }
}

pub fn toggle_event_participation(
    invoker: &User,
    db: &impl SecretSantaTrait,
) -> CreateInteractionResponseMessage {
    let res = db.toggle_event_participation(invoker.id.get(), invoker.name.clone());
    match res {
        Ok(toggled_participation) => CreateInteractionResponseMessage::new()
            .content(toggled_participation.to_string())
            .ephemeral(false),
        Err(why) => CreateInteractionResponseMessage::new()
            .content(why.to_string())
            .ephemeral(true),
    }
}

pub async fn draw_names(
    ctx: &Context,
    db: impl SecretSantaTrait + Clone + 'static,
) -> CreateInteractionResponseMessage {
    let assignments_res = task::spawn_blocking(move || db.get_drawn_names())
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
        Err(why) => CreateInteractionResponseMessage::new()
            .content(format!("{}", why))
            .ephemeral(true),
    }
}
