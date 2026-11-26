//! Read-only source ablation. Each preview starts from the same live input snapshot.
use super::{
    Runner,
    command::{ControlError, Reply},
    solve::{Solved, solve},
};
use crate::{
    calibration::Store,
    config::{Config, TrackingMode},
    registry::Registry,
    skeleton::Skeleton,
};
use bansayo_contracts::*;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};

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

struct Preview {
    selected: BTreeSet<DeviceId>,
    registry: Registry,
    store: Store,
    skeleton: Skeleton,
    config: Config,
    now: u64,
}

fn prepare(engine: &Runner, ids: &[DeviceId]) -> Result<Preview, ControlError> {
    let selected: BTreeSet<_> = ids.iter().cloned().collect();
    if selected.is_empty()
        || selected.len() > 32
        || selected
            .iter()
            .any(|id| !engine.registry.entries.contains_key(id))
    {
        return Err(ControlError::new(
            ApiErrorCode::InvalidRequest,
            "Select one to 32 known devices.",
        ));
    }
    Ok(Preview {
        registry: filtered(&engine.registry, &selected),
        selected,
        store: engine.store.clone(),
        skeleton: engine.skeleton.clone(),
        config: engine.config.config.clone(),
        now: engine.clock.now_us(),
    })
}

impl Preview {
    fn solve(&self) -> Value {
        let run = |mode| {
            let mut config = self.config.clone();
            config.tracking.mode = mode;
            solve(
                &self.skeleton,
                &self.registry,
                &self.store,
                self.now,
                &config,
            )
        };
        json!({
            "t_us": self.now,
            "selected": self.selected,
            "skeleton": self.skeleton.definition,
            "imu": wire(run(TrackingMode::ImuOnly)),
            "camera": wire(run(TrackingMode::CameraOnly)),
            "hybrid": wire(run(TrackingMode::HybridAuto)),
        })
    }
}

pub(super) fn start(
    engine: &Runner,
    ids: &[DeviceId],
    permit: &Arc<tokio::sync::Semaphore>,
    reply: Reply,
) {
    let Ok(permit) = permit.clone().try_acquire_owned() else {
        let _ = reply.send(Err(ControlError::new(
            ApiErrorCode::Conflict,
            "A solver inspection is already running.",
        )));
        return;
    };
    match prepare(engine, ids) {
        Ok(input) => {
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                if !reply.is_closed() {
                    let _ = reply.send(Ok(input.solve()));
                }
            });
        }
        Err(e) => {
            let _ = reply.send(Err(e));
        }
    }
}
