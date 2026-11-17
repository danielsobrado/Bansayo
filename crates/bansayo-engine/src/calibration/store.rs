use crate::math::{Rot, rot_from_wire};
use bansayo_contracts::*;
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
    pub fn mounting(&self, device: &DeviceId, segment: &SegmentId) -> Option<Rot> {
        self.profiles
            .values()
            .find(|p| {
                p.kind == CalibrationKind::Mounting
                    && valid_profile(p)
                    && !self.stale.contains(&p.id)
                    && p.subject.device_id.as_ref() == Some(device)
                    && p.subject.segment_id.as_ref() == Some(segment)
            })
            .and_then(|p| serde_json::from_value::<MountingData>(p.data.clone().into()).ok())
            .and_then(|p| rot_from_wire(p.q_sensor_from_segment))
    }
    /// Typed `data` of a non-stale profile of `kind` for `device`, with its id.
    pub fn camera<T: serde::de::DeserializeOwned>(
        &self,
        kind: CalibrationKind,
        device: &DeviceId,
    ) -> Option<(ProfileId, T)> {
        let p = self.profiles.values().find(|p| {
            p.kind == kind
                && valid_profile(p)
                && !self.stale.contains(&p.id)
                && p.subject.device_id.as_ref() == Some(device)
        })?;
        Some((
            p.id.clone(),
            serde_json::from_value(p.data.clone().into()).ok()?,
        ))
    }
    pub fn camera_ref(&self, kind: CalibrationKind, device: &DeviceId) -> CalibrationRef {
        let profile = self
            .profiles
            .values()
            .find(|p| p.kind == kind && p.subject.device_id.as_ref() == Some(device));
        CalibrationRef {
            kind,
            state: match profile {
                Some(p) if !valid_profile(p) => CalibrationState::Invalid,
                Some(p) if self.stale.contains(&p.id) => CalibrationState::Stale,
                Some(_) => CalibrationState::Valid,
                None => CalibrationState::Missing,
            },
            profile_id: profile.map(|p| p.id.clone()),
        }
    }
    pub fn reference(&self, device: &DeviceId, segment: Option<&SegmentId>) -> CalibrationRef {
        let profile = segment.and_then(|segment| {
            self.profiles.values().find(|p| {
                p.kind == CalibrationKind::Mounting
                    && p.subject.device_id.as_ref() == Some(device)
                    && p.subject.segment_id.as_ref() == Some(segment)
            })
        });
        let invalid = segment.and_then(|segment| {
            self.invalid.iter().find(|p| {
                p.device_id.as_ref() == Some(device) && p.segment_id.as_ref() == Some(segment)
                    || p.id.as_str() == format!("mounting-{device}-{segment}")
            })
        });
        let state = if invalid.is_some() || profile.is_some_and(|p| !valid_profile(p)) {
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
                if !valid_profile(p) {
                    s.state = CalibrationState::Invalid;
                } else if self.stale.contains(&p.id) {
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
pub fn valid_profile(p: &CalibrationProfile) -> bool {
    use crate::pipeline::triangulate::{extrinsics_valid, intrinsics_valid};
    let data = || serde_json::Value::Object(p.data.clone());
    p.schema_version == CALIBRATION_PROFILE_SCHEMA_VERSION
        && p.subject.device_id.is_some()
        && match p.kind {
            CalibrationKind::Mounting => {
                p.subject.segment_id.is_some()
                    && serde_json::from_value::<MountingData>(data())
                        .ok()
                        .is_some_and(|d| rot_from_wire(d.q_sensor_from_segment).is_some())
            }
            CalibrationKind::CameraIntrinsics => {
                p.subject.segment_id.is_none()
                    && serde_json::from_value::<CameraIntrinsicsData>(data())
                        .ok()
                        .is_some_and(|k| intrinsics_valid(&k) && !k.image_size_px.contains(&0))
            }
            CalibrationKind::CameraExtrinsics => {
                p.subject.segment_id.is_none()
                    && serde_json::from_value::<CameraExtrinsicsData>(data())
                        .ok()
                        .is_some_and(|e| extrinsics_valid(&e))
            }
            _ => false,
        }
}

/// Canonical id of a camera profile; one profile per kind and camera, so a new one replaces it.
pub fn camera_profile_id(kind: CalibrationKind, device: &DeviceId) -> Option<ProfileId> {
    let name = match kind {
        CalibrationKind::CameraIntrinsics => "intrinsics",
        CalibrationKind::CameraExtrinsics => "extrinsics",
        _ => return None,
    };
    ProfileId::new(format!("camera-{name}-{device}")).ok()
}
pub(crate) struct LegacyProfiles {
    pub valid: Vec<(PathBuf, CalibrationProfile)>,
    pub invalid: Vec<CalibrationProfileSummary>,
}

pub(crate) fn scan_legacy_profiles(data_dir: &Path) -> std::io::Result<LegacyProfiles> {
    let dir = data_dir.join("calibration");
    if !dir.exists() {
        return Ok(LegacyProfiles {
            valid: vec![],
            invalid: vec![],
        });
    }
    let mut valid = vec![];
    let mut invalid_list = vec![];
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
            Some(p) if p.id == id => {
                valid.push((path, p));
            }
            other => {
                tracing::warn!(profile=%id, "Ignoring invalid calibration profile");
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
                            data: Default::default(),
                        });
                invalid.id = id;
                invalid.state = CalibrationState::Invalid;
                invalid_list.push(invalid);
            }
        }
    }
    valid.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    invalid_list.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(LegacyProfiles {
        valid,
        invalid: invalid_list,
    })
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
        data: match p.kind {
            CalibrationKind::CameraIntrinsics | CalibrationKind::CameraExtrinsics => p.data.clone(),
            _ if !valid_profile(p) => p.data.clone(),
            _ => Default::default(),
        },
    }
}
