//! OpenXR tracked-device uplink (`GET /api/v1/ingest/xr`): the VR client sends raw head and grip
//! poses in XR stage space; the engine assigns identity, time and origin. Additive to protocol v1.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Quat, TimeQuality, Vec3};

/// Version of the XR uplink messages, independent of the downlink `PROTOCOL_VERSION`.
pub const XR_PROTOCOL_VERSION: u32 = 1;

/// Which tracked device a sample comes from. Order on the wire is fixed: head, left, right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum XrRole {
    Head,
    /// OpenXR `grip_pose` of the left controller: a device point, not the anatomical wrist.
    LeftGrip,
    RightGrip,
}

impl XrRole {
    /// The required order of `XrFrame::samples`.
    pub const ORDER: [XrRole; 3] = [XrRole::Head, XrRole::LeftGrip, XrRole::RightGrip];
}

/// One device in one frame. `tracked` with both pose fields, or untracked with neither.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrDeviceSample {
    pub role: XrRole,
    pub tracked: bool,
    /// Metres, XR stage space.
    pub position_m: Option<Vec3>,
    /// `q_xr_stage_from_device`, `[x, y, z, w]`.
    pub rotation_xyzw: Option<Quat>,
}

/// All three devices sampled in one client tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrFrame {
    /// Starts at 0 on every connection and strictly increases.
    pub seq: u64,
    /// The client's monotonic clock, microseconds. Not the session clock.
    pub client_time_us: u64,
    pub samples: [XrDeviceSample; 3],
}

/// First client message on the socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrHello {
    pub protocol_version: u32,
}

/// Client → engine. Unknown fields are refused: each variant is a strict struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XrClientMessage {
    Hello(XrHello),
    Frame(XrFrame),
}

/// Engine → client. Additive fields may appear; clients ignore unknown ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XrServerMessage {
    /// Hello accepted; frames may follow. `time_quality` is how their `t_us` will be assigned.
    Ready {
        protocol_version: u32,
        time_quality: TimeQuality,
    },
    /// The frame or connection was refused; `code` is stable, `message` is for people.
    Error { code: String, message: String },
}
