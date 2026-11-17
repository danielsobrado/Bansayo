use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config,
    pipeline::{features::capability_report, motion::sample, solve::solve},
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{Control, Scheduler, SimulatedSource, camera, motions::Motion},
};
use std::collections::BTreeMap;
fn rig(yaml: &str) -> (config::Validated, Skeleton, Registry, Store) {
    let cfg = config::parse(yaml).unwrap();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let mut registry = Registry::new(vec![], cfg.assignments.clone(), BTreeMap::new());
    let mut store = Store::default();
    for spec in &cfg.simulated {
        let source = SimulatedSource {
            spec: spec.clone(),
            skeleton: sk.clone(),
            assignments: tokio::sync::watch::channel(cfg.assignments.clone()).1,
        };
        for descriptor in source.descriptors() {
            registry.event(SourceEvent::DeviceAnnounced { descriptor });
        }
        store.insert(
            spec.cameras
                .iter()
                .flat_map(|c| camera::profiles(c, "2026-11-17T00:00:00Z"))
                .collect(),
        );
        let mut scheduler = Scheduler::new(
            spec.clone(),
            sk.clone(),
            0,
            Control {
                motion: Motion::ReferencePose,
                speed: 1.0,
                paused: false,
                generation: 0,
                suspended: false,
            },
        );
        scheduler.step(1_000_000, &cfg.assignments, &BTreeMap::new(), |_, e| {
            registry.event(e);
        });
    }
    for (device, segment) in &cfg.assignments {
        let entry = &registry.entries[device];
        let o = &entry.latest[&bansayo_engine::registry::ObservationKind::Orientation];
        let ObservationData::Orientation { q, .. } = o.data else {
            unreachable!()
        };
        let mounting = bansayo_engine::math::rot_to_wire(
            &bansayo_engine::math::rot_from_wire(q).unwrap().inverse(),
        );
        store.insert(vec![CalibrationProfile {
            schema_version: 1,
            id: ProfileId::new(format!("mounting-{device}-{segment}")).unwrap(),
            kind: CalibrationKind::Mounting,
            method: "reference_pose".into(),
            origin: Origin::Simulated,
            created_utc: "2026-11-17T00:00:00Z".into(),
            subject: CalibrationSubject {
                device_id: Some(device.clone()),
                segment_id: Some(segment.clone()),
            },
            depends_on: vec![],
            data: serde_json::json!({"q_sensor_from_segment":mounting})
                .as_object()
                .unwrap()
                .clone(),
            quality: Default::default(),
        }]);
    }
    (cfg, sk, registry, store)
}
#[test]
fn simulator_profiles_come_from_contributing_inputs_and_hashes_are_stable() {
    for (yaml, expected) in [
        (
            include_str!("../../../config/sim-full-body.yaml"),
            CapabilityProfileId::Inertial,
        ),
        (
            include_str!("../../../config/sim-six-trackers.yaml"),
            CapabilityProfileId::Inertial,
        ),
        (
            include_str!("../../../config/sim-camera-only.yaml"),
            CapabilityProfileId::Basic,
        ),
        ("schema_version: 1", CapabilityProfileId::None),
        (
            include_str!("../../../config/sim-multi-camera.yaml"),
            CapabilityProfileId::Hybrid,
        ),
        (
            include_str!("../../../config/sim-cameras-skeleton.yaml"),
            CapabilityProfileId::MultiView,
        ),
    ] {
        let (cfg, sk, reg, store) = rig(yaml);
        let solved = solve(&sk, &reg, &store, 1_000_000, &cfg.config);
        let report = capability_report(&sk, &reg, &store, &solved, &cfg.config, 1_000_000);
        assert_eq!(report.profile, expected);
        assert_eq!(report.tracking_config_id.len(), 16);
        assert_eq!(report.calibration_set_id.len(), 16);
        let mut changed = cfg.config.clone();
        changed.api.bind = "127.0.0.1:9999".into();
        changed.engine.data_dir = "/tmp/irrelevant".into();
        assert_eq!(
            report.tracking_config_id,
            capability_report(&sk, &reg, &store, &solved, &changed, 1_000_001).tracking_config_id
        );
        changed.triangulation.min_ray_angle_deg += 1.0;
        assert_ne!(
            report.tracking_config_id,
            capability_report(&sk, &reg, &store, &solved, &changed, 1_000_000).tracking_config_id
        );
    }
}
#[test]
fn motion_has_26_keypoints_observation_times_and_unavailable_gyro() {
    let (cfg, sk, reg, store) = rig(include_str!("../../../config/sim-full-body.yaml"));
    let solved = solve(&sk, &reg, &store, 1_010_000, &cfg.config);
    let m = sample(&sk, &reg, &solved, 42, 1_010_000, vec![Origin::Simulated]);
    assert_eq!(m.seq, 42);
    assert!(m.planar.is_empty());
    let b = m.body.unwrap();
    assert_eq!(b.keypoints.len(), 26);
    assert_eq!(b.keypoints[0].as_ref(), Some(&b.root));
    assert_eq!(b.segments[0].axis.method, MethodId::ImuMounted);
    assert_eq!(b.segments[0].axis.t_obs_us, Some(1_000_000));
    assert_eq!(
        b.segments[0].angular_velocity.provenance,
        Provenance::Unavailable
    );
    assert!(b.segments[0].angular_velocity_rad_s.is_none());
}
#[test]
fn camera_fit_never_claims_observed_twist_and_stale_sources_lower_profile() {
    let (cfg, sk, mut reg, store) = rig(include_str!("../../../config/sim-cameras-skeleton.yaml"));
    let solved = solve(&sk, &reg, &store, 1_000_000, &cfg.config);
    let b = sample(&sk, &reg, &solved, 1, 1_000_000, vec![Origin::Simulated])
        .body
        .unwrap();
    assert!(
        b.segments
            .iter()
            .all(|s| s.twist.provenance == Provenance::Inferred)
    );
    for entry in reg.entries.values_mut() {
        entry.connected = false;
    }
    let solved = solve(&sk, &reg, &store, 2_000_000, &cfg.config);
    assert_eq!(
        capability_report(&sk, &reg, &store, &solved, &cfg.config, 2_000_000).profile,
        CapabilityProfileId::None
    );
}
