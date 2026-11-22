//! `GET /api/v1/ingest/xr` over a real socket: handshake, validation, tracking loss, reconnect,
//! single writer, idle timeout, rate bound, replay refusal and recording round trip. No headset.
use bansayo_contracts::*;
use bansayo_engine::{
    EngineHandle, config,
    recording::{
        reader::{self, RecordedData},
        replay::Replay,
    },
    registry::ObservationKind,
    start,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{net::SocketAddr, time::Duration};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, connect_async_with_config,
    tungstenite::{Error as WsError, Message, protocol::frame::coding::CloseCode},
};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
const XR: [&str; 3] = ["XR-HMD", "XR-LEFT-GRIP", "XR-RIGHT-GRIP"];
/// Comfortably below the 120 Hz ingest ceiling.
const FRAME_GAP: Duration = Duration::from_millis(12);

fn yaml(dir: &std::path::Path, xr: &str) -> String {
    format!(
        "schema_version: 2\nengine: {{data_dir: {:?}}}\napi: {{bind: \"127.0.0.1:0\"}}\n{xr}\nsources: []\n",
        dir.display().to_string()
    )
}
async fn engine(dir: &std::path::Path, xr: &str) -> EngineHandle {
    start(config::parse(&yaml(dir, xr)).unwrap()).await.unwrap()
}
const ON: &str = "xr: {enabled: true}";
/// Idle disconnect well beyond the time the test needs to read state after its last frame.
const ON_PATIENT: &str = "xr: {enabled: true, tracking_stale_after_ms: 2000}";

async fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> (u16, Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut r = String::new();
    tokio::time::timeout(Duration::from_secs(10), s.read_to_string(&mut r))
        .await
        .unwrap()
        .unwrap();
    let (h, b) = r.split_once("\r\n\r\n").unwrap();
    let status = h.split_whitespace().nth(1).unwrap().parse().unwrap();
    let start = b.find(['{', '[']).unwrap_or(0);
    let end = b.rfind(['}', ']']).map_or(b.len(), |e| e + 1);
    (
        status,
        serde_json::from_str(&b[start..end]).unwrap_or(Value::Null),
    )
}
/// The XR devices in the engine's state, by id.
async fn xr_devices(addr: SocketAddr) -> Vec<Value> {
    let (_, s) = http(addr, "GET", "state", "").await;
    s["data"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["descriptor"]["kind"] == "xr_tracked_device")
        .cloned()
        .collect()
}
async fn device(addr: SocketAddr, id: &str) -> Value {
    xr_devices(addr)
        .await
        .into_iter()
        .find(|d| d["descriptor"]["id"] == id)
        .unwrap()
}
/// Polls state until `pred` holds for `id`, for at most 5 s.
async fn wait_for(addr: SocketAddr, id: &str, pred: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..100 {
        let d = device(addr, id).await;
        if pred(&d) {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{id} never matched; last: {}", device(addr, id).await);
}
fn received(d: &Value) -> u64 {
    d["stream"]["received"].as_u64().unwrap()
}

/// Nagle off, like the VR client: batched small frames would arrive as bursts and trip the
/// arrival-time rate gate.
async fn connect(addr: SocketAddr) -> Ws {
    connect_async_with_config(format!("ws://{addr}/api/v1/ingest/xr"), None, true)
        .await
        .unwrap()
        .0
}
async fn next_text(ws: &mut Ws) -> Value {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Text(t))) => return serde_json::from_str(&t).unwrap(),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            other => panic!("expected a text message, got {other:?}"),
        }
    }
}
/// Reads until the server closes; returns its close code, if it sent one.
async fn close_code(ws: &mut Ws) -> Option<CloseCode> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(frame))) => return frame.map(|f| f.code),
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return None,
        }
    }
}
async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}
async fn ready(addr: SocketAddr) -> Ws {
    let mut ws = connect(addr).await;
    send(&mut ws, json!({"type": "hello", "protocol_version": 1})).await;
    let r = next_text(&mut ws).await;
    assert_eq!(
        r,
        json!({"type": "ready", "protocol_version": 1, "time_quality": "receive_time"})
    );
    ws
}
fn sample(role: &str, tracked: bool, x: f32) -> Value {
    if tracked {
        json!({"role": role, "tracked": true, "position_m": [x, 1.5, -0.2], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0]})
    } else {
        json!({"role": role, "tracked": false, "position_m": null, "rotation_xyzw": null})
    }
}
fn frame(seq: u64, client_us: u64, right_tracked: bool) -> Value {
    json!({"type": "frame", "seq": seq, "client_time_us": client_us, "samples": [
        sample("head", true, 0.0), sample("left_grip", true, -0.3), sample("right_grip", right_tracked, 0.3)
    ]})
}
/// Sends frames `seqs` at a pace the rate gate accepts.
async fn stream(ws: &mut Ws, seqs: std::ops::Range<u64>, right_tracked: bool) {
    for seq in seqs {
        send(ws, frame(seq, 1_000_000_000 + seq * 16_667, right_tracked)).await;
        tokio::time::sleep(FRAME_GAP).await;
    }
}
/// Streams in the background, like a live client, so state read meanwhile is never stale:
/// `/state` is republished on a state change or once a second, and devices silent for
/// `quality.disconnect_after_ms` (1 s) are disconnected by design.
fn spawn_stream(
    mut ws: Ws,
    seqs: std::ops::Range<u64>,
    right_tracked: fn(u64) -> bool,
) -> tokio::task::JoinHandle<Ws> {
    tokio::spawn(async move {
        for seq in seqs {
            send(
                &mut ws,
                frame(seq, 1_000_000_000 + seq * 16_667, right_tracked(seq)),
            )
            .await;
            tokio::time::sleep(FRAME_GAP).await;
        }
        ws
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_disabled_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), "").await;
    let addr = e.bound_addr();
    assert!(xr_devices(addr).await.is_empty());
    match connect_async(format!("ws://{addr}/api/v1/ingest/xr")).await {
        Err(WsError::Http(r)) => assert_eq!(r.status(), 404),
        other => panic!("expected 404, got {:?}", other.map(|_| ())),
    }
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_announces_three_devices_and_streams_poses() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON_PATIENT).await;
    let addr = e.bound_addr();
    let devices = xr_devices(addr).await;
    let ids: Vec<_> = devices
        .iter()
        .map(|d| d["descriptor"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, XR);
    for d in &devices {
        assert_eq!(d["state"], "acquiring");
        assert_eq!(d["assignment"], Value::Null);
        assert_eq!(d["descriptor"]["origin"], "live");
        assert_eq!(d["descriptor"]["source_id"], "xr-openxr");
    }
    let mut ws = ready(addr).await;
    stream(&mut ws, 0..10, true).await;
    for id in XR {
        wait_for(addr, id, |d| received(d) == 10).await;
    }
    let all = xr_devices(addr).await;
    // One server receive time per frame: all three devices were last seen at the same instant.
    let seen: Vec<_> = all
        .iter()
        .map(|d| d["stream"]["last_seen_t_us"].as_u64().unwrap())
        .collect();
    assert!(seen.iter().all(|t| *t == seen[0]), "{seen:?}");
    for d in &all {
        assert_eq!(d["stream"]["time_quality"], "receive_time");
        assert_eq!(d["stream"]["dropped"], 0);
    }
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_rejects_bad_version_and_missing_hello() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let mut ws = connect(addr).await;
    send(&mut ws, json!({"type": "hello", "protocol_version": 2})).await;
    let err = next_text(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], "xr_protocol_version");
    assert_eq!(close_code(&mut ws).await, Some(CloseCode::Protocol));
    let mut ws = connect(addr).await;
    send(&mut ws, frame(0, 0, true)).await;
    assert_eq!(close_code(&mut ws).await, Some(CloseCode::Protocol));
    // A refused connection released the single writer and sent nothing to the registry.
    let mut ws = ready(addr).await;
    send(&mut ws, json!({"type": "hello", "protocol_version": 1})).await;
    assert_eq!(
        close_code(&mut ws).await,
        Some(CloseCode::Protocol),
        "hello twice"
    );
    for d in xr_devices(addr).await {
        assert_eq!(received(&d), 0);
    }
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_rejects_binary_and_oversized() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let mut ws = ready(addr).await;
    ws.send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    assert_eq!(close_code(&mut ws).await, Some(CloseCode::Unsupported));
    let mut ws = ready(addr).await;
    let huge = json!({"type": "frame", "pad": "x".repeat(5000)});
    let _ = ws.send(Message::Text(huge.to_string().into())).await;
    assert_eq!(close_code(&mut ws).await, Some(CloseCode::Size));
    // The engine is still serving.
    let mut ws = ready(addr).await;
    stream(&mut ws, 0..3, true).await;
    wait_for(addr, "XR-HMD", |d| received(d) == 3).await;
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_rejects_bad_role_order_and_nonfinite_payload() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let mut ws = ready(addr).await;
    let mut swapped = frame(0, 0, true);
    swapped["samples"].as_array_mut().unwrap().swap(1, 2);
    send(&mut ws, swapped).await;
    assert_eq!(next_text(&mut ws).await["code"], "xr_role_order");
    tokio::time::sleep(FRAME_GAP).await;
    let mut huge_norm = frame(0, 0, true);
    huge_norm["samples"][1]["position_m"] = json!([1e30, 0.0, 0.0]);
    send(&mut ws, huge_norm).await;
    assert_eq!(next_text(&mut ws).await["code"], "xr_position_invalid");
    tokio::time::sleep(FRAME_GAP).await;
    let mut nonunit = frame(0, 0, true);
    nonunit["samples"][0]["rotation_xyzw"] = json!([0.0, 0.0, 0.0, 0.0]);
    send(&mut ws, nonunit).await;
    assert_eq!(next_text(&mut ws).await["code"], "xr_rotation_invalid");
    tokio::time::sleep(FRAME_GAP).await;
    // JSON cannot carry NaN: a literal NaN is malformed text, not a frame.
    ws.send(Message::Text(
        frame(0, 0, true).to_string().replace("1.5", "NaN").into(),
    ))
    .await
    .unwrap();
    assert_eq!(next_text(&mut ws).await["code"], "xr_malformed");
    tokio::time::sleep(FRAME_GAP).await;
    for d in xr_devices(addr).await {
        assert_eq!(
            received(&d),
            0,
            "no partial frame reached the registry: {d}"
        );
    }
    // The connection survives rejected frames, and seq 0 is still available.
    stream(&mut ws, 0..2, true).await;
    wait_for(addr, "XR-RIGHT-GRIP", |d| received(d) == 2).await;
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_tracking_loss_and_reacquire() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON_PATIENT).await;
    let addr = e.bound_addr();
    let ws = ready(addr).await;
    // Right controller occluded for frames 40..100 of 160 (16 ms apart).
    let sender = spawn_stream(ws, 0..160, |seq| !(40..100).contains(&seq));
    let lost = |d: &Value| d["state"] == "disconnected";
    wait_for(addr, "XR-RIGHT-GRIP", |d| received(d) > 0 && !lost(d)).await;
    let right = wait_for(addr, "XR-RIGHT-GRIP", lost).await;
    let during = received(&right);
    assert!(
        (1..=40).contains(&during),
        "no pose while untracked: {during}"
    );
    assert!(!lost(&device(addr, "XR-HMD").await), "head continues");
    assert!(!lost(&device(addr, "XR-LEFT-GRIP").await), "left continues");
    let back = wait_for(addr, "XR-RIGHT-GRIP", |d| received(d) > 40 && !lost(d)).await;
    assert!(received(&back) > during);
    sender.await.unwrap();
    // Exact counts once the last frames are in: head and left every frame, right 100 of 160.
    wait_for(addr, "XR-HMD", |d| received(d) == 160).await;
    wait_for(addr, "XR-LEFT-GRIP", |d| received(d) == 160).await;
    wait_for(addr, "XR-RIGHT-GRIP", |d| received(d) == 100).await;
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_single_writer_rejects_second_connection() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let mut first = ready(addr).await;
    match connect_async(format!("ws://{addr}/api/v1/ingest/xr")).await {
        Err(WsError::Http(r)) => assert_eq!(r.status(), 409),
        other => panic!("expected 409, got {:?}", other.map(|_| ())),
    }
    stream(&mut first, 0..4, true).await;
    wait_for(addr, "XR-HMD", |d| received(d) == 4).await;
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_sequence_reset_only_after_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON_PATIENT).await;
    let addr = e.bound_addr();
    let mut ws = ready(addr).await;
    stream(&mut ws, 0..5, true).await;
    wait_for(addr, "XR-HMD", |d| received(d) == 5).await;
    send(&mut ws, frame(4, 2_000_000_000, true)).await;
    assert_eq!(
        next_text(&mut ws).await["code"],
        "xr_sequence",
        "duplicate seq"
    );
    tokio::time::sleep(FRAME_GAP).await;
    send(&mut ws, frame(0, 2_000_000_000, true)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        received(&device(addr, "XR-HMD").await),
        5,
        "old seq refused"
    );
    // Abrupt drop (no close frame): the devices disconnect, the writer slot frees.
    drop(ws);
    wait_for(addr, "XR-HMD", |d| d["state"] == "disconnected").await;
    let sender = spawn_stream(ready(addr).await, 0..60, |_| true);
    for id in XR {
        wait_for(addr, id, |d| {
            received(d) > 5 && d["state"] != "disconnected"
        })
        .await;
    }
    sender.await.unwrap();
    for id in XR {
        wait_for(addr, id, |d| received(d) == 65).await;
    }
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_idle_disconnects_without_false_pose() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(
        dir.path(),
        "xr: {enabled: true, tracking_stale_after_ms: 100}",
    )
    .await;
    let addr = e.bound_addr();
    let mut ws = ready(addr).await;
    stream(&mut ws, 0..3, true).await;
    for id in XR {
        let d = wait_for(addr, id, |d| d["state"] == "disconnected").await;
        assert_eq!(received(&d), 3, "no pose invented while idle");
    }
    // Same socket recovers with the next valid frame, while frames keep coming.
    let sender = spawn_stream(ws, 3..60, |_| true);
    wait_for(addr, "XR-HMD", |d| {
        received(d) > 3 && d["state"] != "disconnected"
    })
    .await;
    sender.await.unwrap();
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_flood_is_rate_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let mut ws = ready(addr).await;
    // A client that never reads, flooding as fast as it can for ~0.5 s.
    let started = std::time::Instant::now();
    let mut seq = 0;
    while started.elapsed() < Duration::from_millis(500) {
        send(&mut ws, frame(seq, seq, true)).await;
        seq += 1;
    }
    let elapsed = started.elapsed().as_secs_f64();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let n = received(&device(addr, "XR-HMD").await);
    let ceiling = (120.0 * (elapsed + 0.2)) as u64 + 2;
    assert!(
        seq > 2 * ceiling,
        "the flood must exceed the ceiling to test it ({seq} sent)"
    );
    assert!(
        n > 0 && n <= ceiling,
        "{n} accepted of {seq} in {elapsed:.2} s"
    );
    e.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xr_ingest_recording_roundtrip_and_replay_refuses_live() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path(), ON).await;
    let addr = e.bound_addr();
    let (status, _) = http(addr, "POST", "recording/start", "{}").await;
    assert_eq!(status, 200);
    let mut ws = ready(addr).await;
    stream(&mut ws, 0..6, true).await;
    stream(&mut ws, 6..8, false).await;
    wait_for(addr, "XR-HMD", |d| received(d) == 8).await;
    ws.close(None).await.unwrap();
    wait_for(addr, "XR-HMD", |d| d["state"] == "disconnected").await;
    let (_, stopped) = http(addr, "POST", "recording/stop", "{}").await;
    let id = stopped["data"]["id"].as_str().unwrap().to_owned();

    // Replay: live XR is refused at the gate and cannot change the session.
    let (status, _) = http(
        addr,
        "POST",
        "playback/start",
        &json!({"session_id": id}).to_string(),
    )
    .await;
    assert_eq!(status, 200);
    match connect_async(format!("ws://{addr}/api/v1/ingest/xr")).await {
        Err(WsError::Http(r)) => assert_eq!(r.status(), 409),
        other => panic!("expected 409 during replay, got {:?}", other.map(|_| ())),
    }
    http(addr, "POST", "playback/stop", "{}").await;
    e.shutdown().await.unwrap();

    let recording = reader::load(dir.path(), &SessionId::new(&id).unwrap()).unwrap();
    let events: Vec<&SourceEvent> = recording
        .events
        .iter()
        .filter_map(|e| match &e.data {
            RecordedData::Event(event) => Some(event.as_ref()),
            _ => None,
        })
        .filter(|e| match e {
            SourceEvent::Observation { observation } => {
                observation.source_id.as_str() == "xr-openxr"
            }
            SourceEvent::DeviceLink { device_id, .. } => device_id.as_str().starts_with("XR-"),
            _ => false,
        })
        .collect();
    let poses: Vec<&SensorObservation> = events
        .iter()
        .filter_map(|e| match e {
            SourceEvent::Observation { observation } => Some(observation),
            _ => None,
        })
        .collect();
    assert_eq!(poses.len(), 8 + 8 + 6, "head and left 8 frames, right 6");
    for frame in poses.chunk_by(|a, b| a.seq == b.seq) {
        assert!(
            frame.iter().all(|o| o.t_us == frame[0].t_us),
            "one receive time per frame"
        );
        assert!(
            frame
                .iter()
                .all(|o| o.device_time_us == Some(1_000_000_000 + o.seq * 16_667))
        );
        assert!(
            frame
                .iter()
                .all(|o| o.time_quality == TimeQuality::ReceiveTime
                    && o.origin == Origin::Live
                    && o.confidence.is_none())
        );
        assert!(frame.iter().all(|o| matches!(
            o.data,
            ObservationData::RigidPose {
                frame: FrameRef::XrStage,
                ..
            }
        )));
    }
    // Link up precedes each device's first pose; the tracking loss and the close are recorded.
    for id in XR {
        let mine: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                SourceEvent::DeviceLink {
                    device_id,
                    connected,
                    ..
                } if device_id.as_str() == id => Some(format!("link {connected}")),
                SourceEvent::Observation { observation }
                    if observation.device_id.as_str() == id =>
                {
                    Some("pose".into())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            mine.first().map(String::as_str),
            Some("link true"),
            "{id}: {mine:?}"
        );
        assert_eq!(
            mine.last().map(String::as_str),
            Some("link false"),
            "{id}: {mine:?}"
        );
    }
    // Replay re-applies the same observations without a live socket, twice (seek resets).
    let last_hmd = poses
        .iter()
        .rev()
        .find(|o| o.device_id.as_str() == "XR-HMD")
        .cloned()
        .cloned();
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    for _ in 0..2 {
        replay.seek(0).unwrap();
        replay.paused = false;
        while !replay.paused {
            replay.advance(50_000).unwrap();
        }
        let hmd = &replay.registry.entries[&DeviceId::new("XR-HMD").unwrap()];
        assert_eq!(hmd.received, 8);
        assert!(!hmd.connected, "the recorded close replays as disconnected");
        assert_eq!(
            hmd.latest.get(&ObservationKind::RigidPose).cloned(),
            last_hmd
        );
    }
}
