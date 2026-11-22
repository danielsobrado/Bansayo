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
    /// An OpenXR headset or controller reporting 6-DoF poses in XR stage space. Additive.
    XrTrackedDevice,
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
    /// A network camera's RTSP stream (IP cameras). Additive.
    Rtsp,
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
    /// Atomic position + orientation of a tracked device in a named frame. Additive.
    RigidPose,
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
    /// Physical cameras: identity, lifecycle, capture mode and measured frame timing. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub camera_runtime: Option<CameraRuntimeStatus>,
    /// Devices with their own clock: how their timestamps map onto the session clock. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub time_sync: Option<TimeSyncStatus>,
}

/// Where a physical camera is in its lifecycle (08 §3, ADR-020).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CameraLifecycle {
    /// Configured; the hardware has not been seen yet.
    Discovered,
    /// The configured identity matched exactly one present camera.
    Identified,
    /// Opening the stream and applying the capture mode.
    Configuring,
    Active,
    /// Streaming, but slower than requested or with settings not applied.
    Degraded,
    Disconnected,
    /// Lost after being active; reopening with the same mode and calibration.
    Reconnecting,
}

/// Physical identity of a camera, as far as the operating system exposes it. The index in the
/// system list is never part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CameraIdentity {
    pub name: String,
    /// USB vendor and product id, lower-case hex, when the device is USB.
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    /// A serial number, only when the operating system exposes one.
    pub serial: Option<String>,
    /// Stable operating-system endpoint (on Windows the device interface path); changes with the
    /// USB port for cameras without a serial number.
    pub interface_id: Option<String>,
    /// `usb:<vid>:<pid>:<serial or interface>`, or the endpoint for other buses.
    pub fingerprint: String,
}

/// One capture mode: size, pixel format and nominal rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CaptureMode {
    pub resolution_px: [u32; 2],
    pub pixel_format: String,
    /// Nominal frames per second; `null` when the backend does not report it reliably.
    pub fps: Option<f32>,
    /// The device advertised this mode, or the open succeeded with exactly this mode.
    pub verified: bool,
}

/// Measured frame arrival intervals over a recent window, monotonic session clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct FrameTiming {
    pub window_frames: u32,
    pub measured_fps: f32,
    pub mean_interval_ms: f32,
    pub p95_interval_ms: f32,
    pub p99_interval_ms: f32,
    /// Standard deviation of the interval.
    pub jitter_ms: f32,
}

/// Live capture state of one physical camera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CameraRuntimeStatus {
    pub lifecycle: CameraLifecycle,
    pub identity: Option<CameraIdentity>,
    /// `false` when the matched hardware differs from the hardware this camera's calibration was
    /// made with; its calibration is then not used until the user confirms the identity.
    pub identity_confirmed: bool,
    pub requested: Option<CaptureMode>,
    pub negotiated: Option<CaptureMode>,
    pub timing: Option<FrameTiming>,
    /// Frames replaced in the newest-frame buffer before inference took them (not transport loss).
    pub replaced_frames: u64,
    /// Pose inference rate for this camera, Hz.
    pub inference_hz: Option<f32>,
    /// Decode time of the last frame, milliseconds.
    pub decode_ms: Option<f32>,
    pub reconnects: u32,
    pub reasons: Vec<Reason>,
}

/// How a device's own clock is mapped onto the session clock (docs/03 §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TimeSyncStatus {
    pub time_quality: TimeQuality,
    /// Session minus device time, microseconds, when estimated.
    pub offset_us: Option<i64>,
    /// Bound on the residual transport delay the offset cannot remove, microseconds.
    pub uncertainty_us: Option<u32>,
    /// Device clock rate error, parts per million, once enough time has passed to estimate it.
    pub drift_ppm: Option<f32>,
    pub settled: bool,
    /// Device clock resets (reboot, wraparound, jump) seen since start.
    pub resets: u32,
}
