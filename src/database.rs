use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rusqlite;
use rusqlite::params;
use serenity::prelude::TypeMapKey;
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinError;

use crate::commands::gotd::GotdTrait;
use crate::commands::secret::{
    check_assignment_validation, current_year, Assignee, Assignments, GifteeHistory,
    ParticipantUpdate, SecretSantaTrait, ToggledParticipation, PREV_RELEVANT_EVENTS,
    SECRET_ADMIN_ID,
};

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct DbPoolWrapper;

impl TypeMapKey for DbPoolWrapper {
    type Value = Arc<DbPool>;
}

#[derive(Debug, Error, PartialEq)]
pub enum DatabaseError {
    #[error("A connection to the database could not be opened: {0}")]
    PoolError(String),

    #[error("Failed to run query: {0}")]
    QueryError(String),

    #[error("Failed to execute the task")]
    TaskError(String),

    #[error("Cannot join event as names have already been drawn")]
    JoinEventError(),
}

impl From<r2d2::Error> for DatabaseError {
    fn from(e: r2d2::Error) -> Self {
        DatabaseError::PoolError(e.to_string())
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(e: rusqlite::Error) -> Self {
        DatabaseError::QueryError(e.to_string())
    }
}

impl From<JoinError> for DatabaseError {
    fn from(e: JoinError) -> Self {
        DatabaseError::TaskError(e.to_string())
    }
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;

pub fn establish_connection(db_path: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(db_path);
    Pool::new(manager).expect("Failed to create pool.")
}

#[derive(Clone)]
pub struct BotDatabase {
    pool: DbPool,
}

impl BotDatabase {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn insert_user(&self, user_id: u64, username: String) -> DatabaseResult<()> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone.get()?;
        conn.execute(
            "
            INSERT OR IGNORE INTO users (user_id, username)
            VALUES (?1, ?2);
        ",
            params![user_id, username],
        )?;
        Ok(())
    }
}

#[async_trait]
impl GotdTrait for BotDatabase {
    async fn insert_gif(&self, user_id: u64, username: String, url: String) -> DatabaseResult<()> {
        self.insert_user(user_id, username.clone())?;

        let pool_clone = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool_clone.get()?;

            conn.execute(
                "
                INSERT INTO gifs (submitted_by, url, posts)
                VALUES (?1, ?2, 0);
            ",
                params![user_id, url],
            )?;
            Ok(())
        })
        .await?
    }
    async fn select_random_gif(&self) -> DatabaseResult<(u64, String)> {
        let pool_clone = self.pool.clone();
        tokio::task::spawn_blocking(move || -> DatabaseResult<(u64, String)> {
            let conn = pool_clone.get()?;

            let gif_stmt = "
                SELECT submitted_by, url
                FROM gifs
                WHERE gifs.posts = (SELECT MIN(posts) FROM gifs)
                ORDER BY RANDOM()
                LIMIT 1;
            ";

            let (gif_submitter, gif_url): (u64, String) =
                conn.query_row(gif_stmt, params![], |row| Ok((row.get(0)?, row.get(1)?)))?;

            conn.execute(
                "
                UPDATE gifs
                SET posts = posts + 1
                WHERE url = ?1;
                ",
                params![gif_url.clone()],
            )?;

            Ok((gif_submitter, gif_url))
        })
        .await?
    }
}

#[async_trait]
impl SecretSantaTrait for BotDatabase {
    fn get_latest_giftee(&self, user_id: u64) -> DatabaseResult<Assignee> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone.get()?;

        let mut stmt = conn.prepare(
            "
            SELECT user_giftee
            FROM participation
            WHERE user = (?1) AND event = (
                SELECT MAX(event_id) FROM events
            )
        ",
        )?;

        match stmt.query_row(params![user_id], |row| row.get::<_, u64>(0)) {
            Ok(giftee_id) => Ok(Some(giftee_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(why) => Err(why.into()),
        }
    }

    fn start_new_event(&self) -> DatabaseResult<()> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone.get()?;
        conn.execute(
            // New event in the database
            "INSERT INTO events (event_id) VALUES (?1)",
            params![current_year()],
        )?;
        conn.execute(
            // Adding admin to the event
            "INSERT INTO participation (event, user) VALUES (?1, ?2);",
            params![current_year(), SECRET_ADMIN_ID],
        )?;
        Ok(())
    }

    fn is_event_open(&self) -> DatabaseResult<bool> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone.get()?;

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

    fn toggle_event_participation(
        &self,
        user_id: u64,
        username: String,
    ) -> DatabaseResult<ParticipantUpdate> {
        let pool_clone = self.pool.clone();

        let conn = pool_clone.get()?;
        if !self.is_event_open()? {
            return Err(DatabaseError::JoinEventError());
        }

        self.insert_user(user_id, username)?;

        let mut stmt = conn.prepare(
            "
            SELECT 1
            FROM participation
            WHERE user = ?1 and event = ?2
            LIMIT 1;
        ",
        )?;
        let mut iter = stmt.query_map(params![user_id, current_year()], |_row| Ok(()))?;

        let mut count_participants_stmt = conn.prepare(
            "
            SELECT COUNT(*)
            FROM participation
            WHERE event = ?1;
        ",
        )?;
        let total_participants: u64 =
            count_participants_stmt.query_row(params![current_year()], |row| row.get(0))?;

        match iter.next().transpose()?.is_some() {
            true => {
                conn.execute(
                    "
                    DELETE FROM participation
                    WHERE user = ?1 AND event = ?2
                ",
                    params![user_id, current_year()],
                )?;

                Ok(ParticipantUpdate::new(
                    total_participants,
                    ToggledParticipation::UserLeft(user_id),
                ))
            }
            false => {
                conn.execute(
                    "
                    INSERT INTO participation (event, user, user_giftee)
                    VALUES (?1, ?2, NULL);
                ",
                    params![current_year(), user_id],
                )?;
                Ok(ParticipantUpdate::new(
                    total_participants,
                    ToggledParticipation::UserJoined(user_id),
                ))
            }
        }
    }

    fn get_drawn_names(&self) -> DatabaseResult<Assignments> {
        let conn = self.pool.get()?;
        let mut prev_years_stmt = conn.prepare(
            "
            SELECT event_id
            FROM events
            WHERE event_id != ?1
            ORDER BY event_id DESC
            LIMIT ?2",
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
}
