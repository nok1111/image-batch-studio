//! Batch history: one row per finished job, powering the dashboard.

use crate::{Store, StoreError};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    pub module: String,
    pub files: i64,
    pub ok: i64,
    pub failed: i64,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub output_dir: String,
    /// Frontend blob (module + options + inputs) for "repeat".
    pub meta: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStats {
    pub batches: i64,
    pub files: i64,
    pub ok: i64,
    pub failed: i64,
    pub bytes_in: i64,
    pub bytes_out: i64,
    /// Most-run module id.
    pub top_module: Option<String>,
}

const KEEP: i64 = 200;

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub fn record_batch(
        &self,
        module: &str,
        files: i64,
        ok: i64,
        failed: i64,
        bytes_in: i64,
        bytes_out: i64,
        output_dir: &str,
        meta: &str,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO job_history
             (at, module, files, ok, failed, bytes_in, bytes_out, output_dir, meta)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![at, module, files, ok, failed, bytes_in, bytes_out, output_dir, meta],
        )?;
        conn.execute(
            "DELETE FROM job_history WHERE id NOT IN
             (SELECT id FROM job_history ORDER BY id DESC LIMIT ?1)",
            [KEEP],
        )?;
        Ok(())
    }

    pub fn recent_batches(&self, limit: i64) -> Result<Vec<HistoryRow>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let mut st = conn.prepare(
            "SELECT id, at, module, files, ok, failed, bytes_in, bytes_out, output_dir, meta
             FROM job_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = st
            .query_map([limit], |r| {
                Ok(HistoryRow {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    module: r.get(2)?,
                    files: r.get(3)?,
                    ok: r.get(4)?,
                    failed: r.get(5)?,
                    bytes_in: r.get(6)?,
                    bytes_out: r.get(7)?,
                    output_dir: r.get(8)?,
                    meta: r.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn batch_stats(&self) -> Result<HistoryStats, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let (batches, files, ok, failed, bin, bout) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(files),0), COALESCE(SUM(ok),0),
                    COALESCE(SUM(failed),0), COALESCE(SUM(bytes_in),0),
                    COALESCE(SUM(bytes_out),0)
             FROM job_history",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;
        let top_module = conn
            .query_row(
                "SELECT module FROM job_history GROUP BY module
                 ORDER BY COUNT(*) DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        Ok(HistoryStats {
            batches,
            files,
            ok,
            failed,
            bytes_in: bin,
            bytes_out: bout,
            top_module,
        })
    }

    pub fn delete_batch(&self, id: i64) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.execute("DELETE FROM job_history WHERE id = ?1", [id])?;
        Ok(())
    }
}
