//! Training wire types (martial-arts 04 §6–7, 05 §8, 07 §7–8, 14 §7).
use crate::{AthleteId, CapabilityProfileId, Evidence, Reason, SessionId, TimeUs, Vec3};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `"<ContentId>@<version>"`, e.g. `taekwondo.dollyeo_chagi@1`.
pub type ContentRef = String;

/// One observable at one tick (04 §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ObservableValue {
    /// Sided ids end in `.left` / `.right`, e.g. `knee_flexion.left`.
    pub id: String,
    /// Native unit; `null` when unavailable.
    pub value: Option<f64>,
    /// Normalised form where one is defined.
    pub norm: Option<f64>,
    pub evidence: Evidence,
    /// True when any input had a new observation time this tick.
    pub fresh: bool,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Orthodox,
    Southpaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MovementEventKind {
    ActivityStart,
    ActivityEnd,
    PhaseEnter,
    StrikePeak,
    StanceEnter,
    StanceExit,
    ContactChange,
    HoldStart,
    HoldEnd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MovementEvent {
    pub seq: u64,
    /// Interpolated event time (04 §7).
    pub t_us: TimeUs,
    pub kind: MovementEventKind,
    pub subject: Option<String>,
    pub value: Option<f64>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PhaseInterval {
    pub phase: String,
    pub t_start_us: TimeUs,
    pub t_end_us: TimeUs,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RepOutcome {
    Completed,
    Incomplete,
    RejectedQuality,
    WrongSide,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Repetition {
    /// Per training session, from 1.
    pub rep_index: u32,
    pub technique: ContentRef,
    pub side: Side,
    pub stance: Option<Stance>,
    pub outcome: RepOutcome,
    pub t_start_us: TimeUs,
    pub t_end_us: TimeUs,
    pub phases: Vec<PhaseInterval>,
    pub strike_t_us: Option<TimeUs>,
    /// 0..1 (07 §3.2).
    pub recognition_confidence: f64,
    pub capability_profile: CapabilityProfileId,
    /// Furthest phase reached by an incomplete repetition. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub reached_phase: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub enum Grade {
    A,
    B,
    C,
    X,
}

/// One metric value with its grade and evidence (05 §8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MetricMeasurement {
    /// Metric definition `id@version`.
    pub metric: ContentRef,
    pub value: Option<f64>,
    pub norm: Option<f64>,
    pub unit: String,
    pub grade: Grade,
    pub evidence: Evidence,
    pub comparability_class: String,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TargetFrom {
    Reference,
    Personal,
    Definition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ScoreTarget {
    pub value: f64,
    pub from: TargetFrom,
    pub ref_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ComponentScore {
    pub id: String,
    /// 0..1; `null` with reasons when not scored.
    pub score: Option<f64>,
    pub measurement: Option<MetricMeasurement>,
    pub target: Option<ScoreTarget>,
    /// Signed, in the metric's unit.
    pub deviation: Option<f64>,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ScoreResult {
    pub model: ContentRef,
    pub components: Vec<ComponentScore>,
    pub reference_match: Option<f64>,
    pub coverage: f64,
    pub comparability_class: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TextFormat {
    Deg,
    Cm,
    Ms,
    Percent,
    Count,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TextParam {
    pub value: f64,
    pub format: TextFormat,
}

/// An i18n key with numeric parameters in SI units; clients format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TextRef {
    pub key: String,
    pub params: std::collections::BTreeMap<String, TextParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueChannel {
    Realtime,
    AfterRep,
    AfterRound,
    AfterStep,
    AfterSet,
    SessionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueCategory {
    Safety,
    Tracking,
    Balance,
    Technique,
    Guard,
    Timing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueBasis {
    Measurement,
    Reference,
    History,
    Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PracticeStage {
    Demo,
    Slow,
    Normal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CueDrill {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stage: Option<PracticeStage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CueEvidence {
    pub metric: String,
    pub grade: Grade,
    pub deviation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CueInstance {
    pub id: String,
    pub channel: CueChannel,
    pub category: CueCategory,
    pub basis: CueBasis,
    pub observed: TextRef,
    pub suggestion: TextRef,
    pub drill: Option<CueDrill>,
    pub evidence: Vec<CueEvidence>,
    /// True for the one-off "resolved" cue.
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TargetResult {
    pub id: String,
    /// `null` = not assessed.
    pub reached: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct RepetitionResult {
    pub repetition: Repetition,
    pub measurements: Vec<MetricMeasurement>,
    pub score: Option<ScoreResult>,
    pub cue: Option<CueInstance>,
    pub targets: Vec<TargetResult>,
    pub scored: bool,
    pub excluded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ReadinessGate {
    pub gate: String,
    pub state: GateState,
    pub blocking: bool,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct MetricAvailability {
    pub metric: String,
    pub best_grade: Grade,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Free,
    Technique,
    Combination,
    Workout,
    Form,
    Mobility,
    Assessment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    Preparing,
    Ready,
    Countdown,
    Active,
    Resting,
    Paused,
    Completed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubState {
    Prompting,
    WaitingStance,
    Armed,
    InRep,
    BetweenReps,
    Holding,
    TrackingLost,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct PlanPosition {
    pub block: Option<u32>,
    pub item: Option<u32>,
    pub set: Option<u32>,
    pub round: Option<u32>,
    pub step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PlanSummary {
    pub kind: PlanKind,
    #[serde(rename = "ref")]
    pub plan_ref: Option<ContentRef>,
    pub title: String,
    pub level: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ExerciseSummary {
    pub kind: PlanKind,
    #[serde(rename = "ref")]
    pub exercise_ref: ContentRef,
    pub side: Option<Side>,
    pub stage: Option<PracticeStage>,
    pub stance: Option<Stance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CountUnit {
    Reps,
    Rounds,
    Holds,
    Seconds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Counts {
    pub done: u32,
    pub target: Option<u32>,
    pub unit: CountUnit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SetCounts {
    pub done: u32,
    pub target: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimerKind {
    Countdown,
    Rest,
    Item,
    Hold,
    Session,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Timer {
    pub kind: TimerKind,
    /// Session clock.
    pub started_t_us: TimeUs,
    pub duration_ms: Option<u64>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Beat {
    pub t0_us: TimeUs,
    pub period_us: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TargetPlacement {
    pub id: String,
    pub position_m: Vec3,
    pub radius_m: f64,
    pub step_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ExpectedStep {
    pub id: String,
    pub technique: ContentRef,
    pub side: Side,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CurrentItem {
    pub item_id: String,
    pub title: String,
    pub exercise: ExerciseSummary,
    pub sub_state: Option<SubState>,
    pub phase_id: Option<String>,
    pub counts: Counts,
    pub sets: SetCounts,
    pub timer: Option<Timer>,
    pub beat: Option<Beat>,
    pub targets: Vec<TargetPlacement>,
    pub expected_steps: Vec<ExpectedStep>,
    pub reference_id: Option<String>,
    pub instructions: Vec<TextRef>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct TrainingTotals {
    pub reps_completed: u32,
    pub reps_other: u32,
    pub sets_completed: u32,
    pub elapsed_ms: u64,
    pub active_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct RecordingFlags {
    pub inputs: bool,
    pub motion: bool,
    pub video: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum StorageHealth {
    #[default]
    Ok,
    Degraded,
}

/// Authoritative training session state; events are increments (14 §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingState {
    pub session_id: SessionId,
    pub athlete_id: AthleteId,
    /// Increments on every change.
    pub rev: u64,
    pub last_event_seq: u64,
    pub phase: SessionPhase,
    pub plan: PlanSummary,
    pub readiness: Vec<ReadinessGate>,
    pub metric_availability: Vec<MetricAvailability>,
    pub position: PlanPosition,
    pub current: Option<CurrentItem>,
    pub totals: TrainingTotals,
    pub live_observables: Vec<String>,
    pub last_rep: Option<RepetitionResult>,
    pub capability_profile: CapabilityProfileId,
    pub recording: RecordingFlags,
    pub storage: StorageHealth,
}

/// One increment of the session (06 §8). `event_seq` increases by exactly 1 per session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingEvent {
    pub event_seq: u64,
    pub t_us: TimeUs,
    #[serde(rename = "type")]
    pub event_type: String,
    pub position: PlanPosition,
    #[ts(type = "Record<string, unknown>")]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TrainingCommand {
    Start,
    Pause,
    Resume,
    SkipRest,
    SkipItem,
    RepeatItem,
    SkipDemo,
    SetSide,
    End,
    AckSafety,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct TrainingCommandArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub side: Option<Side>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content_refs: Option<Vec<ContentRef>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingCommandRequest {
    pub command: TrainingCommand,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub args: Option<TrainingCommandArgs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanSide {
    Left,
    Right,
    Alternate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CoachingLevel {
    Minimal,
    #[default]
    Standard,
    Detailed,
}

/// A technique plan (13 §7). `sets`, `reps` and `rest_s` follow 03 §4 and fill the one-item
/// workout the plan runs as; they are absent from the 13 §7 shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TechniquePlan {
    pub technique: String,
    pub side: PlanSide,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stages: Option<Vec<PracticeStage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub coaching: Option<CoachingLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scoring: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sets: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rest_s: Option<u32>,
}

/// Plan of a training session. Only `technique` exists in this phase; later kinds are additive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Plan {
    Technique(TechniquePlan),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CreateSessionRequest {
    pub plan: Plan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub record: Option<RecordingFlags>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub level: Option<u32>,
}

/// The running training session as listed in `SystemState`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingRef {
    pub session_id: SessionId,
    pub phase: SessionPhase,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum StorageStateKind {
    Ok,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct StorageStatus {
    pub state: StorageStateKind,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Completed,
    Aborted,
    Interrupted,
}

/// One stored training session (`GET /training/sessions`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingSessionSummary {
    pub id: SessionId,
    pub athlete_id: AthleteId,
    pub started_utc: String,
    pub ended_utc: Option<String>,
    pub status: SessionStatus,
    pub plan_kind: PlanKind,
    pub plan_ref: Option<String>,
    pub title: String,
    pub origin: crate::Origin,
    pub capability_profile: CapabilityProfileId,
    pub reps_completed: u32,
    pub reps_other: u32,
    pub sets: u32,
    pub analysis_rev: u32,
    pub reanalysis_of: Option<SessionId>,
    pub has_inputs: bool,
    pub has_motion: bool,
    pub rpe: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TrainingSessionDetail {
    pub session: TrainingSessionSummary,
    pub plan: Plan,
    #[ts(type = "Record<string, unknown> | null")]
    pub summary: Option<serde_json::Value>,
    pub sets: Vec<SetRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SetRecord {
    pub set_index: u32,
    pub content_ref: Option<String>,
    pub side: Option<Side>,
    pub status: String,
    pub started_t_us: TimeUs,
    pub ended_t_us: Option<TimeUs>,
    #[ts(type = "Record<string, unknown> | null")]
    pub summary: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ProvenanceRecord {
    pub id: String,
    pub first_used_utc: String,
    #[ts(type = "Record<string, unknown>")]
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ContentSnapshotRef {
    pub hash: String,
    pub kind: String,
    pub id: String,
    pub version: u32,
}

/// `GET /training/sessions/{id}/provenance` (32 §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SessionProvenance {
    pub session_id: SessionId,
    pub engine_version: String,
    pub observables_rev: u32,
    pub analysis_rev: u32,
    pub origin: crate::Origin,
    pub capability_profile: CapabilityProfileId,
    pub tracking_config: ProvenanceRecord,
    pub calibration_set: ProvenanceRecord,
    pub calibration_qa: String,
    pub content: Vec<ContentRef>,
    pub snapshots: Vec<ContentSnapshotRef>,
    pub reanalysis_of: Option<SessionId>,
    pub validation_reports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct RpeRequest {
    pub rpe: u8,
}
