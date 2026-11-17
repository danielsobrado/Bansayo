//! System state: the single object describing the engine for clients.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    CalibrationProfileSummary, DeviceStatus, EngineMode, FeatureStatus, PlaybackState,
    RecordingState, SimulationState, TrackingState,
};

/// Engine identity and overall state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct EngineInfo {
    pub version: String,
    pub protocol_version: u32,
    pub mode: EngineMode,
    pub state: TrackingState,
    /// Seconds since engine start.
    pub uptime_s: f64,
}

/// Everything clients need to render status. Sent on change and at 1 Hz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SystemState {
    /// Increments on every change.
    pub rev: u64,
    /// Additive under protocol version 1; absent in older recordings and engines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub capability_report: Option<crate::CapabilityReport>,
    pub engine: EngineInfo,
    pub devices: Vec<DeviceStatus>,
    pub calibration: Vec<CalibrationProfileSummary>,
    pub features: Vec<FeatureStatus>,
    pub recording: Option<RecordingState>,
    pub playback: Option<PlaybackState>,
    /// `null` when no simulated source is configured.
    pub simulation: Option<SimulationState>,
}
