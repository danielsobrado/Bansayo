//! Component scores against stated targets (07 §7, `bansayo.scoring/v1`).
use crate::content::types::{
    ComponentDef, ComponentSource, PersonalStat, ScoreDirection, ScoringModel, SourceKind,
    TargetSource, UseForm,
};
use crate::evidence::reason;
use bansayo_contracts::{ComponentScore, MetricMeasurement, ScoreResult, ScoreTarget, TargetFrom};
use std::collections::BTreeMap;

/// The athlete's grade-A history of one metric in one class (12 §6), by use form.
#[derive(Debug, Clone, PartialEq)]
pub struct Personal {
    /// Session values of the most recent sessions, oldest first.
    pub session_values: Vec<f64>,
    pub best: Option<f64>,
}

/// Personal statistics keyed by `(metric id, comparability class, use form is norm)`.
pub type PersonalTargets = BTreeMap<(String, String, bool), Personal>;

/// Minimum sessions before a personal target exists (07 §7.3).
pub const MIN_TARGET_SESSIONS: usize = 3;

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Linear tolerance mapping: ≤ inner → 1, ≥ outer → 0.
pub fn tolerance_score(deviation: f64, inner: f64, outer: f64) -> f64 {
    if deviation <= inner {
        1.0
    } else if deviation >= outer {
        0.0
    } else {
        1.0 - (deviation - inner) / (outer - inner)
    }
}

fn metric_id(r: &str) -> &str {
    r.split_once('@').map_or(r, |(id, _)| id)
}

fn component(
    c: &ComponentDef,
    completed: bool,
    measurements: &[MetricMeasurement],
    personal: &PersonalTargets,
    tolerance_scale: f64,
) -> ComponentScore {
    let null = |code: &str, msg: &str, m: Option<MetricMeasurement>| ComponentScore {
        id: c.id.clone(),
        score: None,
        measurement: m,
        target: None,
        deviation: None,
        reasons: vec![reason(code, Some(&c.id), msg)],
    };
    let (metric, use_form) = match &c.source {
        ComponentSource::Kind {
            kind: SourceKind::Outcome,
        } => {
            return ComponentScore {
                id: c.id.clone(),
                score: Some(if completed { 1.0 } else { 0.0 }),
                measurement: None,
                target: None,
                deviation: None,
                reasons: vec![],
            };
        }
        ComponentSource::Kind {
            kind: SourceKind::StepResults,
        } => {
            return null(
                "not_applicable",
                "Step results exist only for sequences.",
                None,
            );
        }
        ComponentSource::Metric { metric, use_form } => (metric, *use_form),
    };
    let Some(m) = measurements
        .iter()
        .find(|m| metric_id(&m.metric) == metric)
        .cloned()
    else {
        return null(
            "measurement_missing",
            "This technique does not measure it.",
            None,
        );
    };
    if c.min_grade.is_some_and(|g| m.grade > g) {
        return null(
            "measurement_quality",
            "The measurement is not reliable enough to score.",
            Some(m),
        );
    }
    let measured = match use_form {
        UseForm::Norm => m.norm,
        UseForm::Value => m.value,
    };
    let Some(measured) = measured else {
        return null("measurement_quality", "No value was measured.", Some(m));
    };
    let target = match &c.target {
        Some(TargetSource::Definition { value, .. }) => Some(ScoreTarget {
            value: *value,
            from: TargetFrom::Definition,
            ref_id: None,
        }),
        Some(TargetSource::Personal {
            stat,
            window_sessions,
        }) => personal
            .get(&(
                metric.clone(),
                m.comparability_class.clone(),
                use_form == UseForm::Norm,
            ))
            .filter(|p| p.session_values.len() >= MIN_TARGET_SESSIONS)
            .and_then(|p| {
                let n = p.session_values.len();
                let mut recent =
                    p.session_values[n.saturating_sub(*window_sessions as usize)..].to_vec();
                match stat {
                    PersonalStat::Median => median(&mut recent),
                    PersonalStat::Best => p.best,
                }
            })
            .map(|value| ScoreTarget {
                value,
                from: TargetFrom::Personal,
                ref_id: None,
            }),
        // References are roadmap Phase 3; no reference can be selected yet.
        Some(TargetSource::Reference) | None => None,
    };
    let Some(target) = target else {
        return null(
            "no_target",
            "Not assessed: no target is available yet.",
            Some(m),
        );
    };
    let deviation = measured - target.value;
    let miss = match c.direction.unwrap_or(ScoreDirection::Match) {
        ScoreDirection::AtLeast => (-deviation).max(0.0),
        ScoreDirection::AtMost => deviation.max(0.0),
        ScoreDirection::Match => deviation.abs(),
    };
    let tol = c
        .tolerance
        .clone()
        .unwrap_or(crate::content::types::Tolerance {
            inner: 0.0,
            outer: f64::INFINITY,
        });
    ComponentScore {
        id: c.id.clone(),
        score: Some(tolerance_score(
            miss,
            tol.inner * tolerance_scale,
            tol.outer * tolerance_scale,
        )),
        measurement: Some(m),
        target: Some(target),
        deviation: Some(deviation),
        reasons: vec![],
    }
}

