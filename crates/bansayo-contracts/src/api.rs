//! HTTP API (`/api/v1`): response envelope, error codes, request and small response bodies.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use ts_rs::TS;

use crate::{DeviceId, SegmentId, SessionId};

/// A JSON boolean fixed to `V`: serialises as `V` and rejects the other value on input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LiteralBool<const V: bool>;

/// The JSON literal `true`.
pub type True = LiteralBool<true>;
/// The JSON literal `false`.
pub type False = LiteralBool<false>;

impl<const V: bool> Serialize for LiteralBool<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(V)
    }
}

impl<'de, const V: bool> Deserialize<'de> for LiteralBool<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? == V {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format_args!("expected `{V}`")))
        }
    }
}

/// Envelope of every HTTP response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum ApiResponse<T> {
    Ok {
        #[ts(type = "true")]
        ok: True,
        data: T,
    },
    Err {
        #[ts(type = "false")]
        ok: False,
        error: ApiError,
    },
}

impl<T> ApiResponse<T> {
    /// A successful response carrying `data`.
    pub fn ok(data: T) -> Self {
        Self::Ok {
            ok: LiteralBool,
            data,
        }
    }

    /// A failed response.
    pub fn err(error: ApiError) -> Self {
        Self::Err {
            ok: LiteralBool,
            error,
        }
    }
}

/// Error body. `message` is written for the user and never leaks paths or internals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    #[ts(type = "unknown | null")]
    pub details: Option<Value>,
}

/// Error category; the HTTP status is noted on each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    /// 400: malformed body, unknown segment, value out of range.
    InvalidRequest,
    /// 403: `Origin` header not in the allow-list.
    ForbiddenOrigin,
    /// 404: unknown device, session or profile.
    NotFound,
    /// 409: not valid in the current mode.
    Conflict,
    /// 503: dependency missing, e.g. data directory not writable.
    Unavailable,
    /// 500.
    Internal,
    /// 401: LAN mode and the bearer token is missing or wrong (17 §5). Additive.
    Unauthorized,
}

impl ApiErrorCode {
    /// The HTTP status code this error is returned with.
    pub fn http_status(self) -> u16 {
        match self {
            Self::InvalidRequest => 400,
            Self::ForbiddenOrigin => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::Unavailable => 503,
            Self::Internal => 500,
            Self::Unauthorized => 401,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Value of `GET /health`'s `status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    #[default]
    Ok,
}

/// `GET /health` response data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct HealthResponse {
    pub status: HealthStatus,
}

/// `DELETE /calibration/{profile_id}` response data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct DeletedResponse {
    #[ts(type = "true")]
    pub deleted: True,
}

/// Mode reported by `POST /playback/stop`: always `live`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LiveMode {
    #[default]
    Live,
}

/// `POST /playback/stop` response data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct PlaybackStopResponse {
    pub mode: LiveMode,
}

/// `PUT /devices/{id}/assignment` body. `null` unassigns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AssignmentRequest {
    pub segment: Option<SegmentId>,
}

/// `POST /calibration/reference-pose` body. `null` captures every assigned, tracking device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReferencePoseRequest {
    pub device_ids: Option<Vec<DeviceId>>,
}

/// `PUT /rig`: selects the room whose camera extrinsics are used; `null` for the default room.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct RigRequest {
    pub active_room: Option<String>,
}

/// `PUT /cameras/{id}/settings` body (martial-arts 14 §5.5). Absent fields are left unchanged.
/// `resolution_px` and `rate_hz` are accepted in the schema but refused by this engine: camera
/// calibration depends on them (09 §9), so they change in the configuration with a restart.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct CameraSettingsRequest {
    /// Manual exposure, seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exposure_s: Option<f64>,
    /// Manual gain, device units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gain: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub resolution_px: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rate_hz: Option<f64>,
}

/// `PUT /simulation` body. Absent fields are left unchanged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct SimulationUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub motion: Option<String>,
    /// Speed multiplier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub speed: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paused: Option<bool>,
}

/// Fault injected into a simulated device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FaultAction {
    Disconnect,
    Reconnect,
    Degrade,
    Restore,
}

/// `POST /simulation/devices/{id}/fault` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct FaultRequest {
    pub action: FaultAction,
    /// Device-reported quality 0..1 for `degrade`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub quality: Option<f32>,
}

/// `POST /recording/start` body.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct RecordingStartRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
}

/// `POST /playback/start` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PlaybackStartRequest {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub speed: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub looping: Option<bool>,
}

/// `PUT /playback` body. Absent fields are left unchanged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct PlaybackUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub speed: Option<f32>,
    /// Microseconds, `0 ..= session.duration_us`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub position_us: Option<u64>,
}
