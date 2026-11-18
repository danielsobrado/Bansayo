//! Semantic validation. Every problem is collected with its path; nothing stops at the first.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use bansayo_contracts::{DeviceId, SegmentId, SourceId};

use super::schema::*;
use crate::skeleton::Skeleton;
use crate::sources::simulated::motions::Motion;

/// One configuration problem, located by its path in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// A simulated tracker with validated identity.
#[derive(Debug, Clone)]
pub struct TrackerSpec {
    pub id: DeviceId,
    pub rate_hz: f64,
    pub noise_deg: f64,
    pub heading_offset_deg: f64,
}

/// A simulated camera with validated identity.
#[derive(Debug, Clone)]
pub struct CameraSpec {
    pub id: DeviceId,
    pub rate_hz: f64,
    pub resolution_px: [u32; 2],
    pub fov_y_deg: f64,
    pub position_m: [f64; 3],
    pub look_at_m: [f64; 3],
    pub noise_px: f64,
}

/// An enabled simulated source with validated identities.
#[derive(Debug, Clone)]
pub struct SimulatedSpec {
    pub landmark_schema: String,
    pub id: SourceId,
    pub seed: u64,
    pub motion: Motion,
    pub speed: f64,
    pub trackers: Vec<TrackerSpec>,
    pub cameras: Vec<CameraSpec>,
    pub scenario: Vec<ScenarioStep>,
    pub scenario_period_s: Option<f64>,
}

/// A physical camera (or image folder) with validated identity.
#[derive(Debug, Clone)]
pub struct LiveCameraSpec {
    pub id: DeviceId,
    pub backend: crate::sources::camera::capture::Backend,
    pub rate_hz: f64,
    pub resolution_px: [u32; 2],
    pub latency_us: u64,
    pub exposure_s: Option<f64>,
    pub gain: Option<f64>,
    pub fiducials: bool,
}

/// An enabled camera source with validated identities and present model files.
#[derive(Debug, Clone)]
pub struct CameraSourceSpec {
    pub id: SourceId,
    pub models_dir: PathBuf,
    pub execution: crate::sources::camera::pose::Execution,
    pub min_person_score: f32,
    pub min_keypoint_score: f32,
    pub fiducials: FiducialsConfig,
    pub cameras: Vec<LiveCameraSpec>,
}

/// Most cameras one source accepts; inference is shared across them.
pub const MAX_CAMERAS: usize = 8;

/// A configuration that passed validation, with typed identifiers.
#[derive(Debug, Clone)]
pub struct Validated {
    pub config: Config,
    pub bind: SocketAddr,
    pub assignments: BTreeMap<DeviceId, SegmentId>,
    pub simulated: Vec<SimulatedSpec>,
    pub cameras: Vec<CameraSourceSpec>,
    /// Non-fatal findings, logged at startup.
    pub warnings: Vec<Problem>,
}

/// Maximum simulator speed multiplier.
pub const MAX_SIM_SPEED: f64 = 10.0;

struct Check {
    problems: Vec<Problem>,
    warnings: Vec<Problem>,
}

impl Check {
    fn err(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem {
            path: path.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Problem {
            path: path.into(),
            message: message.into(),
        });
    }

    fn range(&mut self, path: &str, value: f64, min: f64, max: f64) {
        if !(value.is_finite() && value >= min && value <= max) {
            self.err(
                path,
                format!("must be between {min} and {max}, got {value}"),
            );
        }
    }

    fn positive(&mut self, path: &str, value: f64, max: f64) {
        if !(value.is_finite() && value > 0.0 && value <= max) {
            self.err(
                path,
                format!("must be greater than 0 and at most {max}, got {value}"),
            );
        }
    }
}

