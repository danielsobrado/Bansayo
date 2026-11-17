//! Camera-only skeleton: segment rotations fitted to triangulated world landmarks. Pure.
//!
//! Pelvis and chest get full frames from their left-right line and an up direction. Every other
//! fitted segment takes the smallest rotation ("swing") that points its rest long axis along the
//! observed bone, starting from its parent's rotation: a bone direction says nothing about axial
//! twist, so twist is inherited from the parent, never invented. Joint angles computed from long
//! axes and the pelvis frame are therefore exact functions of the landmarks.
use crate::{
    math::{Rot, V3, v3_from_wire},
    skeleton::{Skeleton, proportions::LANDMARK_SCHEMA_ID},
};
use bansayo_contracts::*;
use nalgebra::{Matrix3, Rotation3};

/// COCO-17 body keypoints, the output order of RTMPose body models.
pub const COCO17_SCHEMA_ID: &str = "coco17.body";
pub const COCO17_NAMES: [&str; 17] = [
    "nose",
    "left_eye",
    "right_eye",
    "left_ear",
    "right_ear",
    "left_shoulder",
    "right_shoulder",
    "left_elbow",
    "right_elbow",
    "left_wrist",
    "right_wrist",
    "left_hip",
    "right_hip",
    "left_knee",
    "right_knee",
    "left_ankle",
    "right_ankle",
];
pub const COCO17_EDGES: [[u32; 2]; 16] = [
    [5, 6],
    [5, 7],
    [7, 9],
    [6, 8],
    [8, 10],
    [5, 11],
    [6, 12],
    [11, 12],
    [11, 13],
    [13, 15],
    [12, 14],
    [14, 16],
    [0, 1],
    [0, 2],
    [1, 3],
    [2, 4],
];
pub fn coco17_schema() -> LandmarkSchema {
    LandmarkSchema {
        id: COCO17_SCHEMA_ID.into(),
        names: COCO17_NAMES.map(String::from).to_vec(),
        edges: COCO17_EDGES.to_vec(),
    }
}

/// Points below this confidence are treated as missing.
pub const MIN_POINT_CONFIDENCE: f32 = 0.3;

/// Anatomical keypoints the fit uses; each schema maps onto them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum K {
    Pelvis,
    Spine,
    ChestOrigin,
    Neck,
    Head,
    ClavL,
    ClavR,
    HipL,
    HipR,
    KneeL,
    KneeR,
    AnkleL,
    AnkleR,
    ShoulderL,
    ShoulderR,
    ElbowL,
    ElbowR,
    WristL,
    WristR,
}

struct Point {
    m: V3,
    confidence: f32,
    views: Vec<DeviceId>,
}

/// Keypoint lookup for one triangulated set.
struct Keypoints<'a> {
    schema: &'a str,
    points: &'a [Option<TriangulatedLandmark3D>],
}
impl Keypoints<'_> {
    fn raw(&self, i: usize) -> Option<Point> {
        let p = self.points.get(i)?.as_ref()?;
        (p.confidence >= MIN_POINT_CONFIDENCE).then(|| Point {
            m: v3_from_wire(p.m),
            confidence: p.confidence,
            views: p.used_views.clone(),
        })
    }
    fn mid(&self, a: usize, b: usize) -> Option<Point> {
        let (a, b) = (self.raw(a)?, self.raw(b)?);
        let mut views = a.views;
        views.extend(b.views);
        Some(Point {
            m: (a.m + b.m) / 2.0,
            confidence: a.confidence.min(b.confidence),
            views,
        })
    }
    fn get(&self, k: K, sk: &Skeleton) -> Option<Point> {
        if self.schema == LANDMARK_SCHEMA_ID {
            // One landmark per segment origin, in skeleton order.
            let name = match k {
                K::Pelvis => "pelvis",
                K::Spine => "spine",
                K::ChestOrigin => "chest",
                K::Neck => "neck",
                K::Head => "head",
                K::ClavL => "left_clavicle",
                K::ClavR => "right_clavicle",
                K::HipL => "left_thigh",
                K::HipR => "right_thigh",
                K::KneeL => "left_shin",
                K::KneeR => "right_shin",
                K::AnkleL => "left_foot",
                K::AnkleR => "right_foot",
                K::ShoulderL => "left_upper_arm",
                K::ShoulderR => "right_upper_arm",
                K::ElbowL => "left_forearm",
                K::ElbowR => "right_forearm",
                K::WristL => "left_hand",
                K::WristR => "right_hand",
            };
            return self.raw(sk.segments.iter().position(|s| s.id.as_str() == name)?);
        }
        if self.schema == COCO17_SCHEMA_ID {
            return match k {
                K::Pelvis => self.mid(11, 12),
                K::Neck => self.mid(5, 6),
                K::ClavL | K::ShoulderL => self.raw(5),
                K::ClavR | K::ShoulderR => self.raw(6),
                K::ElbowL => self.raw(7),
                K::ElbowR => self.raw(8),
                K::WristL => self.raw(9),
                K::WristR => self.raw(10),
                K::HipL => self.raw(11),
                K::HipR => self.raw(12),
                K::KneeL => self.raw(13),
                K::KneeR => self.raw(14),
                K::AnkleL => self.raw(15),
                K::AnkleR => self.raw(16),
                // The nose and ears do not locate the head segment's origin or axis reliably.
                K::Spine | K::ChestOrigin | K::Head => None,
            };
        }
        None
    }
}

