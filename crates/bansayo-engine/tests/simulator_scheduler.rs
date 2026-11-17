//! Simulator scheduler tests: deadline-based wake-ups, rate preservation, pose reuse, and recovery.
use bansayo_contracts::*;
use bansayo_engine::{
    clock::SessionClock,
    config::{self, CameraSpec, SimulatedSpec, TrackerSpec},
    skeleton::Skeleton,
    sources::{
        Ingest, Source, SourceContext,
        simulated::{
            Control, FaultSetting, Scheduler, SimulatedContext, SimulatedSource, motions::Motion,
        },
    },
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

fn skeleton() -> Skeleton {
    Skeleton::builtin("bansayo.body.v1", 1.75).unwrap()
}

fn tracker_spec(id: &str, rate_hz: f64) -> TrackerSpec {
    TrackerSpec {
        id: DeviceId::new(id).unwrap(),
        rate_hz,
        noise_deg: 0.0,
        heading_offset_deg: 0.0,
    }
}

fn camera_spec(id: &str, rate_hz: f64) -> CameraSpec {
    CameraSpec {
        id: DeviceId::new(id).unwrap(),
        rate_hz,
        resolution_px: [1280, 720],
        fov_y_deg: 50.0,
        position_m: [0.0, 1.2, 3.2],
        look_at_m: [0.0, 1.0, 0.0],
        noise_px: 0.0,
    }
}

fn spec(trackers: Vec<TrackerSpec>, cameras: Vec<CameraSpec>) -> SimulatedSpec {
    SimulatedSpec {
        landmark_schema: bansayo_engine::skeleton::proportions::LANDMARK_SCHEMA_ID.into(),
        id: SourceId::new("sim").unwrap(),
        seed: 42,
        motion: Motion::Squat,
        speed: 1.0,
        trackers,
        cameras,
        scenario_period_s: None,
        scenario: vec![],
    }
}

fn default_control(motion: Motion) -> Control {
    Control {
        motion,
        speed: 1.0,
        paused: false,
        generation: 0,
        suspended: false,
    }
}

struct Harness {
    rx: mpsc::Receiver<Ingest>,
    control: watch::Sender<Control>,
    faults: watch::Sender<BTreeMap<DeviceId, FaultSetting>>,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<Result<(), bansayo_engine::sources::SourceError>>,
}

fn spawn_source(spec: SimulatedSpec, assignments: BTreeMap<DeviceId, SegmentId>) -> Harness {
    let sk = skeleton();
    let control_init = default_control(spec.motion);
    let source = SimulatedSource {
        spec,
        skeleton: sk,
        assignments: watch::channel(assignments).1,
    };
    let (events, rx) = mpsc::channel(2048);
    let (control, control_rx) = watch::channel(control_init);
    let (faults, faults_rx) = watch::channel(BTreeMap::new());
    let cancel = CancellationToken::new();
    let dropped = source
        .descriptors()
        .into_iter()
        .map(|d| {
            (
                d.id,
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            )
        })
        .collect();
    let context = SimulatedContext {
        control: control_rx,
        faults: faults_rx,
        common: SourceContext {
            events,
            clock: SessionClock::start(),
            cancel: cancel.clone(),
            dropped,
        },
    };
    let task = tokio::spawn(source.run(context));
    Harness {
        rx,
        control,
        faults,
        cancel,
        task,
    }
}

fn drain(rx: &mut mpsc::Receiver<Ingest>) -> Vec<SourceEvent> {
    let mut out = vec![];
    while let Ok(item) = rx.try_recv() {
        if let Ingest::Event(ev) = item {
            // These cadence tests inspect the original primary streams. Gyro cadence has its own regression.
            if !matches!(
                &ev,
                SourceEvent::Observation {
                    observation: SensorObservation {
                        data: ObservationData::AngularVelocity { .. },
                        ..
                    }
                }
            ) {
                out.push(ev);
            }
        }
    }
    out
}

#[test]
fn idle_wakeups_skip_body_pose_and_shared_instants_reuse_pose() {
    let sk = skeleton();
    let cam_only = spec(vec![], vec![camera_spec("CAM1", 30.0)]);
    let mut sched = Scheduler::new(cam_only, sk.clone(), 0, default_control(Motion::Squat));
    let assignments = BTreeMap::new();
    let faults = BTreeMap::new();

    let mut total_evals = 0;
    let mut total_rebuilds = 0;
    let mut total_obs = 0;
    for ms in 0..=1000 {
        let m = sched.step(ms * 1_000, &assignments, &faults, |_, _| {});
        total_evals += m.body_pose_evals;
        total_rebuilds += m.fault_rebuilds;
        total_obs += m.emitted_observations;
    }
    // 30 Hz over 1s emits 31 samples (t=0 plus 30 intervals of 33_333 us).
    // All 970 idle wake-ups perform zero body_pose evaluations and zero fault rebuilds.
    assert_eq!(total_obs, 31);
    assert_eq!(total_evals, 31);
    assert_eq!(total_rebuilds, 1);
    assert_eq!(sched.next_deadline_us(), Some(31 * 33_333));

    // Mixed 12 trackers + 4 cameras all due at t=0 compute body_pose exactly once.
    let trackers = (1..=12)
        .map(|i| tracker_spec(&format!("T{i}"), 100.0))
        .collect();
    let cameras = (1..=4)
        .map(|i| camera_spec(&format!("CAM{i}"), 30.0))
        .collect();
    let mut mixed = Scheduler::new(
        spec(trackers, cameras),
        sk,
        0,
        default_control(Motion::Squat),
    );
    let m0 = mixed.step(0, &assignments, &faults, |_, _| {});
    assert_eq!(m0.emitted_observations, 28);
    assert_eq!(m0.body_pose_evals, 1);
    assert_eq!(mixed.next_deadline_us(), Some(10_000));

    // Idle call at 5_000 us triggers zero pose calculations.
    let m_idle = mixed.step(5_000, &assignments, &faults, |_, _| {});
    assert_eq!(m_idle.emitted_observations, 0);
    assert_eq!(m_idle.body_pose_evals, 0);
    assert_eq!(m_idle.fault_rebuilds, 0);

    // Suspended (playback) returns no deadline and evaluates nothing.
    mixed.on_control(
        5_000,
        Control {
            suspended: true,
            ..default_control(Motion::Squat)
        },
    );
    assert_eq!(mixed.next_deadline_us(), None);
    let m_susp = mixed.step(50_000, &assignments, &faults, |_, _| {});
    assert_eq!(m_susp.emitted_observations, 0);
    assert_eq!(m_susp.body_pose_evals, 0);
}

#[test]
fn delayed_wakeup_recovers_with_single_observation_and_no_burst() {
    let sk = skeleton();
    let mut sched = Scheduler::new(
        spec(vec![tracker_spec("T1", 100.0)], vec![]),
        sk,
        0,
        default_control(Motion::ReferencePose),
    );
    let assignments = BTreeMap::new();
    let faults = BTreeMap::new();

    assert_eq!(
        sched
            .step(0, &assignments, &faults, |_, _| {})
            .emitted_observations,
        2
    );
    assert_eq!(sched.next_deadline_us(), Some(10_000));
    assert_eq!(
        sched
            .step(10_000, &assignments, &faults, |_, _| {})
            .emitted_observations,
        2
    );
    assert_eq!(sched.next_deadline_us(), Some(20_000));

    // Simulate a 500 ms scheduling stall: emit exactly 1 observation, never 50 overdue ones.
    let mut seqs = vec![];
    let stalled = sched.step(520_000, &assignments, &faults, |_, ev| {
        if let SourceEvent::Observation { observation } = ev {
            if matches!(observation.data, ObservationData::Orientation { .. }) {
                seqs.push((observation.seq, observation.t_us));
            }
        }
    });
    assert_eq!(stalled.emitted_observations, 2);
    assert_eq!(seqs, vec![(3, 520_000)]);
    assert_eq!(sched.next_deadline_us(), Some(530_000));

    // Normal pacing resumes immediately.
    assert_eq!(
        sched
            .step(530_000, &assignments, &faults, |_, _| {})
            .emitted_observations,
        2
    );
    assert_eq!(sched.next_deadline_us(), Some(540_000));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_30hz_camera_streams_at_nominal_rate() {
    let mut h = spawn_source(
        spec(vec![], vec![camera_spec("CAM1", 30.0)]),
        BTreeMap::new(),
    );
    tokio::time::sleep(Duration::from_millis(400)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let seqs: Vec<_> = drain(&mut h.rx)
        .into_iter()
        .filter_map(|e| match e {
            SourceEvent::Observation { observation } => Some(observation.seq),
            _ => None,
        })
        .collect();
    // 400 ms at 30 Hz is ~12-13 samples; allow modest CI scheduling slack.
    assert!(seqs.len() >= 9 && seqs.len() <= 16, "count={}", seqs.len());
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_100hz_tracker_preserves_windows_sampling_rate() {
    let mut h = spawn_source(
        spec(vec![tracker_spec("T1", 100.0)], vec![]),
        BTreeMap::new(),
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let seqs: Vec<_> = drain(&mut h.rx)
        .into_iter()
        .filter_map(|e| match e {
            SourceEvent::Observation { observation } => Some(observation.seq),
            _ => None,
        })
        .collect();
    // A coarse 15.6 ms timer yields <= 23 samples in 350 ms; high-res yields ~35.
    assert!(seqs.len() >= 28, "100 Hz degraded: {}", seqs.len());
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_30hz_cameras_and_100hz_trackers_maintain_rates() {
    let mut h = spawn_source(
        spec(
            vec![tracker_spec("T1", 100.0), tracker_spec("T2", 100.0)],
            vec![camera_spec("CAM1", 30.0), camera_spec("CAM2", 30.0)],
        ),
        BTreeMap::new(),
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for ev in drain(&mut h.rx) {
        if let SourceEvent::Observation { observation } = ev {
            *counts.entry(observation.device_id.to_string()).or_default() += 1;
        }
    }
    assert!(counts["T1"] >= 28, "{counts:?}");
    assert!(counts["T2"] >= 28, "{counts:?}");
    assert!((8..=15).contains(&counts["CAM1"]), "{counts:?}");
    assert!((8..=15).contains(&counts["CAM2"]), "{counts:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_trackers_at_different_rates_emit_proportionally() {
    let mut h = spawn_source(
        spec(
            vec![
                tracker_spec("SLOW", 25.0),
                tracker_spec("MID", 50.0),
                tracker_spec("FAST", 100.0),
            ],
            vec![],
        ),
        BTreeMap::new(),
    );
    tokio::time::sleep(Duration::from_millis(360)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for ev in drain(&mut h.rx) {
        if let SourceEvent::Observation { observation } = ev {
            *counts.entry(observation.device_id.to_string()).or_default() += 1;
        }
    }
    assert!(counts["FAST"] > counts["MID"] + 8, "{counts:?}");
    assert!(counts["MID"] > counts["SLOW"] + 4, "{counts:?}");
    assert!(counts["FAST"] >= 28, "{counts:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pause_resume_and_motion_speed_changes_work() {
    let t1 = DeviceId::new("T1").unwrap();
    let thigh = SegmentId::new("right_thigh").unwrap();
    let mut h = spawn_source(
        spec(vec![tracker_spec("T1", 100.0)], vec![]),
        BTreeMap::from([(t1, thigh)]),
    );
    tokio::time::sleep(Duration::from_millis(80)).await;
    // Pause: pose must freeze while samples continue arriving.
    h.control.send_modify(|c| c.paused = true);
    tokio::time::sleep(Duration::from_millis(30)).await;
    let _ = drain(&mut h.rx);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let paused_quats: Vec<_> = drain(&mut h.rx)
        .into_iter()
        .filter_map(|e| match e {
            SourceEvent::Observation {
                observation:
                    SensorObservation {
                        data: ObservationData::Orientation { q, .. },
                        ..
                    },
            } => Some(q),
            _ => None,
        })
        .collect();
    assert!(paused_quats.len() >= 5, "{paused_quats:?}");
    assert!(paused_quats.windows(2).all(|w| w[0] == w[1]));

    // Resume at 4x speed with FrontKick (generation bump): orientations change over time.
    h.control.send_modify(|c| {
        c.paused = false;
        c.motion = Motion::FrontKick;
        c.speed = 4.0;
        c.generation += 1;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let _ = drain(&mut h.rx);
    tokio::time::sleep(Duration::from_millis(150)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let moving_quats: Vec<_> = drain(&mut h.rx)
        .into_iter()
        .filter_map(|e| match e {
            SourceEvent::Observation {
                observation:
                    SensorObservation {
                        data: ObservationData::Orientation { q, .. },
                        ..
                    },
            } => Some(q),
            _ => None,
        })
        .collect();
    assert!(moving_quats.len() >= 5);
    assert!(moving_quats.first() != moving_quats.last());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_after_simulated_disconnection_and_scripted_scenario() {
    let t1 = DeviceId::new("T1").unwrap();
    let mut h = spawn_source(
        spec(vec![tracker_spec("T1", 100.0)], vec![]),
        BTreeMap::new(),
    );
    tokio::time::sleep(Duration::from_millis(60)).await;
    h.faults.send_modify(|f| {
        f.insert(
            t1.clone(),
            FaultSetting {
                connected: false,
                quality: 1.0,
            },
        );
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    let before = drain(&mut h.rx);
    assert!(before.iter().any(|e| matches!(
        e,
        SourceEvent::DeviceLink {
            connected: false,
            ..
        }
    )));

    // While disconnected, no observations arrive.
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(drain(&mut h.rx).is_empty());

    // Reconnect: DeviceLink(true) and observations resume immediately.
    h.faults.send_modify(|f| {
        f.insert(
            t1.clone(),
            FaultSetting {
                connected: true,
                quality: 1.0,
            },
        );
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    h.cancel.cancel();
    h.task.await.unwrap().unwrap();

    let after = drain(&mut h.rx);
    assert!(after.iter().any(|e| matches!(
        e,
        SourceEvent::DeviceLink {
            connected: true,
            ..
        }
    )));
    let resumed_obs = after
        .iter()
        .filter(|e| matches!(e, SourceEvent::Observation { .. }))
        .count();
    assert!(resumed_obs >= 4, "{resumed_obs}");

    // Scripted scenario that disconnects at 0.0 s and reconnects at 0.1 s wakes on schedule.
    let scripted_spec = SimulatedSpec {
        landmark_schema: bansayo_engine::skeleton::proportions::LANDMARK_SCHEMA_ID.into(),
        scenario_period_s: None,
        scenario: vec![
            config::ScenarioStep {
                at_s: 0.0,
                device: "T1".into(),
                action: FaultAction::Disconnect,
                quality: None,
            },
            config::ScenarioStep {
                at_s: 0.1,
                device: "T1".into(),
                action: FaultAction::Reconnect,
                quality: None,
            },
        ],
        ..spec(vec![tracker_spec("T1", 100.0)], vec![])
    };
    let mut sh = spawn_source(scripted_spec, BTreeMap::new());
    tokio::time::sleep(Duration::from_millis(220)).await;
    sh.cancel.cancel();
    sh.task.await.unwrap().unwrap();
    let events = drain(&mut sh.rx);
    assert!(events.iter().any(|e| matches!(
        e,
        SourceEvent::DeviceLink {
            connected: true,
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SourceEvent::Observation { .. }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_while_sleeping_stops_promptly() {
    // 0.2 Hz tracker has a 5 second next deadline; cancellation must not wait for it.
    let h = spawn_source(spec(vec![tracker_spec("T1", 0.2)], vec![]), BTreeMap::new());
    tokio::time::sleep(Duration::from_millis(30)).await;
    let started = Instant::now();
    h.cancel.cancel();
    tokio::time::timeout(Duration::from_millis(250), h.task)
        .await
        .expect("source must stop promptly on cancel")
        .unwrap()
        .unwrap();
    assert!(started.elapsed() < Duration::from_millis(250));
}