fn check_training_sections(c: &mut Check, config: &Config) {
    let v2 = [
        ("content", config.content.is_some()),
        ("analysis", config.analysis.is_some()),
        ("recognition", config.recognition.is_some()),
        ("training", config.training.is_some()),
        ("coaching", config.coaching.is_some()),
        ("progress", config.progress.is_some()),
        ("storage", config.storage.is_some()),
        ("diagnostics", config.diagnostics.is_some()),
    ];
    if config.schema_version < 2 {
        for (key, present) in v2 {
            if present {
                c.err(key, "needs schema_version: 2");
            }
        }
        for (i, s) in config.sources.iter().enumerate() {
            if let SourceConfig::Simulated(s) = s
                && s.detector_id.is_some()
            {
                c.err(
                    format!("sources[{i}].detector_id"),
                    "needs schema_version: 2",
                );
            }
        }
    }
    let a = config.analysis();
    c.positive("analysis.filters.position_hz", a.filters.position_hz, 100.0);
    c.positive("analysis.filters.angle_hz", a.filters.angle_hz, 100.0);
    if a.planar_camera != "auto" {
        let known = config.sources.iter().any(|s| match s {
            SourceConfig::Camera(k) => k.cameras.iter().any(|x| x.id == a.planar_camera),
            SourceConfig::Simulated(s) => s.cameras.iter().any(|x| x.id == a.planar_camera),
            _ => false,
        });
        if !known {
            c.err(
                "analysis.planar_camera",
                format!(
                    "{:?} is neither auto nor a configured camera",
                    a.planar_camera
                ),
            );
        }
    }
    let r = config.recognition();
    c.range(
        "recognition.accept_confidence.free",
        r.accept_confidence.free,
        0.0,
        1.0,
    );
    c.range(
        "recognition.accept_confidence.guided",
        r.accept_confidence.guided,
        0.0,
        1.0,
    );
    c.range("recognition.ambiguity_margin", r.ambiguity_margin, 0.0, 1.0);
    let s = &r.segmenter;
    c.positive("recognition.segmenter.start_above", s.start_above, 100.0);
    c.positive("recognition.segmenter.end_below", s.end_below, 100.0);
    if s.end_below > s.start_above {
        c.err(
            "recognition.segmenter.end_below",
            "must not exceed start_above",
        );
    }
    if s.max_ms <= s.min_ms {
        c.err("recognition.segmenter.max_ms", "must exceed min_ms");
    }
    let t = config.training();
    c.range("training.countdown_s", t.countdown_s, 0.0, 60.0);
    c.positive(
        "training.limits.session_minutes",
        f64::from(t.limits.session_minutes),
        1440.0,
    );
    c.positive(
        "training.limits.strikes_per_session",
        f64::from(t.limits.strikes_per_session),
        100_000.0,
    );
    let k = config.coaching();
    for (key, v) in [
        (
            "coaching.tolerance_scale.beginner",
            k.tolerance_scale.beginner,
        ),
        (
            "coaching.tolerance_scale.intermediate",
            k.tolerance_scale.intermediate,
        ),
        (
            "coaching.tolerance_scale.advanced",
            k.tolerance_scale.advanced,
        ),
    ] {
        c.positive(key, v, 10.0);
    }
    let p = config.progress();
    c.positive(
        "progress.baseline_sessions",
        f64::from(p.baseline_sessions),
        100.0,
    );
    c.positive(
        "progress.baseline_window_days",
        p.baseline_window_days,
        3650.0,
    );
    if p.trend_sessions < 4 {
        c.err("progress.trend_sessions", "must be at least 4");
    }
    for (i, d) in config.content().dirs.iter().enumerate() {
        if d.as_os_str().is_empty() {
            c.err(format!("content.dirs[{i}]"), "must not be empty");
        }
    }
}

