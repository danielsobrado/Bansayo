//! Frozen pre-Phase-0 simulator inputs and frame joint bits, captured at b61c6b7.
use bansayo_contracts::*;
use bansayo_engine::recording::{reader, replay::Replay};
use std::path::PathBuf;
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
