//! Metric kinds, the grade rule (05 §8–9) and comparability classes (12 §5.3).
use crate::content::compile::{CAtom, CMetric, Obs};
use crate::content::types::{Agg, MetricKind, Op, Window, WindowName};
use crate::evidence::{combine, fidelity, reason, unavailable};
use crate::observables::{Lengths, ObservableFrame};
use bansayo_contracts::{
    CalibrationKind, CalibrationState, Evidence, Grade, MethodId, MetricMeasurement, Provenance,
    Reason,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qa {
    Pass,
    Warn,
    Fail,
    NotRun,
}

/// What a grade depends on outside the repetition itself.
#[derive(Debug, Clone)]
pub struct GradeContext {
    pub calibration: BTreeMap<CalibrationKind, CalibrationState>,
    pub qa: Qa,
    /// Origin is `live` for every input (05 §8 grade A).
    pub origin_live: bool,
    /// Pose model identifier for comparability (12 §5.3).
    pub detector_id: String,
    pub proportions_valid: bool,
    pub max_gap_us: u64,
}

/// One analysed repetition: re-filtered frames and its anchors.
#[derive(Debug, Clone)]
pub struct RepWindow<'a> {
    pub frames: &'a [ObservableFrame],
    pub rep_start: u64,
    pub rep_end: u64,
    /// `(phase id, start, end)` in order.
    pub phases: Vec<(String, u64, u64)>,
    pub strike: Option<u64>,
    /// Observables whose evidence stands for phase events (the technique's required ones).
    pub event_inputs: Vec<Obs>,
}

impl RepWindow<'_> {
    pub fn anchor(&self, a: &str) -> Option<u64> {
        match a {
            "rep_start" => Some(self.rep_start),
            "rep_end" => Some(self.rep_end),
            "strike" => self.strike,
            _ => {
                if let Some(p) = a.strip_prefix("phase_start:") {
                    self.phases.iter().find(|x| x.0 == p).map(|x| x.1)
                } else if let Some(p) = a.strip_prefix("phase_end:") {
                    self.phases.iter().find(|x| x.0 == p).map(|x| x.2)
                } else {
                    None
                }
            }
        }
    }

    fn window(&self, w: &Window) -> Result<(u64, u64), Reason> {
        let phase = |p: &str| {
            self.phases
                .iter()
                .find(|x| x.0 == p)
                .ok_or_else(|| reason("phase_not_detected", Some(p), "The phase was not detected."))
        };
        match w {
            Window::Named(WindowName::Rep) => Ok((self.rep_start, self.rep_end)),
            Window::Phase { phase: p } => phase(p).map(|x| (x.1, x.2)),
            Window::Span {
                from_phase,
                to_phase,
            } => Ok((phase(from_phase)?.1, phase(to_phase)?.2)),
        }
    }
}

/// `method_class` of 12 §5.3.
pub fn method_class(m: MethodId) -> &'static str {
    use MethodId::*;
    match m {
        Triangulated | DepthCamera => "optical_3d",
        MarkerCluster => "marker",
        ImuMounted | ImuGyro => "inertial",
        ImuOpticalFused => "fused",
        XrDevice => "xr",
        Fk | SkeletonFitAxis | SkeletonFitPlane => "kinematic",
        ImagePlane => "planar",
        MonocularLift | RestFill | FloorContact => "inferred",
    }
}

/// `hex16(SHA-256(metric_id | metric_version | method_class | detector_id | norm_basis))`.
pub fn comparability_class(
    metric_id: &str,
    version: u32,
    method: MethodId,
    detector_id: &str,
    norm_basis: &str,
) -> String {
    let class = method_class(method);
    let detector = if matches!(class, "optical_3d" | "kinematic" | "planar") {
        detector_id
    } else {
        "-"
    };
    crate::hash::sha256(format!("{metric_id}|{version}|{class}|{detector}|{norm_basis}").as_bytes())
        [..16]
        .to_owned()
}

fn x(
    m: &CMetric,
    value: Option<f64>,
    unit: &str,
    e: Evidence,
    why: Vec<Reason>,
    class: String,
) -> MetricMeasurement {
    MetricMeasurement {
        metric: m.reference.clone(),
        value: if why.is_empty() { value } else { None },
        norm: None,
        unit: unit.into(),
        grade: Grade::X,
        evidence: e,
        comparability_class: class,
        reasons: why,
    }
}

