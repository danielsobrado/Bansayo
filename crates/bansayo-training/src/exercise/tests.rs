#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::observables::{ids, index};

fn plan(sets: u32, reps: u32, rest_s: u32, demo: bool) -> CompiledPlan {
    let req = |s: &str| {
        vec![
            index(&format!("ankle_height.{s}")),
            index(&format!("knee_height.{s}")),
        ]
    };
    CompiledPlan {
        title: "Roundhouse kick".into(),
        level: None,
        item: ItemPlan {
            item_id: "main".into(),
            title: "Roundhouse kick".into(),
            technique: "taekwondo.dollyeo_chagi@1".into(),
            family: "kick.roundhouse".into(),
            side: PlanSide::Right,
            sides_allowed: vec![Side::Left, Side::Right],
            stage: PracticeStage::Normal,
            has_demo: demo,
            sets,
            reps,
            rest_s,
            time_cap_s: None,
            max_reps_per_set: 20,
            max_gap_us: 80_000,
            instructions: vec!["instr.kick.clear_space".into()],
            start_stance: None,
            required: [req("left"), req("right")],
        },
    }
}

fn ctx(acked: bool) -> SessionContext {
    SessionContext {
        session_id: SessionId::new("20261117T120000Z-abcd").unwrap(),
        athlete_id: AthleteId::new("ana").unwrap(),
        countdown_ms: 5000,
        pause_after_tracking_loss_ms: 3000,
        strikes_per_session: 400,
        session_minutes: 90,
        plan_problems: vec![],
        safety_acked: acked,
        recording: RecordingFlags::default(),
        capability_profile: CapabilityProfileId::MultiView,
        live_observables: vec![],
        metric_availability: vec![],
        tracking_config_id: "t".into(),
        stance: None,
    }
}

fn frame(t: u64, seen: bool) -> ObservableFrame {
    ObservableFrame {
        t_us: t,
        values: ids()
            .iter()
            .map(|(id, _)| ObservableValue {
                id: id.clone(),
                value: seen.then_some(0.1),
                norm: seen.then_some(0.1),
                evidence: crate::evidence::unavailable(),
                fresh: true,
                reasons: vec![],
            })
            .collect(),
    }
}

fn result(rep: u32, outcome: RepOutcome, t: u64) -> RepetitionResult {
    RepetitionResult {
        repetition: Repetition {
            rep_index: rep,
            technique: "taekwondo.dollyeo_chagi@1".into(),
            side: Side::Right,
            stance: None,
            outcome,
            t_start_us: t.saturating_sub(1_000_000),
            t_end_us: t,
            phases: vec![],
            strike_t_us: None,
            recognition_confidence: 0.9,
            capability_profile: CapabilityProfileId::MultiView,
            reached_phase: None,
        },
        measurements: vec![],
        score: None,
        cue: None,
        targets: vec![],
        scored: outcome == RepOutcome::Completed,
        excluded: false,
    }
}

struct Run {
    e: ExerciseEngine,
    t: u64,
    events: Vec<TrainingEvent>,
}

impl Run {
    fn new(p: CompiledPlan, acked: bool) -> Self {
        let (e, events) = ExerciseEngine::new(p, ctx(acked), 0);
        Self { e, t: 0, events }
    }
    fn wait(&mut self, ms: u64, seen: bool) {
        for _ in 0..ms / 10 {
            self.t += 10_000;
            let ev = self.e.sample(&frame(self.t, seen));
            self.events.extend(ev);
        }
    }
    fn cmd(&mut self, c: TrainingCommand) -> bool {
        self.cmd_args(c, TrainingCommandArgs::default())
    }
    fn cmd_args(&mut self, c: TrainingCommand, a: TrainingCommandArgs) -> bool {
        match self.e.command(self.t, c, &a) {
            Ok(ev) => {
                self.events.extend(ev);
                true
            }
            Err(_) => false,
        }
    }
    fn rep(&mut self, outcome: RepOutcome) {
        let n = self.e.state().totals.reps_completed + self.e.state().totals.reps_other + 1;
        let ev = self.e.repetition(&result(n, outcome, self.t));
        self.events.extend(ev);
    }
    fn activity(&mut self) {
        let ev = self.e.movement(&MovementEvent {
            seq: 0,
            t_us: self.t,
            kind: MovementEventKind::ActivityStart,
            subject: None,
            value: None,
            evidence: crate::evidence::unavailable(),
        });
        self.events.extend(ev);
    }
    fn active(mut self) -> Self {
        self.wait(20, true);
        assert!(self.cmd(TrainingCommand::Start));
        self.wait(5000, true);
        assert_eq!(self.e.phase(), SessionPhase::Active);
        self
    }
    fn sub(&self) -> Option<SubState> {
        self.e.state().current.and_then(|c| c.sub_state)
    }
    fn kinds(&self) -> Vec<&str> {
        self.events.iter().map(|e| e.event_type.as_str()).collect()
    }
}

