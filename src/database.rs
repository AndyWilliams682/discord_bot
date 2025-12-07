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

    fn is_user_participating(
        &self,
        conn: &rusqlite::Connection,
        user_id: u64,
        event_id: i32,
    ) -> DatabaseResult<bool> {
        let mut stmt =
            conn.prepare("SELECT 1 FROM participation WHERE user = ?1 and event = ?2 LIMIT 1;")?;
        Ok(stmt.exists(params![user_id, event_id])?)
    }

    fn get_participant_count(
        &self,
        conn: &rusqlite::Connection,
        event_id: i32,
    ) -> DatabaseResult<u64> {
        conn.query_row(
            "SELECT COUNT(*) FROM participation WHERE event = ?1;",
            params![event_id],
            |row| row.get(0),
        )
        .map_err(DatabaseError::from)
    }

    fn add_participant(
        &self,
        conn: &rusqlite::Connection,
        user_id: u64,
        event_id: i32,
    ) -> DatabaseResult<()> {
        conn.execute(
            "INSERT INTO participation (event, user, user_giftee) VALUES (?1, ?2, NULL);",
            params![event_id, user_id],
        )?;
        Ok(())
    }

    fn remove_participant(
        &self,
        conn: &rusqlite::Connection,
        user_id: u64,
        event_id: i32,
    ) -> DatabaseResult<()> {
        conn.execute(
            "DELETE FROM participation WHERE user = ?1 AND event = ?2;",
            params![user_id, event_id],
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
            let stmt = "
                UPDATE gifs
                SET posts = posts + 1
                WHERE url = (
                    SELECT url
                    FROM gifs
                    WHERE posts = (SELECT MIN(posts) FROM gifs)
                    ORDER BY RANDOM()
                    LIMIT 1
                )
                RETURNING submitted_by, url;
            ";

            let (gif_submitter, gif_url): (u64, String) =
                conn.query_row(stmt, params![], |row| Ok((row.get(0)?, row.get(1)?)))?;

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
        let mut conn = pool_clone.get()?;
        let tx = conn.transaction()?;
        let year = current_year();

        tx.execute("INSERT INTO events (event_id) VALUES (?1)", params![year])?;
        tx.execute(
            "INSERT INTO participation (event, user) VALUES (?1, ?2);",
            params![year, SECRET_ADMIN_ID],
        )?;
        tx.commit()?;
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
        if !self.is_event_open()? {
            return Err(DatabaseError::JoinEventError());
        }

        self.insert_user(user_id, username)?;

        let conn = self.pool.get()?;
        let year = current_year();
        let count = self.get_participant_count(&conn, year)?;

        if self.is_user_participating(&conn, user_id, year)? {
            self.remove_participant(&conn, user_id, year)?;
            Ok(ParticipantUpdate::new(
                count,
                ToggledParticipation::UserLeft(user_id),
            ))
        } else {
            self.add_participant(&conn, user_id, year)?;
            Ok(ParticipantUpdate::new(
                count,
                ToggledParticipation::UserJoined(user_id),
            ))
        }
    }

    fn get_drawn_names(&self) -> DatabaseResult<Assignments> {
        let conn = self.pool.get()?;

        let prev_years = get_previous_event_ids(&conn)?;
        let current_participants = get_current_event_participants(&conn)?;
        let giftee_history = get_giftee_history(&conn, &current_participants, &prev_years)?;

        let solution = solve_assignments(current_participants.len(), &giftee_history);

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

        save_assignments(&conn, &assignments)?;

        Ok(assignments)
    }
}