fn sample(f: &ObservableFrame, obs: &Obs) -> Option<(f64, Option<f64>, Evidence)> {
    let v = f.get(obs.index?);
    Some((
        v.value? * obs.sign,
        v.norm.map(|n| n * obs.sign),
        v.evidence.clone(),
    ))
}

/// Linear interpolation of value and norm at `t` between neighbouring samples.
fn interpolate(
    frames: &[&ObservableFrame],
    obs: &Obs,
    t: u64,
) -> Option<(f64, Option<f64>, Evidence)> {
    let after = frames.iter().position(|f| f.t_us >= t)?;
    let b = sample(frames[after], obs)?;
    if frames[after].t_us == t || after == 0 {
        return Some(b);
    }
    let a = sample(frames[after - 1], obs)?;
    let (ta, tb) = (frames[after - 1].t_us as f64, frames[after].t_us as f64);
    let u = (t as f64 - ta) / (tb - ta);
    let lerp = |p: f64, q: f64| p + (q - p) * u;
    Some((
        lerp(a.0, b.0),
        a.1.zip(b.1).map(|(p, q)| lerp(p, q)),
        combine([&a.2, &b.2]).unwrap_or_else(unavailable),
    ))
}

/// Longest stretch inside `[a, b]` with `obs` unavailable, in µs.
fn longest_gap(frames: &[&ObservableFrame], obs: &Obs, a: u64, b: u64) -> u64 {
    let mut gap = 0;
    let mut since: Option<u64> = None;
    for f in frames {
        if sample(f, obs).is_some() {
            if let Some(s) = since.take() {
                gap = gap.max(f.t_us - s);
            }
        } else if since.is_none() {
            since = Some(f.t_us.max(a));
        }
    }
    if let Some(s) = since {
        gap = gap.max(b.saturating_sub(s));
    }
    gap
}

fn rate_hz(frames: &[&ObservableFrame]) -> Option<f64> {
    let mut d: Vec<u64> = frames
        .windows(2)
        .map(|w| w[1].t_us - w[0].t_us)
        .filter(|d| *d > 0)
        .collect();
    if d.is_empty() {
        return None;
    }
    d.sort_unstable();
    Some(1e6 / d[d.len() / 2] as f64)
}

fn atom_holds(a: &CAtom, frames: &[&ObservableFrame]) -> bool {
    let mut since: Option<u64> = None;
    for f in frames {
        let v = a.obs.index.map(|i| f.get(i));
        let x = v
            .and_then(|v| if a.obs.norm { v.norm } else { v.value })
            .map(|x| x * a.obs.sign);
        let truth = x.is_some_and(|x| match a.op {
            Op::Gt => x > a.lo,
            Op::Lt => x < a.lo,
            Op::Between => x >= a.lo && x <= a.hi,
            _ => false,
        });
        if truth {
            let s = *since.get_or_insert(f.t_us);
            if f.t_us - s >= a.hold_us {
                return true;
            }
        } else {
            since = None;
        }
    }
    false
}

