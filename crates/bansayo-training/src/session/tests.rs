#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::metrics::Qa;
use crate::testkit::{library, roundhouse_sample, skeleton};

fn setup(plan: TechniquePlan) -> SessionSetup {
    SessionSetup {
        session_id: SessionId::new("20261117T120000Z-abcd").unwrap(),
        athlete_id: AthleteId::new("ana").unwrap(),
        stance: Some(Stance::Orthodox),
        coaching: CoachingLevel::Standard,
        tolerance_scale: 1.0,
        safety_acked: vec!["taekwondo.dollyeo_chagi@1".into()],
        skeleton: skeleton(1.75),
        library: library(),
        plan,
        grade: GradeContext {
            calibration: [
                (CalibrationKind::CameraIntrinsics, CalibrationState::Valid),
                (CalibrationKind::CameraExtrinsics, CalibrationState::Valid),
                (CalibrationKind::FloorPlane, CalibrationState::Valid),
            ]
            .into(),
            qa: Qa::NotRun,
            origin_live: false,
            detector_id: "simulated-projection".into(),
            proportions_valid: false,
            max_gap_us: 80_000,
        },
        personal: Default::default(),
        recording: RecordingFlags::default(),
        capability_profile: CapabilityProfileId::MultiView,
        tracking_config_id: "t".into(),
    }
}

pub fn plan(sets: u32, reps: u32) -> TechniquePlan {
    TechniquePlan {
        technique: "taekwondo.dollyeo_chagi".into(),
        side: PlanSide::Right,
        stages: None,
        reference: None,
        coaching: None,
        scoring: None,
        sets: Some(sets),
        reps: Some(reps),
        rest_s: Some(10),
    }
}

/// Drives a session at `rate` Hz: standing except while active, where kicks run after 1 s.
pub fn drive(sets: u32, reps: u32, rate: f64) -> (Session, Vec<TrainingEvent>) {
    let (s, e, _) = drive_with_origins(sets, reps, rate);
    (s, e)
}

/// As [`drive`], also returning the session time of script time zero of each set.
pub fn drive_with_origins(
    sets: u32,
    reps: u32,
    rate: f64,
) -> (Session, Vec<TrainingEvent>, Vec<u64>) {
    drive_script(sets, reps, rate, None)
}

const TEST_TARGET: Vec3 = [0.40, 0.50, 0.0];

fn drive_script(
    sets: u32,
    reps: u32,
    rate: f64,
    observed_ankle: Option<(Vec3, Provenance)>,
) -> (Session, Vec<TrainingEvent>, Vec<u64>) {
    let mut origins = vec![];
    let sk = skeleton(1.75);
    let (mut s, mut events) =
        Session::new(SessionConfig::default(), setup(plan(sets, reps)), 0).unwrap();
    let dt = 1e6 / rate;
    let mut kick_t: Option<f64> = None;
    let mut started = false;
    for k in 1..(400.0 * rate) as u64 {
        let t = (k as f64 * dt) as u64;
        let active = s.engine.phase() == SessionPhase::Active;
        kick_t = if active {
            Some(kick_t.map_or(-1.0, |x| x + 1.0 / rate))
        } else {
            None
        };
        if let Some(x) = kick_t
            && x.abs() < 0.5 / rate
        {
            origins.push(t);
        }
        let script = match kick_t {
            Some(x) if x >= 0.0 => x,
            _ => 2.6,
        };
        let mut sample = roundhouse_sample(&sk, script);
        sample.t_us = t;
        if let Some(b) = sample.body.as_mut() {
            b.root.evidence.t_obs_us = Some(t);
            if let Some((position_m, provenance)) = observed_ankle {
                b.observed_ankles = Some([
                    None,
                    Some(KeypointState {
                        position_m,
                        evidence: Evidence {
                            provenance,
                            ..crate::testkit::evidence(t)
                        },
                    }),
                ]);
            }
            for p in b.keypoints.iter_mut().flatten() {
                p.evidence.t_obs_us = Some(t);
            }
            for g in &mut b.segments {
                g.axis.t_obs_us = Some(t);
                g.twist.t_obs_us = Some(t);
            }
        }
        let out = s.sample(&sample);
        events.extend(out.events);
        events.extend(s.tick(t));
        if !started && s.engine.phase() == SessionPhase::Ready {
            events.extend(
                s.command(
                    t,
                    TrainingCommand::Start,
                    &TrainingCommandArgs {
                        target_position_m: observed_ankle.map(|_| TEST_TARGET),
                        ..Default::default()
                    },
                )
                .unwrap(),
            );
            started = true;
        }
        if s.finished() {
            break;
        }
    }
    (s, events, origins)
}

