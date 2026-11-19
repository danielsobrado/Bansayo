//! `ExerciseEngine` (06 §2): session lifecycle and readiness (03 §1), the item machine (06 §3),
//! commands (06 §7), events (06 §8), timers and limits (06 §9). A `technique` plan runs as a
//! one-block, one-item workout. Pure: time is only what the inputs carry.
use crate::evidence::reason;
use crate::observables::ObservableFrame;
use bansayo_contracts::*;
use serde_json::{Value, json};

pub mod timed;

/// One workout item: a technique practised for `sets` × `reps`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPlan {
    pub item_id: String,
    pub title: String,
    /// `id@version`.
    pub technique: ContentRef,
    pub family: String,
    pub side: PlanSide,
    pub sides_allowed: Vec<Side>,
    pub stage: PracticeStage,
    pub has_demo: bool,
    pub sets: u32,
    pub reps: u32,
    pub rest_s: u32,
    pub time_cap_s: Option<u32>,
    pub max_reps_per_set: u32,
    pub max_gap_us: u64,
    pub instructions: Vec<String>,
    /// The technique's start stance. Stance geometry needs world keypoints and no stance
    /// definition ships yet, so the gate is 500 ms of stillness and the stance is reported as
    /// not assessed (06 §3).
    pub start_stance: Option<String>,
    /// Required observables per active side `[left, right]`, as catalogue indices.
    pub required: [Vec<Option<usize>>; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPlan {
    pub title: String,
    pub level: Option<u32>,
    pub item: ItemPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionContext {
    pub session_id: SessionId,
    pub athlete_id: AthleteId,
    pub countdown_ms: u64,
    pub pause_after_tracking_loss_ms: u64,
    pub strikes_per_session: u32,
    pub session_minutes: u32,
    pub plan_problems: Vec<Reason>,
    pub safety_acked: bool,
    pub recording: RecordingFlags,
    pub capability_profile: CapabilityProfileId,
    pub live_observables: Vec<String>,
    pub metric_availability: Vec<MetricAvailability>,
    pub tracking_config_id: String,
    pub stance: Option<Stance>,
}

/// What recognition should look for now (06 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognitionContext {
    pub technique: ContentRef,
    pub side: Side,
    /// False while nothing is to be counted (not active, paused, resting, tracking lost).
    pub counting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub reason: Reason,
}

/// Session-clock delays the documents leave to the engine.
pub const BETWEEN_REPS_US: u64 = 1_000_000;
pub const ARMED_LOSS_US: u64 = 1_000_000;
pub const RESTORE_US: u64 = 500_000;
/// Stillness that replaces the start-stance gate (06 §3).
pub const STILL_US: u64 = 500_000;

#[derive(Debug, Clone)]
pub struct ExerciseEngine {
    plan: CompiledPlan,
    ctx: SessionContext,
    phase: SessionPhase,
    sub: Option<SubState>,
    phase_id: Option<String>,
    seq: u64,
    rev: u64,
    now: u64,
    started_t: Option<u64>,
    phase_t: u64,
    sub_t: u64,
    paused_from: Option<(SessionPhase, u64)>,
    set_index: u32,
    set_open: bool,
    sets_done: u32,
    sets_target: u32,
    done: u32,
    side: Side,
    totals: TrainingTotals,
    recorded: u32,
    last_rep: Option<RepetitionResult>,
    available: Option<Vec<bool>>,
    lost_since: Option<u64>,
    back_since: Option<u64>,
    missing_since: Option<u64>,
    still_since: Option<u64>,
    /// The current pause was caused by tracking loss and ends when tracking returns (08 §10).
    paused_by_loss: bool,
    space_confirmed: bool,
    strikes: u32,
    duration_notice: bool,
    active_from: Option<u64>,
}

fn rejected(code: &str, message: &str) -> Rejected {
    Rejected {
        reason: reason(code, None, message),
    }
}

fn side_index(s: Side) -> usize {
    usize::from(s == Side::Right)
}

fn other(s: Side) -> Side {
    if s == Side::Left {
        Side::Right
    } else {
        Side::Left
    }
}

impl ExerciseEngine {
    /// A new session in `preparing`, with its `session_started` event.
    pub fn new(plan: CompiledPlan, ctx: SessionContext, t_us: u64) -> (Self, Vec<TrainingEvent>) {
        let side = match plan.item.side {
            PlanSide::Left => Side::Left,
            PlanSide::Right | PlanSide::Alternate => Side::Right,
        };
        let mut e = Self {
            sets_target: plan.item.sets,
            plan,
            ctx,
            phase: SessionPhase::Preparing,
            sub: None,
            phase_id: None,
            seq: 0,
            rev: 1,
            now: t_us,
            started_t: None,
            phase_t: t_us,
            sub_t: t_us,
            paused_from: None,
            set_index: 0,
            set_open: false,
            sets_done: 0,
            done: 0,
            side,
            totals: TrainingTotals::default(),
            recorded: 0,
            last_rep: None,
            available: None,
            lost_since: None,
            back_since: None,
            missing_since: None,
            still_since: None,
            paused_by_loss: false,
            space_confirmed: false,
            strikes: 0,
            duration_notice: false,
            active_from: None,
        };
        let mut out = vec![e.event(
            "session_started",
            json!({
                "plan": { "kind": "technique", "title": e.plan.title, "technique": e.plan.item.technique,
                          "sets": e.plan.item.sets, "reps": e.target_reps(), "rest_s": e.plan.item.rest_s },
                "capability_profile": e.ctx.capability_profile,
                "tracking_config_id": e.ctx.tracking_config_id,
            }),
        )];
        if e.plan.item.reps > e.plan.item.max_reps_per_set {
            out.push(e.event(
                "safety_notice",
                json!({ "code": "reps_capped", "key": "safety.reps_capped",
                        "limit": e.plan.item.max_reps_per_set }),
            ));
        }
        out.extend(e.readiness_transition());
        (e, out)
    }

    fn target_reps(&self) -> u32 {
        self.plan.item.reps.min(self.plan.item.max_reps_per_set)
    }

    fn position(&self) -> PlanPosition {
        let started = self.set_index > 0;
        PlanPosition {
            block: started.then_some(0),
            item: started.then_some(0),
            set: started.then_some(self.set_index),
            round: None,
            step: None,
        }
    }

    fn event(&mut self, kind: &str, data: Value) -> TrainingEvent {
        self.seq += 1;
        self.rev += 1;
        TrainingEvent {
            event_seq: self.seq,
            t_us: self.now,
            event_type: kind.into(),
            position: self.position(),
            data,
        }
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn finished(&self) -> bool {
        matches!(self.phase, SessionPhase::Completed | SessionPhase::Aborted)
    }

    pub fn recognition_context(&self) -> RecognitionContext {
        RecognitionContext {
            technique: self.plan.item.technique.clone(),
            side: self.side,
            counting: self.phase == SessionPhase::Active
                && matches!(
                    self.sub,
                    Some(SubState::Armed | SubState::InRep | SubState::BetweenReps)
                ),
        }
    }

    pub fn set_metric_availability(
        &mut self,
        m: Vec<MetricAvailability>,
        profile: CapabilityProfileId,
    ) {
        if self.ctx.metric_availability != m || self.ctx.capability_profile != profile {
            self.ctx.metric_availability = m;
            self.ctx.capability_profile = profile;
            self.rev += 1;
        }
    }

    fn gates(&self) -> Vec<ReadinessGate> {
        let gate = |gate: &str, ok: bool, blocking: bool, reasons: Vec<Reason>| ReadinessGate {
            gate: gate.into(),
            state: if ok {
                GateState::Pass
            } else if blocking {
                GateState::Fail
            } else {
                GateState::Warn
            },
            blocking,
            reasons: if ok { vec![] } else { reasons },
        };
        let tracking = self
            .available
            .as_ref()
            .is_some_and(|a| a.iter().any(|x| *x));
        let degraded: Vec<Reason> = self
            .ctx
            .metric_availability
            .iter()
            .filter(|m| m.best_grade == Grade::X)
            .map(|m| {
                reason(
                    "metric_unavailable",
                    Some(&m.metric),
                    "This measurement will not be taken.",
                )
            })
            .collect();
        vec![
            gate("athlete_selected", true, true, vec![]),
            gate(
                "plan_valid",
                self.ctx.plan_problems.is_empty(),
                true,
                self.ctx.plan_problems.clone(),
            ),
            gate(
                "tracking_minimum",
                tracking,
                true,
                vec![reason(
                    "not_observed",
                    None,
                    "None of the movements this technique needs is observed yet.",
                )],
            ),
            gate("metrics_available", degraded.is_empty(), false, degraded),
            gate("calibration", true, false, vec![]),
            gate(
                "space_check",
                self.space_confirmed,
                false,
                vec![reason(
                    "space_not_confirmed",
                    None,
                    "Confirm a clear space around you before starting.",
                )],
            ),
            gate(
                "safety_ack",
                self.ctx.safety_acked,
                true,
                vec![reason(
                    "safety_not_acknowledged",
                    Some(&self.plan.item.technique),
                    "Read and acknowledge the safety notes.",
                )],
            ),
        ]
    }

    fn readiness_transition(&mut self) -> Vec<TrainingEvent> {
        if self.phase == SessionPhase::Preparing
            && self
                .gates()
                .iter()
                .all(|g| !g.blocking || g.state == GateState::Pass)
        {
            self.phase = SessionPhase::Ready;
            self.rev += 1;
        }
        vec![]
    }

    fn required(&self) -> &[Option<usize>] {
        &self.plan.item.required[side_index(self.side)]
    }

    /// Feeds one tick's observables.
    pub fn sample(&mut self, f: &ObservableFrame) -> Vec<TrainingEvent> {
        self.now = self.now.max(f.t_us);
        let a: Vec<bool> = self
            .required()
            .iter()
            .map(|i| i.is_some_and(|i| crate::observables::present(f.get(i))))
            .collect();
        if self.available.as_ref() != Some(&a) {
            self.available = Some(a);
            self.rev += 1;
        }
        let mut out = self.readiness_transition();
        out.extend(self.stance_gate());
        out.extend(self.tracking());
        out.extend(self.tick(f.t_us));
        out
    }

    fn tracking(&mut self) -> Vec<TrainingEvent> {
        let a = self.available.clone().unwrap_or_default();
        let all = !a.is_empty() && a.iter().all(|x| *x);
        // 08 §10: after an automatic pause the session resumes when the observables return.
        if self.phase == SessionPhase::Paused && self.paused_by_loss {
            if !all {
                self.back_since = None;
                return vec![];
            }
            let since = *self.back_since.get_or_insert(self.now);
            if self.now - since < RESTORE_US {
                return vec![];
            }
            self.back_since = None;
            return self.resume("tracking_restored");
        }
        if self.phase != SessionPhase::Active || !self.set_open {
            return vec![];
        }
        let none = !a.iter().any(|x| *x);
        let missing: Vec<String> = self
            .required()
            .iter()
            .zip(&a)
            .filter(|(_, ok)| !**ok)
            .filter_map(|(i, _)| i.map(|i| crate::observables::ids()[i].0.clone()))
            .collect();
        let now = self.now;
        match self.sub {
            Some(SubState::TrackingLost) => {
                if all {
                    let since = *self.back_since.get_or_insert(now);
                    if now - since >= RESTORE_US {
                        self.lost_since = None;
                        self.back_since = None;
                        self.set_sub(SubState::Armed);
                        return vec![self.event("tracking_restored", json!({}))];
                    }
                } else {
                    self.back_since = None;
                    if self
                        .lost_since
                        .is_some_and(|l| now - l >= self.ctx.pause_after_tracking_loss_ms * 1000)
                    {
                        self.lost_since = None;
                        return self.pause("tracking_lost");
                    }
                }
                vec![]
            }
            Some(SubState::Armed) => {
                if none {
                    let since = *self.missing_since.get_or_insert(now);
                    if now - since >= ARMED_LOSS_US {
                        return self.lose(missing);
                    }
                } else {
                    self.missing_since = None;
                }
                vec![]
            }
            Some(SubState::InRep) => {
                if !all {
                    let since = *self.missing_since.get_or_insert(now);
                    if now - since > self.plan.item.max_gap_us {
                        return self.lose(missing);
                    }
                } else {
                    self.missing_since = None;
                }
                vec![]
            }
            _ => vec![],
        }
    }

    fn lose(&mut self, missing: Vec<String>) -> Vec<TrainingEvent> {
        self.missing_since = None;
        self.lost_since = Some(self.now);
        self.set_sub(SubState::TrackingLost);
        self.phase_id = None;
        vec![self.event("tracking_lost", json!({ "missing": missing }))]
    }

    /// Whether the segmenter's driver is below its end threshold now (06 §3 stillness).
    pub fn set_still(&mut self, still: bool) {
        self.still_since = if still {
            Some(self.still_since.unwrap_or(self.now))
        } else {
            None
        };
    }

    /// The set's first armed state: the start-stance gate when the technique has a start stance.
    fn ready_sub(&self) -> SubState {
        if self.plan.item.start_stance.is_some() {
            SubState::WaitingStance
        } else {
            SubState::Armed
        }
    }

    /// `waiting_stance` → `armed` after 500 ms of stillness; the stance itself is not assessed.
    fn stance_gate(&mut self) -> Vec<TrainingEvent> {
        if self.phase != SessionPhase::Active || self.sub != Some(SubState::WaitingStance) {
            return vec![];
        }
        match self.still_since {
            Some(s) if self.now.saturating_sub(s) >= STILL_US => {
                self.set_sub(SubState::Armed);
                vec![self.event(
                    "stance_ready",
                    json!({ "stance": self.plan.item.start_stance, "assessed": false, "rule": "stillness" }),
                )]
            }
            _ => vec![],
        }
    }

    fn set_sub(&mut self, s: SubState) {
        if self.sub != Some(s) {
            self.sub = Some(s);
            self.sub_t = self.now;
            self.rev += 1;
        }
    }

    fn enter(&mut self, p: SessionPhase) {
        if p == SessionPhase::Active && self.phase != SessionPhase::Active {
            self.active_from = Some(self.now);
        }
        if self.phase == SessionPhase::Active
            && p != SessionPhase::Active
            && let Some(a) = self.active_from.take()
        {
            self.totals.active_ms += (self.now - a) / 1000;
        }
        self.phase = p;
        self.phase_t = self.now;
        self.rev += 1;
    }

    /// Advances timers to `t_us`.
    pub fn tick(&mut self, t_us: u64) -> Vec<TrainingEvent> {
        self.now = self.now.max(t_us);
        if let Some(s) = self.started_t {
            let e = (self.now - s) / 1000;
            if e / 1000 != self.totals.elapsed_ms / 1000 {
                self.rev += 1;
            }
            self.totals.elapsed_ms = e;
        }
        let mut out = vec![];
        match self.phase {
            SessionPhase::Countdown if self.now - self.phase_t >= self.ctx.countdown_ms * 1000 => {
                self.enter(SessionPhase::Active);
                out.push(self.event("item_started", json!({ "item_id": self.plan.item.item_id, "technique": self.plan.item.technique, "target": { "reps": self.target_reps(), "sets": self.sets_target } })));
                out.extend(self.start_set());
            }
            SessionPhase::Resting
                if self.now - self.phase_t >= u64::from(self.plan.item.rest_s) * 1_000_000 =>
            {
                out.push(self.event("rest_ended", json!({})));
                self.enter(SessionPhase::Active);
                out.extend(self.start_set());
            }
            SessionPhase::Active => {
                if self.sub == Some(SubState::BetweenReps)
                    && self.now - self.sub_t >= BETWEEN_REPS_US
                {
                    self.set_sub(SubState::Armed);
                }
                if let Some(cap) = self.plan.item.time_cap_s
                    && self.set_open
                    && self.now - self.phase_t >= u64::from(cap) * 1_000_000
                {
                    out.extend(self.finish_set("capped"));
                }
            }
            _ => {}
        }
        if !self.duration_notice
            && self.totals.elapsed_ms >= u64::from(self.ctx.session_minutes) * 60_000
        {
            self.duration_notice = true;
            out.push(self.event("safety_notice", json!({ "code": "session_duration", "key": "safety.session_duration", "minutes": self.ctx.session_minutes })));
        }
        out
    }

    fn start_set(&mut self) -> Vec<TrainingEvent> {
        if self.plan.item.side == PlanSide::Alternate && self.set_index > 0 {
            self.side = other(self.side);
        }
        self.set_index += 1;
        self.set_open = true;
        self.done = 0;
        self.phase_id = None;
        self.missing_since = None;
        self.lost_since = None;
        self.back_since = None;
        self.sub = Some(if self.plan.item.has_demo && self.set_index == 1 {
            SubState::Prompting
        } else {
            self.ready_sub()
        });
        self.sub_t = self.now;
        vec![self.event(
            "set_started",
            json!({ "set_index": self.set_index, "side": self.side, "target": { "reps": self.target_reps() },
                    "technique": self.plan.item.technique, "stage": self.plan.item.stage }),
        )]
    }

    fn finish_set(&mut self, status: &str) -> Vec<TrainingEvent> {
        self.set_open = false;
        self.phase_id = None;
        let counted = status == "completed" || status == "capped";
        if counted {
            self.sets_done += 1;
            self.totals.sets_completed += 1;
        }
        let mut out = vec![self.event(
            "set_completed",
            json!({ "set_index": self.set_index, "status": status, "reps": self.done, "side": self.side,
                    "target": { "reps": self.target_reps() } }),
        )];
        if status == "abandoned" {
            return out;
        }
        if self.sets_done >= self.sets_target || status == "skipped" {
            self.sub = None;
            out.push(self.event("item_completed", json!({ "item_id": self.plan.item.item_id, "status": if status == "skipped" { "skipped" } else { "completed" } })));
            out.push(self.event("block_completed", json!({ "block": 0 })));
            out.extend(self.close(SessionPhase::Completed, "plan_finished"));
        } else if self.plan.item.rest_s > 0 {
            self.sub = None;
            self.enter(SessionPhase::Resting);
            out.push(self.event(
                "rest_started",
                json!({ "duration_s": self.plan.item.rest_s }),
            ));
        } else {
            out.extend(self.start_set());
        }
        out
    }

    fn close(&mut self, p: SessionPhase, why: &str) -> Vec<TrainingEvent> {
        let p = if p == SessionPhase::Completed && self.recorded == 0 {
            SessionPhase::Aborted
        } else {
            p
        };
        self.enter(p);
        self.sub = None;
        self.phase_id = None;
        let kind = if p == SessionPhase::Completed {
            "session_completed"
        } else {
            "session_aborted"
        };
        let summary =
            json!({ "cause": why, "totals": self.totals, "sets_completed": self.sets_done });
        vec![self.event(kind, summary)]
    }

    fn resume(&mut self, cause: &str) -> Vec<TrainingEvent> {
        let (back, elapsed) = self.paused_from.take().unwrap_or((SessionPhase::Active, 0));
        self.paused_by_loss = false;
        self.enter(back);
        // Timers continue where they stopped.
        self.phase_t = self.now - elapsed.min(self.now);
        if self.sub == Some(SubState::TrackingLost) {
            self.lost_since = None;
            self.set_sub(SubState::Armed);
        }
        vec![self.event("session_resumed", json!({ "cause": cause }))]
    }

    fn pause(&mut self, cause: &str) -> Vec<TrainingEvent> {
        self.paused_by_loss = cause == "tracking_lost";
        self.paused_from = Some((self.phase, self.now - self.phase_t));
        self.enter(SessionPhase::Paused);
        vec![self.event("session_paused", json!({ "cause": cause }))]
    }

    /// A movement event from segmentation or recognition (counted only while armed or in a rep).
    pub fn movement(&mut self, e: &MovementEvent) -> Vec<TrainingEvent> {
        self.now = self.now.max(e.t_us);
        if !self.recognition_context().counting {
            return vec![];
        }
        match e.kind {
            MovementEventKind::ActivityStart => {
                self.set_sub(SubState::InRep);
                self.phase_id = None;
                vec![]
            }
            MovementEventKind::PhaseEnter => {
                self.set_sub(SubState::InRep);
                self.phase_id = e.subject.clone();
                self.rev += 1;
                let rep = self.totals.reps_completed + self.totals.reps_other + 1;
                vec![self.event(
                    "phase_changed",
                    json!({ "rep_index": rep, "phase_id": e.subject, "t_us": e.t_us }),
                )]
            }
            MovementEventKind::StrikePeak => {
                let rep = self.totals.reps_completed + self.totals.reps_other + 1;
                vec![self.event(
                    "strike",
                    json!({ "rep_index": rep, "t_us": e.t_us, "value": e.value }),
                )]
            }
            _ => vec![],
        }
    }

    /// The recognition rep-started instant (06 §8 `rep_started`).
    pub fn rep_started(&mut self, rep_index: u32, t_us: u64) -> Vec<TrainingEvent> {
        if !self.recognition_context().counting {
            return vec![];
        }
        self.now = self.now.max(t_us);
        self.set_sub(SubState::InRep);
        vec![self.event(
            "rep_started",
            json!({ "rep_index": rep_index, "side": self.side }),
        )]
    }

    /// Whether a repetition result would be counted now.
    pub fn accepts_results(&self) -> bool {
        self.phase == SessionPhase::Active && self.set_open
    }

    pub(crate) fn in_phase(&self, id: &str) -> bool {
        self.phase == SessionPhase::Active
            && self.sub == Some(SubState::InRep)
            && self.phase_id.as_deref() == Some(id)
    }

    pub(crate) fn target_reached(&mut self, t_us: TimeUs, id: &str, rep: u32) -> TrainingEvent {
        self.now = self.now.max(t_us);
        self.event("target_reached", json!({ "id": id, "rep_index": rep }))
    }

    /// A classified repetition (completed, incomplete, rejected or wrong side).
    pub fn repetition(&mut self, r: &RepetitionResult) -> Vec<TrainingEvent> {
        if !self.accepts_results() {
            return vec![];
        }
        self.now = self.now.max(r.repetition.t_end_us);
        self.recorded += 1;
        let counted = matches!(
            r.repetition.outcome,
            RepOutcome::Completed | RepOutcome::RejectedQuality
        );
        if r.repetition.outcome == RepOutcome::Completed {
            self.totals.reps_completed += 1;
        } else {
            self.totals.reps_other += 1;
        }
        if counted {
            self.done += 1;
        }
        self.last_rep = Some(r.clone());
        let mut out = vec![if r.repetition.outcome == RepOutcome::Completed {
            self.event(
                "rep_completed",
                serde_json::to_value(r).unwrap_or(Value::Null),
            )
        } else {
            self.event(
                "rep_rejected",
                json!({ "rep_index": r.repetition.rep_index, "outcome": r.repetition.outcome,
                        "reached_phase": r.repetition.reached_phase, "result": r }),
            )
        }];
        let strike_family = self.plan.item.family.starts_with("kick.")
            || self.plan.item.family.starts_with("knee.");
        if counted && strike_family {
            self.strikes += 1;
            let limit = self.ctx.strikes_per_session;
            if self.strikes >= limit && (self.strikes - limit).is_multiple_of(50) {
                out.push(self.event("safety_notice", json!({ "code": "strikes_per_session", "key": "safety.strike_limit", "count": self.strikes })));
            }
        }
        if self.sub != Some(SubState::TrackingLost) {
            self.set_sub(SubState::BetweenReps);
        }
        self.phase_id = None;
        if self.done >= self.target_reps() {
            let status = if self.plan.item.reps > self.plan.item.max_reps_per_set {
                "capped"
            } else {
                "completed"
            };
            out.extend(self.finish_set(status));
        }
        out
    }

    /// Activity that matched nothing expected (06 §8 `movement_unrecognised`).
    pub fn unrecognised(&mut self, t_us: u64, data: Value) -> Vec<TrainingEvent> {
        if !self.accepts_results() {
            return vec![];
        }
        self.now = self.now.max(t_us);
        self.totals.reps_other += 1;
        if self.sub == Some(SubState::InRep) {
            self.set_sub(SubState::Armed);
        }
        self.phase_id = None;
        vec![self.event("movement_unrecognised", data)]
    }

    /// Applies a client command in arrival order; invalid commands change nothing (06 §7).
    pub fn command(
        &mut self,
        t_us: u64,
        cmd: TrainingCommand,
        args: &TrainingCommandArgs,
    ) -> Result<Vec<TrainingEvent>, Rejected> {
        use SessionPhase as P;
        use TrainingCommand as C;
        let p = self.phase;
        let valid = match cmd {
            C::Start => p == P::Ready,
            C::Pause => matches!(p, P::Active | P::Resting),
            C::Resume => p == P::Paused,
            C::SkipRest => p == P::Resting,
            C::SkipItem | C::RepeatItem => matches!(p, P::Active | P::Resting),
            C::SkipDemo => p == P::Active && self.sub == Some(SubState::Prompting),
            C::SetSide => {
                p == P::Active
                    && matches!(self.sub, Some(SubState::Armed | SubState::BetweenReps))
                    && args
                        .side
                        .is_some_and(|s| self.plan.item.sides_allowed.contains(&s))
            }
            C::End => !self.finished(),
            C::AckSafety => {
                p == P::Preparing
                    && args
                        .content_refs
                        .as_ref()
                        .is_some_and(|r| r.contains(&self.plan.item.technique))
            }
        };
        if !valid {
            return Err(rejected(
                "conflict",
                "This command is not valid in the current state.",
            ));
        }
        self.now = self.now.max(t_us);
        let mut out = vec![self.event("command", json!({ "command": cmd, "args": args }))];
        match cmd {
            C::Start => {
                self.space_confirmed = true;
                self.started_t = Some(self.now);
                self.enter(P::Countdown);
            }
            C::Pause => out.extend(self.pause("command")),
            C::Resume => out.extend(self.resume("command")),
            C::SkipRest => {
                out.push(self.event("rest_ended", json!({ "skipped": true })));
                self.enter(P::Active);
                out.extend(self.start_set());
            }
            C::SkipItem => {
                if p == P::Resting {
                    self.enter(P::Active);
                    self.set_index += 1;
                }
                out.extend(self.finish_set("skipped"));
            }
            C::RepeatItem => {
                if p == P::Active {
                    out.extend(self.finish_set("abandoned"));
                } else {
                    self.sets_target += 1;
                    self.enter(P::Active);
                }
                out.extend(self.start_set());
            }
            C::SkipDemo => self.set_sub(self.ready_sub()),
            C::SetSide => {
                if let Some(s) = args.side {
                    self.side = s;
                    self.rev += 1;
                }
            }
            C::End => out.extend(self.close(P::Completed, "end")),
            C::AckSafety => {
                self.ctx.safety_acked = true;
                out.extend(self.readiness_transition());
            }
        }
        Ok(out)
    }

    fn timer(&self) -> Option<Timer> {
        let elapsed = (self.now - self.phase_t) / 1000;
        match self.phase {
            SessionPhase::Countdown => Some(Timer {
                kind: TimerKind::Countdown,
                started_t_us: self.phase_t,
                duration_ms: Some(self.ctx.countdown_ms),
                elapsed_ms: elapsed,
            }),
            SessionPhase::Resting => Some(Timer {
                kind: TimerKind::Rest,
                started_t_us: self.phase_t,
                duration_ms: Some(u64::from(self.plan.item.rest_s) * 1000),
                elapsed_ms: elapsed,
            }),
            SessionPhase::Active => Some(Timer {
                kind: TimerKind::Item,
                started_t_us: self.phase_t,
                duration_ms: self.plan.item.time_cap_s.map(|c| u64::from(c) * 1000),
                elapsed_ms: elapsed,
            }),
            _ => None,
        }
    }

    pub fn last_event_seq(&self) -> u64 {
        self.seq
    }

    pub fn state(&self) -> TrainingState {
        let started = self.set_index > 0;
        TrainingState {
            session_id: self.ctx.session_id.clone(),
            athlete_id: self.ctx.athlete_id.clone(),
            rev: self.rev,
            last_event_seq: self.seq,
            phase: self.phase,
            plan: PlanSummary {
                kind: PlanKind::Technique,
                plan_ref: None,
                title: self.plan.title.clone(),
                level: self.plan.level,
            },
            readiness: self.gates(),
            metric_availability: self.ctx.metric_availability.clone(),
            position: self.position(),
            current: (started
                || matches!(
                    self.phase,
                    SessionPhase::Countdown | SessionPhase::Ready | SessionPhase::Preparing
                ))
            .then(|| CurrentItem {
                item_id: self.plan.item.item_id.clone(),
                title: self.plan.item.title.clone(),
                exercise: ExerciseSummary {
                    kind: PlanKind::Technique,
                    exercise_ref: self.plan.item.technique.clone(),
                    side: Some(self.side),
                    stage: Some(self.plan.item.stage),
                    stance: self.ctx.stance,
                },
                sub_state: if self.phase == SessionPhase::Active {
                    self.sub
                } else {
                    None
                },
                phase_id: self.phase_id.clone(),
                counts: Counts {
                    done: self.done,
                    target: Some(self.target_reps()),
                    unit: CountUnit::Reps,
                },
                sets: SetCounts {
                    done: self.sets_done,
                    target: self.sets_target,
                },
                timer: self.timer(),
                beat: None,
                targets: vec![],
                expected_steps: vec![],
                reference_id: None,
                instructions: self
                    .plan
                    .item
                    .instructions
                    .iter()
                    .map(|k| TextRef {
                        key: k.clone(),
                        params: Default::default(),
                    })
                    .collect(),
            }),
            totals: self.totals.clone(),
            live_observables: self.ctx.live_observables.clone(),
            last_rep: self.last_rep.clone(),
            capability_profile: self.ctx.capability_profile,
            recording: self.ctx.recording,
            storage: StorageHealth::Ok,
        }
    }
}

#[cfg(test)]
mod tests;
