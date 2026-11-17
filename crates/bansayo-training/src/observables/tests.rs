#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::testkit::{body, index as seg, rest, sample, skeleton};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};

const H: f64 = 1.75;

fn frame(rot: &[Rot<f64>], root: V3<f64>) -> ObservableFrame {
    let sk = skeleton(H);
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    o.step(&sample(body(&sk, rot, root, 1000), 1000))
}

fn val(f: &ObservableFrame, id: &str) -> Option<f64> {
    f.get(index(id).unwrap()).value
}

fn why(f: &ObservableFrame, id: &str) -> String {
    f.get(index(id).unwrap())
        .reasons
        .first()
        .map(|r| r.code.clone())
        .unwrap_or_default()
}

fn root() -> V3<f64> {
    V3::new(0.0, 0.530 * H, 0.0)
}

fn rx(a: f64) -> Rot<f64> {
    Rot::from_axis_angle(&V3::x_axis(), a)
}

#[test]
fn catalogue_ids_are_unique_and_sided() {
    let all: Vec<&String> = ids().iter().map(|(i, _)| i).collect();
    let mut sorted = all.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), all.len());
    assert!(index("knee_flexion.left").is_some());
    assert!(index("pelvis_yaw").is_some());
    assert!(index("pelvis_yaw.left").is_none());
    assert_eq!(
        ids()[mirror_index(index("ankle_speed.left").unwrap())].0,
        "ankle_speed.right"
    );
}

#[test]
fn knee_hip_and_thigh_angles_follow_the_definitions() {
    let sk = skeleton(H);
    let mut r = rest(&sk);
    r[seg(&sk, "left_thigh")] = rx(-FRAC_PI_3);
    r[seg(&sk, "left_shin")] = rx(-FRAC_PI_3) * rx(FRAC_PI_2);
    let f = frame(&r, root());
    assert!((val(&f, "knee_flexion.left").unwrap() - FRAC_PI_2).abs() < 1e-6);
    assert!((val(&f, "hip_flexion.left").unwrap() - FRAC_PI_3).abs() < 1e-6);
    assert!((val(&f, "thigh_elevation.left").unwrap() - FRAC_PI_3).abs() < 1e-6);
    assert!(val(&f, "thigh_azimuth.left").unwrap().abs() < 1e-6);
    assert!(val(&f, "knee_flexion.right").unwrap().abs() < 1e-6);
    // Thigh to the side: azimuth π/2 on either side.
    let mut r = rest(&sk);
    r[seg(&sk, "left_thigh")] = Rot::from_axis_angle(&V3::z_axis(), FRAC_PI_2);
    r[seg(&sk, "right_thigh")] = Rot::from_axis_angle(&V3::z_axis(), -FRAC_PI_2);
    let f = frame(&r, root());
    for s in ["left", "right"] {
        let az = val(&f, &format!("thigh_azimuth.{s}")).unwrap();
        assert!((az - FRAC_PI_2).abs() < 1e-6, "{s} {az}");
        assert!((val(&f, &format!("thigh_elevation.{s}")).unwrap() - FRAC_PI_2).abs() < 1e-6);
    }
}

#[test]
fn every_suppression_rule_reports_its_reason() {
    let sk = skeleton(H);
    // out_of_plane: thigh straight to the side leaves no sagittal projection.
    let mut r = rest(&sk);
    r[seg(&sk, "left_thigh")] = Rot::from_axis_angle(&V3::z_axis(), FRAC_PI_2);
    let f = frame(&r, root());
    assert_eq!(val(&f, "hip_flexion.left"), None);
    assert_eq!(why(&f, "hip_flexion.left"), "out_of_plane");
    // direction_undefined: a hanging thigh has no azimuth.
    let f = frame(&rest(&sk), root());
    assert_eq!(why(&f, "thigh_azimuth.left"), "direction_undefined");
    // foot_vertical: toes pointing at the floor.
    let mut r = rest(&sk);
    r[seg(&sk, "left_foot")] = rx(1.4);
    let f = frame(&r, root());
    assert_eq!(why(&f, "foot_yaw.left"), "foot_vertical");
    assert!(val(&f, "foot_yaw.right").is_some());
    // input_not_observed: an unobserved keypoint and a rest-filled segment.
    let mut b = body(&sk, &rest(&sk), root(), 1000);
    b.keypoints[kp("left_ankle")]
        .as_mut()
        .unwrap()
        .evidence
        .provenance = Provenance::Unavailable;
    b.segments[seg(&sk, "left_shin")].axis.method = bansayo_contracts::MethodId::RestFill;
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let f = o.step(&sample(b, 1000));
    assert_eq!(why(&f, "ankle_height.left"), "input_not_observed");
    assert_eq!(why(&f, "knee_flexion.left"), "input_not_observed");
    assert_eq!(why(&f, "knee_extension_rate.left"), "input_not_observed");
    // no_world_body.
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let mut s = sample(body(&sk, &rest(&sk), root(), 1000), 1000);
    s.body = None;
    let f = o.step(&s);
    assert!(f.values.iter().all(|v| v.value.is_none()));
    assert_eq!(why(&f, "pelvis_yaw"), "no_world_body");
}

