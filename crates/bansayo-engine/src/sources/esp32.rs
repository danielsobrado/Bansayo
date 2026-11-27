//! Direct ESP32 IMU trackers over UDP (`esp32_udp`, ADR-022 item 5): for rates, device sample
//! times, gyro and accelerometer data that SolarXR does not carry.
//!
//! BIMU v1 is 64 bytes. V2 appends a nonzero u64 orientation-reference id at offset 64.
//! The original v1 layout is otherwise unchanged. A new reference invalidates mounting.
//!
//! | Offset | Type | Field |
//! |---|---|---|
//! | 0 | `[u8; 4]` | magic `BIMU` |
//! | 4 | u8 | version, 1 |
//! | 5 | u8 | flags: bit 0 gyro valid, bit 1 accel valid, bit 2 battery valid |
//! | 6 | u8 | sensor number on the board |
//! | 7 | u8 | status: 0 ok, 1 calibrating or degraded, 2 error |
//! | 8 | `[u8; 6]` | board MAC address |
//! | 14 | u8 | battery percent |
//! | 15 | i8 | RSSI dBm, 0 when unknown |
//! | 16 | u32 | sequence number |
//! | 20 | u32 | device time of the sample, µs, wrapping |
//! | 24 | `[f32; 4]` | `q_reference_from_sensor` `[x, y, z, w]`, reference right-handed +Y up |
//! | 40 | `[f32; 3]` | angular velocity, rad/s, sensor frame |
//! | 52 | `[f32; 3]` | specific force (accelerometer), m/s², sensor frame, gravity included |
//!
//! Accelerations are published as raw sensor-frame measurements; nothing integrates them.
//! Datagrams from boards or sensors not listed in the configuration are ignored, so one tracker
//! never feeds two streams.
use crate::clock::{SourceTime, Unwrap32};
use crate::config::Esp32Spec;
use crate::sources::{Ingest, RuntimeMap, Source, SourceContext, SourceError};
use bansayo_contracts::*;
use std::collections::BTreeMap;

pub const PACKET_LEN: usize = 64;
pub const MAGIC: &[u8; 4] = b"BIMU";
pub const VERSION: u8 = 1;
pub const PACKET_V2_LEN: usize = 72;
pub const VERSION_V2: u8 = 2;
mod magnetic;
/// Beyond the ±2000 °/s range of common MEMS gyros, with margin: a corrupt value.
const MAX_RAD_S: f32 = 70.0;
const MAX_M_S2: f32 = 2_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub mac: [u8; 6],
    pub sensor: u8,
    pub status: u8,
    pub seq: u32,
    pub device_time_us: u32,
    pub q: [f32; 4],
    pub gyro: Option<[f32; 3]>,
    pub accel: Option<[f32; 3]>,
    pub battery_pct: Option<u8>,
    pub rssi_dbm: Option<i8>,
}

pub fn key(mac: &[u8; 6], sensor: u8) -> String {
    let [a, b, c, d, e, f] = mac;
    format!("{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{f:02X}/{sensor}")
}

fn f32s<const N: usize>(b: &[u8], at: usize) -> [f32; N] {
    std::array::from_fn(|i| {
        let o = at + i * 4;
        f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    })
}

/// Parses and validates one datagram.
pub fn parse(b: &[u8]) -> Result<Packet, &'static str> {
    if b.len() != PACKET_LEN {
        return Err("wrong length");
    }
    if &b[0..4] != MAGIC {
        return Err("not a bansayo.imu packet");
    }
    if b[4] != VERSION {
        return Err("unsupported version");
    }
    parse_fields(b)
}

fn parse_fields(b: &[u8]) -> Result<Packet, &'static str> {
    let flags = b[5];
    let q: [f32; 4] = f32s(b, 24);
    let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > 1e-2 {
        return Err("orientation is not a unit quaternion");
    }
    let vector = |bit: u8, at: usize, max: f32| -> Result<Option<[f32; 3]>, &'static str> {
        if flags & bit == 0 {
            return Ok(None);
        }
        let v: [f32; 3] = f32s(b, at);
        if v.iter().all(|x| x.is_finite() && x.abs() <= max) {
            Ok(Some(v))
        } else {
            Err("gyro or accelerometer value out of range")
        }
    };
    Ok(Packet {
        mac: b[8..14].try_into().map_err(|_| "mac")?,
        sensor: b[6],
        status: b[7],
        seq: u32::from_le_bytes([b[16], b[17], b[18], b[19]]),
        device_time_us: u32::from_le_bytes([b[20], b[21], b[22], b[23]]),
        q: q.map(|v| v / norm),
        gyro: vector(1, 40, MAX_RAD_S)?,
        accel: vector(2, 52, MAX_M_S2)?,
        battery_pct: (flags & 4 != 0).then_some(b[14].min(100)),
        rssi_dbm: (b[15] != 0).then_some(b[15] as i8),
    })
}

