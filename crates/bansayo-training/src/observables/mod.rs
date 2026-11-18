//! Observable catalogue `bansayo.observables.v1` (04 §6): the observables the kick definitions,
//! the kick metric pack and the segmenter reference. World realisations are used whenever a
//! `BodyState` exists; otherwise the image-plane realisations of 04 §6.1, §6.2 and §6.4 are served
//! from one camera's `PlanarBody`, with `method: image_plane`, `provenance: derived` and no sigma.
//! Image-plane positions and speeds carry only the normalised form (`value: null`).
pub mod angles;
pub mod filter;

use crate::content::types::NormBy;
use crate::evidence::{combine, reason, unavailable, usable};
use bansayo_contracts::{
    BODY_KEYPOINT_NAMES, BodyState, DeviceId, Evidence, MethodId, MotionSample, ObservableValue,
    PlanarBody, PlanarView, Provenance, Reason, SkeletonDefinition,
};
use filter::{CriticallyDamped, filtfilt};
use nalgebra::{Matrix3, Quaternion, Rotation3, UnitQuaternion as Rot, Vector3 as V3};
use std::collections::VecDeque;
use std::sync::OnceLock;

/// Catalogue revision stored per session (`observables_rev`, 13 §3).
pub const OBSERVABLES_REV: u32 = 1;
pub const CATALOGUE_ID: &str = "bansayo.observables.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Angle,
    Position,
    Rate,
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub name: &'static str,
    pub sided: bool,
    pub class: Class,
    /// Native unit: `rad`, `m`, `m_s` or `rad_s`.
    pub unit: &'static str,
    pub norm: Option<NormBy>,
    /// Negated by a left/right mirror (04 §10); multiplied by −1 for a right active side (05 §3).
    pub signed: bool,
    /// Defined against world up or heading; not invariant under every rigid transform.
    pub world: bool,
}

const fn e(
    name: &'static str,
    sided: bool,
    class: Class,
    unit: &'static str,
    norm: Option<NormBy>,
    signed: bool,
    world: bool,
) -> Entry {
    Entry {
        name,
        sided,
        class,
        unit,
        norm,
        signed,
        world,
    }
}

use Class::*;
use NormBy::*;
pub const ENTRIES: &[Entry] = &[
    e("knee_flexion", true, Angle, "rad", None, false, false),
    e("hip_flexion", true, Angle, "rad", None, false, false),
    e("thigh_elevation", true, Angle, "rad", None, false, false),
    e("thigh_azimuth", true, Angle, "rad", None, false, false),
    e("foot_yaw", true, Angle, "rad", None, false, true),
    e("pelvis_yaw", false, Angle, "rad", None, true, true),
    e(
        "ankle_height",
        true,
        Position,
        "m",
        Some(LegLength),
        false,
        true,
    ),
    e(
        "knee_height",
        true,
        Position,
        "m",
        Some(LegLength),
        false,
        true,
    ),
    e(
        "toe_height",
        true,
        Position,
        "m",
        Some(LegLength),
        false,
        true,
    ),
    e(
        "heel_height",
        true,
        Position,
        "m",
        Some(LegLength),
        false,
        true,
    ),
    e(
        "guard_distance",
        true,
        Position,
        "m",
        Some(ArmLength),
        false,
        false,
    ),
    e(
        "com_proxy_offset",
        true,
        Position,
        "m",
        Some(FootLength),
        false,
        true,
    ),
    e(
        "ankle_speed",
        true,
        Rate,
        "m_s",
        Some(LegLength),
        false,
        false,
    ),
    e(
        "knee_speed",
        true,
        Rate,
        "m_s",
        Some(LegLength),
        false,
        false,
    ),
    e(
        "wrist_speed",
        true,
        Rate,
        "m_s",
        Some(ArmLength),
        false,
        false,
    ),
    e(
        "elbow_speed",
        true,
        Rate,
        "m_s",
        Some(ArmLength),
        false,
        false,
    ),
    e(
        "root_speed",
        false,
        Rate,
        "m_s",
        Some(LegLength),
        false,
        true,
    ),
    e(
        "knee_extension_rate",
        true,
        Rate,
        "rad_s",
        None,
        false,
        false,
    ),
];

/// Expanded ids in catalogue order: `name.left`, `name.right` or `name`.
pub fn ids() -> &'static [(String, usize)] {
    static IDS: OnceLock<Vec<(String, usize)>> = OnceLock::new();
    IDS.get_or_init(|| {
        ENTRIES
            .iter()
            .enumerate()
            .flat_map(|(i, e)| {
                if e.sided {
                    vec![
                        (format!("{}.left", e.name), i),
                        (format!("{}.right", e.name), i),
                    ]
                } else {
                    vec![(e.name.to_string(), i)]
                }
            })
            .collect()
    })
}

pub fn index(id: &str) -> Option<usize> {
    ids().iter().position(|(i, _)| i == id)
}

pub fn entry(index: usize) -> &'static Entry {
    &ENTRIES[ids()[index].1]
}

/// The id of the same observable on the other side, or itself when unsided.
pub fn mirror_index(index: usize) -> usize {
    let id = &ids()[index].0;
    let other = if let Some(base) = id.strip_suffix(".left") {
        format!("{base}.right")
    } else if let Some(base) = id.strip_suffix(".right") {
        format!("{base}.left")
    } else {
        return index;
    };
    self::index(&other).unwrap_or(index)
}

