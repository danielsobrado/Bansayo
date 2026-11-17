//! Progress series and summaries (12 §6, 14 §5.4).
use crate::{CapabilityProfileId, SessionId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct GradeMix {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub x: u32,
}

/// Which grade a session point summarises. `A` points enter baselines, records and trends;
/// `B` points form the visually secondary series (05 §8, 10 §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum PointBasis {
    A,
    B,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SeriesPoint {
    pub session_id: SessionId,
    pub date_utc: String,
    /// Median of the basis grade's values in the session, native unit.
    pub value: f64,
    pub norm: Option<f64>,
    /// Interquartile range.
    pub spread: f64,
    pub count: u32,
    pub basis: PointBasis,
    pub grade_mix: GradeMix,
    pub comparability_class: String,
    pub capability_profile: CapabilityProfileId,
    pub tracking_config_id: String,
    pub calibration_set_id: String,
    pub calibration_qa: String,
    pub technique_version: u32,
    pub metric_version: u32,
    pub origin: crate::Origin,
    pub reanalysis_of: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Baseline {
    pub value: f64,
    pub spread: f64,
    pub sessions: u32,
    pub from_utc: String,
    pub to_utc: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Best {
    /// Best single grade-A repetition value.
    pub value: f64,
    pub session_id: SessionId,
    pub rep_index: Option<u32>,
    /// Best grade-A session value.
    pub session_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Trend {
    /// Theil–Sen slope, native unit per week.
    pub slope_per_week: f64,
    pub sessions: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClass {
    Increased,
    Decreased,
    NoClearChange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Classification {
    pub result: ChangeClass,
    pub delta: f64,
    pub noise: f64,
    /// e.g. `coincides_with_recalibration`.
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SeriesBreak {
    /// First session after the break.
    pub session_id: SessionId,
    pub reason: String,
}

/// One comparability class; classes are never merged (C1, C2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SeriesClass {
    pub comparability_class: String,
    pub method_class: String,
    pub detector_id: String,
    pub points: Vec<SeriesPoint>,
    pub baseline: Option<Baseline>,
    pub best: Option<Best>,
    pub trend: Option<Trend>,
    pub classification: Option<Classification>,
    pub breaks: Vec<SeriesBreak>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SeriesResponse {
    pub technique: String,
    pub metric: String,
    pub side: Option<crate::Side>,
    pub unit: String,
    pub direction: String,
    pub classes: Vec<SeriesClass>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct HeadlineMetric {
    pub metric: String,
    pub comparability_class: String,
    pub latest: SeriesPoint,
    pub classification: Option<Classification>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TechniqueSummary {
    pub technique: String,
    pub last_session_id: SessionId,
    pub last_session_utc: String,
    pub sessions: u32,
    pub headline: Vec<HeadlineMetric>,
    pub new_records: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ProgressSummary {
    pub athlete_id: crate::AthleteId,
    pub techniques: Vec<TechniqueSummary>,
}
