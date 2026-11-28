use super::{Context, Item, SessionFile};
use crate::{clock::utc_now_rfc3339, registry::persistence::atomic_write};
use bansayo_contracts::*;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
};
static IDS: AtomicU64 = AtomicU64::new(0);
pub struct Recorder {
    pub state: RecordingState,
    pub tx: SyncSender<Item>,
    pub dropped: Arc<AtomicU64>,
    pub result: tokio::sync::oneshot::Receiver<Result<RecordingSession, String>>,
}
impl Recorder {
    /// Called on the blocking pool; the dedicated thread reports open failure before acknowledgement.
    pub fn start(dir: PathBuf, name: Option<String>, context: Context) -> Result<Self, String> {
        Self::start_as(dir, name, context, None)
    }

    /// As `start`, under a given session id (a training session's inputs share its id).
    pub fn start_as(
        dir: PathBuf,
        name: Option<String>,
        context: Context,
        fixed: Option<SessionId>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir.join("sessions")).map_err(|e| e.to_string())?;
        let utc = utc_now_rfc3339();
        let stamp = utc.replace(['-', ':'], "");
        let (id, path) = loop {
            if let Some(id) = fixed.clone() {
                let path = dir.join("sessions").join(id.as_str());
                std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
                if path.join("recording.mcap").exists() {
                    return Err("This session already has a recording.".into());
                }
                break (id, path);
            }
            let n = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos()) as u64
                + IDS.fetch_add(1, Ordering::Relaxed))
                % 36_u64.pow(4);
            let mut suffix = String::new();
            let mut n = n;
            for _ in 0..4 {
                suffix.push(char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
                n /= 36;
            }
            let id = SessionId::new(format!("{stamp}-{suffix}")).map_err(|e| e.to_string())?;
            let path = dir.join("sessions").join(id.as_str());
            match std::fs::create_dir(&path) {
                Ok(()) => break (id, path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        };
        let origins: Vec<Origin> =
            context
                .devices
                .iter()
                .map(|d| d.origin)
                .fold(vec![], |mut v, x| {
                    if !v.contains(&x) {
                        v.push(x);
                    }
                    v
                });
        let session = RecordingSession {
            id: id.clone(),
            name: name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("Session {utc}")),
            created_utc: utc,
            duration_us: 0,
            size_bytes: 0,
            origins,
            device_count: context.devices.len() as u32,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
        };
        let (tx, rx) = sync_channel::<Item>(4096);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (result_tx, result) = tokio::sync::oneshot::channel();
        let started = context.started_t_us;
        std::thread::Builder::new()
            .name("bansayo-recorder".into())
            .spawn(move || {
                let opened = open(&path, &session, &context);
                match opened {
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        let _ = result_tx.send(Err(e.to_string()));
                    }
                    Ok((mut writer, mut channels)) => {
                        let _ = ready_tx.send(Ok(()));
                        let result = (|| -> Result<RecordingSession, String> {
                            let duration = loop {
                                let item = rx.recv().map_err(|_| {
                                    "Recording interrupted before explicit finalisation.".to_owned()
                                })?;
                                match item {
                                    Item::Finish { duration_us } => break duration_us,
                                    Item::Event { accepted_us, event } => {
                                        write_event(&mut writer, &mut channels, accepted_us, event)
                                            .map_err(|e| e.to_string())?;
                                    }
                                    Item::Context {
                                        accepted_us,
                                        context,
                                    } => {
                                        write_json(
                                            &mut writer,
                                            &mut channels,
                                            "/bansayo/context",
                                            accepted_us,
                                            accepted_us,
                                            &*context,
                                        )
                                        .map_err(|e| e.to_string())?;
                                    }
                                }
                            };
                            writer.finish().map_err(|e| e.to_string())?;
                            let file = writer
                                .into_inner()
                                .into_inner()
                                .map_err(|e| e.to_string())?;
                            file.sync_all().map_err(|e| e.to_string())?;
                            let mut session = session;
                            session.duration_us = duration;
                            session.size_bytes = std::fs::metadata(path.join("recording.mcap"))
                                .map_err(|e| e.to_string())?
                                .len();
                            atomic_write(
                                &path.join("session.json"),
                                &serde_json::to_vec_pretty(&SessionFile {
                                    schema_version: 1,
                                    session: session.clone(),
                                })
                                .map_err(|e| e.to_string())?,
                            )
                            .map_err(|e| e.to_string())?;
                            Ok(session)
                        })();
                        let _ = result_tx.send(result);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|e| e.to_string())??;
        Ok(Self {
            state: RecordingState {
                session_id: id,
                started_t_us: started,
                events: 0,
                dropped_events: 0,
            },
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
            result,
        })
    }
    pub fn push(&mut self, item: Item) {
        match self.tx.try_send(item) {
            Ok(()) => self.state.events += 1,
            Err(_) => {
                self.state.dropped_events += 1;
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    pub async fn finish(self, duration_us: u64) -> Result<RecordingSession, String> {
        let tx = self.tx;
        tokio::task::spawn_blocking(move || tx.send(Item::Finish { duration_us }))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        self.result.await.map_err(|e| e.to_string())?
    }
}
type FileWriter = mcap::Writer<std::io::BufWriter<std::fs::File>>;
fn open(
    path: &Path,
    session: &RecordingSession,
    context: &Context,
) -> Result<(FileWriter, BTreeMap<String, u16>), Box<dyn std::error::Error>> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.join("recording.mcap"))?;
    atomic_write(
        &path.join("session.json"),
        &serde_json::to_vec_pretty(&SessionFile {
            schema_version: 1,
            session: session.clone(),
        })?,
    )?;
    // Input recordings stay uncompressed, as before the zstd feature was enabled for motion.mcap.
    let mut writer = mcap::WriteOptions::new()
        .compression(None)
        .create(std::io::BufWriter::new(file))?;
    writer.attach(&mcap::Attachment {
        log_time: context.started_t_us * 1000,
        create_time: context.started_t_us * 1000,
        name: "context.json".into(),
        media_type: "application/json".into(),
        data: Cow::Owned(serde_json::to_vec(context)?),
    })?;
    writer.write_metadata(&mcap::records::Metadata {
        name: "bansayo.session".into(),
        metadata: BTreeMap::from([
            ("id".into(), session.id.to_string()),
            ("name".into(), session.name.clone()),
            ("created_utc".into(), session.created_utc.clone()),
            ("origins".into(), serde_json::to_string(&session.origins)?),
        ]),
    })?;
    let mut channels = BTreeMap::new();
    write_json(
        &mut writer,
        &mut channels,
        "/bansayo/context",
        context.started_t_us,
        context.started_t_us,
        context,
    )?;
    Ok((writer, channels))
}
fn write_event(
    writer: &mut FileWriter,
    channels: &mut BTreeMap<String, u16>,
    accepted: u64,
    event: SourceEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    match event {
        SourceEvent::Observation { observation } => write_json(
            writer,
            channels,
            &format!("/bansayo/obs/{}", observation.device_id),
            accepted,
            observation.t_us,
            &observation,
        ),
        other => write_json(
            writer,
            channels,
            "/bansayo/devices",
            accepted,
            accepted,
            &other,
        ),
    }
}
fn write_json(
    writer: &mut FileWriter,
    channels: &mut BTreeMap<String, u16>,
    topic: &str,
    accepted: u64,
    capture: u64,
    value: &impl serde::Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    let channel = match channels.get(topic) {
        Some(id) => *id,
        None => {
            let id = writer.add_channel(0, topic, "json", &BTreeMap::new())?;
            channels.insert(topic.into(), id);
            id
        }
    };
    writer.write_to_known_channel(
        &mcap::records::MessageHeader {
            channel_id: channel,
            sequence: 0,
            log_time: accepted * 1000,
            publish_time: capture * 1000,
        },
        &serde_json::to_vec(value)?,
    )?;
    Ok(())
}