/// Observables of one tick, index-aligned with [`ids`].
#[derive(Debug, Clone, PartialEq)]
pub struct ObservableFrame {
    pub t_us: u64,
    pub values: Vec<ObservableValue>,
}

impl ObservableFrame {
    pub fn get(&self, index: usize) -> &ObservableValue {
        &self.values[index]
    }
}

/// Normalising lengths of the active skeleton (04 §5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lengths {
    pub stature: f64,
    pub leg_length: f64,
    pub arm_length: f64,
    pub shoulder_width: f64,
    pub foot_length: f64,
}

impl Lengths {
    pub fn by(&self, n: NormBy) -> f64 {
        match n {
            LegLength => self.leg_length,
            ArmLength => self.arm_length,
            Stature => self.stature,
            ShoulderWidth => self.shoulder_width,
            FootLength => self.foot_length,
        }
    }
}

#[derive(Debug, Clone)]
struct Geometry {
    lengths: Lengths,
    /// Rest pelvis-to-neck length: the planar torso unit in metres (04 §6.2 image-plane form).
    torso: f64,
    /// Rest height of the ankle joint above the floor, metres.
    ankle_rest: f64,
    /// `[pelvis, chest, l_thigh, l_shin, l_foot, r_thigh, r_shin, r_foot]` segment indices.
    seg: [usize; 8],
    thigh_tail: [V3<f64>; 2],
    shin_tail: [V3<f64>; 2],
}

fn v(a: [f32; 3]) -> V3<f64> {
    V3::new(a[0].into(), a[1].into(), a[2].into())
}

fn q(a: [f32; 4]) -> Rot<f64> {
    Rot::from_quaternion(Quaternion::new(
        a[3].into(),
        a[0].into(),
        a[1].into(),
        a[2].into(),
    ))
}

impl Geometry {
    fn new(sk: &SkeletonDefinition) -> Result<Self, String> {
        let find = |id: &str| {
            sk.segments
                .iter()
                .position(|s| s.id.as_str() == id)
                .ok_or_else(|| format!("skeleton has no segment {id}"))
        };
        let seg = [
            find("pelvis")?,
            find("chest")?,
            find("left_thigh")?,
            find("left_shin")?,
            find("left_foot")?,
            find("right_thigh")?,
            find("right_shin")?,
            find("right_foot")?,
        ];
        let mut rest = vec![V3::zeros(); sk.segments.len()];
        for (i, s) in sk.segments.iter().enumerate() {
            let parent = s
                .parent
                .as_ref()
                .and_then(|p| sk.segments.iter().position(|x| &x.id == p));
            rest[i] = parent.map_or(V3::zeros(), |p| rest[p]) + v(s.offset_m);
        }
        let tail = |i: usize| v(sk.segments[i].tail_m);
        let (lu, lf, ru) = (
            find("left_upper_arm")?,
            find("left_forearm")?,
            find("right_upper_arm")?,
        );
        let foot = &sk.segments[seg[4]].contact_points_m;
        let foot_length = match (foot.first(), foot.get(1)) {
            (Some(h), Some(t)) => (v(*t) - v(*h)).norm(),
            _ => tail(seg[4]).norm(),
        };
        let floor = foot
            .iter()
            .map(|c| (rest[seg[4]] + v(*c)).y)
            .fold(rest[seg[4]].y, f64::min);
        Ok(Self {
            torso: (rest[find("neck")?] - rest[seg[0]]).norm(),
            ankle_rest: rest[seg[4]].y - floor,
            lengths: Lengths {
                stature: sk.height_m.into(),
                leg_length: tail(seg[2]).norm() + tail(seg[3]).norm(),
                arm_length: tail(lu).norm() + tail(lf).norm(),
                shoulder_width: (rest[lu] - rest[ru]).norm(),
                foot_length,
            },
            seg,
            thigh_tail: [tail(seg[2]), tail(seg[5])],
            shin_tail: [tail(seg[3]), tail(seg[6])],
        })
    }
}

type Value = Result<(f64, Evidence), Reason>;
type Point = Result<(V3<f64>, Evidence), Reason>;

fn kp(name: &str) -> usize {
    BODY_KEYPOINT_NAMES
        .iter()
        .position(|n| *n == name)
        .expect("canonical keypoint")
}

fn missing(what: &str) -> Reason {
    reason(
        "input_not_observed",
        Some(what),
        "A required body point or segment is not observed.",
    )
}

fn point(body: &BodyState, name: &str) -> Point {
    match body.keypoints.get(kp(name)).and_then(Option::as_ref) {
        Some(k) if usable(&k.evidence) => Ok((v(k.position_m), k.evidence.clone())),
        _ => Err(missing(name)),
    }
}

fn axis(body: &BodyState, seg: usize, tail: &V3<f64>, what: &str) -> Point {
    match body.segments.get(seg) {
        Some(s) if usable(&s.axis) => Ok(((q(s.rotation) * tail).normalize(), s.axis.clone())),
        _ => Err(missing(what)),
    }
}

