//! Input recordings: bounded writer thread, versioned context and deterministic replay clock.
pub mod reader;
pub mod replay;
mod writer;
use crate::{
    calibration::Store,
    config::{Config, QualityConfig},
    registry::Registry,
    skeleton::Skeleton,
};
use kinexa_contracts::*;
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
                .filter_map(|e| e.latest.clone())
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
        }
    }
    pub fn restore(&self) -> Result<(Registry, Store, Skeleton), String> {
        if !self.config.engine.publish_rate_hz.is_finite()
            || !(1.0..=1000.0).contains(&self.config.engine.publish_rate_hz)
            || !(1..=60000).contains(&self.config.engine.stale_after_ms)
            || !(0.5..=2.5).contains(&self.skeleton.height_m)
            || self.schema_version != 1
            || self.protocol_version != PROTOCOL_VERSION
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
        for profile in &self.profiles {
            if profile.schema_version != CALIBRATION_PROFILE_SCHEMA_VERSION
                || profile.kind != CalibrationKind::Mounting
                || profile
                    .subject
                    .device_id
                    .as_ref()
                    .is_none_or(|id| !ids.contains(id))
                || profile
                    .subject
                    .segment_id
                    .as_ref()
                    .is_none_or(|id| skeleton.index_of(id).is_none())
                || serde_json::from_value::<MountingData>(profile.data.clone().into())
                    .ok()
                    .is_none_or(|data| {
                        crate::math::rot_from_wire(data.q_sensor_from_segment).is_none()
                    })
            {
                return Err("Recording calibration profile is invalid.".into());
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
        let mut store = Store::default();
        store.insert(self.profiles.clone());
        store.stale = self.stale.clone();
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
