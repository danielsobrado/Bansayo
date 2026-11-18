//! Pipeline-owned latest observation and stream statistics; bounded by configured devices.
pub mod persistence;
use crate::{calibration::Store, config::QualityConfig, sources::DropCounters};
use bansayo_contracts::*;
use std::collections::BTreeMap;
/// Camera frames kept per camera: about a quarter second at 30 Hz.
pub const LANDMARK_HISTORY: usize = 8;
/// Streams have independent sequence numbers and timestamps, even on the same device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservationKind {
    Orientation,
    AngularVelocity,
    Acceleration,
    Position,
    Distance,
    Contact,
    Landmarks2d,
    Landmarks3d,
    Diagnostics,
}
impl ObservationKind {
    pub fn of(data: &ObservationData) -> Self {
        match data {
            ObservationData::Orientation { .. } => Self::Orientation,
            ObservationData::AngularVelocity { .. } => Self::AngularVelocity,
            ObservationData::Acceleration { .. } => Self::Acceleration,
            ObservationData::Position { .. } => Self::Position,
            ObservationData::Distance { .. } => Self::Distance,
            ObservationData::Contact { .. } => Self::Contact,
            ObservationData::Landmarks2d { .. } => Self::Landmarks2d,
            ObservationData::Landmarks3d { .. } => Self::Landmarks3d,
            ObservationData::Diagnostics { .. } => Self::Diagnostics,
        }
    }
}
#[derive(Clone)]
pub struct Entry {
    pub descriptor: DeviceDescriptor,
    pub latest: BTreeMap<ObservationKind, SensorObservation>,
    pub received: u64,
    pub first_us: Option<u64>,
    pub dropped: u64,
    pub connected: bool,
    pub reported_quality: f32,
    pub recent: std::collections::VecDeque<u64>,
    /// Newest camera landmark observations, oldest first, for time alignment across cameras.
    pub landmarks: std::collections::VecDeque<SensorObservation>,
    pub failure: Option<String>,
}
#[derive(Clone)]
pub struct Registry {
    pub entries: BTreeMap<DeviceId, Entry>,
    pub assignments: BTreeMap<DeviceId, SegmentId>,
    pub drops: DropCounters,
}
impl Registry {
    pub fn new(
        descriptors: Vec<DeviceDescriptor>,
        assignments: BTreeMap<DeviceId, SegmentId>,
        drops: DropCounters,
    ) -> Self {
        Self {
            entries: descriptors
                .into_iter()
                .map(|d| {
                    (
                        d.id.clone(),
                        Entry {
                            descriptor: d,
                            latest: BTreeMap::new(),
                            received: 0,
                            first_us: None,
                            dropped: 0,
                            connected: true,
                            reported_quality: 1.0,
                            recent: Default::default(),
                            landmarks: Default::default(),
                            failure: None,
                        },
                    )
                })
                .collect(),
            assignments,
            drops,
        }
    }
    /// Returns whether the event changed pipeline state. Only accepted events may be recorded:
    /// replay re-applies them and rejects a recording holding anything the live registry refused.
    pub fn event(&mut self, event: SourceEvent) -> bool {
        match event {
            SourceEvent::Observation { observation } => self.accept(observation),
            SourceEvent::DeviceLink {
                device_id,
                connected,
                ..
            } => {
                let Some(e) = self.entries.get_mut(&device_id) else {
                    return false;
                };
                e.connected = connected;
                if connected {
                    e.first_us = None;
                    e.recent.clear();
                    e.landmarks.clear();
                    e.latest.clear();
                }
                true
            }
            SourceEvent::DeviceAnnounced { descriptor } => {
                let std::collections::btree_map::Entry::Vacant(slot) =
                    self.entries.entry(descriptor.id.clone())
                else {
                    return false;
                };
                slot.insert(Entry {
                    descriptor,
                    latest: BTreeMap::new(),
                    received: 0,
                    first_us: None,
                    dropped: 0,
                    connected: true,
                    reported_quality: 1.0,
                    recent: Default::default(),
                    landmarks: Default::default(),
                    failure: None,
                });
                true
            }
        }
    }
    /// Rejects unknown devices, duplicates and observations older than the latest accepted one.
    pub fn accept(&mut self, o: SensorObservation) -> bool {
        let Some(entry) = self.entries.get_mut(&o.device_id) else {
            return false;
        };
        let kind = ObservationKind::of(&o.data);
        if let Some(last) = entry.latest.get(&kind) {
            if o.seq <= last.seq || o.t_us < last.t_us {
                return false;
            }
            entry.dropped += o.seq.saturating_sub(last.seq + 1);
        }
        entry.connected = true;
        entry.failure = None;
        // The descriptor rate is the primary stream rate, not the sum of multiplexed streams.
        let primary = if entry
            .descriptor
            .capabilities
            .contains(&SensorCapability::Orientation)
        {
            ObservationKind::Orientation
        } else if entry
            .descriptor
            .capabilities
            .contains(&SensorCapability::Landmarks2d)
        {
            ObservationKind::Landmarks2d
        } else {
            kind
        };
        if kind == primary {
            if entry.recent.len() == 256 {
                entry.recent.pop_front();
            }
            entry.recent.push_back(o.t_us);
        }
        if let Some(q) = o.confidence {
            entry.reported_quality = q;
        }
        entry.first_us.get_or_insert(o.t_us);
        entry.received += 1;
        if matches!(o.data, ObservationData::Landmarks2d { .. }) {
            if entry.landmarks.len() == LANDMARK_HISTORY {
                entry.landmarks.pop_front();
            }
            entry.landmarks.push_back(o.clone());
        }
        entry.latest.insert(kind, o);
        true
    }
    pub fn status(&self, now: u64, quality: &QualityConfig, store: &Store) -> Vec<DeviceStatus> {
        self.entries
            .values()
            .map(|e| {
                let assignment = self.assignments.get(&e.descriptor.id).cloned();
                let last = e.latest.values().max_by_key(|o| o.t_us);
                let age = last.map(|o| now.saturating_sub(o.t_us));
                let rate = match (e.recent.front(), e.recent.back()) {
                    (Some(first), Some(last)) if last > first => {
                        ((e.recent.len() - 1) as f64 * 1e6 / (last - first) as f64) as f32
                    }
                    _ => 0.0,
                };
                let settings =
                    e.latest
                        .get(&ObservationKind::Landmarks2d)
                        .and_then(|o| match &o.data {
                            ObservationData::Landmarks2d { landmarks } => {
                                landmarks.frame_meta.settings.clone()
                            }
                            _ => None,
                        });
                let unapplied = settings.as_ref().is_some_and(|s| !s.problems.is_empty());
                let state = if e.failure.is_some() {
                    TrackingState::Unavailable
                } else if !e.connected
                    || age.is_some_and(|a| a > quality.disconnect_after_ms * 1000)
                {
                    TrackingState::Disconnected
                } else if last.is_none() {
                    TrackingState::Acquiring
                } else if unapplied
                    || e.reported_quality < quality.degraded_quality as f32
                    || e.received > 2
                        && e.descriptor
                            .nominal_rate_hz
                            .is_some_and(|n| rate < n * quality.degraded_rate_fraction as f32)
                {
                    TrackingState::Degraded
                } else {
                    TrackingState::Tracking
                };
                let mut reasons = vec![];
                match state {
                    TrackingState::Disconnected => reasons.push(reason(
                        "device_disconnected",
                        Some(e.descriptor.id.to_string()),
                        "Device is disconnected. Waiting for fresh data.",
                    )),
                    // Settings problems name their property (08 §3.2) and stand in for the generic reason.
                    TrackingState::Degraded if unapplied => {
                        reasons.extend(settings.iter().flat_map(|s| s.problems.iter().cloned()))
                    }
                    TrackingState::Degraded => reasons.push(reason(
                        "device_degraded",
                        Some(e.descriptor.id.to_string()),
                        "Device data rate or reported quality is below the configured threshold.",
                    )),
                    TrackingState::Unavailable => reasons.push(reason(
                        "source_failed",
                        Some(e.descriptor.source_id.to_string()),
                        e.failure.as_deref().unwrap_or("Source is unavailable."),
                    )),
                    TrackingState::Acquiring => reasons.push(reason(
                        "device_acquiring",
                        Some(e.descriptor.id.to_string()),
                        "Waiting for usable observations.",
                    )),
                    _ => {}
                }

                let mut calibration = vec![];
                if e.descriptor.kind == DeviceKind::ImuTracker {
                    calibration.push(store.reference(&e.descriptor.id, assignment.as_ref()));
                    calibration.push(CalibrationRef {
                        kind: CalibrationKind::HeadingAlignment,
                        state: CalibrationState::Assumed,
                        profile_id: None,
                    });
                    reasons.push(reason(
                        "heading_alignment_assumed",
                        Some(e.descriptor.id.to_string()),
                        "Heading alignment has not been calibrated; identity is assumed.",
                    ));
                }
                if e.descriptor.kind == DeviceKind::Camera {
                    calibration.extend(
                        [
                            CalibrationKind::CameraIntrinsics,
                            CalibrationKind::CameraExtrinsics,
                        ]
                        .map(|kind| store.camera_ref(kind, &e.descriptor.id)),
                    );
                }
                DeviceStatus {
                    descriptor: e.descriptor.clone(),
                    state,
                    reasons,
                    assignment,
                    stream: StreamStatus {
                        measured_rate_hz: rate,
                        last_seen_t_us: last.map(|o| o.t_us),
                        received: e.received,
                        dropped: e.dropped.max(
                            self.drops
                                .get(&e.descriptor.id)
                                .map_or(0, |c| c.load(std::sync::atomic::Ordering::Relaxed)),
                        ),
                        time_quality: last.map_or(TimeQuality::ReceiveTime, |o| o.time_quality),
                    },
                    battery: None,
                    signal: None,
                    quality: last.map(|_| e.reported_quality),
                    calibration,
                    camera_settings: settings,
                }
            })
            .collect()
    }
}
pub fn reason(code: &str, subject: Option<String>, message: &str) -> Reason {
    Reason {
        code: code.into(),
        subject,
        message: message.into(),
    }
}
