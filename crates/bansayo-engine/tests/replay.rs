//! Replay of synthetic recordings: every recorded camera observation replays exactly once and in
//! order, whatever the speed, step size, seeks, loops or solver tick backlog.
use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    config,
    recording::{
        Context,
        reader::{Recorded, RecordedData, Recording},
        replay::{MAX_TICKS_PER_ADVANCE, Replay},
    },
    registry::Registry,
    skeleton::Skeleton,
    sources::simulated::{SimulatedSource, body::body_pose, camera, motions::Motion},
};
use std::collections::BTreeMap;

const STARTED: u64 = 1_000_000;
const RATE_HZ: f64 = 60.0;
const STALE_US: u64 = 250_000;
/// Capture-to-acceptance latency of every synthetic camera frame.
const LATENCY_US: u64 = 1_000;
const RING: [[f64; 3]; 4] = [
    [0.0, 1.2, 3.2],
    [3.0, 1.6, 0.4],
    [-0.6, 0.9, -3.1],
    [-3.1, 2.0, 0.8],
];

/// (device, frame seq, capture time) of a camera observation.
type Id = (DeviceId, u64, u64);

/// Offset of solver tick `n` from the session start.
fn tick(n: u64) -> u64 {
    (n as f64 * 1e6 / RATE_HZ).round() as u64
}
/// Offsets of every solver tick within `duration`.
fn ticks(duration: u64) -> Vec<u64> {
    (0..).map(tick).take_while(|&t| t <= duration).collect()
}
/// Every camera at 30 Hz from `from` to `until`, each with its own phase, ordered by time.
fn stream(cameras: usize, from: u64, until: u64) -> Vec<(usize, u64)> {
    let mut at: Vec<_> = (0..cameras)
        .flat_map(|cam| {
            (0..)
                .map(move |k| from + cam as u64 * 4_000 + k * 33_333)
                .take_while(move |&t| t <= until)
                .map(move |t| (cam, t))
        })
        .collect();
    at.sort_by_key(|a| a.1);
    at
}
/// A recording of `cameras` simulated cameras, one observation per `(camera, accepted offset)`.
fn recording(cameras: usize, duration_us: u64, at: &[(usize, u64)]) -> Recording {
    let yaml = format!(
        "schema_version: 1\nengine: {{publish_rate_hz: {RATE_HZ}, stale_after_ms: {}}}\nsources:\n  - id: sim\n    kind: simulated\n    cameras:\n{}",
        STALE_US / 1000,
        RING[..cameras]
            .iter()
            .enumerate()
            .map(|(i, p)| format!(
                "      - {{id: CAM{}, rate_hz: 30, resolution_px: [1280, 720], fov_y_deg: 50, position_m: {p:?}, look_at_m: [0.0, 1.0, 0.0]}}\n",
                i + 1
            ))
            .collect::<String>()
    );
    let config = config::parse(&yaml).unwrap();
    assert_eq!(config.config.engine.publish_rate_hz, RATE_HZ);
    let skeleton = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let spec = config.simulated[0].clone();
    let source = SimulatedSource {
        spec: spec.clone(),
        skeleton: skeleton.clone(),
        assignments: tokio::sync::watch::channel(BTreeMap::new()).1,
    };
    let registry = Registry::new(source.descriptors(), BTreeMap::new(), BTreeMap::new());
    let mut store = Store::default();
    store.insert(
        spec.cameras
            .iter()
            .flat_map(|c| camera::profiles(c, "2026-01-01T00:00:00Z"))
            .collect(),
    );
    let initial = Context::snapshot(&registry, &store, &skeleton, &config.config, STARTED);
    let truth = body_pose(&skeleton, Motion::ReferencePose, 0.0).positions;
    let mut at = at.to_vec();
    at.sort_by_key(|a| a.1);
    let mut seqs = vec![0; cameras];
    let events = at
        .iter()
        .map(|&(cam, offset)| {
            seqs[cam] += 1;
            let accepted_us = STARTED + offset;
            let t_us = accepted_us - LATENCY_US;
            let landmarks = camera::project(&spec.cameras[cam], spec.seed, seqs[cam], t_us, &truth);
            let observation = SensorObservation {
                device_id: spec.cameras[cam].id.clone(),
                source_id: SourceId::new("sim").unwrap(),
                origin: Origin::Simulated,
                seq: seqs[cam],
                t_us,
                device_time_us: Some(t_us),
                time_quality: TimeQuality::Synchronized,
                calibration_ref: None,
                confidence: Some(1.0),
                data: ObservationData::Landmarks2d { landmarks },
            };
            Recorded {
                accepted_us,
                data: RecordedData::Event(Box::new(SourceEvent::Observation { observation })),
            }
        })
        .collect();
    Recording {
        session: RecordingSession {
            id: SessionId::new("20261116T000000Z-0000").unwrap(),
            name: "synthetic".into(),
            created_utc: "2026-11-16T00:00:00Z".into(),
            duration_us,
            size_bytes: 0,
            origins: vec![Origin::Simulated],
            device_count: cameras as u32,
            engine_version: initial.engine_version.clone(),
            protocol_version: PROTOCOL_VERSION,
        },
        initial,
        events,
    }
}
fn id(landmarks: &PoseLandmarks2D) -> Id {
    let meta = &landmarks.frame_meta;
    (meta.device_id.clone(), meta.frame_seq, meta.t_us)
}
/// (accepted offset, identity) of every recorded camera observation, in recorded order.
fn observed(recording: &Recording) -> Vec<(u64, Id)> {
    recording
        .events
        .iter()
        .filter_map(|e| match &e.data {
            RecordedData::Event(event) => match event.as_ref() {
                SourceEvent::Observation {
                    observation:
                        SensorObservation {
                            data: ObservationData::Landmarks2d { landmarks },
                            ..
                        },
                } => Some((e.accepted_us - STARTED, id(landmarks))),
                _ => None,
            },
            _ => None,
        })
        .collect()
}
/// Identities of the observations accepted after `after` (if any) and at or before `until`.
fn within(observed: &[(u64, Id)], after: Option<u64>, until: u64) -> Vec<Id> {
    observed
        .iter()
        .filter(|(at, _)| after.is_none_or(|a| *at > a) && *at <= until)
        .map(|(_, id)| id.clone())
        .collect()
}
fn all(recording: &Recording) -> Vec<Id> {
    within(&observed(recording), None, recording.session.duration_us)
}
fn ids(messages: &[ServerMessage]) -> Vec<Id> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Landmarks2d { landmarks } => Some(id(landmarks)),
            _ => None,
        })
        .collect()
}
/// Offsets of every emitted frame.
fn frames(messages: &[ServerMessage]) -> Vec<u64> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Frame { t_us, .. } => Some(t_us - STARTED),
            _ => None,
        })
        .collect()
}
/// Advances by `step` until the replay pauses at the end.
fn run(replay: &mut Replay, step: u64) -> Vec<ServerMessage> {
    let mut messages = vec![];
    for _ in 0..100_000 {
        if replay.paused {
            return messages;
        }
        messages.extend(replay.advance(step).unwrap());
    }
    panic!("the replay never reached its end");
}
fn replay(recording: &Recording, step: u64) -> Vec<ServerMessage> {
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    let messages = run(&mut replay, step);
    assert_eq!(replay.position_us, recording.session.duration_us);
    messages
}
/// Each frame is preceded by exactly the observations accepted since the previous tick, and the
/// observations after the last tick follow the last frame.
fn assert_grouped(messages: &[ServerMessage], recording: &Recording) {
    let observed = observed(recording);
    let duration = recording.session.duration_us;
    let mut expected = vec![];
    let mut previous = None;
    for t in ticks(duration) {
        expected.push((within(&observed, previous, t), Some(t)));
        previous = Some(t);
    }
    expected.push((within(&observed, previous, duration), None));
    let mut actual = vec![(vec![], None)];
    for message in messages {
        match message {
            ServerMessage::Landmarks2d { landmarks } => {
                actual.last_mut().unwrap().0.push(id(landmarks))
            }
            ServerMessage::Frame { t_us, .. } => {
                actual.last_mut().unwrap().1 = Some(t_us - STARTED);
                actual.push((vec![], None));
            }
            _ => {}
        }
    }
    assert_eq!(actual, expected);
}
/// Each camera's latest observation accepted by `position`, unless stale there; sorted.
fn current(recording: &Recording, position: u64) -> Vec<Id> {
    let mut latest = BTreeMap::new();
    for (at, id) in observed(recording) {
        if at <= position {
            latest.insert(id.0.clone(), id);
        }
    }
    latest
        .into_values()
        .filter(|id| STARTED + position - id.2 <= STALE_US)
        .collect()
}
fn sorted(mut ids: Vec<Id>) -> Vec<Id> {
    ids.sort();
    ids
}
fn cam(i: usize) -> DeviceId {
    DeviceId::new(format!("CAM{i}")).unwrap()
}

