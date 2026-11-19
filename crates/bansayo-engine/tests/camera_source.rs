//! Physical-camera source plumbing without model weights or hardware: image-folder cameras,
//! calibration import, persistence and the preview endpoint. Inference itself fails here (the
//! model files are empty placeholders), which also proves a failing detector cannot stop the
//! engine or its API.
use bansayo_engine::config;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::SocketAddr,
    path::Path,
    time::Duration,
};

async fn request(addr: SocketAddr, method: &str, path: &str, body: &str) -> (u16, Value) {
    let (method, path, body) = (method.to_owned(), path.to_owned(), body.to_owned());
    tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(addr).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(socket, "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        let status = response[9..12].parse().unwrap();
        let body = response.split_once("\r\n\r\n").unwrap().1;
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

fn yaml(dir: &Path) -> String {
    let models = dir.join("models");
    let frames = dir.join("frames");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::create_dir_all(&frames).unwrap();
    for model in ["rtmpose-m.onnx", "yolox-m-humanart.onnx"] {
        std::fs::write(models.join(model), b"").unwrap();
    }
    image::RgbImage::new(64, 48)
        .save(frames.join("0001.png"))
        .unwrap();
    format!(
        r#"schema_version: 1
engine: {{ data_dir: {data:?} }}
api: {{ bind: "127.0.0.1:0" }}
sources:
  - id: cams
    kind: camera
    models_dir: {models:?}
    cameras:
      - {{ id: CAM1, frames_dir: {frames:?}, rate_hz: 10 }}
      - {{ id: CAM2, frames_dir: {frames:?}, rate_hz: 10 }}
"#,
        data = dir.join("data").display().to_string(),
        models = models.display().to_string(),
        frames = frames.display().to_string(),
    )
}

fn profile(kind: &str, device: &str, data: Value) -> Value {
    json!({
        "schema_version": 1,
        "id": format!("camera-{kind}-{device}"),
        "kind": format!("camera_{kind}"),
        "method": "checkerboard_opencv",
        "origin": "live",
        "created_utc": "2026-11-17T00:00:00Z",
        "subject": { "device_id": device, "segment_id": null },
        "depends_on": [],
        "data": data,
        "quality": { "rms_px": 0.3 }
    })
}

fn intrinsics(device: &str, fx: f64) -> Value {
    profile(
        "intrinsics",
        device,
        json!({ "image_size_px": [64, 48], "fx_px": fx, "fy_px": fx, "cx_px": 32.0, "cy_px": 24.0, "distortion": [0.0, 0.0, 0.0, 0.0, 0.0] }),
    )
}

fn extrinsics(device: &str, x: f64) -> Value {
    profile(
        "extrinsics",
        device,
        json!({ "q_world_from_camera": [0.0, 0.0, 0.0, 1.0], "t_world_from_camera_m": [x, 1.2, 3.0] }),
    )
}

fn camera_state(state: &Value, device: &str) -> Vec<(String, String)> {
    state["data"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["descriptor"]["id"] == device)
        .unwrap()["calibration"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["kind"].as_str().unwrap().to_owned(),
                c["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn camera_profiles_import_validate_persist_and_survive_a_failing_detector() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = yaml(dir.path());
    let engine = bansayo_engine::start(config::parse(&yaml).unwrap())
        .await
        .unwrap();
    let addr = engine.bound_addr();

    let (status, state) = request(addr, "GET", "state", "").await;
    assert_eq!(status, 200);
    let cam1 = state["data"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["descriptor"]["id"] == "CAM1")
        .unwrap()
        .clone();
    assert_eq!(cam1["descriptor"]["origin"], "fixture");
    assert_eq!(cam1["descriptor"]["transport"], "file");
    assert_eq!(
        cam1["descriptor"]["camera"]["resolution_px"],
        json!([64, 48])
    );
    assert_eq!(
        cam1["descriptor"]["camera"]["landmark_schema"]["id"],
        "coco17.body"
    );

    // Rejections: unknown device, wrong id, extrinsics without intrinsics, bad data.
    let mut renamed = intrinsics("CAM1", 50.0);
    renamed["id"] = json!("my-profile");
    for (body, text) in [
        (json!([intrinsics("NOPE", 50.0)]), "not a configured camera"),
        (
            Value::Array(vec![renamed]),
            "id camera-intrinsics-CAM1",
        ),
        (json!([extrinsics("CAM1", 0.0)]), "need its intrinsics"),
        (json!([intrinsics("CAM1", -5.0)]), "invalid or non-finite"),
        (json!([]), "at least one"),
    ] {
        let (status, response) =
            request(addr, "POST", "calibration/profiles", &body.to_string()).await;
        assert_eq!(status, 400, "{response}");
        let message = response["error"]["message"].as_str().unwrap();
        assert!(message.contains(text), "{message}");
    }

    let pair = json!([
        intrinsics("CAM1", 50.0),
        extrinsics("CAM1", 0.0),
        intrinsics("CAM2", 50.0),
        extrinsics("CAM2", 1.0)
    ]);
    let (status, response) = request(addr, "POST", "calibration/profiles", &pair.to_string()).await;
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["data"].as_array().unwrap().len(), 4);
    // New intrinsics alone would orphan the extrinsics solved with the old ones.
    let (status, response) = request(
        addr,
        "POST",
        "calibration/profiles",
        &json!([intrinsics("CAM1", 60.0)]).to_string(),
    )
    .await;
    assert_eq!(status, 400);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("need new extrinsics")
    );

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let (_, state) = request(addr, "GET", "state", "").await;
    assert!(
        camera_state(&state, "CAM1")
            .iter()
            .all(|(_, s)| s == "valid"),
        "{:?}",
        camera_state(&state, "CAM1")
    );
    let triangulation = state["data"]["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "landmarks_3d_triangulated")
        .unwrap();
    assert_ne!(
        triangulation["reasons"][0]["code"], "camera_calibration_missing",
        "{triangulation}"
    );
    engine.shutdown().await.unwrap();

    // Profiles survive a restart.
    let engine = bansayo_engine::start(config::parse(&yaml).unwrap())
        .await
        .unwrap();
    let (_, state) = request(engine.bound_addr(), "GET", "state", "").await;
    let cam2 = camera_state(&state, "CAM2");
    assert_eq!(cam2.len(), 2, "{cam2:?}");
    assert!(cam2.iter().all(|(_, s)| s == "valid"), "{cam2:?}");
    engine.shutdown().await.unwrap();
}

/// A standing person in COCO-17 order on a 64×48 image, pixels.
fn standing_row() -> String {
    let p: [[f32; 2]; 17] = [
        [32.0, 6.0],
        [33.0, 5.0],
        [31.0, 5.0],
        [34.0, 6.0],
        [30.0, 6.0],
        [36.0, 11.0],
        [28.0, 11.0],
        [37.0, 17.0],
        [27.0, 17.0],
        [37.0, 22.0],
        [27.0, 22.0],
        [35.0, 24.0],
        [29.0, 24.0],
        [35.0, 33.0],
        [29.0, 33.0],
        [35.0, 42.0],
        [29.0, 42.0],
    ];
    serde_json::to_string(&p.map(|[x, y]| [x, y, 0.9])).unwrap()
}

/// 08 §3.2 / D2: a requested setting the backend cannot apply makes the camera `degraded` with a
/// reason naming the property; the camera keeps delivering; settings are changed over HTTP.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unapplied_camera_settings_degrade_the_camera_with_the_property_named() {
    let dir = tempfile::tempdir().unwrap();
    let frames = dir.path().join("frames");
    std::fs::create_dir_all(&frames).unwrap();
    let mut rows = String::new();
    for i in 0..10 {
        image::RgbImage::new(64, 48)
            .save(frames.join(format!("{i:04}.png")))
            .unwrap();
        rows.push_str(&standing_row());
        rows.push('\n');
    }
    std::fs::write(frames.join("landmarks.jsonl"), rows).unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{ data_dir: {data:?} }}
api: {{ bind: "127.0.0.1:0" }}
sources:
  - id: cams
    kind: camera
    models_dir: {models:?}
    cameras:
      - {{ id: CAM1, frames_dir: {frames:?}, rate_hz: 20, exposure_s: 0.004 }}
"#,
        data = dir.path().join("data").display().to_string(),
        models = dir.path().join("no-models").display().to_string(),
        frames = frames.display().to_string(),
    );
    let engine = bansayo_engine::start(config::parse(&yaml).unwrap())
        .await
        .unwrap();
    let addr = engine.bound_addr();
    let cam = |state: &Value| {
        state["data"]["devices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["descriptor"]["id"] == "CAM1")
            .unwrap()
            .clone()
    };
    let mut status = Value::Null;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        status = cam(&request(addr, "GET", "state", "").await.1);
        if status["state"] == "degraded" {
            break;
        }
    }
    assert_eq!(status["state"], "degraded", "{status}");
    assert_eq!(status["reasons"][0]["code"], "camera_setting_not_applied");
    assert_eq!(status["reasons"][0]["subject"], "exposure_s");
    assert!(
        status["camera_settings"]["exposure_s"].is_null(),
        "{status}"
    );
    assert!(status["stream"]["received"].as_u64().unwrap() > 0);

    let (code, body) = request(addr, "PUT", "cameras/CAM1/settings", r#"{"gain": 4}"#).await;
    assert_eq!(code, 200, "{body}");
    let subjects: Vec<&str> = body["data"]["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["subject"].as_str())
        .collect();
    assert_eq!(subjects, ["exposure_s", "gain"], "{body}");
    for (body, want) in [
        (r#"{"resolution_px": [640, 480]}"#, 400),
        (r#"{"exposure_s": 5}"#, 400),
    ] {
        assert_eq!(
            request(addr, "PUT", "cameras/CAM1/settings", body).await.0,
            want
        );
    }
    assert_eq!(
        request(addr, "PUT", "cameras/NOPE/settings", r#"{"gain": 1}"#)
            .await
            .0,
        404
    );
    engine.shutdown().await.unwrap();
}
