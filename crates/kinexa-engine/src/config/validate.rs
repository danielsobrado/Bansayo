//! Semantic validation. Every problem is collected with its path; nothing stops at the first.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use kinexa_contracts::{DeviceId, SegmentId, SourceId};

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
    pub id: SourceId,
    pub seed: u64,
    pub motion: Motion,
    pub speed: f64,
    pub trackers: Vec<TrackerSpec>,
    pub cameras: Vec<CameraSpec>,
    pub scenario: Vec<ScenarioStep>,
    pub scenario_period_s: Option<f64>,
}

/// A configuration that passed validation, with typed identifiers.
#[derive(Debug, Clone)]
pub struct Validated {
    pub config: Config,
    pub bind: SocketAddr,
    pub assignments: BTreeMap<DeviceId, SegmentId>,
    pub simulated: Vec<SimulatedSpec>,
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

pub(super) fn validate(config: Config) -> Result<Validated, Vec<Problem>> {
    let mut c = Check {
        problems: vec![],
        warnings: vec![],
    };
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        c.err(
            "schema_version",
            format!(
                "{} is not supported by this build (supports {SUPPORTED_SCHEMA_VERSION})",
                config.schema_version
            ),
        );
    }
    check_engine(&mut c, &config.engine);
    check_quality(&mut c, &config.quality);
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
    let (simulated, devices) = check_sources(&mut c, &config.sources);
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
            warnings: c.warnings,
        })
    } else {
        Err(c.problems)
    }
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
    for index in 0..64 {
        let probe = dir.join(format!(
            ".kinexa-write-probe-{}-{index}",
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

fn check_api(c: &mut Check, api: &ApiConfig) -> Option<SocketAddr> {
    match api.bind.parse::<SocketAddr>() {
        Ok(addr) => {
            if !addr.ip().is_loopback() {
                c.warn(
                    "api.bind",
                    format!("{addr} exposes the engine to the network"),
                );
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

/// Validates every source; returns the enabled simulated sources and every device id with its
/// kind (`true` for orientation trackers).
fn check_sources(
    c: &mut Check,
    sources: &[SourceConfig],
) -> (Vec<SimulatedSpec>, HashMap<String, bool>) {
    let mut source_ids: HashMap<String, usize> = HashMap::new();
    let mut devices: HashMap<String, (String, bool)> = HashMap::new();
    let mut out = vec![];
    for (i, source) in sources.iter().enumerate() {
        let path = format!("sources[{i}]");
        let id = match source {
            SourceConfig::Simulated(s) => &s.id,
            SourceConfig::SlimevrSolarxr(d)
            | SourceConfig::Esp32Udp(d)
            | SourceConfig::Camera(d) => &d.id,
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
            _ => c.err(format!("{path}.kind"), "not implemented in this build"),
        }
    }
    (
        out,
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
    Some(SimulatedSpec {
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
                format!("{segment:?} is not a segment of kinexa.body.v1"),
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
schema_version: 2
engine: {{ publish_rate_hz: 0, data_dir: {dir:?} }}
api: {{ bind: "nowhere" }}
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
        assert!(has("schema_version", "2 is not supported"), "{p:#?}");
        assert!(
            has("engine.publish_rate_hz", "between 1 and 1000"),
            "{p:#?}"
        );
        assert!(has("api.bind", "not an address"), "{p:#?}");
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
}
