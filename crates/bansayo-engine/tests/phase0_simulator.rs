use bansayo_contracts::*;
use bansayo_engine::{
    config,
    pipeline::joints::measure,
    skeleton::Skeleton,
    sources::simulated::{Control, Scheduler, SimulatedSource, body::body_pose, motions::Motion},
};
use std::collections::BTreeMap;
#[test]
fn feet_schema_and_gyro_streams_are_declared_and_emitted() {
    let cfg=config::parse("schema_version: 1\nassignments: {T1: right_thigh}\nsources:\n  - id: sim\n    kind: simulated\n    landmark_schema: bansayo.body.v1.points\n    motion: roundhouse_kick\n    trackers: [{id: T1, rate_hz: 100}]\n    cameras: [{id: CAM1, rate_hz: 30, resolution_px: [1280,720], fov_y_deg: 70, position_m: [0,1.2,3.2], look_at_m: [0,1,0]}]\n").unwrap();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let spec = cfg.simulated[0].clone();
    let source = SimulatedSource {
        spec: spec.clone(),
        skeleton: sk.clone(),
        assignments: tokio::sync::watch::channel(cfg.assignments.clone()).1,
    };
    let ds = source.descriptors();
    assert!(
        ds[0]
            .capabilities
            .contains(&SensorCapability::AngularVelocity)
    );
    assert_eq!(
        ds[1]
            .camera
            .as_ref()
            .unwrap()
            .landmark_schema
            .as_ref()
            .unwrap()
            .names
            .len(),
        26
    );
    let mut scheduler = Scheduler::new(
        spec,
        sk.clone(),
        0,
        Control {
            motion: Motion::RoundhouseKick,
            speed: 1.0,
            paused: false,
            generation: 0,
            suspended: false,
        },
    );
    let mut obs = vec![];
    for i in 0..100 {
        scheduler.step(i * 10_000, &cfg.assignments, &BTreeMap::new(), |_, e| {
            if let SourceEvent::Observation { observation } = e {
                obs.push(observation)
            }
        });
    }
    let gyros: Vec<_> = obs
        .iter()
        .filter_map(|o| match &o.data {
            ObservationData::AngularVelocity { frame, rad_s } => {
                assert!(matches!(frame,FrameRef::Sensor{id} if id==&o.device_id));
                Some(rad_s)
            }
            _ => None,
        })
        .collect();
    assert_eq!(gyros.len(), 100);
    assert!(gyros.iter().any(|v| v.iter().any(|x| x.abs() > 0.1)));
    let landmarks = obs
        .iter()
        .find_map(|o| match &o.data {
            ObservationData::Landmarks2d { landmarks } => Some(landmarks),
            _ => None,
        })
        .unwrap();
    assert_eq!(landmarks.schema_id, BODY_KEYPOINT_SCHEMA_ID);
    assert_eq!(landmarks.points.len(), 26);
    assert!(landmarks.points[22].is_some());
    assert!(landmarks.points[24].is_some());
    let old = body_pose(&sk, Motion::ReferencePose, 0.0);
    let points = old.keypoints(&sk);
    assert_eq!(points.len(), 26);
    assert!((points[22] - points[24]).norm() > 0.2);
}
#[test]
fn kicks_expose_scripted_phases_and_known_peaks() {
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    for motion in [Motion::FrontKick, Motion::RoundhouseKick] {
        let truth = motion.kick_truth().unwrap();
        assert!(truth.phases.len() >= 4);
        assert!(truth.strike_t_s > 0.0 && truth.strike_t_s < truth.period_s);
        for pair in truth.phases.windows(2) {
            assert_eq!(pair[0].end_s, pair[1].start_s);
        }
        for peak in &truth.peaks {
            let pose = body_pose(&sk, motion, peak.t_s);
            let joints = measure(&sk, &pose.rotations, &vec![true; sk.len()]);
            if peak.id == "right_knee_flexion" {
                assert!((joints[1].value.unwrap() as f64 - peak.value).abs() < 1e-6);
            }
        }
        assert!(body_pose(&sk, motion, truth.strike_t_s).positions[18].y > 0.3);
    }
}

#[test]
fn changing_script_does_not_invent_a_gyro_peak() {
    let cfg=config::parse("schema_version: 1\nassignments: {T1: right_thigh}\nsources: [{id: sim, kind: simulated, trackers: [{id: T1, rate_hz: 100}]}]").unwrap();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let control = Control {
        motion: Motion::FrontKick,
        speed: 1.0,
        paused: false,
        generation: 0,
        suspended: false,
    };
    let mut scheduler = Scheduler::new(cfg.simulated[0].clone(), sk, 0, control);
    scheduler.step(500_000, &cfg.assignments, &BTreeMap::new(), |_, _| {});
    scheduler.on_control(
        510_000,
        Control {
            motion: Motion::ReferencePose,
            generation: 1,
            ..control
        },
    );
    let mut gyro = None;
    scheduler.step(510_000, &cfg.assignments, &BTreeMap::new(), |_, e| {
        if let SourceEvent::Observation {
            observation:
                SensorObservation {
                    data: ObservationData::AngularVelocity { rad_s, .. },
                    ..
                },
        } = e
        {
            gyro = Some(rad_s)
        }
    });
    assert_eq!(gyro, Some([0.0; 3]));
}
