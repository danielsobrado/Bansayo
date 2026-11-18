//! LAN mode (ADR-031, ADR-042, 17 §5): a non-loopback bind needs `api.lan.enabled`; then a
//! request from another machine needs the bearer token, the pairing link is shown only on the
//! engine's machine, and a phone camera streams frames to its ingest socket.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use bansayo_engine::config;
use futures_util::SinkExt;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::Duration;

fn request(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, Value) {
    let mut s = std::net::TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let auth = token.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
    write!(
        s,
        "GET /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n"
    )
    .unwrap();
    let mut r = String::new();
    s.read_to_string(&mut r).unwrap();
    let status = r[9..12].parse().unwrap();
    let body = r.split_once("\r\n\r\n").unwrap().1;
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

fn yaml(dir: &std::path::Path, bind: &str, lan: bool) -> String {
    let models = dir.join("models");
    std::fs::create_dir_all(&models).unwrap();
    for m in ["rtmpose-m.onnx", "yolox-m-humanart.onnx"] {
        std::fs::write(models.join(m), b"").unwrap();
    }
    format!(
        r#"schema_version: 1
engine: {{ data_dir: {data:?} }}
api: {{ bind: "{bind}", lan: {{ enabled: {lan} }} }}
sources:
  - id: phones
    kind: camera
    models_dir: {models:?}
    cameras:
      - {{ id: PHONE1, network: true, resolution_px: [64, 48], rate_hz: 30 }}
"#,
        data = dir.join("data").display().to_string(),
        models = models.display().to_string(),
    )
}

#[test]
fn a_network_bind_without_lan_mode_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let err = config::parse(&yaml(dir.path(), "0.0.0.0:0", false)).unwrap_err();
    assert!(err.to_string().contains("api.lan.enabled"), "{err}");
    assert!(config::parse(&yaml(dir.path(), "127.0.0.1:0", false)).is_ok());
    let network = yaml(dir.path(), "127.0.0.1:0", false)
        .replace("network: true,", "network: true, exposure_s: 0.004,");
    assert!(
        config::parse(&network)
            .unwrap_err()
            .to_string()
            .contains("set on the phone")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lan_mode_needs_the_token_from_other_machines() {
    let dir = tempfile::tempdir().unwrap();
    let engine =
        bansayo_engine::start(config::parse(&yaml(dir.path(), "0.0.0.0:0", true)).unwrap())
            .await
            .unwrap();
    let port = engine.bound_addr().port();
    let local: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    // The desktop on this machine: no token needed, and it may read the pairing link.
    let (status, pairing) =
        tokio::task::spawn_blocking(move || request(local, "lan/pairing", None))
            .await
            .unwrap();
    assert_eq!(status, 200, "{pairing}");
    let token = pairing["data"]["token"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 32);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("data/lan-token")).unwrap(),
        token
    );
    let Some(host) = pairing["data"]["host"].as_str().map(str::to_owned) else {
        eprintln!("no LAN address on this machine; network half skipped");
        engine.shutdown().await.unwrap();
        return;
    };
    assert_eq!(
        pairing["data"]["uri"],
        format!("bansayo://pair?host={host}&port={port}&token={token}")
    );
    // Another machine (this PC reached through its LAN address): token or 401.
    let lan: SocketAddr = format!("{host}:{port}").parse().unwrap();
    let t = token.clone();
    let (no, with, wrong, pair) = tokio::task::spawn_blocking(move || {
        (
            request(lan, "state", None),
            request(lan, "state", Some(&t)),
            request(lan, "state", Some("0123456789abcdef0123456789abcdef")),
            request(lan, "lan/pairing", Some(&t)),
        )
    })
    .await
    .unwrap();
    assert_eq!(no.0, 401, "{}", no.1);
    assert_eq!(no.1["error"]["code"], "unauthorized");
    assert_eq!(with.0, 200);
    assert_eq!(wrong.0, 401);
    assert_eq!(
        pair.0, 403,
        "pairing is answered on the engine's machine only"
    );
    // The phone camera: the ingest socket needs the token too, then accepts frames.
    let url = format!("ws://{lan}/api/v1/ingest/camera/PHONE1");
    assert!(
        tokio_tungstenite::connect_async(url.as_str())
            .await
            .is_err()
    );
    let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
        url.as_str(),
    )
    .unwrap();
    req.headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
        .encode_image(&image::RgbImage::new(64, 48))
        .unwrap();
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(jpeg.into()))
        .await
        .unwrap();
    ws.close(None).await.unwrap();
    let missing = format!("ws://{lan}/api/v1/ingest/camera/NOPE");
    let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
        missing.as_str(),
    )
    .unwrap();
    req.headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    assert!(tokio_tungstenite::connect_async(req).await.is_err());
    engine.shutdown().await.unwrap();
}