#[test]
fn one_observation_after_the_last_tick_replays() {
    let mut at = stream(3, 5_000, 100_000);
    at.push((1, 105_000));
    let recording = recording(3, 110_000, &at);
    assert_eq!(ticks(110_000).last(), Some(&100_000));
    let messages = replay(&recording, 10_000);
    assert_eq!(ids(&messages), all(&recording));
    assert_eq!(ids(&messages).last(), Some(&(cam(2), 4, STARTED + 104_000)));
    assert_grouped(&messages, &recording);
}

#[test]
fn several_trailing_observations_from_different_cameras_replay_in_order() {
    let mut at = stream(3, 5_000, 100_000);
    at.extend([(0, 101_000), (2, 104_500), (1, 108_000), (0, 112_000)]);
    let recording = recording(3, 115_000, &at);
    assert_eq!(ticks(115_000).last(), Some(&100_000));
    let messages = replay(&recording, 7_000);
    let replayed = ids(&messages);
    assert_eq!(replayed, all(&recording));
    let trailing: Vec<_> = replayed[replayed.len() - 4..]
        .iter()
        .map(|id| (id.0.clone(), id.2 - STARTED))
        .collect();
    assert_eq!(
        trailing,
        [
            (cam(1), 100_000),
            (cam(3), 103_500),
            (cam(2), 107_000),
            (cam(1), 111_000)
        ]
    );
    assert_grouped(&messages, &recording);
}

