//! Skeleton definition: segments and joint measurement definitions, as data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{CalibrationState, SegmentId, Vec3};

/// One body segment. Its origin is its proximal joint centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SegmentDef {
    pub id: SegmentId,
    /// `null` for the root.
    pub parent: Option<SegmentId>,
    /// Origin of this segment in the parent's frame at rest, metres.
    pub offset_m: Vec3,
    /// From the origin to the distal end, in this segment's frame, metres.
    pub tail_m: Vec3,
    /// Points that can touch the floor, in this segment's frame, metres.
    pub contact_points_m: Vec<Vec3>,
}

/// How a joint measurement is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum JointMethod {
    AxisAngle,
    PlanarAngle,
}

/// Angle unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AngleUnit {
    #[default]
    Rad,
}

/// A joint measurement computed by the engine from two segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct JointDef {
    /// e.g. `left_knee_flexion`.
    pub id: String,
    pub label: String,
    pub segments: [SegmentId; 2],
    pub method: JointMethod,
    pub unit: AngleUnit,
    /// Whether the definition has been validated against a reference.
    pub validated: bool,
}

/// The active skeleton. In the rest pose (T-pose facing +Z) every segment frame is aligned with
/// world axes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SkeletonDefinition {
    /// e.g. `kinexa.body.v1`.
    pub model_id: String,
    /// Increments whenever the definition or its scaling changes.
    pub rev: u32,
    /// Body height the proportions are scaled by, metres.
    pub height_m: f32,
    /// `assumed` until body-proportion calibration exists.
    pub proportions: CalibrationState,
    /// Parents precede children.
    pub segments: Vec<SegmentDef>,
    pub joints: Vec<JointDef>,
}
