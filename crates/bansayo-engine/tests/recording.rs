//! Recording integrity: only accepted observations are recorded, recorded camera landmarks replay,
//! and session listing reads metadata instead of loading every recording.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    clock::SessionClock,
    config,
    pipeline::{
        Runner,
        command::{Client, Command, ControlRequest},
    },
    recording::{
        reader::{self, RecordedData},
        replay::Replay,
    },
    registry::Registry,
    skeleton::Skeleton,
    sources::{Ingest, simulated::SimulatedSource},
    start,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::SocketAddr,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

async fn control(client: &Client, request: ControlRequest) -> Value {
    let (tx, rx) = oneshot::channel();
    client
        .commands
        .send(Command::Control(request, tx))
        .await
        .unwrap();
    rx.await.unwrap().unwrap()
}
fn orientation(seq: u64, t_us: u64) -> Ingest {
    Ingest::Event(SourceEvent::Observation {
        observation: serde_json::from_value(json!({
            "device_id": "T1", "source_id": "sim", "origin": "simulated", "seq": seq,
            "t_us": t_us, "device_time_us": null, "time_quality": "synchronized",
            "calibration_ref": null, "confidence": 1.0,
            "data": {"kind": "orientation", "frame": {"type": "sensor_reference", "id": "T1"},
                     "q": [0.0, 0.0, 0.0, 1.0]}
        }))
        .unwrap(),
    })
}
fn observations(recording: &reader::Recording) -> Vec<&SensorObservation> {
    recording
        .events
        .iter()
        .filter_map(|e| match &e.data {
            RecordedData::Event(event) => match event.as_ref() {
                SourceEvent::Observation { observation } => Some(observation),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_reordered_and_delayed_packets_never_reach_the_recording() {
    let dir = tempfile::tempdir().unwrap();
    let config = config::parse(&format!(
        "schema_version: 1\nengine: {{data_dir: {:?}}}\nassignments: {{T1: pelvis}}\nsources:\n  - {{id: sim,kind: simulated,trackers: [{{id: T1,rate_hz: 100}}]}}\n",
        dir.path().display().to_string()
    ))
    .unwrap();
    let skeleton = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let source = SimulatedSource {
        spec: config.simulated[0].clone(),
        skeleton: skeleton.clone(),
        assignments: watch::channel(config.assignments.clone()).1,
    };
    let clock = SessionClock::start();
    let (events, events_rx) = mpsc::channel(64);
    let runner = Runner {
        registry: Registry::new(
            source.descriptors(),
            config.assignments.clone(),
            BTreeMap::new(),
        ),
        config,
        store: Store::default(),
        skeleton,
        clock: clock.clone(),
        events: events_rx,
        controls: vec![],
        faults: watch::channel(BTreeMap::new()).0,
        placements: watch::channel(BTreeMap::new()).0,
        training: None,
        training_status: None,
    };
    let cancel = CancellationToken::new();
    let (client, _streams, task) = runner.spawn(cancel.clone());
    control(
        &client,
        ControlRequest::RecordStart(RecordingStartRequest { name: None }),
    )
    .await;
    // Every packet timestamp below must not be in the engine's future.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let link = |connected| {
        Ingest::Event(SourceEvent::DeviceLink {
            device_id: DeviceId::new("T1").unwrap(),
            connected,
            t_us: clock.now_us(),
        })
    };
    for ingest in [
        orientation(1, 1_000),
        orientation(3, 3_000),
        orientation(2, 2_000), // reordered
        orientation(3, 3_000), // duplicate
        orientation(4, 2_500), // delayed: newer seq, older timestamp
        link(false),
        link(true),
        orientation(1, 5_000), // the device restarted its sequence after reconnecting
    ] {
        events.send(ingest).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let session = control(&client, ControlRequest::RecordStop).await;
    let id = SessionId::new(session["id"].as_str().unwrap()).unwrap();
    let recording = reader::load(dir.path(), &id).expect("the recording replays");
    let accepted: Vec<_> = observations(&recording)
        .iter()
        .map(|o| (o.seq, o.t_us))
        .collect();
    assert_eq!(accepted, [(1, 1_000), (3, 3_000), (1, 5_000)]);
    cancel.cancel();
    task.await.unwrap();
}

async fn request(addr: SocketAddr, method: &str, path: &str, body: &str) -> Value {
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
fn landmark_seqs(messages: &[ServerMessage]) -> Vec<u64> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Landmarks2d { landmarks } => Some(landmarks.frame_meta.frame_seq),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn camera_landmarks_replay_and_sessions_list_from_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!(
        r#"schema_version: 1
engine: {{data_dir: {:?}}}
api: {{bind: "127.0.0.1:0"}}
sources:
  - id: sim
    kind: simulated
    cameras:
      - {{id: CAM1, rate_hz: 30, resolution_px: [1280, 720], fov_y_deg: 50, position_m: [0.0, 1.2, 3.2], look_at_m: [0.0, 1.0, 0.0]}}
"#,
        dir.path().display().to_string()
    );
    let engine = start(config::parse(&yaml).unwrap()).await.unwrap();
    let addr = engine.bound_addr();
    tokio::time::sleep(Duration::from_millis(50)).await;
    request(addr, "POST", "recording/start", "{}").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let stopped = request(addr, "POST", "recording/stop", "{}").await;
    engine.shutdown().await.unwrap();
    let id = stopped["data"]["id"].as_str().unwrap().to_owned();

    let recording = reader::load(dir.path(), &SessionId::new(&id).unwrap()).unwrap();
    // (accepted at, frame seq) of every recorded camera frame.
    let frames: Vec<_> = recording
        .events
        .iter()
        .filter_map(|e| match &e.data {
            RecordedData::Event(event) => match event.as_ref() {
                SourceEvent::Observation {
                    observation:
                        SensorObservation {
                            data: ObservationData::Landmarks2d { landmarks },
                            ..
                        },
                } => Some((e.accepted_us, landmarks.frame_meta.frame_seq)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let recorded: Vec<_> = frames.iter().map(|f| f.1).collect();
    assert!(recorded.len() >= 5, "{recorded:?}");
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    let mut replayed = vec![];
    while !replay.paused {
        replayed.extend(landmark_seqs(&replay.advance(10_000).unwrap()));
    }
    // Every recorded camera frame replays exactly once, including any after the last tick.
    assert_eq!(replayed, recorded);
    assert_eq!(
        replay.position_us, recording.session.duration_us,
        "a finished replay reports the session end"
    );
    // A paused seek shows the camera frame current at that position.
    let (accepted_us, seq) = frames[2];
    replay
        .seek(accepted_us - recording.initial.started_t_us)
        .unwrap();
    assert_eq!(landmark_seqs(&replay.preview()), [seq]);

    // A valid recording too large to replay is still listed; an unfinalised one is not.
    let sessions = dir.path().join("sessions");
    let metadata: Value =
        serde_json::from_slice(&std::fs::read(sessions.join(&id).join("session.json")).unwrap())
            .unwrap();
    let fake = |suffix: &str, size: u64| {
        let fake_id = format!("{}-{suffix}", &id[..id.len() - 5]);
        let path = sessions.join(&fake_id);
        std::fs::create_dir(&path).unwrap();
        let mut session = metadata.clone();
        session["id"] = json!(fake_id);
        session["size_bytes"] = json!(size);
        std::fs::write(path.join("session.json"), session.to_string()).unwrap();
        std::fs::File::create(path.join("recording.mcap"))
            .unwrap()
            .set_len(size.max(64))
            .unwrap();
        fake_id
    };
    let large = fake("zzzz", reader::MAX_REPLAY_BYTES + 1);
    let unfinalised = fake("yyyy", 0);
    let (listed, damaged) = reader::sessions(dir.path()).unwrap();
    let mut listed: Vec<_> = listed.iter().map(|s| s.id.to_string()).collect();
    listed.sort();
    let mut expected = vec![id, large.clone()];
    expected.sort();
    assert_eq!(listed, expected);
    assert_eq!(damaged, [unfinalised]);
    let error = reader::load(dir.path(), &SessionId::new(&large).unwrap())
        .err()
        .unwrap();
    assert!(error.contains("too large"), "{error}");
}
