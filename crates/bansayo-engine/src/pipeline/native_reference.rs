//! Native AHRS reference changes invalidate old mounting transforms.
use super::Runner;
use bansayo_contracts::{CalibrationKind, CalibrationProfile, DeviceId, ProfileId};
use std::collections::BTreeSet;

pub(super) fn is_native(engine: &Runner, id: &DeviceId) -> bool {
    engine
        .config
        .esp32
        .iter()
        .any(|source| source.trackers.iter().any(|(tracker, _)| tracker == id))
}

fn belongs(profile: &CalibrationProfile, id: &DeviceId) -> bool {
    profile.kind == CalibrationKind::Mounting && profile.subject.device_id.as_ref() == Some(id)
}

pub(super) fn invalidate(engine: &mut Runner, id: &DeviceId) -> bool {
    let mut changed = false;
    for profile in engine.store.profiles.values().filter(|p| belongs(p, id)) {
        changed |= engine.store.stale.insert(profile.id.clone());
    }
    changed
}

pub(super) fn changed_profiles(
    profiles: &[CalibrationProfile],
    devices: &BTreeSet<DeviceId>,
) -> BTreeSet<ProfileId> {
    profiles
        .iter()
        .filter(|p| devices.iter().any(|id| belongs(p, id)))
        .map(|p| p.id.clone())
        .collect()
}
