//! Calibration kinds, states, summaries and the persisted profile.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::{DeviceId, Origin, ProfileId, Quat, SegmentId};

/// `schema_version` written into every persisted [`CalibrationProfile`].
pub const CALIBRATION_PROFILE_SCHEMA_VERSION: u32 = 1;

/// What a calibration profile calibrates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
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
