//! Pure orientation, arbitration, triangulation, root-placement and forward-kinematics stages
//! (docs/05 §8, ADR-025 rung 1).
//!
//! Reconstruction instant `t_ref`: with IMUs, the oldest of their newest fresh samples, and every
//! IMU is SLERPed to it (never extrapolated); without IMUs, the camera set's time. Per segment the
//! evidence is arbitrated, not blended: a fresh, mounted, time-aligned IMU orientation (heading
//! corrected by cameras once that has converged), else the camera skeleton fit, else the parent's
//! rotation, marked inferred. The root is the camera anchor while its age allows, with an
//! uncertainty that grows with that age, then floor contact. Accelerations are never integrated.
use crate::{
    calibration::Store,
    config::{Config, TrackingMode},
    math::{Rot, rot_to_wire, v3_from_wire, v3_to_wire},
    pipeline::{
        align,
        fusion::{self, Anchor, FusionState},
        skeleton_fit::{Fit, fit},
        triangulate::triangulate_with_noise_floor,
    },
    registry::{ObservationKind, Registry, reason},
    skeleton::{Skeleton, floor_contact_root, forward_kinematics, proportions::LANDMARK_SCHEMA_ID},
};
use bansayo_contracts::*;

pub struct Solved {
    pub pose: Option<SkeletonPose>,
    pub rotations: Vec<Rot>,
    pub observed: Vec<bool>,
    pub quality: TrackingQuality,
    pub landmarks_3d: Option<PoseLandmarks3D>,
    /// Method of each segment's rotation, index-aligned with the skeleton.
    pub methods: Vec<MethodId>,
    /// Measurement time of each segment's rotation.
    pub t_obs: Vec<Option<u64>>,
    /// The reconstruction instant.
    pub t_ref: Option<u64>,
    /// Root uncertainty including the anchor's age, metres.
    pub root_sigma_m: Option<f32>,
    /// Measurement time of the root position, when it comes from a camera.
    pub root_t_obs: Option<u64>,
    pub root_sources: Vec<DeviceId>,
    pub root_confidence: Option<f32>,
    /// The root is a camera anchor held through an occlusion, not a current measurement.
    pub root_held: bool,
    /// Fusion state for the next tick; committed only by live and replay ticks.
    pub fusion_next: FusionState,
}

/// One time-aligned, mounted IMU orientation.
struct Imu {
    id: DeviceId,
    rotation: Rot,
    confidence: Option<f32>,
    /// Mounted, uncorrected, at the camera set's capture time: compared with the cameras only at
    /// the same instant, so a fast limb is not mistaken for a heading error.
    at_camera: Option<Rot>,
}

fn orientation_trackers(registry: &Registry) -> bool {
    registry
        .entries
        .values()
        .any(|e| e.descriptor.kind == DeviceKind::ImuTracker)
}