#[test]
fn an_observation_at_the_session_end_replays() {
    // The end falls between two ticks, then exactly on a tick.
    for duration in [110_000, 100_000] {
        let mut at = stream(3, 5_000, 90_000);
        at.push((2, duration));
        let recording = recording(3, duration, &at);
        for step in [1_000, 10_000, 1_000_000] {
            let messages = replay(&recording, step);
            assert_eq!(ids(&messages), all(&recording), "{duration} {step}");
            assert_eq!(
                ids(&messages).last().map(|id| (id.0.clone(), id.2)),
                Some((cam(3), STARTED + duration - LATENCY_US))
            );
            assert_grouped(&messages, &recording);
        }
    }
}

#[test]
fn a_session_ending_between_ticks_invents_no_tick() {
    let recording = recording(3, 110_000, &stream(3, 5_000, 110_000));
    for step in [1_000, 10_000, 37_123, 1_000_000] {
        let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
        let messages = run(&mut replay, step);
        assert_eq!(frames(&messages), ticks(110_000), "{step}");
        assert_eq!(frames(&messages).last(), Some(&100_000));
        assert!(frames(&messages).iter().all(|&t| t <= 110_000));
        assert_eq!(replay.position_us, 110_000);
        assert!(replay.paused);
        assert_eq!(replay.state().position_us, 110_000);
        assert!(replay.state().paused);
        assert_eq!(ids(&messages), all(&recording));
    }
}

#[test]
fn seeking_back_and_forward_shows_current_landmarks_and_replays_the_rest_once() {
    // CAM4 stops early, so it is stale at later positions.
    let mut at = stream(3, 5_000, 1_000_000);
    at.extend([(3, 20_000), (3, 53_333), (3, 86_666)]);
    let recording = recording(4, 1_000_000, &at);
    let observed = observed(&recording);
    let check_preview = |replay: &Replay, position: u64| {
        let preview = replay.preview();
        let expected = current(&recording, position);
        assert_eq!(sorted(ids(&preview)), expected, "{position}");
        assert_eq!(frames(&preview), [position]);
        expected
    };

    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    run(&mut replay, 10_000);
    // Backward, between observations.
    replay.seek(400_000).unwrap();
    let shown = check_preview(&replay, 400_000);
    assert_eq!(shown.len(), 3, "{shown:?}");
    assert!(shown.iter().all(|id| id.0 != cam(4)));
    replay.paused = false;
    let rest = run(&mut replay, 10_000);
    assert_eq!(ids(&rest), within(&observed, Some(400_000), 1_000_000));
    assert_eq!(replay.position_us, 1_000_000);

    // Backward, exactly onto an observation: it is shown, and not replayed again.
    let (exact, exact_id) = observed[40].clone();
    replay.seek(exact).unwrap();
    assert!(check_preview(&replay, exact).contains(&exact_id));
    replay.paused = false;
    let rest = run(&mut replay, 10_000);
    assert_eq!(ids(&rest), within(&observed, Some(exact), 1_000_000));
    assert!(!ids(&rest).contains(&exact_id));

    // Backward to where CAM4 is still current.
    replay.seek(60_000).unwrap();
    let shown = check_preview(&replay, 60_000);
    assert!(shown.contains(&(cam(4), 2, STARTED + 53_333 - LATENCY_US)));
    replay.paused = false;
    let rest = run(&mut replay, 10_000);
    assert_eq!(ids(&rest), within(&observed, Some(60_000), 1_000_000));

    // Forward from part-way through.
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    let start = replay.advance(200_000).unwrap();
    assert_eq!(ids(&start), within(&observed, None, 200_000));
    replay.seek(700_000).unwrap();
    let shown = check_preview(&replay, 700_000);
    assert_eq!(shown.len(), 3, "{shown:?}");
    let rest = run(&mut replay, 10_000);
    assert_eq!(ids(&rest), within(&observed, Some(700_000), 1_000_000));
}

