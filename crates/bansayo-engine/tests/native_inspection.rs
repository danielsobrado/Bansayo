//! End-to-end native tracker diagnostics, mounting reference invalidation and solver ablation.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "support/training.rs"]
mod support;

use bansayo_engine::sources::esp32::{PACKET_LEN, PACKET_V2_LEN, Packet, VERSION_V2, encode};
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use support::{engine, http, ok};

async fn wait_for(addr: SocketAddr, condition: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let state = ok(addr, "GET", "state", Value::Null).await;
        if condition(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "state did not converge: {state}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn mounting_state(state: &Value, expected: &str) -> bool {
    state["devices"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|device| device["descriptor"]["id"] == "MOSAICO")
        .flat_map(|device| device["calibration"].as_array().unwrap())
        .any(|profile| profile["kind"] == "mounting" && profile["state"] == expected)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reference_change_retires_mounting_and_solver_preview_uses_only_selected_inputs() {
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 2
engine: {{ publish_rate_hz: 60, stale_after_ms: 250, data_dir: {data:?} }}
api: {{ bind: "127.0.0.1:0" }}
tracking: {{ mode: imu_only }}
sources:
  - id: mosaico
    kind: esp32_udp
    bind: "127.0.0.1:{port}"
    rate_hz: 100
    trackers:
      - {{ id: MOSAICO, tracker: "AA:BB:CC:DD:EE:01/0" }}
assignments:
  MOSAICO: right_shin
"#,
        data = dir.path().display().to_string()
    );
    let handle = engine(&yaml).await;
    let addr = handle.bound_addr();
    let generation = Arc::new(AtomicU64::new(101));
    let sender_ref = generation.clone();
    let sender = tokio::spawn(async move {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut seq = 0;
        loop {
            interval.tick().await;
            seq += 1;
            let packet = Packet {
                mac: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 1],
                sensor: 0,
                status: 0,
                seq,
                device_time_us: start.elapsed().as_micros() as u32,
                q: [0.0, 0.0, 0.0, 1.0],
                gyro: Some([0.0; 3]),
                accel: Some([0.0, 9.80665, 0.0]),
                battery_pct: Some(80),
                rssi_dbm: Some(-45),
            };
            let mut bytes = [0_u8; PACKET_V2_LEN];
            bytes[..PACKET_LEN].copy_from_slice(&encode(&packet));
            bytes[4] = VERSION_V2;
            bytes[64..].copy_from_slice(&sender_ref.load(Ordering::Relaxed).to_le_bytes());
            socket.send_to(&bytes, ("127.0.0.1", port)).await.unwrap();
        }
    });
    wait_for(addr, |state| {
        state["devices"][0]["stream"]["measured_rate_hz"]
            .as_f64()
            .is_some_and(|rate| rate > 50.0)
    })
    .await;
    let capture = ok(
        addr,
        "POST",
        "calibration/reference-pose",
        json!({"device_ids": ["MOSAICO"]}),
    )
    .await;
    assert_eq!(capture[0]["kind"], "mounting");
    let persisted = std::fs::read_to_string(
        bansayo_engine::registry::persistence::runtime_snapshot_path(dir.path()),
    )
    .unwrap();
    let snapshot: bansayo_engine::registry::persistence::RuntimeSnapshot =
        serde_saphyr::from_str(&persisted).unwrap();
    assert!(snapshot.calibration_profiles.iter().any(|p| {
        p.subject
            .device_id
            .as_ref()
            .is_some_and(|id| id.as_str() == "MOSAICO")
            && p.origin == bansayo_contracts::Origin::Live
    }));
    wait_for(addr, |state| mounting_state(state, "valid")).await;
    let trace = ok(addr, "GET", "devices/MOSAICO/inspection", Value::Null).await;
    assert!(trace["reference"].as_u64().unwrap() > 0);
    assert!(
        trace["channels"]["gyro"]["points"]
            .as_array()
            .unwrap()
            .len()
            > 2
    );
    let start = ok(addr, "GET", "devices/MOSAICO/inspection/batch", Value::Null).await;
    let epoch = start["epoch"].as_u64().unwrap();
    let cursor = start["next"].as_u64().unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let path = format!("devices/MOSAICO/inspection/batch?epoch={epoch}&after={cursor}");
    let batch = ok(addr, "GET", &path, Value::Null).await;
    assert_eq!(batch["lost"], 0);
    assert_eq!(batch["reset"], false);
    assert!(
        batch["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["channel"] == "gyro")
            .count()
            > 15
    );
    let recording = ok(
        addr,
        "POST",
        "inspection/recording",
        json!({"device_ids":["MOSAICO"],"duration_s":30}),
    )
    .await;
    assert_eq!(recording["running"], true);
    tokio::time::sleep(Duration::from_millis(350)).await;
    ok(addr, "POST", "inspection/recording/stop", Value::Null).await;
    let deadline = Instant::now() + Duration::from_secs(5);
    let finished = loop {
        let s = ok(addr, "GET", "inspection/recording", Value::Null).await;
        if s["running"] == false {
            break s;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(finished["observations"].as_u64().unwrap() > 0);
    let file = dir.path().join(finished["file"].as_str().unwrap());
    assert_eq!(file.extension().unwrap(), "jsonl");
    let contents = std::fs::read_to_string(file).unwrap();
    let rows: Vec<Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.first().unwrap()["type"], "header");
    assert_eq!(rows.last().unwrap()["type"], "footer");
    let preview = ok(
        addr,
        "POST",
        "inspection/solver",
        json!({"device_ids": ["MOSAICO"]}),
    )
    .await;
    assert_eq!(preview["selected"], json!(["MOSAICO"]));
    assert_eq!(preview["imu"]["quality"]["observed_segments"], 1);
    assert!(preview["camera"]["pose"].is_null());
    assert_eq!(
        http(
            addr,
            "POST",
            "inspection/solver",
            json!({"device_ids": ["UNKNOWN"]}),
        )
        .await
        .0,
        400
    );

    generation.store(102, Ordering::Relaxed);
    wait_for(addr, |state| mounting_state(state, "stale")).await;
    let reset = ok(addr, "GET", &path, Value::Null).await;
    assert_eq!(reset["reset"], true);
    let new_trace = ok(addr, "GET", "devices/MOSAICO/inspection", Value::Null).await;
    assert_ne!(new_trace["reference"], trace["reference"]);
    let preview = ok(
        addr,
        "POST",
        "inspection/solver",
        json!({"device_ids": ["MOSAICO"]}),
    )
    .await;
    assert_eq!(preview["imu"]["quality"]["observed_segments"], 0);
    sender.abort();
    handle.shutdown().await.unwrap();
}