/// How a segment's rotation is fitted.
enum Rule {
    /// Full frame: `x` from `left - right`, `y` from `up_to - up_from`, orthogonalised.
    Frame {
        left: K,
        right: K,
        up_from: &'static [K],
        up_to: &'static [K],
    },
    /// Swing of the rest long axis onto `to - from`.
    Bone { from: K, to: K },
}

fn rule(segment: &str) -> Option<Rule> {
    use K::*;
    Some(match segment {
        "pelvis" => Rule::Frame {
            left: HipL,
            right: HipR,
            up_from: &[Pelvis],
            up_to: &[Spine, Neck],
        },
        "chest" => Rule::Frame {
            left: ClavL,
            right: ClavR,
            up_from: &[ChestOrigin, Pelvis],
            up_to: &[Neck],
        },
        "spine" => Rule::Bone {
            from: Spine,
            to: ChestOrigin,
        },
        "neck" => Rule::Bone {
            from: Neck,
            to: Head,
        },
        "left_clavicle" => Rule::Bone {
            from: ClavL,
            to: ShoulderL,
        },
        "right_clavicle" => Rule::Bone {
            from: ClavR,
            to: ShoulderR,
        },
        "left_upper_arm" => Rule::Bone {
            from: ShoulderL,
            to: ElbowL,
        },
        "right_upper_arm" => Rule::Bone {
            from: ShoulderR,
            to: ElbowR,
        },
        "left_forearm" => Rule::Bone {
            from: ElbowL,
            to: WristL,
        },
        "right_forearm" => Rule::Bone {
            from: ElbowR,
            to: WristR,
        },
        "left_thigh" => Rule::Bone {
            from: HipL,
            to: KneeL,
        },
        "right_thigh" => Rule::Bone {
            from: HipR,
            to: KneeR,
        },
        "left_shin" => Rule::Bone {
            from: KneeL,
            to: AnkleL,
        },
        "right_shin" => Rule::Bone {
            from: KneeR,
            to: AnkleR,
        },
        _ => return None,
    })
}

/// A camera-only skeleton.
pub struct Fit {
    /// World rotations, rest = identity.
    pub rotations: Vec<Rot>,
    pub observed: Vec<bool>,
    pub confidence: Vec<Option<f32>>,
    pub sources: Vec<Vec<DeviceId>>,
    /// Pelvis origin, world metres.
    pub root: V3,
}

/// Fits the skeleton to a triangulated set; `None` unless the pelvis can be placed and oriented.
pub fn fit(sk: &Skeleton, landmarks: &PoseLandmarks3D) -> Option<Fit> {
    let PoseLandmarks3D::Triangulated {
        schema_id, points, ..
    } = landmarks
    else {
        return None;
    };
    let kp = Keypoints {
        schema: schema_id,
        points,
    };
    let root = kp.get(K::Pelvis, sk)?.m;
    let n = sk.len();
    let mut out = Fit {
        rotations: vec![Rot::identity(); n],
        observed: vec![false; n],
        confidence: vec![None; n],
        sources: vec![vec![]; n],
        root,
    };
    for (i, s) in sk.segments.iter().enumerate() {
        let parent = s.parent.map_or(Rot::identity(), |p| out.rotations[p]);
        let fitted = rule(s.id.as_str()).and_then(|r| match r {
            Rule::Frame {
                left,
                right,
                up_from,
                up_to,
            } => {
                let (l, r) = (kp.get(left, sk)?, kp.get(right, sk)?);
                let from = up_from.iter().find_map(|k| kp.get(*k, sk))?;
                let to = up_to.iter().find_map(|k| kp.get(*k, sk))?;
                let rot = frame(l.m - r.m, to.m - from.m)?;
                Some((rot, [l, r, from, to]))
            }
            Rule::Bone { from, to } => {
                let (a, b) = (kp.get(from, sk)?, kp.get(to, sk)?);
                let rest = parent * s.tail;
                let swing = Rot::rotation_between(&rest, &(b.m - a.m))
                    // Antiparallel: any perpendicular axis; take the parent's X.
                    .or_else(|| {
                        Some(Rot::from_axis_angle(
                            &nalgebra::Unit::new_normalize(parent * V3::x()),
                            std::f64::consts::PI,
                        ))
                    })
                    .filter(|_| (b.m - a.m).norm() > 1e-6)?;
                Some((swing * parent, [a, b, Point::none(), Point::none()]))
            }
        });
        match fitted {
            Some((rot, pts)) => {
                out.rotations[i] = rot;
                out.observed[i] = true;
                out.confidence[i] = pts
                    .iter()
                    .filter(|p| p.confidence > 0.0)
                    .map(|p| p.confidence)
                    .reduce(f32::min);
                let mut views: Vec<DeviceId> = pts.into_iter().flat_map(|p| p.views).collect();
                views.sort();
                views.dedup();
                out.sources[i] = views;
            }
            None => out.rotations[i] = parent,
        }
    }
    out.observed[0].then_some(out)
}
impl Point {
    fn none() -> Self {
        Point {
            m: V3::zeros(),
            confidence: 0.0,
            views: vec![],
        }
    }
}

