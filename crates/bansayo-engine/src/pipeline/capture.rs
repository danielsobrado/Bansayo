use crate::{
    calibration::mounting::{MAX_SPREAD_RAD, mean_and_spread},
    clock::utc_now_rfc3339,
    math::{Rot, rot_to_wire},
    pipeline::{Runner, command::ControlError},
};
use bansayo_contracts::*;
use std::collections::BTreeMap;
use tokio::sync::oneshot;
pub(super) struct Capture {
    pub until: u64,
    pub targets: BTreeMap<DeviceId, (SegmentId, Vec<Rot>)>,
    pub reply: oneshot::Sender<Result<Vec<CalibrationProfileSummary>, ControlError>>,
}
impl Runner {
    pub(super) fn targets(
        &self,
        r: ReferencePoseRequest,
    ) -> Result<BTreeMap<DeviceId, (SegmentId, Vec<Rot>)>, ControlError> {
        let ids = r.device_ids.unwrap_or_else(|| {
            self.registry
                .assignments
                .keys()
                .filter(|id| self.registry.entries.contains_key(*id))
                .cloned()
                .collect()
        });
        let mut targets = BTreeMap::new();
        let now = self.clock.now_us();
        for id in ids {
            let e = self.registry.entries.get(&id).ok_or_else(|| {
                ControlError::new(
                    ApiErrorCode::NotFound,
                    format!("Tracker {id} was not found."),
                )
            })?;
            let segment = self.registry.assignments.get(&id).ok_or_else(|| {
                ControlError::new(
                    ApiErrorCode::InvalidRequest,
                    format!("Tracker {id} is not assigned to a segment."),
                )
            })?;
            if e.latest.as_ref().is_none_or(|o| {
                now.saturating_sub(o.t_us) > self.config.config.engine.stale_after_ms * 1000
            }) {
                return Err(ControlError::new(
                    ApiErrorCode::Unavailable,
                    format!("Tracker {id} is not delivering data."),
                ));
            }
            targets.insert(id, (segment.clone(), vec![]));
        }
        if targets.is_empty() {
            return Err(ControlError::new(
                ApiErrorCode::Unavailable,
                "No assigned tracking devices are available for reference-pose capture.",
            ));
        }
        Ok(targets)
    }
}

pub(super) fn profiles(
    targets: BTreeMap<DeviceId, (SegmentId, Vec<Rot>)>,
) -> Result<Vec<CalibrationProfile>, ControlError> {
    let mut out = vec![];
    for (device, (segment, samples)) in targets {
        if samples.len() < 2 {
            return Err(ControlError::new(
                ApiErrorCode::Unavailable,
                format!(
                    "Tracker {device} is not delivering enough data. Increase its sampling rate and retry."
                ),
            ));
        }
        let (mean, spread) = mean_and_spread(&samples).ok_or_else(|| {
            ControlError::new(
                ApiErrorCode::InvalidRequest,
                "Invalid orientation during capture.",
            )
        })?;
        if spread > MAX_SPREAD_RAD {
            return Err(ControlError::new(
                ApiErrorCode::InvalidRequest,
                format!(
                    "Movement detected during capture for {device}. Hold the reference pose still and retry."
                ),
            ));
        }
        let id = ProfileId::new(format!("mounting-{device}-{segment}")).map_err(|_| {
            ControlError::new(
                ApiErrorCode::Internal,
                "Could not create calibration identity.",
            )
        })?;
        let data = serde_json::json!({"q_sensor_from_segment":rot_to_wire(&mean.inverse())});
        let quality = serde_json::json!({"capture_window_ms":1000,"angular_spread_rad":spread});
        out.push(CalibrationProfile {
            schema_version: CALIBRATION_PROFILE_SCHEMA_VERSION,
            id,
            kind: CalibrationKind::Mounting,
            method: "reference_pose".into(),
            origin: Origin::Simulated,
            created_utc: utc_now_rfc3339(),
            subject: CalibrationSubject {
                device_id: Some(device.clone()),
                segment_id: Some(segment),
            },
            depends_on: vec![CalibrationDependency {
                kind: CalibrationKind::HeadingAlignment,
                device_id: Some(device),
            }],
            data: data.as_object().cloned().unwrap_or_default(),
            quality: quality.as_object().cloned().unwrap_or_default(),
        });
    }
    Ok(out)
}
