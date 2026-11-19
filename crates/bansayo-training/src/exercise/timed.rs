//! Session-clock execution of timed follow-along workouts.
use super::{Rejected, rejected};
use crate::content::validate::{problem, schema_problems, valid_id};
use bansayo_contracts::*;
use serde_json::{Value, json};

pub fn validate(def: &TimedWorkoutDef) -> Vec<ContentProblem> {
    let mut problems = schema_problems(&def.schema, "bansayo.timed-workout/v1");
    if !valid_id(&def.id) || def.version == 0 || def.names.display.trim().is_empty() {
        problems.push(problem(
            "id",
            "invalid_header",
            "a workout needs an id, version and title",
        ));
    }
    if def.blocks.is_empty() {
        problems.push(problem("blocks", "missing_items", "add at least one block"));
    }
    for (b, block) in def.blocks.iter().enumerate() {
        if block.id.is_empty() || block.title.trim().is_empty() || block.items.is_empty() {
            problems.push(problem(
                &format!("blocks[{b}]"),
                "invalid_block",
                "a block needs an id, title and at least one item",
            ));
        }
        for (i, item) in block.items.iter().enumerate() {
            if item.id.is_empty()
                || item.title.trim().is_empty()
                || item.instruction.trim().is_empty()
                || item.duration_s == 0
            {
                problems.push(problem(
                    &format!("blocks[{b}].items[{i}]"),
                    "invalid_item",
                    "a timed item needs an id, title, prompt and positive duration",
                ));
            }
        }
    }
    problems
}

struct Station {
    block: u32,
    item: u32,
    end_us: u64,
    ends_block: bool,
    def: TimedWorkoutItem,
}

pub struct TimedWorkout {
    stations: Vec<Station>,
    station: usize,
    blocks_completed: u32,
    state: TrainingState,
    now: u64,
    countdown_us: u64,
    /// Origin for active time. Rebased when a paused workout resumes.
    origin: u64,
    active_us: u64,
}

impl TimedWorkout {
    pub fn new(
        def: TimedWorkoutDef,
        session_id: SessionId,
        athlete_id: AthleteId,
        countdown_ms: u64,
        now: u64,
    ) -> Result<(Self, Vec<TrainingEvent>), Vec<ContentProblem>> {
        let problems = validate(&def);
        if !problems.is_empty() {
            return Err(problems);
        }
        let mut stations = Vec::new();
        let mut end_us = 0;
        for (b, block) in def.blocks.into_iter().enumerate() {
            let last = block.items.len() - 1;
            for (i, item) in block.items.into_iter().enumerate() {
                end_us += u64::from(item.duration_s) * 1_000_000;
                stations.push(Station {
                    block: b as u32,
                    item: i as u32,
                    end_us,
                    ends_block: i == last,
                    def: item,
                });
            }
        }
        let mut workout = Self {
            stations,
            station: 0,
            blocks_completed: 0,
            state: TrainingState {
                session_id,
                athlete_id,
                rev: 1,
                last_event_seq: 0,
                phase: SessionPhase::Ready,
                plan: PlanSummary {
                    kind: PlanKind::Workout,
                    plan_ref: Some(format!("{}@{}", def.id, def.version)),
                    title: def.names.display,
                    level: None,
                },
                readiness: vec![],
                metric_availability: vec![],
                position: PlanPosition::default(),
                current: None,
                totals: TrainingTotals::default(),
                live_observables: vec![],
                last_rep: None,
                capability_profile: CapabilityProfileId::None,
                recording: RecordingFlags::default(),
                storage: StorageHealth::Ok,
            },
            now,
            countdown_us: countdown_ms * 1000,
            origin: now,
            active_us: 0,
        };
        let event = workout.event(
            "session_started",
            now,
            json!({ "plan": workout.state.plan }),
        );
        Ok((workout, vec![event]))
    }

    pub fn session_id(&self) -> &SessionId {
        &self.state.session_id
    }

