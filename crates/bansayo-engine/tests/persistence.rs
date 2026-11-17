//! Atomic runtime persistence, legacy migration, failure isolation and recording context tests.
use bansayo_contracts::*;
use bansayo_engine::{
    config,
    recording::{
        reader::{self, RecordedData},
        replay::Replay,
    },
    registry::persistence::{
        RUNTIME_SCHEMA_VERSION, RuntimeSnapshot, inject_persist_failure, load_runtime,
        runtime_snapshot_path, save_runtime_snapshot,
    },
    skeleton::Skeleton,
    start,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::SocketAddr,
    path::Path,
    time::Duration,
};

async fn request(addr: SocketAddr, method: &str, path: &str, body: &str) -> (u16, Value) {
    let method = method.to_owned();
    let path = path.to_owned();
    let body = body.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(addr).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(
            socket,
            "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        (
            headers.split_whitespace().nth(1).unwrap().parse().unwrap(),
            serde_json::from_str(body).unwrap(),
        )
    })
    .await
    .unwrap()
}

fn tracker_yaml(dir: &Path, extra_camera: bool) -> String {
    let camera = if extra_camera {
        "\n    cameras:\n      - { id: CAM1, rate_hz: 30, resolution_px: [1280, 720], fov_y_deg: 50, position_m: [0, 1.2, 3.2], look_at_m: [0, 1, 0] }"
    } else {
        ""
    };
    format!(
        r#"schema_version: 1
engine: {{ data_dir: {:?}, publish_rate_hz: 60 }}
api: {{ bind: "127.0.0.1:0" }}
assignments: {{ T1: pelvis, T2: left_thigh }}
sources:
  - id: sim
    kind: simulated
    motion: reference_pose
    trackers:
      - {{ id: T1, rate_hz: 100 }}
      - {{ id: T2, rate_hz: 100 }}{camera}
"#,
        dir.display().to_string()
    )
}

fn device_assignment(state: &Value, id: &str) -> Option<String> {
    state["data"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["descriptor"]["id"] == id)
        .unwrap()["assignment"]
        .as_str()
        .map(str::to_owned)
}

