//! Atomic runtime state persistence (`state/runtime.yaml`) and legacy state migration.
use crate::{
    calibration::Store, skeleton::Skeleton, sources::simulated::camera::DECLARED_BY_SIMULATOR,
};
use bansayo_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

pub const RUNTIME_SCHEMA_VERSION: u32 = 2;
const RUNTIME_SNAPSHOT_REL: &str = "state/runtime.yaml";
const LEGACY_ASSIGNMENTS_REL: &str = "state/assignments.yaml";
const LEGACY_STATES_REL: &str = "state/calibration-states.json";

static TEMP: AtomicU64 = AtomicU64::new(0);
static FAIL_NEXT_SAVE: OnceLock<Mutex<BTreeMap<PathBuf, u32>>> = OnceLock::new();

#[doc(hidden)]
pub fn inject_persist_failure(data_dir: &Path) {
    let map = FAIL_NEXT_SAVE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    *guard.entry(data_dir.to_path_buf()).or_insert(0) += 1;
}

fn take_injected_failure(data_dir: &Path) -> bool {
    let Some(map) = FAIL_NEXT_SAVE.get() else {
        return false;
    };
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    match guard.get_mut(data_dir) {
        Some(count) if *count > 1 => {
            *count -= 1;
            true
        }
        Some(_) => {
            guard.remove(data_dir);
            true
        }
        None => false,
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSnapshot {
    pub schema_version: u32,
    #[serde(default)]
    pub assignments: BTreeMap<DeviceId, Option<SegmentId>>,
    #[serde(default)]
    pub stale_calibrations: BTreeSet<ProfileId>,
    #[serde(default)]
    pub calibration_profiles: Vec<CalibrationProfile>,
}

impl RuntimeSnapshot {
    pub fn from_state(
        assignments: &BTreeMap<DeviceId, SegmentId>,
        known: impl Iterator<Item = DeviceId>,
        stale: &BTreeSet<ProfileId>,
        profiles: &BTreeMap<ProfileId, CalibrationProfile>,
    ) -> Self {
        let overrides: BTreeMap<DeviceId, Option<SegmentId>> = known
            .map(|id| {
                let value = assignments.get(&id).cloned();
                (id, value)
            })
            .chain(
                assignments
                    .iter()
                    .map(|(id, s)| (id.clone(), Some(s.clone()))),
            )
            .collect();
        let calibration_profiles = profiles
            .values()
            .filter(|p| p.method != DECLARED_BY_SIMULATOR)
            .cloned()
            .collect();
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            assignments: overrides,
            stale_calibrations: stale.clone(),
            calibration_profiles,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationStates {
    pub schema_version: u32,
    pub stale: BTreeSet<ProfileId>,
}

pub fn runtime_snapshot_path(dir: &Path) -> PathBuf {
    dir.join(RUNTIME_SNAPSHOT_REL)
}

pub fn save_runtime_snapshot(dir: &Path, snapshot: &RuntimeSnapshot) -> std::io::Result<()> {
    if snapshot.schema_version != RUNTIME_SCHEMA_VERSION {
        return Err(std::io::Error::other("unsupported runtime snapshot schema"));
    }
    let yaml = serde_saphyr::to_string(snapshot).map_err(std::io::Error::other)?;
    if take_injected_failure(dir) {
        return Err(std::io::Error::other(
            "injected runtime persistence failure",
        ));
    }
    atomic_write(&runtime_snapshot_path(dir), yaml.as_bytes())
}

pub fn load_runtime(
    dir: &Path,
    initial: &BTreeMap<DeviceId, SegmentId>,
    sk: &Skeleton,
) -> std::io::Result<(Store, BTreeMap<DeviceId, SegmentId>)> {
    let runtime_path = runtime_snapshot_path(dir);
    if runtime_path.exists() {
        return load_authoritative_runtime(dir, &runtime_path, initial, sk);
    }
    if has_legacy_state(dir)? {
        return migrate_legacy_state(dir, initial, sk);
    }
    Ok((Store::default(), initial.clone()))
}

fn load_authoritative_runtime(
    dir: &Path,
    runtime_path: &Path,
    initial: &BTreeMap<DeviceId, SegmentId>,
    sk: &Skeleton,
) -> std::io::Result<(Store, BTreeMap<DeviceId, SegmentId>)> {
    let text = std::fs::read_to_string(runtime_path)?;
    let snapshot: RuntimeSnapshot = serde_saphyr::from_str(&text).map_err(std::io::Error::other)?;
    if snapshot.schema_version != RUNTIME_SCHEMA_VERSION {
        return Err(std::io::Error::other("unsupported runtime snapshot schema"));
    }
    let assignments = apply_assignments(initial, &snapshot.assignments, sk)?;
    let mut store = Store::default();
    for profile in snapshot.calibration_profiles {
        if store.profiles.insert(profile.id.clone(), profile).is_some() {
            return Err(std::io::Error::other(
                "runtime snapshot contains a duplicate calibration profile",
            ));
        }
    }
    store.stale = snapshot.stale_calibrations;
    // Surface any unmigrated corrupted legacy profile files without letting valid legacy files
    // conflict with the authoritative runtime snapshot.
    let legacy = crate::calibration::scan_legacy_profiles(dir)?;
    for invalid in legacy.invalid {
        if !store.profiles.contains_key(&invalid.id) {
            store.invalid.push(invalid);
        }
    }
    Ok((store, assignments))
}

fn has_legacy_state(dir: &Path) -> std::io::Result<bool> {
    if dir.join(LEGACY_ASSIGNMENTS_REL).exists() || dir.join(LEGACY_STATES_REL).exists() {
        return Ok(true);
    }
    let cal_dir = dir.join("calibration");
    if !cal_dir.exists() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(cal_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_legacy_state(
    dir: &Path,
    initial: &BTreeMap<DeviceId, SegmentId>,
    sk: &Skeleton,
) -> std::io::Result<(Store, BTreeMap<DeviceId, SegmentId>)> {
    let assignments_path = dir.join(LEGACY_ASSIGNMENTS_REL);
    let overrides: BTreeMap<DeviceId, Option<SegmentId>> = if assignments_path.exists() {
        let text = std::fs::read_to_string(&assignments_path)?;
        serde_saphyr::from_str(&text).map_err(std::io::Error::other)?
    } else {
        BTreeMap::new()
    };
    let assignments = apply_assignments(initial, &overrides, sk)?;

    let legacy = crate::calibration::scan_legacy_profiles(dir)?;
    let mut store = Store::default();
    let mut migrated_paths = Vec::with_capacity(legacy.valid.len());
    for (path, profile) in legacy.valid {
        store.profiles.insert(profile.id.clone(), profile);
        migrated_paths.push(path);
    }
    store.invalid = legacy.invalid;

    let states_path = dir.join(LEGACY_STATES_REL);
    let mut states_present = false;
    if states_path.exists() {
        states_present = true;
        match read_legacy_states(&states_path) {
            Ok(stale) => store.stale = stale,
            Err(e) => {
                tracing::error!(%e, "Legacy calibration state file is invalid; marking migrated profiles stale");
                store.stale.extend(store.profiles.keys().cloned());
                store.warnings.push(crate::registry::reason(
                    "calibration_state_invalid",
                    None,
                    "Saved calibration state could not be read. Recapture the reference pose to repair it.",
                ));
            }
        }
    }

    let snapshot = RuntimeSnapshot {
        schema_version: RUNTIME_SCHEMA_VERSION,
        assignments: overrides,
        stale_calibrations: store.stale.clone(),
        calibration_profiles: store.profiles.values().cloned().collect(),
    };
    // Commit runtime.yaml atomically before touching any legacy file.
    save_runtime_snapshot(dir, &snapshot)?;

    tracing::info!(
        dir=%dir.display(),
        profiles=store.profiles.len(),
        invalid_profiles=store.invalid.len(),
        stale=store.stale.len(),
        "Migrated legacy runtime state to state/runtime.yaml"
    );

    if assignments_path.exists() {
        archive_migrated(&assignments_path, "yaml.migrated");
    }
    if states_present {
        archive_migrated(&states_path, "json.migrated");
    }
    for path in migrated_paths {
        archive_migrated(&path, "json.migrated");
    }

    Ok((store, assignments))
}

fn archive_migrated(path: &Path, extension: &str) {
    let archived = path.with_extension(extension);
    if let Err(error) = std::fs::rename(path, &archived) {
        tracing::warn!(
            path=%path.display(),
            archived=%archived.display(),
            %error,
            "Runtime snapshot committed, but legacy file could not be archived"
        );
    }
}

fn read_legacy_states(path: &Path) -> std::io::Result<BTreeSet<ProfileId>> {
    let data: CalibrationStates = serde_json::from_slice(&std::fs::read(path)?)?;
    if data.schema_version != 1 {
        return Err(std::io::Error::other(
            "unsupported calibration-state schema",
        ));
    }
    Ok(data.stale)
}

fn apply_assignments(
    initial: &BTreeMap<DeviceId, SegmentId>,
    overrides: &BTreeMap<DeviceId, Option<SegmentId>>,
    sk: &Skeleton,
) -> std::io::Result<BTreeMap<DeviceId, SegmentId>> {
    let mut assignments = initial.clone();
    for (id, segment) in overrides {
        match segment {
            Some(s) if sk.index_of(s).is_some() => {
                assignments.insert(id.clone(), s.clone());
            }
            None => {
                assignments.remove(id);
            }
            _ => {
                return Err(std::io::Error::other(
                    "runtime assignment refers to an unknown segment",
                ));
            }
        }
    }
    let mut taken = BTreeSet::new();
    if assignments.values().any(|s| !taken.insert(s)) {
        return Err(std::io::Error::other(
            "runtime assignments contain a duplicate segment",
        ));
    }
    Ok(assignments)
}

pub(crate) fn remove_legacy_profile_file(dir: &Path, id: &ProfileId) -> std::io::Result<()> {
    let path = dir.join("calibration").join(format!("{id}.json"));
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