#[test]
fn target_feedback_uses_observed_ankles_once_per_kick() {
    for (point, provenance, reached) in [
        (TEST_TARGET, Provenance::Measured, Some(true)),
        ([1.40, 0.50, 0.0], Provenance::Measured, Some(false)),
        (TEST_TARGET, Provenance::Inferred, None),
    ] {
        let (_, events, _) = drive_script(1, 2, 60.0, Some((point, provenance)));
        let reps: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "rep_completed")
            .collect();
        assert_eq!(reps.len(), 2);
        assert!(
            reps.iter()
                .all(|e| e.data["targets"][0]["reached"].as_bool() == reached)
        );
        let hits: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "target_reached")
            .collect();
        assert_eq!(hits.len(), if reached == Some(true) { 2 } else { 0 });
        for hit in hits {
            let before = events
                .iter()
                .take_while(|e| e.event_seq < hit.event_seq)
                .filter(|e| e.event_type == "phase_changed")
                .last()
                .unwrap();
            assert_eq!(before.data["phase_id"], "extension");
        }
    }
}

/// Phase starts of one scripted kick, seconds from the cycle start: the same definition run on a
/// noise-free 1 kHz stream with the filter effectively off.
fn truth() -> Vec<(String, f64)> {
    let sk = skeleton(1.75);
    let lib = library();
    let t = Arc::new(
        crate::content::compile::compile_technique(
            &lib.techniques["taekwondo.dollyeo_chagi"],
            Side::Right,
            Some(Stance::Orthodox),
            &lib,
        )
        .unwrap(),
    );
    let mut o = Observer::new(
        &sk,
        Cutoffs {
            position_hz: 400.0,
            angle_hz: 400.0,
        },
    )
    .unwrap();
    let mut r = Recognizer::new(
        RecognizerConfig::default(),
        vec![(Role::Expected, t.clone())],
    );
    for k in 0..5600u64 {
        let script = if k < 1000 {
            2.6
        } else {
            (k - 1000) as f64 / 1000.0
        };
        let mut sm = roundhouse_sample(&sk, script);
        sm.t_us = k * 1000;
        if let Some(b) = sm.body.as_mut() {
            for p in b.keypoints.iter_mut().flatten() {
                p.evidence.t_obs_us = Some(sm.t_us);
            }
            for g in &mut b.segments {
                g.axis.t_obs_us = Some(sm.t_us);
            }
        }
        for out in r.step(&o.step(&sm)) {
            if let RecognitionOutput::Result(f) = out {
                return f
                    .phases
                    .iter()
                    .map(|(p, at)| (t.phases[*p].id.clone(), (*at as f64 - 1e6) / 1e6))
                    .collect();
            }
        }
    }
    panic!("no truth repetition")
}

#[test]
fn phase_boundaries_within_two_sample_periods_and_metric_values_within_tolerance() {
    let truth = truth();
    let rate = 60.0;
    let (_, events, origins) = drive_with_origins(1, 5, rate);
    let reps: Vec<RepetitionResult> = events
        .iter()
        .filter(|e| e.event_type == "rep_completed")
        .map(|e| serde_json::from_value(e.data.clone()).unwrap())
        .collect();
    assert_eq!(reps.len(), 5);
    let origin = origins[0] as f64 / 1e6;
    let period = 2.0 / rate;
    for (n, r) in reps.iter().enumerate() {
        let cycle = origin + 2.8 * n as f64;
        for (p, (id, at)) in r.repetition.phases.iter().zip(&truth) {
            assert_eq!(&p.phase, id);
            let measured = p.t_start_us as f64 / 1e6 - cycle;
            assert!(
                (measured - at).abs() <= period,
                "rep {n} {id}: {measured:.4} vs {at:.4}"
            );
        }
        let get = |id: &str| {
            r.measurements
                .iter()
                .find(|m| m.metric.starts_with(id))
                .unwrap()
        };
        // Scripted values: durations from the truth boundaries; heights from the scripted pose.
        let truth_at = |id: &str| truth.iter().find(|x| x.0 == id).unwrap().1;
        let retraction = (truth_at("recovery") - truth_at("retraction")) * 1000.0;
        let measured_retraction_end = get("kick.retraction_time").value.unwrap()
            + (r.repetition.strike_t_us.unwrap() as f64 / 1e6 - cycle - truth_at("retraction"))
                * 1000.0;
        assert!(
            (measured_retraction_end - retraction).abs() <= period * 1000.0 * 2.0,
            "retraction {measured_retraction_end} vs {retraction}"
        );
        let knee = get("kick.peak_knee_height").norm.unwrap();
        let ankle = get("kick.peak_ankle_height").norm.unwrap();
        let (knee_truth, ankle_truth) = scripted_peaks();
        assert!(
            (knee - knee_truth.0).abs() <= 0.02,
            "knee {knee} vs {knee_truth:?}"
        );
        assert!(
            (ankle - ankle_truth).abs() <= 0.02,
            "ankle {ankle} vs {ankle_truth}"
        );
    }
}