#[test]
fn a_paused_seek_previews_without_advancing() {
    let recording = recording(3, 1_000_000, &stream(3, 5_000, 1_000_000));
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    replay.paused = true;
    replay.seek(500_000).unwrap();
    let preview = replay.preview();
    let expected = current(&recording, 500_000);
    assert_eq!(expected.len(), 3);
    assert_eq!(sorted(ids(&preview)), expected);
    assert_eq!(frames(&preview), [500_000]);
    for _ in 0..3 {
        assert_eq!(replay.advance(1_000_000).unwrap(), vec![]);
    }
    assert!(replay.paused);
    assert_eq!(replay.position_us, 500_000);
    assert_eq!(replay.preview(), preview);
}

#[test]
fn every_loop_replays_the_full_recording_exactly_once() {
    let mut at = stream(3, 5_000, 290_000);
    at.push((2, 300_000));
    let recording = recording(3, 300_000, &at);
    let expected = all(&recording);
    for step in [10_000, 37_123] {
        let mut replay = Replay::new(recording.clone(), 1.0, true).unwrap();
        let mut loops: Vec<Vec<ServerMessage>> = vec![vec![]];
        for _ in 0..10_000 {
            if loops.len() > 4 {
                break;
            }
            let messages = replay.advance(step).unwrap();
            assert!(!replay.paused);
            loops.last_mut().unwrap().extend(messages);
            if replay.position_us == 0 {
                loops.push(vec![]);
            }
        }
        loops.pop();
        assert_eq!(loops.len(), 4);
        for (i, messages) in loops.iter().enumerate() {
            assert_eq!(ids(messages), expected, "loop {i} step {step}");
            assert_eq!(frames(messages), ticks(300_000), "loop {i} step {step}");
            assert_eq!(*messages, loops[0], "loop {i} step {step}");
        }
    }
}

#[test]
fn replay_speed_does_not_change_the_messages() {
    let recording = recording(3, 500_000, &stream(3, 5_000, 500_000));
    let messages: Vec<_> = [0.1, 1.0, 4.0]
        .map(|speed| {
            let mut replay = Replay::new(recording.clone(), speed, false).unwrap();
            let messages = run(&mut replay, 10_000);
            assert_eq!(replay.position_us, 500_000);
            messages
        })
        .into();
    assert_eq!(ids(&messages[1]), all(&recording));
    assert_eq!(frames(&messages[1]), ticks(500_000));
    assert_eq!(messages[0], messages[1]);
    assert_eq!(messages[2], messages[1]);
}

#[test]
fn a_tick_backlog_is_bounded_and_drains_to_the_same_messages() {
    let recording = recording(2, 6_000_000, &stream(2, 5_000, 6_000_000));
    let observed = observed(&recording);
    assert!(ticks(6_000_000).len() > MAX_TICKS_PER_ADVANCE);
    let mut replay = Replay::new(recording.clone(), 1.0, false).unwrap();
    let mut messages = replay.advance(10_000_000).unwrap();
    let last = tick(MAX_TICKS_PER_ADVANCE as u64 - 1);
    assert_eq!(
        frames(&messages),
        (0..MAX_TICKS_PER_ADVANCE as u64)
            .map(tick)
            .collect::<Vec<_>>()
    );
    assert_eq!(replay.position_us, last);
    assert!(!replay.paused);
    assert_eq!(ids(&messages), within(&observed, None, last));
    for _ in 0..100 {
        if replay.paused {
            break;
        }
        messages.extend(replay.advance(0).unwrap());
    }
    assert!(replay.paused);
    assert_eq!(replay.position_us, 6_000_000);
    assert_eq!(ids(&messages), all(&recording));
    assert_eq!(messages, self::replay(&recording, 10_000));
}

#[test]
fn advance_step_sizes_do_not_change_the_messages() {
    let mut at = stream(3, 5_000, 1_000_000);
    at.push((0, 1_000_000));
    let recording = recording(3, 1_000_000, &at);
    let reference = replay(&recording, 1_000);
    assert_eq!(ids(&reference), all(&recording));
    assert_eq!(frames(&reference), ticks(1_000_000));
    for step in [10_000, 37_123, 1_000_000] {
        assert_eq!(replay(&recording, step), reference, "{step}");
    }
}

#[test]
fn rejects_invalid_seek_and_speed() {
    let recording = recording(1, 100_000, &[]);
    assert!(Replay::new(recording.clone(), f32::NAN, false).is_err());
    assert!(Replay::new(recording.clone(), 0.0, false).is_err());
    let mut replay = Replay::new(recording, 1.0, false).unwrap();
    assert!(replay.seek(100_001).is_err());
    assert_eq!(replay.position_us, 0);
    replay.seek(16_667).unwrap();
    let messages = replay.advance(0).unwrap();
    assert_eq!(frames(&messages), [16_667]);
}
