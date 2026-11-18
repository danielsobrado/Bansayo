//! One training session as a pure state machine: MotionSample → observables → segmentation and
//! recognition → re-filtered metrics → scoring → coaching → exercise engine (14 §3). The engine's
//! training runner hosts it; replaying the same inputs reproduces the same events.
use crate::coaching::Scheduler;
use crate::content::compile::{CompiledTechnique, Obs};
use crate::content::plan::{PlanTechniques, compile_workout, wrap};
use crate::content::types::MetricKind;
use crate::content::validate::Library;
use crate::evidence::{combine, reason, unavailable};
use crate::exercise::{ExerciseEngine, Rejected, SessionContext};
use crate::metrics::{GradeContext, RepWindow, measure};
use crate::observables::{Cutoffs, ObservableFrame, Observer, ids};
use crate::recognize::{
    Finalized, Outcome, RecognitionEvent, RecognitionOutput, Recognizer, RecognizerConfig, Role,
    rerun,
};
use crate::scoring::{PersonalTargets, score};
use bansayo_contracts::*;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Arc;

/// Engine configuration values the session uses (13 §4).
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub cutoffs: Cutoffs,
    pub recognizer: RecognizerConfig,
    pub countdown_ms: u64,
    pub pause_after_tracking_loss_ms: u64,
    pub strikes_per_session: u32,
    pub session_minutes: u32,
    pub cooldown_reps: u32,
    /// `analysis.planar_camera`; `None` is `auto` (04 §6.4).
    pub planar_camera: Option<DeviceId>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cutoffs: Cutoffs::default(),
            recognizer: RecognizerConfig::default(),
            countdown_ms: 5000,
            pause_after_tracking_loss_ms: 3000,
            strikes_per_session: 400,
            session_minutes: 90,
            cooldown_reps: 3,
            planar_camera: None,
        }
    }
}

/// Everything fixed at session creation.
#[derive(Debug, Clone)]
pub struct SessionSetup {
    pub session_id: SessionId,
    pub athlete_id: AthleteId,
    pub stance: Option<Stance>,
    pub coaching: CoachingLevel,
    pub tolerance_scale: f64,
    pub safety_acked: Vec<ContentRef>,
    pub skeleton: SkeletonDefinition,
    pub library: Library,
    pub plan: TechniquePlan,
    pub grade: GradeContext,
    pub personal: PersonalTargets,
    pub recording: RecordingFlags,
    pub capability_profile: CapabilityProfileId,
    pub tracking_config_id: String,
}

/// What one input produced.
#[derive(Debug, Clone, Default)]
pub struct Output {
    pub events: Vec<TrainingEvent>,
    /// The observables of this tick (for the `observables` topic and `motion.mcap`).
    pub frame: Option<ObservableFrame>,
}

pub struct Session {
    cfg: SessionConfig,
    setup: SessionSetup,
    observer: Observer,
    recognizer: Recognizer,
    recognizer_side: Side,
    techniques: PlanTechniques,
    engine: ExerciseEngine,
    scheduler: Scheduler,
    next_rep: u32,
    movement_seq: u64,
    set_results: Vec<RepetitionResult>,
    fresh_times: VecDeque<u64>,
    live_ids: Vec<String>,
    basic_since: Option<u64>,
}

/// How long profile `basic` must hold before image-plane observables are served.
pub const BASIC_HOLD_US: u64 = 1_000_000;

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

impl Session {
    pub fn new(
        cfg: SessionConfig,
        setup: SessionSetup,
        t_us: u64,
    ) -> Result<(Self, Vec<TrainingEvent>), Vec<ContentProblem>> {
        let display = setup
            .library
            .techniques
            .get(&setup.plan.technique)
            .map_or(setup.plan.technique.clone(), |t| t.names.display.clone());
        let workout = wrap(&setup.plan, &display);
        let (plan, techniques) = compile_workout(&workout, &setup.library, setup.stance)?;
        let observer =
            Observer::with_planar_camera(&setup.skeleton, cfg.cutoffs, cfg.planar_camera.clone())
                .map_err(|e| {
                vec![crate::content::validate::problem(
                    "skeleton",
                    "invalid_skeleton",
                    e,
                )]
            })?;
        let ctx = SessionContext {
            session_id: setup.session_id.clone(),
            athlete_id: setup.athlete_id.clone(),
            countdown_ms: cfg.countdown_ms,
            pause_after_tracking_loss_ms: cfg.pause_after_tracking_loss_ms,
            strikes_per_session: cfg.strikes_per_session,
            session_minutes: cfg.session_minutes,
            plan_problems: vec![],
            safety_acked: setup.safety_acked.contains(&plan.item.technique),
            recording: setup.recording,
            capability_profile: setup.capability_profile,
            live_observables: vec![],
            metric_availability: vec![],
            tracking_config_id: setup.tracking_config_id.clone(),
            stance: setup.stance,
        };
        let slow = plan.item.stage == PracticeStage::Slow;
        let level = crate::content::plan::coaching(&setup.plan, setup.coaching);
        let (engine, events) = ExerciseEngine::new(plan, ctx, t_us);
        let side = engine.side();
        let mut s = Self {
            recognizer: Recognizer::new(cfg.recognizer, vec![]),
            recognizer_side: side,
            scheduler: Scheduler::new(level, cfg.cooldown_reps, slow),
            cfg,
            setup,
            observer,
            techniques,
            engine,
            next_rep: 1,
            movement_seq: 0,
            set_results: vec![],
            fresh_times: VecDeque::new(),
            live_ids: vec![],
            basic_since: None,
        };
        s.rebuild_recognizer(side);
        Ok((s, events))
    }