pub fn solve(
    sk: &Skeleton,
    registry: &Registry,
    store: &Store,
    now: u64,
    config: &Config,
) -> Solved {
    let stale_us = config.engine.stale_after_ms * 1000;
    let mode = config.tracking.mode;
    let fusion_cfg = &config.fusion;
    let max_anchor_us = config.time_alignment.max_optical_anchor_age_ms * 1000;
    let mut reasons = store.warnings.clone();
    let has_trackers = mode != TrackingMode::CameraOnly && orientation_trackers(registry);
    let mut next = registry.fusion.clone();

    // IMU candidates: assigned, mounted and fresh.
    let mut candidates = vec![];
    if mode != TrackingMode::CameraOnly {
        for (id, segment) in &registry.assignments {
            let (Some(index), Some(entry)) = (sk.index_of(segment), registry.entries.get(id))
            else {
                continue;
            };
            if !entry.connected || entry.failure.is_some() {
                continue;
            }
            let Some(mounting) = store.mounting(id, segment) else {
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
            let Some(newest) = entry
                .latest
                .get(&ObservationKind::Orientation)
                .filter(|o| o.t_us <= now && now - o.t_us <= stale_us)
            else {
                reasons.push(reason(
                    "observation_stale",
                    Some(id.to_string()),
                    "Tracker is not delivering fresh orientation data.",
                ));
                continue;
            };
            candidates.push((index, id, mounting, entry, newest.t_us));
        }
    }
    let landmarks_3d = (mode != TrackingMode::ImuOnly)
        .then(|| {
            triangulate_with_noise_floor(
                registry,
                store,
                now,
                stale_us,
                &config.triangulation,
                config.evidence.detector_noise_floor_px,
            )
        })
        .flatten();
    let t_cam = match &landmarks_3d {
        Some(PoseLandmarks3D::Triangulated { t_us, .. }) => Some(*t_us),
        _ => None,
    };
    let t_imu = align::reference_time(candidates.iter().map(|c| c.4), now, stale_us);
    let mut imu: Vec<Option<Imu>> = (0..sk.len()).map(|_| None).collect();
    if let Some(t) = t_imu {
        let max_gap = config.time_alignment.max_imu_gap_ms * 1000;
        for (index, id, mounting, entry, _) in &candidates {
            let Some(a) = align::orientation_at(entry.orientations.iter(), t, max_gap) else {
                reasons.push(reason(
                    "imu_gap",
                    Some(id.to_string()),
                    "Tracker samples around the reconstruction time are too far apart to interpolate.",
                ));
                continue;
            };
            let mounted = a.rotation * mounting;
            let heading = if fusion_cfg.optical_heading_correction {
                next.heading.get(*id).map_or(0.0, |h| h.yaw_rad)
            } else {
                0.0
            };
            let age = now.saturating_sub(t) as f32 / stale_us.max(1) as f32;
            imu[*index] = Some(Imu {
                id: (*id).clone(),
                rotation: fusion::corrected(&mounted, heading),
                confidence: a.confidence.map(|c| c * (1.0 - age).clamp(0.0, 1.0)),
                at_camera: t_cam
                    .and_then(|tc| align::orientation_at(entry.orientations.iter(), tc, max_gap))
                    .map(|c| c.rotation * mounting),
            });
        }
    }
    let t_ref = t_imu.or(t_cam);
    let anchor_age = |t: u64| t_ref.map_or(0, |r| r.abs_diff(t));
    // The camera set anchors t_ref only while it is recent enough.
    let camera_current =
        t_cam.is_some_and(|t| anchor_age(t) <= max_anchor_us && now - t <= max_anchor_us);
    let camera_fit: Option<Fit> = landmarks_3d
        .as_ref()
        .filter(|_| camera_current)
        .and_then(|l| fit(sk, l));

    // Arbitration per segment.
    let n = sk.len();
    let mut rotations = vec![Rot::identity(); n];
    let mut observed = vec![false; n];
    let mut sources = vec![vec![]; n];
    let mut confidence = vec![None; n];
    let mut methods = vec![MethodId::RestFill; n];
    let mut t_obs = vec![None; n];
    let mut heading_assumed = false;
    for i in 0..n {
        if let Some(m) = &imu[i] {
            let converged = fusion_cfg.optical_heading_correction
                && next.heading.get(&m.id).is_some_and(|h| h.converged);
            heading_assumed |= !converged;
            rotations[i] = m.rotation;
            observed[i] = true;
            sources[i] = vec![m.id.clone()];
            confidence[i] = m.confidence;
            methods[i] = if converged {
                MethodId::ImuOpticalFused
            } else {
                MethodId::ImuMounted
            };
            t_obs[i] = t_imu;
            if converged
                && let Some(f) = &camera_fit
                && let Some(at_camera) = m.at_camera
                && f.observed[i]
                && f.confidence[i].is_some_and(|c| c >= fusion_cfg.min_optical_confidence)
            {
                let tail = sk.segments[i].tail;
                let heading = next.heading.get(&m.id).map_or(0.0, |h| h.yaw_rad);
                let imu_then = fusion::corrected(&at_camera, heading);
                let between = (imu_then * tail).angle(&(f.rotations[i] * tail));
                if between > fusion_cfg.imu_camera_disagree_deg.to_radians() {
                    reasons.push(reason(
                        "imu_camera_disagree",
                        Some(sk.segments[i].id.to_string()),
                        &format!(
                            "The tracker and the cameras disagree by {:.0}° on this segment; check the strap and the camera calibration.",
                            between.to_degrees()
                        ),
                    ));
                    confidence[i] = confidence[i].map(|c| c * 0.5);
                }
            }
        } else if let Some(f) = camera_fit.as_ref().filter(|f| f.observed[i]) {
            rotations[i] = f.rotations[i];
            observed[i] = true;
            sources[i].clone_from(&f.sources[i]);
            confidence[i] = f.confidence[i];
            methods[i] = MethodId::SkeletonFitAxis;
            t_obs[i] = t_cam;
        }
    }
    // An unobserved segment follows its parent at its rest-relative rotation; never observed.
    for (i, s) in sk.segments.iter().enumerate() {
        if !observed[i] {
            rotations[i] = s.parent.map_or(Rot::identity(), |p| rotations[p]);
        }
    }

    // Optical heading correction, for the next tick.
    if fusion_cfg.optical_heading_correction
        && let (Some(f), Some(t)) = (&camera_fit, t_imu)
    {
        for (i, m) in imu.iter().enumerate() {
            let Some((m, at_camera)) = m.as_ref().and_then(|m| Some((m, m.at_camera?))) else {
                continue;
            };
            if !f.observed[i]
                || f.confidence[i].is_none_or(|c| c < fusion_cfg.min_optical_confidence)
            {
                continue;
            }
            let state = next.heading.entry(m.id.clone()).or_default();
            if let Ok(e) = fusion::heading_error(
                &sk.segments[i].tail,
                &at_camera,
                state.yaw_rad,
                &f.rotations[i],
                fusion_cfg.heading_min_tilt_deg,
            ) {
                let _ = fusion::step(state, e, t, fusion_cfg);
            }
        }
    }

    // Root: the current camera anchor, a held anchor within its age, or floor contact.
    let pelvis_sigma = super::motion::measured_keypoints(landmarks_3d.as_ref())[0]
        .as_ref()
        .and_then(|p| p.sigma_m);
    let fresh_anchor = camera_fit
        .as_ref()
        .map(|f| (f.root, pelvis_sigma, f.confidence[0], f.sources[0].clone()))
        .or_else(|| match &landmarks_3d {
            Some(PoseLandmarks3D::Triangulated {
                schema_id, points, ..
            }) if camera_current
                && (schema_id == LANDMARK_SCHEMA_ID || schema_id == BODY_KEYPOINT_SCHEMA_ID) =>
            {
                sk.segments
                    .iter()
                    .position(|s| s.parent.is_none())
                    .and_then(|i| points.get(i)?.as_ref())
                    .map(|p| {
                        (
                            v3_from_wire(p.m),
                            p.sigma_m,
                            Some(p.confidence),
                            p.used_views.clone(),
                        )
                    })
            }
            _ => None,
        });
    let speed_sigma =
        |age_us: u64| (fusion_cfg.anchor_speed_bound_m_s * age_us as f64 * 1e-6) as f32;
    let mut root_provenance = Provenance::Inferred;
    let mut root_sigma_m = None;
    let mut root_t_obs = None;
    let mut root_sources = vec![];
    let mut root_confidence = None;
    let mut root_held = false;
    let held = registry
        .fusion
        .anchor
        .as_ref()
        .filter(|_| imu.iter().any(Option::is_some))
        .zip(t_ref)
        .filter(|(a, t)| {
            *t >= a.t_us
                && *t - a.t_us <= max_anchor_us
                && now.saturating_sub(a.t_us) <= max_anchor_us
                && a.sources
                    .iter()
                    .all(|id| !store.identity_unconfirmed.contains(id))
        });
    let root = if let (Some((p, sigma, conf, views)), Some(t)) = (fresh_anchor, t_cam) {
        next.anchor = Some(Anchor {
            root: p,
            t_us: t,
            sigma_m: sigma,
            confidence: conf,
            sources: views.clone(),
        });
        root_provenance = Provenance::Measured;
        root_sigma_m = sigma.map(|s| s.hypot(speed_sigma(anchor_age(t))));
        root_t_obs = Some(t);
        root_sources = views;
        root_confidence = conf;
        p
    } else if let Some((a, t)) = held {
        // Held through a short occlusion while trackers keep the limbs live; not a measurement.
        reasons.push(reason(
            "root_anchor_held",
            None,
            "Cameras lost the body; the root is held at its last camera position with growing uncertainty.",
        ));
        root_sigma_m = a.sigma_m.map(|s| s.hypot(speed_sigma(t - a.t_us)));
        root_t_obs = Some(a.t_us);
        root_sources.clone_from(&a.sources);
        root_confidence = a.confidence;
        root_held = true;
        a.root
    } else {
        next.anchor = None;
        floor_contact_root(sk, &rotations)
    };

    let observed_count = observed.iter().filter(|o| **o).count() as u32;
    let any_input = has_trackers || camera_fit.is_some();
    let state = if !any_input {
        TrackingState::Unavailable
    } else if observed_count == 0 {
        TrackingState::Acquiring
    } else if observed_count < n as u32 {
        TrackingState::Degraded
    } else {
        TrackingState::Tracking
    };
    if !any_input {
        reasons.push(reason(
            "no_orientation_devices",
            None,
            "No orientation trackers are configured; camera landmarks do not provide a world skeleton.",
        ));
    }
    if heading_assumed {
        reasons.push(reason(
            "heading_alignment_assumed",
            None,
            "Heading alignment is assumed to be identity.",
        ));
    }
    let positions = forward_kinematics(sk, &rotations, root);
    // Uncalibrated but live trackers still give the (inferred) rest skeleton.
    let fresh_orientation = has_trackers
        && registry.entries.values().any(|e| {
            e.latest
                .get(&ObservationKind::Orientation)
                .is_some_and(|o| o.t_us <= now && now - o.t_us <= stale_us)
        });
    let pose = (fresh_orientation || camera_fit.is_some()).then(|| SkeletonPose {
        skeleton_rev: sk.definition.rev,
        root_position_m: v3_to_wire(&root),
        root_position_provenance: root_provenance,
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
                method: Some(methods[i]),
                t_obs_us: t_obs[i],
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
            total_segments: n as u32,
            reasons,
        },
        landmarks_3d,
        methods,
        t_obs,
        t_ref,
        root_sigma_m,
        root_t_obs,
        root_sources,
        root_confidence,
        root_held,
        fusion_next: next,
    }
}
