//! WebSocket stream messages (`GET /api/v1/stream`), server to client.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AngleUnit, JointMeasurement, Origin, PoseLandmarks2D, PoseLandmarks3D, SkeletonDefinition,
    SkeletonPose, SystemState, TimeUs, TrackingQuality,
};

/// Wire protocol version announced in `hello` and in the URL prefix.
pub const PROTOCOL_VERSION: u32 = 1;

macro_rules! literal {
    ($(#[$meta:meta])* $name:ident, $variant:ident = $value:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
        pub enum $name {
            #[default]
            #[serde(rename = $value)]
            $variant,
        }
    };
}

literal!(/// Always `right`.
    Handedness, Right = "right");
literal!(/// Always `+y`.
    UpAxis, PosY = "+y");
literal!(/// Always `+z`.
    ForwardAxis, PosZ = "+z");
literal!(/// Always `m`.
    LengthUnit, M = "m");
literal!(/// Always `xyzw`.
    QuaternionOrder, Xyzw = "xyzw");
literal!(/// Always `us`.
    TimeUnit, Us = "us");

/// Conventions every value on the wire follows. Fixed for protocol version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct Conventions {
    pub handedness: Handedness,
    pub up: UpAxis,
    pub forward: ForwardAxis,
    pub length_unit: LengthUnit,
    pub angle_unit: AngleUnit,
    pub quaternion_order: QuaternionOrder,
    pub time_unit: TimeUnit,
}

/// Severity of a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

/// One WebSocket text frame. `hello` is always first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// First message on every connection.
    Hello {
        protocol_version: u32,
        engine_version: String,
        /// RFC 3339 UTC instant of session time zero.
        session_epoch_utc: String,
        conventions: Conventions,
        skeleton: SkeletonDefinition,
        state: SystemState,
        /// Engine feature flags, e.g. `training`, `content`, `athletes`, `progress`. Additive.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        capabilities: Option<Vec<String>>,
        /// Observable catalogue revision. Additive.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        observables_rev: Option<u32>,
    },
    /// Sent when the skeleton `rev` changes, before the first frame that refers to it.
    Skeleton { skeleton: SkeletonDefinition },
    /// Sent on change and at 1 Hz.
    State { state: SystemState },
    /// One solved tick. `seq` increases strictly; `t_us` is the tick time.
    Frame {
        seq: u64,
        t_us: TimeUs,
        /// Origins of every contributing source.
        origins: Vec<Origin>,
        pose: Option<SkeletonPose>,
        /// Index-aligned with `SkeletonDefinition.joints`.
        joints: Vec<JointMeasurement>,
        quality: TrackingQuality,
    },
    #[serde(rename = "landmarks_2d")]
    Landmarks2d { landmarks: PoseLandmarks2D },
    /// Engine-triangulated landmarks (`kind: "triangulated"`), sent when a new camera frame set is solved.
    #[serde(rename = "landmarks_3d")]
    Landmarks3d { landmarks: PoseLandmarks3D },
    Notice {
        level: NoticeLevel,
        code: String,
        message: String,
    },
    /// Topic `training`: authoritative session state; `null` when no session exists.
    TrainingState { state: Option<crate::TrainingState> },
    /// Topic `training`: one increment; `event_seq` increases by exactly 1 per session.
    TrainingEvent {
        session_id: crate::SessionId,
        event: crate::TrainingEvent,
    },
    /// Topic `observables`: latest values, at most `max_rate_hz`.
    Observables {
        seq: u64,
        t_us: TimeUs,
        values: Vec<crate::ObservableValue>,
    },
    /// Topic `diagnostics`: every 10 s while `diagnostics.timing` is on, per-stage latency
    /// percentiles over the window and drop counters (martial-arts 15 §4.1).
    Diagnostics {
        window_s: u32,
        stages: Vec<StageTiming>,
        dropped: Vec<DropCount>,
    },
}

/// Latency of one pipeline stage over a diagnostics window, microseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct StageTiming {
    pub stage: String,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
    /// Samples in the window.
    pub count: u64,
    /// The stage's budget (15 §2.1).
    pub budget_us: u64,
}

/// Items a bounded channel or one-slot buffer dropped since the engine started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DropCount {
    pub channel: String,
    pub count: u64,
}