    fn tempo_scale(&self) -> f64 {
        if self.scheduler.slow_stage {
            self.expected()
                .and_then(|t| t.def.tempo.as_ref())
                .map_or(0.5, |t| t.slow_scale)
        } else {
            1.0
        }
    }

    fn expected(&self) -> Option<&Arc<CompiledTechnique>> {
        self.techniques.sides[side_index(self.recognizer_side)].as_ref()
    }

    fn rebuild_recognizer(&mut self, side: Side) {
        self.recognizer_side = side;
        let mut bank = vec![];
        if let Some(t) = &self.techniques.sides[side_index(side)] {
            bank.push((Role::Expected, t.clone()));
            self.live_ids = t
                .required
                .iter()
                .chain(&t.optional)
                .filter_map(|o| o.index)
                .map(|i| ids()[i].0.clone())
                .collect();
            self.live_ids.dedup();
        }
        if let Some(t) = &self.techniques.sides[side_index(other(side))] {
            bank.push((Role::Mirror, t.clone()));
        }
        for t in &self.techniques.distractors[side_index(side)] {
            bank.push((Role::Distractor, t.clone()));
        }
        let mut cfg = self.cfg.recognizer;
        cfg.tempo_scale = self.tempo_scale();
        self.recognizer = Recognizer::new(cfg, bank);
    }

    pub fn state(&self) -> TrainingState {
        let mut s = self.engine.state();
        s.live_observables = self.live_ids.clone();
        s
    }

    pub fn finished(&self) -> bool {
        self.engine.finished()
    }

    pub fn session_id(&self) -> &SessionId {
        &self.setup.session_id
    }

    pub fn technique(&self) -> Option<&Arc<CompiledTechnique>> {
        self.expected()
    }

    pub fn command(
        &mut self,
        t_us: u64,
        cmd: TrainingCommand,
        args: &TrainingCommandArgs,
    ) -> Result<Vec<TrainingEvent>, Rejected> {
        let ev = self.engine.command(t_us, cmd, args)?;
        Ok(self.post(ev))
    }

    pub fn tick(&mut self, t_us: u64) -> Vec<TrainingEvent> {
        let ev = self.engine.tick(t_us);
        self.post(ev)
    }

    fn movement(
        &mut self,
        kind: MovementEventKind,
        t_us: u64,
        subject: Option<String>,
        value: Option<f64>,
        frame: &ObservableFrame,
    ) -> MovementEvent {
        self.movement_seq += 1;
        let evidence = self
            .expected()
            .and_then(|t| t.driver.index)
            .map_or_else(unavailable, |i| frame.get(i).evidence.clone());
        MovementEvent {
            seq: self.movement_seq,
            t_us,
            kind,
            subject,
            value,
            evidence,
        }
    }