/// Computes and grades one metric for one repetition.
pub fn measure(
    m: &CMetric,
    w: &RepWindow,
    g: &GradeContext,
    lengths: &Lengths,
) -> MetricMeasurement {
    let d = &m.def;
    let norm_by = d.norm.as_ref().map(|n| n.by);
    let norm_basis = norm_by.map_or("raw".to_owned(), |n| {
        format!(
            "{}/{}",
            serde_json::to_value(n)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            if g.proportions_valid {
                "valid"
            } else {
                "assumed"
            }
        )
    });
    let class_for = |method: MethodId| {
        comparability_class(&d.id, d.version, method, &g.detector_id, &norm_basis)
    };
    if !d.kind.implemented() {
        return x(
            m,
            None,
            &d.unit,
            unavailable(),
            vec![reason(
                "metric_kind_not_implemented",
                Some(&d.id),
                "This kind of measurement is not computed by this engine version.",
            )],
            class_for(MethodId::RestFill),
        );
    }
    let (a, b, inputs): (u64, u64, Vec<Obs>) = match d.kind {
        MetricKind::Duration => {
            let (from, to) = (
                d.from.as_deref().unwrap_or(""),
                d.to.as_deref().unwrap_or(""),
            );
            match (w.anchor(from), w.anchor(to)) {
                (Some(p), Some(q)) => (p.min(q), p.max(q), w.event_inputs.clone()),
                (p, _) => {
                    let missing = if p.is_none() { from } else { to };
                    return x(
                        m,
                        None,
                        &d.unit,
                        unavailable(),
                        vec![reason(
                            "anchor_not_detected",
                            Some(missing),
                            "The event this measurement starts or ends at was not detected.",
                        )],
                        class_for(MethodId::RestFill),
                    );
                }
            }
        }
        MetricKind::ValueAt => match w.anchor(d.at.as_deref().unwrap_or("")) {
            Some(t) => (t, t, m.obs.clone().into_iter().collect()),
            None => {
                return x(
                    m,
                    None,
                    &d.unit,
                    unavailable(),
                    vec![reason(
                        "anchor_not_detected",
                        d.at.as_deref(),
                        "The event this measurement is taken at was not detected.",
                    )],
                    class_for(MethodId::RestFill),
                );
            }
        },
        _ => match d.window.as_ref().map(|win| w.window(win)) {
            Some(Ok((p, q))) => (p, q, m.obs.clone().into_iter().collect()),
            Some(Err(why)) => {
                return x(
                    m,
                    None,
                    &d.unit,
                    unavailable(),
                    vec![why],
                    class_for(MethodId::RestFill),
                );
            }
            None => (w.rep_start, w.rep_end, m.obs.clone().into_iter().collect()),
        },
    };
    if inputs.iter().any(|o| o.index.is_none()) || (m.obs.is_some() && inputs.is_empty()) {
        return x(
            m,
            None,
            &d.unit,
            unavailable(),
            vec![reason(
                "role_unresolved",
                None,
                "Lead and rear sides are unknown: set a default stance in the athlete profile.",
            )],
            class_for(MethodId::RestFill),
        );
    }
    // Samples from the window, plus the neighbours a point anchor interpolates between.
    let frames: Vec<&ObservableFrame> = {
        let lo = w.frames.iter().rposition(|f| f.t_us <= a).unwrap_or(0);
        let hi = w
            .frames
            .iter()
            .position(|f| f.t_us >= b)
            .unwrap_or(w.frames.len().saturating_sub(1));
        w.frames
            .get(lo..=hi.max(lo))
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    };
    let mut why = vec![];
    for o in &inputs {
        if longest_gap(&frames, o, a, b) > g.max_gap_us {
            why.push(reason(
                "input_gap",
                None,
                "A required input was missing for too long inside the measurement window.",
            ));
            break;
        }
    }
    let rate = rate_hz(&frames);
    let gates = &d.gates;
    if let (Some(min), Some(r)) = (gates.min_rate_hz, rate)
        && r < min * 0.95
    {
        why.push(reason(
            "rate_too_low",
            None,
            &format!("Input rate {r:.0} Hz is below the {min:.0} Hz this measurement needs."),
        ));
    }
    for kind in &gates.calibration {
        let state = g
            .calibration
            .get(kind)
            .copied()
            .unwrap_or(CalibrationState::Missing);
        if state != CalibrationState::Valid {
            let k = serde_json::to_value(kind)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            why.push(reason(
                if state == CalibrationState::Stale {
                    "calibration_stale"
                } else {
                    "calibration_missing"
                },
                Some(&k),
                "A calibration this measurement needs is not valid.",
            ));
        }
    }
    let evidences: Vec<Evidence> = frames
        .iter()
        .filter(|f| f.t_us >= a.saturating_sub(1) && f.t_us <= b + 1 || a == b)
        .flat_map(|f| {
            inputs
                .iter()
                .filter_map(|o| sample(f, o).map(|s| s.2))
                .collect::<Vec<_>>()
        })
        .collect();
    let ev = combine(evidences.iter()).unwrap_or_else(unavailable);
    let methods: Vec<MethodId> = {
        let mut v: Vec<MethodId> = evidences.iter().map(|e| e.method).collect();
        v.sort_by_key(|m| fidelity(*m));
        v.dedup();
        v
    };
    if matches!(
        ev.provenance,
        Provenance::Inferred | Provenance::Unavailable
    ) && why.is_empty()
    {
        why.push(reason(
            "model_fill",
            None,
            "Inputs were filled by a model, not observed.",
        ));
    }
    let accepted =
        |list: &[MethodId]| !methods.is_empty() && methods.iter().all(|m| list.contains(m));
    let ab: Vec<MethodId> = gates
        .grade_a_methods
        .iter()
        .chain(&gates.grade_b_methods)
        .copied()
        .collect();
    if !accepted(&ab) && why.is_empty() {
        why.push(reason(
            "method_not_accepted",
            None,
            "The way this was observed is not accurate enough for this measurement.",
        ));
    }
    let window_frames: Vec<&ObservableFrame> = frames
        .iter()
        .copied()
        .filter(|f| f.t_us >= a && f.t_us <= b)
        .collect();
    if m.suppress_when
        .iter()
        .any(|s| atom_holds(s, &window_frames))
    {
        why.push(reason(
            "suppressed_condition",
            None,
            "A condition under which this measurement is unreliable held.",
        ));
    }
    let class = class_for(ev.method);
    if !why.is_empty() {
        return x(m, None, &d.unit, ev, why, class);
    }
    let obs = m.obs.clone();
    let value: Option<(f64, Option<f64>)> = match d.kind {
        MetricKind::Duration => Some(((b - a) as f64 / 1000.0, None)),
        MetricKind::ValueAt => obs
            .as_ref()
            .and_then(|o| interpolate(&frames, o, a))
            .map(|s| (s.0, s.1)),
        MetricKind::Extremum | MetricKind::Range => {
            let o = obs.clone().unwrap_or(Obs {
                index: None,
                norm: false,
                sign: 1.0,
            });
            let s: Vec<(f64, Option<f64>)> = window_frames
                .iter()
                .filter_map(|f| sample(f, &o))
                .map(|s| (s.0, s.1))
                .collect();
            let max = s.iter().copied().max_by(|p, q| p.0.total_cmp(&q.0));
            let min = s.iter().copied().min_by(|p, q| p.0.total_cmp(&q.0));
            match (d.kind, d.agg) {
                (MetricKind::Extremum, Some(Agg::Min)) => min,
                (MetricKind::Extremum, _) => max,
                _ => max
                    .zip(min)
                    .map(|(p, q)| (p.0 - q.0, p.1.zip(q.1).map(|(p, q)| p - q))),
            }
        }
        _ => None,
    };
    let Some((value, obs_norm)) = value else {
        return x(
            m,
            None,
            &d.unit,
            ev,
            vec![reason(
                "no_samples",
                None,
                "No samples fell inside the measurement window.",
            )],
            class,
        );
    };
    let norm = norm_by.map(|n| obs_norm.unwrap_or(value / lengths.by(n)));
    let min_a = gates
        .grade_a
        .as_ref()
        .and_then(|x| x.min_confidence)
        .unwrap_or(0.7);
    let max_sigma = gates.grade_a.as_ref().and_then(|x| x.max_sigma);
    let min_b = gates
        .grade_b
        .as_ref()
        .and_then(|x| x.min_confidence)
        .unwrap_or(0.4);
    let conf = ev.confidence.map_or(1.0, f64::from);
    let capped = gates
        .rate_for_a_hz
        .zip(rate)
        .is_some_and(|(need, r)| r < need * 0.95)
        || g.qa == Qa::Fail
        || (g.qa == Qa::NotRun && !gates.calibration.is_empty());
    let grade = if accepted(&gates.grade_a_methods)
        && conf >= min_a
        && ev
            .sigma
            .zip(max_sigma)
            .is_none_or(|(s, m)| f64::from(s) <= m)
        && g.origin_live
        && !capped
    {
        Grade::A
    } else if accepted(&ab) && conf >= min_b {
        Grade::B
    } else {
        Grade::C
    };
    let mut reasons = vec![];
    if grade != Grade::A {
        let r = if !g.origin_live {
            reason(
                "origin_not_live",
                None,
                "Simulated or recorded inputs are not graded A.",
            )
        } else if capped {
            reason(
                "capped",
                None,
                "Sampling rate or calibration check limits this measurement.",
            )
        } else {
            reason(
                "estimated",
                None,
                "Estimated from inputs below the highest accuracy class.",
            )
        };
        reasons.push(r);
    }
    MetricMeasurement {
        metric: m.reference.clone(),
        value: Some(value),
        norm,
        unit: d.unit.clone(),
        grade,
        evidence: ev,
        comparability_class: class,
        reasons,
    }
}

#[cfg(test)]
mod tests;
