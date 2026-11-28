//! The ESP32 source over a real UDP socket: device time mapping across a 32-bit wrap, listed
//! trackers only, rejected garbage and out-of-order packets, raw gyro and accelerometer streams.
use bansayo_contracts::*;
use bansayo_engine::{
    clock::SessionClock,
    config,
    sources::{
        Ingest, Source, SourceContext,
        esp32::{Esp32Source, Packet, encode},
    },
};
use std::time::Duration;

fn packet(sensor: u8, seq: u32, t: u32) -> Packet {
    Packet {
        mac: [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01],
        sensor,
        status: 0,
        seq,
        device_time_us: t,
        q: [0.0, 0.0, 0.0, 1.0],
        gyro: Some([0.0, 5.0, 0.0]),
        accel: Some([0.0, 9.81, 0.0]),
        battery_pct: Some(77),
        rssi_dbm: Some(-58),
    }
}

#[tokio::test]
async fn packets_become_time_mapped_observations() {
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let yaml = format!(
        "schema_version: 2
sources:
  - kind: esp32_udp
    id: esp
    bind: 127.0.0.1:{port}
    rate_hz: 200
    trackers:
      - {{ id: ESP-SHIN-R, tracker: 'aa:bb:cc:dd:ee:01/0' }}
assignments:
  ESP-SHIN-R: right_shin
"
    );
    let validated = config::parse(&yaml).unwrap();
    let source = Esp32Source {
        spec: validated.esp32[0].clone(),
        runtime: Default::default(),
    };
    assert_eq!(source.descriptors()[0].transport, Transport::Udp);
    let (events, mut rx) = tokio::sync::mpsc::channel(4096);
    let context = SourceContext {
        events,
        clock: SessionClock::start(),
        cancel: tokio_util::sync::CancellationToken::new(),
        dropped: Default::default(),
    };
    let cancel = context.cancel.clone();
    let runtime = source.runtime.clone();
    let task = tokio::spawn(source.run(context));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let tx = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let to = format!("127.0.0.1:{port}");
    // 5 ms apart, crossing the u32 wrap of the device clock.
    let start = u32::MAX - 20_000;
    for i in 0..20u32 {
        tx.send_to(
            &encode(&packet(0, i + 1, start.wrapping_add(i * 5_000))),
            &to,
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    tx.send_to(&encode(&packet(0, 5, 0)), &to).await.unwrap(); // out of order: dropped
    tx.send_to(&encode(&packet(3, 1, 0)), &to).await.unwrap(); // sensor 3 not listed
    tx.send_to(b"garbage", &to).await.unwrap();
    let mut mag = [0_u8; 32];
    mag[..4].copy_from_slice(b"BMAG");
    mag[4] = 1;
    mag[6] = 1;
    mag[8..14].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 1]);
    mag[16..20].copy_from_slice(&42_u32.to_le_bytes());
    mag[24..26].copy_from_slice(&(-100_i16).to_le_bytes());
    tx.send_to(&mag, &to).await.unwrap();
    mag[5] = 3; // Not allowlisted.
    mag[6] = 0;
    tx.send_to(&mag, &to).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.cancel();
    task.await.unwrap().unwrap();

    let mut orientation = vec![];
    let (mut gyro, mut accel, mut diag) = (0, 0, 0);
    let mut magnetic = 0;
    while let Ok(event) = rx.try_recv() {
        let o = match event {
            Ingest::Magnetic {
                device_id,
                channel,
                t_us,
                seq,
                device_time_us,
                values,
            } => {
                assert_eq!(device_id.as_str(), "ESP-SHIN-R");
                magnetic += 1;
                bansayo_engine::sources::inspection::magnetic(
                    &runtime,
                    &device_id,
                    channel,
                    t_us,
                    seq,
                    device_time_us,
                    values,
                );
                continue;
            }
            Ingest::Event(SourceEvent::Observation { observation }) => observation,
            _ => continue,
        };
        assert_eq!(o.device_id.as_str(), "ESP-SHIN-R");
        match o.data {
            ObservationData::Orientation { .. } => orientation.push(o),
            ObservationData::AngularVelocity { ref frame, .. } => {
                assert!(matches!(frame, FrameRef::Sensor { .. }));
                gyro += 1
            }
            ObservationData::Acceleration { .. } => accel += 1,
            ObservationData::Diagnostics { battery_level, .. } => {
                assert_eq!(battery_level, Some(0.77));
                diag += 1
            }
            _ => {}
        }
    }
    assert_eq!(orientation.len(), 20);
    assert_eq!((gyro, accel, diag), (20, 20, 1));
    assert_eq!(magnetic, 1);
    assert!(
        orientation
            .iter()
            .all(|o| o.time_quality == TimeQuality::Estimated)
    );
    // Device time keeps increasing through the wrap; mapped times are 5 ms apart once the
    // offset minimum has settled, never later than arrival.
    assert!(
        orientation
            .windows(2)
            .all(|w| w[1].device_time_us > w[0].device_time_us)
    );
    assert!(orientation.windows(2).all(|w| w[1].t_us >= w[0].t_us));
    let sync = runtime.read().unwrap()[&DeviceId::new("ESP-SHIN-R").unwrap()]
        .time_sync
        .clone()
        .unwrap();
    assert_eq!(sync.resets, 0);
    let id = DeviceId::new("ESP-SHIN-R").unwrap();
    let snapshot =
        serde_json::to_value(bansayo_engine::sources::inspection::snapshot(&runtime, &id)).unwrap();
    assert_eq!(
        snapshot["channels"]["mag1"]["points"][0]["values"],
        serde_json::json!([-100.0, 0.0, 0.0])
    );
    assert!(snapshot["channels"]["mag0"].is_null());
}

#[test]
fn bad_tracker_keys_are_rejected() {
    let yaml = "schema_version: 2
sources:
  - kind: esp32_udp
    id: esp
    trackers:
      - { id: A, tracker: 'not-a-mac' }
";
    assert!(config::parse(yaml).is_err());
}
