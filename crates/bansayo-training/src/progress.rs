//! Progress analytics as pure functions over stored rows (12 §6). Every quantity is computed
//! inside one comparability class (C1); classes are never joined (C2).
use bansayo_contracts::{
    Baseline, Best, ChangeClass, Classification, Grade, GradeMix, PointBasis, SeriesBreak,
    SeriesClass, SeriesPoint, Trend,
};
use std::collections::BTreeMap;

/// One stored measurement with the session context it is joined to.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub session: SessionInfo,
    pub rep_index: Option<u32>,
    pub value: f64,
    pub norm: Option<f64>,
    pub grade: Grade,
    pub sigma: Option<f64>,
    pub method_class: String,
    pub detector_id: String,
    pub comparability_class: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub point: SeriesPoint,
    /// Days since an epoch, for slopes and windows.
    pub day: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressConfig {
    pub baseline_sessions: usize,
    pub baseline_window_days: f64,
    pub trend_sessions: usize,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            baseline_sessions: 3,
            baseline_window_days: 30.0,
            trend_sessions: 8,
        }
    }
}

pub fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    Some(if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    })
}

fn quantile(s: &[f64], q: f64) -> f64 {
    let pos = q * (s.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
}

pub fn iqr(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    quantile(&s, 0.75) - quantile(&s, 0.25)
}

fn mad(v: &[f64]) -> f64 {
    let Some(m) = median(v) else { return 0.0 };
    median(&v.iter().map(|x| (x - m).abs()).collect::<Vec<_>>()).unwrap_or(0.0)
}

/// Theil–Sen slope of `(x, y)`: the median of pairwise slopes.
pub fn theil_sen(points: &[(f64, f64)]) -> Option<f64> {
    let mut slopes = vec![];
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let dx = points[j].0 - points[i].0;
            if dx.abs() > 1e-12 {
                slopes.push((points[j].1 - points[i].1) / dx);
            }
        }
    }
    median(&slopes)
}

/// Minimum values of one grade for a session point (12 §6).
pub const MIN_SESSION_VALUES: usize = 3;

/// Session points of one class: grade-A medians where ≥ 3 exist, otherwise a secondary point
/// from grade-B values.
pub fn session_points(rows: &[&Row]) -> Vec<SeriesPoint> {
    let mut by: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by.entry(r.session.point.session_id.to_string())
            .or_default()
            .push(r);
    }
    let mut out: Vec<(f64, SeriesPoint)> = vec![];
    for rs in by.values() {
        let mut mix = GradeMix::default();
        for r in rs {
            match r.grade {
                Grade::A => mix.a += 1,
                Grade::B => mix.b += 1,
                Grade::C => mix.c += 1,
                Grade::X => mix.x += 1,
            }
        }
        let pick = |g: Grade| {
            rs.iter()
                .filter(|r| r.grade == g)
                .copied()
                .collect::<Vec<_>>()
        };
        let (basis, chosen) = match (pick(Grade::A), pick(Grade::B)) {
            (a, _) if a.len() >= MIN_SESSION_VALUES => (PointBasis::A, a),
            (_, b) if b.len() >= MIN_SESSION_VALUES => (PointBasis::B, b),
            _ => continue,
        };
        let values: Vec<f64> = chosen.iter().map(|r| r.value).collect();
        let norms: Vec<f64> = chosen.iter().filter_map(|r| r.norm).collect();
        let mut p = rs[0].session.point.clone();
        p.value = median(&values).unwrap_or_default();
        p.norm = median(&norms);
        p.spread = iqr(&values);
        p.count = values.len() as u32;
        p.basis = basis;
        p.grade_mix = mix;
        p.comparability_class = rs[0].comparability_class.clone();
        out.push((rs[0].session.day, p));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, p)| p).collect()
}

fn better(direction: &str, a: f64, b: f64) -> bool {
    if direction == "lower_is_more" {
        a < b
    } else {
        a > b
    }
}

