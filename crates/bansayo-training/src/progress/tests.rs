#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use bansayo_contracts::{CapabilityProfileId, Origin, SessionId};

fn session(k: u32, calib: &str) -> SessionInfo {
    SessionInfo {
        point: SeriesPoint {
            session_id: SessionId::new(format!("202611{:02}T100000Z-a{:03}", 1 + k % 28, k))
                .unwrap(),
            date_utc: format!("2026-11-{:02}", 1 + k % 28),
            value: 0.0,
            norm: None,
            spread: 0.0,
            count: 0,
            basis: PointBasis::A,
            grade_mix: GradeMix::default(),
            comparability_class: String::new(),
            capability_profile: CapabilityProfileId::MultiView,
            tracking_config_id: "t".into(),
            calibration_set_id: calib.into(),
            calibration_qa: "pass".into(),
            technique_version: 1,
            metric_version: 1,
            origin: Origin::Live,
            reanalysis_of: None,
        },
        day: f64::from(k) * 2.0,
    }
}

fn rows(k: u32, values: &[f64], grade: Grade, class: &str, calib: &str) -> Vec<Row> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| Row {
            session: session(k, calib),
            rep_index: Some(i as u32 + 1),
            value: *v,
            norm: Some(v / 0.86),
            grade,
            sigma: Some(0.005),
            method_class: "optical_3d".into(),
            detector_id: "det".into(),
            comparability_class: class.into(),
        })
        .collect()
}

#[test]
fn classes_stay_separate_and_b_points_are_secondary() {
    let mut r = rows(0, &[1.0, 1.1, 1.2], Grade::A, "c1", "k");
    r.extend(rows(1, &[2.0, 2.1, 2.2], Grade::A, "c2", "k"));
    r.extend(rows(2, &[1.5, 1.5, 1.6], Grade::B, "c1", "k"));
    r.extend(rows(3, &[1.5, 1.6], Grade::A, "c1", "k"));
    let s = series(&r, "higher_is_more", &ProgressConfig::default());
    assert_eq!(s.len(), 2);
    let c1 = s.iter().find(|c| c.comparability_class == "c1").unwrap();
    assert_eq!(c1.points.len(), 2, "two A values are not a session value");
    assert_eq!(c1.points[0].basis, PointBasis::A);
    assert!((c1.points[0].value - 1.1).abs() < 1e-12);
    assert!((c1.points[0].spread - 0.1).abs() < 1e-12);
    assert_eq!(c1.points[1].basis, PointBasis::B);
    assert_eq!(
        c1.best.as_ref().unwrap().value,
        1.6,
        "best is a grade-A repetition"
    );
    assert!(c1.baseline.is_none());
}

#[test]
fn baseline_trend_and_classification() {
    let mut r = vec![];
    for k in 0..8 {
        let base = 1.0 + 0.05 * f64::from(k);
        r.extend(rows(
            k,
            &[base - 0.01, base, base + 0.01],
            Grade::A,
            "c",
            "k",
        ));
    }
    let s = &series(&r, "higher_is_more", &ProgressConfig::default())[0];
    let b = s.baseline.as_ref().unwrap();
    assert!((b.value - 1.05).abs() < 1e-9);
    assert_eq!(b.sessions, 3);
    let t = s.trend.as_ref().unwrap();
    assert!((t.slope_per_week - 0.05 / 2.0 * 7.0).abs() < 1e-9);
    let c = s.classification.as_ref().unwrap();
    assert_eq!(c.result, ChangeClass::Increased);
    assert!((c.delta - 0.15).abs() < 1e-9);
    // Flat history: no clear change.
    let mut flat = vec![];
    for k in 0..8 {
        flat.extend(rows(k, &[1.0, 1.01, 0.99], Grade::A, "c", "k"));
    }
    let s = &series(&flat, "higher_is_more", &ProgressConfig::default())[0];
    assert_eq!(
        s.classification.as_ref().unwrap().result,
        ChangeClass::NoClearChange
    );
}

#[test]
fn recalibration_draws_a_break_and_withholds_the_label() {
    let mut r = vec![];
    for k in 0..6 {
        let (v, cal) = if k < 4 { (1.0, "k1") } else { (1.3, "k2") };
        r.extend(rows(k, &[v - 0.005, v, v + 0.005], Grade::A, "c", cal));
    }
    let s = &series(&r, "higher_is_more", &ProgressConfig::default())[0];
    assert_eq!(s.breaks.len(), 1);
    assert_eq!(s.breaks[0].reason, "calibration_set_changed");
    let c = s.classification.as_ref().unwrap();
    assert_eq!(c.result, ChangeClass::NoClearChange);
    assert_eq!(c.flags, ["coincides_with_recalibration"]);
}

#[test]
fn theil_sen_resists_an_outlier() {
    let mut p: Vec<(f64, f64)> = (0..8).map(|i| (f64::from(i), 2.0 * f64::from(i))).collect();
    p[3].1 = 100.0;
    assert_eq!(theil_sen(&p), Some(2.0));
    assert_eq!(median(&[3.0, 1.0, 2.0, 4.0]), Some(2.5));
}
