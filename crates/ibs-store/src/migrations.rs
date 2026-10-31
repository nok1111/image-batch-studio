use crate::StoreError;
use rusqlite::Connection;

/// Ordered schema migrations; index + 1 is the resulting `user_version`.
/// Never edit a released migration, only append new ones.
const MIGRATIONS: &[&str] = &["CREATE TABLE settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    );"];

pub const LATEST: usize = MIGRATIONS.len();

pub fn run(conn: &mut Connection) -> Result<(), StoreError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > LATEST as i64 {
        return Err(StoreError::UnsupportedSchema {
            found: current,
            supported: LATEST as i64,
        });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}
