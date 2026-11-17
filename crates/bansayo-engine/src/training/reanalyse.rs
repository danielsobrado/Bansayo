//! Re-analysis (martial-arts 12 §7.2): a fresh solver and training session over a recording's
//! inputs, unpaced, with the original commands replayed at their session times. Same build, same
//! content snapshots and same mask give identical events.
use crate::recording::reader::{RecordedData, Recording};
use crate::recording::replay::Replay;
use bansayo_contracts::*;
use bansayo_training::content::validate::Library;
use bansayo_training::metrics::{GradeContext, Qa};
use bansayo_training::session::{Session, SessionSetup};
use serde::{Deserialize, Serialize};

/// A command as replayed: `(session time, command, arguments)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedCommand {
    pub t_us: TimeUs,
    pub command: TrainingCommand,
    #[serde(default)]
    pub args: TrainingCommandArgs,
}

/// The athlete facts a session uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AthleteSettings {
    pub id: AthleteId,
    pub stance: Option<Stance>,
    pub coaching: CoachingLevel,
    pub tolerance_scale: f64,
}

/// Everything about the original session that is not in its recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Original {
    pub plan: TechniquePlan,
    pub created_t_us: TimeUs,
    pub commands: Vec<TimedCommand>,
    pub athlete: AthleteSettings,
    /// Content refs already acknowledged when the session was created.
    pub safety_acked: Vec<ContentRef>,
}

impl Original {
    /// Recovers plan, creation time and commands from a session's stored events.
    pub fn from_events(
        plan: TechniquePlan,
        athlete: AthleteSettings,
        events: &[TrainingEvent],
    ) -> Self {
        let created_t_us = events.first().map_or(0, |e| e.t_us);
        let commands: Vec<TimedCommand> = events
            .iter()
            .filter(|e| e.event_type == "command")
            .filter_map(|e| {
                Some(TimedCommand {
                    t_us: e.t_us,
                    command: serde_json::from_value(e.data["command"].clone()).ok()?,
                    args: serde_json::from_value(e.data["args"].clone()).unwrap_or_default(),
                })
            })
            .collect();
        // A session that never acknowledged had the acknowledgement on file already.
        let acked = commands
            .iter()
            .any(|c| c.command == TrainingCommand::AckSafety);
        let technique = events
            .first()
            .and_then(|e| e.data["plan"]["technique"].as_str())
            .map(String::from);
        Self {
            plan,
            created_t_us,
            commands,
            athlete,
            safety_acked: if acked {
                vec![]
            } else {
                technique.into_iter().collect()
            },
        }
    }
}

pub struct Output {
    pub events: Vec<TrainingEvent>,
    pub context: crate::pipeline::command::TrainingContext,
}

/// Inputs from masked devices are removed before solving (`source_mask`, 12 §7.2).
pub fn mask(mut r: Recording, devices: &[DeviceId]) -> Recording {
    if devices.is_empty() {
        return r;
    }
    r.events.retain(|e| match &e.data {
        RecordedData::Event(ev) => match ev.as_ref() {
            SourceEvent::Observation { observation } => !devices.contains(&observation.device_id),
            _ => true,
        },
        RecordedData::Context(_) => true,
    });
    r.initial.latest.retain(|o| !devices.contains(&o.device_id));
    r
}

pub fn run(
    recording: Recording,
    original: &Original,
    library: &Library,
    session_id: SessionId,
) -> Result<Output, String> {
    let rate = recording.initial.config.engine.publish_rate_hz;
    let started = recording.initial.started_t_us;
    let mut replay = Replay::new(recording, 1.0, false)?;
    let context = crate::pipeline::training_context_of(
        &replay.skeleton,
        &replay.registry,
        &replay.store,
        &replay.recording.initial.config,
        started,
        false,
    );
    let def = library
        .techniques
        .get(&original.plan.technique)
        .ok_or("The session's technique is not in its content snapshot.")?;
    let setup = SessionSetup {
        session_id,
        athlete_id: original.athlete.id.clone(),
        stance: original.athlete.stance,
        coaching: original.athlete.coaching,
        tolerance_scale: original.athlete.tolerance_scale,
        safety_acked: original.safety_acked.clone(),
        skeleton: context.skeleton.clone(),
        library: library.clone(),
        plan: original.plan.clone(),
        grade: GradeContext {
            calibration: context.calibration_states.clone(),
            qa: Qa::NotRun,
            origin_live: context.origin == Origin::Live,
            detector_id: context.detector_id.clone(),
            proportions_valid: context.skeleton.proportions == CalibrationState::Valid,
            max_gap_us: u64::from(def.max_gap_ms.unwrap_or(80)) * 1000,
        },
        // Personal targets are not part of a recording; re-analysis scores without them.
        personal: Default::default(),
        recording: RecordingFlags::default(),
        capability_profile: context.capability.profile,
        tracking_config_id: context.tracking.0.clone(),
    };
    let (mut session, mut events) = Session::new(
        super::session_config(&replay.recording.initial.config),
        setup,
        original.created_t_us,
    )
    .map_err(|p| format!("The plan does not compile: {p:?}"))?;
    let period = (1e6 / rate).round() as u64;
    let mut commands = original.commands.iter().peekable();
    let mut last = None;
    while !replay.paused {
        replay.advance(period)?;
        let Some(sample) = replay.latest_motion.clone() else {
            continue;
        };
        // Ticks before the session existed only warm the solver up.
        if last == Some(sample.t_us) || sample.t_us < original.created_t_us {
            continue;
        }
        last = Some(sample.t_us);
        let out = session.sample(&sample);
        events.extend(out.events);
        events.extend(session.tick(sample.t_us));
        // Commands act on the state after this tick's sample, as they did live (one tick at most).
        while let Some(c) = commands.next_if(|c| c.t_us <= sample.t_us) {
            // A command rejected now was rejected then too, or the inputs diverged; either way
            // the replay continues and the event log shows it.
            if let Ok(ev) = session.command(c.t_us, c.command, &c.args) {
                events.extend(ev);
            }
        }
        if session.finished() {
            break;
        }
    }
    // A re-analysis runs to the end of its inputs; a session still open there is ended.
    if !session.finished()
        && let Some(t) = last
        && let Ok(ev) = session.command(t, TrainingCommand::End, &TrainingCommandArgs::default())
    {
        events.extend(ev);
    }
    Ok(Output { events, context })
}
