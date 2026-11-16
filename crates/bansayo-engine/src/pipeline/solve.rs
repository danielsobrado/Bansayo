//! Pure orientation, fill, forward-kinematics and floor-contact stages.
use crate::{
    calibration::Store,
    math::{Rot, rot_from_wire, rot_to_wire, v3_to_wire},
    registry::{Registry, reason},
    skeleton::{Skeleton, floor_contact_root, forward_kinematics},
};
use kinexa_contracts::*;
pub struct Solved {
    pub pose: Option<SkeletonPose>,
    pub rotations: Vec<Rot>,
    pub observed: Vec<bool>,
    pub quality: TrackingQuality,
}
pub fn solve(sk: &Skeleton, registry: &Registry, store: &Store, now: u64, stale_us: u64) -> Solved {
    let mut rotations = vec![Rot::identity(); sk.len()];
    let mut observed = vec![false; sk.len()];
    let mut sources = vec![vec![]; sk.len()];
    let mut confidence = vec![None; sk.len()];
    let mut reasons = store.warnings.clone();
    let has_trackers = registry
        .entries
        .values()
        .any(|e| e.descriptor.kind == DeviceKind::ImuTracker);
    for (id, segment) in &registry.assignments {
        let Some(index) = sk.index_of(segment) else {
            continue;
        };
        let Some(entry) = registry.entries.get(id) else {
            continue;
        };
        let mounting = store.mounting(id, segment);
        let Some(mounting) = mounting else {
            reasons.push(reason(
                match store.reference(id, Some(segment)).state {
                    CalibrationState::Stale => "mounting_calibration_stale",
                    CalibrationState::Invalid => "mounting_calibration_invalid",
                    _ => "mounting_calibration_missing",
                },
                Some(id.to_string()),
                "Capture the reference pose to calibrate this tracker's mounting.",
            ));
            continue;
        };
        let Some(o) = entry
            .latest
            .as_ref()
            .filter(|o| now.saturating_sub(o.t_us) <= stale_us)
        else {
            reasons.push(reason(
                "observation_stale",
                Some(id.to_string()),
                "Tracker is not delivering fresh orientation data.",
            ));
            continue;
        };
        if let ObservationData::Orientation { q, .. } = o.data
            && let Some(reading) = rot_from_wire(q)
        {
            rotations[index] = reading * mounting;
            observed[index] = true;
            sources[index].push(id.clone());
            confidence[index] = o.confidence.map(|c| {
                c * (1.0 - now.saturating_sub(o.t_us) as f32 / stale_us as f32).clamp(0.0, 1.0)
            });
        }
    }
    // An unobserved segment follows its parent at its rest-relative rotation; never labelled observed.
    for (i, s) in sk.segments.iter().enumerate() {
        if !observed[i] {
            rotations[i] = s.parent.map_or(Rot::identity(), |p| rotations[p]);
        }
    }
    let observed_count = observed.iter().filter(|o| **o).count() as u32;
    let state = if !has_trackers {
        TrackingState::Unavailable
    } else if observed_count == 0 {
        TrackingState::Acquiring
    } else if observed_count < sk.len() as u32 {
        TrackingState::Degraded
    } else {
        TrackingState::Tracking
    };
    if !has_trackers {
        reasons.push(reason("no_orientation_devices",None,"No orientation trackers are configured; camera landmarks do not provide a world skeleton."));
    }
    if observed_count > 0 {
        reasons.push(reason(
            "heading_alignment_assumed",
            None,
            "Heading alignment is assumed to be identity.",
        ));
    }
    let root = floor_contact_root(sk, &rotations);
    let positions = forward_kinematics(sk, &rotations, root);
    let fresh_orientation = registry.entries.values().any(|e| {
        e.latest.as_ref().is_some_and(|o| {
            matches!(o.data, ObservationData::Orientation { .. })
                && now.saturating_sub(o.t_us) <= stale_us
        })
    });
    let pose = fresh_orientation.then(|| SkeletonPose {
        skeleton_rev: sk.definition.rev,
        root_position_m: v3_to_wire(&root),
        root_position_provenance: Provenance::Inferred,
        segments: rotations
            .iter()
            .enumerate()
            .map(|(i, q)| SegmentPose {
                rotation: rot_to_wire(q),
                position_m: v3_to_wire(&positions[i]),
                provenance: if observed[i] {
                    Provenance::Derived
                } else {
                    Provenance::Inferred
                },
                confidence: confidence[i],
                sources: sources[i].clone(),
            })
            .collect(),
    });
    Solved {
        pose,
        rotations,
        observed,
        quality: TrackingQuality {
            state,
            observed_segments: observed_count,
            total_segments: sk.len() as u32,
            reasons,
        },
    }
}
