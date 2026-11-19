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
    /// Reconstruction-time alignment of asynchronous observations.
    #[serde(default)]
    pub time_alignment: TimeAlignmentConfig,
    /// Rung-1 fusion of camera and IMU evidence (ADR-025).
    #[serde(default)]
    pub fusion: FusionConfig,
    #[serde(default)]
    pub fiducials: FiducialsConfig,
    /// Camera placement: which room's extrinsics are used at start.
    #[serde(default)]
    pub rig: RigConfig,
    /// Capture-mode fallback and rate warnings for physical cameras.
    #[serde(default)]
    pub camera_management: CameraManagementConfig,
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
    /// `diagnostics.timing`: per-stage latency histograms (15 §4.1). Schema version 2.
    #[serde(default)]
    pub diagnostics: Option<DiagnosticsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct FiducialsConfig {
    pub enabled: bool,
    pub library_path: Option<PathBuf>,
    pub family: String,
    pub max_detection_hz: f64,
    pub max_hamming: u8,
    pub min_decision_margin: f32,
    pub allowed_ids: Vec<u32>,
}
impl Default for FiducialsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            library_path: None,
            family: "tag36h11".into(),
            max_detection_hz: 15.0,
            max_hamming: 1,
            min_decision_margin: 30.0,
            allowed_ids: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RigConfig {
    /// Room whose camera extrinsics are used; unset is the default room. A room selected later
    /// in the desktop (`PUT /rig`) is remembered and takes precedence.
    pub active_room: Option<String>,
}