fn calibration_state(state: &Value, id: &str) -> Option<String> {
    state["data"]["calibration"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .map(|p| p["state"].as_str().unwrap().to_owned())
}

fn mounting_profile_json(id: &str, device: &str, segment: &str) -> Value {
    json!({
        "schema_version": 1,
        "id": id,
        "kind": "mounting",
        "method": "reference_pose",
        "origin": "simulated",
        "created_utc": "2026-11-16T00:00:00Z",
        "subject": { "device_id": device, "segment_id": segment },
        "depends_on": [{ "kind": "heading_alignment", "device_id": device }],
        "data": { "q_sensor_from_segment": [0.0, 0.0, 0.0, 1.0] },
        "quality": { "capture_window_ms": 1000, "angular_spread_rad": 0.0 }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn assignment_unassignment_and_reassignment_invalidation_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = tracker_yaml(dir.path(), false);
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#
        )
        .await
        .0,
        200
    );
    assert!(runtime_snapshot_path(dir.path()).exists());

    // Reassign T2 -> head (invalidates T2's mounting profile, leaves T1 valid).
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T2/assignment",
            r#"{"segment":"head"}"#
        )
        .await
        .0,
        200
    );
    let mid_state = request(addr, "GET", "state", "").await.1;
    assert_eq!(
        calibration_state(&mid_state, "mounting-T1-pelvis").as_deref(),
        Some("valid")
    );
    assert_eq!(
        calibration_state(&mid_state, "mounting-T2-left_thigh").as_deref(),
        Some("stale")
    );
    // Unassign T1 (must override static config assignment and mark T1's calibration stale).
    assert_eq!(
        request(addr, "PUT", "devices/T1/assignment", r#"{"segment":null}"#)
            .await
            .0,
        200
    );
    engine.shutdown().await.unwrap();

    let restarted = start(config::parse(&yaml).unwrap()).await.unwrap();
    let state = request(restarted.bound_addr(), "GET", "state", "").await.1;
    assert_eq!(device_assignment(&state, "T1"), None);
    assert_eq!(device_assignment(&state, "T2").as_deref(), Some("head"));
    assert_eq!(
        calibration_state(&state, "mounting-T1-pelvis").as_deref(),
        Some("stale")
    );
    assert_eq!(
        calibration_state(&state, "mounting-T2-left_thigh").as_deref(),
        Some("stale")
    );
    restarted.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_persistence_does_not_modify_live_or_restarted_state() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = tracker_yaml(dir.path(), false);
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#
        )
        .await
        .0,
        200
    );

    // 1. Failed assignment must neither change assignment nor mark calibration stale.
    inject_persist_failure(dir.path());
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T2/assignment",
            r#"{"segment":"head"}"#
        )
        .await
        .0,
        503
    );
    let state = request(addr, "GET", "state", "").await.1;
    assert_eq!(
        device_assignment(&state, "T2").as_deref(),
        Some("left_thigh")
    );
    assert_eq!(
        calibration_state(&state, "mounting-T2-left_thigh").as_deref(),
        Some("valid")
    );

    // 2. Failed delete must leave the profile valid in memory and on disk.
    inject_persist_failure(dir.path());
    assert_eq!(
        request(addr, "DELETE", "calibration/mounting-T1-pelvis", "")
            .await
            .0,
        503
    );
    let state = request(addr, "GET", "state", "").await.1;
    assert_eq!(
        calibration_state(&state, "mounting-T1-pelvis").as_deref(),
        Some("valid")
    );

    // 3. Commit a reassignment, then fail reference-pose capture persistence.
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T2/assignment",
            r#"{"segment":"head"}"#
        )
        .await
        .0,
        200
    );
    inject_persist_failure(dir.path());
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":["T2"]}"#,
        )
        .await
        .0,
        503
    );
    let state = request(addr, "GET", "state", "").await.1;
    assert_eq!(calibration_state(&state, "mounting-T2-head"), None);
    assert_eq!(
        calibration_state(&state, "mounting-T2-left_thigh").as_deref(),
        Some("stale")
    );
    engine.shutdown().await.unwrap();

    // Restarting must observe the exact last committed state with zero contradictions.
    let restarted = start(config::parse(&yaml).unwrap()).await.unwrap();
    let state = request(restarted.bound_addr(), "GET", "state", "").await.1;
    assert_eq!(device_assignment(&state, "T1").as_deref(), Some("pelvis"));
    assert_eq!(device_assignment(&state, "T2").as_deref(), Some("head"));
    assert_eq!(
        calibration_state(&state, "mounting-T1-pelvis").as_deref(),
        Some("valid")
    );
    assert_eq!(
        calibration_state(&state, "mounting-T2-left_thigh").as_deref(),
        Some("stale")
    );
    assert_eq!(calibration_state(&state, "mounting-T2-head"), None);
    restarted.shutdown().await.unwrap();

    // Real filesystem failure (state path blocked by a regular file) also fails cleanly.
    let blocked = tempfile::tempdir().unwrap();
    std::fs::write(blocked.path().join("state"), b"not-a-directory").unwrap();
    let err = save_runtime_snapshot(
        blocked.path(),
        &RuntimeSnapshot {
            schema_version: RUNTIME_SCHEMA_VERSION,
            assignments: BTreeMap::new(),
            stale_calibrations: Default::default(),
            calibration_profiles: vec![],
        },
    )
    .unwrap_err();
    assert!(!err.to_string().is_empty());
    assert_eq!(
        std::fs::read(blocked.path().join("state")).unwrap(),
        b"not-a-directory"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn calibration_capture_replacement_and_deletion_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = tracker_yaml(dir.path(), true);
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#
        )
        .await
        .0,
        200
    );
    // Recapture T1 (replacement) and delete stale T2 profile after reassigning T2.
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":["T1"]}"#,
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T2/assignment",
            r#"{"segment":"head"}"#
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":["T2"]}"#,
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(addr, "DELETE", "calibration/mounting-T2-left_thigh", "")
            .await
            .0,
        200
    );

    // Declared simulator camera profiles must never be persisted to runtime.yaml.
    let snapshot_text = std::fs::read_to_string(runtime_snapshot_path(dir.path())).unwrap();
    assert!(!snapshot_text.contains("camera-intrinsics-CAM1"));
    assert!(!snapshot_text.contains("camera-extrinsics-CAM1"));
    engine.shutdown().await.unwrap();

    let restarted = start(config::parse(&yaml).unwrap()).await.unwrap();
    let state = request(restarted.bound_addr(), "GET", "state", "").await.1;
    assert_eq!(
        calibration_state(&state, "mounting-T1-pelvis").as_deref(),
        Some("valid")
    );
    assert_eq!(
        calibration_state(&state, "mounting-T2-head").as_deref(),
        Some("valid")
    );
    assert_eq!(calibration_state(&state, "mounting-T2-left_thigh"), None);
    assert_eq!(
        calibration_state(&state, "camera-intrinsics-CAM1").as_deref(),
        Some("valid")
    );
    restarted.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_migration_retains_valid_state_handles_invalid_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("state")).unwrap();
    std::fs::create_dir_all(dir.path().join("calibration")).unwrap();
    std::fs::write(
        dir.path().join("state/assignments.yaml"),
        "T1: null\nT2: head\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("state/calibration-states.json"),
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "stale": ["mounting-T1-pelvis"]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calibration/mounting-T1-pelvis.json"),
        serde_json::to_vec(&mounting_profile_json("mounting-T1-pelvis", "T1", "pelvis")).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calibration/mounting-T2-head.json"),
        serde_json::to_vec(&mounting_profile_json("mounting-T2-head", "T2", "head")).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("calibration/broken.json"), b"{corrupt").unwrap();

    let yaml = tracker_yaml(dir.path(), false);
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    let state = request(addr, "GET", "state", "").await.1;
    assert_eq!(device_assignment(&state, "T1"), None);
    assert_eq!(device_assignment(&state, "T2").as_deref(), Some("head"));
    assert_eq!(
        calibration_state(&state, "mounting-T1-pelvis").as_deref(),
        Some("stale")
    );
    assert_eq!(
        calibration_state(&state, "mounting-T2-head").as_deref(),
        Some("valid")
    );
    assert_eq!(
        calibration_state(&state, "broken").as_deref(),
        Some("invalid")
    );

    // Delete migrated valid profile, and verify stray legacy files cannot resurrect or conflict.
    assert_eq!(
        request(addr, "DELETE", "calibration/mounting-T2-head", "")
            .await
            .0,
        200
    );
    engine.shutdown().await.unwrap();

    std::fs::write(
        dir.path().join("state/assignments.yaml"),
        "T1: pelvis\nT2: left_thigh\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calibration/mounting-T2-head.json"),
        serde_json::to_vec(&mounting_profile_json("mounting-T2-head", "T2", "head")).unwrap(),
    )
    .unwrap();

    let restarted = start(config::parse(&yaml).unwrap()).await.unwrap();
    let state = request(restarted.bound_addr(), "GET", "state", "").await.1;
    assert_eq!(device_assignment(&state, "T1"), None);
    assert_eq!(device_assignment(&state, "T2").as_deref(), Some("head"));
    assert_eq!(calibration_state(&state, "mounting-T2-head"), None);
    assert_eq!(
        calibration_state(&state, "broken").as_deref(),
        Some("invalid")
    );
    assert_eq!(
        request(restarted.bound_addr(), "DELETE", "calibration/broken", "")
            .await
            .0,
        200
    );
    restarted.shutdown().await.unwrap();

    // Failed migration must leave original legacy files intact and not create runtime.yaml.
    let failed_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(failed_dir.path().join("state")).unwrap();
    std::fs::create_dir_all(failed_dir.path().join("calibration")).unwrap();
    let orig_assign = b"T1: pelvis\nT2: head\n";
    let orig_states = serde_json::to_vec(&json!({ "schema_version": 1, "stale": [] })).unwrap();
    let orig_prof =
        serde_json::to_vec(&mounting_profile_json("mounting-T2-head", "T2", "head")).unwrap();
    std::fs::write(
        failed_dir.path().join("state/assignments.yaml"),
        orig_assign,
    )
    .unwrap();
    std::fs::write(
        failed_dir.path().join("state/calibration-states.json"),
        &orig_states,
    )
    .unwrap();
    std::fs::write(
        failed_dir.path().join("calibration/mounting-T2-head.json"),
        &orig_prof,
    )
    .unwrap();
    inject_persist_failure(failed_dir.path());
    assert!(
        start(config::parse(&tracker_yaml(failed_dir.path(), false)).unwrap())
            .await
            .is_err()
    );
    assert!(!runtime_snapshot_path(failed_dir.path()).exists());
    assert_eq!(
        std::fs::read(failed_dir.path().join("state/assignments.yaml")).unwrap(),
        orig_assign
    );
    assert_eq!(
        std::fs::read(failed_dir.path().join("state/calibration-states.json")).unwrap(),
        orig_states
    );
    assert_eq!(
        std::fs::read(failed_dir.path().join("calibration/mounting-T2-head.json")).unwrap(),
        orig_prof
    );
}

