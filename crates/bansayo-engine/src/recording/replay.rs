use super::reader::{RecordedData, Recording};
use crate::{
    calibration::Store,
    pipeline::{joints::measure, solve::solve},
    registry::Registry,
    skeleton::Skeleton,
};
use kinexa_contracts::*;
pub struct Replay {
    pub recording: Recording,
    pub registry: Registry,
    pub store: Store,
    pub skeleton: Skeleton,
    pub paused: bool,
    pub speed: f32,
    pub looping: bool,
    pub position_us: u64,
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
        self.apply_until(self.recording.initial.started_t_us + position)?;
        self.next_tick = (position as f64 * self.rate() / 1e6).ceil() as u64;
        Ok(())
    }
    fn rate(&self) -> f64 {
        self.recording.initial.config.engine.publish_rate_hz
    }
    fn apply_until(&mut self, time: u64) -> Result<(), String> {
        while let Some(event) = self
            .recording
            .events
            .get(self.cursor)
            .filter(|e| e.accepted_us <= time)
        {
            match &event.data {
                RecordedData::Event(event) => self.registry.event(*event.clone()),
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
    pub fn preview(&self) -> ServerMessage {
        let time = self.recording.initial.started_t_us + self.position_us;
        let solved = solve(
            &self.skeleton,
            &self.registry,
            &self.store,
            time,
            self.recording.initial.config.engine.stale_after_ms * 1000,
        );
        let mut joints = measure(&self.skeleton, &solved.rotations, &solved.observed);
        crate::pipeline::joints::confidence(&self.skeleton, &mut joints, solved.pose.as_ref());
        ServerMessage::Frame {
            seq: 0,
            t_us: time,
            origins: self.recording.session.origins.clone(),
            joints,
            pose: solved.pose,
            quality: solved.quality,
        }
    }
    /// Fixed virtual ticks are independent of wall-clock pacing. The caller publishes only latest.
    pub fn advance(&mut self, elapsed_us: u64) -> Result<Vec<ServerMessage>, String> {
        if self.paused {
            return Ok(vec![]);
        }
        self.clock_position += elapsed_us as f64 * self.speed as f64;
        let duration = self.recording.session.duration_us;
        let target = (self.clock_position as u64).min(duration);
        let mut frames = vec![];
        loop {
            let offset = (self.next_tick as f64 * 1e6 / self.rate()).round() as u64;
            if offset > target {
                break;
            }
            let time = self.recording.initial.started_t_us + offset;
            self.apply_until(time)?;
            let solved = solve(
                &self.skeleton,
                &self.registry,
                &self.store,
                time,
                self.recording.initial.config.engine.stale_after_ms * 1000,
            );
            let mut joints = measure(&self.skeleton, &solved.rotations, &solved.observed);
            crate::pipeline::joints::confidence(&self.skeleton, &mut joints, solved.pose.as_ref());
            frames.push(ServerMessage::Frame {
                seq: 0,
                t_us: time,
                origins: self.recording.session.origins.clone(),
                joints,
                pose: solved.pose,
                quality: solved.quality,
            });
            self.next_tick += 1;
            // A pathological scheduling pause cannot allocate an unbounded tick batch.
            if frames.len() >= 256 {
                break;
            }
        }
        self.position_us = target;
        if self.clock_position >= duration as f64
            && (self.next_tick as f64 * 1e6 / self.rate()).round() as u64 > duration
        {
            if self.looping && duration > 0 {
                self.seek(0)?;
            } else {
                self.paused = true;
            }
        }
        Ok(frames)
    }
}
