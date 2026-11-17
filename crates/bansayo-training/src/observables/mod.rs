//! Observable catalogue `bansayo.observables.v1` (04 §6): the observables the kick definitions,
//! the kick metric pack and the segmenter reference, world realisations only. Image-plane
//! realisations need `PlanarBody`, which roadmap Phase 2 builds; until then they report
//! `image_plane_not_implemented`.
pub mod angles;
pub mod filter;

use crate::content::types::NormBy;
use crate::evidence::{combine, reason, unavailable, usable};
use bansayo_contracts::{
    BODY_KEYPOINT_NAMES, BodyState, Evidence, MotionSample, ObservableValue, Provenance, Reason,
    SkeletonDefinition,
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
        Ok(Self {
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

/// Every non-rate observable and every speed point of one body, raw.
fn raw(body: Option<&BodyState>, g: &Geometry) -> (Vec<Option<Value>>, Vec<Point>) {
    fn none<T>() -> Result<T, Reason> {
        Err(reason(
            "no_world_body",
            None,
            "No world skeleton is available; image-plane realisations are not implemented yet.",
        ))
    }
    let values = ids()
        .iter()
        .map(|(id, i)| {
            let e = &ENTRIES[*i];
            if e.class == Rate {
                return None;
            }
            let s = usize::from(id.ends_with(".right"));
            Some(body.map_or_else(none, |b| base(b, g, e.name, s)))
        })
        .collect();
    let points = speed_points()
        .iter()
        .map(|(_, k)| body.map_or_else(none, |b| point(b, k)))
        .collect();
    (values, points)
}

#[derive(Debug, Clone)]
struct Row {
    t_obs: u64,
    values: Vec<Option<Value>>,
    points: Vec<Point>,
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

    /// Advances by one solver tick and returns every observable (latest held between inputs).
    pub fn step(&mut self, sample: &MotionSample) -> ObservableFrame {
        let (values, points) = raw(sample.body.as_ref(), &self.geometry);
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
            values: self.held.clone(),
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
                values,
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
