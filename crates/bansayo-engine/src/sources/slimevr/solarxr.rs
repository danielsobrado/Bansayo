//! The part of the SolarXR protocol this source uses, from the official schemas
//! (github.com/SlimeVR/SolarXR-Protocol, commit 00c38a6, `schema/all.fbs` and `schema/data_feed/`).
//! Field slots are the `VT_*` constants of the official generated Rust code. SlimeVR Server
//! (`WebsocketAPI.java`, port 21110) exchanges one FlatBuffers `MessageBundle` per binary
//! WebSocket message, without a file identifier.
//!
//! Decoding reads only what it needs through a bounds-checked reader, so a malformed message is
//! an error, never a panic or an out-of-bounds read. Encoding uses the official runtime.

/// `DataFeedMessage` union tags (`data_feed.fbs`).
const START_DATA_FEED: u8 = 2;
const DATA_FEED_UPDATE: u8 = 3;
/// `TrackerStatus` (`datatypes.fbs`); SlimeVR Server sends its own status id + 1.
pub const STATUS_OK: u8 = 2;
pub const STATUS_BUSY: u8 = 3;
pub const STATUS_OCCLUDED: u8 = 5;

/// Most devices and trackers per device read from one message.
const MAX_DEVICES: usize = 256;
const MAX_TRACKERS: usize = 32;

/// One tracker in a `DataFeedUpdate`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracker {
    /// `<hardware_identifier>/<tracker_num>` when the server sends a hardware identifier (stable
    /// across reconnects); otherwise `device-<id>/<num>`, which SolarXR does not keep stable.
    pub key: String,
    pub stable: bool,
    pub name: Option<String>,
    /// `BodyPart` the user assigned in SlimeVR, informational.
    pub body_part: Option<u8>,
    pub status: Option<u8>,
    pub is_imu: Option<bool>,
    /// `rotation`: the sensor's fused orientation (`Tracker.getRawRotation()` in the server).
    pub rotation: Option<[f32; 4]>,
    /// `rotation_reference_adjusted`: with SlimeVR's own mounting and reset applied.
    pub rotation_reference_adjusted: Option<[f32; 4]>,
    /// Server-processed data ticks per second.
    pub tps: Option<u16>,
    pub poll_rate_hz: Option<f32>,
    pub battery_pct: Option<u8>,
    pub rssi_dbm: Option<i16>,
}

/// Which rotation a tracker publishes as its observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationField {
    /// The sensor's own fused orientation; Bansayo calibrates mounting and heading.
    #[default]
    Raw,
    /// SlimeVR's mounting- and reset-adjusted rotation. Then calibrate Bansayo mounting in the
    /// same pose as SlimeVR's reset, or the two calibrations are applied twice.
    ReferenceAdjusted,
}

/// A `MessageBundle` holding one `StartDataFeed` asking for device data with tracker info,
/// status, both rotations and tps, at most every `min_interval_ms`.
pub fn start_data_feed(min_interval_ms: u16) -> Vec<u8> {
    let mut b = flatbuffers::FlatBufferBuilder::new();
    // TrackerDataMask: info 4, status 6, rotation 8, rotation_reference_adjusted 20, tps 24.
    let t = b.start_table();
    for slot in [4, 6, 8, 20, 24] {
        b.push_slot::<bool>(slot, true, false);
    }
    let tracker_mask = b.end_table(t);
    // DeviceDataMask: tracker_data 4, device_data 6 (the server sends nothing without it).
    let d = b.start_table();
    b.push_slot_always(4, tracker_mask);
    b.push_slot::<bool>(6, true, false);
    let device_mask = b.end_table(d);
    // DataFeedConfig: minimum_time_since_last 4, data_mask 6.
    let c = b.start_table();
    b.push_slot::<u16>(4, min_interval_ms, 0);
    b.push_slot_always(6, device_mask);
    let config = b.end_table(c);
    let feeds = b.create_vector(&[config]);
    // StartDataFeed: data_feeds 4.
    let s = b.start_table();
    b.push_slot_always(4, feeds);
    let start = b.end_table(s);
    // DataFeedMessageHeader: message_type 4, message 6.
    let h = b.start_table();
    b.push_slot::<u8>(4, START_DATA_FEED, 0);
    b.push_slot_always(6, start);
    let header = b.end_table(h);
    let messages = b.create_vector(&[header]);
    // MessageBundle: data_feed_msgs 4.
    let r = b.start_table();
    b.push_slot_always(4, messages);
    let root = b.end_table(r);
    b.finish(root, None);
    b.finished_data().to_vec()
}

