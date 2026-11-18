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
        planar_view: None,
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
        "needs_calibrated_cameras"
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

/// Frames at `hz` where `id` carries only its normalised form from one camera.
fn planar_frames(id: &str, hz: u64, conf: f32) -> Vec<ObservableFrame> {
    let i = index(id).unwrap();
    (0..=hz)
        .map(|k| {
            let t = k * 1_000_000 / hz;
            let y = 0.2 + (t as f64 * 1e-6 - 0.5).abs().mul_add(-1.0, 0.5);
            ObservableFrame {
                t_us: t,
                values: ids()
                    .iter()
                    .enumerate()
                    .map(|(j, (name, _))| ObservableValue {
                        id: name.clone(),
                        value: None,
                        norm: (j == i).then_some(y),
                        evidence: if j == i {
                            ev(MethodId::ImagePlane, conf)
                        } else {
                            unavailable()
                        },
                        fresh: true,
                        reasons: vec![],
                    })
                    .collect(),
            }
        })
        .collect()
}

fn planar_window(frames: &[ObservableFrame], view: PlanarView) -> RepWindow<'_> {
    let mut w = window(frames);
    w.planar_view = Some(view);
    w
}

fn codes(r: &MetricMeasurement) -> Vec<&str> {
    r.reasons.iter().map(|r| r.code.as_str()).collect()
}

/// 05 §10.6 and 22 §3 at profile `basic`: heights B normalised only; durations B at 60 fps and
/// C (indicative) at 30 fps; speeds and methods without an image-plane rule suppressed.
#[test]
fn image_plane_metrics_grade_as_basic_allows() {
    let gated = format!(
        "{HEIGHT}  image_plane: {{ views: [frontal, lateral, oblique], use: norm }}
"
    );
    let m = metric(&gated);
    let fr = planar_frames("ankle_height.left", 60, 0.8);
    let r = measure(
        &m,
        &planar_window(&fr, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!(r.grade, Grade::B, "{r:?}");
    assert!(r.value.is_none(), "{r:?}");
    assert!((r.norm.unwrap() - 0.7).abs() < 1e-9, "{r:?}");
    assert_eq!(codes(&r), ["normalised_only", "calibration_missing"]);
    assert_eq!(r.reasons[1].message, "no floor calibration");
    // Low confidence: indicative.
    let low = planar_frames("ankle_height.left", 60, 0.2);
    let r = measure(
        &m,
        &planar_window(&low, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!(r.grade, Grade::C);
    // Without the image-plane gate a height is suppressed.
    let r = measure(
        &metric(HEIGHT),
        &planar_window(&fr, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!(
        (r.grade, codes(&r)),
        (Grade::X, vec!["needs_calibrated_cameras"])
    );
    // A gate listing views suppresses the others.
    let side_only = format!(
        "{HEIGHT}  image_plane: {{ views: [lateral], use: norm }}
"
    );
    let r = measure(
        &metric(&side_only),
        &planar_window(&fr, PlanarView::Frontal),
        &ctx(true),
        &lengths(),
    );
    assert_eq!((r.grade, codes(&r)), (Grade::X, vec!["view_unsuitable"]));

    let duration = "id: kick.extension_time
version: 1
label: e
kind: duration
from: \"phase_start:extension\"
to: strike
unit: ms
direction: lower_is_more
gates:
  min_rate_hz: 30
  rate_for_a_hz: 100
  grade_a_methods: [triangulated]
  grade_b_methods: [fk, image_plane]
";
    let m = metric(duration);
    let r = measure(
        &m,
        &planar_window(&fr, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!((r.grade, r.value), (Grade::B, Some(100.0)), "{r:?}");
    let fr30 = planar_frames("ankle_height.left", 30, 0.8);
    let r = measure(
        &m,
        &planar_window(&fr30, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!(
        (r.grade, codes(&r)),
        (Grade::C, vec!["camera_rate"]),
        "{r:?}"
    );
    assert_eq!(r.reasons[0].message, "camera rate 30 fps");

    let speed = "id: kick.peak_ankle_speed
version: 1
label: s
kind: extremum
obs: ankle_speed.active
agg: max
window: rep
unit: m_s
direction: higher_is_more
gates:
  min_rate_hz: 50
  rate_for_a_hz: 100
  calibration: [camera_intrinsics, camera_extrinsics]
  grade_a_methods: [triangulated]
  grade_b_methods: [fk]
";
    let m = metric(speed);
    let sp30 = planar_frames("ankle_speed.left", 30, 0.8);
    let r = measure(
        &m,
        &planar_window(&sp30, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!((r.grade, codes(&r)), (Grade::X, vec!["rate_too_low"]));
    assert_eq!(r.reasons[0].message, "sampling rate insufficient");
    let sp60 = planar_frames("ankle_speed.left", 60, 0.8);
    let r = measure(
        &m,
        &planar_window(&sp60, PlanarView::Lateral),
        &ctx(true),
        &lengths(),
    );
    assert_eq!(
        (r.grade, codes(&r)),
        (Grade::X, vec!["needs_calibrated_cameras"])
    );
    assert!(r.value.is_none() && r.norm.is_none());
}