#[test]
fn corrupted_runtime_and_legacy_state_are_rejected_or_handled_conservatively() {
    let skeleton = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let initial = BTreeMap::from([(
        DeviceId::new("T1").unwrap(),
        SegmentId::new("pelvis").unwrap(),
    )]);

    // Corrupted runtime.yaml is rejected.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("state")).unwrap();
    std::fs::write(runtime_snapshot_path(dir.path()), b":::not valid yaml").unwrap();
    assert!(load_runtime(dir.path(), &initial, &skeleton).is_err());

    // Wrong schema_version in runtime.yaml is rejected.
    std::fs::write(
        runtime_snapshot_path(dir.path()),
        "schema_version: 99\nassignments: {}\nstale_calibrations: []\ncalibration_profiles: []\n",
    )
    .unwrap();
    assert!(load_runtime(dir.path(), &initial, &skeleton).is_err());

    // Corrupted legacy calibration-states.json conservatively marks migrated profiles stale.
    let legacy = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(legacy.path().join("state")).unwrap();
    std::fs::create_dir_all(legacy.path().join("calibration")).unwrap();
    std::fs::write(legacy.path().join("state/calibration-states.json"), b"{bad").unwrap();
    std::fs::write(
        legacy.path().join("calibration/mounting-T1-pelvis.json"),
        serde_json::to_vec(&mounting_profile_json("mounting-T1-pelvis", "T1", "pelvis")).unwrap(),
    )
    .unwrap();
    let (store, _) = load_runtime(legacy.path(), &initial, &skeleton).unwrap();
    let id = ProfileId::new("mounting-T1-pelvis").unwrap();
    assert!(store.stale.contains(&id));
    assert!(
        store
            .warnings
            .iter()
            .any(|w| w.code == "calibration_state_invalid")
    );
    // Reloading from the migrated runtime.yaml keeps it stale.
    let (reloaded, _) = load_runtime(legacy.path(), &initial, &skeleton).unwrap();
    assert!(reloaded.stale.contains(&id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recording_context_only_captures_committed_state_and_replays_it() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = tracker_yaml(dir.path(), false);
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#
        )
        .await
        .0,
        200
    );
    let started = request(addr, "POST", "recording/start", "{}").await;
    assert_eq!(started.0, 200);

    // Failed assignment must NOT appear in the recording context stream.
    inject_persist_failure(dir.path());
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T1/assignment",
            r#"{"segment":"chest"}"#
        )
        .await
        .0,
        503
    );

    // Committed assignment MUST appear in the recording context stream.
    assert_eq!(
        request(
            addr,
            "PUT",
            "devices/T2/assignment",
            r#"{"segment":"head"}"#
        )
        .await
        .0,
        200
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let stopped = request(addr, "POST", "recording/stop", "{}").await;
    assert_eq!(stopped.0, 200);
    let session_id = SessionId::new(stopped.1["data"]["id"].as_str().unwrap()).unwrap();
    engine.shutdown().await.unwrap();

    let recording = reader::load(dir.path(), &session_id).unwrap();
    let t1 = DeviceId::new("T1").unwrap();
    let t2 = DeviceId::new("T2").unwrap();
    assert_eq!(
        recording.initial.assignments.get(&t1),
        Some(&SegmentId::new("pelvis").unwrap())
    );
    assert_eq!(
        recording.initial.assignments.get(&t2),
        Some(&SegmentId::new("left_thigh").unwrap())
    );
    let contexts: Vec<_> = recording
        .events
        .iter()
        .filter_map(|e| match &e.data {
            RecordedData::Context(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(contexts.len(), 1);
    assert_eq!(
        contexts[0].assignments.get(&t1),
        Some(&SegmentId::new("pelvis").unwrap())
    );
    assert_eq!(
        contexts[0].assignments.get(&t2),
        Some(&SegmentId::new("head").unwrap())
    );
    assert!(
        contexts[0]
            .stale
            .contains(&ProfileId::new("mounting-T2-left_thigh").unwrap())
    );

    let mut replay = Replay::new(recording, 1.0, false).unwrap();
    while !replay.paused {
        replay.advance(10_000).unwrap();
    }
    assert_eq!(
        replay.registry.assignments.get(&t2),
        Some(&SegmentId::new("head").unwrap())
    );
    assert!(
        replay
            .store
            .stale
            .contains(&ProfileId::new("mounting-T2-left_thigh").unwrap())
    );
}