#[test]
fn session_machine_table() {
    use SessionPhase as P;
    use TrainingCommand as C;
    // (setup, command, accepted, resulting phase)
    type Setup = fn() -> Run;
    let preparing: Setup = || Run::new(plan(2, 2, 10, false), false);
    let ready: Setup = || {
        let mut r = Run::new(plan(2, 2, 10, false), true);
        r.wait(20, true);
        r
    };
    let active: Setup = || Run::new(plan(2, 2, 10, false), true).active();
    let resting: Setup = || {
        let mut r = Run::new(plan(2, 2, 10, false), true).active();
        r.rep(RepOutcome::Completed);
        r.rep(RepOutcome::Completed);
        r
    };
    let paused: Setup = || {
        let mut r = Run::new(plan(2, 2, 10, false), true).active();
        r.cmd(C::Pause);
        r
    };
    let cases: Vec<(&str, Setup, C, bool, P)> = vec![
        ("preparing start", preparing, C::Start, false, P::Preparing),
        ("preparing end", preparing, C::End, true, P::Aborted),
        ("ready start", ready, C::Start, true, P::Countdown),
        ("ready pause", ready, C::Pause, false, P::Ready),
        ("active pause", active, C::Pause, true, P::Paused),
        ("active resume", active, C::Resume, false, P::Active),
        ("active skip_rest", active, C::SkipRest, false, P::Active),
        ("active end without reps", active, C::End, true, P::Aborted),
        ("resting skip_rest", resting, C::SkipRest, true, P::Active),
        ("resting pause", resting, C::Pause, true, P::Paused),
        ("resting end", resting, C::End, true, P::Completed),
        ("paused resume", paused, C::Resume, true, P::Active),
        ("paused start", paused, C::Start, false, P::Paused),
    ];
    for (name, setup, c, ok, phase) in cases {
        let mut r = setup();
        let before = r.e.state();
        let accepted = r.cmd(c);
        assert_eq!(accepted, ok, "{name}");
        assert_eq!(r.e.phase(), phase, "{name}");
        if !ok {
            assert_eq!(
                r.e.state(),
                before,
                "{name}: a rejected command changes nothing"
            );
        }
    }
    // Resting resumes as resting with its timer continuing.
    let mut r = Run::new(plan(2, 2, 10, false), true).active();
    r.rep(RepOutcome::Completed);
    r.rep(RepOutcome::Completed);
    r.wait(4000, true);
    r.cmd(C::Pause);
    r.wait(30_000, true);
    r.cmd(C::Resume);
    assert_eq!(r.e.phase(), P::Resting);
    r.wait(6100, true);
    assert_eq!(
        r.e.phase(),
        P::Active,
        "10 s rest = 4 s before + 6 s after the pause"
    );
}

#[test]
fn readiness_gates_block_until_safety_and_tracking() {
    let mut r = Run::new(plan(1, 1, 0, false), false);
    r.wait(50, false);
    let gate = |r: &Run, g: &str| {
        r.e.state()
            .readiness
            .into_iter()
            .find(|x| x.gate == g)
            .unwrap()
            .state
    };
    assert_eq!(gate(&r, "safety_ack"), GateState::Fail);
    assert_eq!(gate(&r, "tracking_minimum"), GateState::Fail);
    assert_eq!(gate(&r, "space_check"), GateState::Warn);
    assert!(!r.cmd_args(
        TrainingCommand::AckSafety,
        TrainingCommandArgs {
            side: None,
            content_refs: Some(vec!["other@1".into()])
        }
    ));
    assert!(r.cmd_args(
        TrainingCommand::AckSafety,
        TrainingCommandArgs {
            side: None,
            content_refs: Some(vec!["taekwondo.dollyeo_chagi@1".into()])
        }
    ));
    assert_eq!(
        r.e.phase(),
        SessionPhase::Preparing,
        "tracking still missing"
    );
    r.wait(20, true);
    assert_eq!(r.e.phase(), SessionPhase::Ready);
}

