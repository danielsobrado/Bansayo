#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::content::compile::compile_metric;
use crate::content::types::MetricDef;
use crate::observables::{ids, index};
use bansayo_contracts::{ObservableValue, Side, Stance};

fn lengths() -> Lengths {
    Lengths {
        stature: 1.75,
        leg_length: 0.86,
        arm_length: 0.58,
        shoulder_width: 0.4,
        foot_length: 0.27,
    }
}

fn ev(method: MethodId, conf: f32) -> Evidence {
    Evidence {
        provenance: Provenance::Derived,
        method,
        confidence: Some(conf),
        ..unavailable()
    }
}

/// 60 Hz frames over 1 s where `id` follows `f(t_s)` with the given evidence.
fn frames(id: &str, f: impl Fn(f64) -> f64, e: Evidence) -> Vec<ObservableFrame> {
    let i = index(id).unwrap();
    (0..=60)
        .map(|k| {
            let t = k * 1_000_000 / 60;
            ObservableFrame {
                t_us: t,
                values: ids()
                    .iter()
                    .enumerate()
                    .map(|(j, (name, _))| ObservableValue {
                        id: name.clone(),
                        value: (j == i).then(|| f(t as f64 * 1e-6)),
                        norm: (j == i).then(|| f(t as f64 * 1e-6) / 0.86),
                        evidence: if j == i { e.clone() } else { unavailable() },
                        fresh: true,
                        reasons: vec![],
                    })
                    .collect(),
            }
        })
        .collect()
}

fn ctx(live: bool) -> GradeContext {
    GradeContext {
        calibration: [
            (CalibrationKind::CameraIntrinsics, CalibrationState::Valid),
            (CalibrationKind::CameraExtrinsics, CalibrationState::Valid),
            (CalibrationKind::FloorPlane, CalibrationState::Valid),
        ]
        .into(),
        qa: Qa::Pass,
        origin_live: live,
        detector_id: "det-a".into(),
        proportions_valid: false,
        max_gap_us: 80_000,
    }
}

fn metric(yaml: &str) -> CMetric {
    let d: MetricDef = serde_saphyr::from_str(yaml).unwrap();
    compile_metric(&d, Side::Left, Some(Stance::Orthodox), true)
}

const HEIGHT: &str = "id: kick.peak_ankle_height\nversion: 1\nlabel: h\nkind: extremum\nobs: ankle_height.active\nagg: max\nwindow: rep\nunit: m\nnorm: { by: leg_length }\ndirection: higher_is_more\ngates:\n  min_rate_hz: 25\n  calibration: [camera_intrinsics, camera_extrinsics, floor_plane]\n  grade_a_methods: [triangulated]\n  grade_b_methods: [fk]\n";

fn window(frames: &[ObservableFrame]) -> RepWindow<'_> {
    RepWindow {
        frames,
        rep_start: 100_000,
        rep_end: 900_000,
        phases: vec![
            ("chamber".into(), 100_000, 400_000),
            ("extension".into(), 400_000, 900_000),
        ],
        strike: Some(500_000),
        event_inputs: vec![crate::content::compile::resolve(
            "ankle_height.active",
            None,
            Side::Left,
            None,
            true,
        )],
    }
}

#[test]
fn extremum_value_and_norm_with_grades() {
    let m = metric(HEIGHT);
    let fr = frames(
        "ankle_height.left",
        |t| 0.1 + (t - 0.5).abs().mul_add(-1.0, 0.5),
        ev(MethodId::Triangulated, 0.9),
    );
    let r = measure(&m, &window(&fr), &ctx(true), &lengths());
    assert_eq!(r.grade, Grade::A);
    assert!((r.value.unwrap() - 0.6).abs() < 1e-9);
    assert!((r.norm.unwrap() - 0.6 / 0.86).abs() < 1e-9);
    assert_eq!(r.metric, "kick.peak_ankle_height@1");
    // Not live: at most B. Grade-B method: B. Low confidence: C.
    assert_eq!(
        measure(&m, &window(&fr), &ctx(false), &lengths()).grade,
        Grade::B
    );
    let fk = frames("ankle_height.left", |_| 0.1, ev(MethodId::Fk, 0.9));
    assert_eq!(
        measure(&m, &window(&fk), &ctx(true), &lengths()).grade,
        Grade::B
    );
    let weak = frames("ankle_height.left", |_| 0.1, ev(MethodId::Fk, 0.2));
    assert_eq!(
        measure(&m, &window(&weak), &ctx(true), &lengths()).grade,
        Grade::C
    );
}

