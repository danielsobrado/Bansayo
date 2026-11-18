//! Configuration schema. Unknown fields are rejected everywhere.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The newest `schema_version` this build reads; version 1 files still load (13 §3).
pub const SUPPORTED_SCHEMA_VERSION: u32 = 2;
pub const MIN_SCHEMA_VERSION: u32 = 1;
pub use super::training::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub engine: EngineConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub skeleton: SkeletonConfig,
    /// Initial device → segment assignments. Validated into typed ids.
    #[serde(default)]
    pub assignments: BTreeMap<String, String>,
    #[serde(default)]
    pub quality: QualityConfig,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub triangulation: TriangulationConfig,
    #[serde(default)]
    pub evidence: EvidenceConfig,
    #[serde(default)]
    pub tracking: TrackingConfig,
    /// Reserved for camera processing; rejected until the camera milestone.
    #[serde(default)]
    pub processing_profiles: Option<Value>,
    // Sections added by schema_version 2; absent means defaults.
    #[serde(default)]
    pub content: Option<ContentConfig>,
    #[serde(default)]
    pub analysis: Option<AnalysisConfig>,
    #[serde(default)]
    pub recognition: Option<RecognitionConfig>,
    #[serde(default)]
    pub training: Option<TrainingSection>,
    #[serde(default)]
    pub coaching: Option<CoachingSection>,
    #[serde(default)]
    pub progress: Option<ProgressSection>,
    #[serde(default)]
    pub storage: Option<StorageSection>,
}