#[test]
fn item_machine_table() {
    let mut r = Run::new(plan(2, 2, 3, true), true).active();
    assert_eq!(r.sub(), Some(SubState::Prompting));
    assert!(r.cmd(TrainingCommand::SkipDemo));
    assert_eq!(r.sub(), Some(SubState::Armed));
    r.activity();
    assert_eq!(r.sub(), Some(SubState::InRep));
    r.rep(RepOutcome::Completed);
    assert_eq!(r.sub(), Some(SubState::BetweenReps));
    r.wait(1100, true);
    assert_eq!(r.sub(), Some(SubState::Armed));
    // Armed with nothing observed for 1 s → tracking_lost; back for 0.5 s → armed.
    r.wait(1100, false);
    assert_eq!(r.sub(), Some(SubState::TrackingLost));
    r.wait(600, true);
    assert_eq!(r.sub(), Some(SubState::Armed));
    // In a repetition a gap beyond max_gap_ms → tracking_lost; 3 s lost → paused.
    r.activity();
    r.wait(100, false);
    assert_eq!(r.sub(), Some(SubState::TrackingLost));
    r.wait(3100, false);
    assert_eq!(r.e.phase(), SessionPhase::Paused);
    assert!(r.cmd(TrainingCommand::Resume));
    r.wait(600, true);
    assert_eq!(r.sub(), Some(SubState::Armed));
    // Incomplete and wrong-side results are recorded but not counted.
    r.rep(RepOutcome::Incomplete);
    r.rep(RepOutcome::WrongSide);
    assert_eq!(r.e.state().current.unwrap().counts.done, 1);
    r.rep(RepOutcome::RejectedQuality);
    assert_eq!(
        r.e.phase(),
        SessionPhase::Resting,
        "counted rejected_quality reaches the target"
    );
    r.wait(3100, true);
    assert_eq!(r.e.phase(), SessionPhase::Active);
    r.rep(RepOutcome::Completed);
    r.rep(RepOutcome::Completed);
    assert_eq!(r.e.phase(), SessionPhase::Completed);
    let k = r.kinds();
    for kind in [
        "session_started",
        "item_started",
        "set_started",
        "rep_completed",
        "rep_rejected",
        "tracking_lost",
        "tracking_restored",
        "session_paused",
        "session_resumed",
        "set_completed",
        "rest_started",
        "rest_ended",
        "item_completed",
        "session_completed",
    ] {
        assert!(k.contains(&kind), "missing {kind}");
    }
    for (i, e) in r.events.iter().enumerate() {
        assert_eq!(
            e.event_seq,
            i as u64 + 1,
            "event_seq increases by exactly one"
        );
    }
    assert_eq!(r.e.state().last_event_seq, r.events.len() as u64);
}

#[test]
fn repeat_and_skip_item_and_set_side() {
    let mut r = Run::new(plan(2, 3, 5, false), true).active();
    r.rep(RepOutcome::Completed);
    assert!(r.cmd(TrainingCommand::RepeatItem));
    let abandoned = r
        .events
        .iter()
        .rfind(|e| e.event_type == "set_completed")
        .unwrap();
    assert_eq!(abandoned.data["status"], "abandoned");
    assert_eq!(r.e.state().current.unwrap().counts.done, 0);
    assert_eq!(r.e.state().position.set, Some(2));
    assert!(!r.cmd_args(
        TrainingCommand::SetSide,
        TrainingCommandArgs {
            side: None,
            content_refs: None
        }
    ));
    assert!(r.cmd_args(
        TrainingCommand::SetSide,
        TrainingCommandArgs {
            side: Some(Side::Left),
            content_refs: None
        }
    ));
    assert_eq!(r.e.recognition_context().side, Side::Left);
    assert!(r.cmd(TrainingCommand::SkipItem));
    assert_eq!(r.e.phase(), SessionPhase::Completed, "one rep was recorded");
}

#[test]
fn strike_limit_notices() {
    let mut p = plan(1, 20, 0, false);
    p.item.max_reps_per_set = 30;
    let (mut e, _) = ExerciseEngine::new(
        p,
        SessionContext {
            strikes_per_session: 3,
            ..ctx(true)
        },
        0,
    );
    e.sample(&frame(10_000, true));
    e.command(
        10_000,
        TrainingCommand::Start,
        &TrainingCommandArgs::default(),
    )
    .unwrap();
    e.tick(6_000_000);
    let mut notices = 0;
    for k in 1..=6 {
        notices += e
            .repetition(&result(k, RepOutcome::Completed, 7_000_000 + u64::from(k)))
            .iter()
            .filter(|x| x.event_type == "safety_notice")
            .count();
    }
    assert_eq!(notices, 1, "at the limit, then every 50");
}

fn lcg(s: &mut u64) -> u64 {
    *s = s
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *s >> 33
}