/// Pelvis frame: the full rotation when its twist is observed, otherwise from the hip points
/// and the pelvis long axis (both determine it; 04 §6.1 allows hips for the heading).
fn pelvis_frame(body: &BodyState, g: &Geometry) -> Result<(Rot<f64>, Evidence), Reason> {
    let s = body
        .segments
        .get(g.seg[0])
        .ok_or_else(|| missing("pelvis"))?;
    if usable(&s.axis) && usable(&s.twist) {
        let e = combine([&s.axis, &s.twist]).unwrap_or_else(unavailable);
        return Ok((q(s.rotation), e));
    }
    let (up, eu) = axis(body, g.seg[0], &V3::y(), "pelvis")?;
    let (l, el) = point(body, "left_hip")?;
    let (r, er) = point(body, "right_hip")?;
    let left = (l - r) - up * (l - r).dot(&up);
    if left.norm() < 1e-6 {
        return Err(missing("pelvis"));
    }
    let left = left.normalize();
    let m = Matrix3::from_columns(&[left, up, left.cross(&up)]);
    let e = combine([&eu, &el, &er]).unwrap_or_else(unavailable);
    Ok((
        Rot::from_rotation_matrix(&Rotation3::from_matrix_unchecked(m)),
        e,
    ))
}

fn side_name(s: usize) -> &'static str {
    ["left", "right"][s]
}

fn with(e: Evidence, provenance_floor: Provenance) -> Evidence {
    let mut e = e;
    if e.provenance == Provenance::Measured && provenance_floor == Provenance::Derived {
        e.provenance = Provenance::Derived;
    }
    e
}

/// Thigh long axis in the pelvis frame, with the combined evidence.
fn thigh_in_pelvis(body: &BodyState, g: &Geometry, s: usize) -> Point {
    let (pelvis, ep) = pelvis_frame(body, g)?;
    let (thigh, et) = axis(body, g.seg[2 + 3 * s], &g.thigh_tail[s], "thigh")?;
    Ok((
        pelvis.inverse() * thigh,
        combine([&ep, &et]).unwrap_or_else(unavailable),
    ))
}

fn heading(d: &V3<f64>) -> f64 {
    d.x.atan2(d.z)
}

/// Raw (unfiltered) value of a non-rate observable.
fn base(body: &BodyState, g: &Geometry, name: &str, s: usize) -> Value {
    let sign = if s == 0 { 1.0 } else { -1.0 };
    let side = side_name(s);
    let height = |k: &str| point(body, &format!("{side}_{k}")).map(|(p, e)| (p.y, e));
    match name {
        "knee_flexion" => {
            let (a, ea) = axis(body, g.seg[2 + 3 * s], &g.thigh_tail[s], "thigh")?;
            let (b, eb) = axis(body, g.seg[3 + 3 * s], &g.shin_tail[s], "shin")?;
            Ok((
                a.dot(&b).clamp(-1.0, 1.0).acos(),
                combine([&ea, &eb]).unwrap_or_else(unavailable),
            ))
        }
        "thigh_elevation" => {
            // Only the pelvis long axis enters: twist about it cannot change a_p.y.
            let (up, ep) = axis(body, g.seg[0], &V3::y(), "pelvis")?;
            let (t, et) = axis(body, g.seg[2 + 3 * s], &g.thigh_tail[s], "thigh")?;
            Ok((
                (-t.dot(&up)).clamp(-1.0, 1.0).acos(),
                combine([&ep, &et]).unwrap_or_else(unavailable),
            ))
        }
        "hip_flexion" => {
            let (a, e) = thigh_in_pelvis(body, g, s)?;
            if (a.y * a.y + a.z * a.z).sqrt() < 0.35 {
                return Err(reason(
                    "out_of_plane",
                    Some(side),
                    "The thigh is too far out of the sagittal plane for a stable value.",
                ));
            }
            Ok((a.z.atan2(-a.y), e))
        }
        "thigh_azimuth" => {
            let (a, e) = thigh_in_pelvis(body, g, s)?;
            if (-a.y).clamp(-1.0, 1.0).acos() < 0.26 {
                return Err(reason(
                    "direction_undefined",
                    Some(side),
                    "The thigh is too close to hanging for its direction to be defined.",
                ));
            }
            Ok(((sign * a.x).atan2(a.z), e))
        }
        "foot_yaw" => {
            let (h, eh) = point(body, &format!("{side}_heel"))?;
            let (t, et) = point(body, &format!("{side}_toe"))?;
            let d = t - h;
            if d.y.abs().atan2((d.x * d.x + d.z * d.z).sqrt()) > 1.05 {
                return Err(reason(
                    "foot_vertical",
                    Some(side),
                    "The foot points too steeply for its heading to be defined.",
                ));
            }
            Ok((heading(&d), combine([&eh, &et]).unwrap_or_else(unavailable)))
        }
        "pelvis_yaw" => {
            let s = body
                .segments
                .get(g.seg[0])
                .ok_or_else(|| missing("pelvis"))?;
            if usable(&s.axis) && usable(&s.twist) {
                let e = combine([&s.axis, &s.twist]).unwrap_or_else(unavailable);
                return Ok((heading(&(q(s.rotation) * V3::z())), e));
            }
            let (l, el) = point(body, "left_hip")?;
            let (r, er) = point(body, "right_hip")?;
            let f = (l - r).cross(&V3::y());
            if f.norm() < 1e-6 {
                return Err(missing("pelvis"));
            }
            Ok((heading(&f), combine([&el, &er]).unwrap_or_else(unavailable)))
        }
        "ankle_height" => height("ankle"),
        "knee_height" => height("knee"),
        "toe_height" => height("toe"),
        "heel_height" => height("heel"),
        "guard_distance" => {
            let (w, ew) = point(body, &format!("{side}_wrist"))?;
            let (h, eh) = point(body, "head")?;
            let (t, et) = point(body, "head_top")?;
            Ok((
                (w - (h + t) * 0.5).norm(),
                combine([&ew, &eh, &et]).unwrap_or_else(unavailable),
            ))
        }
        "com_proxy_offset" => com_offset(body, side),
        other => Err(reason(
            "not_implemented",
            Some(other),
            "This observable is not computed by this engine.",
        )),
    }
}

