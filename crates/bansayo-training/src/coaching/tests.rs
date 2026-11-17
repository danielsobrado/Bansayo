#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use bansayo_contracts::{
    ComponentScore, Evidence, MethodId, MetricMeasurement, Provenance, ScoreResult,
};

fn model() -> ScoringModel {
    serde_saphyr::from_str(
        "schema: bansayo.scoring/v1\nid: scoring.t\nversion: 1\nnames: { display: t }\napplies_to: { families: [kick.front] }\ncomponents:\n  - { id: a, label: a, source: { metric: m.a, use: value }, target: { from: definition, value: 1, rationale: r }, direction: at_least, tolerance: { inner: 0.1, outer: 0.5 }, weight: 1 }\n  - { id: b, label: b, source: { metric: m.b, use: value }, target: { from: definition, value: 1, rationale: r }, direction: at_least, tolerance: { inner: 0.1, outer: 0.5 }, weight: 1 }\n  - { id: t, label: t, source: { metric: m.t, use: value }, target: { from: definition, value: 100, rationale: r }, direction: at_most, tolerance: { inner: 10, outer: 50 }, weight: 1 }\ncomposite: { id: reference_match, min_coverage: 0.5 }\n",
    )
    .unwrap()
}

fn cue(id: &str, component: &str, category: &str, priority: u32, min_grade: &str) -> CueDef {
    serde_saphyr::from_str(&format!(
        "id: {id}\ncategory: {category}\nbasis: measurement\nwhen: {{ component: {component}, below: 0.5 }}\npriority: {priority}\nmin_grade: {min_grade}\nobserved: o.{id}\nsuggestion: s.{id}\nfocus: external\n"
    ))
    .unwrap()
}

fn cues() -> Vec<CueDef> {
    vec![
        cue("ca", "a", "technique", 2, "B"),
        cue("cb", "b", "technique", 1, "B"),
        cue("ct", "t", "timing", 1, "B"),
    ]
}

/// A repetition where components a, b, t have the given scores and grades.
fn rep(a: f64, b: f64, grade: Grade) -> ScoreResult {
    let comp = |id: &str, s: f64| ComponentScore {
        id: id.into(),
        score: Some(s),
        measurement: Some(MetricMeasurement {
            metric: format!("m.{id}@1"),
            value: Some(s),
            norm: None,
            unit: "rad".into(),
            grade,
            evidence: Evidence {
                provenance: Provenance::Derived,
                method: MethodId::Fk,
                ..crate::evidence::unavailable()
            },
            comparability_class: "c".into(),
            reasons: vec![],
        }),
        target: None,
        deviation: Some(-(1.0 - s) * 0.5),
        reasons: vec![],
    };
    ScoreResult {
        model: "scoring.t@1".into(),
        components: vec![comp("a", a), comp("b", b), comp("t", 1.0)],
        reference_match: None,
        coverage: 1.0,
        comparability_class: "x".into(),
    }
}

fn run(s: &mut Scheduler, reps: &[ScoreResult]) -> Vec<Option<String>> {
    reps.iter()
        .map(|r| s.after_rep(&cues(), r, &model()).map(|c| c.id))
        .collect()
}

