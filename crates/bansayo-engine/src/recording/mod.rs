//! Input recordings: bounded writer thread, versioned context and deterministic replay clock.
pub mod motion;
pub mod reader;
pub mod replay;
mod writer;
use crate::{
    calibration::Store,
    config::{Config, QualityConfig},
    registry::Registry,
    skeleton::Skeleton,
};
use bansayo_contracts::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub use writer::*;
#[derive(Clone, Serialize, Deserialize)]
pub struct Context {
    pub schema_version: u32,
    pub started_t_us: u64,
    pub skeleton: SkeletonDefinition,
    pub assignments: BTreeMap<DeviceId, SegmentId>,
    pub profiles: Vec<CalibrationProfile>,
    pub stale: BTreeSet<ProfileId>,
    pub devices: Vec<DeviceDescriptor>,
    pub latest: Vec<SensorObservation>,
    pub links: BTreeMap<DeviceId, bool>,
    pub qualities: BTreeMap<DeviceId, f32>,
    pub config: Config,
    pub engine_version: String,
    pub protocol_version: u32,
    /// Room whose camera extrinsics were in use; absent in older recordings (default room).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_room: Option<String>,
}
impl Context {
    pub fn snapshot(
        registry: &Registry,
        store: &Store,
        skeleton: &Skeleton,
        config: &Config,
        now: u64,
    ) -> Self {
        Self {
            schema_version: 1,
            started_t_us: now,
            skeleton: skeleton.definition.clone(),
            assignments: registry.assignments.clone(),
            profiles: store.profiles.values().cloned().collect(),
            stale: store.stale.clone(),
            devices: registry
                .entries
                .values()
                .map(|e| e.descriptor.clone())
                .collect(),
            latest: registry
                .entries
                .values()
                .flat_map(|e| e.latest.values().cloned())
                .collect(),
            config: config.clone(),
            links: registry
                .entries
                .iter()
                .map(|(id, e)| (id.clone(), e.connected))
                .collect(),
            qualities: registry
                .entries
                .iter()
                .map(|(id, e)| (id.clone(), e.reported_quality))
                .collect(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
            active_room: store.active_room.clone(),
        }
    }
    pub fn restore(&self) -> Result<(Registry, Store, Skeleton), String> {
        if !self.config.engine.publish_rate_hz.is_finite()
            || !(1.0..=1000.0).contains(&self.config.engine.publish_rate_hz)
            || !(1..=60000).contains(&self.config.engine.stale_after_ms)
            || !(0.5..=2.5).contains(&self.skeleton.height_m)
            || self.schema_version != 1
            || self.protocol_version != PROTOCOL_VERSION
            || !self.config.evidence.detector_noise_floor_px.is_finite()
            || self.config.evidence.detector_noise_floor_px <= 0.0
            || !crate::config::triangulation_valid(&self.config.triangulation)
        {
            return Err("Recording uses an unsupported context version.".into());
        }
        let skeleton = Skeleton::builtin(&self.skeleton.model_id, self.skeleton.height_m as f64)
            .map_err(|e| e.to_string())?;
        let mut ids = BTreeSet::new();
        for descriptor in &self.devices {
            crate::sources::validation::descriptor(descriptor)?;
            if !ids.insert(descriptor.id.clone()) {
                return Err("Recording has duplicate device identities.".into());
            }
        }
        let mut assigned = BTreeSet::new();
        for (device, segment) in &self.assignments {
            if !self
                .devices
                .iter()
                .any(|d| &d.id == device && d.capabilities.contains(&SensorCapability::Orientation))
                || skeleton.index_of(segment).is_none()
                || !assigned.insert(segment)
            {
                return Err("Recording assignments are invalid.".into());
            }
        }
        if self
            .qualities
            .iter()
            .any(|(id, q)| !ids.contains(id) || !q.is_finite() || !(0.0..=1.0).contains(q))
            || self.links.keys().any(|id| !ids.contains(id))
        {
            return Err("Recording device state is invalid.".into());
        }
        for observation in &self.latest {
            let device = self
                .devices
                .iter()
                .find(|d| d.id == observation.device_id)
                .ok_or("Recording observation refers to an unknown device.")?;
            crate::sources::validation::observation(observation, device, self.started_t_us)?;
        }
        let mut profile_ids = BTreeSet::new();
        for profile in &self.profiles {
            if !profile_ids.insert(&profile.id) {
                return Err("Recording has duplicate calibration profiles.".into());
            }
            // Retained opaque/invalid profiles are never consumed. Store reports them as invalid.
            if !crate::calibration::valid_profile(profile) {
                continue;
            }
            let device = profile
                .subject
                .device_id
                .as_ref()
                .and_then(|id| self.devices.iter().find(|d| &d.id == id));
            let camera = matches!(
                profile.kind,
                CalibrationKind::CameraIntrinsics | CalibrationKind::CameraExtrinsics
            );
            if device.is_none_or(|d| camera != (d.kind == DeviceKind::Camera))
                || profile.kind == CalibrationKind::Mounting
                    && profile
                        .subject
                        .segment_id
                        .as_ref()
                        .is_none_or(|id| skeleton.index_of(id).is_none())
            {
                return Err("Recording calibration subject is invalid.".into());
            }
        }
        let mut registry = Registry::new(
            self.devices.clone(),
            self.assignments.clone(),
            BTreeMap::new(),
        );
        for o in &self.latest {
            registry.accept(o.clone());
        }
        for (id, entry) in &mut registry.entries {
            entry.connected = *self.links.get(id).unwrap_or(&true);
            entry.reported_quality = *self.qualities.get(id).unwrap_or(&1.0);
        }
        if self.active_room.as_deref().is_some_and(|r| !valid_room(r)) {
            return Err("Recording names an invalid room.".into());
        }
        let mut store = Store::default();
        store.insert(self.profiles.clone());
        store.stale = self.stale.clone();
        store.active_room.clone_from(&self.active_room);
        Ok((registry, store, skeleton))
    }
    pub fn quality(&self) -> &QualityConfig {
        &self.config.quality
    }
}
// Queue storage stays inline to avoid per-observation heap allocations.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum Item {
    Event {
        accepted_us: u64,
        event: SourceEvent,
    },
    Context {
        accepted_us: u64,
        context: Box<Context>,
    },
    Finish {
        duration_us: u64,
    },
}
#[derive(Serialize, Deserialize)]
pub struct SessionFile {
    pub schema_version: u32,
    #[serde(flatten)]
    pub session: RecordingSession,
}

