//! Recording sessions, recording and playback state, simulation state.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Origin, SessionId, SourceId, TimeUs};

/// Metadata of a recorded session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RecordingSession {
    pub id: SessionId,
    pub name: String,
    /// RFC 3339 UTC.
    pub created_utc: String,
    /// Microseconds.
    pub duration_us: u64,
    pub size_bytes: u64,
    pub origins: Vec<Origin>,
    pub device_count: u32,
    pub engine_version: String,
    pub protocol_version: u32,
}

/// An active recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RecordingState {
    pub session_id: SessionId,
    /// Session time recording started.
    pub started_t_us: TimeUs,
    pub events: u64,
    /// Events dropped because the recorder could not keep up.
    pub dropped_events: u64,
}

/// An active playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PlaybackState {
    pub session: RecordingSession,
    pub paused: bool,
    /// Playback speed multiplier, 1 = real time.
    pub speed: f32,
    /// Microseconds, `0 ..= session.duration_us`.
    pub position_us: u64,
    pub looping: bool,
}

/// A motion the simulator can play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MotionOption {
    pub id: String,
    pub label: String,
}

/// State of the simulated source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SimulationState {
    pub source_id: SourceId,
    /// Id of the current motion.
    pub motion: String,
    pub available_motions: Vec<MotionOption>,
    /// Speed multiplier, 1 = real time.
    pub speed: f32,
    pub paused: bool,
}
