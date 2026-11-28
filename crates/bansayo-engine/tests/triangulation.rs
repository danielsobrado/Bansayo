//! Multi-camera triangulation against the simulator's own projection of a known body.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config::{
        self, CameraSpec, Config, QualityConfig, SimulatedSpec, TrackerSpec, TriangulationConfig,
    },
    math::{V3, v3_from_wire},
    pipeline::{
        features::features,
        solve::{Solved, solve},
        triangulate::{Camera, point, triangulate},
    },
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{SimulatedSource, body::body_pose, camera, motions::Motion},
};
use futures_util::StreamExt;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::SocketAddr,
    time::Duration,
};

/// Four cameras around a subject at the origin, at different azimuths and heights.
const RING: [[f64; 3]; 4] = [
    [0.0, 1.2, 3.2],
    [3.0, 1.6, 0.4],
    [-0.6, 0.9, -3.1],
    [-3.1, 2.0, 0.8],
];
const NOW: u64 = 10_000_000;
const STALE_US: u64 = 250_000;
const SEED: u64 = 42;

fn spec(i: usize, position_m: [f64; 3], noise_px: f64) -> CameraSpec {
    CameraSpec {
        id: DeviceId::new(format!("CAM{}", i + 1)).unwrap(),
        rate_hz: 30.0,
        resolution_px: [1280, 720],
        fov_y_deg: 50.0,
        position_m,
        look_at_m: [0.0, 1.0, 0.0],
        noise_px,
    }
}
struct Rig {
    specs: Vec<CameraSpec>,
    skeleton: Skeleton,
    registry: Registry,
    store: Store,
    config: Config,
}
fn rig(positions: &[[f64; 3]], noise_px: f64, tracker: bool) -> Rig {
    let skeleton = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let specs: Vec<_> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| spec(i, *p, noise_px))
        .collect();
    let t1 = DeviceId::new("T1").unwrap();
    let source = SimulatedSource {
        spec: SimulatedSpec {
            landmark_schema: bansayo_engine::skeleton::proportions::LANDMARK_SCHEMA_ID.into(),
            id: SourceId::new("sim").unwrap(),
            seed: SEED,
            motion: Motion::Standing,
            speed: 1.0,
            trackers: tracker
                .then(|| TrackerSpec {
                    id: t1.clone(),
                    rate_hz: 100.0,
                    noise_deg: 0.0,
                    heading_offset_deg: 0.0,
                })
                .into_iter()
                .collect(),
            cameras: specs.clone(),
            scenario: vec![],
            scenario_period_s: None,
        },
        skeleton: skeleton.clone(),
        assignments: tokio::sync::watch::channel(BTreeMap::new()).1,
    };
    let assignments = if tracker {
        [(t1, SegmentId::new("pelvis").unwrap())].into()
    } else {
        BTreeMap::new()
    };
    let mut store = Store::default();
    store.insert(
        specs
            .iter()
            .flat_map(|c| camera::profiles(c, "2026-11-16T00:00:00Z"))
            .collect(),
    );
    Rig {
        specs,
        skeleton,
        registry: Registry::new(source.descriptors(), assignments, BTreeMap::new()),
        store,
        config: serde_json::from_str(r#"{"schema_version":1}"#).unwrap(),
    }
}
fn observation(
    device_id: DeviceId,
    t_us: u64,
    seq: u64,
    data: ObservationData,
) -> SensorObservation {
    SensorObservation {
        device_id,
        source_id: SourceId::new("sim").unwrap(),
        origin: Origin::Simulated,
        seq,
        t_us,
        device_time_us: Some(t_us),
        time_quality: TimeQuality::Synchronized,
        calibration_ref: None,
        confidence: Some(1.0),
        data,
    }
}
impl Rig {
    fn truth(&self, motion: Motion, t_s: f64) -> Vec<V3> {
        body_pose(&self.skeleton, motion, t_s).positions
    }
    fn view(&self, camera: usize, truth: &[V3], seq: u64, t_us: u64) -> PoseLandmarks2D {
        camera::project(&self.specs[camera], SEED, seq, t_us, truth)
    }
    fn see(&mut self, landmarks: PoseLandmarks2D) {
        let meta = landmarks.frame_meta.clone();
        self.registry.accept(observation(
            meta.device_id,
            meta.t_us,
            meta.frame_seq,
            ObservationData::Landmarks2d { landmarks },
        ));
    }
    /// Every camera observes the body at `t_s`, captured at `t_us`.
    fn observe(&mut self, motion: Motion, t_s: f64, seq: u64, t_us: u64) -> Vec<V3> {
        let truth = self.truth(motion, t_s);
        for i in 0..self.specs.len() {
            let view = self.view(i, &truth, seq, t_us);
            self.see(view);
        }
        truth
    }
    fn triangulate(&self, now: u64) -> Option<PoseLandmarks3D> {
        triangulate(
            &self.registry,
            &self.store,
            now,
            STALE_US,
            &TriangulationConfig::default(),
        )
    }
    fn points(&self, now: u64) -> Vec<Option<TriangulatedLandmark3D>> {
        match self.triangulate(now) {
            Some(PoseLandmarks3D::Triangulated { points, .. }) => points,
            other => panic!("expected a triangulated set, got {other:?}"),
        }
    }
    fn solve(&self, now: u64) -> Solved {
        solve(
            &self.skeleton,
            &self.registry,
            &self.store,
            now,
            &self.config,
        )
    }
    fn feature(&self, now: u64, id: FeatureId) -> FeatureStatus {
        let devices = self
            .registry
            .status(now, &QualityConfig::default(), &self.store);
        features(&devices, &self.solve(now))
            .into_iter()
            .find(|f| f.id == id)
            .unwrap()
    }
}
fn error_m(p: &TriangulatedLandmark3D, truth: &V3) -> f64 {
    (v3_from_wire(p.m) - truth).norm()
}
fn declared(spec: &CameraSpec) -> (CameraIntrinsicsData, CameraExtrinsicsData) {
    let profiles = camera::profiles(spec, "2026-11-16T00:00:00Z");
    let data = |i: usize| serde_json::Value::Object(profiles[i].data.clone());
    (
        serde_json::from_value(data(0)).unwrap(),
        serde_json::from_value(data(1)).unwrap(),
    )
}

