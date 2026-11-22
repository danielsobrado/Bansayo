//! Phase 01 OpenXR observations in the registry, the pipeline validator, the solver and the
//! recorded event shape, without a network or a headset.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config::{self, QualityConfig, XrConfig},
    pipeline::solve::solve,
    registry::Registry,
    skeleton::Skeleton,
    sources::{validation, xr},
};
use std::collections::BTreeMap;

fn on() -> XrConfig {
    XrConfig {
        enabled: true,
        ..Default::default()
    }
}
fn pose(device: &str, seq: u64, t_us: u64) -> SensorObservation {
    SensorObservation {
        device_id: DeviceId::new(device).unwrap(),
        source_id: SourceId::new(xr::SOURCE_ID).unwrap(),
        origin: Origin::Live,
        seq,
        t_us,
        device_time_us: Some(t_us + 9_000_000),
        time_quality: TimeQuality::ReceiveTime,
        calibration_ref: None,
        confidence: None,
        data: ObservationData::RigidPose {
            frame: FrameRef::XrStage,
            position_m: [0.0, 1.7, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        },
    }
}
fn link(device: &str, connected: bool, t_us: u64) -> SourceEvent {
    SourceEvent::DeviceLink {
        device_id: DeviceId::new(device).unwrap(),
        connected,
        t_us,
    }
}
fn registry() -> Registry {
    Registry::new(xr::descriptors(&on()), BTreeMap::new(), BTreeMap::new())
}
fn status(reg: &Registry, now: u64, id: &str) -> DeviceStatus {
    reg.status(now, &QualityConfig::default(), &Store::default())
        .into_iter()
        .find(|d| d.descriptor.id.as_str() == id)
        .unwrap()
}

#[test]
fn xr_devices_start_acquiring_without_assignment() {
    let reg = registry();
    let all = reg.status(0, &QualityConfig::default(), &Store::default());
    assert_eq!(all.len(), 3);
    for d in all {
        assert_eq!(d.state, TrackingState::Acquiring, "{}", d.descriptor.id);
        assert_eq!(d.assignment, None);
        assert!(
            d.calibration.is_empty(),
            "no IMU or camera calibration implied"
        );
    }
}

#[test]
fn xr_registry_counts_pose_as_primary_stream() {
    let mut reg = registry();
    // 60 Hz cadence: 16_667 us apart.
    for i in 0..60u64 {
        assert!(reg.accept(pose("XR-HMD", i, 1_000_000 + i * 16_667)));
    }
    let s = status(&reg, 1_000_000 + 59 * 16_667, "XR-HMD");
    assert!(
        (s.stream.measured_rate_hz - 60.0).abs() < 0.5,
        "{}",
        s.stream.measured_rate_hz
    );
    assert_eq!(s.state, TrackingState::Tracking);
    assert_eq!(s.stream.time_quality, TimeQuality::ReceiveTime);
    assert_eq!(s.stream.received, 60);
    // Other XR devices are independent and unaffected.
    assert_eq!(
        status(&reg, 2_000_000, "XR-LEFT-GRIP").state,
        TrackingState::Acquiring
    );
}

#[test]
fn xr_registry_reconnect_resets_sequence_floor() {
    let mut reg = registry();
    assert!(reg.event(link("XR-RIGHT-GRIP", true, 1)));
    assert!(reg.accept(pose("XR-RIGHT-GRIP", 50, 10)));
    assert!(
        !reg.accept(pose("XR-RIGHT-GRIP", 0, 20)),
        "old session floor holds"
    );
    assert!(reg.event(link("XR-RIGHT-GRIP", false, 30)));
    assert_eq!(
        status(&reg, 40, "XR-RIGHT-GRIP").state,
        TrackingState::Disconnected
    );
    assert!(reg.event(link("XR-RIGHT-GRIP", true, 50)));
    assert!(
        reg.accept(pose("XR-RIGHT-GRIP", 0, 60)),
        "new session seq 0"
    );
    // Stale data never reads as current: no frames for longer than disconnect_after_ms.
    let later = 60 + QualityConfig::default().disconnect_after_ms * 1000 + 1;
    assert_eq!(
        status(&reg, later, "XR-RIGHT-GRIP").state,
        TrackingState::Disconnected
    );
}

#[test]
fn xr_rigid_pose_validation_is_specific_to_xr_stage_devices() {
    let reg = registry();
    let d = &reg.entries.values().next().unwrap().descriptor;
    let ok = pose("XR-HMD", 0, 10);
    assert!(validation::observation(&ok, d, 10).is_ok());
    let with = |f: fn(&mut SensorObservation)| {
        let mut o = ok.clone();
        f(&mut o);
        validation::observation(&o, d, 10)
    };
    assert!(
        with(|o| o.data = ObservationData::RigidPose {
            frame: FrameRef::World,
            position_m: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0]
        })
        .is_err()
    );
    assert!(
        with(|o| o.data = ObservationData::RigidPose {
            frame: FrameRef::XrStage,
            position_m: [f32::NAN, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0]
        })
        .is_err()
    );
    assert!(
        with(|o| o.data = ObservationData::RigidPose {
            frame: FrameRef::XrStage,
            position_m: [0.0; 3],
            rotation: [0.0; 4]
        })
        .is_err()
    );
    assert!(with(|o| o.origin = Origin::Simulated).is_err());
    // An IMU descriptor cannot carry a rigid pose, and XR cannot carry an IMU orientation.
    let mut imu = d.clone();
    imu.kind = DeviceKind::ImuTracker;
    imu.capabilities.push(SensorCapability::Orientation);
    assert!(validation::observation(&ok, &imu, 10).is_err());
    let mut orientation = ok.clone();
    orientation.data = ObservationData::Orientation {
        frame: FrameRef::SensorReference {
            id: ok.device_id.clone(),
        },
        q: [0.0, 0.0, 0.0, 1.0],
    };
    assert!(validation::observation(&orientation, d, 10).is_err());
}

