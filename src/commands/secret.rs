use async_trait::async_trait;
use chrono::Datelike;
use rand::random;
use serenity::all::{
    ButtonStyle, CommandDataOption, CreateActionRow, CreateButton, CreateCommand,
    CreateInteractionResponseMessage, User, UserId,
};
use serenity::prelude::*;
use tokio::task;

use crate::database::DatabaseResult;

// const SECRET_ADMIN_ID: u64 = 255117530253754378; // My ID
pub const SECRET_ADMIN_ID: u64 = 248966803139723264; // Grif's ID
const WEIGHTS: [f32; 3] = [0.0, 0.0, 0.5];
pub const PREV_RELEVANT_EVENTS: usize = WEIGHTS.len();

use crate::commands::error::CommandError;

pub type SecretResult<T> = Result<T, CommandError>;

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
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let response_data = match invoker.id.get() {
        SECRET_ADMIN_ID => admin_response(),
        _ => user_response(invoker.id.get(), db),
    };
    response_from_result(response_data)
}

fn response_from_result(
    res: SecretResult<SecretResponse>,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    match res {
        Ok(data) => {
            let mut response = CreateInteractionResponseMessage::new()
                .content(data.content)
                .ephemeral(true);
            if !data.buttons.is_empty() {
                let mut row_buttons = Vec::new();
                for button_id in data.buttons {
                    row_buttons.push(
                        CreateButton::new(button_id.clone())
                            .style(ButtonStyle::Success)
                            .label(get_button_label(&button_id)),
                    );
                }
                response = response.components(vec![CreateActionRow::Buttons(row_buttons)]);
            }
            Ok(response)
        }
        Err(why) => Err(why),
    }
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

pub fn start_new_event_logic(db: &impl SecretSantaTrait) -> SecretResult<SecretResponse> {
    db.start_new_event()?;
    Ok(SecretResponse {
        content: "New event has begun!".to_string(),
        buttons: vec!["toggle_event_participation".to_string()],
    })
}

pub async fn start_new_event_interaction(
    db: &impl SecretSantaTrait,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    response_from_result(start_new_event_logic(db))
}

pub fn toggle_event_participation_logic(
    user_id: u64,
    username: String,
    db: &impl SecretSantaTrait,
) -> SecretResult<SecretResponse> {
    let toggled_participation = db.toggle_event_participation(user_id, username)?;
    Ok(SecretResponse {
        content: toggled_participation.to_string(),
        buttons: vec![],
    })
}

pub fn toggle_event_participation_interaction(
    invoker: &User,
    db: &impl SecretSantaTrait,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    response_from_result(toggle_event_participation_logic(
        invoker.id.get(),
        invoker.name.clone(),
        db,
    ))
}

pub async fn draw_names_interaction(
    ctx: &Context,
    db: impl SecretSantaTrait + Clone + 'static,
) -> Result<CreateInteractionResponseMessage, CommandError> {
    let assignments_res = task::spawn_blocking(move || db.get_drawn_names())
        .await
        .expect("Failed to run database tasks");

    match assignments_res {
        Ok(assignments) => {
            notify_participants(ctx, &assignments).await;
            Ok(CreateInteractionResponseMessage::new()
                .content("Names have been drawn! Check your DMs"))
        }
        Err(why) => Err(why.into()),
    }
}

async fn notify_participants(ctx: &Context, assignments: &Assignments) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseError;

    #[derive(Clone)]
    struct MockSecretDB {
        should_fail: bool,
    }

    #[async_trait]
    impl SecretSantaTrait for MockSecretDB {
        fn get_latest_giftee(&self, user_id: u64) -> DatabaseResult<Assignee> {
            if self.should_fail {
                Err(DatabaseError::QueryError("Mock DB Error".to_string()))
            } else if user_id == 1 {
                Ok(Some(2))
            } else {
                Ok(None)
            }
        }

        fn start_new_event(&self) -> DatabaseResult<()> {
            if self.should_fail {
                Err(DatabaseError::QueryError("Mock DB Error".to_string()))
            } else {
                Ok(())
            }
        }

        fn is_event_open(&self) -> DatabaseResult<bool> {
            Ok(true)
        }

        fn toggle_event_participation(
            &self,
            _user_id: u64,
            _username: String,
        ) -> DatabaseResult<ParticipantUpdate> {
            if self.should_fail {
                Err(DatabaseError::QueryError("Mock DB Error".to_string()))
            } else {
                Ok(ParticipantUpdate::new(
                    1,
                    ToggledParticipation::UserJoined(1),
                ))
            }
        }

        fn get_drawn_names(&self) -> DatabaseResult<Assignments> {
            if self.should_fail {
                Err(DatabaseError::QueryError("Mock DB Error".to_string()))
            } else {
                Ok(vec![(1, 2), (2, 1)])
            }
        }
    }

    #[test]
    fn test_check_assignment_validation_derangement() {
        // [0, 1, 2] -> 0 maps to 0 -> Invalid
        let invalid_perm = vec![0, 1, 2];
        let restrictions = vec![[9, 9, 9], [9, 9, 9], [9, 9, 9]]; // Dummy restrictions
        assert!(!check_assignment_validation(&invalid_perm, &restrictions));

        // [1, 2, 0] -> 0->1, 1->2, 2->0 -> Valid derangement
        let valid_perm = vec![1, 2, 0];
        assert!(check_assignment_validation(&valid_perm, &restrictions));
    }

    #[test]
    fn test_start_new_event_logic() {
        let db = MockSecretDB { should_fail: false };
        let result = start_new_event_logic(&db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "New event has begun!");
    }

    #[test]
    fn test_toggle_event_participation_logic() {
        let db = MockSecretDB { should_fail: false };
        let result = toggle_event_participation_logic(1, "user".to_string(), &db);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.content.contains("has joined the event"));
    }

    #[test]
    fn test_admin_response() {
        let response = admin_response();
        assert!(response.is_ok());
        let data = response.unwrap();
        assert_eq!(data.content, "Hello admin!");
        assert!(data.buttons.contains(&"start_new_event".to_string()));
    }

    #[test]
    fn test_user_response_found() {
        let db = MockSecretDB { should_fail: false };
        // User 1 has giftee 2
        let result = user_response(1, &db);
        assert!(result.is_ok());
        assert!(result.unwrap().content.contains("Your giftee is"));
    }

    #[test]
    fn test_user_response_not_found() {
        let db = MockSecretDB { should_fail: false };
        // User 3 has no giftee
        let result = user_response(3, &db);
        assert!(result.is_ok());
        assert!(result.unwrap().content.contains("No giftee found"));
    }
}
