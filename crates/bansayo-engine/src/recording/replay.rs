use super::reader::{RecordedData, Recording};
use crate::{
    calibration::Store,
    pipeline::{joints::measure, solve::solve},
    registry::Registry,
    skeleton::Skeleton,
};
use bansayo_contracts::*;
/// Solver ticks per `advance` call; a longer backlog continues on the next call.
pub const MAX_TICKS_PER_ADVANCE: usize = 256;
pub struct Replay {
    pub recording: Recording,
    pub registry: Registry,
    pub store: Store,
    pub skeleton: Skeleton,
    pub paused: bool,
    pub speed: f32,
    pub looping: bool,
    pub position_us: u64,
    pub latest_motion: Option<MotionSample>,
    /// Re-analysis opens the planar capture window at the session's creation (04 §4).
    pub planar: crate::pipeline::planar::PlanarState,
    cursor: usize,
    next_tick: u64,
    clock_position: f64,
}
impl Replay {
    pub fn new(recording: Recording, speed: f32, looping: bool) -> Result<Self, String> {
        let (registry, store, skeleton) = recording.initial.restore()?;
        Ok(Self {
            recording,
            registry,
            store,
            skeleton,
            paused: false,
            speed,
            looping,
            position_us: 0,
            latest_motion: None,
            planar: Default::default(),
            cursor: 0,
            next_tick: 0,
            clock_position: 0.0,
        })
    }
    pub fn state(&self) -> PlaybackState {
        PlaybackState {
            session: self.recording.session.clone(),
            paused: self.paused,
            speed: self.speed,
            position_us: self.position_us,
            looping: self.looping,
        }
    }
    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        let (registry, store, skeleton) = self.recording.initial.restore()?;
        self.registry = registry;
        self.store = store;
        self.skeleton = skeleton;
        self.cursor = 0;
        self.position_us = position;
        self.clock_position = position as f64;
        self.apply_until(self.recording.initial.started_t_us + position, None)?;
        let time = self.recording.initial.started_t_us + position;
        let sample = self.solve_at(time).1;
        self.latest_motion = Some(self.with_planar(sample, time));
        self.next_tick = (position as f64 * self.rate() / 1e6).ceil() as u64;
        Ok(())
    }
    fn with_planar(&mut self, mut sample: MotionSample, time: u64) -> MotionSample {
        sample.planar = self.planar.update(
            &self.registry,
            time,
            self.recording.initial.config.engine.stale_after_ms * 1000,
        );
        sample
    }
    fn rate(&self) -> f64 {
        self.recording.initial.config.engine.publish_rate_hz
    }
    fn tick_offset(&self, tick: u64) -> u64 {
        (tick as f64 * 1e6 / self.rate()).round() as u64
    }
    /// Applies every recorded event accepted at or before `time`, in recorded order, and appends
    /// the 2D landmarks of the applied camera observations to `landmarks`.
    fn apply_until(
        &mut self,
        time: u64,
        mut landmarks: Option<&mut Vec<ServerMessage>>,
    ) -> Result<(), String> {
        while let Some(recorded) = self
            .recording
            .events
            .get(self.cursor)
            .filter(|e| e.accepted_us <= time)
        {
            match &recorded.data {
                RecordedData::Event(event) => {
                    if let (Some(out), SourceEvent::Observation { observation }) =
                        (landmarks.as_deref_mut(), event.as_ref())
                    {
                        out.extend(landmarks_2d(observation));
                    }
                    self.registry.event(*event.clone());
                }
                RecordedData::Context(context) => {
                    let (registry, store, skeleton) = context.restore()?;
                    self.registry = registry;
                    self.store = store;
                    self.skeleton = skeleton;
                }
            }
            self.cursor += 1;
        }
        Ok(())
    }
    /// The frame at `time`, preceded by the triangulated landmarks when there are any.
    fn solve_at(&self, time: u64) -> (Vec<ServerMessage>, MotionSample) {
        let solved = solve(
            &self.skeleton,
            &self.registry,
            &self.store,
            time,
            &self.recording.initial.config,
        );
        let sample = crate::pipeline::motion::sample(
            &self.skeleton,
            &self.registry,
            &solved,
            0,
            time,
            self.recording.session.origins.clone(),
        );
        let mut joints = measure(&self.skeleton, &solved.rotations, &solved.observed);
        crate::pipeline::joints::confidence(&self.skeleton, &mut joints, solved.pose.as_ref());
        let landmarks = solved
            .landmarks_3d
            .map(|landmarks| ServerMessage::Landmarks3d { landmarks });
        let messages = landmarks
            .into_iter()
            .chain([ServerMessage::Frame {
                seq: 0,
                t_us: time,
                origins: self.recording.session.origins.clone(),
                joints,
                pose: solved.pose,
                quality: solved.quality,
            }])
            .collect();
        (messages, sample)
    }
    /// The state at the current position, with each camera's latest landmarks unless stale there.
    pub fn preview(&self) -> Vec<ServerMessage> {
        let time = self.recording.initial.started_t_us + self.position_us;
        let stale_us = self.recording.initial.config.engine.stale_after_ms * 1000;
        self.registry
            .entries
            .values()
            .filter_map(|e| e.latest.get(&crate::registry::ObservationKind::Landmarks2d))
            .filter(|o| time.saturating_sub(o.t_us) <= stale_us)
            .filter_map(landmarks_2d)
            .chain(self.solve_at(time).0)
            .collect()
    }
    /// Advances the virtual clock. Solver ticks run at fixed virtual intervals, each after the
    /// events accepted by its time; once no tick is pending, events up to the virtual target are
    /// applied too, so observations after the last tick are not lost. Recorded 2D landmarks come
    /// out once each, in recorded order. The output depends only on the virtual time reached, not
    /// on how it was divided into calls.
    pub fn advance(&mut self, elapsed_us: u64) -> Result<Vec<ServerMessage>, String> {
        if self.paused {
            return Ok(vec![]);
        }
        self.clock_position += elapsed_us as f64 * self.speed as f64;
        let duration = self.recording.session.duration_us;
        let started = self.recording.initial.started_t_us;
        let target = (self.clock_position as u64).min(duration);
        let mut messages = vec![];
        let mut ticks = 0;
        loop {
            let offset = self.tick_offset(self.next_tick);
            if offset > target {
                break;
            }
            if ticks == MAX_TICKS_PER_ADVANCE {
                // Backlog: the position stops at the last solved tick; later events wait for it.
                return Ok(messages);
            }
            self.apply_until(started + offset, Some(&mut messages))?;
            let (frames, sample) = self.solve_at(started + offset);
            self.latest_motion = Some(self.with_planar(sample, started + offset));
            messages.extend(frames);
            self.position_us = offset;
            self.next_tick += 1;
            ticks += 1;
        }
        self.apply_until(started + target, Some(&mut messages))?;
        self.position_us = target;
        if self.clock_position >= duration as f64 {
            if self.looping && duration > 0 {
                self.seek(0)?;
            } else {
                self.paused = true;
            }
        }
        Ok(messages)
    }
}
fn landmarks_2d(observation: &SensorObservation) -> Option<ServerMessage> {
    match &observation.data {
        ObservationData::Landmarks2d { landmarks } => Some(ServerMessage::Landmarks2d {
            landmarks: landmarks.clone(),
        }),
        _ => None,
    }
}
