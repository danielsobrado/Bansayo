//! Solved skeleton pose and joint measurements.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DeviceId, Provenance, Quat, Reason, Vec3};

/// Pose of one segment, world frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SegmentPose {
    /// `q_world_from_segment`.
    pub rotation: Quat,
    /// Segment origin in world, metres, from forward kinematics.
    pub position_m: Vec3,
    /// Provenance of the rotation.
    pub provenance: Provenance,
    pub confidence: Option<f32>,
    pub sources: Vec<DeviceId>,
}

/// Pose of the whole skeleton at one tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SkeletonPose {
    /// `SkeletonDefinition.rev` this pose refers to.
    pub skeleton_rev: u32,
    /// Root segment origin in world, metres.
    pub root_position_m: Vec3,
    pub root_position_provenance: Provenance,
    /// Index-aligned with `SkeletonDefinition.segments`.
    pub segments: Vec<SegmentPose>,
}

/// One joint measurement, index-aligned with `SkeletonDefinition.joints`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct JointMeasurement {
    /// In `JointDef.unit`; `null` when unavailable.
    pub value: Option<f32>,
    pub provenance: Provenance,
    pub confidence: Option<f32>,
    pub reasons: Vec<Reason>,
}
