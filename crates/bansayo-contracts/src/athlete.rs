//! Athlete profile (martial-arts 12 §1): only what training needs.
use crate::{AthleteId, CapabilityProfileId, CoachingLevel, RecordingFlags, Stance};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

pub const ATHLETE_PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DominantSide {
    Left,
    Right,
    Ambidextrous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Experience {
    #[default]
    Beginner,
    Intermediate,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteDiscipline {
    pub id: String,
    pub style: Option<String>,
    pub experience: Experience,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteGoal {
    pub id: String,
    pub text: String,
    pub metric: Option<String>,
    pub technique: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteCoaching {
    pub level: CoachingLevel,
    /// 1.5 | 1.0 | 0.75 by experience unless edited.
    pub tolerance_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteTracking {
    pub profile_override: Option<CapabilityProfileId>,
    pub record_defaults: RecordingFlags,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct Consent {
    /// Nothing about this athlete's motion is stored until this is set.
    pub motion_data_utc: Option<String>,
    pub video_utc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct RetentionDays {
    pub inputs_days: u32,
    pub motion_days: u32,
    pub video_days: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteProfile {
    pub schema_version: u32,
    pub id: AthleteId,
    /// Free text, at most 64 characters.
    pub display_name: String,
    pub created_utc: String,
    /// Drives skeleton scaling.
    pub height_m: f64,
    pub dominant_side: Option<DominantSide>,
    pub default_stance: Option<Stance>,
    pub disciplines: Vec<AthleteDiscipline>,
    pub preferred_discipline: Option<String>,
    pub goals: Vec<AthleteGoal>,
    pub coaching: AthleteCoaching,
    pub enabled_techniques: Vec<String>,
    pub tracking: AthleteTracking,
    pub consent: Consent,
    /// `null` = installation defaults.
    pub retention: Option<RetentionDays>,
    /// ContentRef → UTC of acknowledgement.
    pub safety_ack: BTreeMap<String, String>,
}

/// `POST /athletes`: identity and height are required; the rest take defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CreateAthleteRequest {
    pub id: AthleteId,
    pub display_name: String,
    pub height_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dominant_side: Option<DominantSide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default_stance: Option<Stance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub disciplines: Option<Vec<AthleteDiscipline>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preferred_discipline: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ConsentRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub motion_data: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub video: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ActiveAthleteRequest {
    pub athlete_id: Option<AthleteId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AthleteRef {
    pub id: AthleteId,
    pub display_name: String,
}
