use crate::providers::{UsageSnapshot, UsageStatus};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct Storage {
    conn: Mutex<Connection>,
}

fn status_to_str(status: &UsageStatus) -> &'static str {
    match status {
        UsageStatus::Ok => "ok",
        UsageStatus::Warning => "warning",
        UsageStatus::Rejected => "rejected",
        UsageStatus::Unavailable => "unavailable",
        UsageStatus::Error => "error",
    }
}

impl Storage {
    pub fn new(app_data_dir: &Path) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(app_data_dir).ok();
        let conn = Connection::open(app_data_dir.join("usage_history.sqlite3"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                percent_5h REAL,
                percent_7d REAL,
                reset_5h INTEGER,
                reset_7d INTEGER,
                status TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_provider_time
                ON snapshots(provider, fetched_at);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn insert(&self, snapshot: &UsageSnapshot) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        conn.execute(
            "INSERT INTO snapshots
                (provider, percent_5h, percent_7d, reset_5h, reset_7d, status, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snapshot.provider,
                snapshot.percent_5h,
                snapshot.percent_7d,
                snapshot.reset_5h,
                snapshot.reset_7d,
                status_to_str(&snapshot.status),
                snapshot.fetched_at,
            ],
        )?;
        Ok(())
    }

    /// Returns (fetched_at, percent_5h, percent_7d) points since `since`, oldest first.
    pub fn history(
        &self,
        provider: &str,
        since: i64,
    ) -> rusqlite::Result<Vec<(i64, Option<f32>, Option<f32>)>> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT fetched_at, percent_5h, percent_7d FROM snapshots
             WHERE provider = ?1 AND fetched_at >= ?2
             ORDER BY fetched_at ASC",
        )?;
        let rows = stmt
            .query_map(params![provider, since], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Drops rows older than `before` to keep the DB small.
    pub fn prune(&self, before: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        conn.execute("DELETE FROM snapshots WHERE fetched_at < ?1", params![before])?;
        Ok(())
    }
}
