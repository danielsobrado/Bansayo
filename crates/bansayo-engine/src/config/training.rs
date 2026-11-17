//! Configuration sections introduced by `schema_version: 2` (martial-arts 13 §4). Every value that
//! affects measurement is a default pending validation.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ContentConfig {
    /// Shipped content roots; `<data_dir>/content/user` is always added.
    pub dirs: Vec<PathBuf>,
}
impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            dirs: vec![PathBuf::from("./content")],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Filters {
    pub position_hz: f64,
    pub angle_hz: f64,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            position_hz: 12.0,
            angle_hz: 12.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct AnalysisConfig {
    pub filters: Filters,
    /// `auto` or a camera id; image-plane observables arrive in roadmap Phase 2.
    pub planar_camera: String,
}
impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            filters: Filters::default(),
            planar_camera: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct AcceptConfidence {
    pub free: f64,
    pub guided: f64,
}
impl Default for AcceptConfidence {
    fn default() -> Self {
        Self {
            free: 0.75,
            guided: 0.5,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct SegmenterSection {
    pub start_above: f64,
    pub start_hold_ms: u64,
    pub end_below: f64,
    pub end_hold_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub merge_gap_ms: u64,
}
impl Default for SegmenterSection {
    fn default() -> Self {
        Self {
            start_above: 1.0,
            start_hold_ms: 40,
            end_below: 0.4,
            end_hold_ms: 150,
            min_ms: 150,
            max_ms: 4000,
            merge_gap_ms: 80,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecognitionConfig {
    pub accept_confidence: AcceptConfidence,
    #[serde(default = "margin")]
    pub ambiguity_margin: f64,
    pub segmenter: SegmenterSection,
}
fn margin() -> f64 {
    0.10
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Limits {
    pub strikes_per_session: u32,
    pub session_minutes: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            strikes_per_session: 400,
            session_minutes: 90,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TrainingSection {
    pub countdown_s: f64,
    pub pause_after_tracking_loss_ms: u64,
    pub free_set_gap_s: f64,
    pub round_gap_ms: u64,
    pub limits: Limits,
}
impl Default for TrainingSection {
    fn default() -> Self {
        Self {
            countdown_s: 5.0,
            pause_after_tracking_loss_ms: 3000,
            free_set_gap_s: 8.0,
            round_gap_ms: 800,
            limits: Limits::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ToleranceScale {
    pub beginner: f64,
    pub intermediate: f64,
    pub advanced: f64,
}
impl Default for ToleranceScale {
    fn default() -> Self {
        Self {
            beginner: 1.5,
            intermediate: 1.0,
            advanced: 0.75,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CoachingSection {
    pub cooldown_reps: u32,
    pub tolerance_scale: ToleranceScale,
}
impl Default for CoachingSection {
    fn default() -> Self {
        Self {
            cooldown_reps: 3,
            tolerance_scale: ToleranceScale::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ProgressSection {
    pub baseline_sessions: u32,
    pub baseline_window_days: f64,
    pub trend_sessions: u32,
}
impl Default for ProgressSection {
    fn default() -> Self {
        Self {
            baseline_sessions: 3,
            baseline_window_days: 30.0,
            trend_sessions: 8,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Retention {
    pub inputs_days: u32,
    pub motion_days: u32,
    pub video_days: u32,
}
impl Default for Retention {
    fn default() -> Self {
        Self {
            inputs_days: 90,
            motion_days: 365,
            video_days: 14,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecordDefaults {
    pub inputs: bool,
    pub motion: bool,
    pub video: bool,
}
impl Default for RecordDefaults {
    fn default() -> Self {
        Self {
            inputs: true,
            motion: true,
            video: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct StorageSection {
    /// Read and validated; retention jobs are not part of roadmap Phase 1.
    pub retention: Retention,
    pub record_defaults: RecordDefaults,
}