/// Accept legacy BIMU and the reference-aware extension.
fn parse_datagram(b: &[u8]) -> Result<(Packet, Option<u64>), &'static str> {
    if b.len() == PACKET_V2_LEN && &b[..4] == MAGIC && b[4] == VERSION_V2 {
        let reference = u64::from_le_bytes(b[64..72].try_into().map_err(|_| "reference")?);
        if reference == 0 {
            return Err("zero reference id");
        }
        return Ok((parse_fields(b)?, Some(reference)));
    }
    parse(b).map(|p| (p, None))
}

/// Encodes a packet; firmware reference and tests.
pub fn encode(p: &Packet) -> [u8; PACKET_LEN] {
    let mut b = [0u8; PACKET_LEN];
    b[0..4].copy_from_slice(MAGIC);
    b[4] = VERSION;
    b[5] = u8::from(p.gyro.is_some())
        | u8::from(p.accel.is_some()) << 1
        | u8::from(p.battery_pct.is_some()) << 2;
    b[6] = p.sensor;
    b[7] = p.status;
    b[8..14].copy_from_slice(&p.mac);
    b[14] = p.battery_pct.unwrap_or(0);
    b[15] = p.rssi_dbm.unwrap_or(0) as u8;
    b[16..20].copy_from_slice(&p.seq.to_le_bytes());
    b[20..24].copy_from_slice(&p.device_time_us.to_le_bytes());
    for (i, v) in
        p.q.iter()
            .chain(&p.gyro.unwrap_or_default())
            .chain(&p.accel.unwrap_or_default())
            .enumerate()
    {
        b[24 + i * 4..28 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    b
}

#[derive(Clone)]
pub struct Esp32Source {
    pub spec: Esp32Spec,
    pub runtime: RuntimeMap,
}

impl Esp32Source {
    pub fn descriptors(&self) -> Vec<DeviceDescriptor> {
        self.spec
            .trackers
            .iter()
            .map(|(id, _)| DeviceDescriptor {
                id: id.clone(),
                source_id: self.spec.id.clone(),
                kind: DeviceKind::ImuTracker,
                origin: Origin::Live,
                model: Some("ESP32 IMU (bansayo.imu.v1)".into()),
                firmware: None,
                transport: Transport::Udp,
                capabilities: vec![
                    SensorCapability::Orientation,
                    SensorCapability::AngularVelocity,
                    SensorCapability::Acceleration,
                    SensorCapability::Battery,
                    SensorCapability::SignalStrength,
                ],
                nominal_rate_hz: Some(self.spec.rate_hz as f32),
                camera: None,
            })
            .collect()
    }
}

/// Per-tracker clock and stream state.
#[derive(Default)]
struct Track {
    wrap: Unwrap32,
    time: SourceTime,
    sequence: Sequence,
    reference: Option<u64>,
    retired_references: std::collections::VecDeque<u64>,
    diag_seq: u64,
    last_mapped_us: Option<u64>,
    last_diag_us: Option<u64>,
    status_us: Option<u64>,
}

// Accept forward modular sequences, including wrap. A backwards sequence is a
// reset only after a quiet interval and a backwards device clock.
const RESET_QUIET_US: u64 = 500_000;

#[derive(Default)]
struct Sequence {
    last: Option<(u32, u32, u64)>,
    logical: u64,
}

impl Sequence {
    fn observe(&mut self, seq: u32, clock: u32, now: u64) -> Option<(u64, bool)> {
        let mut reset = false;
        if let Some((previous, device, arrival)) = self.last {
            let delta = seq.wrapping_sub(previous);
            let clock_delta = clock.wrapping_sub(device);
            if delta == 0 || delta >= (1 << 31) || clock_delta >= (1 << 31) {
                if now.saturating_sub(arrival) < RESET_QUIET_US || clock >= device || seq > previous
                {
                    return None;
                }
                reset = true;
                self.logical += 1;
            } else {
                self.logical += u64::from(delta);
            }
        } else {
            self.logical += 1;
        }
        self.last = Some((seq, clock, now));
        Some((self.logical, reset))
    }
}

impl Track {
    /// Do not let a delayed packet from a previous AHRS reference revive it.
    fn observe(&mut self, p: &Packet, reference: Option<u64>, now: u64) -> Option<(u64, bool)> {
        let mut changed = false;
        if let Some(reference) = reference {
            if self.reference != Some(reference) {
                if self.retired_references.contains(&reference) {
                    return None;
                }
                if let Some(old) = self.reference.replace(reference) {
                    if self.retired_references.len() == 16 {
                        self.retired_references.pop_front();
                    }
                    self.retired_references.push_back(old);
                }
                self.sequence.last = None;
                changed = true;
            }
        } else if self.reference.is_some() {
            // A reference-aware tracker must not silently downgrade.
            return None;
        }
        let (seq, reset) = self.sequence.observe(p.seq, p.device_time_us, now)?;
        Some((seq, changed || reset))
    }
}

impl Source for Esp32Source {
    type Context = SourceContext;
    async fn run(self, context: SourceContext) -> Result<(), SourceError> {
        let socket = tokio::net::UdpSocket::bind(&self.spec.bind)
            .await
            .map_err(|e| SourceError(format!("{}: {e}", self.spec.bind)))?;
        tracing::info!(source=%self.spec.id, bind=%self.spec.bind, "Listening for ESP32 IMU packets");
        let by_key: BTreeMap<&str, &DeviceId> = self
            .spec
            .trackers
            .iter()
            .map(|(id, k)| (k.as_str(), id))
            .collect();
        let mut tracks: BTreeMap<DeviceId, Track> = BTreeMap::new();
        let mut ignored = std::collections::BTreeSet::new();
        let mut buf = [0u8; 2048];
        loop {
            let n = tokio::select! {
                _ = context.cancel.cancelled() => return Ok(()),
                r = socket.recv_from(&mut buf) => r.map_err(|e| SourceError(e.to_string()))?.0,
            };
            let now = context.clock.now_us();
            if buf[..n].starts_with(b"BMAG") {
                if let Ok(packet) = magnetic::parse(&buf[..n])
                    && let Some(id) = by_key.get(key(&packet.mac, packet.sensor).as_str())
                {
                    crate::sources::inspection::magnetic(
                        &self.runtime,
                        id,
                        packet.channel,
                        now,
                        packet.seq,
                        packet.time_us,
                        packet.values,
                    );
                }
                continue;
            }
            let (p, reference) = match parse_datagram(&buf[..n]) {
                Ok(p) => p,
                Err(why) => {
                    tracing::debug!(source=%self.spec.id, why, "ESP32 datagram rejected");
                    continue;
                }
            };
            let k = key(&p.mac, p.sensor);
            let Some(id) = by_key.get(k.as_str()).copied() else {
                if ignored.len() < 128 && ignored.insert(k.clone()) {
                    tracing::info!(source=%self.spec.id, tracker=%k, "ESP32 tracker not configured; ignored");
                }
                continue;
            };
            let track = tracks.entry(id.clone()).or_default();
            let Some((seq, reset)) = track.observe(&p, reference, now) else {
                continue;
            };
            if reset {
                track.wrap = Default::default();
                if track.last_mapped_us.take().is_some() {
                    track.time.reset();
                }
                track.last_diag_us = None;
                track.status_us = None;
                tracing::warn!(device=%id, "Tracker orientation reference changed; repeat mounting calibration");
                let event = Ingest::Event(SourceEvent::DeviceLink {
                    device_id: id.clone(),
                    connected: true,
                    t_us: now,
                });
                tokio::select! {
                    _ = context.cancel.cancelled() => return Ok(()),
                    result = context.events.send(event) => {
                        if result.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            let device = track.wrap.extend(p.device_time_us);
            let (mapped, quality) = track.time.map(Some(device), now, self.spec.latency_us);
            // A newly observed minimum offset can move the estimate backwards.
            // Keep accepted capture times nondecreasing within this stream.
            let t_us = mapped.max(track.last_mapped_us.unwrap_or(0)).min(now);
            track.last_mapped_us = Some(t_us);
            let emit = |seq: u64, data: ObservationData, confidence: Option<f32>| {
                let observation = SensorObservation {
                    device_id: id.clone(),
                    source_id: self.spec.id.clone(),
                    origin: Origin::Live,
                    seq,
                    t_us,
                    device_time_us: Some(device),
                    time_quality: quality,
                    calibration_ref: None,
                    confidence,
                    data,
                };
                if context
                    .events
                    .try_send(Ingest::Event(SourceEvent::Observation { observation }))
                    .is_err()
                    && let Some(c) = context.dropped.get(id)
                {
                    c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            };
            if p.status >= 2 {
                continue;
            }
            let confidence = if p.status == 0 { 1.0 } else { 0.4 };
            emit(
                seq,
                ObservationData::Orientation {
                    frame: FrameRef::SensorReference { id: id.clone() },
                    q: p.q,
                },
                Some(confidence),
            );
            if let Some(rad_s) = p.gyro {
                emit(
                    seq,
                    ObservationData::AngularVelocity {
                        frame: FrameRef::Sensor { id: id.clone() },
                        rad_s,
                    },
                    None,
                );
            }
            if let Some(m_s2) = p.accel {
                emit(
                    seq,
                    ObservationData::Acceleration {
                        frame: FrameRef::Sensor { id: id.clone() },
                        m_s2,
                    },
                    None,
                );
            }
            if track
                .last_diag_us
                .is_none_or(|l| now.saturating_sub(l) >= 1_000_000)
            {
                track.last_diag_us = Some(now);
                track.diag_seq += 1;
                emit(
                    track.diag_seq,
                    ObservationData::Diagnostics {
                        battery_level: p.battery_pct.map(|b| b as f32 / 100.0),
                        charging: None,
                        rssi_dbm: p.rssi_dbm.map(i16::from),
                        quality: None,
                    },
                    None,
                );
            }
            if track
                .status_us
                .is_none_or(|l| now.saturating_sub(l) >= 1_000_000)
            {
                track.status_us = Some(now);
                if let Ok(mut r) = self.runtime.write() {
                    r.entry(id.clone()).or_default().time_sync = Some(track.time.status());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn packet(seq: u32, t: u32) -> Packet {
        Packet {
            mac: [0xAA, 0xBB, 0xCC, 0, 0, 1],
            sensor: 0,
            status: 0,
            seq,
            device_time_us: t,
            q: [0.0, 0.0, 0.0, 1.0],
            gyro: Some([0.1, -2.0, 3.0]),
            accel: Some([0.0, 9.81, 0.0]),
            battery_pct: Some(80),
            rssi_dbm: Some(-60),
        }
    }

    #[test]
    fn reference_change_rejects_retired_packets_and_downgrade() {
        let mut track = Track::default();
        assert_eq!(
            track.observe(&packet(7, 100), Some(10), 100),
            Some((1, true))
        );
        assert_eq!(
            track.observe(&packet(8, 200), Some(10), 200),
            Some((2, false))
        );
        assert_eq!(
            track.observe(&packet(9, 300), Some(11), 300),
            Some((3, true))
        );
        assert_eq!(track.observe(&packet(10, 400), Some(10), 400), None);
        assert_eq!(track.observe(&packet(10, 400), None, 400), None);
        assert_eq!(
            track.observe(&packet(1, 100), Some(12), 500),
            Some((4, true))
        );
    }

    #[test]
    fn v2_wire_reference() {
        let p = packet(7, 123);
        let mut b = [0_u8; PACKET_V2_LEN];
        b[..PACKET_LEN].copy_from_slice(&encode(&p));
        b[4] = VERSION_V2;
        b[64..].copy_from_slice(&0x1234_5678_90ab_cdef_u64.to_le_bytes());
        assert_eq!(parse_datagram(&b), Ok((p, Some(0x1234_5678_90ab_cdef))));
        b[64..].fill(0);
        assert!(parse_datagram(&b).is_err());
    }

    #[test]
    fn sequence_wrap_reorder_and_reboot() {
        let mut s = Sequence::default();
        assert_eq!(s.observe(u32::MAX - 1, 100, 0), Some((1, false)));
        assert_eq!(s.observe(0, 200, 100), Some((3, false)));
        assert_eq!(s.observe(u32::MAX, 150, 200), None);
        assert_eq!(s.observe(10, 50_000, 50_000), Some((13, false)));
        assert_eq!(s.observe(1, 1_000, 51_000), None);
        assert_eq!(s.observe(1, 1_000, 600_000), Some((14, true)));
        assert_eq!(s.observe(2, 6_000, 605_000), Some((15, false)));
    }

    #[test]
    fn packets_round_trip_and_bad_ones_are_rejected() {
        let p = packet(7, 123_456);
        assert_eq!(parse(&encode(&p)), Ok(p.clone()));
        assert_eq!(key(&p.mac, 2), "AA:BB:CC:00:00:01/2");
        let mut b = encode(&p);
        assert!(parse(&b[..63]).is_err());
        b[4] = 2;
        assert_eq!(parse(&b), Err("unsupported version"));
        let mut q = p.clone();
        q.q = [0.0, 0.0, 0.0, 2.0];
        assert!(parse(&encode(&q)).is_err());
        let mut g = p.clone();
        g.gyro = Some([f32::NAN, 0.0, 0.0]);
        assert!(parse(&encode(&g)).is_err());
        let mut none = p;
        none.gyro = None;
        none.accel = None;
        none.battery_pct = None;
        let back = parse(&encode(&none)).unwrap();
        assert_eq!(
            (back.gyro, back.accel, back.battery_pct),
            (None, None, None)
        );
    }
}