#[test]
fn noiseless_two_three_and_four_cameras_reconstruct_below_a_millimetre() {
    for n in 2..=4 {
        for (motion, t_s) in [
            (Motion::Standing, 0.5),
            (Motion::Squat, 2.0),
            (Motion::FrontKick, 1.25),
        ] {
            let mut rig = rig(&RING[..n], 0.0, false);
            let truth = rig.observe(motion, t_s, 1, NOW);
            let points = rig.points(NOW);
            let (mut worst, mut rms) = (0.0_f64, 0.0_f32);
            for (p, t) in points.iter().zip(&truth) {
                let p = p.as_ref().expect("every landmark is reconstructed");
                assert_eq!(p.views as usize, n, "every camera sees every landmark");
                assert!(p.rejected_views.is_empty());
                let trace = p.view_diagnostics.as_ref().unwrap();
                assert_eq!(trace.len(), n);
                assert!(trace.iter().all(|v| v.used && v.error_px.unwrap() < 0.01));
                worst = worst.max(error_m(p, t));
                rms = rms.max(p.reprojection_error_px);
            }
            eprintln!(
                "{n} cameras, {motion:?}: worst 3D error {:.5} mm, worst RMS reprojection {rms:.5} px",
                worst * 1e3
            );
            assert!(worst < 1e-3, "{worst}");
            assert!(rms < 0.01, "{rms}");
        }
    }
}

