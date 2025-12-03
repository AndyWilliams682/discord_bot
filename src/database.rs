use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serenity::prelude::TypeMapKey;
use std::sync::Arc;

use crate::commands::gotd::{InsertGif, SelectRandomGif};

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
