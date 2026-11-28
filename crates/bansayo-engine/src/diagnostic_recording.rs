//! One bounded, engine-owned diagnostic recording; disk I/O stays off the tracking actor.
use crate::{
    clock::SessionClock,
    sources::{RuntimeMap, inspection},
};
use bansayo_contracts::{DeviceId, EngineMode, SystemState};
use serde::Serialize;
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

const MAX_SECONDS: u64 = 3600;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 32;
const POLL: Duration = Duration::from_millis(100);
const DRAIN_US: u64 = 250_000;
const NO_STOP: u64 = u64::MAX;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub running: bool,
    pub file: Option<String>,
    pub observations: u64,
    pub lost: u64,
    pub bytes: u64,
    pub reason: String,
    pub window_start_us: Option<u64>,
    pub window_end_us: Option<u64>,
}

struct Job {
    ids: Vec<DeviceId>,
    duration: Duration,
    stop: Arc<AtomicU64>,
}

#[derive(Default)]
struct Shared {
    status: Status,
    stop: Option<Arc<AtomicU64>>,
}

#[derive(Clone)]
pub struct Recorder {
    shared: Arc<Mutex<Shared>>,
    tx: mpsc::Sender<Job>,
    clock: SessionClock,
}

impl Recorder {
    pub fn status(&self) -> Status {
        self.shared
            .lock()
            .map(|s| s.status.clone())
            .unwrap_or_default()
    }

    pub fn start(&self, ids: Vec<DeviceId>, seconds: u64) -> Result<Status, &'static str> {
        if ids.is_empty() || ids.len() > 4 || !(1..=MAX_SECONDS).contains(&seconds) {
            return Err("Select 1–4 devices and a duration of 1–3600 seconds.");
        }
        let mut shared = self.shared.lock().map_err(|_| "Recorder unavailable.")?;
        if shared.status.running {
            return Err("A diagnostic recording is already running.");
        }
        let stop = Arc::new(AtomicU64::new(NO_STOP));
        self.tx
            .try_send(Job {
                ids,
                duration: Duration::from_secs(seconds),
                stop: stop.clone(),
            })
            .map_err(|_| "Recorder unavailable.")?;
        shared.stop = Some(stop);
        shared.status = Status {
            running: true,
            reason: "Starting".into(),
            ..Default::default()
        };
        Ok(shared.status.clone())
    }

    pub fn stop(&self) -> Status {
        if let Ok(shared) = self.shared.lock()
            && let Some(stop) = &shared.stop
        {
            let _ = stop.compare_exchange(
                NO_STOP,
                self.clock.now_us(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
        self.status()
    }
}

struct Environment {
    runtime: RuntimeMap,
    state: watch::Receiver<SystemState>,
    clock: SessionClock,
    dir: PathBuf,
    cancel: CancellationToken,
    shared: Arc<Mutex<Shared>>,
}

pub fn spawn(
    runtime: RuntimeMap,
    state: watch::Receiver<SystemState>,
    clock: SessionClock,
    dir: PathBuf,
    cancel: CancellationToken,
) -> (
    Recorder,
    tokio::task::JoinHandle<Result<(), crate::app::EngineError>>,
) {
    let (tx, mut rx) = mpsc::channel(1);
    let shared = Arc::new(Mutex::new(Shared::default()));
    let recorder = Recorder {
        shared: shared.clone(),
        tx,
        clock: clock.clone(),
    };
    let env = Arc::new(Environment {
        runtime,
        state,
        clock,
        dir: dir.join("diagnostics"),
        cancel,
        shared,
    });
    let task = tokio::spawn(async move {
        loop {
            let job = tokio::select! {
                _ = env.cancel.cancelled() => break,
                job = rx.recv() => match job {
                    Some(job) => job,
                    None => break,
                },
            };
            let worker = env.clone();
            let result = tokio::task::spawn_blocking(move || run(&worker, job)).await;
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e.to_string()),
                Err(e) => Some(e.to_string()),
            };
            if let Ok(mut shared) = env.shared.lock() {
                shared.status.running = false;
                shared.stop = None;
                if let Some(error) = error {
                    tracing::error!(%error, "Diagnostic recording failed");
                    shared.status.reason = format!("Failed: {error}");
                }
            }
        }
        Ok(())
    });
    (recorder, task)
}

