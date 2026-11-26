//! Read-only source ablation. Each preview starts from the same live input snapshot.
use super::{
    Runner,
    command::ControlError,
    solve::{Solved, solve},
};
use crate::{config::TrackingMode, registry::Registry};
use bansayo_contracts::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn filtered(registry: &Registry, selected: &BTreeSet<DeviceId>) -> Registry {
    let mut result = registry.clone();
    result.entries.retain(|id, _| selected.contains(id));
    result.assignments.retain(|id, _| selected.contains(id));
    // Do not carry heading corrections or root anchors from excluded sources.
    result.fusion = Default::default();
    result
}

fn wire(solved: Solved) -> Value {
    json!({
        "pose": solved.pose,
        "quality": solved.quality,
        "t_ref": solved.t_ref,
    })
}

pub(super) fn preview(engine: &Runner, ids: &[DeviceId]) -> Result<Value, ControlError> {
    let selected: BTreeSet<_> = ids.iter().cloned().collect();
    if selected.is_empty()
        || selected.len() > 32
        || selected.iter().any(|id| !engine.registry.entries.contains_key(id))
    {
        return Err(ControlError::new(
            ApiErrorCode::InvalidRequest,
            "Select one to 32 known devices.",
        ));
    }
    let registry = filtered(&engine.registry, &selected);
    let now = engine.clock.now_us();
    let run = |mode| {
        let mut config = engine.config.config.clone();
        config.tracking.mode = mode;
        solve(&engine.skeleton, &registry, &engine.store, now, &config)
    };
    Ok(json!({
        "t_us": now,
        "selected": selected,
        "skeleton": engine.skeleton.definition,
        "imu": wire(run(TrackingMode::ImuOnly)),
        "camera": wire(run(TrackingMode::CameraOnly)),
        "hybrid": wire(run(TrackingMode::HybridAuto)),
    }))
}