/// Segment mass fractions after Winter, at segment centres between canonical keypoints.
const MASS: &[(&str, &str, f64, bool)] = &[
    ("pelvis", "neck", 0.497, false),
    ("head", "head_top", 0.081, false),
    ("shoulder", "elbow", 0.028, true),
    ("elbow", "wrist", 0.016, true),
    ("wrist", "hand_tip", 0.006, true),
    ("hip", "knee", 0.100, true),
    ("knee", "ankle", 0.0465, true),
    ("heel", "toe", 0.0145, true),
];

fn com_offset(body: &BodyState, side: &str) -> Value {
    let mut centre = V3::zeros();
    let mut evidence = vec![];
    for (a, b, m, sided) in MASS {
        let sides: &[&str] = if *sided { &["left_", "right_"] } else { &[""] };
        for p in sides {
            let (pa, ea) = point(body, &format!("{p}{a}"))?;
            let (pb, eb) = point(body, &format!("{p}{b}"))?;
            centre += (pa + pb) * 0.5 * *m;
            evidence.extend([ea, eb]);
        }
    }
    let (h, eh) = point(body, &format!("{side}_heel"))?;
    let (t, et) = point(body, &format!("{side}_toe"))?;
    let base = (h + t) * 0.5;
    evidence.extend([eh, et]);
    let d = centre - base;
    Ok((
        (d.x * d.x + d.z * d.z).sqrt(),
        combine(evidence.iter()).unwrap_or_else(unavailable),
    ))
}

fn needs_world(what: &str) -> Reason {
    reason(
        "needs_calibrated_cameras",
        Some(what),
        "needs two calibrated cameras",
    )
}

fn no_feet(what: &str) -> Reason {
    reason(
        "detector_no_feet",
        Some(what),
        "pose model has no foot points",
    )
}

fn scale_missing() -> Reason {
    reason(
        "planar_scale_missing",
        None,
        "hold still in the start position so body size can be measured",
    )
}

/// One camera's planar body with the floor level captured with its reference.
#[derive(Debug, Clone, Copy)]
struct Planar<'a> {
    body: &'a PlanarBody,
    /// Planar `y` of the floor, torso lengths.
    floor: f64,
}

type P2 = nalgebra::Vector2<f64>;

fn planar_point(p: &Planar, name: &str) -> Result<(P2, f32), Reason> {
    p.body
        .keypoints
        .get(kp(name))
        .and_then(Option::as_ref)
        .map(|k| (P2::new(k.x.into(), k.y.into()), k.confidence))
        .ok_or_else(|| missing(name))
}

fn planar_evidence(p: &Planar, confidences: &[f32]) -> Evidence {
    Evidence {
        provenance: Provenance::Derived,
        method: MethodId::ImagePlane,
        confidence: confidences.iter().copied().reduce(f32::min),
        sigma: None,
        sigma_basis: None,
        t_obs_us: Some(p.body.t_obs_us),
        sources: vec![p.body.camera_id.clone()],
    }
}

fn angle_between(a: &P2, b: &P2) -> f64 {
    (a.dot(b) / (a.norm() * b.norm()).max(1e-12))
        .clamp(-1.0, 1.0)
        .acos()
}

/// Image-plane realisation of a non-rate observable (04 §6.1, §6.2). Positions are returned in
/// metres-equivalent (torso lengths × the rest torso length) so their normalised form divides
/// by the usual length; the metre value itself is never published.
fn planar_base(p: &Planar, g: &Geometry, name: &str, s: usize) -> Value {
    let side = side_name(s);
    let pt = |k: &str| planar_point(p, &format!("{side}_{k}"));
    let feet = |k: &str| {
        pt(k).map_err(|_| {
            if p.body
                .keypoints
                .get(kp("left_heel"))
                .is_some_and(Option::is_some)
            {
                needs_world(name)
            } else {
                no_feet(name)
            }
        })
    };
    let height = |(q, c): (P2, f32)| Ok(((q.y - p.floor) * g.torso, planar_evidence(p, &[c])));
    match name {
        "knee_flexion" => {
            let ((h, ch), (k, ck), (a, ca)) = (pt("hip")?, pt("knee")?, pt("ankle")?);
            Ok((
                std::f64::consts::PI - angle_between(&(h - k), &(a - k)),
                planar_evidence(p, &[ch, ck, ca]),
            ))
        }
        "thigh_elevation" => {
            let ((h, ch), (k, ck)) = (pt("hip")?, pt("knee")?);
            Ok((
                angle_between(&(k - h), &P2::new(0.0, -1.0)),
                planar_evidence(p, &[ch, ck]),
            ))
        }
        "hip_flexion" => {
            if p.body.view != PlanarView::Lateral {
                return Err(reason(
                    "view_unsuitable",
                    Some(name),
                    "camera view unsuitable (needs a side view)",
                ));
            }
            let ((h, ch), (k, ck)) = (pt("hip")?, pt("knee")?);
            let (pelvis, cp) = planar_point(p, "pelvis")?;
            let (neck, cn) = planar_point(p, "neck")?;
            // shortcut: unsigned, because one side view does not say which way the athlete faces;
            // sign it from a facing estimate (nose or toes) if a definition needs extension.
            Ok((
                angle_between(&(k - h), &(pelvis - neck)),
                planar_evidence(p, &[ch, ck, cp, cn]),
            ))
        }
        "ankle_height" => height(pt("ankle")?),
        "knee_height" => height(pt("knee")?),
        "toe_height" => height(feet("toe")?),
        "heel_height" => height(feet("heel")?),
        "foot_yaw" => Err(feet("toe")
            .and(feet("heel"))
            .map_or_else(|e| e, |_| needs_world(name))),
        // Headings, twist, and distances that need depth have no image-plane realisation.
        _ => Err(needs_world(name)),
    }
}

