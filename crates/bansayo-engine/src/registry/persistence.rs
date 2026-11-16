//! Conservative persistence: invalidation is durable before a changed assignment is acknowledged.
use crate::skeleton::Skeleton;
use bansayo_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static TEMP: AtomicU64 = AtomicU64::new(0);
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
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, path)
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationStates {
    pub schema_version: u32,
    pub stale: BTreeSet<ProfileId>,
}
pub fn persist_states(dir: &Path, stale: &BTreeSet<ProfileId>) -> std::io::Result<()> {
    atomic_write(
        &dir.join("state/calibration-states.json"),
        &serde_json::to_vec_pretty(&CalibrationStates {
            schema_version: 1,
            stale: stale.clone(),
        })?,
    )
}
pub fn load_states(dir: &Path) -> std::io::Result<BTreeSet<ProfileId>> {
    let path = dir.join("state/calibration-states.json");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let data: CalibrationStates = serde_json::from_slice(&std::fs::read(path)?)?;
    if data.schema_version != 1 {
        return Err(std::io::Error::other(
            "unsupported calibration-state schema",
        ));
    }
    Ok(data.stale)
}
pub fn save_assignments(
    dir: &Path,
    assignments: &BTreeMap<DeviceId, SegmentId>,
    known: impl Iterator<Item = DeviceId>,
) -> std::io::Result<()> {
    let map: BTreeMap<DeviceId, Option<SegmentId>> = known
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
    atomic_write(
        &dir.join("state/assignments.yaml"),
        &serde_json::to_vec_pretty(&map)?,
    )
}
pub fn load_assignments(
    dir: &Path,
    initial: &BTreeMap<DeviceId, SegmentId>,
    sk: &Skeleton,
) -> std::io::Result<BTreeMap<DeviceId, SegmentId>> {
    let path = dir.join("state/assignments.yaml");
    if !path.exists() {
        return Ok(initial.clone());
    }
    let text = std::fs::read_to_string(path)?;
    let overrides: BTreeMap<DeviceId, Option<SegmentId>> =
        serde_saphyr::from_str(&text).map_err(std::io::Error::other)?;
    let mut assignments = initial.clone();
    for (id, segment) in overrides {
        match segment {
            Some(s) if sk.index_of(&s).is_some() => {
                assignments.insert(id, s);
            }
            None => {
                assignments.remove(&id);
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
