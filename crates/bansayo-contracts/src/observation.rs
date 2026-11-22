//! Sensor observations and source events: the engine's input, recorded and replayed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    DeviceDescriptor, DeviceId, FrameRef, Origin, PoseLandmarks2D, PoseLandmarks3D, ProfileId,
    Quat, SourceId, TimeQuality, TimeUs, Vec3,
};

/// One measurement from one device, with its source, time, frame and calibration made explicit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SensorObservation {
    pub device_id: DeviceId,
    pub source_id: SourceId,
    pub origin: Origin,
    /// Sequence number per (device, observation kind); gaps count as loss.
    pub seq: u64,
    /// Capture instant, session clock.
    pub t_us: TimeUs,
    /// Device clock, if the device has one.
    pub device_time_us: Option<u64>,
    pub time_quality: TimeQuality,
    /// Profile already applied by the adapter, if any.
    pub calibration_ref: Option<ProfileId>,
    /// 0..1, or `null` when nothing meaningful can be said.
    pub confidence: Option<f32>,
    pub data: ObservationData,
}

/// The measured value. Units are in the field names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationData {
    /// `q` is `q_frame_from_sensor`.
    Orientation { frame: FrameRef, q: Quat },
    /// Radians per second.
    AngularVelocity { frame: FrameRef, rad_s: Vec3 },
    /// Metres per second squared.
    Acceleration { frame: FrameRef, m_s2: Vec3 },
    /// Metres.
    Position { frame: FrameRef, m: Vec3 },
    /// Range in metres.
    Distance { m: f32 },
    /// Contact state and optional load in newtons.
    Contact {
        in_contact: bool,
        force_n: Option<f32>,
    },
    #[serde(rename = "landmarks_2d")]
    Landmarks2d { landmarks: PoseLandmarks2D },
    #[serde(rename = "landmarks_3d")]
    Landmarks3d { landmarks: PoseLandmarks3D },
    /// Battery level 0..1, RSSI in dBm, device-reported quality 0..1.
    Diagnostics {
        battery_level: Option<f32>,
        charging: Option<bool>,
        rssi_dbm: Option<i16>,
        quality: Option<f32>,
    },
    /// Atomic 6-DoF device pose: `rotation` is `q_frame_from_device`, position in metres.
    RigidPose {
        frame: FrameRef,
        position_m: Vec3,
        rotation: Quat,
    },
}

/// Everything a source emits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceEvent {
    DeviceAnnounced {
        descriptor: DeviceDescriptor,
    },
    /// Transport link change at session time `t_us`.
    DeviceLink {
        device_id: DeviceId,
        connected: bool,
        t_us: TimeUs,
    },
    Observation {
        observation: SensorObservation,
    },
}