#[test]
fn one_off_deviations_produce_no_cue_and_the_gate_needs_persistence() {
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(5);
    let good = rep(1.0, 1.0, Grade::A);
    let bad = rep(0.2, 1.0, Grade::A);
    assert_eq!(
        run(&mut s, &[good.clone(), bad.clone(), good.clone()]),
        [None, None, None]
    );
    // Two of the last three at grade A.
    assert_eq!(run(&mut s, std::slice::from_ref(&bad)), [Some("ca".into())]);
    // Grade B needs three of the last four.
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(5);
    let bad_b = rep(0.2, 1.0, Grade::B);
    let got = run(&mut s, &[bad_b.clone(), bad_b.clone(), bad_b.clone()]);
    assert_eq!(got, [None, None, Some("ca".into())]);
    // Grade C never.
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(5);
    assert!(
        run(&mut s, &vec![rep(0.2, 1.0, Grade::C); 6])
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn at_most_one_cue_ordered_by_category_priority_then_deviation() {
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(5);
    let both = rep(0.2, 0.3, Grade::A);
    let got = run(&mut s, &[both.clone(), both.clone()]);
    assert_eq!(
        got,
        [None, Some("cb".into())],
        "same category: priority 1 first"
    );
}

#[test]
fn cooldown_and_its_override_on_growing_deviation() {
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(20);
    let bad = rep(0.4, 1.0, Grade::A);
    let got = run(&mut s, &vec![bad.clone(); 6]);
    assert_eq!(
        got,
        [None, Some("ca".into()), None, None, None, Some("ca".into())]
    );
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(20);
    let got = run(
        &mut s,
        &[
            rep(0.4, 1.0, Grade::A),
            rep(0.4, 1.0, Grade::A),
            rep(0.0, 1.0, Grade::A),
        ],
    );
    assert_eq!(
        got[2],
        Some("ca".into()),
        "deviation grew by at least 0.25 of outer"
    );
}

#[test]
fn fading_by_coaching_level() {
    let bad = rep(0.2, 0.2, Grade::A);
    let mut s = Scheduler::new(CoachingLevel::Standard, 0, false);
    s.start_set(10);
    let got = run(&mut s, &vec![bad.clone(); 6]);
    for w in got.windows(2) {
        assert!(
            w[0].is_none() || w[1].is_none(),
            "standard, long set: every second rep at most {got:?}"
        );
    }
    let mut s = Scheduler::new(CoachingLevel::Standard, 0, false);
    s.start_set(5);
    assert!(
        run(&mut s, &vec![bad.clone(); 4])[1..]
            .iter()
            .all(Option::is_some)
    );
    let mut s = Scheduler::new(CoachingLevel::Minimal, 0, false);
    s.start_set(5);
    assert!(
        run(&mut s, &vec![bad.clone(); 6])
            .iter()
            .all(Option::is_none)
    );
    // Stage slow disables timing cues.
    let mut s = Scheduler::new(CoachingLevel::Detailed, 0, true);
    s.start_set(5);
    assert!(!s.disabled(&cues()[0]) && s.disabled(&cues()[2]));
}

#[test]
fn resolution_after_three_reps_within_inner() {
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(20);
    let bad = rep(0.2, 1.0, Grade::A);
    let good = rep(1.0, 1.0, Grade::A);
    let got: Vec<Option<CueInstance>> = [bad.clone(), bad, good.clone(), good.clone(), good]
        .iter()
        .map(|r| s.after_rep(&cues(), r, &model()))
        .collect();
    assert_eq!(got[1].as_ref().unwrap().id, "ca");
    assert!(got[2].is_none() && got[3].is_none());
    assert!(got[4].as_ref().unwrap().resolved);
}

#[test]
fn set_summary_limits() {
    let mut s = Scheduler::new(CoachingLevel::Detailed, 3, false);
    s.start_set(5);
    run(&mut s, &vec![rep(0.4, 0.4, Grade::A); 5]);
    s.start_set(5);
    let mut more = cues();
    more.push(cue("cx", "a", "balance", 1, "B"));
    for r in vec![rep(0.45, 0.45, Grade::A); 5] {
        s.after_rep(&more, &r, &model());
    }
    let sum = s.after_set(&more, Some(&model()));
    let corrective = sum
        .iter()
        .filter(|c| !c.id.starts_with("improved."))
        .count();
    let positive = sum.iter().filter(|c| c.id.starts_with("improved.")).count();
    assert_eq!(corrective, 2);
    assert_eq!(sum[0].id, "cx", "balance before technique");
    assert_eq!(positive, 1);
}