#[test]
fn noisy_four_camera_reconstruction_stays_within_the_geometric_bound() {
    // Uniform ±1.5 px noise has σ = 1.5/√3 ≈ 0.87 px. At ~3.2 m with f ≈ 772 px one pixel spans
    // ~4.1 mm, so one view constrains its two lateral axes with σ ≈ 3.6 mm and depth not at all.
    // Four cameras ~90° apart give every world axis at least two such views, so the per-axis σ is
    // at most ~3.6/√2 ≈ 2.6 mm and the expected 3D error about √3 × 2.6 ≈ 4.5 mm. Bound: mean < 8 mm.
    let mut rig = rig(&RING, 1.5, false);
    let mut errors = vec![];
    for seq in 1..=40 {
        let t_us = NOW + seq * 33_333;
        let truth = rig.observe(Motion::Squat, seq as f64 * 0.1, seq, t_us);
        for (p, t) in rig.points(t_us).iter().zip(&truth) {
            let p = p.as_ref().expect("every landmark survives 1.5 px noise");
            assert!(p.rejected_views.is_empty() && p.reprojection_error_px < 3.0);
            errors.push(error_m(p, t));
        }
    }
    let mean = errors.iter().sum::<f64>() / errors.len() as f64;
    let max = errors.iter().copied().fold(0.0, f64::max);
    eprintln!(
        "4 cameras, 1.5 px noise, {} points: mean 3D error {:.2} mm, max {:.2} mm",
        errors.len(),
        mean * 1e3,
        max * 1e3
    );
    assert!(mean < 0.008, "{mean}");
}

#[test]
fn one_camera_or_one_view_gives_no_point_and_the_feature_says_why() {
    let feature = |rig: &Rig| rig.feature(NOW, FeatureId::Landmarks3dTriangulated);
    let mut single = rig(&RING[..1], 0.0, false);
    single.observe(Motion::Standing, 0.5, 1, NOW);
    assert!(single.triangulate(NOW).is_none());
    let f = feature(&single);
    assert_eq!(f.state, FeatureState::Unavailable);
    assert_eq!(f.reasons[0].code, "insufficient_cameras");

    let mut uncalibrated = rig(&RING[..2], 0.0, false);
    uncalibrated.store = Store::default();
    uncalibrated.observe(Motion::Standing, 0.5, 1, NOW);
    assert_eq!(
        feature(&uncalibrated).reasons[0].code,
        "camera_calibration_missing"
    );

    let silent = rig(&RING[..2], 0.0, false);
    assert_eq!(
        feature(&silent).reasons[0].code,
        "insufficient_synchronized_views"
    );

    let mut pair = rig(&RING[..2], 0.0, false);
    let truth = pair.truth(Motion::Standing, 0.5);
    let mut first = pair.view(0, &truth, 1, NOW);
    first.points[3] = None;
    let second = pair.view(1, &truth, 1, NOW);
    pair.see(first);
    pair.see(second);
    let points = pair.points(NOW);
    assert!(points[3].is_none());
    assert_eq!(points.iter().flatten().count(), points.len() - 1);
    let f = feature(&pair);
    assert_eq!(f.state, FeatureState::Degraded);
    assert_eq!(f.reasons[0].code, "landmarks_partially_reconstructed");
    pair.observe(Motion::Standing, 0.5, 2, NOW + 1);
    assert_eq!(feature(&pair).state, FeatureState::Available);
}