#[test]
fn heights_are_world_y_and_normalised_by_leg_length() {
    let sk = skeleton(H);
    let f = frame(&rest(&sk), root());
    let ankle = f.get(index("ankle_height.left").unwrap());
    assert!((ankle.value.unwrap() - 0.039 * H).abs() < 1e-5);
    assert!((ankle.norm.unwrap() - 0.039 / 0.491).abs() < 1e-5);
    assert!(val(&f, "toe_height.left").unwrap().abs() < 1e-5);
    assert!(val(&f, "heel_height.right").unwrap().abs() < 1e-5);
    let o = Observer::new(&sk, Cutoffs::default()).unwrap();
    assert!((o.lengths().leg_length - 0.491 * H).abs() < 1e-6);
    assert!((o.lengths().foot_length - 0.152 * H).abs() < 1e-6);
}

#[test]
fn speeds_and_rates_settle_on_constant_motion() {
    let sk = skeleton(H);
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let mut last = None;
    for k in 0..120u64 {
        let t = 1000 + k * 10_000;
        let root = root() + V3::new(0.01 * k as f64, 0.0, 0.0);
        last = Some(o.step(&sample(body(&sk, &rest(&sk), root, t), t)));
    }
    let f = last.unwrap();
    for id in ["ankle_speed.left", "root_speed", "wrist_speed.right"] {
        let v = val(&f, id).unwrap();
        assert!((v - 1.0).abs() < 1e-3, "{id} {v}");
    }
    assert!(val(&f, "knee_extension_rate.left").unwrap().abs() < 1e-9);
    let n = f.get(index("ankle_speed.left").unwrap()).norm.unwrap();
    assert!((n - 1.0 / (0.491 * H)).abs() < 1e-3);
}

#[test]
fn held_values_are_not_fresh_between_inputs() {
    let sk = skeleton(H);
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let b = body(&sk, &rest(&sk), root(), 1000);
    assert!(o.step(&sample(b.clone(), 1000)).get(0).fresh);
    let mut s = sample(b, 2000);
    s.t_us = 2000;
    let f = o.step(&s);
    assert!(!f.get(0).fresh);
    assert!(f.get(0).value.is_some());
}

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

fn random_pose(sk: &SkeletonDefinition, seed: &mut u64) -> Vec<Rot<f64>> {
    let mut r = rest(sk);
    for name in [
        "pelvis",
        "chest",
        "left_thigh",
        "left_shin",
        "left_foot",
        "right_thigh",
        "right_shin",
        "right_foot",
        "left_upper_arm",
        "right_forearm",
    ] {
        let axis = V3::new(lcg(seed) - 0.5, lcg(seed) - 0.5, lcg(seed) - 0.5);
        let mag = 1.2 * lcg(seed);
        r[seg(sk, name)] = Rot::from_scaled_axis(axis.normalize() * mag);
    }
    r
}

fn transform(b: &BodyState, rot: &Rot<f64>, shift: &V3<f64>) -> BodyState {
    let mut out = b.clone();
    for k in out.keypoints.iter_mut().flatten() {
        let p = rot * v(k.position_m) + shift;
        k.position_m = [p.x as f32, p.y as f32, p.z as f32];
    }
    for s in &mut out.segments {
        let r = rot * q(s.rotation);
        let qq = r.quaternion();
        s.rotation = [qq.i as f32, qq.j as f32, qq.k as f32, qq.w as f32];
    }
    out.root = out.keypoints[0].clone().unwrap();
    out
}

fn compare(a: &ObservableFrame, b: &ObservableFrame, check: impl Fn(&str) -> bool) {
    for (x, y) in a.values.iter().zip(&b.values) {
        if !check(&x.id) || entry(index(&x.id).unwrap()).class == Rate {
            continue;
        }
        match (x.value, y.value) {
            (Some(p), Some(q)) => assert!((p - q).abs() < 2e-4, "{} {p} {q}", x.id),
            (p, q) => assert_eq!(p.is_some(), q.is_some(), "{}", x.id),
        }
    }
}