/// Keypoints whose velocities define the speed observables.
fn speed_points() -> &'static [(&'static str, &'static str)] {
    &[
        ("ankle_speed.left", "left_ankle"),
        ("ankle_speed.right", "right_ankle"),
        ("knee_speed.left", "left_knee"),
        ("knee_speed.right", "right_knee"),
        ("wrist_speed.left", "left_wrist"),
        ("wrist_speed.right", "right_wrist"),
        ("elbow_speed.left", "left_elbow"),
        ("elbow_speed.right", "right_elbow"),
        ("root_speed", "pelvis"),
    ]
}

fn is_heading(name: &str) -> bool {
    matches!(name, "pelvis_yaw" | "foot_yaw")
}

/// Where the observables of one tick come from (04 §6.4).
#[derive(Debug, Clone, Copy)]
enum Input<'a> {
    World(&'a BodyState),
    Planar(Planar<'a>),
    /// A camera delivers landmarks but the planar reference is not captured yet.
    ScaleMissing,
    Nothing,
}

/// Every non-rate observable and every speed point of one tick, raw.
fn raw(input: Input, g: &Geometry) -> (Vec<Option<Value>>, Vec<Point>) {
    let none = || {
        reason(
            "no_world_body",
            None,
            "No body is observed: no world skeleton and no camera landmarks.",
        )
    };
    let values = ids()
        .iter()
        .map(|(id, i)| {
            let e = &ENTRIES[*i];
            if e.class == Rate {
                return None;
            }
            let s = usize::from(id.ends_with(".right"));
            Some(match input {
                Input::World(b) => base(b, g, e.name, s),
                Input::Planar(p) => planar_base(&p, g, e.name, s),
                Input::ScaleMissing => Err(scale_missing()),
                Input::Nothing => Err(none()),
            })
        })
        .collect();
    let points = speed_points()
        .iter()
        .map(|(id, k)| match input {
            Input::World(b) => point(b, k),
            Input::Planar(_) if *id == "root_speed" => Err(needs_world("root_speed")),
            Input::Planar(p) => planar_point(&p, k).map(|(q, c)| {
                (
                    V3::new(q.x * g.torso, q.y * g.torso, 0.0),
                    planar_evidence(&p, &[c]),
                )
            }),
            Input::ScaleMissing => Err(scale_missing()),
            Input::Nothing => Err(none()),
        })
        .collect();
    (values, points)
}

/// Image-plane positions and speeds publish only their normalised form (04 §6.2).
fn published(mut v: ObservableValue, index: usize) -> ObservableValue {
    if v.evidence.method == MethodId::ImagePlane && entry(index).norm.is_some() {
        v.value = None;
    }
    v
}

/// Whether an observable has a usable value in either form.
pub fn present(v: &ObservableValue) -> bool {
    v.value.is_some() || v.norm.is_some()
}

#[derive(Debug, Clone)]
struct Row {
    t_obs: u64,
    values: Vec<Option<Value>>,
    points: Vec<Point>,
}

#[derive(Debug, Clone, Default)]
struct PlanarTrack {
    confidence: f64,
    t_us: u64,
    /// `(scale_px, floor y)` of the current reference.
    floor: Option<(f32, f64)>,
}

#[derive(Debug, Clone, Copy)]
pub struct Cutoffs {
    pub position_hz: f64,
    pub angle_hz: f64,
}

impl Default for Cutoffs {
    fn default() -> Self {
        Self {
            position_hz: 12.0,
            angle_hz: 12.0,
        }
    }
}

#[derive(Debug, Clone)]
struct ScalarState {
    filter: CriticallyDamped,
    raw_last: Option<f64>,
    last_t: Option<u64>,
    /// `(t_obs, filtered)` of the two newest distinct inputs.
    newest: [Option<(u64, f64)>; 2],
}

#[derive(Debug, Clone)]
struct PointState {
    filter: [CriticallyDamped; 3],
    last_t: Option<u64>,
    newest: [Option<(u64, V3<f64>)>; 2],
}

/// Live observable computation for one training session. History is bounded by
/// [`HISTORY_US`] so completed repetitions can be re-filtered with zero phase.
#[derive(Debug, Clone)]
pub struct Observer {
    geometry: Geometry,
    cutoffs: Cutoffs,
    /// `analysis.planar_camera`: `None` = auto (highest mean confidence over about a second).
    planar_camera: Option<DeviceId>,
    /// Image-plane realisations are served only while this holds: at profile `basic` (08 §10,
    /// "only one camera left"). A multi-view rig's momentary loss of the world body stays a gap.
    planar_allowed: bool,
    /// Per camera: smoothed mean keypoint confidence and the floor captured with its reference.
    planar: std::collections::BTreeMap<DeviceId, PlanarTrack>,
    scalars: Vec<ScalarState>,
    points: Vec<PointState>,
    held: Vec<ObservableValue>,
    history: VecDeque<Row>,
}

pub const HISTORY_US: u64 = 12_000_000;
/// Samples before a repetition fed to the zero-phase filter so its start transient decays.
pub const REFILTER_PAD_US: u64 = 500_000;

fn empty(id: &str, why: Reason) -> ObservableValue {
    ObservableValue {
        id: id.into(),
        value: None,
        norm: None,
        evidence: unavailable(),
        fresh: false,
        reasons: vec![why],
    }
}

impl Observer {
    pub fn new(skeleton: &SkeletonDefinition, cutoffs: Cutoffs) -> Result<Self, String> {
        Self::with_planar_camera(skeleton, cutoffs, None)
    }

    pub fn with_planar_camera(
        skeleton: &SkeletonDefinition,
        cutoffs: Cutoffs,
        planar_camera: Option<DeviceId>,
    ) -> Result<Self, String> {
        let geometry = Geometry::new(skeleton)?;
        let scalars = ids()
            .iter()
            .map(|(_, i)| ScalarState {
                filter: CriticallyDamped::new(match ENTRIES[*i].class {
                    Angle => cutoffs.angle_hz,
                    _ => cutoffs.position_hz,
                }),
                raw_last: None,
                last_t: None,
                newest: [None, None],
            })
            .collect();
        let points = speed_points()
            .iter()
            .map(|_| PointState {
                filter: [CriticallyDamped::new(cutoffs.position_hz); 3],
                last_t: None,
                newest: [None, None],
            })
            .collect();
        let held = ids()
            .iter()
            .map(|(id, _)| empty(id, reason("no_sample", None, "No sample received yet.")))
            .collect();
        Ok(Self {
            geometry,
            cutoffs,
            planar_camera,
            planar_allowed: true,
            planar: Default::default(),
            scalars,
            points,
            held,
            history: VecDeque::new(),
        })
    }

    pub fn lengths(&self) -> Lengths {
        self.geometry.lengths
    }

    fn norm_of(&self, index: usize, value: f64) -> Option<f64> {
        entry(index)
            .norm
            .map(|n| value / self.geometry.lengths.by(n))
    }

    /// Whether image-plane realisations may stand in for a missing world body.
    pub fn allow_planar(&mut self, allowed: bool) {
        self.planar_allowed = allowed;
    }

    /// Picks the planar body observables are served from when there is no world body (04 §6.4).
    fn input<'a>(&mut self, sample: &'a MotionSample) -> Input<'a> {
        if let Some(b) = &sample.body {
            return Input::World(b);
        }
        if !self.planar_allowed {
            return Input::Nothing;
        }
        for b in &sample.planar {
            let seen: Vec<f64> = b
                .keypoints
                .iter()
                .flatten()
                .map(|k| f64::from(k.confidence))
                .collect();
            let t = self.planar.entry(b.camera_id.clone()).or_default();
            if b.t_obs_us != t.t_us && !seen.is_empty() {
                let mean = seen.iter().sum::<f64>() / seen.len() as f64;
                let dt = b.t_obs_us.saturating_sub(t.t_us) as f64 * 1e-6;
                let a = if t.t_us == 0 {
                    1.0
                } else {
                    (dt / 1.0).min(1.0)
                };
                t.confidence += a * (mean - t.confidence);
                t.t_us = b.t_obs_us;
            }
        }
        let chosen = match &self.planar_camera {
            Some(id) => sample.planar.iter().find(|b| &b.camera_id == id),
            None => sample.planar.iter().max_by(|a, b| {
                let c =
                    |x: &PlanarBody| self.planar.get(&x.camera_id).map_or(0.0, |t| t.confidence);
                c(a).total_cmp(&c(b))
                    .then_with(|| b.camera_id.cmp(&a.camera_id))
            }),
        };
        let Some(body) = chosen else {
            return Input::Nothing;
        };
        let Some(scale) = body.scale_px else {
            return Input::ScaleMissing;
        };
        // The floor is taken from the first frame of a reference that shows both ankles, while
        // the athlete still stands as captured.
        let rest = self.geometry.ankle_rest / self.geometry.torso;
        let track = self.planar.entry(body.camera_id.clone()).or_default();
        if track.floor.is_none_or(|(s, _)| s != scale) {
            let ankles: Vec<f64> = ["left_ankle", "right_ankle"]
                .iter()
                .filter_map(|n| body.keypoints.get(kp(n)).and_then(Option::as_ref))
                .map(|k| f64::from(k.y))
                .collect();
            track.floor =
                (ankles.len() == 2).then(|| (scale, (ankles[0] + ankles[1]) / 2.0 - rest));
        }
        match track.floor {
            Some((_, floor)) => Input::Planar(Planar { body, floor }),
            None => Input::ScaleMissing,
        }
    }

    /// Advances by one solver tick and returns every observable (latest held between inputs).
    pub fn step(&mut self, sample: &MotionSample) -> ObservableFrame {
        let input = self.input(sample);
        let (values, points) = raw(input, &self.geometry);
        let mut fresh_any = false;
        let mut row_values = values.clone();
        for (i, v) in values.into_iter().enumerate() {
            let Some(v) = v else { continue };
            let id = ids()[i].0.clone();
            match v {
                Err(why) => {
                    self.scalars[i].filter.reset();
                    self.scalars[i].raw_last = None;
                    self.scalars[i].newest = [None, None];
                    self.held[i] = empty(&id, why);
                }
                Ok((raw, ev)) => {
                    let t = ev.t_obs_us.unwrap_or(sample.t_us);
                    let st = &mut self.scalars[i];
                    let raw = match (is_heading(entry(i).name), st.raw_last) {
                        (true, Some(last)) => unwrap_near(raw, last),
                        _ => raw,
                    };
                    row_values[i] = Some(Ok((raw, ev.clone())));
                    if st.last_t.is_some_and(|l| t <= l) {
                        self.held[i].fresh = false;
                        continue;
                    }
                    let dt = st.last_t.map_or(0.0, |l| (t - l) as f64 * 1e-6);
                    let y = st.filter.update(raw, dt);
                    st.raw_last = Some(raw);
                    st.last_t = Some(t);
                    st.newest = [Some((t, y)), st.newest[0]];
                    fresh_any = true;
                    self.held[i] = ObservableValue {
                        id,
                        value: Some(y),
                        norm: self.norm_of(i, y),
                        evidence: ev,
                        fresh: true,
                        reasons: vec![],
                    };
                }
            }
        }
        for (k, p) in points.iter().enumerate() {
            let st = &mut self.points[k];
            match p {
                Err(_) => {
                    st.filter.iter_mut().for_each(CriticallyDamped::reset);
                    st.newest = [None, None];
                }
                Ok((p, ev)) => {
                    let t = ev.t_obs_us.unwrap_or(sample.t_us);
                    if st.last_t.is_some_and(|l| t <= l) {
                        continue;
                    }
                    let dt = st.last_t.map_or(0.0, |l| (t - l) as f64 * 1e-6);
                    let y = V3::from_fn(|r, _| st.filter[r].update(p[r], dt));
                    st.last_t = Some(t);
                    st.newest = [Some((t, y)), st.newest[0]];
                    fresh_any = true;
                }
            }
        }
        self.rates(&points);
        if fresh_any {
            let t_obs = self
                .held
                .iter()
                .filter(|v| v.fresh)
                .filter_map(|v| v.evidence.t_obs_us)
                .max()
                .unwrap_or(sample.t_us);
            self.history.push_back(Row {
                t_obs,
                values: row_values,
                points,
            });
            while self
                .history
                .front()
                .is_some_and(|r| t_obs.saturating_sub(r.t_obs) > HISTORY_US)
            {
                self.history.pop_front();
            }
        }
        ObservableFrame {
            t_us: sample.t_us,
            values: self
                .held
                .iter()
                .enumerate()
                .map(|(i, v)| published(v.clone(), i))
                .collect(),
        }
    }

    /// Rates from the two newest distinct filtered inputs (04 §6.3), held between inputs.
    fn rates(&mut self, points: &[Point]) {
        for (k, (id, _)) in speed_points().iter().enumerate() {
            let i = index(id).expect("catalogue id");
            let st = &self.points[k];
            match (&points[k], st.newest) {
                (Err(why), _) => self.held[i] = empty(id, why.clone()),
                (Ok((_, ev)), [Some((t1, p1)), Some((t0, p0))]) => {
                    if self.held[i].evidence.t_obs_us == Some(t1) && self.held[i].value.is_some() {
                        self.held[i].fresh = false;
                        continue;
                    }
                    let mut d = (p1 - p0) / ((t1 - t0) as f64 * 1e-6);
                    if *id == "root_speed" {
                        d.y = 0.0;
                    }
                    let speed = d.norm();
                    let mut ev = with(ev.clone(), Provenance::Derived);
                    ev.t_obs_us = Some(t1);
                    self.held[i] = ObservableValue {
                        id: (*id).into(),
                        value: Some(speed),
                        norm: self.norm_of(i, speed),
                        evidence: ev,
                        fresh: true,
                        reasons: vec![],
                    };
                }
                (Ok(_), _) => {
                    self.held[i] = empty(
                        id,
                        reason("warming_up", None, "Two inputs are needed for a rate."),
                    )
                }
            }
        }
        for s in ["left", "right"] {
            let i = index(&format!("knee_extension_rate.{s}")).expect("catalogue id");
            let k = index(&format!("knee_flexion.{s}")).expect("catalogue id");
            let source = &self.held[k];
            match (source.value, self.scalars[k].newest) {
                (Some(_), [Some((t1, a1)), Some((t0, a0))]) => {
                    if self.held[i].evidence.t_obs_us == Some(t1) && self.held[i].value.is_some() {
                        self.held[i].fresh = false;
                        continue;
                    }
                    let rate = -(a1 - a0) / ((t1 - t0) as f64 * 1e-6);
                    self.held[i] = ObservableValue {
                        id: format!("knee_extension_rate.{s}"),
                        value: Some(rate),
                        norm: None,
                        evidence: with(source.evidence.clone(), Provenance::Derived),
                        fresh: true,
                        reasons: vec![],
                    };
                }
                (None, _) => {
                    let why = source
                        .reasons
                        .first()
                        .cloned()
                        .unwrap_or_else(|| missing("knee"));
                    self.held[i] = empty(&format!("knee_extension_rate.{s}"), why);
                }
                _ => {
                    self.held[i] = empty(
                        &format!("knee_extension_rate.{s}"),
                        reason("warming_up", None, "Two inputs are needed for a rate."),
                    )
                }
            }
        }
    }

    /// Frames rebuilt from raw history between `from_us − REFILTER_PAD_US` and `to_us` with the
    /// zero-phase filter (04 §6.3), one per distinct input time. Gaps split the filtered runs.
    pub fn refiltered(&self, from_us: u64, to_us: u64) -> Vec<ObservableFrame> {
        let start = from_us.saturating_sub(REFILTER_PAD_US);
        let rows: Vec<&Row> = self
            .history
            .iter()
            .filter(|r| r.t_obs >= start && r.t_obs <= to_us)
            .collect();
        if rows.is_empty() {
            return vec![];
        }
        let n = rows.len();
        let mut out: Vec<Vec<ObservableValue>> = vec![self.held.clone(); n];
        for (i, (id, e)) in ids().iter().enumerate() {
            if ENTRIES[*e].class == Rate {
                continue;
            }
            let cutoff = if ENTRIES[*e].class == Angle {
                self.cutoffs.angle_hz
            } else {
                self.cutoffs.position_hz
            };
            let series: Vec<Option<(f64, Evidence)>> = rows
                .iter()
                .map(|r| r.values[i].as_ref().and_then(|v| v.as_ref().ok().cloned()))
                .collect();
            let filtered = filter_runs(
                &rows,
                &series
                    .iter()
                    .map(|s| s.as_ref().map(|s| s.0))
                    .collect::<Vec<_>>(),
                cutoff,
            );
            for (r, f) in filtered.iter().enumerate() {
                out[r][i] = match (f, &series[r], &rows[r].values[i]) {
                    (Some(y), Some((_, ev)), _) => ObservableValue {
                        id: id.clone(),
                        value: Some(*y),
                        norm: self.norm_of(i, *y),
                        evidence: ev.clone(),
                        fresh: true,
                        reasons: vec![],
                    },
                    (_, _, Some(Err(why))) => empty(id, why.clone()),
                    _ => empty(id, missing(id)),
                };
            }
        }
        let mut positions: Vec<Vec<Option<V3<f64>>>> = vec![];
        for k in 0..speed_points().len() {
            let comps: Vec<Vec<Option<f64>>> = (0..3)
                .map(|c| {
                    rows.iter()
                        .map(|r| r.points[k].as_ref().ok().map(|(p, _)| p[c]))
                        .collect()
                })
                .collect();
            let f: Vec<Vec<Option<f64>>> = comps
                .iter()
                .map(|c| filter_runs(&rows, c, self.cutoffs.position_hz))
                .collect();
            positions.push(
                (0..n)
                    .map(|r| match (f[0][r], f[1][r], f[2][r]) {
                        (Some(x), Some(y), Some(z)) => Some(V3::new(x, y, z)),
                        _ => None,
                    })
                    .collect(),
            );
        }
        for r in 0..n {
            for (k, (id, _)) in speed_points().iter().enumerate() {
                let i = index(id).expect("catalogue id");
                out[r][i] = match (r.checked_sub(1), &rows[r].points[k]) {
                    (Some(p), Ok((_, ev))) => match (positions[k][p], positions[k][r]) {
                        (Some(a), Some(b)) => {
                            let mut d = (b - a) / ((rows[r].t_obs - rows[p].t_obs) as f64 * 1e-6);
                            if *id == "root_speed" {
                                d.y = 0.0;
                            }
                            let s = d.norm();
                            ObservableValue {
                                id: (*id).into(),
                                value: Some(s),
                                norm: self.norm_of(i, s),
                                evidence: with(ev.clone(), Provenance::Derived),
                                fresh: true,
                                reasons: vec![],
                            }
                        }
                        _ => empty(id, missing(id)),
                    },
                    (_, Err(why)) => empty(id, why.clone()),
                    _ => empty(id, missing(id)),
                };
            }
            for s in ["left", "right"] {
                let i = index(&format!("knee_extension_rate.{s}")).expect("catalogue id");
                let k = index(&format!("knee_flexion.{s}")).expect("catalogue id");
                let id = format!("knee_extension_rate.{s}");
                out[r][i] = match r.checked_sub(1) {
                    Some(p) => match (out[p][k].value, out[r][k].value) {
                        (Some(a), Some(b)) => ObservableValue {
                            id,
                            value: Some(-(b - a) / ((rows[r].t_obs - rows[p].t_obs) as f64 * 1e-6)),
                            norm: None,
                            evidence: with(out[r][k].evidence.clone(), Provenance::Derived),
                            fresh: true,
                            reasons: vec![],
                        },
                        _ => empty(&id, missing("knee")),
                    },
                    None => empty(&id, missing("knee")),
                };
            }
        }
        rows.iter()
            .zip(out)
            .map(|(r, values)| ObservableFrame {
                t_us: r.t_obs,
                values: values
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| published(v, i))
                    .collect(),
            })
            .collect()
    }
}

/// Zero-phase filter over each contiguous run of available samples.
fn filter_runs(rows: &[&Row], series: &[Option<f64>], cutoff_hz: f64) -> Vec<Option<f64>> {
    let mut out = vec![None; series.len()];
    let mut i = 0;
    while i < series.len() {
        if series[i].is_none() {
            i += 1;
            continue;
        }
        let start = i;
        while i < series.len() && series[i].is_some() {
            i += 1;
        }
        let run: Vec<f64> = series[start..i].iter().flatten().copied().collect();
        let span = (rows[i - 1].t_obs - rows[start].t_obs) as f64 * 1e-6;
        let rate = if run.len() > 1 && span > 0.0 {
            (run.len() - 1) as f64 / span
        } else {
            0.0
        };
        for (k, y) in filtfilt(&run, cutoff_hz, rate).into_iter().enumerate() {
            out[start + k] = Some(y);
        }
    }
    out
}

/// `x` shifted by whole turns to lie within π of `near`.
pub fn unwrap_near(x: f64, near: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    x + ((near - x) / tau).round() * tau
}

#[cfg(test)]
mod tests;
