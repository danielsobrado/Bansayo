//! Deterministic body observations. The solver never receives body ground truth.
pub mod body;
mod camera;
pub mod motions;
mod trackers;
use crate::{
    config::SimulatedSpec,
    skeleton::Skeleton,
    sources::{Source, SourceContext},
};
use kinexa_contracts::*;
use motions::Motion;
use std::{collections::BTreeMap, sync::atomic::Ordering, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct Control {
    pub motion: Motion,
    pub speed: f64,
    pub paused: bool,
    pub generation: u64,
    pub suspended: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaultSetting {
    pub connected: bool,
    pub quality: f32,
}
impl Default for FaultSetting {
    fn default() -> Self {
        Self {
            connected: true,
            quality: 1.0,
        }
    }
}
impl FaultSetting {
    pub fn apply(&mut self, action: FaultAction, quality: Option<f32>) {
        match action {
            FaultAction::Disconnect => self.connected = false,
            FaultAction::Reconnect => self.connected = true,
            FaultAction::Degrade => self.quality = quality.unwrap_or(0.3),
            FaultAction::Restore => self.quality = 1.0,
        }
    }
}
#[derive(Clone)]
pub struct SimulatedSource {
    pub spec: SimulatedSpec,
    pub skeleton: Skeleton,
    pub assignments: BTreeMap<DeviceId, SegmentId>,
}
impl SimulatedSource {
    pub fn descriptors(&self) -> Vec<DeviceDescriptor> {
        self.spec
            .trackers
            .iter()
            .map(|t| DeviceDescriptor {
                id: t.id.clone(),
                source_id: self.spec.id.clone(),
                kind: DeviceKind::ImuTracker,
                origin: Origin::Simulated,
                model: Some("Simulated IMU".into()),
                firmware: None,
                transport: Transport::Simulated,
                capabilities: vec![SensorCapability::Orientation, SensorCapability::Quality],
                nominal_rate_hz: Some(t.rate_hz as f32),
                camera: None,
            })
            .chain(self.spec.cameras.iter().map(|c| DeviceDescriptor {
                id: c.id.clone(),
                source_id: self.spec.id.clone(),
                kind: DeviceKind::Camera,
                origin: Origin::Simulated,
                model: Some("Simulated pinhole camera".into()),
                firmware: None,
                transport: Transport::Simulated,
                capabilities: vec![SensorCapability::Landmarks2d],
                nominal_rate_hz: Some(c.rate_hz as f32),
                camera: Some(CameraInfo {
                    resolution_px: c.resolution_px,
                    landmark_schema: Some(self.skeleton.landmark_schema()),
                }),
            }))
            .collect()
    }
}
impl Source for SimulatedSource {
    async fn run(self, context: SourceContext) -> Result<(), crate::sources::SourceError> {
        let mut timer = tokio::time::interval(Duration::from_millis(1));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut devices: BTreeMap<DeviceId, (u64, u64)> = BTreeMap::new();
        let mut last_us = context.clock.now_us();
        let mut sim_s = 0.0;
        let mut generation = 0;
        let mut links: BTreeMap<DeviceId, bool> = BTreeMap::new();
        loop {
            tokio::select! { _=context.cancel.cancelled()=>break, _=timer.tick()=>{} }
            let now = context.clock.now_us();
            let control = *context.control.borrow();
            if control.generation != generation {
                sim_s = 0.0;
                generation = control.generation;
            } else if !control.paused && !control.suspended {
                sim_s += (now - last_us) as f64 / 1e6 * control.speed;
            }
            last_us = now;
            if control.suspended {
                continue;
            }
            let mut faults: BTreeMap<DeviceId, FaultSetting> = BTreeMap::new();
            let phase = self
                .spec
                .scenario_period_s
                .map_or(sim_s, |period| sim_s.rem_euclid(period));
            for step in &self.spec.scenario {
                if step.at_s <= phase
                    && let Ok(id) = DeviceId::new(step.device.as_str())
                {
                    faults
                        .entry(id)
                        .or_default()
                        .apply(step.action, step.quality);
                }
            }
            faults.extend(
                context
                    .faults
                    .borrow()
                    .iter()
                    .map(|(id, s)| (id.clone(), *s)),
            );
            for id in self
                .spec
                .trackers
                .iter()
                .map(|t| &t.id)
                .chain(self.spec.cameras.iter().map(|c| &c.id))
            {
                let connected = faults.get(id).copied().unwrap_or_default().connected;
                if links
                    .insert(id.clone(), connected)
                    .is_some_and(|old| old != connected)
                {
                    send(
                        &context,
                        id,
                        SourceEvent::DeviceLink {
                            device_id: id.clone(),
                            connected,
                            t_us: now,
                        },
                    );
                }
            }
            let truth = body::body_pose(&self.skeleton, control.motion, sim_s);
            for tracker in &self.spec.trackers {
                let fault = faults.get(&tracker.id).copied().unwrap_or_default();
                if !fault.connected {
                    continue;
                }
                let (seq, next) = devices.entry(tracker.id.clone()).or_default();
                if now < *next {
                    continue;
                }
                *seq += 1;
                *next = now + (1e6 / tracker.rate_hz) as u64;
                let index = self
                    .assignments
                    .get(&tracker.id)
                    .and_then(|s| self.skeleton.index_of(s))
                    .unwrap_or(0);
                let q = trackers::reading(
                    &tracker.id,
                    self.spec.seed,
                    *seq,
                    tracker.noise_deg,
                    tracker.heading_offset_deg,
                    truth.rotations[index],
                );
                let event = SourceEvent::Observation {
                    observation: SensorObservation {
                        device_id: tracker.id.clone(),
                        source_id: self.spec.id.clone(),
                        origin: Origin::Simulated,
                        seq: *seq,
                        t_us: now,
                        device_time_us: Some(now),
                        time_quality: TimeQuality::Synchronized,
                        calibration_ref: None,
                        confidence: Some(fault.quality),
                        data: ObservationData::Orientation {
                            frame: FrameRef::SensorReference {
                                id: tracker.id.clone(),
                            },
                            q,
                        },
                    },
                };
                send(&context, &tracker.id, event);
            }
            for camera in &self.spec.cameras {
                let fault = faults.get(&camera.id).copied().unwrap_or_default();
                if !fault.connected {
                    continue;
                }
                let (seq, next) = devices.entry(camera.id.clone()).or_default();
                if now < *next {
                    continue;
                }
                *seq += 1;
                *next = now + (1e6 / camera.rate_hz) as u64;
                let landmarks =
                    camera::project(camera, self.spec.seed, *seq, now, &truth.positions);
                let event = SourceEvent::Observation {
                    observation: SensorObservation {
                        device_id: camera.id.clone(),
                        source_id: self.spec.id.clone(),
                        origin: Origin::Simulated,
                        seq: *seq,
                        t_us: now,
                        device_time_us: Some(now),
                        time_quality: TimeQuality::Synchronized,
                        calibration_ref: None,
                        confidence: Some(1.0),
                        data: ObservationData::Landmarks2d { landmarks },
                    },
                };
                send(&context, &camera.id, event);
            }
        }
        Ok(())
    }
}
fn send(context: &SourceContext, id: &DeviceId, event: SourceEvent) {
    if context
        .events
        .try_send(crate::sources::Ingest::Event(event))
        .is_err()
        && let Some(c) = context.dropped.get(id)
    {
        c.fetch_add(1, Ordering::Relaxed);
    }
}