#[test]
fn a_displaced_view_is_rejected_among_four_and_voids_the_point_between_two() {
    let mut four = rig(&RING, 0.0, false);
    let truth = four.truth(Motion::Standing, 0.5);
    for i in 0..4 {
        let mut view = four.view(i, &truth, 1, NOW);
        if i == 2 {
            view.points[5].as_mut().unwrap().x_px += 80.0;
        }
        four.see(view);
    }
    let p = four.points(NOW)[5].clone().unwrap();
    eprintln!(
        "80 px outlier among 4 views: 3D error {:.4} mm, RMS {:.4} px",
        error_m(&p, &truth[5]) * 1e3,
        p.reprojection_error_px
    );
    assert_eq!(p.rejected_views, [four.specs[2].id.clone()]);
    assert_eq!(p.views, 3);
    let trace = p.view_diagnostics.as_ref().unwrap();
    assert_eq!(trace.len(), 4);
    assert_eq!(trace.iter().filter(|v| v.used).count(), 3);
    assert!(trace.iter().any(|v| !v.used
        && v.device_id == four.specs[2].id
        && v.error_px.is_some_and(|error| error > 50.0)));
    assert!(error_m(&p, &truth[5]) < 2e-3);

    // Displaced across the near-horizontal epipolar lines of two cameras at similar heights.
    let mut two = rig(&RING[..2], 0.0, false);
    let first = two.view(0, &truth, 1, NOW);
    let mut second = two.view(1, &truth, 1, NOW);
    second.points[5].as_mut().unwrap().y_px += 80.0;
    two.see(first);
    two.see(second);
    let points = two.points(NOW);
    assert!(points[5].is_none());
    assert!(points[4].is_some());
}

#[test]
fn views_out_of_sync_or_stale_are_not_combined() {
    let mut rig = rig(&RING[..3], 0.0, false);
    let truth = rig.truth(Motion::Standing, 0.5);
    for (i, t_us) in [NOW, NOW - 10_000, NOW - 30_000].into_iter().enumerate() {
        let view = rig.view(i, &truth, 1, t_us);
        rig.see(view);
    }
    let Some(PoseLandmarks3D::Triangulated {
        t_us,
        calibration_refs,
        points,
        origin,
        ..
    }) = rig.triangulate(NOW)
    else {
        panic!("two synchronised views");
    };
    // Aligned to the older of the two participating cameras' newest frames.
    assert_eq!(t_us, NOW - 10_000);
    assert_eq!(origin, Origin::Simulated);
    assert_eq!(calibration_refs.len(), 4, "only CAM1 and CAM2 contribute");
    assert!(points.iter().flatten().all(|p| p.views == 2));

    let mut skewed = self::rig(&RING[..2], 0.0, false);
    let a = skewed.view(0, &truth, 1, NOW);
    let b = skewed.view(1, &truth, 1, NOW - 30_000);
    skewed.see(a);
    skewed.see(b);
    assert!(
        skewed.triangulate(NOW).is_none(),
        "30 ms apart exceeds max_skew_ms"
    );
    assert!(
        rig.triangulate(NOW + STALE_US + 20_000).is_none(),
        "stale views are ignored"
    );
}

#[test]
fn distorted_views_triangulate_below_a_millimetre() {
    let truth = body_pose(
        &Skeleton::builtin("bansayo.body.v1", 1.75).unwrap(),
        Motion::FrontKick,
        1.25,
    )
    .positions;
    let cameras: Vec<(DeviceId, Camera, Camera)> = RING
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = spec(i, *p, 0.0);
            let (mut k, e) = declared(&s);
            let ideal = Camera::new(k.clone(), &e).unwrap();
            k.distortion = [-0.35, 0.15, 0.002, -0.003, -0.02];
            let c = Camera::new(k, &e).unwrap();
            // Undistortion inverts the model out to the image corners.
            for [u, v] in [[0.0, 0.0], [1279.0, 719.0], [1000.0, 100.0]] {
                let [x, y] = c.undistort([u, v]);
                let [u2, v2] = c
                    .project(&(c.centre + c.q_world_from_camera * V3::new(x, y, 1.0)))
                    .unwrap();
                assert!((u - u2).hypot(v - v2) < 1e-6, "{u},{v} -> {u2},{v2}");
            }
            (s.id, c, ideal)
        })
        .collect();
    let mut shift = 0.0_f64;
    let mut worst = 0.0_f64;
    for t in &truth {
        let detections: Vec<_> = cameras
            .iter()
            .map(|(id, c, ideal)| {
                let [x, y] = c.project(t).unwrap();
                let [u, v] = ideal.project(t).unwrap();
                shift = shift.max((x - u).hypot(y - v));
                let landmark = Landmark2D {
                    x_px: x as f32,
                    y_px: y as f32,
                    confidence: 1.0,
                };
                (id, c, landmark)
            })
            .collect();
        let refs: Vec<_> = detections.iter().map(|(id, c, l)| (*id, *c, l)).collect();
        let p = point(&refs, &TriangulationConfig::default()).unwrap();
        worst = worst.max(error_m(&p, t));
    }
    eprintln!(
        "distortion shifts pixels by up to {shift:.1} px; worst 3D error {:.5} mm",
        worst * 1e3
    );
    assert!(shift > 5.0, "distortion must matter: {shift}");
    assert!(worst < 1e-3, "{worst}");
}