/// Rotation whose X axis is along `x` and Y axis along `up` made orthogonal to it.
fn frame(x: V3, up: V3) -> Option<Rot> {
    let x = x.try_normalize(1e-6)?;
    let y = (up - x * up.dot(&x)).try_normalize(1e-6)?;
    let z = x.cross(&y);
    Some(Rot::from_rotation_matrix(
        &Rotation3::from_matrix_unchecked(Matrix3::from_columns(&[x, y, z])),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        math::v3_to_wire,
        pipeline::joints::measure,
        sources::simulated::{body::body_pose, motions::Motion},
    };
    fn set(schema: &str, pts: Vec<Option<V3>>) -> PoseLandmarks3D {
        PoseLandmarks3D::Triangulated {
            t_us: 0,
            frame: FrameRef::World,
            origin: Origin::Simulated,
            schema_id: schema.into(),
            calibration_refs: vec![],
            points: pts
                .into_iter()
                .map(|p| {
                    p.map(|m| TriangulatedLandmark3D {
                        sigma_m: None,
                        m: v3_to_wire(&m),
                        views: 2,
                        reprojection_error_px: 0.0,
                        confidence: 1.0,
                        used_views: vec![],
                        rejected_views: vec![],
                    })
                })
                .collect(),
        }
    }
    #[test]
    fn camera_fit_reproduces_scripted_knee_and_hip_angles() {
        let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
        for motion in [Motion::Squat, Motion::FrontKick, Motion::Standing] {
            for i in 0..60 {
                let t = i as f64 * 0.07;
                let body = body_pose(&sk, motion, t);
                let lm = set(
                    LANDMARK_SCHEMA_ID,
                    body.positions.iter().map(|p| Some(*p)).collect(),
                );
                let f = fit(&sk, &lm).expect("fit");
                assert!((f.root - body.root).norm() < 1e-5);
                let m = measure(&sk, &f.rotations, &f.observed);
                let a = motion.angles(t);
                for (j, expected) in [
                    (0, a.left_knee),
                    (1, a.right_knee),
                    (4, a.left_hip),
                    (5, a.right_hip),
                ] {
                    let got = m[j].value.expect("measured") as f64;
                    assert!(
                        (got - expected).abs() < 1e-5,
                        "{motion:?} {t} {j} {got} {expected}"
                    );
                }
            }
        }
    }
    #[test]
    fn coco_points_fit_legs_and_leave_head_and_feet_inferred() {
        let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
        let body = body_pose(&sk, Motion::Squat, 1.0);
        let at = |name: &str| {
            Some(
                body.positions[sk
                    .segments
                    .iter()
                    .position(|s| s.id.as_str() == name)
                    .unwrap()],
            )
        };
        let mut pts = vec![None; 17];
        for (i, name) in [
            (5, "left_upper_arm"),
            (6, "right_upper_arm"),
            (7, "left_forearm"),
            (8, "right_forearm"),
            (9, "left_hand"),
            (10, "right_hand"),
            (11, "left_thigh"),
            (12, "right_thigh"),
            (13, "left_shin"),
            (14, "right_shin"),
            (15, "left_foot"),
            (16, "right_foot"),
        ] {
            pts[i] = at(name);
        }
        let f = fit(&sk, &set(COCO17_SCHEMA_ID, pts.clone())).expect("fit");
        let observed = |name: &str| {
            f.observed[sk
                .segments
                .iter()
                .position(|s| s.id.as_str() == name)
                .unwrap()]
        };
        assert!(observed("left_shin") && observed("right_thigh") && observed("pelvis"));
        assert!(!observed("head") && !observed("left_foot") && !observed("left_hand"));
        let m = measure(&sk, &f.rotations, &f.observed);
        let a = Motion::Squat.angles(1.0);
        assert!((m[0].value.unwrap() as f64 - a.left_knee).abs() < 1e-5);
        // A missing ankle leaves the shin, and so the knee angle, unavailable.
        pts[15] = None;
        let f = fit(&sk, &set(COCO17_SCHEMA_ID, pts.clone())).unwrap();
        assert!(measure(&sk, &f.rotations, &f.observed)[0].value.is_none());
        // Without both hips nothing can be placed.
        pts[11] = None;
        assert!(fit(&sk, &set(COCO17_SCHEMA_ID, pts)).is_none());
    }
}
