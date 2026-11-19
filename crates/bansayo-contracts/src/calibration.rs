//! Calibration kinds, states, summaries and the persisted profile.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::{DeviceId, Origin, ProfileId, Quat, SegmentId, Vec3};

/// `schema_version` written into every persisted [`CalibrationProfile`].
pub const CALIBRATION_PROFILE_SCHEMA_VERSION: u32 = 1;

/// What a calibration profile calibrates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationKind {
    ImuBias,
    /// `q_sensor_from_segment` for an assigned tracker.
    Mounting,
    /// `q_world_from_sensor_reference`, yaw only.
    HeadingAlignment,
    BodyProportions,
    CameraIntrinsics,
    /// `T_world_from_camera`.
    CameraExtrinsics,
    FloorPlane,
    /// `T_xr_stage_from_world`.
    XrAlignment,
    TimeSync,
}

/// State of a calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationState {
    /// No profile exists; dependent values are not derived.
    Missing,
    /// Profile exists and its dependencies hold.
    Valid,
    /// Something the profile depended on changed; not used.
    Stale,
    /// No calibration performed; a documented default is in use and flagged.
    Assumed,
    /// Could not be loaded or failed a consistency check; not used.
    Invalid,
}

/// The state of one calibration kind relevant to a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CalibrationRef {
    pub kind: CalibrationKind,
    pub state: CalibrationState,
    pub profile_id: Option<ProfileId>,
}

/// A calibration profile as listed in the system state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CalibrationProfileSummary {
    pub id: ProfileId,
    pub kind: CalibrationKind,
    pub state: CalibrationState,
    pub device_id: Option<DeviceId>,
    pub segment_id: Option<SegmentId>,
    /// How it was obtained, e.g. `reference_pose`, `declared_by_simulator`, `default`.
    pub method: String,
    /// RFC 3339 UTC creation time; `null` for defaults.
    pub created_utc: Option<String>,
    /// The profile's `data` for camera kinds (`CameraIntrinsicsData`, `CameraExtrinsicsData`),
    /// so clients can draw cameras without a second request. Omitted for other kinds.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    #[ts(type = "Record<string, unknown>", optional)]
    pub data: Map<String, Value>,
}

/// What a profile is keyed by: a device, a segment, or both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CalibrationSubject {
    pub device_id: Option<DeviceId>,
    pub segment_id: Option<SegmentId>,
}

/// Another calibration this profile depends on; a change to it makes this profile stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CalibrationDependency {
    pub kind: CalibrationKind,
    pub device_id: Option<DeviceId>,
}

/// A persisted, versioned calibration result: one JSON file in `<data_dir>/calibration/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CalibrationProfile {
    /// Currently 1.
    pub schema_version: u32,
    pub id: ProfileId,
    pub kind: CalibrationKind,
    /// How it was obtained, e.g. `reference_pose`.
    pub method: String,
    pub origin: Origin,
    /// RFC 3339 UTC creation time.
    pub created_utc: String,
    pub subject: CalibrationSubject,
    pub depends_on: Vec<CalibrationDependency>,
    /// Kind-specific result, e.g. `MountingData` for `mounting`.
    #[ts(type = "Record<string, unknown>")]
    pub data: Map<String, Value>,
    /// Kind-specific capture quality, e.g. `MountingQuality` for `mounting`.
    #[ts(type = "Record<string, unknown>")]
    pub quality: Map<String, Value>,
}

/// `data` of a `mounting` profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MountingData {
    /// Rotation from the segment frame into the sensor body frame.
    pub q_sensor_from_segment: Quat,
}

/// `quality` of a `mounting` profile captured by reference pose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MountingQuality {
    /// Length of the capture window, milliseconds.
    pub capture_window_ms: u32,
    /// Angular spread of the readings over the window, radians.
    pub angular_spread_rad: f32,
}

/// `data` of a `camera_intrinsics` profile: pinhole model with Brown-Conrady distortion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CameraIntrinsicsData {
    /// Image width and height the intrinsics were calibrated at, pixels.
    pub image_size_px: [u32; 2],
    pub fx_px: f64,
    pub fy_px: f64,
    pub cx_px: f64,
    pub cy_px: f64,
    /// `[k1, k2, p1, p2, k3]` (OpenCV order); all zero for an ideal pinhole.
    pub distortion: [f64; 5],
}

/// `data` of a `camera_extrinsics` profile: `T_world_from_camera`.
///
/// The camera frame is `FrameRef::Camera`: +X right, +Y down, +Z along the optical axis.
/// `p_world = q_world_from_camera * p_camera + t_world_from_camera_m`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CameraExtrinsicsData {
    pub q_world_from_camera: Quat,
    /// Camera optical centre in world, metres.
    pub t_world_from_camera_m: Vec3,
    /// The room (rig placement) this pose belongs to; `null` is the default room. A camera keeps
    /// one intrinsics profile and one extrinsics profile per room. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub room: Option<String>,
}

/// A room name: lowercase letters, digits, `_` and `-`, starting with a letter or digit, at most
/// 32 bytes, so it can form part of a profile id.
pub fn valid_room(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}
