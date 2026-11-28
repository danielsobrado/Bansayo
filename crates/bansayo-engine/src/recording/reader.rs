use super::{Context, SessionFile};
use bansayo_contracts::*;
use std::path::Path;
pub const MAX_REPLAY_BYTES: u64 = 128 * 1024 * 1024;
#[derive(Clone)]
pub enum RecordedData {
    Event(Box<SourceEvent>),
    Context(Box<Context>),
}
#[derive(Clone)]
pub struct Recorded {
    pub accepted_us: u64,
    pub data: RecordedData,
}
#[derive(Clone)]
pub struct Recording {
    pub session: RecordingSession,
    pub initial: Context,
    pub events: Vec<Recorded>,
}
fn metadata(path: &Path, id: &SessionId) -> Result<RecordingSession, String> {
    let metadata: SessionFile = serde_json::from_slice(
        &std::fs::read(path.join("session.json")).map_err(|_| "Session metadata is damaged.")?,
    )
    .map_err(|_| "Session metadata is damaged.")?;
    if metadata.schema_version != 1
        || metadata.session.id != *id
        || metadata.session.protocol_version != PROTOCOL_VERSION
    {
        return Err("Session uses an unsupported format or is damaged.".into());
    }
    Ok(metadata.session)
}
pub fn load(dir: &Path, id: &SessionId) -> Result<Recording, String> {
    let path = dir.join("sessions").join(id.as_str());
    if !path.exists() {
        return Err("Session was not found.".into());
    }
    let session = metadata(&path, id)?;
    if session.size_bytes == 0 {
        return Err("Recording was not finalised.".into());
    }
    let file = path.join("recording.mcap");
    let size = std::fs::metadata(&file)
        .map_err(|_| "Recording file is missing.")?
        .len();
    if size > MAX_REPLAY_BYTES {
        return Err("Session is too large for in-memory playback (maximum 128 MiB).".into());
    }
    let bytes = std::fs::read(file).map_err(|_| "Recording could not be read.")?;
    if mcap::Summary::read(&bytes)
        .map_err(|_| "Recording is damaged or was not finalised.")?
        .is_none()
    {
        return Err("Recording has no finalised summary.".into());
    }
    let stream = mcap::MessageStream::new(&bytes).map_err(|_| "Recording is damaged.")?;
    let mut initial = None;
    let mut registry: Option<crate::registry::Registry> = None;
    let mut events = vec![];
    let mut previous_log_time = None;
    for message in stream {
        let message = message.map_err(|_| "Recording contains a damaged message.")?;
        if previous_log_time.is_some_and(|previous| message.log_time < previous) {
            return Err("Recording acceptance timestamps are out of order.".into());
        }
        previous_log_time = Some(message.log_time);
        let data = match message.channel.topic.as_str() {
            "/bansayo/context" => {
                let context: Context = serde_json::from_slice(&message.data)
                    .map_err(|_| "Recording context is invalid.")?;
                let (restored, _, _) = context.restore()?;
                registry = Some(restored);
                if initial.is_none() {
                    initial = Some(context);
                    continue;
                }
                RecordedData::Context(Box::new(context))
            }
            "/bansayo/devices" => RecordedData::Event(
                serde_json::from_slice(&message.data).map_err(|_| "Device event is invalid.")?,
            ),
            topic if topic.starts_with("/bansayo/obs/") => {
                RecordedData::Event(Box::new(SourceEvent::Observation {
                    observation: serde_json::from_slice(&message.data)
                        .map_err(|_| "Recorded observation is invalid.")?,
                }))
            }
            _ => continue,
        };
        if let RecordedData::Event(event) = &data {
            let known = registry
                .as_mut()
                .ok_or("Recording context must precede all events.")?;
            match event.as_ref() {
                SourceEvent::DeviceAnnounced { descriptor } => {
                    crate::sources::validation::descriptor(descriptor)?
                }
                SourceEvent::DeviceLink {
                    device_id, t_us, ..
                } => {
                    if !known.entries.contains_key(device_id) || *t_us > message.log_time / 1000 {
                        return Err("Recorded device link is invalid.".into());
                    }
                }
                SourceEvent::Observation { observation } => {
                    let entry = known
                        .entries
                        .get(&observation.device_id)
                        .ok_or("Recorded observation has an unknown device.")?;
                    crate::sources::validation::observation(
                        observation,
                        &entry.descriptor,
                        message.log_time / 1000,
                    )?;
                    if entry
                        .latest
                        .get(&crate::registry::ObservationKind::of(&observation.data))
                        .is_some_and(|last| {
                            observation.seq <= last.seq || observation.t_us < last.t_us
                        })
                    {
                        return Err("Recorded observations are out of order.".into());
                    }
                }
            }
            if !known.event(*event.clone()) {
                return Err("Recorded event was rejected by the registry.".into());
            }
        }
        events.push(Recorded {
            accepted_us: message.log_time / 1000,
            data,
        });
        if events.len() > 1_000_000 {
            return Err("Session has too many events for in-memory playback.".into());
        }
    }
    let initial: Context = initial.ok_or("Recording context is missing.")?;
    // Replay ends at the session duration, so every event must be within it to be replayed.
    let end = initial
        .started_t_us
        .checked_add(session.duration_us)
        .ok_or("Recording duration overflows the session clock.")?;
    if events.last().is_some_and(|e| e.accepted_us > end) {
        return Err("Recording events extend beyond the session duration.".into());
    }
    Ok(Recording {
        session,
        initial,
        events,
    })
}
pub fn sessions(dir: &Path) -> Result<(Vec<RecordingSession>, Vec<String>), String> {
    let directory = dir.join("sessions");
    if !directory.exists() {
        return Ok((vec![], vec![]));
    }
    let mut sessions = vec![];
    let mut damaged = vec![];
    for entry in std::fs::read_dir(directory).map_err(|_| "Session index cannot be read.")? {
        let entry = entry.map_err(|_| "Session index cannot be read.")?;
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|s| SessionId::new(s).ok())
        else {
            continue;
        };
        match finalised(&entry.path(), &id) {
            Ok(session) => sessions.push(session),
            Err(_) => damaged.push(id.to_string()),
        }
    }
    sessions.sort_by(|a, b| b.created_utc.cmp(&a.created_utc).then(b.id.cmp(&a.id)));
    Ok((sessions, damaged))
}
/// Listing reads metadata only; full MCAP validation happens at playback, so a valid recording
/// too large to replay is still listed. The writer rewrites `session.json` with the final size only
/// after the MCAP is finalised and synced, so a matching size means the recording was completed.
fn finalised(path: &Path, id: &SessionId) -> Result<RecordingSession, String> {
    let session = metadata(path, id)?;
    let size = std::fs::metadata(path.join("recording.mcap"))
        .map_err(|_| "Recording file is missing.")?
        .len();
    if session.size_bytes == 0 || size != session.size_bytes {
        return Err("Recording was not finalised.".into());
    }
    Ok(session)
}
