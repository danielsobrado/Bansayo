//! RTSP cameras in the configuration (ADR-020 item 7): an `rtsp://` URL is one more way to give a
//! camera; exposure belongs to the camera's own app; the password never reaches the descriptor.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use bansayo_engine::config;

fn yaml(dir: &std::path::Path, camera: &str) -> String {
    let models = dir.join("models");
    std::fs::create_dir_all(&models).unwrap();
    for m in ["rtmpose-m.onnx", "yolox-m-humanart.onnx"] {
        std::fs::write(models.join(m), b"").unwrap();
    }
    format!(
        "schema_version: 1\nengine: {{ data_dir: {data:?} }}\nsources:\n  - id: cams\n    kind: camera\n    models_dir: {models:?}\n    cameras:\n      - {camera}\n",
        data = dir.join("data").display().to_string(),
        models = models.display().to_string(),
    )
}

#[test]
fn rtsp_cameras_are_validated_and_described_without_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let url = "rtsp://admin:s3cret@192.168.1.40:554/h264Preview_01_sub";
    let ok = config::parse(&yaml(
        dir.path(),
        &format!("{{ id: REOLINK1, rtsp: {url:?}, resolution_px: [640, 360], rate_hz: 15 }}"),
    ))
    .unwrap();
    let source = bansayo_engine::sources::camera::CameraSource {
        spec: ok.cameras[0].clone(),
        previews: Default::default(),
        fiducials: Default::default(),
        settings: Default::default(),
        timing: bansayo_engine::diagnostics::Timing::new(false, 16_667),
        feeds: Default::default(),
        data_dir: dir.path().into(),
        runtime: Default::default(),
        management: Default::default(),
    };
    let d = &source.descriptors()[0];
    assert_eq!(d.model.as_deref(), Some("RTSP camera 192.168.1.40:554"));
    assert_eq!(d.transport, bansayo_contracts::Transport::Rtsp);
    assert_eq!(d.origin, bansayo_contracts::Origin::Live);
    assert!(!format!("{d:?}").contains("s3cret"));
    for (camera, text) in [
        (
            format!("{{ id: C, rtsp: {url:?}, exposure_s: 0.004 }}"),
            "camera's own app",
        ),
        (
            "{ id: C, rtsp: \"http://192.168.1.40/snap.jpg\" }".to_owned(),
            "must start with rtsp://",
        ),
        (
            format!("{{ id: C, rtsp: {url:?}, network: true }}"),
            "exactly one of",
        ),
        (
            format!("{{ id: C, rtsp: {url:?}, device: 0 }}"),
            "no device or frames_dir",
        ),
    ] {
        let err = config::parse(&yaml(dir.path(), &camera))
            .unwrap_err()
            .to_string();
        assert!(err.contains(text), "{camera}: {err}");
    }
}