fn get_previous_event_ids(conn: &rusqlite::Connection) -> DatabaseResult<Vec<i32>> {
    let mut stmt = conn.prepare(
        "
        SELECT event_id
        FROM events
        WHERE event_id != ?1
        ORDER BY event_id DESC
        LIMIT ?2",
    )?;
    let result = stmt
        .query_map(params![current_year(), PREV_RELEVANT_EVENTS], |row| {
            row.get::<_, i32>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}

fn get_current_event_participants(conn: &rusqlite::Connection) -> DatabaseResult<Vec<u64>> {
    let mut stmt = conn.prepare(
        "
    SELECT user
    FROM participation
    WHERE event = ?1
",
    )?;
    let result = stmt
        .query_map(params![current_year()], |row| row.get::<_, u64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}

fn get_giftee_history(
    conn: &rusqlite::Connection,
    current_participants: &[u64],
    prev_years: &[i32],
) -> DatabaseResult<Vec<[usize; PREV_RELEVANT_EVENTS]>> {
    let num_participants = current_participants.len();
    let mut giftee_history: Vec<[usize; PREV_RELEVANT_EVENTS]> =
        vec![[num_participants; PREV_RELEVANT_EVENTS]; num_participants];

    if prev_years.is_empty() {
        return Ok(giftee_history);
    }

    let mut stmt = conn.prepare(
        "
    SELECT user, user_giftee, event
    FROM participation
    WHERE event < ?1 AND event >= ?2
    ORDER BY event DESC
",
    )?;
    let giftee_history_iter = stmt
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
    Ok(giftee_history)
}

fn solve_assignments(
    num_participants: usize,
    giftee_history: &Vec<[usize; PREV_RELEVANT_EVENTS]>,
) -> Vec<usize> {
    let mut solution: Vec<usize> = (0..num_participants).collect();
    let mut viable_solution = false;
    let mut rng = thread_rng();
    while !viable_solution {
        solution.shuffle(&mut rng);
        viable_solution = check_assignment_validation(&solution, giftee_history);
    }
    solution
}

fn save_assignments(conn: &rusqlite::Connection, assignments: &[(u64, u64)]) -> DatabaseResult<()> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_assignments_logic() {
        let num_participants = 5;
        let mut history = vec![[num_participants; PREV_RELEVANT_EVENTS]; num_participants];

        // Let's say user 0 cannot be gifted to user 1
        history[0][0] = 1;

        let solution = solve_assignments(num_participants, &history);

        assert_eq!(solution.len(), num_participants);
        // Ensure everyone is assigned to someone
        let mut giftees = solution.clone();
        giftees.sort();
        let expected: Vec<usize> = (0..num_participants).collect();
        assert_eq!(giftees, expected);

        // Ensure self-assignment didn't happen (handled by check_assignment_validation)
        for (i, &giftee) in solution.iter().enumerate() {
            assert_ne!(i, giftee);
        }

        // Ideally we should also check if the restriction is respected,
        // but since we are mocking history and check_assignment_validation
        // logic is what we rely on, verifying it calls it and finds a valid solution is good.
        // If check_assignment_validation is working, the solution generated will be valid.

        // Check manually for our restriction
        assert_ne!(
            solution[0], 1,
            "User 0 should not be assigned to User 1 due to history"
        );
    }

    fn setup_test_db() -> BotDatabase {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager).unwrap();
        let db = BotDatabase::new(pool);

        db.pool
            .get()
            .unwrap()
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS users (
                user_id INTEGER PRIMARY KEY, 
                username TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gifs (
                submitted_by INTEGER NOT NULL, 
                url TEXT NOT NULL, 
                posts INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS events (
                event_id INTEGER PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS participation (
                event INTEGER NOT NULL, 
                user INTEGER NOT NULL, 
                user_giftee INTEGER
            );
            ",
            )
            .unwrap();

        db
    }

    #[tokio::test]
    async fn test_insert_and_retrieve_gif() {
        let db = setup_test_db();

        db.insert_gif(
            123,
            "TestUser".to_string(),
            "http://example.com/cat.gif".to_string(),
        )
        .await
        .unwrap();

        db.insert_gif(
            123,
            "TestUser".to_string(),
            "http://example.com/posted.gif".to_string(),
        )
        .await
        .unwrap();
        db.pool
            .get()
            .unwrap()
            .execute(
                "
            UPDATE gifs SET posts = posts + 1 WHERE url = ?1;
            ",
                params!["http://example.com/posted.gif"],
            )
            .unwrap();

        let (user, url) = db.select_random_gif().await.unwrap();
        assert_eq!(user, 123);
        assert_eq!(url, "http://example.com/cat.gif");
    }

    #[tokio::test]
    async fn test_secret_santa_flow() {
        let db = setup_test_db();

        db.start_new_event().unwrap();

        let is_open = db.is_event_open().unwrap();
        assert!(is_open);

        db.toggle_event_participation(1, "User1".to_string())
            .unwrap();
        db.toggle_event_participation(2, "User2".to_string())
            .unwrap();

        db.pool
            .get()
            .unwrap()
            .execute_batch(
                "
            INSERT INTO events (event_id) VALUES (2000);
            INSERT INTO events (event_id) VALUES (2001);
            INSERT INTO events (event_id) VALUES (2002);
        ",
            )
            .unwrap();

        let assignments = db.get_drawn_names().unwrap();

        assert_eq!(assignments.len(), 3);

        for (user, giftee) in &assignments {
            assert_ne!(user, giftee, "User {} was assigned to themselves!", user);
        }
    }
}
