//! Default proportions of `bansayo.body.v1`, as fractions of body height H.
//!
//! Anthropometric ratios of Drillis and Contini as tabulated by Winter (Biomechanics and Motor
//! Control of Human Movement); the subdivision of the spine is a modelling choice. Right-side
//! segments mirror X. This is the only place these numbers live in the engine.

use bansayo_contracts::JointMethod;

pub const MODEL_ID: &str = "bansayo.body.v1";
/// Landmark schema of simulated cameras: one landmark per segment origin, in skeleton order.
pub const LANDMARK_SCHEMA_ID: &str = "bansayo.body.v1.joints";

/// Root (pelvis) height at rest.
pub const PELVIS_HEIGHT: f64 = 0.530;
/// Foot contact points in the foot frame.
pub const HEEL: [f64; 3] = [0.0, -0.039, -0.042];
pub const TOE: [f64; 3] = [0.0, -0.039, 0.110];

/// `(segment, parent, offset / H, tail / H)`.
pub type Row = (&'static str, Option<&'static str>, [f64; 3], [f64; 3]);

pub const CENTRE: &[Row] = &[
    ("pelvis", None, [0.0, PELVIS_HEIGHT, 0.0], [0.0, 0.060, 0.0]),
    (
        "spine",
        Some("pelvis"),
        [0.0, 0.060, 0.0],
        [0.0, 0.110, 0.0],
    ),
    ("chest", Some("spine"), [0.0, 0.110, 0.0], [0.0, 0.118, 0.0]),
    ("neck", Some("chest"), [0.0, 0.118, 0.0], [0.0, 0.052, 0.0]),
    ("head", Some("neck"), [0.0, 0.052, 0.0], [0.0, 0.130, 0.0]),
];

/// Left arm; the right arm mirrors X.
pub const LEFT_ARM: &[Row] = &[
    (
        "left_clavicle",
        Some("chest"),
        [0.020, 0.100, 0.0],
        [0.1095, 0.018, 0.0],
    ),
    (
        "left_upper_arm",
        Some("left_clavicle"),
        [0.1095, 0.018, 0.0],
        [0.186, 0.0, 0.0],
    ),
    (
        "left_forearm",
        Some("left_upper_arm"),
        [0.186, 0.0, 0.0],
        [0.146, 0.0, 0.0],
    ),
    (
        "left_hand",
        Some("left_forearm"),
        [0.146, 0.0, 0.0],
        [0.108, 0.0, 0.0],
    ),
];

/// Left leg; the right leg mirrors X.
pub const LEFT_LEG: &[Row] = &[
    (
        "left_thigh",
        Some("pelvis"),
        [0.0955, 0.0, 0.0],
        [0.0, -0.245, 0.0],
    ),
    (
        "left_shin",
        Some("left_thigh"),
        [0.0, -0.245, 0.0],
        [0.0, -0.246, 0.0],
    ),
    (
        "left_foot",
        Some("left_shin"),
        [0.0, -0.246, 0.0],
        [0.0, -0.039, 0.110],
    ),
];

/// How the engine evaluates a joint measurement. All axes are in segment frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointKind {
    /// Angle between the long (tail) axes of the two segments.
    LongAxes,
    /// Angle between the second segment's long axis and the first segment's downward axis.
    FromDownAxis,
    /// Signed angle of the second segment's long axis from the first segment's downward axis,
    /// projected on the first segment's sagittal (Y-Z) plane, positive forward.
    SagittalFromDown,
    /// Signed angle of the second segment's up axis from world up, in the first segment's
    /// sagittal plane, positive forward.
    TrunkLean,
}

/// `(id, label, first segment, second segment, contract method, evaluation)`.
pub type JointRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    JointMethod,
    JointKind,
);

pub const JOINTS: &[JointRow] = &[
    (
        "left_knee_flexion",
        "Left knee flexion",
        "left_thigh",
        "left_shin",
        JointMethod::AxisAngle,
        JointKind::LongAxes,
    ),
    (
        "right_knee_flexion",
        "Right knee flexion",
        "right_thigh",
        "right_shin",
        JointMethod::AxisAngle,
        JointKind::LongAxes,
    ),
    (
        "left_elbow_flexion",
        "Left elbow flexion",
        "left_upper_arm",
        "left_forearm",
        JointMethod::AxisAngle,
        JointKind::LongAxes,
    ),
    (
        "right_elbow_flexion",
        "Right elbow flexion",
        "right_upper_arm",
        "right_forearm",
        JointMethod::AxisAngle,
        JointKind::LongAxes,
    ),
    (
        "left_hip_flexion",
        "Left hip flexion",
        "pelvis",
        "left_thigh",
        JointMethod::PlanarAngle,
        JointKind::SagittalFromDown,
    ),
    (
        "right_hip_flexion",
        "Right hip flexion",
        "pelvis",
        "right_thigh",
        JointMethod::PlanarAngle,
        JointKind::SagittalFromDown,
    ),
    (
        "left_shoulder_elevation",
        "Left shoulder elevation",
        "chest",
        "left_upper_arm",
        JointMethod::AxisAngle,
        JointKind::FromDownAxis,
    ),
    (
        "right_shoulder_elevation",
        "Right shoulder elevation",
        "chest",
        "right_upper_arm",
        JointMethod::AxisAngle,
        JointKind::FromDownAxis,
    ),
    (
        "trunk_forward_lean",
        "Trunk forward lean",
        "pelvis",
        "chest",
        JointMethod::PlanarAngle,
        JointKind::TrunkLean,
    ),
];