pub(super) fn validate(config: Config) -> Result<Validated, Vec<Problem>> {
    let mut c = Check {
        problems: vec![],
        warnings: vec![],
    };
    if !(MIN_SCHEMA_VERSION..=SUPPORTED_SCHEMA_VERSION).contains(&config.schema_version) {
        c.err(
            "schema_version",
            format!(
                "{} is not supported by this build (supports {MIN_SCHEMA_VERSION} and {SUPPORTED_SCHEMA_VERSION})",
                config.schema_version
            ),
        );
    }
    check_training_sections(&mut c, &config);
    if !config.evidence.detector_noise_floor_px.is_finite()
        || config.evidence.detector_noise_floor_px <= 0.0
    {
        c.err(
            "evidence.detector_noise_floor_px",
            "must be finite and positive",
        );
    }
    check_engine(&mut c, &config.engine);
    check_quality(&mut c, &config.quality);
    check_triangulation(&mut c, &config.triangulation);
    check_fiducials(&mut c, &config.fiducials, &config.sources);
    let bind = check_api(&mut c, &config.api);
    let skeleton = Skeleton::builtin(
        &config.skeleton.model,
        config.skeleton.height_m.clamp(0.5, 2.5),
    );
    if let Err(e) = &skeleton {
        c.err("skeleton.model", e.to_string());
    }
    c.range("skeleton.height_m", config.skeleton.height_m, 0.5, 2.5);
    if config.processing_profiles.is_some() {
        c.err("processing_profiles", "not implemented in this build");
    }
    let (simulated, mut cameras, devices) = check_sources(&mut c, &config.sources);
    for source in &mut cameras { source.fiducials = config.fiducials.clone(); }
    let assignments = check_assignments(
        &mut c,
        &config.assignments,
        skeleton.ok().as_ref(),
        &devices,
    );
    if c.problems.is_empty() {
        Ok(Validated {
            config,
            bind: bind.unwrap_or_else(|| ([127, 0, 0, 1], 0).into()),
            assignments,
            simulated,
            cameras,
            warnings: c.warnings,
        })
    } else {
        Err(c.problems)
    }
}

fn check_fiducials(c: &mut Check, f: &FiducialsConfig, sources: &[SourceConfig]) {
    if !["tag36h11", "tag25h9", "tagStandard41h12", "tagCircle49h12"].contains(&f.family.as_str()) {
        c.err("fiducials.family", "unsupported AprilTag family");
    }
    c.range("fiducials.max_detection_hz", f.max_detection_hz, 1.0, 240.0);
    c.range("fiducials.min_decision_margin", f.min_decision_margin.into(), 0.0, 255.0);
    if f.max_hamming > 2 { c.err("fiducials.max_hamming", "must be 0..2"); }
    if f.allowed_ids.len() > 4096 { c.err("fiducials.allowed_ids", "too many marker ids"); }
    let mut seen = std::collections::BTreeSet::new();
    for id in &f.allowed_ids {
        if !seen.insert(id) { c.err("fiducials.allowed_ids", format!("duplicate id {id}")); }
    }
    let selected = sources.iter().any(|s| matches!(s, SourceConfig::Camera(k) if k.enabled && k.cameras.iter().any(|cam| cam.fiducials)));
    if selected && !f.enabled { c.err("fiducials.enabled", "enable the global detector or disable camera fiducials"); }
    if f.enabled && !selected { c.err("fiducials.enabled", "no enabled camera requested AprilTag detection"); }
}

fn check_engine(c: &mut Check, e: &EngineConfig) {
    c.range("engine.publish_rate_hz", e.publish_rate_hz, 1.0, 1000.0);
    c.range(
        "engine.stale_after_ms",
        e.stale_after_ms as f64,
        1.0,
        60_000.0,
    );
    if let Err(msg) = check_data_dir(&e.data_dir) {
        c.err(
            "engine.data_dir",
            format!("{} is not writable: {msg}", e.data_dir.display()),
        );
    }
}