#[test]
fn property_rigid_transform_invariance() {
    let sk = skeleton(H);
    let mut seed = 7;
    for _ in 0..40 {
        let pose = random_pose(&sk, &mut seed);
        let b = body(&sk, &pose, root(), 1000);
        let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
        let base = o.step(&sample(b.clone(), 1000));
        let any = Rot::from_scaled_axis(V3::new(lcg(&mut seed), lcg(&mut seed), lcg(&mut seed)));
        let shift = V3::new(lcg(&mut seed), lcg(&mut seed), lcg(&mut seed));
        let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
        let moved = o.step(&sample(transform(&b, &any, &shift), 1000));
        compare(&base, &moved, |id| !entry(index(id).unwrap()).world);
        // A turn about world up and a floor shift keep the up-referenced ones too, except headings.
        let yaw = 2.0 * lcg(&mut seed) - 1.0;
        let turn = Rot::from_axis_angle(&V3::y_axis(), yaw);
        let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
        let turned = o.step(&sample(
            transform(&b, &turn, &V3::new(0.3, 0.0, -0.2)),
            1000,
        ));
        compare(&base, &turned, |id| {
            !is_heading(id.split('.').next().unwrap())
        });
        for id in ["pelvis_yaw", "foot_yaw.left"] {
            if let (Some(a), Some(c)) = (val(&base, id), val(&turned, id)) {
                assert!((unwrap_near(c - a, yaw) - yaw).abs() < 2e-4, "{id}");
            }
        }
    }
}

fn mirror(sk: &SkeletonDefinition, b: &BodyState) -> BodyState {
    let reflect_q = |a: [f32; 4]| [a[0], -a[1], -a[2], a[3]];
    let reflect_p = |a: [f32; 3]| [-a[0], a[1], a[2]];
    let swap = |name: &str| {
        if let Some(r) = name.strip_prefix("left_") {
            format!("right_{r}")
        } else if let Some(r) = name.strip_prefix("right_") {
            format!("left_{r}")
        } else {
            name.to_string()
        }
    };
    let mut out = b.clone();
    for (i, s) in sk.segments.iter().enumerate() {
        let j = seg(sk, &swap(s.id.as_str()));
        out.segments[i] = b.segments[j].clone();
        out.segments[i].rotation = reflect_q(b.segments[j].rotation);
    }
    for (i, name) in BODY_KEYPOINT_NAMES.iter().enumerate() {
        let j = kp(&swap(name));
        out.keypoints[i] = b.keypoints[j].clone();
        if let Some(k) = out.keypoints[i].as_mut() {
            k.position_m = reflect_p(k.position_m);
        }
    }
    out.root = out.keypoints[0].clone().unwrap();
    out
}

#[test]
fn property_mirror_swaps_sides_and_negates_signed_observables() {
    let sk = skeleton(H);
    let mut seed = 11;
    for _ in 0..40 {
        let b = body(&sk, &random_pose(&sk, &mut seed), root(), 1000);
        let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
        let a = o.step(&sample(b.clone(), 1000));
        let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
        let m = o.step(&sample(mirror(&sk, &b), 1000));
        for (i, x) in a.values.iter().enumerate() {
            let e = entry(i);
            if e.class == Rate {
                continue;
            }
            let y = m.get(mirror_index(i));
            let sign = if e.signed || is_heading(e.name) {
                -1.0
            } else {
                1.0
            };
            match (x.value, y.value) {
                (Some(p), Some(q)) => {
                    let d = if is_heading(e.name) {
                        unwrap_near(q * sign, p) - p
                    } else {
                        q * sign - p
                    };
                    assert!(d.abs() < 2e-4, "{} {p} {q}", x.id);
                }
                (p, q) => assert_eq!(p.is_some(), q.is_some(), "{}", x.id),
            }
        }
    }
}

#[test]
fn zero_phase_refilter_removes_live_lag() {
    let sk = skeleton(H);
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let dt = 1_000_000 / 60;
    let mut live = vec![];
    for k in 0..120u64 {
        let t = 1_000_000 + k * dt;
        let x = (-(((k as f64) - 60.0) / 8.0).powi(2)).exp();
        let r = root() + V3::new(0.0, 0.2 * x, 0.0);
        let f = o.step(&sample(body(&sk, &rest(&sk), r, t), t));
        live.push((t, val(&f, "ankle_height.left").unwrap()));
    }
    let peak = |s: &[(u64, f64)]| s.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
    let truth = 1_000_000 + 60 * dt;
    let frames = o.refiltered(1_000_000 + REFILTER_PAD_US, 1_000_000 + 119 * dt);
    let refiltered: Vec<(u64, f64)> = frames
        .iter()
        .map(|f| (f.t_us, val(f, "ankle_height.left").unwrap()))
        .collect();
    assert_eq!(peak(&refiltered), truth);
    // The live filter lags: after the true peak it reads higher than before it.
    let at = |s: &[(u64, f64)], t: u64| s.iter().find(|p| p.0 == t).unwrap().1;
    assert!(at(&live, truth + 4 * dt) > at(&live, truth - 4 * dt) + 1e-3);
    assert!((at(&refiltered, truth + 4 * dt) - at(&refiltered, truth - 4 * dt)).abs() < 1e-3);
    let speed = frames
        .iter()
        .filter_map(|f| val(f, "ankle_speed.left"))
        .fold(0.0, f64::max);
    assert!(speed > 0.5 && speed < 2.0, "{speed}");
}