    pub fn finished(&self) -> bool {
        matches!(
            self.state.phase,
            SessionPhase::Completed | SessionPhase::Aborted
        )
    }

    fn event(&mut self, kind: &str, t_us: u64, data: Value) -> TrainingEvent {
        self.state.last_event_seq += 1;
        self.state.rev += 1;
        TrainingEvent {
            event_seq: self.state.last_event_seq,
            t_us,
            event_type: kind.into(),
            position: self.state.position.clone(),
            data,
        }
    }

    fn start_item(&mut self, at: u64) -> TrainingEvent {
        let station = &self.stations[self.station];
        self.state.position.block = Some(station.block);
        self.state.position.item = Some(station.item);
        self.event(
            "item_started",
            at,
            json!({
                "item_id": station.def.id, "target": { "duration_s": station.def.duration_s },
            }),
        )
    }

    fn close(&mut self, cause: &str, at: u64) -> TrainingEvent {
        let complete = cause == "plan_finished";
        self.state.phase = if complete {
            SessionPhase::Completed
        } else {
            SessionPhase::Aborted
        };
        self.event(if complete { "session_completed" } else { "session_aborted" }, at, json!({
            "cause": cause, "totals": self.state.totals, "blocks_completed": self.blocks_completed,
        }))
    }

    pub fn tick(&mut self, now: u64) -> Vec<TrainingEvent> {
        self.now = self.now.max(now);
        let mut out = Vec::new();
        if self.state.phase == SessionPhase::Countdown && self.now >= self.origin {
            self.state.phase = SessionPhase::Active;
            out.push(self.start_item(self.origin));
        }
        if self.state.phase != SessionPhase::Active {
            return out;
        }
        let elapsed_us = self.now - self.origin;
        self.active_us = elapsed_us.min(self.stations.last().unwrap().end_us);
        let ms = self.active_us / 1000;
        if ms / 1000 != self.state.totals.active_ms / 1000 {
            self.state.rev += 1;
        }
        self.state.totals.active_ms = ms;
        self.state.totals.elapsed_ms = ms;
        while self.active_us >= self.stations[self.station].end_us {
            let station = &self.stations[self.station];
            let at = self.origin + station.end_us;
            let ends_block = station.ends_block;
            let block = station.block;
            out.push(self.event(
                "item_completed",
                at,
                json!({ "item_id": station.def.id, "status": "completed" }),
            ));
            if ends_block {
                self.blocks_completed += 1;
                out.push(self.event("block_completed", at, json!({ "block": block })));
            }
            if self.station + 1 == self.stations.len() {
                out.push(self.close("plan_finished", at));
                break;
            }
            self.station += 1;
            out.push(self.start_item(at));
        }
        out
    }

    pub fn command(
        &mut self,
        now: u64,
        cmd: TrainingCommand,
    ) -> Result<Vec<TrainingEvent>, Rejected> {
        use SessionPhase as P;
        use TrainingCommand as C;
        let valid = match cmd {
            C::Start => self.state.phase == P::Ready,
            C::Pause => self.state.phase == P::Active,
            C::Resume => self.state.phase == P::Paused,
            C::End => !self.finished(),
            _ => false,
        };
        if !valid {
            return Err(rejected(
                "conflict",
                "This command is not valid in the current state.",
            ));
        }
        let mut out = self.tick(now);
        if self.finished() {
            return Ok(out);
        }
        out.push(self.event("command", self.now, json!({ "command": cmd })));
        match cmd {
            C::Start => {
                self.origin = self.now + self.countdown_us;
                self.state.phase = P::Countdown;
                out.extend(self.tick(self.now));
            }
            C::Pause => {
                self.state.phase = P::Paused;
                out.push(self.event("session_paused", self.now, json!({ "cause": "command" })));
            }
            C::Resume => {
                self.origin = self.now - self.active_us;
                self.state.phase = P::Active;
                out.push(self.event("session_resumed", self.now, json!({ "cause": "command" })));
            }
            C::End => out.push(self.close("end", self.now)),
            _ => unreachable!(),
        }
        Ok(out)
    }

