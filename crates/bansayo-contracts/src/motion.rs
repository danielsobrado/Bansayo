//! Canonical movement input to pure training code (martial-arts 04 §2–4).
use crate::{DeviceId, Origin, Provenance, Quat, TimeUs, Vec3};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const BODY_KEYPOINT_SCHEMA_ID: &str = "bansayo.body.v1.points";
pub const BODY_KEYPOINT_NAMES: [&str; 26] = [
    "pelvis",
    "spine",
    "chest",
    "neck",
    "head",
    "left_clavicle",
    "left_shoulder",
    "left_elbow",
    "left_wrist",
    "right_clavicle",
    "right_shoulder",
    "right_elbow",
    "right_wrist",
    "left_hip",
    "left_knee",
    "left_ankle",
    "right_hip",
    "right_knee",
    "right_ankle",
    "head_top",
    "left_hand_tip",
    "right_hand_tip",
    "left_toe",
    "right_toe",
    "left_heel",
    "right_heel",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MethodId {
    ImuMounted,
    ImuGyro,
    ImuOpticalFused,
    MarkerCluster,
    Triangulated,
    SkeletonFitAxis,
    SkeletonFitPlane,
    Fk,
    XrDevice,
    DepthCamera,
    ImagePlane,
    MonocularLift,
    RestFill,
    FloorContact,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SigmaBasis {
    Propagated,
    Validated,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Evidence {
    pub provenance: Provenance,
    pub method: MethodId,
    pub confidence: Option<f32>,
    pub sigma: Option<f32>,
    pub sigma_basis: Option<SigmaBasis>,
    pub t_obs_us: Option<TimeUs>,
    pub sources: Vec<DeviceId>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SegmentState {
    pub rotation: Quat,
    pub axis: Evidence,
    pub twist: Evidence,
    pub angular_velocity_rad_s: Option<Vec3>,
    pub angular_velocity: Evidence,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct KeypointState {
    pub position_m: Vec3,
    pub evidence: Evidence,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct BodyState {
    pub skeleton_rev: u32,
    pub segments: Vec<SegmentState>,
    pub keypoints: Vec<Option<KeypointState>>,
    pub root: KeypointState,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PlanarKeypoint {
    pub x: f32,
    pub y: f32,
    pub confidence: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanarView {
    Frontal,
    Lateral,
    Oblique,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PlanarBody {
    pub camera_id: DeviceId,
    pub t_obs_us: TimeUs,
    pub scale_px: Option<f32>,
    pub keypoints: Vec<Option<PlanarKeypoint>>,
    pub view: PlanarView,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MotionSample {
    pub seq: u64,
    pub t_us: TimeUs,
    pub origins: Vec<Origin>,
    pub body: Option<BodyState>,
    pub planar: Vec<PlanarBody>,
}