/// Shared camera policy. Discovery, reconnection and strict identity matching are always on.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CameraManagementConfig {
    /// Rates tried, in order, when the requested rate is not offered at the requested size.
    pub fallback_fps: Vec<f64>,
    /// A camera measured below this fraction of its requested rate is reported degraded.
    pub warn_fps_ratio: f64,
}
impl Default for CameraManagementConfig {
    fn default() -> Self {
        Self {
            fallback_fps: vec![60.0, 30.0],
            warn_fps_ratio: 0.85,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct DiagnosticsConfig {
    pub timing: bool,
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
    pub fn diagnostics(&self) -> DiagnosticsConfig {
        self.diagnostics.clone().unwrap_or_default()
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
    /// LAN mode (ADR-031, ADR-042): needed for a non-loopback bind; then every request from
    /// another machine needs the bearer token.
    pub lan: LanConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct LanConfig {
    pub enabled: bool,
    /// File holding the token; default `<data_dir>/lan-token`, created on first start.
    pub token_file: Option<PathBuf>,
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
            lan: LanConfig::default(),
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
    SlimevrSolarxr(SlimevrSourceConfig),
    Esp32Udp(Esp32SourceConfig),
    Camera(CameraSourceConfig),
}

/// SlimeVR Server's SolarXR WebSocket (`slimevr_solarxr`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SlimevrSourceConfig {
    pub id: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// SlimeVR Server's WebSocket API (`WebsocketAPI.java`: port 21110).
    #[serde(default = "slimevr_endpoint_default")]
    pub endpoint: String,
    /// `raw` (default): the sensor's fused rotation, calibrated by Bansayo. `reference_adjusted`:
    /// SlimeVR's own mounting and reset applied.
    #[serde(default)]
    pub rotation: crate::sources::slimevr::solarxr::RotationField,
    /// Least time between feed snapshots the server sends, milliseconds.
    #[serde(default = "slimevr_interval_default")]
    pub feed_interval_ms: u16,
    /// Sensor-to-arrival delay subtracted from arrival time, milliseconds. SolarXR carries no
    /// sample timestamps.
    #[serde(default)]
    pub latency_ms: f64,
    /// Trackers with Bansayo device ids, matched by `<hardware identifier>/<tracker number>` as
    /// the engine log prints it (`SlimeVR tracker announced`).
    #[serde(default)]
    pub trackers: Vec<SlimevrTrackerConfig>,
    /// Trackers not listed are announced as `SVR-<identifier>-<n>` for assignment in the UI.
    #[serde(default = "enabled_default")]
    pub announce_unlisted: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SlimevrTrackerConfig {
    pub id: String,
    pub tracker: String,
}

fn slimevr_endpoint_default() -> String {
    "ws://127.0.0.1:21110".into()
}

fn slimevr_interval_default() -> u16 {
    5
}

/// ESP32 trackers sending `bansayo.imu.v1` datagrams (`sources/esp32.rs`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32SourceConfig {
    pub id: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// UDP address to listen on. Trackers on Wi-Fi need a LAN address; only listed boards are
    /// accepted.
    #[serde(default = "esp32_bind_default")]
    pub bind: String,
    /// Nominal sample rate per tracker, Hz.
    #[serde(default = "esp32_rate_default")]
    pub rate_hz: f64,
    /// Fixed sample-to-arrival delay subtracted after offset estimation, milliseconds.
    #[serde(default)]
    pub latency_ms: f64,
    /// Device id and `<MAC>/<sensor>` as the engine log prints it.
    pub trackers: Vec<SlimevrTrackerConfig>,
}

fn esp32_bind_default() -> String {
    "0.0.0.0:7710".into()
}

fn esp32_rate_default() -> f64 {
    100.0
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
    /// System camera index; `bansayo-engine --list-cameras` prints them. Legacy: an index changes
    /// with hotplug order. Prefer `hardware_id`, `serial` or `interface_id`.
    #[serde(default)]
    pub device: Option<u32>,
    /// Substring of the device interface path, e.g. `vid_0c45&pid_6369`.
    #[serde(default)]
    pub hardware_id: Option<String>,
    /// USB serial number, when the camera has one (`--list-cameras` shows it).
    #[serde(default)]
    pub serial: Option<String>,
    /// Full device interface path: tells identical cameras without serials apart (per USB port).
    #[serde(default)]
    pub interface_id: Option<String>,
    /// Capture preference; sets `resolution_px` and `rate_hz` when they are not given.
    #[serde(default)]
    pub preset: Option<crate::sources::camera::modes::Preset>,
    /// `front`, `left`, `right`, `rear` or a custom label.
    #[serde(default)]
    pub role: Option<String>,
    /// A folder of images played as this camera, for testing and offline footage.
    #[serde(default)]
    pub frames_dir: Option<PathBuf>,
    /// A phone or other network client sends this camera's JPEG frames to
    /// `/api/v1/ingest/camera/{id}` (ADR-042). Exposure is set on the phone.
    #[serde(default)]
    pub network: bool,
    /// An IP camera's RTSP stream, e.g. `rtsp://user:password@192.168.1.40:554/h264Preview_01_main`.
    /// Read through `ffmpeg` (on PATH), scaled to `resolution_px`. Keep this in a git-ignored
    /// local configuration: it holds the camera's password.
    #[serde(default)]
    pub rtsp: Option<String>,
    #[serde(default)]
    pub rate_hz: Option<f64>,
    /// Requested capture size; the camera may negotiate another.
    #[serde(default)]
    pub resolution_px: Option<[u32; 2]>,
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
    #[serde(default)]
    pub fiducials: bool,
}

/// Without a preset or explicit values: 1280x720 at 30 fps.
pub const CAMERA_RATE_DEFAULT: f64 = 30.0;
pub const CAMERA_RESOLUTION_DEFAULT: [u32; 2] = [1280, 720];

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
    /// Which evidence the solver uses; sources keep running either way.
    pub mode: TrackingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingMode {
    /// Cameras and IMUs, each segment from the best fresh evidence.
    #[default]
    HybridAuto,
    CameraOnly,
    ImuOnly,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TimeAlignmentConfig {
    /// Orientation samples further apart than this are not interpolated between.
    pub max_imu_gap_ms: u64,
    /// A camera root (anchor) older than this, relative to the reconstruction instant, places
    /// nothing; until then its uncertainty grows with its age.
    pub max_optical_anchor_age_ms: u64,
}
impl Default for TimeAlignmentConfig {
    fn default() -> Self {
        Self {
            max_imu_gap_ms: 50,
            max_optical_anchor_age_ms: 150,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct FusionConfig {
    /// Slow per-tracker heading correction from camera-observed segment directions.
    pub optical_heading_correction: bool,
    /// Largest heading change the correction applies, degrees per second.
    pub max_heading_correction_deg_s: f64,
    /// Time constant of the correction, seconds.
    pub heading_time_constant_s: f64,
    /// After convergence, a disagreement larger than this is an outlier, not a correction.
    pub heading_outlier_deg: f64,
    /// A segment tilted less than this from vertical carries no heading information.
    pub heading_min_tilt_deg: f64,
    /// Camera segments below this fit confidence do not correct heading.
    pub min_optical_confidence: f32,
    /// Worst plausible root speed, m/s: a held anchor's uncertainty grows by this times its age.
    pub anchor_speed_bound_m_s: f64,
    /// An IMU segment and a confident camera segment differing by more than this are reported.
    pub imu_camera_disagree_deg: f64,
}
impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            optical_heading_correction: true,
            max_heading_correction_deg_s: 15.0,
            heading_time_constant_s: 1.0,
            heading_outlier_deg: 30.0,
            heading_min_tilt_deg: 20.0,
            min_optical_confidence: 0.5,
            anchor_speed_bound_m_s: 3.0,
            imu_camera_disagree_deg: 45.0,
        }
    }
}

fn simulated_schema_default() -> String {
    crate::skeleton::proportions::LANDMARK_SCHEMA_ID.into()
}