#[derive(Clone, Copy)]
struct Table<'a> {
    buf: &'a [u8],
    pos: usize,
    vtable: usize,
    vtable_len: usize,
}

fn read<const N: usize>(buf: &[u8], at: usize) -> Option<[u8; N]> {
    buf.get(at..at.checked_add(N)?)?.try_into().ok()
}
fn u32_at(buf: &[u8], at: usize) -> Option<usize> {
    read::<4>(buf, at).map(|b| u32::from_le_bytes(b) as usize)
}

impl<'a> Table<'a> {
    fn at(buf: &'a [u8], pos: usize) -> Option<Self> {
        let soffset = i32::from_le_bytes(read::<4>(buf, pos)?) as i64;
        let vtable = usize::try_from(pos as i64 - soffset).ok()?;
        let vtable_len = u16::from_le_bytes(read::<2>(buf, vtable)?) as usize;
        (vtable_len >= 4).then_some(Self {
            buf,
            pos,
            vtable,
            vtable_len,
        })
    }
    fn field(&self, slot: usize) -> Option<usize> {
        if slot + 2 > self.vtable_len {
            return None;
        }
        let off = u16::from_le_bytes(read::<2>(self.buf, self.vtable + slot)?) as usize;
        (off != 0).then_some(self.pos + off)
    }
    fn indirect(&self, slot: usize) -> Option<usize> {
        let p = self.field(slot)?;
        p.checked_add(u32_at(self.buf, p)?)
    }
    fn table(&self, slot: usize) -> Option<Table<'a>> {
        Table::at(self.buf, self.indirect(slot)?)
    }
    fn tables(&self, slot: usize, max: usize) -> Vec<Table<'a>> {
        let Some(v) = self.indirect(slot) else {
            return vec![];
        };
        let len = u32_at(self.buf, v).unwrap_or(0).min(max);
        (0..len)
            .filter_map(|i| {
                let e = v + 4 + i * 4;
                Table::at(self.buf, e.checked_add(u32_at(self.buf, e)?)?)
            })
            .collect()
    }
    fn string(&self, slot: usize) -> Option<String> {
        let s = self.indirect(slot)?;
        let len = u32_at(self.buf, s)?;
        let bytes = self.buf.get(s + 4..(s + 4).checked_add(len)?)?;
        std::str::from_utf8(bytes).ok().map(str::to_owned)
    }
    fn u8(&self, slot: usize) -> Option<u8> {
        read::<1>(self.buf, self.field(slot)?).map(|b| b[0])
    }
    fn bool(&self, slot: usize) -> Option<bool> {
        self.u8(slot).map(|v| v != 0)
    }
    fn u16(&self, slot: usize) -> Option<u16> {
        read::<2>(self.buf, self.field(slot)?).map(u16::from_le_bytes)
    }
    fn i16(&self, slot: usize) -> Option<i16> {
        read::<2>(self.buf, self.field(slot)?).map(i16::from_le_bytes)
    }
    fn f32(&self, slot: usize) -> Option<f32> {
        read::<4>(self.buf, self.field(slot)?).map(f32::from_le_bytes)
    }
    /// `math.Quat` struct, stored inline: x, y, z, w.
    fn quat(&self, slot: usize) -> Option<[f32; 4]> {
        let p = self.field(slot)?;
        let mut q = [0.0; 4];
        for (i, v) in q.iter_mut().enumerate() {
            *v = f32::from_le_bytes(read::<4>(self.buf, p + i * 4)?);
        }
        Some(q)
    }
}