fn publish(env: &Environment, status: &Status) {
    if let Ok(mut shared) = env.shared.lock() {
        shared.status = status.clone();
    }
}

fn open(dir: &Path) -> io::Result<(BufWriter<fs::File>, String)> {
    fs::create_dir_all(dir)?;
    let count = fs::read_dir(dir)?.count();
    if count >= MAX_FILES {
        return Err(io::Error::other(
            "Diagnostic directory limit reached; archive old files.",
        ));
    }
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let name = format!("capture-{:016x}.jsonl", u64::from_le_bytes(random));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{name}.partial")))?;
    Ok((BufWriter::new(file), name))
}

fn line(
    writer: &mut BufWriter<fs::File>,
    status: &mut Status,
    value: &impl Serialize,
) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    status.bytes += bytes.len() as u64 + 1;
    Ok(())
}

fn run(env: &Environment, job: Job) -> io::Result<()> {
    let (mut writer, name) = open(&env.dir)?;
    let mut status = Status {
        running: true,
        file: Some(format!("diagnostics/{name}.partial")),
        reason: "Recording".into(),
        ..Default::default()
    };
    let mut cursors: Vec<_> = job
        .ids
        .iter()
        .map(|id| {
            let b = inspection::batch(&env.runtime, id, None, 0);
            (b.epoch, b.next)
        })
        .collect();
    let started = env.clock.now_us();
    let scheduled_end = started.saturating_add(job.duration.as_secs() * 1_000_000);
    status.window_start_us = Some(started);
    status.window_end_us = Some(scheduled_end);
    publish(env, &status);
    line(
        &mut writer,
        &mut status,
        &json!({
            "type": "header", "format": "bansayo-diagnostic-journal-v1",
            "session_epoch_utc": env.clock.epoch_utc(),
            "started_utc": crate::clock::utc_now_rfc3339(),
            "devices": job.ids, "duration_s": job.duration.as_secs(),
            "window_start_us": started, "scheduled_end_us": scheduled_end
        }),
    )?;
    let reason = 'record: loop {
        if env.cancel.is_cancelled() {
            break "Engine shutdown";
        }
        if matches!(env.state.borrow().engine.mode, EngineMode::Replay) {
            break "Engine entered replay";
        }
        for (id, cursor) in job.ids.iter().zip(&mut cursors) {
            let batch = inspection::batch(&env.runtime, id, Some(cursor.0), cursor.1);
            if batch.reset || batch.epoch == 0 {
                break 'record "Stream reference reset";
            }
            *cursor = (batch.epoch, batch.next);
            if batch.entries.is_empty() && batch.lost == 0 {
                continue;
            }
            let count = batch.entries.len() as u64;
            let lost = batch.lost;
            let value = json!({"type":"batch","device":id,"batch":batch});
            let size = serde_json::to_vec(&value).map_err(io::Error::other)?.len() as u64;
            if status.bytes + size + 4096 > MAX_BYTES {
                break 'record "File size limit reached";
            }
            line(&mut writer, &mut status, &value)?;
            status.observations += count;
            status.lost += lost;
        }
        let requested_stop = job.stop.load(Ordering::Relaxed);
        let end = scheduled_end.min(requested_stop).max(started);
        status.window_end_us = Some(end);
        let now = env.clock.now_us();
        if now >= end {
            status.reason = "Finalizing".into();
        }
        writer.flush()?;
        publish(env, &status);
        if now >= end.saturating_add(DRAIN_US) {
            break if requested_stop < scheduled_end {
                "Stopped by operator"
            } else {
                "Duration completed"
            };
        }
        std::thread::sleep(POLL);
    };
    let ended = scheduled_end
        .min(job.stop.load(Ordering::Relaxed))
        .min(env.clock.now_us())
        .max(started);
    status.window_end_us = Some(ended);
    let footer = json!({
        "type":"footer","reason":reason,"observations":status.observations,"lost":status.lost,
        "window_end_us":ended,"late_arrival_drain_us":DRAIN_US
    });
    line(&mut writer, &mut status, &footer)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    fs::rename(env.dir.join(format!("{name}.partial")), env.dir.join(&name))?;
    status.file = Some(format!("diagnostics/{name}"));
    status.reason = reason.into();
    publish(env, &status);
    Ok(())
}
