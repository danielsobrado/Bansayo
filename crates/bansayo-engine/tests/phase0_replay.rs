//! Frozen pre-Phase-0 simulator inputs and frame joint bits, captured at b61c6b7.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config,
    math::rot_from_wire,
    recording::{Context, Item, Recorder, reader, replay::Replay},
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{Control, Scheduler, SimulatedSource, motions::Motion},
};
use std::{collections::BTreeMap, path::PathBuf};
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/phase0")
}
fn frames(recording: reader::Recording) -> Vec<ServerMessage> {
    let mut replay = Replay::new(recording, 1.0, false).unwrap();
    let mut frames = vec![];
    while !replay.paused {
        frames.extend(
            replay
                .advance(10_000)
                .unwrap()
                .into_iter()
                .filter(|m| matches!(m, ServerMessage::Frame { .. })),
        );
    }
    frames
}
#[tokio::test]
#[ignore = "one-time capture, run only against b61c6b7 before production edits"]
async fn capture_before_reference() {
    let cfg = config::parse(include_str!("../../../config/sim-full-body.yaml")).unwrap();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let mut spec = cfg.simulated[0].clone();
    spec.cameras.clear();
    spec.scenario.clear();
    spec.scenario_period_s = None;
    let source = SimulatedSource {
        spec: spec.clone(),
        skeleton: sk.clone(),
        assignments: tokio::sync::watch::channel(cfg.assignments.clone()).1,
    };
    let mut reg = Registry::new(
        source.descriptors(),
        cfg.assignments.clone(),
        BTreeMap::new(),
    );
    let mut control = Control {
        motion: Motion::ReferencePose,
        speed: 1.0,
        paused: false,
        generation: 0,
        suspended: false,
    };
    let mut scheduler = Scheduler::new(spec, sk.clone(), 0, control);
    let mut store = Store::default();
    scheduler.step(0, &cfg.assignments, &BTreeMap::new(), |_, e| {
        if let SourceEvent::Observation { observation: o } = &e {
            if let ObservationData::Orientation { q, .. } = o.data {
                let segment = cfg.assignments[&o.device_id].clone();
                let mount = bansayo_engine::math::rot_to_wire(&rot_from_wire(q).unwrap().inverse());
                let p = CalibrationProfile {
                    schema_version: 1,
                    id: ProfileId::new(format!("mounting-{}-{segment}", o.device_id)).unwrap(),
                    kind: CalibrationKind::Mounting,
                    method: "reference_pose".into(),
                    origin: Origin::Simulated,
                    created_utc: "2026-11-17T00:00:00Z".into(),
                    subject: CalibrationSubject {
                        device_id: Some(o.device_id.clone()),
                        segment_id: Some(segment),
                    },
                    depends_on: vec![],
                    data: serde_json::json!({"q_sensor_from_segment":mount})
                        .as_object()
                        .unwrap()
                        .clone(),
                    quality: Default::default(),
                };
                store.insert(vec![p]);
            }
        }
        reg.event(e);
    });
    let tmp = tempfile::tempdir().unwrap();
    let recorder = Recorder::start(
        tmp.path().into(),
        Some("Phase 0 before reference b61c6b7".into()),
        Context::snapshot(&reg, &store, &sk, &cfg.config, 0),
    )
    .unwrap();
    for tick in 1..=750 {
        let now = tick * 10_000;
        let motion = if now <= 4_000_000 {
            Motion::Squat
        } else if now <= 6_500_000 {
            Motion::FrontKick
        } else {
            Motion::Standing
        };
        if motion != control.motion {
            control.motion = motion;
            control.generation += 1;
            scheduler.on_control(now, control);
        }
        scheduler.step(now, &cfg.assignments, &BTreeMap::new(), |_, event| {
            recorder
                .tx
                .send(Item::Event {
                    accepted_us: now,
                    event,
                })
                .unwrap()
        });
    }
    let session = recorder.finish(7_500_000).await.unwrap();
    let recording = reader::load(tmp.path(), &session.id).unwrap();
    let output = frames(recording);
    let dir = fixtures();
    std::fs::create_dir_all(dir.join("sessions").join(session.id.as_str())).unwrap();
    for f in ["recording.mcap", "session.json"] {
        std::fs::copy(
            tmp.path()
                .join("sessions")
                .join(session.id.as_str())
                .join(f),
            dir.join("sessions").join(session.id.as_str()).join(f),
        )
        .unwrap();
    }
    std::fs::write(dir.join("session-id.txt"), session.id.as_str()).unwrap();
    std::fs::write(
        dir.join("before-frames.json"),
        serde_json::to_vec(&output).unwrap(),
    )
    .unwrap();
}
#[test]
fn recorded_simulator_joint_values_keep_their_bits() {
    let dir = fixtures();
    let id = SessionId::new(std::fs::read_to_string(dir.join("session-id.txt")).unwrap()).unwrap();
    let before: Vec<ServerMessage> =
        serde_json::from_slice(&std::fs::read(dir.join("before-frames.json")).unwrap()).unwrap();
    let after = frames(reader::load(&dir, &id).unwrap());
    assert_eq!(before.len(), after.len());
    for (old, new) in before.iter().zip(&after) {
        if let (
            ServerMessage::Frame {
                t_us: a,
                joints: old,
                ..
            },
            ServerMessage::Frame {
                t_us: b,
                joints: new,
                ..
            },
        ) = (old, new)
        {
            assert_eq!(a, b);
            for (i, (a, b)) in old.iter().zip(new).enumerate() {
                if i == 8 {
                    continue;
                } // O-01: trunk_forward_lean is the sole permitted change.
                assert_eq!(
                    a.value.map(f32::to_bits),
                    b.value.map(f32::to_bits),
                    "tick {a:?} joint {i}"
                );
            }
        }
    }
}
