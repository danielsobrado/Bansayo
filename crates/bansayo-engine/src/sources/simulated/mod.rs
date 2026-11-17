//! Deterministic body observations. The solver never receives body ground truth.
pub mod body;
pub mod camera;
pub mod motions;
mod trackers;
use crate::{
    config::SimulatedSpec,
    skeleton::Skeleton,
    sources::{Source, SourceContext},
};
use bansayo_contracts::*;
use motions::Motion;
use std::{collections::BTreeMap, sync::atomic::Ordering};

const MICROSECONDS_PER_SECOND: f64 = 1e6;
const MIN_SCENARIO_STEP_US: u64 = 1;

/// Simulation-only controls stay out of hardware source contexts.
#[derive(Clone)]
pub struct SimulatedContext {
    pub common: SourceContext,
    pub control: tokio::sync::watch::Receiver<Control>,
    pub faults: tokio::sync::watch::Receiver<BTreeMap<DeviceId, FaultSetting>>,
}
impl AsRef<SourceContext> for SimulatedContext {
    fn as_ref(&self) -> &SourceContext {
        &self.common
    }
}
impl std::ops::Deref for SimulatedContext {
    type Target = SourceContext;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
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
    /// The live assignment: reassigning a simulated tracker moves its virtual strap with it.
    pub assignments: tokio::sync::watch::Receiver<BTreeMap<DeviceId, SegmentId>>,
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
                capabilities: vec![
                    SensorCapability::Orientation,
                    SensorCapability::AngularVelocity,
                    SensorCapability::Quality,
                ],
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
                    landmark_schema: Some(
                        if self.spec.landmark_schema == BODY_KEYPOINT_SCHEMA_ID {
                            self.skeleton.keypoint_schema()
                        } else {
                            self.skeleton.landmark_schema()
                        },
                    ),
                }),
            }))
            .collect()
    }
}
/// Next emission time. Counted from the previous deadline so wake-up jitter does not lower
/// the rate; after a stall, or for the first sample, counted from now so nothing bursts.
fn next_due(previous: u64, now: u64, rate_hz: f64) -> u64 {
    let period = (MICROSECONDS_PER_SECOND / rate_hz) as u64;
    if previous + period > now {
        previous + period
    } else {
        now + period
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StepMetrics {
    pub body_pose_evals: u64,
    pub fault_rebuilds: u64,
    pub emitted_observations: u64,
    pub emitted_links: u64,
}

enum DeviceKindState {
    Tracker {
        noise_deg: f64,
        heading_offset_deg: f64,
    },
    Camera {
        spec_index: usize,
    },
}

struct DeviceSlot {
    id: DeviceId,
    rate_hz: f64,
    seq: u64,
    next_due_us: u64,
    time: crate::clock::SourceTime,
    previous_rotation: Option<(crate::math::Rot, u64)>,
    connected: Option<bool>,
    fault: FaultSetting,
    kind: DeviceKindState,
}

/// Deadline-driven simulator scheduler. Decouples device timing and ground-truth evaluation
/// from fixed polling so idle wake-ups perform zero pose or fault calculations.
pub struct Scheduler {
    spec: SimulatedSpec,
    skeleton: Skeleton,
    devices: Vec<DeviceSlot>,
    last_us: u64,
    sim_s: f64,
    control: Control,
    faults_dirty: bool,
    next_scenario_sim_s: Option<f64>,
    fault_scratch: BTreeMap<DeviceId, FaultSetting>,
}

impl Scheduler {
    pub fn new(spec: SimulatedSpec, skeleton: Skeleton, now_us: u64, control: Control) -> Self {
        let mut devices = Vec::with_capacity(spec.trackers.len() + spec.cameras.len());
        for tracker in &spec.trackers {
            devices.push(DeviceSlot {
                id: tracker.id.clone(),
                rate_hz: tracker.rate_hz,
                seq: 0,
                next_due_us: 0,
                time: Default::default(),
                previous_rotation: None,
                connected: None,
                fault: FaultSetting::default(),
                kind: DeviceKindState::Tracker {
                    noise_deg: tracker.noise_deg,
                    heading_offset_deg: tracker.heading_offset_deg,
                },
            });
        }
        for (spec_index, camera) in spec.cameras.iter().enumerate() {
            devices.push(DeviceSlot {
                id: camera.id.clone(),
                rate_hz: camera.rate_hz,
                seq: 0,
                next_due_us: 0,
                time: Default::default(),
                previous_rotation: None,
                connected: None,
                fault: FaultSetting::default(),
                kind: DeviceKindState::Camera { spec_index },
            });
        }
        Self {
            spec,
            skeleton,
            devices,
            last_us: now_us,
            sim_s: 0.0,
            control,
            faults_dirty: true,
            next_scenario_sim_s: None,
            fault_scratch: BTreeMap::new(),
        }
    }

    fn update_clock(&mut self, now_us: u64) {
        if !self.control.paused && !self.control.suspended {
            self.sim_s += now_us.saturating_sub(self.last_us) as f64 / MICROSECONDS_PER_SECOND
                * self.control.speed;
        }
        self.last_us = now_us;
    }

    pub fn on_control(&mut self, now_us: u64, next: Control) {
        self.update_clock(now_us);
        if next.generation != self.control.generation {
            self.sim_s = 0.0;
            for slot in &mut self.devices {
                slot.previous_rotation = None;
            }
            if !self.spec.scenario.is_empty() {
                self.faults_dirty = true;
            }
        }
        self.control = next;
    }

    pub fn on_faults(&mut self, now_us: u64) {
        self.update_clock(now_us);
        self.faults_dirty = true;
    }

    pub fn next_deadline_us(&self) -> Option<u64> {
        if self.control.suspended {
            return None;
        }
        let device_due = self
            .devices
            .iter()
            .filter(|d| d.fault.connected)
            .map(|d| d.next_due_us)
            .min();
        let scenario_due = if !self.control.paused && self.control.speed > 0.0 {
            self.next_scenario_sim_s.map(|target_sim_s| {
                let delta_sim_s = (target_sim_s - self.sim_s).max(0.0);
                let delta_us = (delta_sim_s / self.control.speed * MICROSECONDS_PER_SECOND)
                    .ceil()
                    .max(MIN_SCENARIO_STEP_US as f64) as u64;
                self.last_us.saturating_add(delta_us)
            })
        } else {
            None
        };
        match (device_due, scenario_due) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn rebuild_faults(
        &mut self,
        now_us: u64,
        manual_faults: &BTreeMap<DeviceId, FaultSetting>,
        emit: &mut impl FnMut(&DeviceId, SourceEvent),
        metrics: &mut StepMetrics,
    ) {
        self.fault_scratch.clear();
        let scenario = &self.spec.scenario;
        if !scenario.is_empty() {
            let phase = self
                .spec
                .scenario_period_s
                .map_or(self.sim_s, |period| self.sim_s.rem_euclid(period));
            for step in scenario {
                if step.at_s <= phase
                    && let Ok(id) = DeviceId::new(step.device.as_str())
                {
                    self.fault_scratch
                        .entry(id)
                        .or_default()
                        .apply(step.action, step.quality);
                }
            }
            self.next_scenario_sim_s =
                next_scenario_transition(scenario, self.spec.scenario_period_s, self.sim_s, phase);
        } else {
            self.next_scenario_sim_s = None;
        }
        for (id, setting) in manual_faults {
            self.fault_scratch.insert(id.clone(), *setting);
        }
        for slot in &mut self.devices {
            let fault = self
                .fault_scratch
                .get(&slot.id)
                .copied()
                .unwrap_or_default();
            slot.fault = fault;
            if slot
                .connected
                .replace(fault.connected)
                .is_some_and(|old| old != fault.connected)
            {
                slot.previous_rotation = None;
                emit(
                    &slot.id,
                    SourceEvent::DeviceLink {
                        device_id: slot.id.clone(),
                        connected: fault.connected,
                        t_us: now_us,
                    },
                );
                metrics.emitted_links += 1;
            }
        }
        metrics.fault_rebuilds += 1;
        self.faults_dirty = false;
    }

    pub fn step(
        &mut self,
        now_us: u64,
        assignments: &BTreeMap<DeviceId, SegmentId>,
        manual_faults: &BTreeMap<DeviceId, FaultSetting>,
        mut emit: impl FnMut(&DeviceId, SourceEvent),
    ) -> StepMetrics {
        self.update_clock(now_us);
        let mut metrics = StepMetrics::default();
        if self.control.suspended {
            return metrics;
        }
        let scenario_due = self
            .next_scenario_sim_s
            .is_some_and(|target| self.sim_s >= target);
        if self.faults_dirty || scenario_due {
            self.rebuild_faults(now_us, manual_faults, &mut emit, &mut metrics);
        }
        let mut truth: Option<body::BodyPose> = None;
        for slot in &mut self.devices {
            if !slot.fault.connected || now_us < slot.next_due_us {
                continue;
            }
            let pose = truth.get_or_insert_with(|| {
                metrics.body_pose_evals += 1;
                body::body_pose(&self.skeleton, self.control.motion, self.sim_s)
            });
            slot.seq += 1;
            slot.next_due_us = next_due(slot.next_due_us, now_us, slot.rate_hz);
            let (t_us, time_quality) = slot.time.map(Some(now_us), now_us, 0);
            let event = match slot.kind {
                DeviceKindState::Tracker {
                    noise_deg,
                    heading_offset_deg,
                } => {
                    let index = assignments
                        .get(&slot.id)
                        .and_then(|s| self.skeleton.index_of(s))
                        .unwrap_or(0);
                    let rotation = pose.rotations[index];
                    let rate = slot
                        .previous_rotation
                        .filter(|(_, t)| now_us > *t)
                        .map_or_else(crate::math::V3::zeros, |(previous, t)| {
                            let world_rate = (rotation * previous.inverse()).scaled_axis()
                                / ((now_us - t) as f64 / 1e6);
                            trackers::mounting(&slot.id, self.spec.seed)
                                * rotation.inverse()
                                * world_rate
                        });
                    slot.previous_rotation = Some((rotation, now_us));
                    emit(
                        &slot.id,
                        SourceEvent::Observation {
                            observation: SensorObservation {
                                device_id: slot.id.clone(),
                                source_id: self.spec.id.clone(),
                                origin: Origin::Simulated,
                                seq: slot.seq,
                                t_us,
                                device_time_us: Some(now_us),
                                time_quality,
                                calibration_ref: None,
                                confidence: Some(slot.fault.quality),
                                data: ObservationData::AngularVelocity {
                                    frame: FrameRef::Sensor {
                                        id: slot.id.clone(),
                                    },
                                    rad_s: crate::math::v3_to_wire(&rate),
                                },
                            },
                        },
                    );
                    metrics.emitted_observations += 1;
                    let q = trackers::reading(
                        &slot.id,
                        self.spec.seed,
                        slot.seq,
                        noise_deg,
                        heading_offset_deg,
                        pose.rotations[index],
                    );
                    SourceEvent::Observation {
                        observation: SensorObservation {
                            device_id: slot.id.clone(),
                            source_id: self.spec.id.clone(),
                            origin: Origin::Simulated,
                            seq: slot.seq,
                            t_us,
                            device_time_us: Some(now_us),
                            time_quality,
                            calibration_ref: None,
                            confidence: Some(slot.fault.quality),
                            data: ObservationData::Orientation {
                                frame: FrameRef::SensorReference {
                                    id: slot.id.clone(),
                                },
                                q,
                            },
                        },
                    }
                }
                DeviceKindState::Camera { spec_index } => {
                    let camera = &self.spec.cameras[spec_index];
                    let points = if self.spec.landmark_schema == BODY_KEYPOINT_SCHEMA_ID {
                        pose.keypoints(&self.skeleton)
                    } else {
                        pose.positions.clone()
                    };
                    let landmarks =
                        camera::project(camera, self.spec.seed, slot.seq, t_us, &points);
                    let mut landmarks = landmarks;
                    landmarks.frame_meta.time_quality = time_quality;
                    SourceEvent::Observation {
                        observation: SensorObservation {
                            device_id: slot.id.clone(),
                            source_id: self.spec.id.clone(),
                            origin: Origin::Simulated,
                            seq: slot.seq,
                            t_us,
                            device_time_us: Some(now_us),
                            time_quality,
                            calibration_ref: None,
                            confidence: Some(1.0),
                            data: ObservationData::Landmarks2d { landmarks },
                        },
                    }
                }
            };
            emit(&slot.id, event);
            metrics.emitted_observations += 1;
        }
        metrics
    }
}

fn next_scenario_transition(
    scenario: &[crate::config::ScenarioStep],
    period_s: Option<f64>,
    sim_s: f64,
    phase: f64,
) -> Option<f64> {
    if let Some(period) = period_s {
        let next_phase = scenario
            .iter()
            .map(|step| step.at_s)
            .filter(|&at| at > phase && at < period)
            .fold(period, f64::min);
        Some(sim_s + (next_phase - phase))
    } else {
        scenario
            .iter()
            .map(|step| step.at_s)
            .filter(|&at| at > sim_s)
            .reduce(f64::min)
    }
}

impl Source for SimulatedSource {
    type Context = SimulatedContext;
    async fn run(self, mut context: SimulatedContext) -> Result<(), crate::sources::SourceError> {
        let start_us = context.clock.now_us();
        let initial_control = *context.control.borrow();
        let mut scheduler = Scheduler::new(self.spec, self.skeleton, start_us, initial_control);
        let mut timer = crate::ticker::DeadlineTimer::new();
        let mut control_open = true;
        let mut faults_open = true;
        loop {
            let now_us = context.clock.now_us();
            {
                let assignments = self.assignments.borrow();
                let faults = context.faults.borrow();
                scheduler.step(now_us, &assignments, &faults, |id, event| {
                    send(&context, id, event);
                });
            }
            let next_deadline = scheduler
                .next_deadline_us()
                .map(|t| context.clock.instant_at(t));
            tokio::select! {
                _ = context.common.cancel.cancelled() => break,
                res = context.control.changed(), if control_open => {
                    match res {
                        Ok(()) => {
                            let now = context.clock.now_us();
                            let control = *context.control.borrow_and_update();
                            scheduler.on_control(now, control);
                        }
                        Err(_) => control_open = false,
                    }
                }
                res = context.faults.changed(), if faults_open => {
                    match res {
                        Ok(()) => {
                            let now = context.clock.now_us();
                            scheduler.on_faults(now);
                        }
                        Err(_) => faults_open = false,
                    }
                }
                _ = async {
                    if let Some(deadline) = next_deadline {
                        timer.wait_until(deadline).await;
                    }
                }, if next_deadline.is_some() => {}
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
