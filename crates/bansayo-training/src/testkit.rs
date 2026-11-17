//! Synthetic bodies for unit tests: `bansayo.body.v1` proportions and forward kinematics.
#![allow(clippy::expect_used)]
use bansayo_contracts::*;
use nalgebra::{UnitQuaternion as Rot, Vector3 as V3};

const PELVIS_HEIGHT: f64 = 0.530;
type Row = (&'static str, Option<&'static str>, [f64; 3], [f64; 3]);
const ROWS: &[Row] = &[
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
const HEEL: [f64; 3] = [0.0, -0.039, -0.042];
const TOE: [f64; 3] = [0.0, -0.039, 0.110];

fn mirror_name(s: &str) -> String {
    s.replace("left_", "right_")
}

fn x(a: [f64; 3]) -> [f64; 3] {
    [-a[0], a[1], a[2]]
}

/// Segments in `bansayo.body.v1` order: centre, left arm, right arm, left leg, right leg.
pub fn skeleton(height: f64) -> SkeletonDefinition {
    let (centre, rest) = ROWS.split_at(5);
    let (arm, leg) = rest.split_at(4);
    type Owned = (String, Option<String>, [f64; 3], [f64; 3]);
    let mut rows: Vec<Owned> = vec![];
    let own = |r: &Row| (r.0.to_string(), r.1.map(String::from), r.2, r.3);
    let mir = |r: &Row| (mirror_name(r.0), r.1.map(mirror_name), x(r.2), x(r.3));
    rows.extend(centre.iter().map(own));
    rows.extend(arm.iter().map(own));
    rows.extend(arm.iter().map(mir));
    rows.extend(leg.iter().map(own));
    rows.extend(leg.iter().map(mir));
    let s = |a: [f64; 3]| a.map(|v| (v * height) as f32);
    SkeletonDefinition {
        model_id: "bansayo.body.v1".into(),
        rev: 1,
        height_m: height as f32,
        proportions: CalibrationState::Missing,
        segments: rows
            .into_iter()
            .map(|(id, parent, offset, tail)| SegmentDef {
                contact_points_m: if id.ends_with("_foot") {
                    vec![s(HEEL), s(TOE)]
                } else {
                    vec![]
                },
                id: SegmentId::new(id).expect("segment"),
                parent: parent.map(|p| SegmentId::new(p).expect("segment")),
                offset_m: s(offset),
                tail_m: s(tail),
            })
            .collect(),
        joints: vec![],
    }
}

pub fn evidence(t: u64) -> Evidence {
    Evidence {
        provenance: Provenance::Measured,
        method: MethodId::Triangulated,
        confidence: Some(0.9),
        sigma: None,
        sigma_basis: None,
        t_obs_us: Some(t),
        sources: vec![DeviceId::new("CAM").expect("id")],
    }
}

fn w(v: &V3<f64>) -> Vec3 {
    [v.x as f32, v.y as f32, v.z as f32]
}

fn wq(r: &Rot<f64>) -> Quat {
    let q = r.quaternion();
    [q.i as f32, q.j as f32, q.k as f32, q.w as f32]
}

fn v(a: [f32; 3]) -> V3<f64> {
    V3::new(a[0].into(), a[1].into(), a[2].into())
}

/// World pose of every segment from world rotations and the root position.
pub fn body(sk: &SkeletonDefinition, rotations: &[Rot<f64>], root: V3<f64>, t: u64) -> BodyState {
    let n = sk.segments.len();
    let mut pos = vec![V3::zeros(); n];
    for (i, s) in sk.segments.iter().enumerate() {
        pos[i] = match s.parent.as_ref() {
            None => root,
            Some(p) => {
                let p = sk.segments.iter().position(|x| &x.id == p).expect("parent");
                pos[p] + rotations[p] * v(s.offset_m)
            }
        };
    }
    let mut keypoints: Vec<Option<KeypointState>> = vec![None; 26];
    // Evidence as the camera-only pipeline publishes it: FK keypoints, a triangulated root,
    // skeleton-fit axes and rest-filled twist.
    let fk = Evidence {
        provenance: Provenance::Derived,
        method: MethodId::Fk,
        ..evidence(t)
    };
    let kp = |p: V3<f64>| {
        Some(KeypointState {
            position_m: w(&p),
            evidence: fk.clone(),
        })
    };
    for i in 0..19 {
        keypoints[i] = kp(pos[i]);
    }
    if let Some(k) = keypoints[0].as_mut() {
        k.evidence = evidence(t);
    }
    for (target, seg, contact) in [
        (19, 4, None),
        (20, 8, None),
        (21, 12, None),
        (22, 15, Some(1)),
        (23, 18, Some(1)),
        (24, 15, Some(0)),
        (25, 18, Some(0)),
    ] {
        let d = &sk.segments[seg];
        let off = contact.map_or(d.tail_m, |c: usize| d.contact_points_m[c]);
        keypoints[target] = kp(pos[seg] + rotations[seg] * v(off));
    }
    BodyState {
        skeleton_rev: sk.rev,
        segments: rotations
            .iter()
            .map(|r| SegmentState {
                rotation: wq(r),
                axis: Evidence {
                    method: MethodId::SkeletonFitAxis,
                    ..evidence(t)
                },
                twist: Evidence {
                    provenance: Provenance::Inferred,
                    method: MethodId::RestFill,
                    confidence: None,
                    ..evidence(t)
                },
                angular_velocity_rad_s: None,
                angular_velocity: crate::evidence::unavailable(),
            })
            .collect(),
        root: keypoints[0].clone().expect("root"),
        keypoints,
    }
}

pub fn sample(body: BodyState, t: u64) -> MotionSample {
    MotionSample {
        seq: t,
        t_us: t,
        origins: vec![Origin::Simulated],
        body: Some(body),
        planar: vec![],
    }
}

pub fn index(sk: &SkeletonDefinition, id: &str) -> usize {
    sk.segments
        .iter()
        .position(|s| s.id.as_str() == id)
        .expect("segment")
}

/// Rest pose, every segment identity.
pub fn rest(sk: &SkeletonDefinition) -> Vec<Rot<f64>> {
    vec![Rot::identity(); sk.segments.len()]
}

/// World rotations of the simulator's scripted roundhouse kick (right leg), a port of
/// `bansayo-engine::sources::simulated::{motions, body}` for pure tests.
pub fn roundhouse(sk: &SkeletonDefinition, t_s: f64) -> Vec<Rot<f64>> {
    use std::f64::consts::PI;
    let t = t_s.rem_euclid(2.8);
    let smooth = |from: f64, to: f64, a: f64, b: f64| {
        let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
        from + (to - from) * u * u * (3.0 - 2.0 * u)
    };
    let (hip, knee) = if t < 0.6 {
        (smooth(0.0, 45.0, 0.0, 0.6), smooth(0.0, 60.0, 0.0, 0.6))
    } else if t < 0.9 {
        (smooth(45.0, 60.0, 0.6, 0.9), 60.0)
    } else if t < 1.2 {
        (smooth(60.0, 80.0, 0.9, 1.2), smooth(60.0, 0.0, 0.9, 1.2))
    } else if t < 1.6 {
        (smooth(80.0, 45.0, 1.2, 1.6), smooth(0.0, 60.0, 1.2, 1.6))
    } else {
        (smooth(45.0, 0.0, 1.6, 2.4), smooth(60.0, 0.0, 1.6, 2.4))
    };
    let pivot = if t < 1.6 {
        smooth(0.0, 1.0, 0.6, 0.9)
    } else {
        smooth(1.0, 0.0, 1.6, 2.4)
    };
    let ry = |a: f64| Rot::from_axis_angle(&V3::y_axis(), a);
    let rx = |a: f64| Rot::from_axis_angle(&V3::x_axis(), a);
    let heading = ry(45f64.to_radians() * pivot);
    let plane = ry(-PI / 2.0 * pivot);
    let mut r: Vec<Option<Rot<f64>>> = vec![None; sk.segments.len()];
    for name in ["pelvis", "spine", "chest", "left_thigh", "left_shin"] {
        r[index(sk, name)] = Some(heading);
    }
    let (h, k) = (hip.to_radians(), knee.to_radians());
    r[index(sk, "right_thigh")] = Some(heading * plane * rx(-h) * plane.inverse());
    r[index(sk, "right_shin")] = Some(heading * plane * rx(k - h) * plane.inverse());
    // Arms hang: the chest-relative T-pose rotated down.
    let rz = |a: f64| Rot::from_axis_angle(&V3::z_axis(), a);
    r[index(sk, "left_upper_arm")] = Some(heading * rz(-PI / 2.0));
    r[index(sk, "right_upper_arm")] = Some(heading * rz(PI / 2.0));
    let mut out: Vec<Rot<f64>> = Vec::with_capacity(sk.segments.len());
    for (i, s) in sk.segments.iter().enumerate() {
        let parent = s
            .parent
            .as_ref()
            .and_then(|p| sk.segments.iter().position(|x| &x.id == p));
        out.push(r[i].unwrap_or_else(|| parent.map_or(Rot::identity(), |p| out[p])));
    }
    out
}

/// Root position that puts the lowest foot contact on the floor (the simulator's rule).
pub fn floor_root(sk: &SkeletonDefinition, rotations: &[Rot<f64>]) -> V3<f64> {
    let b = body(sk, rotations, V3::zeros(), 0);
    let lowest = [22, 23, 24, 25]
        .iter()
        .filter_map(|i| b.keypoints[*i].as_ref())
        .map(|k| f64::from(k.position_m[1]))
        .fold(f64::INFINITY, f64::min);
    V3::new(0.0, -lowest, 0.0)
}

pub fn roundhouse_sample(sk: &SkeletonDefinition, t_s: f64) -> MotionSample {
    let r = roundhouse(sk, t_s);
    let root = floor_root(sk, &r);
    let t = (t_s * 1e6).round() as u64;
    sample(body(sk, &r, root, t), t)
}

/// The shipped content, parsed.
pub fn library() -> crate::content::validate::Library {
    use crate::content::types::*;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/");
    let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).expect("content file");
    let mut lib = crate::content::validate::Library::default();
    let d: DisciplineDef =
        serde_saphyr::from_str(&read("disciplines/taekwondo.yaml")).expect("yaml");
    lib.disciplines.insert(d.id.clone(), d);
    for f in [
        "techniques/taekwondo/dollyeo_chagi.yaml",
        "techniques/taekwondo/ap_chagi.yaml",
    ] {
        let t: TechniqueDef = serde_saphyr::from_str(&read(f)).expect("yaml");
        lib.techniques.insert(t.id.clone(), t);
    }
    let m: MetricPack = serde_saphyr::from_str(&read("metrics/kick.yaml")).expect("yaml");
    for m in m.metrics {
        lib.metrics.insert(m.id.clone(), m);
    }
    let s: ScoringModel = serde_saphyr::from_str(&read("scoring/kick_basic.yaml")).expect("yaml");
    lib.scoring.insert(s.id.clone(), s);
    lib
}
