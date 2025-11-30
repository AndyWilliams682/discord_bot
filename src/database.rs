use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serenity::prelude::TypeMapKey;
use std::sync::Arc;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct DbPoolWrapper;

impl TypeMapKey for DbPoolWrapper {
    type Value = Arc<DbPool>;
}

pub fn establish_connection(db_path: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(db_path);
    Pool::new(manager).expect("Failed to create pool.")
}
