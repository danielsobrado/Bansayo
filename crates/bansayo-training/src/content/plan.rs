//! The one execution path (06 §1): a technique plan becomes a one-block, one-item workout,
//! which compiles into the exercise engine's plan.
use super::compile::{CompiledTechnique, compile_technique, resolve};
use super::types::{Names, Status};
use super::validate::{Library, problem};
use crate::exercise::{CompiledPlan, ItemPlan};
use bansayo_contracts::{
    CoachingLevel, ContentProblem, Plan, PlanSide, PracticeStage, Side, Stance, TechniquePlan,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const WORKOUT_SCHEMA: &str = "bansayo.workout/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub reps: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkoutItem {
    pub id: String,
    pub plan: Plan,
    pub sets: u32,
    pub target: Target,
    pub rest_s: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_cap_s: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub id: String,
    pub kind: String,
    pub items: Vec<WorkoutItem>,
}

/// The subset of `bansayo.workout/v1` (13 §7) this phase executes: one block, one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkoutDef {
    pub schema: String,
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub status: Status,
    pub names: Names,
    pub blocks: Vec<Block>,
}

/// Defaults of 03 §4 for fields a technique plan may omit.
pub const DEFAULT_SETS: u32 = 1;
pub const DEFAULT_REPS: u32 = 10;
pub const DEFAULT_REST_S: u32 = 45;

/// Wraps a technique plan as the one-item workout it runs as.
pub fn wrap(plan: &TechniquePlan, title: &str) -> WorkoutDef {
    WorkoutDef {
        schema: WORKOUT_SCHEMA.into(),
        id: "session.plan".into(),
        version: 1,
        status: Status::Draft,
        names: Names {
            display: title.into(),
            native: None,
            aliases: vec![],
        },
        blocks: vec![Block {
            id: "main".into(),
            kind: "main".into(),
            items: vec![WorkoutItem {
                id: "main".into(),
                plan: Plan::Technique(plan.clone()),
                sets: plan.sets.unwrap_or(DEFAULT_SETS),
                target: Target {
                    reps: plan.reps.unwrap_or(DEFAULT_REPS),
                },
                rest_s: plan.rest_s.unwrap_or(DEFAULT_REST_S),
                time_cap_s: None,
            }],
        }],
    }
}

/// Everything recognition and analysis need for the plan's technique.
#[derive(Debug, Clone)]
pub struct PlanTechniques {
    /// Compiled for `[left, right]` (absent where the definition does not allow the side).
    pub sides: [Option<Arc<CompiledTechnique>>; 2],
    /// `confusable_with` definitions compiled for `[left, right]`.
    pub distractors: [Vec<Arc<CompiledTechnique>>; 2],
}

fn side_index(s: Side) -> usize {
    usize::from(s == Side::Right)
}

/// Compiles a workout of this phase's shape into the exercise plan and its techniques.
pub fn compile_workout(
    w: &WorkoutDef,
    lib: &Library,
    stance: Option<Stance>,
) -> Result<(CompiledPlan, PlanTechniques), Vec<ContentProblem>> {
    let (block, item) = match w.blocks.as_slice() {
        [b] => match b.items.as_slice() {
            [i] => (b, i),
            _ => {
                return Err(vec![problem(
                    "blocks[0].items",
                    "unsupported_shape",
                    "this engine runs one item per workout",
                )]);
            }
        },
        _ => {
            return Err(vec![problem(
                "blocks",
                "unsupported_shape",
                "this engine runs one block per workout",
            )]);
        }
    };
    let _ = block;
    let Plan::Technique(p) = &item.plan;
    let Some(def) = lib.techniques.get(&p.technique) else {
        return Err(vec![problem(
            "plan.technique",
            "unresolved",
            format!("unknown technique \"{}\"", p.technique),
        )]);
    };
    if item.sets == 0 || item.target.reps == 0 {
        return Err(vec![problem(
            "plan",
            "invalid_target",
            "sets and reps must be at least 1",
        )]);
    }
    if item.sets > def.safety.max_sets_per_session {
        return Err(vec![problem(
            "plan.sets",
            "safety_limit",
            format!(
                "{} allows at most {} sets per session",
                def.id, def.safety.max_sets_per_session
            ),
        )]);
    }
    let mut lib = lib.clone();
    if let Some(s) = &p.scoring {
        let Some(model) = lib.scoring.get(s) else {
            return Err(vec![problem(
                "plan.scoring",
                "unresolved",
                format!("unknown scoring model \"{s}\""),
            )]);
        };
        let mut d = def.clone();
        d.scoring = Some(model.id.clone());
        lib.techniques.insert(d.id.clone(), d);
    }
    let def = &lib.techniques[&p.technique];
    let mut sides: [Option<Arc<CompiledTechnique>>; 2] = [None, None];
    let mut distractors: [Vec<Arc<CompiledTechnique>>; 2] = [vec![], vec![]];
    for s in [Side::Left, Side::Right] {
        if def.sides.contains(&s) {
            sides[side_index(s)] = Some(Arc::new(compile_technique(def, s, stance, &lib)?));
            for c in &def.confusable_with {
                if let Some(other) = lib.techniques.get(c)
                    && other.sides.contains(&s)
                    && let Ok(t) = compile_technique(other, s, stance, &lib)
                {
                    distractors[side_index(s)].push(Arc::new(t));
                }
            }
        }
    }
    let wanted = match p.side {
        PlanSide::Left => vec![Side::Left],
        PlanSide::Right => vec![Side::Right],
        PlanSide::Alternate => vec![Side::Left, Side::Right],
    };
    for s in &wanted {
        if sides[side_index(*s)].is_none() {
            return Err(vec![problem(
                "plan.side",
                "side_not_allowed",
                format!("{} has no {s:?} side", def.id),
            )]);
        }
    }
    let stages = p
        .stages
        .clone()
        .unwrap_or_else(|| vec![PracticeStage::Normal]);
    let stage = if stages.contains(&PracticeStage::Normal) {
        PracticeStage::Normal
    } else if stages.contains(&PracticeStage::Slow) {
        PracticeStage::Slow
    } else {
        return Err(vec![problem(
            "plan.stages",
            "invalid_stages",
            "a plan needs slow or normal practice",
        )]);
    };
    let required = |s: Side| {
        def.required_observables
            .iter()
            .map(|o| resolve(o, None, s, stance, def.mirror_signed).index)
            .collect::<Vec<_>>()
    };
    let item_plan = ItemPlan {
        item_id: item.id.clone(),
        title: def.names.display.clone(),
        technique: CompiledTechnique::content_ref(def),
        family: def.family.clone(),
        side: p.side,
        sides_allowed: def.sides.clone(),
        stage,
        has_demo: stages.contains(&PracticeStage::Demo),
        sets: item.sets,
        reps: item.target.reps,
        rest_s: item.rest_s,
        time_cap_s: item.time_cap_s,
        max_reps_per_set: def.safety.max_reps_per_set,
        max_gap_us: u64::from(def.max_gap_ms.unwrap_or(80)) * 1000,
        instructions: def.instructions.clone(),
        start_stance: def.start_stance.clone(),
        required: [required(Side::Left), required(Side::Right)],
    };
    Ok((
        CompiledPlan {
            title: w.names.display.clone(),
            level: None,
            item: item_plan,
        },
        PlanTechniques { sides, distractors },
    ))
}

/// Coaching level of a plan, defaulting to the athlete's.
pub fn coaching(plan: &TechniquePlan, athlete: CoachingLevel) -> CoachingLevel {
    plan.coaching.unwrap_or(athlete)
}