#[test]
fn xr_does_not_affect_camera_imu_solver() {
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let imu = DeviceDescriptor {
        id: DeviceId::new("T1").unwrap(),
        source_id: SourceId::new("sim").unwrap(),
        kind: DeviceKind::ImuTracker,
        origin: Origin::Simulated,
        model: None,
        firmware: None,
        transport: Transport::Simulated,
        capabilities: vec![SensorCapability::Orientation],
        nominal_rate_hz: Some(100.0),
        camera: None,
    };
    let assignments = BTreeMap::from([(imu.id.clone(), SegmentId::new("pelvis").unwrap())]);
    let orientation = SensorObservation {
        device_id: imu.id.clone(),
        source_id: imu.source_id.clone(),
        origin: Origin::Simulated,
        seq: 1,
        t_us: 1000,
        device_time_us: None,
        time_quality: TimeQuality::Synchronized,
        calibration_ref: None,
        confidence: Some(1.0),
        data: ObservationData::Orientation {
            frame: FrameRef::SensorReference { id: imu.id.clone() },
            q: [0.0, 0.3826834, 0.0, 0.9238795],
        },
    };
    let mut without = Registry::new(vec![imu.clone()], assignments.clone(), BTreeMap::new());
    let mut with = Registry::new(
        [vec![imu], xr::descriptors(&on())].concat(),
        assignments,
        BTreeMap::new(),
    );
    assert!(without.accept(orientation.clone()));
    assert!(with.accept(orientation));
    for (i, id) in xr::DEVICE_IDS.iter().enumerate() {
        assert!(with.accept(pose(id, 0, 900 + i as u64)));
    }
    let config = config::parse("schema_version: 1").unwrap().config;
    let a = solve(&sk, &without, &Store::default(), 1010, &config);
    let b = solve(&sk, &with, &Store::default(), 1010, &config);
    assert!(
        a.pose.is_some(),
        "the fixture must produce a pose to compare"
    );
    let json = |p: &Option<SkeletonPose>| serde_json::to_value(p).unwrap();
    assert_eq!(json(&a.pose), json(&b.pose));
    assert_eq!(a.observed, b.observed);
    assert_eq!(a.methods, b.methods);
    assert_eq!(
        serde_json::to_value(&a.quality).unwrap(),
        serde_json::to_value(&b.quality).unwrap()
    );
}

#[test]
fn xr_source_is_recording_compatible() {
    // The recorder stores `SourceEvent` as JSON; replay decodes and re-applies it.
    let events = vec![
        link("XR-HMD", true, 100),
        SourceEvent::Observation {
            observation: pose("XR-HMD", 0, 100),
        },
        link("XR-HMD", false, 200),
    ];
    let mut live = registry();
    let mut replayed = registry();
    for e in &events {
        assert!(live.event(e.clone()));
        let back: SourceEvent = serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap();
        assert_eq!(&back, e);
        assert!(replayed.event(back));
    }
    let s = |r: &Registry| serde_json::to_value(status(r, 300, "XR-HMD")).unwrap();
    assert_eq!(s(&live), s(&replayed));
}
