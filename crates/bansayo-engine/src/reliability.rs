//! Opt-in persistent device status snapshots; no pose, camera images or athlete data.
use bansayo_contracts::{EngineMode, SystemState};
use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const PERIOD: Duration = Duration::from_secs(5);
const RETENTION_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const MAX_SNAPSHOTS: u64 = 20_000;
const MAX_PAYLOAD_BYTES: usize = 32 * 1024;
const MAX_READ: u64 = 120;

#[derive(Serialize)]
pub struct Entry {
    id: u64,
    epoch: String,
    at_ms: u64,
    t_us: u64,
    devices: serde_json::Value,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn open(path: &Path) -> Result<Connection, crate::app::EngineError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(2))?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS reliability (
        id INTEGER PRIMARY KEY, epoch TEXT NOT NULL, at_ms INTEGER NOT NULL,
        t_us INTEGER NOT NULL, devices TEXT NOT NULL
    ); CREATE INDEX IF NOT EXISTS reliability_at ON reliability(at_ms);",
    )?;
    Ok(db)
}

fn record(path: &Path, entry: Entry) -> Result<(), crate::app::EngineError> {
    let payload = serde_json::to_string(&entry.devices)?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err("Reliability snapshot exceeds payload limit".into());
    }
    let mut db = open(path)?;
    let tx = db.transaction()?;
    tx.execute(
        "INSERT INTO reliability(epoch,at_ms,t_us,devices) VALUES (?1,?2,?3,?4)",
        params![entry.epoch, entry.at_ms, entry.t_us, payload],
    )?;
    tx.execute(
        "DELETE FROM reliability WHERE at_ms < ?1",
        [entry.at_ms.saturating_sub(RETENTION_MS)],
    )?;
    tx.execute(
        "DELETE FROM reliability WHERE id <= (SELECT id FROM reliability ORDER BY id DESC LIMIT 1 OFFSET ?1)",
        [MAX_SNAPSHOTS],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn read(path: &Path, before: Option<u64>) -> Result<Vec<Entry>, crate::app::EngineError> {
    if !path.try_exists()? {
        return Ok(vec![]);
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_secs(2))?;
    let mut stmt = db.prepare(
        "SELECT id,epoch,at_ms,t_us,devices FROM reliability
        WHERE at_ms >= ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3",
    )?;
    let mut rows = stmt.query(params![
        now_ms().saturating_sub(RETENTION_MS),
        before.unwrap_or(i64::MAX as u64).min(i64::MAX as u64),
        MAX_READ
    ])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let payload: String = row.get(4)?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err("Invalid reliability snapshot size".into());
        }
        out.push(Entry {
            id: row.get(0)?,
            epoch: row.get(1)?,
            at_ms: row.get(2)?,
            t_us: row.get(3)?,
            devices: serde_json::from_str(&payload)?,
        });
    }
    out.reverse();
    Ok(out)
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join("diagnostics/reliability.sqlite")
}

pub fn spawn(
    path: PathBuf,
    mut state: watch::Receiver<SystemState>,
    clock: crate::clock::SessionClock,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<Result<(), crate::app::EngineError>> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(PERIOD);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut warned = false;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = timer.tick() => {}
            }
            let Ok(changed) = state.has_changed() else {
                break;
            };
            if !changed {
                continue;
            }
            let snapshot = state.borrow_and_update().clone();
            if !matches!(snapshot.engine.mode, EngineMode::Live) {
                continue;
            }
            let devices = snapshot
                .devices
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "device": d.descriptor.id, "source": d.descriptor.source_id, "origin": d.descriptor.origin,
                        "state": d.state, "stream": d.stream, "time_sync": d.time_sync,
                        "battery": d.battery, "signal": d.signal
                    })
                })
                .collect::<Vec<_>>();
            let entry = Entry {
                id: 0,
                epoch: clock.epoch_utc().into(),
                at_ms: now_ms(),
                t_us: clock.now_us(),
                devices: serde_json::Value::Array(devices),
            };
            let file = path.clone();
            let result = tokio::task::spawn_blocking(move || record(&file, entry)).await?;
            match result {
                Ok(()) => warned = false,
                Err(error) if !warned => {
                    tracing::warn!(%error, "Reliability history could not be written");
                    warned = true;
                }
                Err(_) => {}
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_preserves_same_timestamp_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(dir.path());
        let now = now_ms();
        for _ in 0..=MAX_READ {
            record(
                &path,
                Entry {
                    id: 0,
                    epoch: "same".into(),
                    at_ms: now,
                    t_us: 5,
                    devices: serde_json::json!([]),
                },
            )
            .unwrap();
        }
        let page = read(&path, None).unwrap();
        assert_eq!(page.len(), MAX_READ as usize);
        let older = read(&path, Some(page[0].id)).unwrap();
        assert_eq!(older.len(), 1);
        assert!(older[0].id < page[0].id);
    }

    #[test]
    fn persisted_history_survives_reopen_and_excludes_old_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(dir.path());
        let now = now_ms();
        for (at_ms, epoch) in [(now - RETENTION_MS - 1, "old"), (now - 1, "new")] {
            record(
                &path,
                Entry {
                    id: 0,
                    epoch: epoch.into(),
                    at_ms,
                    t_us: 5,
                    devices: serde_json::json!([]),
                },
            )
            .unwrap();
        }
        let rows = read(&path, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].epoch, "new");
        assert!(read(&path, Some(rows[0].id)).unwrap().is_empty());
    }
}
