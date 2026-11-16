//! Pipeline-owned latest observation and stream statistics; bounded by configured devices.
pub mod persistence;
use crate::{calibration::Store, config::QualityConfig, sources::DropCounters};
use bansayo_contracts::*;
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Entry {
    pub descriptor: DeviceDescriptor,
    pub latest: Option<SensorObservation>,
    pub received: u64,
    pub first_us: Option<u64>,
    pub dropped: u64,
    pub connected: bool,
    pub reported_quality: f32,
    pub recent: std::collections::VecDeque<u64>,
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
                            latest: None,
                            received: 0,
                            first_us: None,
                            dropped: 0,
                            connected: true,
                            reported_quality: 1.0,
                            recent: Default::default(),
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
                    e.latest = None;
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
                    latest: None,
                    received: 0,
                    first_us: None,
                    dropped: 0,
                    connected: true,
                    reported_quality: 1.0,
                    recent: Default::default(),
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
        if let Some(last) = &entry.latest {
            if o.seq <= last.seq || o.t_us < last.t_us {
                return false;
            }
            entry.dropped += o.seq.saturating_sub(last.seq + 1);
        }
        entry.connected = true;
        entry.failure = None;
        if entry.recent.len() == 256 {
            entry.recent.pop_front();
        }
        entry.recent.push_back(o.t_us);
        if let Some(q) = o.confidence {
            entry.reported_quality = q;
        }
        entry.first_us.get_or_insert(o.t_us);
        entry.received += 1;
        entry.latest = Some(o);
        true
    }
    pub fn status(&self, now: u64, quality: &QualityConfig, store: &Store) -> Vec<DeviceStatus> {
        self.entries
            .values()
            .map(|e| {
                let assignment = self.assignments.get(&e.descriptor.id).cloned();
                let last = e.latest.as_ref();
                let age = last.map(|o| now.saturating_sub(o.t_us));
                let rate = match (e.recent.front(), e.recent.back()) {
                    (Some(first), Some(last)) if last > first => {
                        ((e.recent.len() - 1) as f64 * 1e6 / (last - first) as f64) as f32
                    }
                    _ => 0.0,
                };
                let state = if e.failure.is_some() {
                    TrackingState::Unavailable
                } else if !e.connected
                    || age.is_some_and(|a| a > quality.disconnect_after_ms * 1000)
                {
                    TrackingState::Disconnected
                } else if last.is_none() {
                    TrackingState::Acquiring
                } else if e.reported_quality < quality.degraded_quality as f32
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
