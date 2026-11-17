//! Analytics store (martial-arts 12 §4, ADR-029): one SQLite file opened only by the engine,
//! WAL, foreign keys, secure_delete, numbered migrations tracked by `user_version`, and one
//! writer thread. Rows are derived from training events; measurements and scores are only
//! ever inserted (I1).
pub mod read;

use bansayo_contracts::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

/// Numbered migrations applied in order.
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_training",
    include_str!("migrations/0001_training.sql"),
)];

/// Capacity of the training → analytics writer channel (14 §3).
pub const WRITER_CAPACITY: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("migration {name} failed: {source}")]
    Migration {
        name: &'static str,
        source: rusqlite::Error,
    },
    #[error("{0}")]
    Other(String),
}

fn configure(c: &Connection) -> rusqlite::Result<()> {
    c.pragma_update(None, "foreign_keys", "ON")?;
    c.pragma_update(None, "secure_delete", "ON")?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Opens a connection; the first opener applies pending migrations.
pub fn open(path: &Path) -> Result<Connection, StoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| StoreError::Other(e.to_string()))?;
    }
    let mut c = Connection::open(path)?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    configure(&c)?;
    migrate(&mut c)?;
    Ok(c)
}

/// Applies migrations after `user_version`; a failure leaves the file untouched (12 §9).
pub fn migrate(c: &mut Connection) -> Result<(), StoreError> {
    let current: usize = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, (name, sql)) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = c.transaction()?;
        tx.execute_batch(sql)
            .map_err(|source| StoreError::Migration { name, source })?;
        tx.pragma_update(None, "user_version", i + 1)
            .map_err(|source| StoreError::Migration { name, source })?;
        tx.commit()?;
    }
    Ok(())
}

/// Everything written when a session is created; the session does not start unless it commits
/// (32 §5, ADR-036 decision 3).
#[derive(Debug, Clone)]
pub struct SessionBegin {
    pub session_id: SessionId,
    pub athlete_id: AthleteId,
    pub started_utc: String,
    pub plan: Plan,
    pub plan_ref: Option<String>,
    pub origin: Origin,
    pub capability_profile: CapabilityProfileId,
    pub tracking_config: (String, String, Value),
    pub calibration_set: (String, String, Value),
    pub snapshots: Vec<crate::content_store::Snapshot>,
    pub has_inputs: bool,
    pub has_motion: bool,
    pub reanalysis_of: Option<SessionId>,
    pub analysis_rev: u32,
    pub source_mask: Option<Vec<DeviceId>>,
}

pub enum Write {
    Begin(
        Box<SessionBegin>,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    ),
    Event(SessionId, Box<TrainingEvent>, String),
    Rpe(
        SessionId,
        u8,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    ),
    Delete(
        SessionId,
        tokio::sync::oneshot::Sender<Result<bool, String>>,
    ),
    /// Rows still `active` from an earlier run become `interrupted` (12 §4).
    Interrupt,
    Flush(tokio::sync::oneshot::Sender<()>),
}

/// Handle on the writer thread. `try_write` never blocks.
#[derive(Clone)]
pub struct Writer {
    tx: SyncSender<Write>,
    pub path: PathBuf,
}

impl Writer {
    pub fn spawn(path: PathBuf) -> Result<Self, StoreError> {
        let conn = open(&path)?;
        let (tx, rx) = sync_channel(WRITER_CAPACITY);
        std::thread::Builder::new()
            .name("analytics-writer".into())
            .spawn(move || run(conn, rx))
            .map_err(|e| StoreError::Other(e.to_string()))?;
        let w = Self { tx, path };
        let _ = w.try_write(Write::Interrupt);
        Ok(w)
    }

    /// `Err` gives the item back when the queue is full or the writer stopped.
    pub fn try_write(&self, w: Write) -> Result<(), Write> {
        self.tx.try_send(w).map_err(|e| match e {
            TrySendError::Full(w) | TrySendError::Disconnected(w) => w,
        })
    }

    /// Blocking send for the off-runtime paths (session end flush, deletes).
    pub fn send(&self, w: Write) -> bool {
        self.tx.send(w).is_ok()
    }

    pub fn read(&self) -> Result<Connection, StoreError> {
        let c = Connection::open(&self.path)?;
        configure(&c)?;
        Ok(c)
    }
}

fn run(mut conn: Connection, rx: Receiver<Write>) {
    let mut sets: std::collections::BTreeMap<String, u32> = Default::default();
    for w in rx {
        match w {
            Write::Begin(b, reply) => {
                let _ = reply.send(begin(&mut conn, &b).map_err(|e| e.to_string()));
            }
            Write::Event(id, e, title) => {
                if let Err(err) = event(&mut conn, &id, &e, &title, &mut sets) {
                    tracing::error!(session=%id, seq=e.event_seq, %err, "analytics write failed");
                }
            }
            Write::Rpe(id, rpe, reply) => {
                let r = conn
                    .execute("UPDATE training_sessions SET rpe = ?2 WHERE id = ?1 AND status != 'active'", params![id.as_str(), rpe])
                    .map_err(|e| e.to_string())
                    .and_then(|n| if n == 1 { Ok(()) } else { Err("not found or still running".into()) });
                let _ = reply.send(r);
            }
            Write::Delete(id, reply) => {
                let _ = reply.send(delete(&mut conn, &id).map_err(|e| e.to_string()));
            }
            Write::Interrupt => {
                if let Err(err) = conn.execute(
                    "UPDATE training_sessions SET status = 'interrupted' WHERE status = 'active'",
                    [],
                ) {
                    tracing::error!(%err, "marking interrupted sessions failed");
                }
            }
            Write::Flush(reply) => {
                let _ = reply.send(());
            }
        }
    }
}