    /// One solver tick.
    pub fn sample(&mut self, sample: &MotionSample) -> Output {
        // Profile `basic` held for a second: a multi-view rig dropping views for a moment stays on
        // world observables with a gap instead of mixing in image-plane values (08 §10).
        self.basic_since = (self.setup.capability_profile == CapabilityProfileId::Basic)
            .then(|| self.basic_since.unwrap_or(sample.t_us));
        self.observer.allow_planar(
            self.basic_since
                .is_some_and(|s| sample.t_us.saturating_sub(s) >= BASIC_HOLD_US),
        );
        let frame = self.observer.step(sample);
        let mut events = vec![];
        let ctx_side = self.engine.recognition_context().side;
        if ctx_side != self.recognizer_side {
            self.rebuild_recognizer(ctx_side);
        }
        for out in self.recognizer.step(&frame) {
            match out {
                RecognitionOutput::Event(e) => {
                    let ev = match e {
                        RecognitionEvent::ActivityStart { t_us } => {
                            let m = self.movement(
                                MovementEventKind::ActivityStart,
                                t_us,
                                None,
                                None,
                                &frame,
                            );
                            self.engine.movement(&m)
                        }
                        RecognitionEvent::ActivityEnd { .. } => vec![],
                        RecognitionEvent::RepStarted { t_us } => {
                            self.engine.rep_started(self.next_rep, t_us)
                        }
                        RecognitionEvent::PhaseEnter { phase, t_us } => {
                            let m = self.movement(
                                MovementEventKind::PhaseEnter,
                                t_us,
                                Some(phase),
                                None,
                                &frame,
                            );
                            self.engine.movement(&m)
                        }
                        RecognitionEvent::Strike { t_us, value } => {
                            let m = self.movement(
                                MovementEventKind::StrikePeak,
                                t_us,
                                self.expected().map(|t| t.def.id.clone()),
                                Some(value),
                                &frame,
                            );
                            self.engine.movement(&m)
                        }
                    };
                    events.extend(ev);
                }
                RecognitionOutput::Result(f) => events.extend(self.result(*f)),
            }
        }
        self.track_rate(&frame);
        let availability = self.availability(&frame);
        self.engine
            .set_metric_availability(availability, self.setup.capability_profile);
        // Stillness for the start-stance gate: the segmentation driver below its end threshold.
        let still = self.expected().and_then(|t| {
            let v = frame.get(t.driver.index?);
            let x = if t.driver.norm { v.norm } else { v.value }?;
            Some(x.abs() < t.end_below)
        });
        self.engine.set_still(still.unwrap_or(false));
        events.extend(self.engine.sample(&frame));
        Output {
            events: self.post(events),
            frame: Some(frame),
        }
    }

    pub fn capability(&mut self, report: &CapabilityReport) {
        self.setup.capability_profile = report.profile;
    }

    fn result(&mut self, f: Finalized) -> Vec<TrainingEvent> {
        if !self.engine.accepts_results() {
            return vec![];
        }
        let outcome = match f.outcome {
            Outcome::Completed => RepOutcome::Completed,
            Outcome::RejectedQuality => RepOutcome::RejectedQuality,
            Outcome::Incomplete => RepOutcome::Incomplete,
            Outcome::WrongSide => RepOutcome::WrongSide,
            Outcome::Unknown | Outcome::Ambiguous | Outcome::Extra => {
                let data = json!({
                    "result": match f.outcome { Outcome::Ambiguous => "ambiguous", Outcome::Extra => "extra", _ => "unknown" },
                    "reason": f.reason,
                    "duration_ms": (f.segment.1 - f.segment.0) / 1000,
                    "peak_activity": f.peak_activity,
                    "candidates": f.candidates.iter().map(|c| json!({ "technique": c.reference, "side": c.side, "confidence": c.confidence })).collect::<Vec<_>>(),
                });
                return self.engine.unrecognised(f.segment.1, data);
            }
        };
        let rep_index = self.next_rep;
        self.next_rep += 1;
        let r = self.analyse(&f, outcome, rep_index);
        self.set_results.push(r.clone());
        self.engine.repetition(&r)
    }

