use crate::math::{Rot, rot_from_wire};
use kinexa_contracts::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Default)]
pub struct Store {
    pub profiles: BTreeMap<ProfileId, CalibrationProfile>,
    pub stale: std::collections::BTreeSet<ProfileId>,
    pub warnings: Vec<Reason>,
    pub invalid: Vec<CalibrationProfileSummary>,
}
impl Store {
    pub fn load(data_dir: &Path) -> std::io::Result<Self> {
        let mut store = Self::default();
        let dir = data_dir.join("calibration");
        std::fs::create_dir_all(&dir)?;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|v| v.to_str())
                .and_then(|v| ProfileId::new(v).ok())
            else {
                continue;
            };
            let profile = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<CalibrationProfile>(&bytes).ok());
            match profile {
                Some(p) if valid(&p) && p.id == id => {
                    store.profiles.insert(id, p);
                }
                other => {
                    tracing::warn!(profile=%id,"Ignoring invalid calibration profile");
                    let mut invalid =
                        other
                            .as_ref()
                            .map(summary)
                            .unwrap_or(CalibrationProfileSummary {
                                id: id.clone(),
                                kind: CalibrationKind::Mounting,
                                state: CalibrationState::Invalid,
                                device_id: None,
                                segment_id: None,
                                method: "invalid_file".into(),
                                created_utc: None,
                            });
                    invalid.id = id;
                    invalid.state = CalibrationState::Invalid;
                    store.invalid.push(invalid);
                }
            }
        }
        match crate::registry::persistence::load_states(data_dir) {
            Ok(stale) => store.stale = stale,
            Err(e) => {
                tracing::error!(%e,"Calibration state file is invalid; recapture required");
                store.stale.extend(store.profiles.keys().cloned());
                store.warnings.push(crate::registry::reason("calibration_state_invalid",None,"Saved calibration state could not be read. Recapture the reference pose to repair it."));
            }
        }
        Ok(store)
    }
    pub fn mounting(&self, device: &DeviceId, segment: &SegmentId) -> Option<Rot> {
        self.profiles
            .values()
            .find(|p| {
                p.kind == CalibrationKind::Mounting
                    && !self.stale.contains(&p.id)
                    && p.subject.device_id.as_ref() == Some(device)
                    && p.subject.segment_id.as_ref() == Some(segment)
            })
            .and_then(|p| serde_json::from_value::<MountingData>(p.data.clone().into()).ok())
            .and_then(|p| rot_from_wire(p.q_sensor_from_segment))
    }
    pub fn reference(&self, device: &DeviceId, segment: Option<&SegmentId>) -> CalibrationRef {
        let profile = segment.and_then(|segment| {
            self.profiles.values().find(|p| {
                p.subject.device_id.as_ref() == Some(device)
                    && p.subject.segment_id.as_ref() == Some(segment)
            })
        });
        let invalid = segment.and_then(|segment| {
            self.invalid.iter().find(|p| {
                p.device_id.as_ref() == Some(device) && p.segment_id.as_ref() == Some(segment)
                    || p.id.as_str() == format!("mounting-{device}-{segment}")
            })
        });
        let state = if invalid.is_some() {
            CalibrationState::Invalid
        } else if profile.is_some_and(|p| self.stale.contains(&p.id)) {
            CalibrationState::Stale
        } else if profile.is_some() {
            CalibrationState::Valid
        } else {
            CalibrationState::Missing
        };
        CalibrationRef {
            kind: CalibrationKind::Mounting,
            state,
            profile_id: profile
                .map(|p| p.id.clone())
                .or_else(|| invalid.map(|p| p.id.clone())),
        }
    }
    pub fn summaries(&self) -> Vec<CalibrationProfileSummary> {
        self.profiles
            .values()
            .map(|p| {
                let mut s = summary(p);
                if self.stale.contains(&p.id) {
                    s.state = CalibrationState::Stale;
                }
                s
            })
            .chain(self.invalid.iter().cloned())
            .collect()
    }
    pub fn insert(&mut self, profiles: Vec<CalibrationProfile>) {
        for p in profiles {
            self.stale.remove(&p.id);
            self.invalid.retain(|s| s.id != p.id);
            self.profiles.insert(p.id.clone(), p);
            if self.stale.is_empty() {
                self.warnings.clear();
            }
        }
    }
}
fn valid(p: &CalibrationProfile) -> bool {
    p.schema_version == CALIBRATION_PROFILE_SCHEMA_VERSION
        && p.kind == CalibrationKind::Mounting
        && p.subject.device_id.is_some()
        && p.subject.segment_id.is_some()
        && serde_json::from_value::<MountingData>(p.data.clone().into())
            .ok()
            .is_some_and(|d| rot_from_wire(d.q_sensor_from_segment).is_some())
}
pub fn summary(p: &CalibrationProfile) -> CalibrationProfileSummary {
    CalibrationProfileSummary {
        id: p.id.clone(),
        kind: p.kind,
        state: CalibrationState::Valid,
        device_id: p.subject.device_id.clone(),
        segment_id: p.subject.segment_id.clone(),
        method: p.method.clone(),
        created_utc: Some(p.created_utc.clone()),
    }
}
/// Called on the blocking pool. A profile becomes visible only after its complete file is renamed.
pub fn persist(data_dir: PathBuf, profiles: &[CalibrationProfile]) -> std::io::Result<()> {
    let dir = data_dir.join("calibration");
    std::fs::create_dir_all(&dir)?;
    for p in profiles {
        let destination = dir.join(format!("{}.json", p.id));
        crate::registry::persistence::atomic_write(&destination, &serde_json::to_vec_pretty(p)?)?;
    }
    Ok(())
}
