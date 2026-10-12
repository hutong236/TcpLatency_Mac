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
    CREATE INDEX IF NOT EXISTS incidents_time_idx ON incidents(timestamp_ms DESC);
    CREATE TABLE IF NOT EXISTS hourly_aggregate (
        target_id TEXT NOT NULL, hour_ms INTEGER NOT NULL,
        sample_count INTEGER NOT NULL DEFAULT 0,
        success_count INTEGER NOT NULL DEFAULT 0,
        latency_sum REAL NOT NULL DEFAULT 0,
        latency_min REAL, latency_max REAL,
        PRIMARY KEY (target_id, hour_ms)
    );")
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HourlyAggregate {
    pub(crate) hour_ms: i64,
    pub(crate) sample_count: i64,
    pub(crate) success_count: i64,
    pub(crate) average_ms: Option<f64>,
    pub(crate) min_ms: Option<f64>,
    pub(crate) max_ms: Option<f64>,
}

pub(crate) fn write_sample(target_id: String, timestamp_ms: i64, latency_ms: Option<f64>) -> Result<(), String> {
    let conn = connect()?;
    let hour = timestamp_ms.div_euclid(3_600_000) * 3_600_000;
    conn.execute(
        "INSERT INTO hourly_aggregate(target_id,hour_ms,sample_count,success_count,latency_sum,latency_min,latency_max)
         VALUES (?1,?2,1,?3,?4,?5,?5)
         ON CONFLICT(target_id,hour_ms) DO UPDATE SET
           sample_count=sample_count+1,
           success_count=success_count+excluded.success_count,
           latency_sum=latency_sum+excluded.latency_sum,
           latency_min=CASE WHEN excluded.latency_min IS NULL THEN latency_min
             WHEN latency_min IS NULL THEN excluded.latency_min
             ELSE MIN(latency_min,excluded.latency_min) END,
           latency_max=CASE WHEN excluded.latency_max IS NULL THEN latency_max
             WHEN latency_max IS NULL THEN excluded.latency_max
             ELSE MAX(latency_max,excluded.latency_max) END",
        params![target_id,hour,if latency_ms.is_some() {1} else {0},
            latency_ms.unwrap_or(0.0),latency_ms],
    ).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM hourly_aggregate WHERE hour_ms < ?1",
        [hour - 30 * 24 * 3_600_000]).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn hourly_history(target_id: String, limit: u32) -> Result<Vec<HourlyAggregate>, String> {
    let conn = connect()?;
    let mut stmt = conn.prepare(
        "SELECT hour_ms,sample_count,success_count,
          CASE WHEN success_count > 0 THEN latency_sum / success_count ELSE NULL END,
          latency_min,latency_max FROM hourly_aggregate
          WHERE target_id=?1 ORDER BY hour_ms DESC LIMIT ?2"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![target_id,limit.clamp(1,720)], |r| Ok(HourlyAggregate {
        hour_ms:r.get(0)?,sample_count:r.get(1)?,success_count:r.get(2)?,
        average_ms:r.get(3)?,min_ms:r.get(4)?,max_ms:r.get(5)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}
