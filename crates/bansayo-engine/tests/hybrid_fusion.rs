//! Synthetic evaluation of camera-only, IMU-only and hybrid tracking on one ground-truth motion
//! (brief §16). Every scenario observes the same simulated body: cameras project it (1 px noise)
//! and reach the solver 40 ms after capture (inference latency); IMUs sample it (0.5° noise) and
//! arrive 5 ms after sampling. The solver ticks at 120 Hz. Errors are against the truth at the
//! publication instant, which is what an athlete or a VR client sees.
//!
//! These are regression checks of the method on synthetic data. They say nothing about accuracy
//! on real cameras or trackers. `cargo test -p bansayo-engine --test hybrid_fusion -- --nocapture`
//! prints the comparison table recorded in docs/validation/hybrid-synthetic.md.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config::{CameraSpec, Config, SimulatedSpec, TrackerSpec, TrackingMode},
    math::{Rot, V3, rot_to_wire},
    pipeline::solve::solve,
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{SimulatedSource, body::body_pose, camera, motions::Motion},
};
use std::collections::BTreeMap;

const SEED: u64 = 7;
const TICK_US: u64 = 8_333;
const CAMERA_LATENCY_US: u64 = 40_000;
const IMU_LATENCY_US: u64 = 5_000;
const CAMERAS: [[f64; 3]; 3] = [[0.0, 1.2, 3.2], [3.0, 1.6, 0.4], [-2.6, 1.4, 1.9]];
const TRACKED: [&str; 5] = [
    "pelvis",
    "left_thigh",
    "left_shin",
    "right_thigh",
    "right_shin",
];

#[derive(Clone)]
struct Scenario {
    name: &'static str,
    camera_hz: Option<f64>,
    imu_hz: Option<f64>,
    mode: TrackingMode,
    heading_deg: f64,
    /// Seconds during which no camera frame is captured.
    occlusion: Option<(f64, f64)>,
    /// Seconds during which the right-shin tracker sends nothing.
    shin_loss: Option<(f64, f64)>,
    motion: Motion,
    speed: f64,
    seconds: f64,
}

impl Scenario {
    fn new(name: &'static str, camera_hz: Option<f64>, imu_hz: Option<f64>) -> Self {
        let mode = match (camera_hz, imu_hz) {
            (Some(_), None) => TrackingMode::CameraOnly,
            (None, Some(_)) => TrackingMode::ImuOnly,
            _ => TrackingMode::HybridAuto,
        };
        Self {
            name,
            camera_hz,
            imu_hz,
            mode,
            heading_deg: 0.0,
            occlusion: None,
            shin_loss: None,
            motion: Motion::FrontKick,
            // A 2.5 s kick at 4x: hip peaks near 650 °/s, a fast kick.
            speed: 4.0,
            seconds: 2.0,
        }
    }
}

#[derive(Default, Debug)]
struct Metrics {
    ticks: u32,
    shin_observed: u32,
    shin_err_deg: Vec<f64>,
    knee_err_deg: Vec<f64>,
    ankle_err_m: Vec<f64>,
    shin_age_ms: Vec<f64>,
    root_measured: u32,
    root_held: u32,
    methods: BTreeMap<String, u32>,
    reasons: BTreeMap<String, u32>,
    heading_deg: Vec<f64>,
}

fn rms(v: &[f64]) -> f64 {
    (v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64).sqrt()
}
fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(0.0, f64::max)
}

fn mount(i: usize) -> Rot {
    Rot::from_euler_angles(
        0.3 * i as f64 - 0.5,
        1.1 - 0.2 * i as f64,
        0.4 + 0.25 * i as f64,
    )
}
fn yaw(deg: f64) -> Rot {
    Rot::from_axis_angle(&V3::y_axis(), deg.to_radians())
}
fn noise(i: usize, k: u64) -> Rot {
    let s =
        |a: u64| ((k * 7_919 + a * 104_729 + i as u64 * 15_485_863) % 1_000) as f64 / 1_000.0 - 0.5;
    let n = 0.5f64.to_radians() * 2.0;
    Rot::from_euler_angles(s(1) * n, s(2) * n, s(3) * n)
}

