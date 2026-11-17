use super::{
    command::{Client, ControlError},
    features::features,
    joints::measure,
    solve::solve,
};
use crate::{
    calibration::Store,
    clock::SessionClock,
    config::Validated,
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{Control, motions::Motion},
};
use bansayo_contracts::*;
use std::collections::BTreeMap;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
pub struct Streams {
    pub state: watch::Receiver<SystemState>,
    pub motion: watch::Receiver<std::sync::Arc<MotionSample>>,
    pub frame: watch::Receiver<std::sync::Arc<super::encoded::Encoded>>,
    pub state_wire: watch::Receiver<std::sync::Arc<super::encoded::Encoded>>,
    pub landmarks: broadcast::Sender<std::sync::Arc<super::encoded::Encoded>>,
    pub skeleton: watch::Receiver<SkeletonDefinition>,
    pub epoch: String,
    pub notices: broadcast::Sender<std::sync::Arc<super::encoded::Encoded>>,
}
pub struct Runner {
    pub config: Validated,
    pub registry: Registry,
    pub store: Store,
    pub skeleton: Skeleton,
    pub clock: SessionClock,
    pub events: mpsc::Receiver<crate::sources::Ingest>,
    pub controls: Vec<watch::Sender<Control>>,
    pub faults: watch::Sender<BTreeMap<DeviceId, crate::sources::simulated::FaultSetting>>,
    /// Where simulated trackers are strapped; mirrors `registry.assignments`.
    pub placements: watch::Sender<BTreeMap<DeviceId, SegmentId>>,
}
impl Runner {
    pub fn spawn(
        self,
        cancel: CancellationToken,
    ) -> (Client, Streams, tokio::task::JoinHandle<()>) {
        let state = self.state(1);
        let solved = solve(
            &self.skeleton,
            &self.registry,
            &self.store,
            0,
            &self.config.config,
        );
        let initial_motion =
            super::motion::sample(&self.skeleton, &self.registry, &solved, 0, 0, vec![]);
        let (motion_tx, motion_rx) = watch::channel(std::sync::Arc::new(initial_motion));
        let frame = ServerMessage::Frame {
            seq: 0,
            t_us: 0,
            origins: vec![],
            pose: solved.pose,
            joints: measure(&self.skeleton, &solved.rotations, &solved.observed),
            quality: solved.quality,
        };
        let (state_wire_tx, state_wire_rx) = watch::channel(super::encoded::Encoded::new(
            &ServerMessage::State {
                state: state.clone(),
            },
            state.rev,
        ));
        let (state_tx, state_rx) = watch::channel(state);
        let (frame_tx, frame_rx) = watch::channel(super::encoded::Encoded::new(&frame, 0));
        let (skeleton_tx, skeleton_rx) = watch::channel(self.skeleton.definition.clone());
        let (landmarks, _) = broadcast::channel(64);
        let (notices, _) = broadcast::channel(32);
        let streams = Streams {
            motion: motion_rx,
            state: state_rx,
            state_wire: state_wire_rx,
            frame: frame_rx,
            landmarks: landmarks.clone(),
            skeleton: skeleton_rx,
            epoch: self.clock.epoch_utc().into(),
            notices: notices.clone(),
        };
        let (tx, rx) = mpsc::channel(32);
        let client = Client {
            commands: tx.clone(),
        };
        let task = tokio::spawn(
            super::runtime::Runtime::new(
                tx,
                state_tx,
                (frame_tx, motion_tx),
                landmarks,
                notices,
                skeleton_tx,
                state_wire_tx,
            )
            .run(self, rx, cancel),
        );
        (client, streams, task)
    }
    pub(super) fn simulation(&self) -> Option<SimulationState> {
        let spec = self.config.simulated.first()?;
        let c = *self.controls.first()?.borrow();
        Some(SimulationState {
            source_id: spec.id.clone(),
            motion: c.motion.id().into(),
            speed: c.speed as f32,
            paused: c.paused,
            available_motions: Motion::ALL
                .into_iter()
                .map(|m| MotionOption {
                    id: m.id().into(),
                    label: m.label().into(),
                })
                .collect(),
        })
    }
    pub(super) fn state(&self, rev: u64) -> SystemState {
        let now = self.clock.now_us();
        let devices = self
            .registry
            .status(now, &self.config.config.quality, &self.store);
        let solved = solve(
            &self.skeleton,
            &self.registry,
            &self.store,
            now,
            &self.config.config,
        );
        let features = features(&devices, &solved);
        SystemState {
            capability_report: Some(super::features::capability_report(
                &self.skeleton,
                &self.registry,
                &self.store,
                &solved,
                &self.config.config,
                now,
            )),
            rev,
            engine: EngineInfo {
                version: env!("CARGO_PKG_VERSION").into(),
                protocol_version: PROTOCOL_VERSION,
                mode: EngineMode::Live,
                state: solved.quality.state,
                uptime_s: now as f64 / 1e6,
            },
            devices,
            calibration: self.store.summaries(),
            features,
            recording: None,
            playback: None,
            simulation: self.simulation(),
        }
    }
    pub(super) fn snapshot_with(
        &self,
        assignments: &BTreeMap<DeviceId, SegmentId>,
        stale: &std::collections::BTreeSet<ProfileId>,
        profiles: &BTreeMap<ProfileId, CalibrationProfile>,
    ) -> crate::registry::persistence::RuntimeSnapshot {
        let known = self
            .registry
            .entries
            .iter()
            .filter(|(_, entry)| entry.descriptor.kind == DeviceKind::ImuTracker)
            .map(|(id, _)| id.clone())
            .chain(self.config.assignments.keys().cloned());
        crate::registry::persistence::RuntimeSnapshot::from_state(
            assignments,
            known,
            stale,
            profiles,
        )
    }
    pub(super) fn update_simulation(
        &mut self,
        r: SimulationUpdateRequest,
    ) -> Result<SimulationState, ControlError> {
        let sender = self.controls.first().ok_or_else(|| {
            ControlError::new(ApiErrorCode::NotFound, "No simulated source is configured.")
        })?;
        let mut c = *sender.borrow();
        if let Some(m) = r.motion {
            c.motion = Motion::from_id(&m).ok_or_else(|| {
                ControlError::new(
                    ApiErrorCode::InvalidRequest,
                    "Choose one of the available simulation motions.",
                )
            })?;
            c.generation += 1;
        }
        if let Some(speed) = r.speed {
            if !(speed.is_finite() && speed > 0.0 && speed <= 10.0) {
                return Err(ControlError::new(
                    ApiErrorCode::InvalidRequest,
                    "Simulation speed must be greater than 0 and at most 10.",
                ));
            }
            c.speed = speed.into();
        }
        if let Some(p) = r.paused {
            c.paused = p;
        }
        sender.send_replace(c);
        self.simulation().ok_or_else(|| {
            ControlError::new(ApiErrorCode::NotFound, "No simulated source is configured.")
        })
    }
}
