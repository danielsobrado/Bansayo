use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store, config, recording::Context, registry::Registry, skeleton::Skeleton,
    sources::simulated::SimulatedSource,
};
use std::collections::BTreeMap;
fn context() -> Context {
    let dir = tempfile::tempdir().unwrap();
    let config=config::parse(&format!("schema_version: 1\nengine: {{data_dir: {:?}}}\nassignments: {{T1: pelvis}}\nsources:\n  - {{id: sim,kind: simulated,trackers: [{{id: T1,rate_hz: 100}}]}}\n",dir.path().display().to_string())).unwrap();
    let skeleton = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let source = SimulatedSource {
        spec: config.simulated[0].clone(),
        skeleton: skeleton.clone(),
        assignments: tokio::sync::watch::channel(config.assignments.clone()).1,
    };
    let registry = Registry::new(
        source.descriptors(),
        config.assignments.clone(),
        BTreeMap::new(),
    );
    Context::snapshot(
        &registry,
        &Store::default(),
        &skeleton,
        &config.config,
        1_000_000,
    )
}
#[test]
fn rejects_invalid_recorded_context_at_file_boundary() {
    let valid = context();
    assert!(valid.restore().is_ok());
    let mut c = valid.clone();
    c.devices.push(c.devices[0].clone());
    assert!(c.restore().is_err(), "duplicate device");
    let mut c = valid.clone();
    c.assignments.insert(
        DeviceId::new("unknown").unwrap(),
        SegmentId::new("head").unwrap(),
    );
    assert!(c.restore().is_err(), "unknown assignment device");
    let mut c = valid.clone();
    c.qualities.insert(DeviceId::new("T1").unwrap(), 2.0);
    assert!(c.restore().is_err(), "invalid reported quality");
    let mut c = valid.clone();
    c.latest.push(SensorObservation {
        device_id: DeviceId::new("T1").unwrap(),
        source_id: SourceId::new("sim").unwrap(),
        origin: Origin::Simulated,
        seq: 1,
        t_us: 900_000,
        device_time_us: None,
        time_quality: TimeQuality::Synchronized,
        calibration_ref: None,
        confidence: Some(1.0),
        data: ObservationData::Orientation {
            frame: FrameRef::SensorReference {
                id: DeviceId::new("T1").unwrap(),
            },
            q: [0.0; 4],
        },
    });
    assert!(c.restore().is_err(), "invalid orientation");
}
#[test]
fn recording_started_with_a_disconnected_tracker_restores_its_link_state() {
    let mut c = context();
    c.links.insert(DeviceId::new("T1").unwrap(), false);
    c.qualities.insert(DeviceId::new("T1").unwrap(), 0.3);
    let (registry, store, _) = c.restore().unwrap();
    let status = registry.status(c.started_t_us, &c.config.quality, &store);
    assert_eq!(status[0].state, TrackingState::Disconnected);
    assert!(
        status[0]
            .reasons
            .iter()
            .any(|r| r.code == "device_disconnected")
    );
    assert!(!registry.entries[&DeviceId::new("T1").unwrap()].connected);
}