/// Trackers of every `DataFeedUpdate` in a `MessageBundle`. Other messages (RPC, pub-sub) are
/// ignored. Synthetic (computed) trackers are not read: they are SlimeVR's skeleton, not sensors.
pub fn decode(bytes: &[u8]) -> Result<Vec<Tracker>, String> {
    let root = u32_at(bytes, 0)
        .and_then(|p| Table::at(bytes, p))
        .ok_or("not a FlatBuffers MessageBundle")?;
    let mut out = vec![];
    for header in root.tables(4, 64) {
        if header.u8(4) != Some(DATA_FEED_UPDATE) {
            continue;
        }
        let update = header.table(6).ok_or("DataFeedUpdate without a body")?;
        for device in update.tables(4, MAX_DEVICES) {
            // DeviceData: id 4 (struct DeviceId { id: u8 }), hardware_info 8, hardware_status 10,
            // trackers 12.
            let device_id = device.u8(4);
            let hardware = device
                .table(8)
                .and_then(|h| h.string(24)) // HardwareInfo.hardware_identifier
                .filter(|s| !s.trim().is_empty() && s != "Unknown");
            let status = device.table(10);
            let battery_pct = status.and_then(|s| s.u8(16)); // HardwareStatus.battery_pct_estimate
            let rssi_dbm = status.and_then(|s| s.i16(10)); // HardwareStatus.rssi
            for t in device.tables(12, MAX_TRACKERS) {
                // TrackerData: tracker_id 4, info 6, status 8, rotation 10,
                // rotation_reference_adjusted 22, tps 26.
                let id = t.table(4);
                let num = id.and_then(|i| i.u8(6)).unwrap_or(0); // TrackerId.tracker_num
                let info = t.table(6);
                let (key, stable) = match &hardware {
                    Some(h) => (format!("{h}/{num}"), true),
                    None => (
                        format!(
                            "device-{}/{num}",
                            id.and_then(|i| i.u8(4)).or(device_id).unwrap_or(0)
                        ),
                        false,
                    ),
                };
                out.push(Tracker {
                    key,
                    stable,
                    // TrackerInfo: body_part 6, poll_rate 8 (struct HzF32), is_imu 16,
                    // display_name 18, custom_name 20.
                    name: info.and_then(|i| i.string(20).or_else(|| i.string(18))),
                    body_part: info.and_then(|i| i.u8(6)),
                    is_imu: info.and_then(|i| i.bool(16)),
                    poll_rate_hz: info.and_then(|i| i.f32(8)),
                    status: t.u8(8),
                    rotation: t.quat(10),
                    rotation_reference_adjusted: t.quat(22),
                    tps: t.u16(26),
                    battery_pct,
                    rssi_dbm,
                });
            }
        }
    }
    Ok(out)
}

