use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config,
    pipeline::solve::solve,
    registry::{LANDMARK_HISTORY, Registry},
    skeleton::Skeleton,
    sources::validation,
};
use std::collections::BTreeMap;
fn setup() -> (Skeleton, Registry, Store, SensorObservation) {
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let id = DeviceId::new("TRACKER").unwrap();
    let source = SourceId::new("sim").unwrap();
    let d = DeviceDescriptor {
        id: id.clone(),
        source_id: source.clone(),
        kind: DeviceKind::ImuTracker,
        origin: Origin::Simulated,
        model: None,
        firmware: None,
        transport: Transport::Simulated,
        capabilities: vec![SensorCapability::Orientation],
        nominal_rate_hz: Some(100.0),
        camera: None,
    };
    let reg = Registry::new(
        vec![d],
        BTreeMap::from([(id.clone(), SegmentId::new("pelvis").unwrap())]),
        BTreeMap::new(),
    );
    let mut store = Store::default();
    store.insert(vec![CalibrationProfile {
        schema_version: 1,
        id: ProfileId::new("mounting-TRACKER-pelvis").unwrap(),
        kind: CalibrationKind::Mounting,
        method: "reference_pose".into(),
        origin: Origin::Simulated,
        created_utc: "2026-11-17T00:00:00Z".into(),
        subject: CalibrationSubject {
            device_id: Some(id.clone()),
            segment_id: Some(SegmentId::new("pelvis").unwrap()),
        },
        depends_on: vec![],
        data: serde_json::json!({"q_sensor_from_segment":[0,0,0,1]})
            .as_object()
            .unwrap()
            .clone(),
        quality: Default::default(),
    }]);
    let o = SensorObservation {
        device_id: id.clone(),
        source_id: source,
        origin: Origin::Simulated,
        seq: 1,
        t_us: 1000,
        device_time_us: Some(1000),
        time_quality: TimeQuality::Estimated,
        calibration_ref: None,
        confidence: Some(1.0),
        data: ObservationData::Orientation {
            frame: FrameRef::SensorReference { id },
            q: [0.0, 0.0, 0.0, 1.0],
        },
    };
    (sk, reg, store, o)
}
#[test]
fn diagnostics_do_not_replace_orientation_and_sequences_are_per_kind() {
    let (sk, mut reg, store, o) = setup();
    assert!(reg.accept(o.clone()));
    let mut diagnostic = o.clone();
    diagnostic.t_us += 1;
    diagnostic.data = ObservationData::Diagnostics {
        battery_level: Some(0.8),
        charging: None,
        rssi_dbm: None,
        quality: Some(1.0),
    };
    assert!(
        reg.accept(diagnostic.clone()),
        "first diagnostic may use the same seq as orientation"
    );
    assert!(!reg.accept(diagnostic));
    let solved = solve(
        &sk,
        &reg,
        &store,
        1010,
        &config::parse("schema_version: 1").unwrap().config,
    );
    assert!(solved.observed[0]);
    assert!(solved.pose.is_some());
    let mut next = o.clone();
    next.seq = 2;
    next.t_us = 1002;
    assert!(reg.accept(next.clone()));
    assert!(!reg.accept(o));
    next.seq = 4;
    next.t_us = 999;
    assert!(!reg.accept(next));
    assert_eq!(LANDMARK_HISTORY, 8);
}
#[test]
fn imu_reference_frame_and_nominal_rate_validation_remain_pinned() {
    let (_, reg, _, mut o) = setup();
    let mut d = reg.entries.values().next().unwrap().descriptor.clone();
    assert!(validation::observation(&o, &d, 1000).is_ok());
    for frame in [
        FrameRef::World,
        FrameRef::SensorReference {
            id: DeviceId::new("OTHER").unwrap(),
        },
        FrameRef::Sensor {
            id: o.device_id.clone(),
        },
    ] {
        o.data = ObservationData::Orientation {
            frame,
            q: [0.0, 0.0, 0.0, 1.0],
        };
        assert!(validation::observation(&o, &d, 1000).is_err());
    }
    for rate in [0.0, -1.0, 1000.01, f32::INFINITY, f32::NAN] {
        d.nominal_rate_hz = Some(rate);
        assert!(validation::descriptor(&d).is_err());
    }
    d.nominal_rate_hz = Some(1000.0);
    assert!(validation::descriptor(&d).is_ok());
}
