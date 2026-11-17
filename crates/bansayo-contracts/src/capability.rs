//! Observability facts and their derived display profile (martial-arts 08 §9).
use crate::{FeatureState, MethodId, Reason};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityProfileId {
    None,
    Basic,
    Inertial,
    MultiView,
    MarkerAssisted,
    Hybrid,
    Extended,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum InputClass {
    P,
    A,
    R,
    W,
    I,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct InputAvailability {
    pub class: InputClass,
    pub target: String,
    pub state: FeatureState,
    pub methods: Vec<MethodId>,
    pub rate_hz: Option<f32>,
    pub reasons: Vec<Reason>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ObservableAvailability {
    pub id: String,
    pub state: FeatureState,
    pub method: Option<MethodId>,
    pub reasons: Vec<Reason>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct CapabilityReport {
    pub profile: CapabilityProfileId,
    pub profile_override: Option<CapabilityProfileId>,
    pub tracking_config_id: String,
    pub calibration_set_id: String,
    pub inputs: Vec<InputAvailability>,
    pub observables: Vec<ObservableAvailability>,
}