/// Peak knee height in the chamber window (bounded by truth) and peak ankle height, in leg lengths.
fn scripted_peaks() -> ((f64, f64), f64) {
    let sk = skeleton(1.75);
    let leg = 0.491 * 1.75;
    let truth = truth();
    let (c0, c1) = (truth[0].1, truth[1].1);
    let y = |t: f64, k: usize| {
        f64::from(
            roundhouse_sample(&sk, t).body.unwrap().keypoints[k]
                .as_ref()
                .unwrap()
                .position_m[1],
        ) / leg
    };
    let knee = (0..=1000)
        .map(|i| c0 + (c1 - c0) * f64::from(i) / 1000.0)
        .map(|t| y(t, 17))
        .fold(f64::MIN, f64::max);
    let ankle = (0..2800)
        .map(|i| f64::from(i) / 1000.0)
        .map(|t| y(t, 18))
        .fold(f64::MIN, f64::max);
    ((knee, c1 - c0), ankle)
}

#[test]
fn three_by_ten_roundhouse_session_counts_exactly() {
    let (s, events) = drive(3, 10, 60.0);
    assert_eq!(s.engine.phase(), SessionPhase::Completed);
    let reps: Vec<RepetitionResult> = events
        .iter()
        .filter(|e| e.event_type == "rep_completed")
        .map(|e| serde_json::from_value(e.data.clone()).unwrap())
        .collect();
    let other: Vec<&TrainingEvent> = events
        .iter()
        .filter(|e| e.event_type == "rep_rejected")
        .collect();
    assert_eq!(
        reps.len(),
        30,
        "rejected: {:?}",
        other.iter().map(|e| &e.data["outcome"]).collect::<Vec<_>>()
    );
    assert_eq!(s.state().totals.sets_completed, 3);
    for r in &reps {
        let ids: Vec<&str> = r
            .repetition
            .phases
            .iter()
            .map(|p| p.phase.as_str())
            .collect();
        assert_eq!(
            ids,
            ["chamber", "pivot", "extension", "retraction", "recovery"]
        );
        assert!(r.repetition.strike_t_us.is_some());
        let get = |id: &str| {
            r.measurements
                .iter()
                .find(|m| m.metric.starts_with(id))
                .unwrap()
        };
        for id in [
            "kick.total_time",
            "kick.extension_time",
            "kick.retraction_time",
            "kick.peak_knee_height",
            "kick.peak_ankle_height",
            "kick.peak_ankle_speed",
            "kick.support_knee_range",
        ] {
            assert_eq!(get(id).grade, Grade::B, "{id}: {:?}", get(id).reasons);
        }
        assert_eq!(get("kick.recovery_settle_time").grade, Grade::X);
        assert!(
            r.cue.is_none()
                || r.cue
                    .as_ref()
                    .is_some_and(|c| c.channel == CueChannel::AfterRep)
        );
    }
    let seqs: Vec<u64> = events.iter().map(|e| e.event_seq).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1));
    let sets: Vec<&TrainingEvent> = events
        .iter()
        .filter(|e| e.event_type == "set_completed")
        .collect();
    assert_eq!(sets.len(), 3);
    assert!(
        sets.iter()
            .all(|e| e.data["cues"].as_array().unwrap().len() <= 3)
    );
}

#[test]
fn identical_inputs_give_identical_event_logs() {
    let (_, a) = drive(1, 3, 60.0);
    let (_, b) = drive(1, 3, 60.0);
    assert_eq!(a, b);
}
