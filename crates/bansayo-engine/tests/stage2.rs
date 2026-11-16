use bansayo_engine::{config, start};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::Duration;

async fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
    extra: &str,
) -> (u16, serde_json::Value) {
    let method = method.to_owned();
    let path = path.to_owned();
    let body = body.to_owned();
    let extra = extra.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(addr).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(socket, "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}", body.len()).unwrap();
        let mut response = String::new(); socket.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        (headers.split_whitespace().nth(1).unwrap().parse().unwrap(), serde_json::from_str(body).unwrap())
    }).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn assignments_faults_and_record_replay_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{data_dir: {:?}}}
api: {{bind: "127.0.0.1:0"}}
assignments: {{T1: pelvis,T2: left_thigh,T3: left_shin}}
sources:
  - id: sim
    kind: simulated
    motion: reference_pose
    trackers:
      - {{id: T1,rate_hz: 100}}
      - {{id: T2,rate_hz: 100}}
      - {{id: T3,rate_hz: 100}}
"#,
        dir.path().display().to_string()
    );
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#,
            ""
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
            r#"{"segment":"head"}"#,
            ""
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
            r#"{"segment":"left_thigh"}"#,
            ""
        )
        .await
        .0,
        200
    );
    let state = request(addr, "GET", "state", "", "").await.1;
    assert!(
        state["data"]["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["state"] == "stale")
    );
    assert_eq!(
        request(
            addr,
            "POST",
            "simulation/devices/T3/fault",
            r#"{"action":"disconnect"}"#,
            ""
        )
        .await
        .1["data"]["state"],
        "disconnected"
    );
    assert_eq!(
        request(
            addr,
            "POST",
            "simulation/devices/T3/fault",
            r#"{"action":"reconnect"}"#,
            ""
        )
        .await
        .0,
        200
    );
    let recording = request(
        addr,
        "POST",
        "recording/start",
        r#"{"name":"Regression recording"}"#,
        "",
    )
    .await;
    assert_eq!(recording.0, 200, "{:?}", recording.1);
    assert_eq!(
        request(addr, "POST", "recording/start", "{}", "").await.0,
        409
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    let stopped = request(addr, "POST", "recording/stop", "{}", "").await;
    assert_eq!(stopped.0, 200, "{:?}", stopped.1);
    let id = stopped.1["data"]["id"].as_str().unwrap();
    let bytes = std::fs::read(dir.path().join("sessions").join(id).join("recording.mcap")).unwrap();
    assert!(bytes.starts_with(b"\x89MCAP0\r\n"));
    let recording = bansayo_engine::recording::reader::load(
        dir.path(),
        &bansayo_contracts::SessionId::new(id).unwrap(),
    )
    .unwrap();
    let replay_frames = |step| {
        let mut replay =
            bansayo_engine::recording::replay::Replay::new(recording.clone(), 1.0, false).unwrap();
        let mut frames = Vec::new();
        while !replay.paused {
            frames.extend(replay.advance(step).unwrap());
        }
        frames
    };
    let first = replay_frames(10_000);
    let second = replay_frames(37_123);
    assert_eq!(first, second);
    assert!(first.len() > 5);
    let body = serde_json::json!({"session_id":id,"looping":false}).to_string();
    assert_eq!(
        request(addr, "POST", "playback/start", &body, "").await.0,
        200
    );
    assert_eq!(
        request(addr, "POST", "recording/start", "{}", "").await.0,
        409
    );
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":null}"#,
            ""
        )
        .await
        .0,
        409
    );
    assert_eq!(
        request(
            addr,
            "PUT",
            "playback",
            r#"{"paused":true,"position_us":100000}"#,
            ""
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(addr, "POST", "playback/stop", "{}", "").await.0,
        200
    );
    assert_eq!(
        request(
            addr,
            "POST",
            "playback/start",
            r#"{"session_id":"../../outside"}"#,
            ""
        )
        .await
        .0,
        400
    );
    engine.shutdown().await.unwrap();
}
