//! The SlimeVR source against a fake SolarXR server: the feed request, tracker identity and
//! announcement, repeated snapshots, status, configuration validation, and a server restart.
use bansayo_contracts::*;
use bansayo_engine::{
    clock::SessionClock,
    config,
    sources::{Ingest, Source, SourceContext, slimevr::SlimevrSource},
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/solarxr/");

fn yaml(endpoint: &str) -> String {
    format!(
        "schema_version: 2
engine: {{ data_dir: ./data }}
sources:
  - kind: slimevr_solarxr
    id: slimevr
    endpoint: {endpoint}
    feed_interval_ms: 5
    latency_ms: 8
    trackers:
      - {{ id: SVR-PELVIS, tracker: 'AA:BB:CC:DD:EE:01/0' }}
assignments:
  SVR-PELVIS: pelvis
"
    )
}

/// Accepts one connection, checks the request, sends `update` every 10 ms for `run`, then closes.
async fn serve_once(listener: &tokio::net::TcpListener, update: Vec<u8>, run: Duration) -> Vec<u8> {
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let request = match ws.next().await {
        Some(Ok(Message::Binary(b))) => b.to_vec(),
        other => panic!("expected a binary StartDataFeed, got {other:?}"),
    };
    let until = tokio::time::Instant::now() + run;
    while tokio::time::Instant::now() < until {
        if ws
            .send(Message::Binary(update.clone().into()))
            .await
            .is_err()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let _ = ws.close(None).await;
    request
}

#[tokio::test]
async fn trackers_flow_in_and_survive_a_server_restart() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("ws://{}", listener.local_addr().unwrap());
    let validated = config::parse(&yaml(&endpoint)).unwrap();
    let spec = validated.slimevr[0].clone();
    assert_eq!(validated.assignments.len(), 1);
    let source = SlimevrSource {
        spec,
        runtime: Default::default(),
    };
    assert_eq!(source.descriptors()[0].id.as_str(), "SVR-PELVIS");
    let (events, mut rx) = tokio::sync::mpsc::channel(4096);
    let context = SourceContext {
        events,
        clock: SessionClock::start(),
        cancel: tokio_util::sync::CancellationToken::new(),
        dropped: Default::default(),
    };
    let cancel = context.cancel.clone();
    let task = tokio::spawn(source.clone().run(context));
    let update = std::fs::read(format!("{FIXTURES}update_two_devices.bin")).unwrap();

    let request = serve_once(&listener, update.clone(), Duration::from_millis(350)).await;
    assert_eq!(
        request,
        bansayo_engine::sources::slimevr::solarxr::start_data_feed(5)
    );
    // Server restarted: the source reconnects by itself.
    let again = serve_once(&listener, update, Duration::from_millis(150)).await;
    assert_eq!(again, request);
    cancel.cancel();
    task.await.unwrap().unwrap();

    let mut announced = vec![];
    let mut links = vec![];
    let mut orientations: Vec<SensorObservation> = vec![];
    let mut diagnostics = 0;
    while let Ok(e) = rx.try_recv() {
        match e {
            Ingest::Event(SourceEvent::DeviceAnnounced { descriptor }) => {
                announced.push(descriptor)
            }
            Ingest::Event(SourceEvent::DeviceLink {
                device_id,
                connected,
                ..
            }) => links.push((device_id.to_string(), connected)),
            Ingest::Event(SourceEvent::Observation { observation }) => match observation.data {
                ObservationData::Orientation { .. } => orientations.push(observation),
                ObservationData::Diagnostics {
                    battery_level,
                    rssi_dbm,
                    ..
                } => {
                    if observation.device_id.as_str() == "SVR-PELVIS" {
                        assert_eq!(battery_level, Some(0.87));
                        assert_eq!(rssi_dbm, Some(-52));
                    }
                    diagnostics += 1;
                }
                other => panic!("unexpected {other:?}"),
            },
            _ => {}
        }
    }
    // The listed tracker keeps its configured id; the others are announced once each.
    let ids: Vec<&str> = announced.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["SVR-AABBCCDDEE02-0", "SVR-AABBCCDDEE02-1"]);
    assert!(announced.iter().all(|d| d.kind == DeviceKind::ImuTracker));
    // Linked on first data, unlinked when the server went away, linked again after it came back.
    let pelvis: Vec<bool> = links
        .iter()
        .filter(|(id, _)| id == "SVR-PELVIS")
        .map(|(_, c)| *c)
        .collect();
    assert_eq!(&pelvis[..3], [true, false, true]);
    let pelvis_q: Vec<&SensorObservation> = orientations
        .iter()
        .filter(|o| o.device_id.as_str() == "SVR-PELVIS")
        .collect();
    // ~50 identical snapshots over 0.5 s: only the first per connection and 100 ms keep-alives.
    assert!(
        (2..=10).contains(&pelvis_q.len()),
        "{} pelvis orientations",
        pelvis_q.len()
    );
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let first = pelvis_q[0];
    assert_eq!(first.time_quality, TimeQuality::ReceiveTime);
    assert_eq!(first.device_time_us, None);
    assert!(matches!(first.data, ObservationData::Orientation { q, .. } if q == [0.0, h, 0.0, h]));
    assert!(
        pelvis_q
            .windows(2)
            .all(|w| w[1].seq > w[0].seq && w[1].t_us >= w[0].t_us)
    );
    // The occluded extension tracker is published with lowered confidence.
    let occluded = orientations
        .iter()
        .find(|o| o.device_id.as_str() == "SVR-AABBCCDDEE02-1")
        .unwrap();
    assert_eq!(occluded.confidence, Some(0.4));
    assert!(diagnostics >= 2);
}

#[test]
fn configuration_is_validated() {
    let bad = yaml("http://localhost:21110").replace(
        "trackers:\n      - { id: SVR-PELVIS, tracker: 'AA:BB:CC:DD:EE:01/0' }",
        "trackers:\n      - { id: A, tracker: 'x/0' }\n      - { id: B, tracker: 'x/0' }",
    );
    let problems = match config::parse(&bad) {
        Err(config::ConfigError::Invalid(p)) => p,
        other => panic!("{:?}", other.map(|_| ())),
    };
    let text: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
    assert!(text.iter().any(|p| p.contains("endpoint")), "{text:?}");
    assert!(
        text.iter().any(|p| p.contains("already listed")),
        "{text:?}"
    );
    let ok = config::parse(&yaml("ws://127.0.0.1:21110")).unwrap();
    assert_eq!(ok.slimevr[0].endpoint, "ws://127.0.0.1:21110");
    assert_eq!(ok.slimevr[0].latency_us, 8_000);
}
