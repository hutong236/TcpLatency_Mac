use rusqlite::{params, Connection};
use serde::Serialize;
use std::{fs, path::PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IncidentEvent {
    pub timestamp_ms: i64,
    pub target_id: String,
    pub target_name: String,
    pub status: String,
    pub detail: String,
}

fn database_path() -> Result<PathBuf, String> {
    let directory = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("TcpLatency");
    fs::create_dir_all(&directory).map_err(|err| err.to_string())?;
    Ok(directory.join("events.sqlite3"))
}

fn connect() -> Result<Connection, String> {
    let conn = Connection::open(database_path()?).map_err(|err| err.to_string())?;
    conn.busy_timeout(std::time::Duration::from_millis(1000)).map_err(|err| err.to_string())?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS incidents (
        id INTEGER PRIMARY KEY,
        timestamp_ms INTEGER NOT NULL,
        target_id TEXT NOT NULL,
        target_name TEXT NOT NULL,
        status TEXT NOT NULL,
        detail TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS incidents_time_idx ON incidents(timestamp_ms DESC);")
        .map_err(|err| err.to_string())?;
    Ok(conn)
}

pub(crate) fn write_event(event: IncidentEvent) -> Result<(), String> {
    let conn = connect()?;
    conn.execute("INSERT INTO incidents(timestamp_ms,target_id,target_name,status,detail)
        VALUES (?1,?2,?3,?4,?5)",
        params![event.timestamp_ms, event.target_id, event.target_name, event.status, event.detail])
        .map_err(|err| err.to_string())?;
    conn.execute("DELETE FROM incidents WHERE timestamp_ms < ?1",
        [event.timestamp_ms - 30 * 24 * 60 * 60 * 1000])
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(crate) fn recent_events(limit: u32) -> Result<Vec<IncidentEvent>, String> {
    let conn = connect()?;
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_millis().min(i64::MAX as u128) as i64
        - 30 * 24 * 60 * 60 * 1000;
    conn.execute("DELETE FROM incidents WHERE timestamp_ms < ?1", [cutoff])
        .map_err(|err| err.to_string())?;
    let mut stmt = conn.prepare("SELECT timestamp_ms,target_id,target_name,status,detail
        FROM incidents ORDER BY timestamp_ms DESC LIMIT ?1").map_err(|err| err.to_string())?;
    let rows = stmt.query_map([limit.clamp(1, 500)], |row| Ok(IncidentEvent {
        timestamp_ms: row.get(0)?, target_id: row.get(1)?, target_name: row.get(2)?,
        status: row.get(3)?, detail: row.get(4)?,
    })).map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}