    fn analyse(&mut self, f: &Finalized, outcome: RepOutcome, rep_index: u32) -> RepetitionResult {
        let measured = matches!(outcome, RepOutcome::Completed | RepOutcome::RejectedQuality);
        let frames = if measured {
            self.observer.refiltered(f.rep_start, f.rep_end)
        } else {
            vec![]
        };
        // Re-running the machine on zero-phase series removes filter lag from phase anchors.
        let fin = if measured {
            rerun(
                f.tech.clone(),
                &frames,
                self.recognizer.rest(),
                f.segment,
                self.tempo_scale(),
            )
            .unwrap_or_else(|| f.clone())
        } else {
            f.clone()
        };
        let tech = fin.tech.clone();
        let phases: Vec<(String, u64, u64)> = fin
            .phases
            .iter()
            .enumerate()
            .map(|(k, (p, t))| {
                let end = fin.phases.get(k + 1).map_or(fin.rep_end, |n| n.1);
                (tech.phases[*p].id.clone(), *t, end)
            })
            .collect();
        let event_evidence = |a: u64, b: u64| {
            let ev: Vec<Evidence> = frames
                .iter()
                .filter(|fr| fr.t_us >= a && fr.t_us <= b)
                .flat_map(|fr| {
                    tech.required
                        .iter()
                        .filter_map(|o| o.index.map(|i| fr.get(i).evidence.clone()))
                })
                .filter(|e| e.provenance != Provenance::Unavailable)
                .collect();
            combine(ev.iter()).unwrap_or_else(unavailable)
        };
        let repetition = Repetition {
            rep_index,
            technique: tech.reference.clone(),
            side: tech.side,
            stance: self.setup.stance,
            outcome,
            t_start_us: fin.rep_start,
            t_end_us: fin.rep_end,
            phases: phases
                .iter()
                .map(|(p, a, b)| PhaseInterval {
                    phase: p.clone(),
                    t_start_us: *a,
                    t_end_us: *b,
                    evidence: event_evidence(*a, *b),
                })
                .collect(),
            strike_t_us: fin.strike.map(|s| s.0),
            recognition_confidence: f.confidence,
            capability_profile: self.setup.capability_profile,
            reached_phase: if outcome == RepOutcome::Incomplete {
                f.reached.map(|p| tech.phases[p].id.clone())
            } else {
                None
            },
        };
        let measurements: Vec<MetricMeasurement> = if measured {
            let w = RepWindow {
                frames: &frames,
                rep_start: fin.rep_start,
                rep_end: fin.rep_end,
                phases,
                strike: fin.strike.map(|s| s.0),
                event_inputs: tech.required.clone(),
                planar_view: self.observer.planar_view(),
            };
            tech.metrics
                .iter()
                .map(|m| measure(m, &w, &self.setup.grade, &self.observer.lengths()))
                .collect()
        } else {
            vec![]
        };
        let scored = outcome == RepOutcome::Completed;
        let score_result = match (&tech.scoring, scored) {
            (Some(model), true) => Some(score(
                model,
                true,
                &measurements,
                &self.setup.personal,
                self.setup.tolerance_scale,
            )),
            _ => None,
        };
        let cue = match (&score_result, &tech.scoring) {
            (Some(s), Some(model)) => self.scheduler.after_rep(&tech.def.cues, s, model),
            _ => None,
        };
        RepetitionResult {
            repetition,
            measurements,
            score: score_result,
            cue,
            targets: vec![],
            scored,
            excluded: false,
        }
    }

    /// Adds set summaries and the set's coaching to engine events; starts scheduler sets.
    fn post(&mut self, mut events: Vec<TrainingEvent>) -> Vec<TrainingEvent> {
        for e in &mut events {
            match e.event_type.as_str() {
                "set_started" => {
                    let target = e.data["target"]["reps"].as_u64().unwrap_or(0) as u32;
                    self.scheduler.start_set(target);
                    self.set_results.clear();
                }
                "set_completed" => {
                    let tech = self.expected().cloned();
                    let cues = tech
                        .as_ref()
                        .map(|t| self.scheduler.after_set(&t.def.cues, t.scoring.as_ref()))
                        .unwrap_or_default();
                    e.data["summary"] = set_summary(&self.set_results);
                    e.data["cues"] = serde_json::to_value(cues).unwrap_or_default();
                }
                _ => {}
            }
        }
        events
    }

    fn track_rate(&mut self, f: &ObservableFrame) {
        let t = f
            .values
            .iter()
            .filter(|v| v.fresh)
            .filter_map(|v| v.evidence.t_obs_us)
            .max();
        if let Some(t) = t
            && self.fresh_times.back() != Some(&t)
        {
            self.fresh_times.push_back(t);
            while self.fresh_times.front().is_some_and(|x| t - x > 1_000_000) {
                self.fresh_times.pop_front();
            }
        }
    }

    fn rate(&self) -> Option<f64> {
        let (a, b) = (self.fresh_times.front()?, self.fresh_times.back()?);
        (b > a).then(|| (self.fresh_times.len() - 1) as f64 / ((b - a) as f64 * 1e-6))
    }