/// Every quantity of 12 §6 for one class. `direction` is the metric's.
pub fn class_series(rows: &[&Row], direction: &str, cfg: &ProgressConfig) -> Option<SeriesClass> {
    let first = rows.first()?;
    let points = session_points(rows);
    let days: BTreeMap<String, f64> = rows
        .iter()
        .map(|r| (r.session.point.session_id.to_string(), r.session.day))
        .collect();
    let a: Vec<(f64, &SeriesPoint)> = points
        .iter()
        .filter(|p| p.basis == PointBasis::A)
        .map(|p| (days[p.session_id.as_str()], p))
        .collect();
    let baseline = (a.len() >= cfg.baseline_sessions)
        .then(|| &a[..cfg.baseline_sessions])
        .filter(|w| w.last().map(|l| l.0).unwrap_or(0.0) - w[0].0 <= cfg.baseline_window_days)
        .map(|w| {
            let v: Vec<f64> = w.iter().map(|p| p.1.value).collect();
            Baseline {
                value: median(&v).unwrap_or_default(),
                spread: iqr(&v),
                sessions: w.len() as u32,
                from_utc: w[0].1.date_utc.clone(),
                to_utc: w[w.len() - 1].1.date_utc.clone(),
            }
        });
    let best_rep = rows
        .iter()
        .filter(|r| r.grade == Grade::A)
        .copied()
        .reduce(|x, y| {
            if better(direction, y.value, x.value) {
                y
            } else {
                x
            }
        });
    let best = best_rep.map(|r| Best {
        value: r.value,
        session_id: r.session.point.session_id.clone(),
        rep_index: r.rep_index,
        session_value: a
            .iter()
            .map(|p| p.1.value)
            .reduce(|x, y| if better(direction, y, x) { y } else { x }),
    });
    let recent: Vec<(f64, f64)> = a
        .iter()
        .rev()
        .take(cfg.trend_sessions)
        .rev()
        .map(|p| (p.0 / 7.0, p.1.value))
        .collect();
    let trend = (recent.len() >= 4).then(|| Trend {
        slope_per_week: theil_sen(&recent).unwrap_or_default(),
        sessions: recent.len() as u32,
    });
    let mut breaks = vec![];
    for w in points.windows(2) {
        if w[0].calibration_set_id != w[1].calibration_set_id {
            breaks.push(SeriesBreak {
                session_id: w[1].session_id.clone(),
                reason: "calibration_set_changed".into(),
            });
        }
    }
    let classification = classify(
        &a.iter().map(|p| p.1).collect::<Vec<_>>(),
        baseline.as_ref(),
        rows,
        &breaks,
    );
    Some(SeriesClass {
        comparability_class: first.comparability_class.clone(),
        method_class: first.method_class.clone(),
        detector_id: first.detector_id.clone(),
        points,
        baseline,
        best,
        trend,
        classification,
        breaks,
    })
}

/// Change classification of 12 §6 with the recalibration flag of C3.
pub fn classify(
    a: &[&SeriesPoint],
    baseline: Option<&Baseline>,
    rows: &[&Row],
    breaks: &[SeriesBreak],
) -> Option<Classification> {
    let n = a.len();
    if n < 4 {
        return None;
    }
    let last: Vec<f64> = a[n.saturating_sub(3)..].iter().map(|p| p.value).collect();
    let before: Vec<f64> = a[n.saturating_sub(6)..n - 3]
        .iter()
        .map(|p| p.value)
        .collect();
    let reference = if before.len() >= 3 {
        median(&before)?
    } else {
        baseline.map(|b| b.value).or_else(|| median(&before))?
    };
    let delta = median(&last)? - reference;
    let sigmas: Vec<f64> = rows
        .iter()
        .filter(|r| r.grade == Grade::A)
        .filter_map(|r| r.sigma)
        .collect();
    let noise = (2.0 * 1.4826 * mad(&before))
        .max(2.0 * median(&sigmas).unwrap_or(0.0))
        .max(f64::EPSILON);
    let mut flags = vec![];
    let pooled = median(&a.iter().map(|p| p.spread).collect::<Vec<_>>()).unwrap_or(0.0);
    for b in breaks {
        if let Some(i) = a.iter().position(|p| p.session_id == b.session_id)
            && i > 0
            && (a[i].value - a[i - 1].value).abs() > 2.0 * pooled
            && i >= n.saturating_sub(3)
        {
            flags.push("coincides_with_recalibration".to_owned());
        }
    }
    let result = if !flags.is_empty() || delta.abs() <= noise {
        ChangeClass::NoClearChange
    } else if delta > 0.0 {
        ChangeClass::Increased
    } else {
        ChangeClass::Decreased
    };
    Some(Classification {
        result,
        delta,
        noise,
        flags,
    })
}

/// Groups rows by class and computes each class's series.
pub fn series(rows: &[Row], direction: &str, cfg: &ProgressConfig) -> Vec<SeriesClass> {
    let mut by: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by.entry(&r.comparability_class).or_default().push(r);
    }
    by.values()
        .filter_map(|rs| class_series(rs, direction, cfg))
        .collect()
}

#[cfg(test)]
mod tests;
