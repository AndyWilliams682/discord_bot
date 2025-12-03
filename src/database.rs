use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serenity::prelude::TypeMapKey;
use std::sync::Arc;

use crate::commands::gotd::{InsertGif, SelectRandomGif};
use crate::commands::secret::{
    check_assignment_validation, current_year, GifteeHistory, SecretSantaTrait,
    PREV_RELEVANT_EVENTS,
};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rusqlite::{Error, Result};
use serenity::all::UserId;
use serenity::prelude::Mentionable;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct DbPoolWrapper;

impl TypeMapKey for DbPoolWrapper {
    type Value = Arc<DbPool>;
}

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

    pub fn insert_user(&self, user_id: u64, username: String) -> Result<(), String> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone.get().map_err(|e| e.to_string())?;
        conn.execute(
            "
            INSERT OR IGNORE INTO users (user_id, username)
            VALUES (?1, ?2);
        ",
            params![user_id, username],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[async_trait]
impl InsertGif for BotDatabase {
    async fn insert_gif(&self, user_id: u64, username: String, url: String) -> Result<(), String> {
        self.insert_user(user_id, username.clone())?;

        let pool_clone = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool_clone.get().map_err(|e| e.to_string())?;

            conn.execute(
                "
                INSERT INTO gifs (submitted_by, url, posts)
                VALUES (?1, ?2, 0);
            ",
                params![user_id, url],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[async_trait]
impl SelectRandomGif for BotDatabase {
    async fn select_random_gif(&self) -> Result<(u64, String), String> {
        let pool_clone = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool_clone.get().map_err(|e| e.to_string())?;

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
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[async_trait]
impl SecretSantaTrait for BotDatabase {
    fn get_latest_giftee(&self, user_id: u64) -> Result<String> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone
            .get()
            .map_err(|e| Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut stmt = conn.prepare(
            "
            SELECT user_giftee
            FROM participation
            WHERE user = (?1) AND event = (
                SELECT MAX(event_id) FROM events
            )
        ",
        )?;

        let result = stmt.query_row(params![user_id], |row| row.get::<_, i64>(0));

        match result {
            Ok(giftee_id) => {
                let uid = UserId::new(giftee_id as u64);
                Ok(format!("Your giftee is: {}", uid.mention().to_string()))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(
                "No giftee found - are you sure you're a participant for this event?".to_string(),
            ),
            Err(e) => Err(e),
        }
    }

    fn is_event_open(&self) -> Result<bool> {
        let pool_clone = self.pool.clone();
        let conn = pool_clone
            .get()
            .map_err(|e| Error::ToSqlConversionFailure(Box::new(e)))?;

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

    fn toggle_event_participation(&self, user_id: u64, username: String) -> Result<String> {
        let pool_clone = self.pool.clone();

        let res: Result<String> = (|| {
            let conn = pool_clone.get().map_err(|_| Error::QueryReturnedNoRows)?; // TODO: Better error handling
            if !self.is_event_open()? {
                return Ok(
                    "Unable to join - the names have already been drawn for this event."
                        .to_string(),
                );
            }

            self.insert_user(user_id, username)
                .map_err(|_| Error::QueryReturnedNoRows)?;

            let mut stmt = conn.prepare(
                "
                SELECT 1
                FROM participation
                WHERE user = ?1
                LIMIT 1;
            ",
            )?;
            let mut iter = stmt.query_map(params![user_id], |_row| Ok(()))?;

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
                        params![user_id, current_year()],
                    )?;
                    Ok(format!(
                        "{} has left the event. {} has {} participants",
                        UserId::new(user_id).mention(),
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
                        params![current_year(), user_id],
                    )?;
                    Ok(format!(
                        "{} has joined the event! {} has {} participants",
                        UserId::new(user_id).mention(),
                        current_year(),
                        participant_count
                    ))
                }
            }
        })();
        res
    }

    fn get_drawn_names(&self) -> Result<Vec<(u64, u64)>> {
        let conn = self.pool.get().map_err(|_| Error::QueryReturnedNoRows)?;
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
}