/// SlimeVR Server rotations are right-handed and +Y up (it rotates firmware quaternions by −90°
/// about X, `AXES_OFFSET` in `TrackersUDPServer.kt`), the same axes as Bansayo's world frame; the
/// heading of the reference frame is arbitrary and is what heading alignment calibrates. So the
/// conversion is a validation and normalisation, not an axis change. Order is `[x, y, z, w]` in
/// both. Returns `None` for a non-finite or far-from-unit quaternion.
pub fn canonical(q: [f32; 4]) -> Option<[f32; 4]> {
    let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !n.is_finite() || !(0.9..=1.1).contains(&n) {
        return None;
    }
    // Already unit to float precision: kept bit for bit, as received.
    let mut out = if (n - 1.0).abs() > 1e-5 {
        q.map(|v| v / n)
    } else {
        q
    };
    // One hemisphere, so identical rotations compare equal.
    if out[3] < 0.0 {
        out = out.map(|v| -v);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/solarxr/");

    #[test]
    fn decodes_a_fixture_built_by_the_official_generated_code() {
        let bytes = std::fs::read(format!("{FIXTURES}update_two_devices.bin")).unwrap();
        let t = decode(&bytes).unwrap();
        assert_eq!(t.len(), 3);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(t[0].key, "AA:BB:CC:DD:EE:01/0");
        assert!(t[0].stable);
        assert_eq!(t[0].name.as_deref(), Some("IMU Tracker 1"));
        assert_eq!(t[0].body_part, Some(5), "BodyPart::HIP");
        assert_eq!(t[0].status, Some(STATUS_OK));
        assert_eq!(t[0].rotation, Some([0.0, h, 0.0, h]));
        assert_eq!(t[0].rotation_reference_adjusted, Some([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(
            (t[0].tps, t[0].battery_pct, t[0].rssi_dbm),
            (Some(100), Some(87), Some(-52))
        );
        assert_eq!(t[0].poll_rate_hz, Some(100.0));
        assert_eq!(t[0].is_imu, Some(true));
        assert_eq!(t[1].key, "AA:BB:CC:DD:EE:02/0");
        assert_eq!(t[2].key, "AA:BB:CC:DD:EE:02/1");
        assert_eq!(t[2].status, Some(STATUS_OCCLUDED));
        assert_eq!(t[2].rotation_reference_adjusted, None);
        assert_eq!((t[2].tps, t[2].rssi_dbm), (None, None));
    }

    #[test]
    fn a_device_without_identifier_is_flagged_unstable_and_missing_rotation_is_none() {
        let bytes = std::fs::read(format!("{FIXTURES}update_no_identifier.bin")).unwrap();
        let t = decode(&bytes).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].key, "device-7/0");
        assert!(!t[0].stable);
        assert_eq!(t[0].rotation, None);
        assert_eq!(t[0].status, Some(1), "DISCONNECTED");
    }

    #[test]
    fn malformed_messages_are_errors_not_panics() {
        let bytes = std::fs::read(format!("{FIXTURES}update_two_devices.bin")).unwrap();
        assert!(decode(&[]).is_err());
        assert!(decode(&[0xff, 0xff, 0xff, 0x7f]).is_err());
        for cut in 0..bytes.len() {
            let _ = decode(&bytes[..cut]);
        }
        let mut flipped = bytes.clone();
        for i in 0..flipped.len() {
            flipped[i] ^= 0xa5;
            let _ = decode(&flipped);
            flipped[i] ^= 0xa5;
        }
    }

    /// The request this source sends, written for the official decoder to verify:
    /// `BANSAYO_SOLARXR_DUMP=<file> cargo test -p bansayo-engine start_data_feed` then
    /// `solarxr-fixtures <dir> <file>` (tests/fixtures/solarxr/README.md).
    #[test]
    fn start_data_feed_round_trips_through_the_reader() {
        let bytes = start_data_feed(5);
        if let Ok(path) = std::env::var("BANSAYO_SOLARXR_DUMP") {
            std::fs::write(path, &bytes).unwrap();
        }
        let root = Table::at(&bytes, u32_at(&bytes, 0).unwrap()).unwrap();
        let header = root.tables(4, 4)[0];
        assert_eq!(header.u8(4), Some(START_DATA_FEED));
        let config = header.table(6).unwrap().tables(4, 4)[0];
        assert_eq!(config.u16(4), Some(5));
        let mask = config.table(6).unwrap();
        assert_eq!(mask.bool(6), Some(true));
        let t = mask.table(4).unwrap();
        for slot in [4, 6, 8, 20, 24] {
            assert_eq!(t.bool(slot), Some(true), "slot {slot}");
        }
        assert_eq!(t.bool(10), None, "position is not requested");
    }

    #[test]
    fn canonical_rotation_keeps_axes_and_rejects_garbage() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        // 90° about +Y stays 90° about +Y: no axis change between SlimeVR and Bansayo.
        assert_eq!(canonical([0.0, h, 0.0, h]), Some([0.0, h, 0.0, h]));
        assert_eq!(canonical([0.0, -h, 0.0, -h]), Some([0.0, h, 0.0, h]));
        let n = canonical([0.0, 0.0, 0.0, 1.05]).unwrap();
        assert!((n[3] - 1.0).abs() < 1e-6);
        assert_eq!(canonical([0.0, 0.0, 0.0, 0.0]), None);
        assert_eq!(canonical([f32::NAN, 0.0, 0.0, 1.0]), None);
        assert_eq!(canonical([0.0, 0.0, 0.0, 2.0]), None);
    }
}
