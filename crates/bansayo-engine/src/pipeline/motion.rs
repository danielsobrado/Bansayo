//! Conversion seam from the current solver to the canonical training input. No solver stage.
use super::solve::Solved;
use crate::{
    math::{rot_from_wire, v3_from_wire, v3_to_wire},
    registry::{ObservationKind, Registry},
    skeleton::{Skeleton, proportions::LANDMARK_SCHEMA_ID},
};
use bansayo_contracts::*;

pub fn evidence(
    provenance: Provenance,
    method: MethodId,
    confidence: Option<f32>,
    t_obs_us: Option<u64>,
    sources: Vec<DeviceId>,
) -> Evidence {
    Evidence {
        provenance,
        method,
        confidence,
        sigma: None,
        sigma_basis: None,
        t_obs_us,
        sources,
    }
}
/// Canonical measured points, without replacing today's FK body positions.
pub fn measured_keypoints(
    landmarks: Option<&PoseLandmarks3D>,
) -> Vec<Option<TriangulatedLandmark3D>> {
    let mut out = vec![None; 26];
    let Some(PoseLandmarks3D::Triangulated {
        schema_id, points, ..
    }) = landmarks
    else {
        return out;
    };
    if schema_id == LANDMARK_SCHEMA_ID || schema_id == BODY_KEYPOINT_SCHEMA_ID {
        for (a, b) in out.iter_mut().zip(points) {
            *a = b.clone();
        }
    } else if schema_id == super::skeleton_fit::COCO17_SCHEMA_ID {
        for (canonical, detector) in [
            (6, 5),
            (7, 7),
            (8, 9),
            (10, 6),
            (11, 8),
            (12, 10),
            (13, 11),
            (14, 13),
            (15, 15),
            (16, 12),
            (17, 14),
            (18, 16),
        ] {
            out[canonical] = points.get(detector).cloned().flatten();
        }
        if let (Some(a), Some(b)) = (
            points.get(11).and_then(Option::as_ref),
            points.get(12).and_then(Option::as_ref),
        ) {
            let mut p = a.clone();
            p.m = std::array::from_fn(|i| (a.m[i] + b.m[i]) * 0.5);
            p.confidence = a.confidence.min(b.confidence);
            p.sigma_m = a
                .sigma_m
                .zip(b.sigma_m)
                .map(|(a, b)| (a * a + b * b).sqrt() * 0.5);
            p.used_views.extend(b.used_views.iter().cloned());
            p.used_views.sort();
            p.used_views.dedup();
            p.views = p.used_views.len() as u32;
            out[0] = Some(p);
        }
    }
    out
}
pub fn sample(
    sk: &Skeleton,
    registry: &Registry,
    solved: &Solved,
    seq: u64,
    t_us: u64,
    origins: Vec<Origin>,
) -> MotionSample {
    let measured = measured_keypoints(solved.landmarks_3d.as_ref());
    let t_camera = match &solved.landmarks_3d {
        Some(PoseLandmarks3D::Triangulated { t_us, .. }) => Some(*t_us),
        _ => None,
    };
    let body = solved.pose.as_ref().map(|pose| {
        let segments: Vec<_> = pose
            .segments
            .iter()
            .map(|p| {
                let imu = p.sources.iter().any(|id| {
                    registry.entries.get(id).is_some_and(|e| {
                        e.descriptor
                            .capabilities
                            .contains(&SensorCapability::Orientation)
                    })
                });
                let observed = matches!(p.provenance, Provenance::Measured | Provenance::Derived);
                let time = if imu {
                    p.sources
                        .iter()
                        .filter_map(|id| {
                            registry
                                .entries
                                .get(id)?
                                .latest
                                .get(&ObservationKind::Orientation)
                                .map(|o| o.t_us)
                        })
                        .max()
                } else if observed {
                    t_camera
                } else {
                    None
                };
                let axis = evidence(
                    p.provenance,
                    if !observed {
                        MethodId::RestFill
                    } else if imu {
                        MethodId::ImuMounted
                    } else {
                        MethodId::SkeletonFitAxis
                    },
                    p.confidence,
                    time,
                    p.sources.clone(),
                );
                let twist = if imu {
                    axis.clone()
                } else {
                    evidence(Provenance::Inferred, MethodId::RestFill, None, None, vec![])
                };
                SegmentState {
                    rotation: p.rotation,
                    axis,
                    twist,
                    angular_velocity_rad_s: None,
                    angular_velocity: evidence(
                        Provenance::Unavailable,
                        MethodId::ImuGyro,
                        None,
                        None,
                        vec![],
                    ),
                }
            })
            .collect();
        let mut root_evidence = if pose.root_position_provenance == Provenance::Measured {
            let point = measured[0].as_ref();
            evidence(
                Provenance::Measured,
                MethodId::Triangulated,
                point.map(|p| p.confidence),
                t_camera,
                point.map_or_else(Vec::new, |p| p.used_views.clone()),
            )
        } else {
            evidence(
                pose.root_position_provenance,
                MethodId::FloorContact,
                None,
                segments.iter().filter_map(|s| s.axis.t_obs_us).max(),
                segments
                    .iter()
                    .flat_map(|s| s.axis.sources.clone())
                    .collect(),
            )
        };
        root_evidence.sources.sort();
        root_evidence.sources.dedup();
        root_evidence.sigma = if pose.root_position_provenance == Provenance::Measured {
            measured[0].as_ref().and_then(|p| p.sigma_m)
        } else {
            None
        };
        root_evidence.sigma_basis = root_evidence.sigma.map(|_| SigmaBasis::Propagated);
        let root = KeypointState {
            position_m: pose.root_position_m,
            evidence: root_evidence,
        };
        let mut keypoints: Vec<Option<KeypointState>> = vec![None; 26];
        keypoints[0] = Some(root.clone());
        for i in 1..sk.len().min(19) {
            let parent = sk.segments[i].parent.and_then(|i| keypoints[i].as_ref());
            // A segment's origin is placed by its parent's rotation, not by its own axis.
            let placing = sk.segments[i].parent.unwrap_or(i);
            let mut e = segments[placing].axis.clone();
            e.method = MethodId::Fk;
            e.sigma = None;
            e.sigma_basis = None;
            if let Some(parent) = parent {
                if parent.evidence.provenance == Provenance::Inferred {
                    e.provenance = Provenance::Inferred;
                }
                e.confidence = e
                    .confidence
                    .zip(parent.evidence.confidence)
                    .map(|(a, b)| a.min(b));
                e.t_obs_us = e.t_obs_us.max(parent.evidence.t_obs_us);
                e.sources.extend(parent.evidence.sources.clone());
                e.sources.sort();
                e.sources.dedup();
            }
            keypoints[i] = Some(KeypointState {
                position_m: pose.segments[i].position_m,
                evidence: e,
            });
        }
        for (target, segment, contact) in [
            (19, 4, None),
            (20, 8, None),
            (21, 12, None),
            (22, 15, Some(1)),
            (23, 18, Some(1)),
            (24, 15, Some(0)),
            (25, 18, Some(0)),
        ] {
            if let (Some(p), Some(def), Some(seg)) = (
                pose.segments.get(segment),
                sk.segments.get(segment),
                segments.get(segment),
            ) {
                let offset = contact
                    .and_then(|i| def.contact_points.get(i))
                    .unwrap_or(&def.tail);
                if let Some(rotation) = rot_from_wire(p.rotation) {
                    let position = v3_from_wire(p.position_m) + rotation * offset;
                    // Distal points are placed by the segment's own rotation on top of its origin.
                    keypoints[target] = keypoints[segment].as_ref().map(|p| {
                        let mut e = p.evidence.clone();
                        if !matches!(
                            seg.axis.provenance,
                            Provenance::Measured | Provenance::Derived
                        ) {
                            e.provenance = Provenance::Inferred;
                            e.method = MethodId::RestFill;
                        }
                        KeypointState {
                            position_m: v3_to_wire(&position),
                            evidence: e,
                        }
                    });
                }
            }
        }
        let observed_ankles = [15, 18].map(|i| {
            let point = measured[i].as_ref()?;
            let mut e = evidence(
                Provenance::Measured,
                MethodId::Triangulated,
                Some(point.confidence),
                t_camera,
                point.used_views.clone(),
            );
            e.sigma = point.sigma_m;
            e.sigma_basis = e.sigma.map(|_| SigmaBasis::Propagated);
            Some(KeypointState {
                position_m: point.m,
                evidence: e,
            })
        });
        BodyState {
            skeleton_rev: pose.skeleton_rev,
            segments,
            keypoints,
            root,
            observed_ankles: observed_ankles
                .iter()
                .any(Option::is_some)
                .then_some(observed_ankles),
        }
    });
    MotionSample {
        seq,
        t_us,
        origins,
        body,
        planar: vec![],
    }
}