#[test]
fn every_suppression_gate_yields_x_with_a_reason() {
    let m = metric(HEIGHT);
    let fr = frames(
        "ankle_height.left",
        |_| 0.1,
        ev(MethodId::Triangulated, 0.9),
    );
    let code = |r: MetricMeasurement| {
        assert_eq!(r.grade, Grade::X, "{r:?}");
        assert_eq!(r.value, None);
        r.reasons[0].code.clone()
    };
    let mut c = ctx(true);
    c.calibration.remove(&CalibrationKind::FloorPlane);
    assert_eq!(
        code(measure(&m, &window(&fr), &c, &lengths())),
        "calibration_missing"
    );
    let img = frames("ankle_height.left", |_| 0.1, ev(MethodId::ImagePlane, 0.9));
    assert_eq!(
        code(measure(&m, &window(&img), &ctx(true), &lengths())),
        "method_not_accepted"
    );
    let slow: Vec<ObservableFrame> = fr.iter().step_by(4).cloned().collect();
    assert_eq!(
        code(measure(&m, &window(&slow), &ctx(true), &lengths())),
        "rate_too_low"
    );
    let mut gap = fr.clone();
    for f in gap
        .iter_mut()
        .filter(|f| (300_000..450_000).contains(&f.t_us))
    {
        f.values[index("ankle_height.left").unwrap()].value = None;
    }
    assert_eq!(
        code(measure(&m, &window(&gap), &ctx(true), &lengths())),
        "input_gap"
    );
    let inferred = frames(
        "ankle_height.left",
        |_| 0.1,
        Evidence {
            provenance: Provenance::Inferred,
            ..ev(MethodId::Fk, 0.9)
        },
    );
    assert_eq!(
        code(measure(&m, &window(&inferred), &ctx(true), &lengths())),
        "model_fill"
    );
    let lead = metric(
        "id: kick.guard_at_strike\nversion: 1\nlabel: g\nkind: value_at\nobs: guard_distance.lead\nat: strike\nunit: m\ndirection: lower_is_more\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: [fk]\n",
    );
    let unresolved = compile_metric(&lead.def, Side::Left, None, true);
    assert_eq!(
        code(measure(&unresolved, &window(&fr), &ctx(true), &lengths())),
        "role_unresolved"
    );
    let settle = metric(
        "id: kick.recovery_settle_time\nversion: 1\nlabel: s\nkind: settle_time\nobs: root_speed\nbelow: 0.15\nfor_ms: 300\nfrom: \"phase_start:recovery\"\nunit: ms\ndirection: lower_is_more\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: []\n",
    );
    assert_eq!(
        code(measure(&settle, &window(&fr), &ctx(true), &lengths())),
        "metric_kind_not_implemented"
    );
    let pivot = metric(
        "id: kick.support_pivot\nversion: 1\nlabel: p\nkind: range\nobs: ankle_height.active\nwindow: rep\nunit: m\ndirection: neutral\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: []\n  suppress_when:\n    - { obs: ankle_height.active, op: gt, value: 0.08, unit: m, hold_ms: 150 }\n",
    );
    assert_eq!(
        code(measure(&pivot, &window(&fr), &ctx(true), &lengths())),
        "suppressed_condition"
    );
}

#[test]
fn value_at_range_duration_and_qa_cap() {
    let fr = frames("ankle_height.left", |t| t, ev(MethodId::Triangulated, 0.9));
    let at = metric(
        "id: a.v\nversion: 1\nlabel: v\nkind: value_at\nobs: ankle_height.active\nat: strike\nunit: m\ndirection: neutral\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: []\n",
    );
    let r = measure(&at, &window(&fr), &ctx(true), &lengths());
    assert!((r.value.unwrap() - 0.5).abs() < 1e-9, "{r:?}");
    let range = metric(
        "id: a.r\nversion: 1\nlabel: r\nkind: range\nobs: ankle_height.active\nwindow: { phase: chamber }\nunit: m\ndirection: neutral\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: []\n",
    );
    let r = measure(&range, &window(&fr), &ctx(true), &lengths());
    assert!((r.value.unwrap() - 0.3).abs() < 0.02, "{r:?}");
    let dur = metric(
        "id: a.d\nversion: 1\nlabel: d\nkind: duration\nfrom: \"phase_start:extension\"\nto: strike\nunit: ms\ndirection: neutral\ngates:\n  grade_a_methods: [triangulated]\n  grade_b_methods: []\n",
    );
    let r = measure(&dur, &window(&fr), &ctx(true), &lengths());
    assert_eq!(r.value, Some(100.0));
    assert_eq!(r.grade, Grade::A);
    let mut c = ctx(true);
    c.qa = Qa::NotRun;
    assert_eq!(
        measure(&metric(HEIGHT), &window(&fr), &c, &lengths()).grade,
        Grade::B,
        "C4"
    );
    assert_eq!(
        measure(&dur, &window(&fr), &c, &lengths()).grade,
        Grade::A,
        "no calibration gate"
    );
}

#[test]
fn comparability_class_separates_methods_detectors_and_versions() {
    let a = comparability_class("m", 1, MethodId::Fk, "det-a", "raw");
    assert_eq!(a.len(), 16);
    assert_eq!(
        a,
        comparability_class("m", 1, MethodId::SkeletonFitAxis, "det-a", "raw")
    );
    assert_ne!(a, comparability_class("m", 1, MethodId::Fk, "det-b", "raw"));
    assert_ne!(a, comparability_class("m", 2, MethodId::Fk, "det-a", "raw"));
    assert_ne!(
        a,
        comparability_class("m", 1, MethodId::Triangulated, "det-a", "raw")
    );
    let i = comparability_class("m", 1, MethodId::ImuMounted, "det-a", "raw");
    assert_eq!(
        i,
        comparability_class("m", 1, MethodId::ImuGyro, "det-b", "raw"),
        "detector ignored for inertial"
    );
}