#[test]
fn property_any_command_sequence_leaves_a_defined_state() {
    use TrainingCommand as C;
    let all = [
        C::Start,
        C::Pause,
        C::Resume,
        C::SkipRest,
        C::SkipItem,
        C::RepeatItem,
        C::SkipDemo,
        C::SetSide,
        C::End,
        C::AckSafety,
    ];
    for seed in 0..300u64 {
        let mut s = seed;
        let mut r = Run::new(
            plan(
                1 + (seed % 3) as u32,
                1 + (seed % 4) as u32,
                (seed % 2) as u32 * 2,
                seed % 2 == 0,
            ),
            seed % 3 != 0,
        );
        let mut finished = false;
        for _ in 0..60 {
            match lcg(&mut s) % 5 {
                0 => r.wait(10 * (lcg(&mut s) % 400), !lcg(&mut s).is_multiple_of(4)),
                1 => r.rep(
                    [
                        RepOutcome::Completed,
                        RepOutcome::Incomplete,
                        RepOutcome::RejectedQuality,
                        RepOutcome::WrongSide,
                    ][(lcg(&mut s) % 4) as usize],
                ),
                2 => r.activity(),
                _ => {
                    let c = all[(lcg(&mut s) % all.len() as u64) as usize];
                    let args = TrainingCommandArgs {
                        side: Some(if lcg(&mut s).is_multiple_of(2) {
                            Side::Left
                        } else {
                            Side::Right
                        }),
                        content_refs: Some(vec!["taekwondo.dollyeo_chagi@1".into()]),
                    };
                    r.cmd_args(c, args);
                }
            }
            let st = r.e.state();
            assert_eq!(st.last_event_seq, r.events.len() as u64, "seed {seed}");
            if let Some(c) = &st.current {
                assert!(c.counts.done <= c.counts.target.unwrap(), "seed {seed}");
                assert!(c.sets.done <= c.sets.target, "seed {seed}");
                if st.phase != SessionPhase::Active {
                    assert!(c.sub_state.is_none(), "seed {seed}");
                }
            }
            if finished {
                assert!(r.e.finished(), "seed {seed}: finished stays finished");
            }
            finished = r.e.finished();
            if st.phase == SessionPhase::Completed {
                assert!(
                    r.events.iter().any(|e| e.event_type.starts_with("rep_")),
                    "seed {seed}: completed needs a recorded repetition"
                );
            }
        }
    }
}

/// 06 §3: with a start stance that cannot be assessed, a set waits for 500 ms of stillness.
#[test]
fn start_stance_gate_falls_back_to_stillness() {
    let mut p = plan(1, 2, 0, false);
    p.item.start_stance = Some("taekwondo.gyeorugi_junbi".into());
    let mut r = Run::new(p, true).active();
    assert_eq!(r.sub(), Some(SubState::WaitingStance));
    // Moving: no arming, and movement is not counted.
    r.e.set_still(false);
    r.wait(800, true);
    r.activity();
    assert_eq!(r.sub(), Some(SubState::WaitingStance));
    r.e.set_still(true);
    r.wait(300, true);
    assert_eq!(r.sub(), Some(SubState::WaitingStance));
    r.wait(300, true);
    assert_eq!(r.sub(), Some(SubState::Armed));
    let ready = r
        .events
        .iter()
        .find(|e| e.event_type == "stance_ready")
        .unwrap();
    assert_eq!(ready.data["assessed"], false);
    assert_eq!(ready.data["rule"], "stillness");
}

/// 08 §10: tracking lost for 3 s pauses; observables back for 500 ms resume without a command.
#[test]
fn a_tracking_loss_pause_resumes_when_tracking_returns() {
    let mut r = Run::new(plan(1, 3, 0, false), true).active();
    assert_eq!(r.sub(), Some(SubState::Armed));
    r.wait(1100, false);
    assert_eq!(r.sub(), Some(SubState::TrackingLost));
    r.wait(3000, false);
    assert_eq!(r.e.phase(), SessionPhase::Paused);
    r.wait(300, true);
    assert_eq!(r.e.phase(), SessionPhase::Paused);
    r.wait(300, true);
    assert_eq!(r.e.phase(), SessionPhase::Active);
    assert_eq!(r.sub(), Some(SubState::Armed));
    let resumed = r
        .events
        .iter()
        .rfind(|e| e.event_type == "session_resumed")
        .unwrap();
    assert_eq!(resumed.data["cause"], "tracking_restored");
    // A pause by command does not end by itself.
    assert!(r.cmd(TrainingCommand::Pause));
    r.wait(1000, true);
    assert_eq!(r.e.phase(), SessionPhase::Paused);
    assert!(r.cmd(TrainingCommand::Resume));
    assert_eq!(r.e.phase(), SessionPhase::Active);
}
