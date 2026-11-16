use bansayo_contracts::{ApiResponse, CalibrationProfileSummary, ServerMessage};
use bansayo_engine::{config, start};
use futures_util::StreamExt;
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
async fn calibrated_simulator_streams_live_motion_and_rejects_bad_requests() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{ data_dir: {:?}, publish_rate_hz: 60 }}
api: {{ bind: "127.0.0.1:0" }}
assignments: {{ T1: pelvis, T2: left_thigh, T3: left_shin }}
sources:
  - id: sim
    kind: simulated
    seed: 42
    motion: reference_pose
    trackers:
      - {{ id: T1, rate_hz: 100 }}
      - {{ id: T2, rate_hz: 100 }}
      - {{ id: T3, rate_hz: 100 }}
"#,
        dir.path().display().to_string()
    );
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    assert_eq!(
        request(addr, "GET", "health", "", "").await.1["data"]["status"],
        "ok"
    );
    assert_eq!(
        request(
            addr,
            "GET",
            "health",
            "",
            "Origin: https://foreign.invalid\r\n"
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(addr, "PUT", "simulation", r#"{"motion":"unknown"}"#, "")
            .await
            .0,
        400
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(format!(
        "ws://{addr}/api/v1/stream?topics=frame,state&max_rate_hz=30"
    ))
    .await
    .unwrap();
    let hello: ServerMessage =
        serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert!(matches!(
        hello,
        ServerMessage::Hello {
            protocol_version: 1,
            ..
        }
    ));
    loop {
        let m: ServerMessage =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        if let ServerMessage::Frame {
            pose: Some(p),
            quality,
            ..
        } = m
        {
            assert!(
                p.segments
                    .iter()
                    .all(|s| s.provenance == bansayo_contracts::Provenance::Inferred)
            );
            assert!(
                quality
                    .reasons
                    .iter()
                    .any(|r| r.code == "mounting_calibration_missing")
            );
            break;
        }
    }
    let (status, response) = request(
        addr,
        "POST",
        "calibration/reference-pose",
        r#"{"device_ids":null}"#,
        "",
    )
    .await;
    assert_eq!(status, 200, "{response}");
    let result: ApiResponse<Vec<CalibrationProfileSummary>> =
        serde_json::from_value(response).unwrap();
    assert!(matches!(result, ApiResponse::Ok { data, .. } if data.len()==3));
    assert_eq!(
        request(addr, "PUT", "simulation", r#"{"motion":"squat"}"#, "")
            .await
            .0,
        200
    );
    let mut last = 0;
    let mut moving = false;
    for _ in 0..50 {
        let m: ServerMessage =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        if let ServerMessage::Frame {
            seq,
            pose: Some(p),
            joints,
            ..
        } = m
        {
            assert!(seq > last);
            last = seq;
            moving |= joints[0].value.is_some_and(|v| v > 0.05);
            if moving {
                assert!(
                    p.segments
                        .iter()
                        .any(|s| s.provenance == bansayo_contracts::Provenance::Derived)
                );
                break;
            }
        }
    }
    assert!(moving);
    let (status, response) = request(
        addr,
        "POST",
        "calibration/reference-pose",
        r#"{"device_ids":null}"#,
        "",
    )
    .await;
    assert_eq!(status, 400, "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Movement detected")
    );
    ws.close(None).await.unwrap();
    engine.shutdown().await.unwrap();
    // Successful capture was persisted; a fresh engine loads it.
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    assert_eq!(
        request(engine.bound_addr(), "GET", "state", "", "").await.1["data"]["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["state"] == "valid")
            .count(),
        3
    );
    engine.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn camera_only_stream_has_landmarks_and_no_world_pose() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{ data_dir: {:?} }}
api: {{ bind: "127.0.0.1:0" }}
sources:
  - id: sim
    kind: simulated
    cameras:
      - {{ id: CAM, rate_hz: 30, resolution_px: [1280,720], fov_y_deg: 50, position_m: [0,1.2,3.2], look_at_m: [0,1,0] }}
"#,
        dir.path().display().to_string()
    );
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    assert_eq!(
        request(
            addr,
            "POST",
            "calibration/reference-pose",
            r#"{"device_ids":["../outside"]}"#,
            ""
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(addr, "GET", "stream?topics=unknown", "", "")
            .await
            .0,
        400
    );
    assert_eq!(
        request(addr, "GET", "stream?max_rate_hz=NaN", "", "")
            .await
            .0,
        400
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(format!(
        "ws://{addr}/api/v1/stream?topics=frame,state,landmarks_2d"
    ))
    .await
    .unwrap();
    let mut seen_landmarks = false;
    let mut seen_frame = false;
    while !(seen_landmarks && seen_frame) {
        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let msg: ServerMessage = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        match msg {
            ServerMessage::Frame { pose, joints, .. } => {
                assert!(pose.is_none());
                assert!(joints.iter().all(|j| j.value.is_none()));
                seen_frame = true;
            }
            ServerMessage::Landmarks2d { landmarks } => {
                assert_eq!(landmarks.points.len(), 19);
                assert!(landmarks.points.iter().any(Option::is_some));
                seen_landmarks = true;
            }
            _ => {}
        }
    }
    // Shutdown closes existing sockets without waiting indefinitely for clients.
    tokio::time::timeout(Duration::from_secs(2), engine.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn bundled_config_data_dir_override_happens_before_validation() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("bundled.yaml");
    std::fs::write(
        &config_path,
        "schema_version: 1\nengine: {data_dir: /dev/null/bansayo-readonly}\n",
    )
    .unwrap();
    let runtime = dir.path().join("runtime");
    let validated = config::load_with_data_dir(&config_path, &runtime).unwrap();
    assert_eq!(validated.config.engine.data_dir, runtime);
}
