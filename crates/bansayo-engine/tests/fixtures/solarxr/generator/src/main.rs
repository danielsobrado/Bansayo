//! Builds SolarXR test fixtures with the official generated code (SlimeVR/SolarXR-Protocol
//! 00c38a6) and verifies a StartDataFeed message produced elsewhere.
//! Usage: solarxr-fixtures <out_dir> [start_feed.bin to verify]
use solarxr_protocol::data_feed::device_data::*;
use solarxr_protocol::data_feed::tracker::*;
use solarxr_protocol::data_feed::*;
use solarxr_protocol::datatypes::hardware_info::*;
use solarxr_protocol::datatypes::math::Quat;
use solarxr_protocol::datatypes::*;
use solarxr_protocol::flatbuffers::FlatBufferBuilder;
use solarxr_protocol::*;

struct T<'s> {
    num: u8,
    part: BodyPart,
    status: TrackerStatus,
    q: Option<[f32; 4]>,
    adjusted: Option<[f32; 4]>,
    tps: Option<u16>,
    name: &'s str,
}

fn device<'a>(
    b: &mut FlatBufferBuilder<'a>,
    id: u8,
    hw: Option<&str>,
    battery: Option<u8>,
    rssi: Option<i16>,
    trackers: &[T],
) -> flatbuffers::WIPOffset<DeviceData<'a>> {
    let mut offs = vec![];
    for t in trackers {
        let did = DeviceId::new(id);
        let tid = TrackerId::create(b, &TrackerIdArgs { device_id: Some(&did), tracker_num: t.num });
        let name = b.create_string(t.name);
        let rate = HzF32::new(100.0);
        let info = TrackerInfo::create(
            b,
            &TrackerInfoArgs {
                body_part: t.part,
                is_imu: true,
                poll_rate: Some(&rate),
                display_name: Some(name),
                ..Default::default()
            },
        );
        let q = t.q.map(|q| Quat::new(q[0], q[1], q[2], q[3]));
        let a = t.adjusted.map(|q| Quat::new(q[0], q[1], q[2], q[3]));
        offs.push(TrackerData::create(
            b,
            &TrackerDataArgs {
                tracker_id: Some(tid),
                info: Some(info),
                status: t.status,
                rotation: q.as_ref(),
                rotation_reference_adjusted: a.as_ref(),
                tps: t.tps,
                ..Default::default()
            },
        ));
    }
    let trackers = b.create_vector(&offs);
    let hw = hw.map(|h| b.create_string(h));
    let info = HardwareInfo::create(
        b,
        &HardwareInfoArgs {
            hardware_identifier: hw,
            ..Default::default()
        },
    );
    let status = HardwareStatus::create(
        b,
        &HardwareStatusArgs {
            battery_pct_estimate: battery,
            rssi,
            ..Default::default()
        },
    );
    let did = DeviceId::new(id);
    DeviceData::create(
        b,
        &DeviceDataArgs {
            id: Some(&did),
            hardware_info: Some(info),
            hardware_status: Some(status),
            trackers: Some(trackers),
            ..Default::default()
        },
    )
}

fn bundle<'a>(b: &mut FlatBufferBuilder<'a>, devices: &[flatbuffers::WIPOffset<DeviceData<'a>>]) -> Vec<u8> {
    let devices = b.create_vector(devices);
    let update = DataFeedUpdate::create(
        b,
        &DataFeedUpdateArgs {
            devices: Some(devices),
            ..Default::default()
        },
    );
    let header = DataFeedMessageHeader::create(
        b,
        &DataFeedMessageHeaderArgs {
            message_type: DataFeedMessage::DataFeedUpdate,
            message: Some(update.as_union_value()),
        },
    );
    let msgs = b.create_vector(&[header]);
    let root = MessageBundle::create(
        b,
        &MessageBundleArgs {
            data_feed_msgs: Some(msgs),
            ..Default::default()
        },
    );
    b.finish(root, None);
    b.finished_data().to_vec()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    let h = std::f32::consts::FRAC_1_SQRT_2;
    // Two devices: a single-IMU tracker on the hip and a tracker with an extension.
    let mut b = FlatBufferBuilder::new();
    let d1 = device(
        &mut b,
        1,
        Some("AA:BB:CC:DD:EE:01"),
        Some(87),
        Some(-52),
        &[T { num: 0, part: BodyPart::HIP, status: TrackerStatus::OK, q: Some([0.0, h, 0.0, h]), adjusted: Some([0.0, 0.0, 0.0, 1.0]), tps: Some(100), name: "IMU Tracker 1" }],
    );
    let d2 = device(
        &mut b,
        2,
        Some("AA:BB:CC:DD:EE:02"),
        Some(41),
        None,
        &[
            T { num: 0, part: BodyPart::LEFT_UPPER_LEG, status: TrackerStatus::OK, q: Some([0.5, 0.5, 0.5, 0.5]), adjusted: None, tps: Some(98), name: "IMU Tracker 2" },
            T { num: 1, part: BodyPart::LEFT_LOWER_LEG, status: TrackerStatus::OCCLUDED, q: Some([0.0, 0.0, 0.2, 0.9]), adjusted: None, tps: None, name: "IMU Tracker 2/1" },
        ],
    );
    std::fs::write(out.join("update_two_devices.bin"), bundle(&mut b, &[d1, d2])).unwrap();
    // A device without a hardware identifier and a disconnected tracker without rotation.
    let mut b = FlatBufferBuilder::new();
    let d3 = device(
        &mut b,
        7,
        None,
        None,
        None,
        &[T { num: 0, part: BodyPart::NONE, status: TrackerStatus::DISCONNECTED, q: None, adjusted: None, tps: None, name: "Unknown" }],
    );
    std::fs::write(out.join("update_no_identifier.bin"), bundle(&mut b, &[d3])).unwrap();
    println!("fixtures written to {}", out.display());

    if let Some(path) = args.get(2) {
        let bytes = std::fs::read(path).unwrap();
        let bundle = flatbuffers::root::<MessageBundle>(&bytes).expect("verifies as MessageBundle");
        let msg = bundle.data_feed_msgs().unwrap().get(0);
        assert_eq!(msg.message_type(), DataFeedMessage::StartDataFeed);
        let start = msg.message_as_start_data_feed().unwrap();
        let cfg = start.data_feeds().unwrap().get(0);
        let mask = cfg.data_mask().unwrap();
        let t = mask.tracker_data().unwrap();
        println!(
            "StartDataFeed verified: minimum_time_since_last={} device_data={} info={} status={} rotation={} rotation_reference_adjusted={} tps={} position={} raw_angular_velocity={}",
            cfg.minimum_time_since_last(),
            mask.device_data(),
            t.info(),
            t.status(),
            t.rotation(),
            t.rotation_reference_adjusted(),
            t.tps(),
            t.position(),
            t.raw_angular_velocity()
        );
    }
}