    pub fn state(&self) -> TrainingState {
        use SessionPhase as P;
        let mut state = self.state.clone();
        if self.finished() {
            return state;
        }
        let station = &self.stations[self.station].def;
        let duration_ms = u64::from(station.duration_s) * 1000;
        let start_us = self.stations[self.station].end_us - duration_ms * 1000;
        let elapsed_ms = self.active_us.saturating_sub(start_us) / 1000;
        let timer = if state.phase == P::Countdown {
            Timer {
                kind: TimerKind::Countdown,
                started_t_us: self.origin - self.countdown_us,
                duration_ms: Some(self.countdown_us / 1000),
                elapsed_ms: self.now.saturating_sub(self.origin - self.countdown_us) / 1000,
            }
        } else {
            Timer {
                kind: TimerKind::Item,
                started_t_us: self.origin + start_us,
                duration_ms: Some(duration_ms),
                elapsed_ms,
            }
        };
        state.current = Some(CurrentItem {
            item_id: station.id.clone(),
            title: station.title.clone(),
            exercise: ExerciseSummary {
                kind: PlanKind::Free,
                exercise_ref: state.plan.plan_ref.clone().unwrap(),
                side: None,
                stage: None,
                stance: None,
            },
            sub_state: None,
            phase_id: None,
            counts: Counts {
                done: (elapsed_ms / 1000) as u32,
                target: Some(station.duration_s),
                unit: CountUnit::Seconds,
            },
            sets: SetCounts { done: 0, target: 1 },
            timer: Some(timer),
            beat: (state.phase == P::Active).then_some(Beat {
                t0_us: self.origin,
                period_us: 500_000,
            }),
            targets: vec![],
            expected_steps: vec![],
            reference_id: None,
            instructions: vec![],
        });
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TrainingCommand::*;

    fn class() -> TimedWorkout {
        let def = serde_saphyr::from_str(include_str!(
            "../../../../content/workouts/full_body_in_20.yaml"
        ))
        .unwrap();
        TimedWorkout::new(
            def,
            SessionId::new("20261118T120000Z-abcd").unwrap(),
            AthleteId::new("ana").unwrap(),
            3_000,
            0,
        )
        .unwrap()
        .0
    }

    #[test]
    fn class_clock_excludes_countdown_and_pause_and_completes_at_twenty_minutes() {
        let mut class = class();
        class.command(0, Start).unwrap();
        class.tick(2_000_000);
        assert_eq!(class.state().phase, SessionPhase::Countdown);
        class.tick(3_000_000);
        class.command(13_000_000, Pause).unwrap();
        assert!(class.tick(40_000_000).is_empty());
        assert_eq!(class.state().totals.active_ms, 10_000);
        class.command(43_000_000, Resume).unwrap();
        class.tick(1_052_999_999);
        assert_eq!(class.state().position.block, Some(3));
        class.tick(1_053_000_000);
        assert_eq!(class.state().position.block, Some(4));
        assert_eq!(class.state().current.unwrap().title, "Easy walk");
        let events = class.tick(1_300_000_000);
        let end = events.last().unwrap();
        assert_eq!(end.event_type, "session_completed");
        assert_eq!(end.t_us, 1_233_000_000);
        assert_eq!(end.data["blocks_completed"], 5);
        assert_eq!(end.data["totals"]["active_ms"], 1_200_000);
        assert!(class.tick(1_500_000_000).is_empty());
    }

    #[test]
    fn early_end_credits_elapsed_time_and_only_completed_blocks() {
        let mut class = class();
        class.command(0, Start).unwrap();
        class.tick(3_000_000);
        class.command(193_500_000, Pause).unwrap();
        let events = class.command(210_500_000, End).unwrap();
        let end = events.last().unwrap();
        assert_eq!(end.event_type, "session_aborted");
        assert_eq!(end.data["totals"]["active_ms"], 190_500);
        assert_eq!(end.data["blocks_completed"], 1);
    }
}