#[test]
fn near_parallel_rays_give_no_points() {
    let mut rig = rig(&[[0.0, 1.2, 3.2], [0.08, 1.2, 3.2]], 0.0, false);
    rig.observe(Motion::Standing, 0.5, 1, NOW);
    assert!(rig.points(NOW).iter().all(Option::is_none));
}

#[test]
fn simulator_declared_extrinsics_reproject_truth_to_the_simulators_pixels() {
    let truth = body_pose(
        &Skeleton::builtin("bansayo.body.v1", 1.75).unwrap(),
        Motion::Squat,
        2.0,
    )
    .positions;
    let mut compared = 0;
    for (i, p) in RING.iter().enumerate() {
        let s = spec(i, *p, 0.0);
        let (k, e) = declared(&s);
        let model = Camera::new(k.clone(), &e).unwrap();
        // Without distortion, undistortion is exactly the pinhole normalisation.
        let n = [(100.0 - k.cx_px) / k.fx_px, (650.0 - k.cy_px) / k.fy_px];
        assert_eq!(model.undistort([100.0, 650.0]), n);
        let view = camera::project(&s, SEED, 1, NOW, &truth);
        for (t, l) in truth.iter().zip(&view.points) {
            let l = l.as_ref().unwrap();
            let [x, y] = model.project(t).unwrap();
            assert!((x - l.x_px as f64).abs() < 1e-3 && (y - l.y_px as f64).abs() < 1e-3);
            compared += 1;
        }
    }
    assert_eq!(compared, 4 * truth.len());
}

#[test]
fn solver_measures_the_root_with_trackers_and_fits_a_camera_only_skeleton() {
    let mut rig = rig(&RING[..2], 0.0, true);
    let truth = body_pose(&rig.skeleton, Motion::Squat, 2.0);
    rig.observe(Motion::Squat, 2.0, 1, NOW);
    rig.registry.accept(observation(
        DeviceId::new("T1").unwrap(),
        NOW,
        1,
        ObservationData::Orientation {
            frame: FrameRef::SensorReference {
                id: DeviceId::new("T1").unwrap(),
            },
            q: [0.0, 0.0, 0.0, 1.0],
        },
    ));
    let solved = rig.solve(NOW);
    let pose = solved.pose.as_ref().unwrap();
    assert_eq!(pose.root_position_provenance, Provenance::Measured);
    let error = (v3_from_wire(pose.root_position_m) - truth.root).norm();
    eprintln!("measured root error {:.5} mm", error * 1e3);
    assert!(error < 1e-3);
    assert_eq!(pose.segments[0].position_m, pose.root_position_m);
    assert_eq!(
        rig.feature(NOW, FeatureId::RootPosition).state,
        FeatureState::Available
    );

    let mut cameras_only = self::rig(&RING[..2], 0.0, false);
    cameras_only.observe(Motion::Squat, 2.0, 1, NOW);
    let solved = cameras_only.solve(NOW);
    assert!(solved.landmarks_3d.is_some());
    // Cameras alone now fit the skeleton: measured root, observed legs, exact knee angle.
    let pose = solved.pose.as_ref().expect("camera-only pose");
    assert_eq!(pose.root_position_provenance, Provenance::Measured);
    assert!((v3_from_wire(pose.root_position_m) - truth.root).norm() < 1e-3);
    let joints = bansayo_engine::pipeline::joints::measure(
        &cameras_only.skeleton,
        &solved.rotations,
        &solved.observed,
    );
    let knee = joints[0].value.expect("left knee measured") as f64;
    assert!(
        (knee - Motion::Squat.angles(2.0).left_knee).abs() < 1e-3,
        "{knee}"
    );
    assert_ne!(solved.quality.state, TrackingState::Unavailable);
}