fn run(s: &Scenario) -> Metrics {
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let cams: Vec<CameraSpec> = CAMERAS
        .iter()
        .enumerate()
        .map(|(i, p)| CameraSpec {
            id: DeviceId::new(format!("CAM{}", i + 1)).unwrap(),
            rate_hz: s.camera_hz.unwrap_or(30.0),
            resolution_px: [1280, 720],
            fov_y_deg: 55.0,
            position_m: *p,
            look_at_m: [0.0, 0.9, 0.0],
            noise_px: 1.0,
        })
        .collect();
    let trackers: Vec<TrackerSpec> = TRACKED
        .iter()
        .map(|seg| TrackerSpec {
            id: DeviceId::new(format!("T-{seg}")).unwrap(),
            rate_hz: s.imu_hz.unwrap_or(100.0),
            noise_deg: 0.5,
            heading_offset_deg: s.heading_deg,
        })
        .collect();
    let source = SimulatedSource {
        spec: SimulatedSpec {
            landmark_schema: bansayo_engine::skeleton::proportions::LANDMARK_SCHEMA_ID.into(),
            id: SourceId::new("sim").unwrap(),
            seed: SEED,
            motion: s.motion,
            speed: 1.0,
            trackers: if s.imu_hz.is_some() {
                trackers.clone()
            } else {
                vec![]
            },
            cameras: if s.camera_hz.is_some() {
                cams.clone()
            } else {
                vec![]
            },
            scenario: vec![],
            scenario_period_s: None,
        },
        skeleton: sk.clone(),
        assignments: tokio::sync::watch::channel(BTreeMap::new()).1,
    };
    let assignments: BTreeMap<DeviceId, SegmentId> = if s.imu_hz.is_some() {
        trackers
            .iter()
            .zip(TRACKED)
            .map(|(t, seg)| (t.id.clone(), SegmentId::new(seg).unwrap()))
            .collect()
    } else {
        BTreeMap::new()
    };
    let mut store = Store::default();
    if s.camera_hz.is_some() {
        store.insert(
            cams.iter()
                .flat_map(|c| camera::profiles(c, "2026-11-19T00:00:00Z"))
                .collect(),
        );
    }
    // Mounting as the reference-pose capture computes it: inverse of the reading at rest.
    let reading = |i: usize, world: &Rot| yaw(-s.heading_deg) * world * mount(i).inverse();
    store.insert(
        trackers
            .iter()
            .enumerate()
            .filter(|_| s.imu_hz.is_some())
            .map(|(i, t)| {
                let data = serde_json::json!({
                    "q_sensor_from_segment": rot_to_wire(&reading(i, &Rot::identity()).inverse())
                });
                CalibrationProfile {
                    schema_version: CALIBRATION_PROFILE_SCHEMA_VERSION,
                    id: ProfileId::new(format!("mounting-{}-{}", t.id, TRACKED[i])).unwrap(),
                    kind: CalibrationKind::Mounting,
                    method: "reference_pose".into(),
                    origin: Origin::Simulated,
                    created_utc: "2026-11-19T00:00:00Z".into(),
                    subject: CalibrationSubject {
                        device_id: Some(t.id.clone()),
                        segment_id: Some(SegmentId::new(TRACKED[i]).unwrap()),
                    },
                    depends_on: vec![],
                    data: data.as_object().cloned().unwrap(),
                    quality: Default::default(),
                }
            })
            .collect(),
    );
    let mut registry = Registry::new(source.descriptors(), assignments, BTreeMap::new());
    let mut config: Config = serde_json::from_str(r#"{"schema_version":2}"#).unwrap();
    config.tracking.mode = s.mode;
    let truth = |t_us: u64| body_pose(&sk, s.motion, t_us as f64 * 1e-6 * s.speed);
    let inside = |w: Option<(f64, f64)>, t_us: u64| {
        w.is_some_and(|(a, b)| (a..b).contains(&(t_us as f64 * 1e-6)))
    };

    // Every observation, ordered by arrival time.
    let mut events: Vec<(u64, SensorObservation)> = vec![];
    let observation = |device: &DeviceId, seq: u64, t_us: u64, data| SensorObservation {
        device_id: device.clone(),
        source_id: SourceId::new("sim").unwrap(),
        origin: Origin::Simulated,
        seq,
        t_us,
        device_time_us: Some(t_us),
        time_quality: TimeQuality::Synchronized,
        calibration_ref: None,
        confidence: Some(1.0),
        data,
    };
    let end_us = (s.seconds * 1e6) as u64;
    if let Some(hz) = s.camera_hz {
        let period = (1e6 / hz) as u64;
        for k in 1..end_us / period {
            let t = k * period + 3_700; // free-running phase
            if inside(s.occlusion, t) {
                continue;
            }
            let body = truth(t);
            for c in &cams {
                let landmarks = camera::project(c, SEED, k, t, &body.positions);
                events.push((
                    t + CAMERA_LATENCY_US,
                    observation(&c.id, k, t, ObservationData::Landmarks2d { landmarks }),
                ));
            }
        }
    }
    if let Some(hz) = s.imu_hz {
        let period = (1e6 / hz) as u64;
        for k in 1..end_us / period {
            let t = k * period;
            let body = truth(t);
            for (i, tr) in trackers.iter().enumerate() {
                if TRACKED[i] == "right_shin" && inside(s.shin_loss, t) {
                    continue;
                }
                let segment = sk.index_of(&SegmentId::new(TRACKED[i]).unwrap()).unwrap();
                let q = rot_to_wire(&(noise(i, k) * reading(i, &body.rotations[segment])));
                let frame = FrameRef::SensorReference { id: tr.id.clone() };
                events.push((
                    t + IMU_LATENCY_US,
                    observation(&tr.id, k, t, ObservationData::Orientation { frame, q }),
                ));
            }
        }
    }
    events.sort_by_key(|(arrival, o)| (*arrival, o.t_us));

    let idx = |name: &str| sk.index_of(&SegmentId::new(name).unwrap()).unwrap();
    let (thigh, shin, foot) = (idx("right_thigh"), idx("right_shin"), idx("right_foot"));
    let mut m = Metrics::default();
    let mut next = 0;
    let mut now = 250_000; // let every stream start before measuring
    while now < end_us {
        while next < events.len() && events[next].0 <= now {
            registry.accept(events[next].1.clone());
            next += 1;
        }
        let solved = solve(&sk, &registry, &store, now, &config);
        registry.fusion = solved.fusion_next.clone();
        m.ticks += 1;
        for r in &solved.quality.reasons {
            *m.reasons.entry(r.code.clone()).or_default() += 1;
        }
        if let Some(h) = registry
            .fusion
            .heading
            .get(&DeviceId::new("T-right_shin").unwrap())
        {
            m.heading_deg.push(h.yaw_rad.to_degrees());
        }
        let Some(pose) = &solved.pose else {
            now += TICK_US;
            continue;
        };
        *m.methods
            .entry(format!("{:?}", solved.methods[shin]))
            .or_default() += 1;
        match pose.root_position_provenance {
            Provenance::Measured => m.root_measured += 1,
            _ if solved.root_held => m.root_held += 1,
            _ => {}
        }
        if solved.observed[shin] {
            m.shin_observed += 1;
            let body = truth(now);
            m.shin_err_deg.push(
                solved.rotations[shin]
                    .angle_to(&body.rotations[shin])
                    .to_degrees(),
            );
            let knee = |r: &[Rot]| r[thigh].angle_to(&r[shin]).to_degrees();
            m.knee_err_deg
                .push((knee(&solved.rotations) - knee(&body.rotations)).abs());
            if pose.root_position_provenance == Provenance::Measured || solved.root_held {
                let p = pose.segments[foot].position_m;
                let ankle = V3::new(p[0].into(), p[1].into(), p[2].into());
                m.ankle_err_m.push((ankle - body.positions[foot]).norm());
            }
            if let Some(t) = solved.t_obs[shin] {
                m.shin_age_ms.push(now.saturating_sub(t) as f64 / 1000.0);
            }
        }
        now += TICK_US;
    }
    m
}

fn row(s: &Scenario, m: &Metrics) -> String {
    let avail = 100.0 * m.shin_observed as f64 / m.ticks.max(1) as f64;
    let root = 100.0 * m.root_measured as f64 / m.ticks.max(1) as f64;
    let ankle = if m.ankle_err_m.is_empty() {
        "—".to_owned()
    } else {
        format!(
            "{:.0} / {:.0}",
            rms(&m.ankle_err_m) * 1000.0,
            max(&m.ankle_err_m) * 1000.0
        )
    };
    format!(
        "| {} | {:.1} / {:.1} | {:.1} | {} | {:.0} | {:.0} | {:.0} |",
        s.name,
        rms(&m.shin_err_deg),
        max(&m.shin_err_deg),
        rms(&m.knee_err_deg),
        ankle,
        rms(&m.shin_age_ms),
        avail,
        root,
    )
}

#[test]
fn camera_only_imu_only_and_hybrid_on_a_fast_front_kick() {
    let scenarios = [
        Scenario::new("camera 30 fps", Some(30.0), None),
        Scenario::new("camera 60 fps", Some(60.0), None),
        Scenario::new("camera 120 fps (reference)", Some(120.0), None),
        Scenario::new("IMU 100 Hz", None, Some(100.0)),
        Scenario::new("IMU 200 Hz", None, Some(200.0)),
        Scenario::new("camera 30 + IMU 100", Some(30.0), Some(100.0)),
        Scenario::new("camera 60 + IMU 100", Some(60.0), Some(100.0)),
    ];
    println!(
        "| Scenario | Shin error RMS / max, ° | Knee error RMS, ° | Ankle error RMS / max, mm | Shin evidence age RMS, ms | Shin available, % | Root measured, % |"
    );
    println!("|---|---|---|---|---|---|---|");
    let results: Vec<Metrics> = scenarios
        .iter()
        .map(|s| {
            let m = run(s);
            println!("{}", row(s, &m));
            m
        })
        .collect();
    let [cam30, cam60, cam120, imu100, imu200, hyb30, hyb60] = &results[..] else {
        unreachable!()
    };
    // Every mode tracks the shin throughout.
    for m in &results {
        assert!(m.shin_observed as f64 > 0.95 * m.ticks as f64, "{m:?}");
    }
    // Faster IMUs carry fresher evidence than a camera behind 40 ms of inference.
    assert!(rms(&hyb30.shin_age_ms) < rms(&cam30.shin_age_ms) / 2.0);
    // At publication time a 30 fps camera lags a fast kick; the trackers do not.
    assert!(rms(&hyb30.shin_err_deg) < rms(&cam30.shin_err_deg) / 2.0);
    assert!(rms(&hyb60.shin_err_deg) < rms(&cam60.shin_err_deg));
    assert!(rms(&cam120.shin_err_deg) < rms(&cam30.shin_err_deg));
    // Hybrid keeps the camera root (IMU-only has none) and the IMU shin.
    assert_eq!(imu100.root_measured, 0);
    assert!(
        imu100.ankle_err_m.is_empty(),
        "no position claims without a camera anchor"
    );
    assert!(hyb30.root_measured as f64 > 0.95 * hyb30.ticks as f64);
    let imu_methods = |m: &Metrics| {
        m.methods.get("ImuMounted").copied().unwrap_or(0)
            + m.methods.get("ImuOpticalFused").copied().unwrap_or(0)
    };
    assert_eq!(imu_methods(hyb30), hyb30.shin_observed);
    assert!(
        !hyb30.reasons.contains_key("imu_camera_disagree"),
        "{:?}",
        hyb30.reasons
    );
    assert!(rms(&imu200.shin_age_ms) <= rms(&imu100.shin_age_ms));
    // The root is a 30 fps measurement 40+ ms old: ankle positions inherit that lag, bounded.
    assert!(max(&hyb30.ankle_err_m) < max(&cam30.ankle_err_m) + 0.05);
}

#[test]
fn roundhouse_kick_camera_only_against_hybrid() {
    let mut rows = vec![];
    for (name, cam, imu) in [
        ("roundhouse, camera 30 fps", Some(30.0), None),
        ("roundhouse, camera 30 + IMU 100", Some(30.0), Some(100.0)),
    ] {
        let mut s = Scenario::new(name, cam, imu);
        s.motion = Motion::RoundhouseKick;
        let m = run(&s);
        println!("{}", row(&s, &m));
        rows.push(m);
    }
    assert!(rms(&rows[1].shin_err_deg) < rms(&rows[0].shin_err_deg) / 2.0);
}

#[test]
fn occlusion_keeps_imu_limbs_and_stops_position_claims_after_the_anchor_limit() {
    let mut s = Scenario::new(
        "camera 30 + IMU 100, 300 ms occlusion",
        Some(30.0),
        Some(100.0),
    );
    s.occlusion = Some((0.8, 1.1));
    let m = run(&s);
    println!("{}", row(&s, &m));
    // The shin never drops: trackers carry it through.
    assert_eq!(m.shin_observed, m.ticks);
    // 300 ms without frames against a 150 ms anchor limit: the last camera set anchors the root
    // (with growing uncertainty) for 150 ms, then position claims stop.
    let unanchored = m.ticks - m.root_measured - m.root_held;
    let expected = ((300_000 - 150_000 - 34_000) / TICK_US) as u32;
    assert!(
        unanchored >= expected,
        "{unanchored} unanchored ticks, expected {expected}"
    );
    assert_eq!(m.ankle_err_m.len() as u32, m.root_measured + m.root_held);
}

#[test]
fn losing_the_shin_tracker_falls_back_to_the_cameras_for_that_segment_only() {
    let mut s = Scenario::new(
        "camera 30 + IMU 100, shin tracker lost",
        Some(30.0),
        Some(100.0),
    );
    s.shin_loss = Some((0.8, 1.6));
    let m = run(&s);
    println!("{}", row(&s, &m));
    assert!(
        m.methods.get("SkeletonFitAxis").copied().unwrap_or(0) > 40,
        "{:?}",
        m.methods
    );
    assert!(
        m.methods.get("ImuMounted").copied().unwrap_or(0)
            + m.methods.get("ImuOpticalFused").copied().unwrap_or(0)
            > 40
    );
    assert_eq!(
        m.shin_observed, m.ticks,
        "one lost tracker does not lose the segment"
    );
}

#[test]
fn optical_heading_correction_converges_slowly_on_kicks_and_holds_when_upright() {
    let mut s = Scenario::new(
        "camera 30 + IMU 100, trackers yawed 40°",
        Some(30.0),
        Some(100.0),
    );
    s.heading_deg = 40.0;
    s.seconds = 12.0;
    let m = run(&s);
    println!("{}", row(&s, &m));
    let last = *m.heading_deg.last().unwrap();
    assert!((last - 40.0).abs() < 3.0, "heading estimate {last:.1}°");
    // Never faster than 15 °/s over the time since the last correction (at most 0.1 s): no snap.
    let steps = m.heading_deg.windows(2).map(|w| (w[1] - w[0]).abs());
    assert!(steps.fold(0.0, f64::max) <= 15.0 * 0.1 + 1e-6);
    assert!(m.methods.get("ImuOpticalFused").copied().unwrap_or(0) > 0);
    // Standing between kicks the shin is vertical: the correction pauses, it does not drift.
    let standing = Scenario {
        motion: Motion::Standing,
        seconds: 3.0,
        ..s.clone()
    };
    let m = run(&standing);
    assert!(
        m.heading_deg.iter().all(|h| h.abs() < 1e-9),
        "no heading evidence while upright"
    );
}
