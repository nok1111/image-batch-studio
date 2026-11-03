use crate::StoreError;
use rusqlite::Connection;

/// Ordered schema migrations; index + 1 is the resulting `user_version`.
/// Never edit a released migration, only append new ones.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    );",
    r#"CREATE TABLE resize_presets (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        name     TEXT NOT NULL,
        spec     TEXT NOT NULL,
        position INTEGER NOT NULL
    );
    INSERT INTO resize_presets (name, spec, position) VALUES
        ('Instagram — square', '{"mode":"fill","width":1080,"height":1080,"percent":100,"allowUpscale":false}', 0),
        ('Instagram — portrait', '{"mode":"fill","width":1080,"height":1350,"percent":100,"allowUpscale":false}', 1),
        ('Instagram / TikTok — story', '{"mode":"fill","width":1080,"height":1920,"percent":100,"allowUpscale":false}', 2),
        ('Facebook — post', '{"mode":"fill","width":1200,"height":630,"percent":100,"allowUpscale":false}', 3),
        ('X — post', '{"mode":"fill","width":1600,"height":900,"percent":100,"allowUpscale":false}', 4),
        ('LinkedIn — post', '{"mode":"fill","width":1200,"height":627,"percent":100,"allowUpscale":false}', 5),
        ('YouTube — thumbnail', '{"mode":"fill","width":1280,"height":720,"percent":100,"allowUpscale":false}', 6),
        ('Full HD (fit)', '{"mode":"fit","width":1920,"height":1080,"percent":100,"allowUpscale":false}', 7),
        ('Web — 1200 px wide', '{"mode":"width","width":1200,"height":0,"percent":100,"allowUpscale":false}', 8);"#,
    r#"CREATE TABLE watermark_presets (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        name     TEXT NOT NULL,
        spec     TEXT NOT NULL,
        position INTEGER NOT NULL
    );"#,
    r#"CREATE TABLE watermark_images (
        path TEXT PRIMARY KEY NOT NULL
    );"#,
    r#"CREATE TABLE job_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        at INTEGER NOT NULL,
        module TEXT NOT NULL DEFAULT '',
        files INTEGER NOT NULL DEFAULT 0,
        ok INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        bytes_in INTEGER NOT NULL DEFAULT 0,
        bytes_out INTEGER NOT NULL DEFAULT 0,
        output_dir TEXT NOT NULL DEFAULT '',
        meta TEXT NOT NULL DEFAULT ''
    )"#,
];

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