fn check_data_dir(dir: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Unique per call: on Windows, creating a name another thread is deleting fails with
    // "Access is denied" rather than `AlreadyExists`.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    for _ in 0..64 {
        let index = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let probe = dir.join(format!(
            ".bansayo-write-probe-{}-{index}",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(file) => {
                drop(file);
                return std::fs::remove_file(probe).map_err(|e| e.to_string());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("could not allocate a validation probe".into())
}

fn check_quality(c: &mut Check, q: &QualityConfig) {
    c.positive(
        "quality.degraded_rate_fraction",
        q.degraded_rate_fraction,
        1.0,
    );
    c.range("quality.degraded_quality", q.degraded_quality, 0.0, 1.0);
    c.range(
        "quality.disconnect_after_ms",
        q.disconnect_after_ms as f64,
        1.0,
        60_000.0,
    );
}

fn check_triangulation(c: &mut Check, t: &TriangulationConfig) {
    c.positive("triangulation.max_skew_ms", t.max_skew_ms as f64, 1000.0);
    c.positive(
        "triangulation.max_reprojection_error_px",
        t.max_reprojection_error_px,
        1000.0,
    );
    c.range("triangulation.min_views", t.min_views.into(), 2.0, 64.0);
    c.positive("triangulation.min_ray_angle_deg", t.min_ray_angle_deg, 90.0);
}

/// The same bounds, for configurations restored from recordings.
pub fn triangulation_valid(t: &TriangulationConfig) -> bool {
    let mut c = Check {
        problems: vec![],
        warnings: vec![],
    };
    check_triangulation(&mut c, t);
    c.problems.is_empty()
}

fn check_api(c: &mut Check, api: &ApiConfig) -> Option<SocketAddr> {
    match api.bind.parse::<SocketAddr>() {
        Ok(addr) => {
            if !addr.ip().is_loopback() {
                if api.lan.enabled {
                    c.warn(
                        "api.bind",
                        format!(
                            "{addr}: LAN mode; other machines need the pairing token; no TLS, use on a trusted home network only"
                        ),
                    );
                } else {
                    c.err(
                        "api.bind",
                        format!(
                            "{addr} is not a loopback address; set api.lan.enabled: true to serve the network with a token (ADR-031)"
                        ),
                    );
                }
            }
            Some(addr)
        }
        Err(_) => {
            c.err(
                "api.bind",
                format!("{:?} is not an address such as 127.0.0.1:7700", api.bind),
            );
            None
        }
    }
}

/// Validates every source; returns the enabled simulated and camera sources and every device id
/// with its kind (`true` for orientation trackers).
fn check_sources(
    c: &mut Check,
    sources: &[SourceConfig],
) -> (
    Vec<SimulatedSpec>,
    Vec<CameraSourceSpec>,
    HashMap<String, bool>,
) {
    let mut source_ids: HashMap<String, usize> = HashMap::new();
    let mut devices: HashMap<String, (String, bool)> = HashMap::new();
    let mut out = vec![];
    let mut cameras = vec![];
    for (i, source) in sources.iter().enumerate() {
        let path = format!("sources[{i}]");
        let id = match source {
            SourceConfig::Simulated(s) => &s.id,
            SourceConfig::Camera(k) => &k.id,
            SourceConfig::SlimevrSolarxr(d) | SourceConfig::Esp32Udp(d) => &d.id,
        };
        if SourceId::new(id.as_str()).is_err() {
            c.err(
                format!("{path}.id"),
                format!("{id:?} is not a valid source id (A-Z a-z 0-9 _ . -)"),
            );
        }
        if let Some(prev) = source_ids.insert(id.clone(), i) {
            c.err(
                format!("{path}.id"),
                format!("{id:?} is already used by sources[{prev}]"),
            );
        }
        match source {
            SourceConfig::Simulated(s) => {
                if let Some(spec) = check_simulated(c, &path, s, &mut devices) {
                    out.extend(s.enabled.then_some(spec));
                }
            }
            SourceConfig::Camera(k) => {
                if let Some(spec) = check_camera_source(c, &path, k, &mut devices) {
                    cameras.extend(k.enabled.then_some(spec));
                }
            }
            _ => c.err(format!("{path}.kind"), "not implemented in this build"),
        }
    }
    (
        out,
        cameras,
        devices
            .into_iter()
            .map(|(k, (_, tracker))| (k, tracker))
            .collect(),
    )
}

fn claim_device(
    c: &mut Check,
    path: &str,
    id: &str,
    what: &str,
    tracker: bool,
    devices: &mut HashMap<String, (String, bool)>,
) -> Option<DeviceId> {
    let parsed = DeviceId::new(id);
    if parsed.is_err() {
        c.err(
            path,
            format!("{id:?} is not a valid device id (A-Z a-z 0-9 _ . -, at most 64)"),
        );
    }
    if let Some((prev, prev_tracker)) = devices.get(id) {
        let kind = if *prev_tracker { "tracker" } else { "camera" };
        c.err(
            path,
            format!("{id:?} is already used by a {kind} at {prev}"),
        );
    } else {
        devices.insert(id.to_owned(), (format!("{path} ({what})"), tracker));
    }
    parsed.ok()
}

fn check_simulated(
    c: &mut Check,
    path: &str,
    s: &SimulatedSourceConfig,
    devices: &mut HashMap<String, (String, bool)>,
) -> Option<SimulatedSpec> {
    let motion = Motion::from_id(&s.motion);
    if motion.is_none() {
        let ids: Vec<&str> = Motion::ALL.iter().map(|m| m.id()).collect();
        c.err(
            format!("{path}.motion"),
            format!("{:?} is not one of {}", s.motion, ids.join(", ")),
        );
    }
    c.positive(&format!("{path}.speed"), s.speed, MAX_SIM_SPEED);
    if let Some(period) = s.scenario_period_s {
        c.positive(&format!("{path}.scenario_period_s"), period, 86400.0);
    }
    for (i, step) in s.scenario.iter().flatten().enumerate() {
        let p = format!("{path}.scenario[{i}]");
        c.range(
            &format!("{p}.at_s"),
            step.at_s,
            0.0,
            s.scenario_period_s.unwrap_or(86400.0),
        );
        if !s.trackers.iter().any(|t| t.id == step.device)
            && !s.cameras.iter().any(|t| t.id == step.device)
        {
            c.err(format!("{p}.device"), "must name a device in this source");
        }
        if let Some(q) = step.quality {
            c.range(&format!("{p}.quality"), q as f64, 0.0, 1.0);
        }
    }
    let mut trackers = vec![];
    for (j, t) in s.trackers.iter().enumerate() {
        let p = format!("{path}.trackers[{j}]");
        c.range(&format!("{p}.rate_hz"), t.rate_hz, 1.0, 1000.0);
        c.range(&format!("{p}.noise_deg"), t.noise_deg, 0.0, 45.0);
        c.range(
            &format!("{p}.heading_offset_deg"),
            t.heading_offset_deg,
            -180.0,
            180.0,
        );
        if let Some(id) = claim_device(c, &format!("{p}.id"), &t.id, "tracker", true, devices) {
            trackers.push(TrackerSpec {
                id,
                rate_hz: t.rate_hz,
                noise_deg: t.noise_deg,
                heading_offset_deg: t.heading_offset_deg,
            });
        }
    }
    let mut cameras = vec![];
    for (j, k) in s.cameras.iter().enumerate() {
        let p = format!("{path}.cameras[{j}]");
        c.range(&format!("{p}.rate_hz"), k.rate_hz, 1.0, 240.0);
        for (axis, v) in ["width", "height"].iter().zip(k.resolution_px) {
            c.range(&format!("{p}.resolution_px.{axis}"), v.into(), 16.0, 8192.0);
        }
        c.range(&format!("{p}.fov_y_deg"), k.fov_y_deg, 1.0, 179.0);
        c.range(&format!("{p}.noise_px"), k.noise_px, 0.0, 50.0);
        let finite = k
            .position_m
            .iter()
            .chain(&k.look_at_m)
            .all(|v| v.is_finite());
        if !finite || (k.position_m[0] == k.look_at_m[0] && k.position_m[2] == k.look_at_m[2]) {
            c.err(
                format!("{p}.look_at_m"),
                "must be finite and have a nonvertical viewing direction from position_m",
            );
        }
        if let Some(id) = claim_device(c, &format!("{p}.id"), &k.id, "camera", false, devices) {
            cameras.push(CameraSpec {
                id,
                rate_hz: k.rate_hz,
                resolution_px: k.resolution_px,
                fov_y_deg: k.fov_y_deg,
                position_m: k.position_m,
                look_at_m: k.look_at_m,
                noise_px: k.noise_px,
            });
        }
    }
    if s.landmark_schema != crate::skeleton::proportions::LANDMARK_SCHEMA_ID
        && s.landmark_schema != bansayo_contracts::BODY_KEYPOINT_SCHEMA_ID
    {
        c.err(
            format!("{path}.landmark_schema"),
            "unsupported simulated landmark schema",
        );
    }
    Some(SimulatedSpec {
        landmark_schema: s.landmark_schema.clone(),
        id: SourceId::new(s.id.as_str()).ok()?,
        seed: s.seed,
        motion: motion?,
        speed: s.speed,
        trackers,
        cameras,
        scenario: s.scenario.clone().unwrap_or_default(),
        scenario_period_s: s.scenario_period_s,
    })
}

fn check_camera_source(
    c: &mut Check,
    path: &str,
    k: &CameraSourceConfig,
    devices: &mut HashMap<String, (String, bool)>,
) -> Option<CameraSourceSpec> {
    use crate::sources::camera::{capture, pose};
    // Image folders with recorded landmarks need no pose models.
    let replayed = |cam: &crate::config::LiveCameraConfig| {
        cam.frames_dir
            .as_ref()
            .is_some_and(|d| d.join(capture::RECORDED_LANDMARKS).is_file())
    };
    if k.enabled && !k.cameras.iter().all(replayed) {
        for model in [pose::DETECTOR_MODEL, pose::POSE_MODEL] {
            let file = k.models_dir.join(model);
            if !file.is_file() {
                c.err(
                    format!("{path}.models_dir"),
                    format!("{} is missing; see models/README.md", file.display()),
                );
            }
        }
    }
    c.range(
        &format!("{path}.min_person_score"),
        k.min_person_score.into(),
        0.0,
        1.0,
    );
    c.range(
        &format!("{path}.min_keypoint_score"),
        k.min_keypoint_score.into(),
        0.0,
        1.0,
    );
    if k.cameras.is_empty() || k.cameras.len() > MAX_CAMERAS {
        c.err(
            format!("{path}.cameras"),
            format!("must list 1 to {MAX_CAMERAS} cameras"),
        );
    }
    let mut indices: HashMap<u32, usize> = HashMap::new();
    let mut cameras = vec![];
    for (j, cam) in k.cameras.iter().enumerate() {
        let p = format!("{path}.cameras[{j}]");
        c.range(&format!("{p}.rate_hz"), cam.rate_hz, 1.0, 240.0);
        c.range(&format!("{p}.latency_ms"), cam.latency_ms, 0.0, 1000.0);
        for (axis, v) in ["width", "height"].iter().zip(cam.resolution_px) {
            c.range(&format!("{p}.resolution_px.{axis}"), v.into(), 16.0, 8192.0);
        }
        if let Some(e) = cam.exposure_s {
            c.range(&format!("{p}.exposure_s"), e, 1e-5, 1.0);
        }
        let mut resolution = cam.resolution_px;
        if cam.network && (cam.exposure_s.is_some() || cam.gain.is_some()) {
            c.err(
                &p,
                "exposure_s and gain of a network camera are set on the phone, not here",
            );
        }
        let backend = match (&cam.device, &cam.frames_dir) {
            (None, None) if cam.network => Some(capture::Backend::Network(cam.id.clone())),
            (Some(_), _) | (_, Some(_)) if cam.network => {
                c.err(&p, "a network camera has no device or frames_dir");
                None
            }
            (Some(index), None) => {
                if let Some(prev) = indices.insert(*index, j) {
                    c.err(
                        format!("{p}.device"),
                        format!("{index} is already used by cameras[{prev}]"),
                    );
                }
                Some(capture::Backend::Device(*index))
            }
            (None, Some(dir)) => match capture::frame_files(dir) {
                // An image folder's resolution is its first image's; every image must match.
                Ok(files) => match image::image_dimensions(&files[0]) {
                    Ok((w, h)) => {
                        resolution = [w, h];
                        Some(capture::Backend::Frames(dir.clone()))
                    }
                    Err(e) => {
                        c.err(
                            format!("{p}.frames_dir"),
                            format!("{}: {e}", files[0].display()),
                        );
                        None
                    }
                },
                Err(e) => {
                    c.err(format!("{p}.frames_dir"), e);
                    None
                }
            },
            _ => {
                c.err(
                    &p,
                    "set exactly one of device, frames_dir and network: true",
                );
                None
            }
        };
        let id = claim_device(c, &format!("{p}.id"), &cam.id, "camera", false, devices);
        if let (Some(id), Some(backend)) = (id, backend) {
            cameras.push(LiveCameraSpec {
                id,
                backend,
                rate_hz: cam.rate_hz,
                resolution_px: resolution,
                latency_us: (cam.latency_ms * 1000.0) as u64,
                exposure_s: cam.exposure_s,
                gain: cam.gain,
                fiducials: cam.fiducials,
            });
        }
    }
    Some(CameraSourceSpec {
        id: SourceId::new(k.id.as_str()).ok()?,
        models_dir: k.models_dir.clone(),
        execution: k.execution,
        min_person_score: k.min_person_score,
        min_keypoint_score: k.min_keypoint_score,
        fiducials: FiducialsConfig::default(),
        cameras,
    })
}

fn check_assignments(
    c: &mut Check,
    assignments: &BTreeMap<String, String>,
    skeleton: Option<&Skeleton>,
    devices: &HashMap<String, bool>,
) -> BTreeMap<DeviceId, SegmentId> {
    let mut out = BTreeMap::new();
    let mut taken: HashMap<&str, &str> = HashMap::new();
    for (device, segment) in assignments {
        let path = format!("assignments.{device}");
        let id = DeviceId::new(device.as_str());
        if id.is_err() {
            c.err(&path, format!("{device:?} is not a valid device id"));
        }
        let seg = SegmentId::new(segment.as_str())
            .ok()
            .filter(|s| skeleton.is_none_or(|sk| sk.index_of(s).is_some()));
        if seg.is_none() {
            c.err(
                &path,
                format!("{segment:?} is not a segment of bansayo.body.v1"),
            );
        }
        match devices.get(device) {
            None => c.warn(
                &path,
                "refers to a device that is not configured; it may appear later",
            ),
            Some(false) => c.err(
                &path,
                "is a camera; only orientation trackers can be assigned",
            ),
            Some(true) => {}
        }
        if let Some(other) = taken.insert(segment, device) {
            c.err(
                "assignments",
                format!("{other} and {device} are both assigned to {segment}"),
            );
        }
        if let (Ok(id), Some(seg)) = (id, seg) {
            out.insert(id, seg);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::config::{ConfigError, parse};

    fn problems(yaml: &str) -> Vec<String> {
        match parse(yaml) {
            Err(ConfigError::Invalid(p)) => p.iter().map(ToString::to_string).collect(),
            Err(other) => vec![format!("other: {other}")],
            Ok(_) => vec![],
        }
    }

    #[test]
    fn reports_every_problem_with_its_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let yaml = format!(
            r#"
schema_version: 3
engine: {{ publish_rate_hz: 0, data_dir: {dir:?} }}
api: {{ bind: "nowhere" }}
triangulation: {{ min_views: 1, max_reprojection_error_px: 0 }}
assignments: {{ SIM-T01: left_tigh, SIM-T02: pelvis, SIM-T03: pelvis }}
sources:
  - id: sim
    kind: simulated
    motion: dance
    trackers:
      - {{ id: SIM-T01, rate_hz: 100 }}
      - {{ id: SIM-T02, rate_hz: 100 }}
      - {{ id: SIM-T03, rate_hz: 0 }}
    cameras:
      - {{ id: SIM-T01, rate_hz: 30, resolution_px: [1280, 720], fov_y_deg: 50, position_m: [0, 1, 3], look_at_m: [0, 1, 0] }}
  - {{ id: slime, kind: slimevr_solarxr, url: "ws://127.0.0.1:21110" }}
"#,
            dir = dir.path().display().to_string()
        );
        let p = problems(&yaml);
        let has =
            |prefix: &str, text: &str| p.iter().any(|l| l.starts_with(prefix) && l.contains(text));
        assert!(has("schema_version", "3 is not supported"), "{p:#?}");
        assert!(
            has("engine.publish_rate_hz", "between 1 and 1000"),
            "{p:#?}"
        );
        assert!(has("api.bind", "not an address"), "{p:#?}");
        assert!(has("triangulation.min_views", "between 2 and 64"), "{p:#?}");
        assert!(
            has("triangulation.max_reprojection_error_px", "greater than 0"),
            "{p:#?}"
        );
        assert!(
            has("assignments.SIM-T01", "\"left_tigh\" is not a segment"),
            "{p:#?}"
        );
        assert!(
            has(
                "assignments",
                "SIM-T02 and SIM-T03 are both assigned to pelvis"
            ),
            "{p:#?}"
        );
        assert!(has("sources[0].motion", "\"dance\""), "{p:#?}");
        assert!(
            has("sources[0].trackers[2].rate_hz", "between 1 and 1000"),
            "{p:#?}"
        );
        assert!(
            has("sources[0].cameras[0].id", "already used by a tracker"),
            "{p:#?}"
        );
        assert!(
            has("sources[1].kind", "not implemented in this build"),
            "{p:#?}"
        );
        assert!(p.len() >= 9);
    }

    #[test]
    fn camera_sources_report_missing_models_and_bad_cameras() {
        let dir = tempfile::tempdir().expect("tempdir");
        let frames = dir.path().join("frames");
        std::fs::create_dir(&frames).unwrap();
        image::RgbImage::new(64, 48)
            .save(frames.join("0001.png"))
            .unwrap();
        let yaml = format!(
            r#"
schema_version: 1
engine: {{ data_dir: {dir:?} }}
sources:
  - id: cams
    kind: camera
    models_dir: {models:?}
    cameras:
      - {{ id: CAM1, device: 0 }}
      - {{ id: CAM2, device: 0 }}
      - {{ id: CAM3 }}
      - {{ id: CAM4, frames_dir: {empty:?} }}
      - {{ id: CAM5, frames_dir: {frames:?}, rate_hz: 500 }}
"#,
            dir = dir.path().display().to_string(),
            models = dir.path().join("nowhere").display().to_string(),
            empty = dir.path().display().to_string(),
            frames = frames.display().to_string(),
        );
        let p = problems(&yaml);
        let has =
            |prefix: &str, text: &str| p.iter().any(|l| l.starts_with(prefix) && l.contains(text));
        assert!(
            has("sources[0].models_dir", "rtmpose-m.onnx is missing"),
            "{p:#?}"
        );
        assert!(
            has("sources[0].cameras[1].device", "already used by cameras[0]"),
            "{p:#?}"
        );
        assert!(
            has(
                "sources[0].cameras[2]",
                "exactly one of device, frames_dir and network: true"
            ),
            "{p:#?}"
        );
        assert!(
            has("sources[0].cameras[3].frames_dir", "no .jpg or .png"),
            "{p:#?}"
        );
        assert!(
            has("sources[0].cameras[4].rate_hz", "between 1 and 240"),
            "{p:#?}"
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let yaml = "schema_version: 1\nsources:\n  - { id: sim, kind: simulated, trackers: [ { id: T1, rate: 100 } ] }\n";
        match parse(yaml) {
            Err(ConfigError::Parse(msg)) => assert!(msg.contains("rate"), "{msg}"),
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn minimal_configuration_is_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let yaml = format!(
            "schema_version: 1\nengine: {{ data_dir: {:?} }}\n",
            dir.path().display().to_string()
        );
        let v = parse(&yaml).expect("valid");
        assert!(v.simulated.is_empty() && v.assignments.is_empty());
    }

    #[test]
    fn schema_two_sections_load_and_version_one_rejects_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = |v: u32, extra: &str| {
            format!(
                "schema_version: {v}\nengine: {{ data_dir: {:?} }}\nsources:\n  - {{ id: sim, kind: simulated, trackers: [{{ id: SIM-T01, rate_hz: 100 }}] }}\n{extra}",
                dir.path().display().to_string()
            )
        };
        let v2 = "content: { dirs: [./content] }\nanalysis: { filters: { position_hz: 10 } }\nrecognition: { accept_confidence: { guided: 0.6 } }\ntraining: { countdown_s: 3 }\ncoaching: { cooldown_reps: 2 }\nprogress: { trend_sessions: 6 }\nstorage: { record_defaults: { video: false } }\n";
        let ok = parse(&base(2, v2)).expect("schema 2 loads");
        assert_eq!(ok.config.analysis().filters.position_hz, 10.0);
        assert_eq!(ok.config.recognition().accept_confidence.guided, 0.6);
        assert_eq!(ok.config.training().pause_after_tracking_loss_ms, 3000);
        assert!(parse(&base(1, "")).is_ok(), "version 1 files still load");
        let p = problems(&base(1, v2));
        for key in [
            "content",
            "analysis",
            "recognition",
            "training",
            "coaching",
            "progress",
            "storage",
        ] {
            assert!(
                p.iter()
                    .any(|l| l.starts_with(key) && l.contains("needs schema_version: 2")),
                "{key}: {p:#?}"
            );
        }
        let p = problems(&base(
            2,
            "recognition: { segmenter: { start_above: 0.2, end_below: 0.5 } }\nprogress: { trend_sessions: 2 }\n",
        ));
        assert!(
            p.iter()
                .any(|l| l.starts_with("recognition.segmenter.end_below")),
            "{p:#?}"
        );
        assert!(
            p.iter().any(|l| l.starts_with("progress.trend_sessions")),
            "{p:#?}"
        );
    }
}