    /// Best attainable grade per metric from what is observed now (22 §5).
    fn availability(&self, frame: &ObservableFrame) -> Vec<MetricAvailability> {
        let Some(t) = self.expected() else {
            return vec![];
        };
        let rate = self.rate();
        t.metrics
            .iter()
            .map(|m| {
                let d = &m.def;
                let inputs: Vec<Obs> = match d.kind {
                    MetricKind::Duration => t.required.clone(),
                    _ => m.obs.clone().into_iter().collect(),
                };
                let mut reasons = vec![];
                let grade = (|| {
                    if !d.kind.implemented() {
                        reasons.push(reason(
                            "metric_kind_not_implemented",
                            Some(&d.id),
                            "Not measured by this engine version.",
                        ));
                        return Grade::X;
                    }
                    if inputs.iter().any(|o| o.index.is_none()) {
                        reasons.push(reason(
                            "role_unresolved",
                            None,
                            "Lead and rear sides are unknown: set a default stance.",
                        ));
                        return Grade::X;
                    }
                    let ev: Vec<Evidence> = inputs
                        .iter()
                        .filter_map(|o| {
                            let v = frame.get(o.index?);
                            if !crate::observables::present(v) {
                                reasons.extend(v.reasons.iter().take(1).cloned());
                                return None;
                            }
                            Some(v.evidence.clone())
                        })
                        .collect();
                    if ev.len() < inputs.len() || inputs.is_empty() && m.obs.is_some() {
                        return Grade::X;
                    }
                    for k in &d.gates.calibration {
                        if self.setup.grade.calibration.get(k) != Some(&CalibrationState::Valid) {
                            let name = serde_json::to_value(k)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_default();
                            reasons.push(reason(
                                "calibration_missing",
                                Some(&name),
                                "A calibration this measurement needs is not valid.",
                            ));
                            return Grade::X;
                        }
                    }
                    if let (Some(min), Some(r)) = (d.gates.min_rate_hz, rate)
                        && r < min * 0.95
                    {
                        reasons.push(reason(
                            "rate_too_low",
                            None,
                            &format!("Input rate {r:.0} Hz is below {min:.0} Hz."),
                        ));
                        return Grade::X;
                    }
                    let Some(e) = combine(ev.iter()) else {
                        return Grade::X;
                    };
                    if e.method == MethodId::ImagePlane {
                        return match crate::metrics::planar_rule(
                            d,
                            rate,
                            self.observer.planar_view(),
                        ) {
                            crate::metrics::Planar::Suppressed(r) => {
                                reasons.push(r);
                                Grade::X
                            }
                            crate::metrics::Planar::Graded {
                                cap, reasons: r, ..
                            } => {
                                reasons.extend(r);
                                cap
                            }
                        };
                    }
                    if e.provenance == Provenance::Inferred {
                        reasons.push(reason(
                            "model_fill",
                            None,
                            "Inputs are filled by a model, not observed.",
                        ));
                        return Grade::X;
                    }
                    let a = d.gates.grade_a_methods.contains(&e.method);
                    let b = a || d.gates.grade_b_methods.contains(&e.method);
                    if !b {
                        reasons.push(reason(
                            "method_not_accepted",
                            None,
                            "The current tracking does not observe this accurately enough.",
                        ));
                        return Grade::X;
                    }
                    let capped = d
                        .gates
                        .rate_for_a_hz
                        .zip(rate)
                        .is_some_and(|(need, r)| r < need * 0.95)
                        || self.setup.grade.qa == crate::metrics::Qa::Fail
                        || (self.setup.grade.qa == crate::metrics::Qa::NotRun
                            && !d.gates.calibration.is_empty());
                    if a && self.setup.grade.origin_live && !capped {
                        Grade::A
                    } else {
                        reasons.push(if !self.setup.grade.origin_live {
                            reason(
                                "origin_not_live",
                                None,
                                "Simulated or recorded inputs are not graded A.",
                            )
                        } else {
                            reason(
                                "estimated",
                                None,
                                "Estimated, below the highest accuracy class.",
                            )
                        });
                        Grade::B
                    }
                })();
                MetricAvailability {
                    metric: d.id.clone(),
                    best_grade: grade,
                    reasons,
                }
            })
            .collect()
    }
}

/// Per-metric medians of the set's graded values (06 §5.3 set summary).
fn set_summary(results: &[RepetitionResult]) -> serde_json::Value {
    let mut by: std::collections::BTreeMap<String, (Vec<f64>, Grade, String)> = Default::default();
    for r in results.iter().filter(|r| r.scored) {
        for m in r.measurements.iter().filter(|m| m.grade <= Grade::B) {
            if let Some(v) = m.value {
                let e = by
                    .entry(m.metric.clone())
                    .or_insert((vec![], Grade::A, m.unit.clone()));
                e.0.push(v);
                e.1 = e.1.max(m.grade);
            }
        }
    }
    json!({
        "reps": results.len(),
        "scored": results.iter().filter(|r| r.scored).count(),
        "medians": by.into_iter().map(|(k, (v, g, unit))| json!({
            "metric": k, "median": crate::progress::median(&v), "count": v.len(), "worst_grade": g, "unit": unit
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests;