async fn request(addr: SocketAddr, method: &str, path: &str, body: &str) -> serde_json::Value {
    let (method, path, body) = (method.to_owned(), path.to_owned(), body.to_owned());
    tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(addr).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(socket, "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap()
    })
    .await
    .unwrap()
}

async fn next<S, E>(ws: &mut S) -> ServerMessage
where
    S: futures_util::Stream<Item = Result<tokio_tungstenite::tungstenite::Message, E>> + Unpin,
    E: std::fmt::Debug,
{
    let m = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(m.to_text().unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_streams_triangulated_landmarks_by_topic_and_replays_them() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{ data_dir: {:?} }}
api: {{ bind: "127.0.0.1:0" }}
sources:
  - id: sim
    kind: simulated
    motion: squat
    cameras:
      - {{ id: CAM1, rate_hz: 30, resolution_px: [1280,720], fov_y_deg: 50, position_m: [0,1.2,3.2], look_at_m: [0,1,0], noise_px: 1.5 }}
      - {{ id: CAM2, rate_hz: 30, resolution_px: [1280,720], fov_y_deg: 50, position_m: [3,1.6,0.4], look_at_m: [0,1,0], noise_px: 1.5 }}
"#,
        dir.path().display().to_string()
    );
    let engine = bansayo_engine::start(config::parse(&yaml).unwrap())
        .await
        .unwrap();
    let addr = engine.bound_addr();
    let (mut three, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/api/v1/stream?topics=landmarks_3d"))
            .await
            .unwrap();
    let (mut two, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/api/v1/stream?topics=landmarks_2d"))
            .await
            .unwrap();
    let mut last = 0;
    let mut sets = 0;
    while sets < 5 {
        match next(&mut three).await {
            ServerMessage::Landmarks3d {
                landmarks:
                    PoseLandmarks3D::Triangulated {
                        t_us,
                        frame,
                        points,
                        calibration_refs,
                        ..
                    },
            } => {
                assert!(t_us > last, "one message per camera frame set");
                last = t_us;
                assert_eq!(frame, FrameRef::World);
                assert_eq!(calibration_refs.len(), 4);
                assert!(points.iter().all(Option::is_some));
                sets += 1;
            }
            ServerMessage::Hello { .. } | ServerMessage::Notice { .. } => {}
            other => panic!("unrequested message {other:?}"),
        }
    }
    for _ in 0..20 {
        assert!(matches!(
            next(&mut two).await,
            ServerMessage::Hello { .. } | ServerMessage::Landmarks2d { .. }
        ));
    }
    let state = request(addr, "GET", "state", "").await;
    let state = &state["data"];
    let feature = |id: &str| {
        state["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == id)
            .unwrap()["state"]
            .clone()
    };
    assert_eq!(feature("landmarks_3d_triangulated"), "available");
    assert_eq!(feature("skeleton_pose"), "degraded");
    let cam1 = state["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["descriptor"]["id"] == "CAM1")
        .unwrap();
    assert_eq!(
        cam1["calibration"],
        serde_json::json!([
            {"kind":"camera_intrinsics","state":"valid","profile_id":"camera-intrinsics-CAM1"},
            {"kind":"camera_extrinsics","state":"valid","profile_id":"camera-extrinsics-CAM1"}
        ])
    );
    let intrinsics = state["calibration"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "camera-intrinsics-CAM1")
        .unwrap();
    assert_eq!(intrinsics["method"], "declared_by_simulator");
    assert!(intrinsics["data"]["fx_px"].as_f64().unwrap() > 700.0);
    assert!(
        !dir.path()
            .join("calibration/camera-intrinsics-CAM1.json")
            .exists(),
        "declared profiles are not persisted"
    );

    let started = request(addr, "POST", "recording/start", "{}").await;
    assert_eq!(started["ok"], true, "{started}");
    tokio::time::sleep(Duration::from_millis(400)).await;
    let stopped = request(addr, "POST", "recording/stop", "{}").await;
    let id = SessionId::new(stopped["data"]["id"].as_str().unwrap()).unwrap();
    engine.shutdown().await.unwrap();
    let recording = bansayo_engine::recording::reader::load(dir.path(), &id).unwrap();
    let mut replay = bansayo_engine::recording::replay::Replay::new(recording, 1.0, false).unwrap();
    let mut messages = vec![];
    while !replay.paused {
        messages.extend(replay.advance(10_000).unwrap());
    }
    let replayed = messages
        .iter()
        .filter(|m| matches!(m, ServerMessage::Landmarks3d { .. }))
        .count();
    assert!(
        replayed > 5,
        "replay triangulates recorded camera views: {replayed}"
    );
}

#[test]
fn bundled_multi_camera_and_example_configs_load() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    let multi = config::load_with_data_dir(root.join("sim-multi-camera.yaml"), dir.path()).unwrap();
    assert_eq!(multi.simulated[0].cameras.len(), 4);
    assert_eq!(multi.simulated[0].trackers.len(), 12);
    let example =
        config::load_with_data_dir(root.join("bansayo.example.yaml"), dir.path()).unwrap();
    assert_eq!(example.config.triangulation.min_views, 2);
    // The camera example is valid apart from the local model files, which CI does not have.
    match config::load_with_data_dir(root.join("cameras-two.yaml"), dir.path()) {
        Ok(v) => assert_eq!(v.cameras[0].cameras.len(), 2),
        Err(config::ConfigError::Invalid(problems)) => assert!(
            problems.iter().all(|p| p.path == "sources[0].models_dir"),
            "{problems:#?}"
        ),
        Err(other) => panic!("{other}"),
    }
}

#[test]
fn free_running_cameras_are_interpolated_to_one_instant() {
    // Camera 0 at 0 and 33 ms; camera 1 runs 16 ms out of phase, during a fast kick.
    let t0 = 0.45; // seconds into the kick, where the leg moves quickly
    let at = |rig: &Rig, ms: u64| rig.truth(Motion::FrontKick, t0 + ms as f64 / 1000.0);
    let error = |rig: &mut Rig, camera1_ms: &[u64]| {
        for (seq, ms) in [0u64, 33].into_iter().enumerate() {
            let view = rig.view(0, &at(rig, ms), seq as u64 + 1, NOW + ms * 1000);
            rig.see(view);
        }
        for (seq, ms) in camera1_ms.iter().enumerate() {
            let view = rig.view(1, &at(rig, *ms), seq as u64 + 1, NOW + ms * 1000);
            rig.see(view);
        }
        let Some(PoseLandmarks3D::Triangulated { t_us, points, .. }) =
            rig.triangulate(NOW + 49_000)
        else {
            panic!("triangulated")
        };
        assert_eq!(t_us, NOW + 33_000, "aligned to camera 0's newest frame");
        let truth = at(rig, 33);
        let errors: Vec<f64> = points
            .iter()
            .zip(&truth)
            .filter_map(|(p, t)| Some(error_m(p.as_ref()?, t)))
            .collect();
        errors.iter().cloned().fold(0.0, f64::max)
    };
    let interpolated = error(&mut rig(&RING[..2], 0.0, false), &[16, 49]);
    let nearest = error(&mut rig(&RING[..2], 0.0, false), &[49]);
    eprintln!(
        "worst landmark error: interpolated {:.1} mm, nearest frame {:.1} mm",
        interpolated * 1e3,
        nearest * 1e3
    );
    assert!(interpolated < nearest / 3.0, "{interpolated} vs {nearest}");
}