impl Config {
    pub fn content(&self) -> ContentConfig {
        self.content.clone().unwrap_or_default()
    }
    pub fn analysis(&self) -> AnalysisConfig {
        self.analysis.clone().unwrap_or_default()
    }
    pub fn recognition(&self) -> RecognitionConfig {
        self.recognition.clone().unwrap_or_default()
    }
    pub fn training(&self) -> TrainingSection {
        self.training.clone().unwrap_or_default()
    }
    pub fn coaching(&self) -> CoachingSection {
        self.coaching.clone().unwrap_or_default()
    }
    pub fn progress(&self) -> ProgressSection {
        self.progress.clone().unwrap_or_default()
    }
    pub fn storage(&self) -> StorageSection {
        self.storage.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct EngineConfig {
    /// Skeleton solve and publication rate.
    pub publish_rate_hz: f64,
    /// An older observation no longer drives its segment.
    pub stale_after_ms: u64,
    /// Calibration, sessions, runtime state. Relative paths are relative to the working directory.
    pub data_dir: PathBuf,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            publish_rate_hz: 60.0,
            stale_after_ms: 250,
            data_dir: PathBuf::from("./data"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ApiConfig {
    pub bind: String,
    pub allowed_origins: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:7700".into(),
            allowed_origins: [
                "http://localhost:5173",
                "http://127.0.0.1:5173",
                "tauri://localhost",
                "http://tauri.localhost",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub format: LogFormat,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct SkeletonConfig {
    pub model: String,
    pub height_m: f64,
}

impl Default for SkeletonConfig {
    fn default() -> Self {
        Self {
            model: "bansayo.body.v1".into(),
            height_m: 1.75,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct QualityConfig {
    /// Measured rate below this fraction of nominal → degraded.
    pub degraded_rate_fraction: f64,
    /// Device-reported quality below this → degraded.
    pub degraded_quality: f64,
    /// No data for this long → disconnected.
    pub disconnect_after_ms: u64,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            degraded_rate_fraction: 0.7,
            degraded_quality: 0.5,
            disconnect_after_ms: 1000,
        }
    }
}

/// Multi-camera triangulation of 2D landmarks into world points.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TriangulationConfig {
    /// Camera views older than the newest usable view by more than this are not combined.
    pub max_skew_ms: u64,
    /// Views reprojecting worse than this are rejected; a point that cannot meet it is omitted.
    pub max_reprojection_error_px: f64,
    /// Fewest camera views a landmark needs.
    pub min_views: u32,
    /// Below this largest pairwise ray angle, depth is too poorly conditioned to publish.
    pub min_ray_angle_deg: f64,
}

impl Default for TriangulationConfig {
    fn default() -> Self {
        Self {
            max_skew_ms: 20,
            max_reprojection_error_px: 15.0,
            min_views: 2,
            min_ray_angle_deg: 3.0,
        }
    }
}

/// A source. Designed kinds parse so they can be rejected with a clear message.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceConfig {
    Simulated(SimulatedSourceConfig),
    SlimevrSolarxr(DesignedSourceConfig),
    Esp32Udp(DesignedSourceConfig),
    Camera(CameraSourceConfig),
}

/// Fields of a source kind that is designed but not implemented.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DesignedSourceConfig {
    pub id: String,
    #[serde(flatten)]
    pub rest: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulatedSourceConfig {
    #[serde(default = "simulated_schema_default")]
    pub landmark_schema: String,
    pub id: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Mounting rotations and noise are derived from this.
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "motion_default")]
    pub motion: String,
    #[serde(default = "speed_default")]
    pub speed: f64,
    /// Pose-model identity of the simulated cameras, for comparability classes (schema 2).
    #[serde(default)]
    pub detector_id: Option<String>,
    #[serde(default)]
    pub trackers: Vec<SimTrackerConfig>,
    #[serde(default)]
    pub cameras: Vec<SimCameraConfig>,
    /// Scripted scenarios arrive in a later build; rejected when present.
    #[serde(default)]
    pub scenario_period_s: Option<f64>,
    #[serde(default)]
    pub scenario: Option<Vec<ScenarioStep>>,
}

fn enabled_default() -> bool {
    true
}

fn motion_default() -> String {
    "standing".into()
}

fn speed_default() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimTrackerConfig {
    pub id: String,
    pub rate_hz: f64,
    #[serde(default)]
    pub noise_deg: f64,
    #[serde(default)]
    pub heading_offset_deg: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimCameraConfig {
    pub id: String,
    pub rate_hz: f64,
    pub resolution_px: [u32; 2],
    pub fov_y_deg: f64,
    pub position_m: [f64; 3],
    pub look_at_m: [f64; 3],
    #[serde(default)]
    pub noise_px: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioStep {
    pub at_s: f64,
    pub device: String,
    pub action: bansayo_contracts::FaultAction,
    pub quality: Option<f32>,
}

/// Physical cameras (or image folders) with 2D pose inference.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSourceConfig {
    pub id: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Holds the ONNX models and `onnxruntime/`; see models/README.md.
    #[serde(default = "models_dir_default")]
    pub models_dir: PathBuf,
    #[serde(default)]
    pub execution: crate::sources::camera::pose::Execution,
    #[serde(default = "min_person_score_default")]
    pub min_person_score: f32,
    /// Keypoints below this are reported as not detected.
    #[serde(default = "min_keypoint_score_default")]
    pub min_keypoint_score: f32,
    pub cameras: Vec<LiveCameraConfig>,
}

fn models_dir_default() -> PathBuf {
    PathBuf::from("./models")
}

fn min_person_score_default() -> f32 {
    0.5
}

fn min_keypoint_score_default() -> f32 {
    0.3
}

/// One camera. Exactly one of `device` and `frames_dir`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCameraConfig {
    pub id: String,
    /// System camera index; `bansayo-engine --list-cameras` prints them.
    #[serde(default)]
    pub device: Option<u32>,
    /// A folder of images played as this camera, for testing and offline footage.
    #[serde(default)]
    pub frames_dir: Option<PathBuf>,
    #[serde(default = "camera_rate_default")]
    pub rate_hz: f64,
    /// Requested capture size; the camera may negotiate another.
    #[serde(default = "camera_resolution_default")]
    pub resolution_px: [u32; 2],
    /// Capture-to-arrival latency subtracted from arrival time, milliseconds.
    #[serde(default)]
    pub latency_ms: f64,
    /// Manual exposure, seconds (08 §3.2); automatic exposure is switched off. Unset on a
    /// physical camera: the camera's automatic exposure, reported `degraded`.
    #[serde(default)]
    pub exposure_s: Option<f64>,
    /// Manual gain in the device's own units; unset leaves the camera's setting.
    #[serde(default)]
    pub gain: Option<f64>,
}

fn camera_rate_default() -> f64 {
    30.0
}

fn camera_resolution_default() -> [u32; 2] {
    [1280, 720]
}

/// Propagated uncertainty defaults; no validated accuracy tables ship.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct EvidenceConfig {
    pub detector_noise_floor_px: f64,
}
impl Default for EvidenceConfig {
    fn default() -> Self {
        Self {
            detector_noise_floor_px: 2.0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TrackingConfig {
    pub profile_override: Option<bansayo_contracts::CapabilityProfileId>,
}

fn simulated_schema_default() -> String {
    crate::skeleton::proportions::LANDMARK_SCHEMA_ID.into()
}