/// Removes a session, its re-analyses and the records that point at them.
fn delete(conn: &mut Connection, id: &SessionId) -> rusqlite::Result<bool> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM personal_records WHERE session_id = ?1 OR session_id IN (SELECT id FROM training_sessions WHERE reanalysis_of = ?1)",
        params![id.as_str()],
    )?;
    let n = tx.execute(
        "DELETE FROM training_sessions WHERE id = ?1 OR reanalysis_of = ?1",
        params![id.as_str()],
    )?;
    tx.commit()?;
    Ok(n > 0)
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn text<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn begin(conn: &mut Connection, b: &SessionBegin) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO athletes (id, created_utc) VALUES (?1, ?2)",
        params![b.athlete_id.as_str(), b.started_utc],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO tracking_configs (id, profile, body_json, first_used_utc) VALUES (?1, ?2, ?3, ?4)",
        params![b.tracking_config.0, b.tracking_config.1, crate::content_store::canonical(&b.tracking_config.2), b.started_utc],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO calibration_sets (id, qa, body_json, first_used_utc) VALUES (?1, ?2, ?3, ?4)",
        params![b.calibration_set.0, b.calibration_set.1, crate::content_store::canonical(&b.calibration_set.2), b.started_utc],
    )?;
    for s in &b.snapshots {
        tx.execute(
            "INSERT OR IGNORE INTO content_snapshots (hash, kind, id, version, body_json, first_used_utc) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![s.hash, s.kind, s.id, s.version, s.body_json, b.started_utc],
        )?;
    }
    tx.execute(
        "INSERT INTO training_sessions (id, athlete_id, started_utc, status, plan_kind, plan_ref, plan_json, origin, engine_version, observables_rev, capability_profile, tracking_config_id, calibration_set_id, has_inputs, has_motion, analysis_rev, reanalysis_of, source_mask_json)
         VALUES (?1, ?2, ?3, 'active', 'technique', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            b.session_id.as_str(),
            b.athlete_id.as_str(),
            b.started_utc,
            b.plan_ref,
            json(&b.plan),
            text(&b.origin),
            env!("CARGO_PKG_VERSION"),
            bansayo_training::observables::OBSERVABLES_REV,
            text(&b.capability_profile),
            b.tracking_config.0,
            b.calibration_set.0,
            b.has_inputs,
            b.has_motion,
            b.analysis_rev,
            b.reanalysis_of.as_ref().map(|s| s.as_str().to_owned()),
            b.source_mask.as_ref().map(json),
        ],
    )?;
    for s in &b.snapshots {
        tx.execute(
            "INSERT OR IGNORE INTO session_content (session_id, snapshot_hash) VALUES (?1, ?2)",
            params![b.session_id.as_str(), s.hash],
        )?;
    }
    tx.commit()
}

fn split_ref(r: &str) -> (String, u32) {
    match r.split_once('@') {
        Some((id, v)) => (id.to_owned(), v.parse().unwrap_or(0)),
        None => (r.to_owned(), 0),
    }
}

fn rep_rows(
    tx: &rusqlite::Transaction,
    sid: &str,
    set_index: u32,
    r: &RepetitionResult,
) -> rusqlite::Result<()> {
    let rep = &r.repetition;
    let (tid, tver) = split_ref(&rep.technique);
    tx.execute(
        "INSERT INTO reps (session_id, rep_index, set_index, technique_id, technique_version, side, stance, outcome, t_start_us, t_end_us, strike_t_us, recognition_confidence, capability_profile, phases_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![sid, rep.rep_index, set_index, tid, tver, text(&rep.side), rep.stance.map(|s| text(&s)), text(&rep.outcome),
                rep.t_start_us as i64, rep.t_end_us as i64, rep.strike_t_us.map(|t| t as i64), rep.recognition_confidence,
                text(&rep.capability_profile), json(&rep.phases)],
    )?;
    for m in &r.measurements {
        let (mid, mver) = split_ref(&m.metric);
        tx.execute(
            "INSERT INTO measurements (session_id, scope, rep_index, set_index, technique_id, side, metric_id, metric_version, value, norm, unit, grade, confidence, sigma, sigma_basis, method, provenance, comparability_class, reasons_json)
             VALUES (?1, 'rep', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![sid, rep.rep_index, set_index, tid, text(&rep.side), mid, mver, m.value, m.norm, m.unit, text(&m.grade),
                    m.evidence.confidence, m.evidence.sigma, m.evidence.sigma_basis.map(|b| text(&b)), text(&m.evidence.method),
                    text(&m.evidence.provenance), m.comparability_class, json(&m.reasons)],
        )?;
    }
    if let Some(s) = &r.score {
        let (mid, mver) = split_ref(&s.model);
        tx.execute(
            "INSERT INTO scores (session_id, rep_index, model_id, model_version, reference_id, reference_match, coverage, comparability_class, components_json) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8)",
            params![sid, rep.rep_index, mid, mver, s.reference_match, s.coverage, s.comparability_class, json(&s.components)],
        )?;
    }
    Ok(())
}

