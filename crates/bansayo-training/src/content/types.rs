//! Serde shapes of content files (13 §3, §5, §6). Unknown fields are rejected (13 §11).
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Names {
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Draft,
    Reviewed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Rad,
    RadS,
    M,
    MS,
    LegLength,
    LegLengthS,
    ArmLength,
    ArmLengthS,
    ShoulderWidth,
    FootLength,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Gt,
    Lt,
    Between,
    RisesAbove,
    FallsBelow,
    LocalMax,
    LocalMin,
    ChangedBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefPoint {
    #[default]
    Absolute,
    RepStart,
    PhaseStart,
    Rest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AtomValue {
    One(f64),
    Range([f64; 2]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Atom {
    pub obs: String,
    pub op: Op,
    pub value: AtomValue,
    pub unit: Unit,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<RefPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prominence: Option<f64>,
}

/// `{all: [...]}`, `{any: [...]}` or one atom; nesting at most one level (05 §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Atom(Atom),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segmentation {
    pub driver: String,
    pub unit: Unit,
    pub start_above: f64,
    pub end_below: f64,
    pub min_ms: u32,
    pub max_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub optional: bool,
    pub enter: Condition,
    #[serde(default)]
    pub min_ms: u32,
    pub max_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrikeDef {
    pub obs: String,
    pub op: Op,
    pub unit: Unit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prominence: Option<f64>,
    pub in_phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricUse {
    #[serde(rename = "use")]
    pub metric: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeyondBound {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LowHigh {
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trend {
    pub metric: String,
    pub worse_by: f64,
    pub over_reps: u32,
}

/// Cue condition forms (07 §8.1). `trend` and `step` parse and validate; roadmap Phase 1
/// evaluates only `component` and `metric`, so the others never fire yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CueWhen {
    Trend {
        trend: Trend,
    },
    Step {
        step: String,
    },
    Component {
        component: String,
        below: f64,
    },
    Metric {
        metric: String,
        beyond: BeyondBound,
        direction: LowHigh,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Focus {
    External,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueDef {
    pub id: String,
    pub category: bansayo_contracts::CueCategory,
    pub basis: bansayo_contracts::CueBasis,
    pub when: CueWhen,
    pub priority: u32,
    pub min_grade: bansayo_contracts::Grade,
    pub observed: String,
    pub suggestion: String,
    pub focus: Focus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill: Option<bansayo_contracts::CueDrill>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDef {
    pub level: String,
    pub distance: f64,
    pub unit: Unit,
    pub radius_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tempo {
    pub slow_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub notes: Vec<String>,
    pub min_clear_radius_m: f64,
    pub max_reps_per_set: u32,
    pub max_sets_per_session: u32,
    pub warmup_required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressionStep {
    pub level: u32,
    pub label: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechniqueKind {
    Dynamic,
    Hold,
    Stance,
    Transition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLimb {
    Leg,
    Arm,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueDef {
    pub schema: String,
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    pub names: Names,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    pub discipline: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    pub family: String,
    pub kind: TechniqueKind,
    pub sides: Vec<bansayo_contracts::Side>,
    pub active_limb: ActiveLimb,
    #[serde(default = "yes")]
    pub mirror_signed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_stance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_stance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_observables_rev: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segmentation: Option<Segmentation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<PhaseDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<StrikeDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<Condition>,
    pub required_observables: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_observables: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub confusable_with: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<MetricUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cues: Vec<CueDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<Tempo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_gap_ms: Option<u32>,
    pub safety: Safety,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progression: Vec<ProgressionStep>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Extremum,
    ValueAt,
    Range,
    Duration,
    Mean,
    Median,
    Rms,
    PeakCount,
    SettleTime,
    PathDeviation,
    SetVariability,
    Asymmetry,
}

impl MetricKind {
    /// Kinds implemented in this phase (roadmap Phase 1).
    pub fn implemented(self) -> bool {
        matches!(
            self,
            Self::Extremum | Self::ValueAt | Self::Range | Self::Duration
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agg {
    Max,
    Min,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Window {
    Named(WindowName),
    Phase {
        phase: String,
    },
    Span {
        from_phase: String,
        to_phase: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowName {
    Rep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormBy {
    LegLength,
    ArmLength,
    Stature,
    ShoulderWidth,
    FootLength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Norm {
    pub by: NormBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HigherIsMore,
    LowerIsMore,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewName {
    Frontal,
    Lateral,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseForm {
    Norm,
    Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePlaneGate {
    pub views: Vec<ViewName>,
    #[serde(rename = "use")]
    pub use_form: UseForm,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradeA {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sigma: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradeB {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rate_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_for_a_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calibration: Vec<bansayo_contracts::CalibrationKind>,
    pub grade_a_methods: Vec<bansayo_contracts::MethodId>,
    pub grade_b_methods: Vec<bansayo_contracts::MethodId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_a: Option<GradeA>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_b: Option<GradeB>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_plane: Option<ImagePlaneGate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppress_when: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricDef {
    pub id: String,
    pub version: u32,
    pub label: String,
    pub kind: MetricKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agg: Option<Agg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unwrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prominence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_ms: Option<u32>,
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub norm: Option<Norm>,
    pub direction: Direction,
    pub gates: Gates,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricPack {
    pub schema: String,
    pub metrics: Vec<MetricDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentSource {
    Metric {
        metric: String,
        #[serde(rename = "use")]
        use_form: UseForm,
    },
    Kind {
        kind: SourceKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Outcome,
    StepResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalStat {
    Median,
    Best,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetSource {
    Reference,
    Personal {
        stat: PersonalStat,
        window_sessions: u32,
    },
    Definition {
        value: f64,
        rationale: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreDirection {
    AtLeast,
    AtMost,
    Match,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    pub inner: f64,
    pub outer: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDef {
    pub id: String,
    pub label: String,
    pub source: ComponentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<ScoreDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<Tolerance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_grade: Option<bansayo_contracts::Grade>,
    #[serde(default)]
    pub required: bool,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    pub families: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composite {
    pub id: String,
    pub min_coverage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringModel {
    pub schema: String,
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_on: Option<String>,
    pub names: Names,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    pub applies_to: AppliesTo,
    pub components: Vec<ComponentDef>,
    pub composite: Composite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleDef {
    pub id: String,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisciplineDef {
    pub schema: String,
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_on: Option<String>,
    pub names: Names,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    pub styles: Vec<StyleDef>,
    pub default_style: Option<String>,
    pub uses_lead_rear: bool,
    /// A stance technique id; may be absent until stance definitions ship.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neutral_stance: Option<String>,
}