/// Rewrites an input recording with zstd chunks (golden sessions kept in the repository); the
/// messages, topics and times are unchanged and the loader reads both forms.
pub fn recompress(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(dst).map_err(|e| e.to_string())?;
    let mut w = mcap::WriteOptions::new()
        .compression(Some(mcap::Compression::Zstd))
        .create(std::io::BufWriter::new(file))
        .map_err(|e| e.to_string())?;
    let mut channels: BTreeMap<String, u16> = BTreeMap::new();
    for m in mcap::MessageStream::new(&bytes).map_err(|e| e.to_string())? {
        let m = m.map_err(|e| e.to_string())?;
        let id = match channels.get(&m.channel.topic) {
            Some(id) => *id,
            None => {
                let id = w
                    .add_channel(
                        0,
                        &m.channel.topic,
                        &m.channel.message_encoding,
                        &BTreeMap::new(),
                    )
                    .map_err(|e| e.to_string())?;
                channels.insert(m.channel.topic.clone(), id);
                id
            }
        };
        w.write_to_known_channel(
            &mcap::records::MessageHeader {
                channel_id: id,
                sequence: m.sequence,
                log_time: m.log_time,
                publish_time: m.publish_time,
            },
            &m.data,
        )
        .map_err(|e| e.to_string())?;
    }
    w.finish().map_err(|e| e.to_string())?;
    Ok(())
}
