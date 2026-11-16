//! The `kinexa.body.v1` reference skeleton at a given height, and its rest T-pose.
//!
//! The proportion table below is the single source of the numbers in the golden skeleton.
//! Fractions of body height H, from docs/03-domain-model.md §3 (Drillis and Contini via Winter).

use kinexa_contracts::{
    AngleUnit, CalibrationState, JointDef, JointMethod, Provenance, SegmentDef, SegmentPose,
    SkeletonDefinition, Vec3,
};

use super::{dev, seg};

pub const MODEL_ID: &str = "kinexa.body.v1";
pub const HEIGHT_M: f64 = 1.75;
pub const SKELETON_REV: u32 = 1;

/// Root (pelvis) height at rest, fraction of H.
const PELVIS_HEIGHT: f64 = 0.530;
/// Foot contact points in the foot frame, fraction of H.
const HEEL: [f64; 3] = [0.0, -0.039, -0.042];
const TOE: [f64; 3] = [0.0, -0.039, 0.110];

/// `(segment, parent, offset / H, tail / H)` for the centre line and the left side.
/// Right-side segments mirror X.
type Row = (&'static str, Option<&'static str>, [f64; 3], [f64; 3]);
const CENTRE: &[Row] = &[
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
const LEFT_ARM: &[Row] = &[
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
const LEFT_LEG: &[Row] = &[
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

/// `(id, label, proximal segment, distal segment, method)`.
const JOINTS: &[(&str, &str, &str, &str, JointMethod)] = &[
    (
        "left_knee_flexion",
        "Left knee flexion",
        "left_thigh",
        "left_shin",
        JointMethod::AxisAngle,
    ),
    (
        "right_knee_flexion",
        "Right knee flexion",
        "right_thigh",
        "right_shin",
        JointMethod::AxisAngle,
    ),
    (
        "left_elbow_flexion",
        "Left elbow flexion",
        "left_upper_arm",
        "left_forearm",
        JointMethod::AxisAngle,
    ),
    (
        "right_elbow_flexion",
        "Right elbow flexion",
        "right_upper_arm",
        "right_forearm",
        JointMethod::AxisAngle,
    ),
    (
        "left_hip_flexion",
        "Left hip flexion",
        "pelvis",
        "left_thigh",
        JointMethod::PlanarAngle,
    ),
    (
        "right_hip_flexion",
        "Right hip flexion",
        "pelvis",
        "right_thigh",
        JointMethod::PlanarAngle,
    ),
    (
        "left_shoulder_elevation",
        "Left shoulder elevation",
        "chest",
        "left_upper_arm",
        JointMethod::AxisAngle,
    ),
    (
        "right_shoulder_elevation",
        "Right shoulder elevation",
        "chest",
        "right_upper_arm",
        JointMethod::AxisAngle,
    ),
    (
        "trunk_forward_lean",
        "Trunk forward lean",
        "pelvis",
        "chest",
        JointMethod::PlanarAngle,
    ),
];

/// One segment row resolved to metres, in definition order.
pub struct Segment {
    pub id: String,
    pub parent: Option<String>,
    pub offset: [f64; 3],
    pub tail: [f64; 3],
}

fn mirror_name(name: &str) -> String {
    name.replacen("left_", "right_", 1)
}

fn mirror_x(v: [f64; 3]) -> [f64; 3] {
    [-v[0], v[1], v[2]]
}

fn scale(v: [f64; 3]) -> [f64; 3] {
    v.map(|c| c * HEIGHT_M)
}

/// Rounds to the micrometre and converts, so the JSON shows short decimals and never `-0.0`.
pub fn m(v: [f64; 3]) -> Vec3 {
    v.map(|c| ((c * 1e6).round() / 1e6 + 0.0) as f32)
}

/// All 19 segments, parents before children, in metres.
pub fn segments() -> Vec<Segment> {
    let row = |r: &Row, mirror: bool| {
        let (id, parent, offset, tail) = *r;
        let (offset, tail) = if mirror {
            (mirror_x(offset), mirror_x(tail))
        } else {
            (offset, tail)
        };
        let name = |s: &str| if mirror { mirror_name(s) } else { s.to_owned() };
        Segment {
            id: name(id),
            parent: parent.map(name),
            offset: scale(offset),
            tail: scale(tail),
        }
    };
    CENTRE
        .iter()
        .map(|r| row(r, false))
        .chain(LEFT_ARM.iter().map(|r| row(r, false)))
        .chain(LEFT_ARM.iter().map(|r| row(r, true)))
        .chain(LEFT_LEG.iter().map(|r| row(r, false)))
        .chain(LEFT_LEG.iter().map(|r| row(r, true)))
        .collect()
}

/// World position of every segment origin in the rest pose: parent offsets summed from the root.
pub fn rest_positions() -> Vec<[f64; 3]> {
    let segs = segments();
    let mut out: Vec<[f64; 3]> = Vec::with_capacity(segs.len());
    for s in &segs {
        let base = match &s.parent {
            None => [0.0; 3],
            Some(p) => {
                out[segs
                    .iter()
                    .position(|q| &q.id == p)
                    .expect("parent precedes child")]
            }
        };
        out.push([
            base[0] + s.offset[0],
            base[1] + s.offset[1],
            base[2] + s.offset[2],
        ]);
    }
    out
}

pub fn definition() -> SkeletonDefinition {
    let segments = segments()
        .into_iter()
        .map(|s| SegmentDef {
            contact_points_m: if s.id.ends_with("_foot") {
                vec![m(scale(HEEL)), m(scale(TOE))]
            } else {
                vec![]
            },
            id: seg(&s.id),
            parent: s.parent.as_deref().map(seg),
            offset_m: m(s.offset),
            tail_m: m(s.tail),
        })
        .collect();
    let joints = JOINTS
        .iter()
        .map(|&(id, label, a, b, method)| JointDef {
            id: id.into(),
            label: label.into(),
            segments: [seg(a), seg(b)],
            method,
            unit: AngleUnit::Rad,
            validated: false,
        })
        .collect();
    SkeletonDefinition {
        model_id: MODEL_ID.into(),
        rev: SKELETON_REV,
        height_m: HEIGHT_M as f32,
        proportions: CalibrationState::Assumed,
        segments,
        joints,
    }
}

/// Segment assignments of the simulated trackers used across the examples.
pub const ASSIGNED: &[(&str, &str)] = &[
    ("SIM-T01", "pelvis"),
    ("SIM-T02", "chest"),
    ("SIM-T03", "left_thigh"),
    ("SIM-T04", "left_shin"),
];

/// Rest T-pose: identity rotations, forward-kinematics positions. Assigned segments are
/// `derived` from their tracker; the rest are `inferred`.
pub fn rest_pose_segments() -> Vec<SegmentPose> {
    segments()
        .iter()
        .zip(rest_positions())
        .map(|(s, p)| {
            let tracker = ASSIGNED.iter().find(|(_, segment)| *segment == s.id);
            SegmentPose {
                rotation: [0.0, 0.0, 0.0, 1.0],
                position_m: m(p),
                provenance: if tracker.is_some() {
                    Provenance::Derived
                } else {
                    Provenance::Inferred
                },
                confidence: tracker.map(|_| 0.97),
                sources: tracker.map(|(d, _)| vec![dev(d)]).unwrap_or_default(),
            }
        })
        .collect()
}

#[test]
fn reference_skeleton_is_complete_and_consistent() {
    let def = definition();
    assert_eq!(def.segments.len(), 19);
    assert_eq!(def.joints.len(), 9);
    for (i, s) in def.segments.iter().enumerate() {
        if let Some(p) = &s.parent {
            assert!(
                def.segments[..i].iter().any(|q| &q.id == p),
                "{} before its parent",
                s.id
            );
        }
    }
    // Floor contact: the lowest foot contact point of the rest pose lies on y = 0.
    let pos = rest_positions();
    let foot = segments()
        .iter()
        .position(|s| s.id == "left_foot")
        .expect("left_foot");
    let heel_y = pos[foot][1] + HEEL[1] * HEIGHT_M;
    assert!(heel_y.abs() < 1e-9, "heel at {heel_y}");
}