fn event(
    conn: &mut Connection,
    id: &SessionId,
    e: &TrainingEvent,
    title: &str,
    sets: &mut std::collections::BTreeMap<String, u32>,
) -> rusqlite::Result<()> {
    let sid = id.as_str();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO events (session_id, event_seq, t_us, type, body_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sid, e.event_seq as i64, e.t_us as i64, e.event_type, json(e)],
    )?;
    let d = &e.data;
    match e.event_type.as_str() {
        "set_started" => {
            let set = d["set_index"].as_u64().unwrap_or(0) as u32;
            sets.insert(sid.to_owned(), set);
            tx.execute(
                "INSERT OR IGNORE INTO sets (session_id, set_index, block_id, item_id, exercise_kind, content_ref, side, target_json, status, started_t_us)
                 VALUES (?1, ?2, 'main', 'main', 'technique', ?3, ?4, ?5, 'abandoned', ?6)",
                params![sid, set, d["technique"].as_str(), d["side"].as_str(), d["target"].to_string(), e.t_us as i64],
            )?;
        }
        "set_completed" => {
            tx.execute(
                "UPDATE sets SET status = ?3, ended_t_us = ?4, summary_json = ?5 WHERE session_id = ?1 AND set_index = ?2",
                params![sid, d["set_index"].as_u64().unwrap_or(0) as i64, d["status"].as_str().unwrap_or("abandoned"), e.t_us as i64, d.to_string()],
            )?;
        }
        "rep_completed" | "rep_rejected" => {
            let body = if e.event_type == "rep_completed" {
                d.clone()
            } else {
                d["result"].clone()
            };
            if let Ok(r) = serde_json::from_value::<RepetitionResult>(body) {
                let set = sets.get(sid).copied().or(e.position.set).unwrap_or(0);
                rep_rows(&tx, sid, set, &r)?;
            }
        }
        "session_completed" | "session_aborted" => {
            let status = if e.event_type == "session_completed" {
                "completed"
            } else {
                "aborted"
            };
            let mut summary = d.clone();
            summary["title"] = Value::from(title);
            tx.execute(
                "UPDATE training_sessions SET status = ?2, ended_utc = ?3, active_ms = ?4, summary_json = ?5 WHERE id = ?1",
                params![sid, status, crate::clock::utc_now_rfc3339(), d["totals"]["active_ms"].as_i64(), summary.to_string()],
            )?;
            records(&tx, sid)?;
            sets.remove(sid);
        }
        _ => {}
    }
    tx.commit()
}

/// Personal records from this session's grade-A repetition values (12 §6 "best valid").
fn records(tx: &rusqlite::Transaction, sid: &str) -> rusqlite::Result<()> {
    let athlete: Option<String> = tx
        .query_row(
            "SELECT athlete_id FROM training_sessions WHERE id = ?1 AND origin = 'live'",
            params![sid],
            |r| r.get(0),
        )
        .optional()?;
    let Some(athlete) = athlete else {
        return Ok(());
    };
    let mut q = tx.prepare(
        "SELECT m.technique_id, m.side, m.metric_id, m.comparability_class, m.rep_index, m.value, m.unit FROM measurements m
         WHERE m.session_id = ?1 AND m.grade = 'A' AND m.value IS NOT NULL",
    )?;
    let rows: Vec<(String, String, String, String, Option<u32>, f64)> = q
        .query_map(params![sid], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let direction = |metric: &str| -> String {
        tx.query_row(
            "SELECT json_extract(c.body_json, '$.direction') FROM content_snapshots c JOIN session_content s ON s.snapshot_hash = c.hash
             WHERE s.session_id = ?1 AND c.kind = 'metrics' AND c.id = ?2",
            params![sid, metric],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or_default()
    };
    for (technique, side, metric, class, rep, value) in rows {
        let lower = direction(&metric) == "lower_is_more";
        let current: Option<f64> = tx
            .query_row(
                "SELECT value FROM personal_records WHERE athlete_id = ?1 AND technique_id = ?2 AND side = ?3 AND metric_id = ?4 AND comparability_class = ?5",
                params![athlete, technique, side, metric, class],
                |r| r.get(0),
            )
            .optional()?;
        let better = current.is_none_or(|c| if lower { value < c } else { value > c });
        if better {
            tx.execute(
                "INSERT OR REPLACE INTO personal_records (athlete_id, technique_id, side, metric_id, comparability_class, session_id, rep_index, value, set_utc) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![athlete, technique, side, metric, class, sid, rep, value, crate::clock::utc_now_rfc3339()],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