pub fn score(
    model: &ScoringModel,
    completed: bool,
    measurements: &[MetricMeasurement],
    personal: &PersonalTargets,
    tolerance_scale: f64,
) -> ScoreResult {
    let components: Vec<ComponentScore> = model
        .components
        .iter()
        .map(|c| component(c, completed, measurements, personal, tolerance_scale))
        .collect();
    let total: f64 = model.components.iter().map(|c| c.weight).sum();
    let mut available = 0.0;
    let mut weighted = 0.0;
    for (d, c) in model.components.iter().zip(&components) {
        if let Some(s) = c.score
            && d.weight > 0.0
        {
            available += d.weight;
            weighted += d.weight * s;
        }
    }
    let coverage = if total > 0.0 { available / total } else { 0.0 };
    let required_ok = model
        .components
        .iter()
        .zip(&components)
        .all(|(d, c)| !d.required || c.score.is_some_and(|s| d.weight > 0.0 || s > 0.0));
    let reference_match =
        (required_ok && coverage >= model.composite.min_coverage && available > 0.0)
            .then(|| weighted / available);
    let mut key = format!("{}@{}", model.id, model.version);
    for c in &components {
        let from = c.target.as_ref().map_or("-".to_owned(), |t| {
            format!("{:?}:{}", t.from, t.ref_id.clone().unwrap_or_default())
        });
        let class = c
            .measurement
            .as_ref()
            .map_or("-", |m| m.comparability_class.as_str());
        key.push_str(&format!("|{}:{from}:{class}", c.id));
    }
    ScoreResult {
        model: format!("{}@{}", model.id, model.version),
        components,
        reference_match,
        coverage,
        comparability_class: crate::hash::sha256(key.as_bytes())[..16].to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bansayo_contracts::{Evidence, Grade, MethodId, Provenance};

    fn model() -> ScoringModel {
        serde_saphyr::from_str(
            "schema: bansayo.scoring/v1\nid: scoring.t\nversion: 1\nnames: { display: t }\napplies_to: { families: [kick.front] }\ncomponents:\n  - { id: completion, label: c, source: { kind: outcome }, required: true, weight: 0 }\n  - { id: h, label: h, source: { metric: m.h, use: norm }, target: { from: personal, stat: median, window_sessions: 5 }, direction: at_least, tolerance: { inner: 0.03, outer: 0.15 }, min_grade: B, weight: 2 }\n  - { id: g, label: g, source: { metric: m.g, use: value }, target: { from: definition, value: 0.6, rationale: r }, direction: at_most, tolerance: { inner: 0.05, outer: 0.4 }, min_grade: A, weight: 1 }\n  - { id: p, label: p, source: { metric: m.p, use: value }, target: { from: reference }, direction: at_least, tolerance: { inner: 0.1, outer: 0.5 }, weight: 1 }\ncomposite: { id: reference_match, min_coverage: 0.7 }\n",
        )
        .unwrap()
    }

    fn m(id: &str, v: f64, grade: Grade) -> MetricMeasurement {
        MetricMeasurement {
            metric: format!("{id}@1"),
            value: Some(v),
            norm: Some(v),
            unit: "m".into(),
            grade,
            evidence: Evidence {
                provenance: Provenance::Derived,
                method: MethodId::Fk,
                ..crate::evidence::unavailable()
            },
            comparability_class: "c1".into(),
            reasons: vec![],
        }
    }

    fn personal(n: usize) -> PersonalTargets {
        [(
            ("m.h".to_owned(), "c1".to_owned(), true),
            Personal {
                session_values: vec![1.0; n],
                best: Some(1.1),
            },
        )]
        .into()
    }

    #[test]
    fn tolerance_mapping_is_monotonic() {
        let mut last = 1.0;
        for k in 0..100 {
            let s = tolerance_score(k as f64 * 0.01, 0.1, 0.5);
            assert!(s <= last && (0.0..=1.0).contains(&s));
            last = s;
        }
        assert_eq!(tolerance_score(0.1, 0.1, 0.5), 1.0);
        assert_eq!(tolerance_score(0.5, 0.1, 0.5), 0.0);
        assert!((tolerance_score(0.3, 0.1, 0.5) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn null_components_carry_reasons_and_never_zero() {
        let ms = [
            m("m.h", 0.95, Grade::B),
            m("m.g", 0.7, Grade::B),
            m("m.p", 1.0, Grade::A),
        ];
        let r = score(&model(), true, &ms, &personal(2), 1.0);
        let code = |id: &str| {
            let c = r.components.iter().find(|c| c.id == id).unwrap();
            assert_eq!(c.score, None);
            c.reasons[0].code.clone()
        };
        assert_eq!(code("h"), "no_target");
        assert_eq!(code("g"), "measurement_quality");
        assert_eq!(code("p"), "no_target");
        assert_eq!(r.reference_match, None);
        assert_eq!(r.coverage, 0.0);
    }

    #[test]
    fn composite_renormalises_and_respects_coverage() {
        let ms = [m("m.h", 0.95, Grade::A), m("m.g", 0.7, Grade::A)];
        let r = score(&model(), true, &ms, &personal(3), 1.0);
        let h = r.components.iter().find(|c| c.id == "h").unwrap();
        // Shortfall 0.05 against a median of 1.0: 1 − (0.05 − 0.03)/0.12.
        assert!((h.score.unwrap() - (1.0 - 0.02 / 0.12)).abs() < 1e-9);
        assert!((h.deviation.unwrap() + 0.05).abs() < 1e-9);
        assert!((r.coverage - 0.75).abs() < 1e-12);
        let g = r
            .components
            .iter()
            .find(|c| c.id == "g")
            .unwrap()
            .score
            .unwrap();
        let expect = (2.0 * h.score.unwrap() + g) / 3.0;
        assert!((r.reference_match.unwrap() - expect).abs() < 1e-12);
        // Below minimum coverage there is no composite; an incomplete repetition gates it.
        let r = score(&model(), true, &ms[..1], &personal(3), 1.0);
        assert_eq!(r.reference_match, None);
        let r = score(&model(), false, &ms, &personal(3), 1.0);
        assert_eq!(r.reference_match, None);
        // A wider tolerance scale raises the score.
        let wide = score(&model(), true, &ms, &personal(3), 1.5);
        assert!(wide.components[1].score.unwrap() > h.score.unwrap());
    }
}
