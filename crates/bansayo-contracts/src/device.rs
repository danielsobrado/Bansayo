//! Device descriptors and live device status.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    CalibrationRef, DeviceId, LandmarkSchema, Origin, Reason, SegmentId, SourceId, TimeQuality,
    TimeUs, TrackingState,
};

/// Broad device category. Capabilities, not kind, decide what a device provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    ImuTracker,
    Camera,
}

/// How the engine reaches a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Simulated,
    File,
    Udp,
    Usb,
    Websocket,
}

/// A kind of observation a device can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SensorCapability {
    /// `q_sensor_reference_from_sensor`.
    Orientation,
    /// Gyroscope, rad/s in the sensor frame.
    AngularVelocity,
    /// Accelerometer, m/s² in the sensor frame.
    Acceleration,
    /// Position, metres in a named frame.
    Position,
    /// Range to a surface or peer, metres.
    Distance,
    /// Contact state, optional load in newtons.
    Contact,
    /// Image frames.
    Image,
    /// 2D body landmarks, pixels in the image frame.
    #[serde(rename = "landmarks_2d")]
    Landmarks2d,
    /// Model-inferred 3D landmarks, model units in the model frame.
    #[serde(rename = "landmarks_3d_model")]
    Landmarks3dModel,
    /// Battery level, fraction.
    Battery,
    /// Signal strength, dBm.
    SignalStrength,
    /// Device-reported quality, fraction.
    Quality,
}

/// Camera-specific descriptor fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CameraInfo {
    /// Image width and height, pixels.
    pub resolution_px: [u32; 2],
    pub landmark_schema: Option<LandmarkSchema>,
}

/// Static description of a device, announced by its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DeviceDescriptor {
    pub id: DeviceId,
    pub source_id: SourceId,
    pub kind: DeviceKind,
    pub origin: Origin,
    /// Hardware model, e.g. `ESP32-C3 + ICM-45686`.
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub transport: Transport,
    pub capabilities: Vec<SensorCapability>,
    /// Nominal output rate, Hz.
    pub nominal_rate_hz: Option<f32>,
    /// Present for cameras.
    pub camera: Option<CameraInfo>,
}

/// Statistics of a device's observation stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct StreamStatus {
    /// Measured observation rate, Hz.
    pub measured_rate_hz: f32,
    /// Session time of the last accepted observation.
    pub last_seen_t_us: Option<TimeUs>,
    /// Observations accepted.
    pub received: u64,
    /// Observations lost in transport (seq gaps) or dropped at a full queue.
    pub dropped: u64,
    pub time_quality: TimeQuality,
}

/// Battery report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct BatteryStatus {
    /// Charge level, 0..1.
    pub level: f32,
    pub charging: Option<bool>,
}

/// Radio signal report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SignalStatus {
    /// Received signal strength, dBm.
    pub rssi_dbm: i16,
}

/// Live status of one device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DeviceStatus {
    pub descriptor: DeviceDescriptor,
    pub state: TrackingState,
    pub reasons: Vec<Reason>,
    /// Body segment this device is assigned to.
    pub assignment: Option<SegmentId>,
    pub stream: StreamStatus,
    pub battery: Option<BatteryStatus>,
    pub signal: Option<SignalStatus>,
    /// Device-reported quality, 0..1.
    pub quality: Option<f32>,
    /// The calibration kinds relevant to this device.
    pub calibration: Vec<CalibrationRef>,
    /// Physical cameras: the capture settings in effect. Additive under protocol version 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub camera_settings: Option<crate::CameraSettingsStatus>,
}
