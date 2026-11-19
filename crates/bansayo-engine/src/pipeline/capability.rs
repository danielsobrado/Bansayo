//! Capability projection of the existing feature rules and current contributing evidence.
use super::{
    features::{features, unconsumed},
    motion::{evidence, measured_keypoints, sample},
    solve::Solved,
};
use crate::{
    calibration::{Store, valid_profile},
    config::Config,
    math::hash::{canonical_id, sha256},
    registry::{ObservationKind, Registry, reason},
    skeleton::Skeleton,
};
use bansayo_contracts::*;
use serde_json::json;
fn rank(p: CapabilityProfileId) -> u8 {
    match p {
        CapabilityProfileId::None => 0,
        CapabilityProfileId::Basic => 1,
        CapabilityProfileId::Inertial => 2,
        CapabilityProfileId::MultiView => 3,
        CapabilityProfileId::MarkerAssisted => 4,
        CapabilityProfileId::Hybrid => 5,
        CapabilityProfileId::Extended => 6,
    }
}
fn availability(
    class: InputClass,
    target: &str,
    e: &Evidence,
    reg: &Registry,
) -> InputAvailability {
    let state = if matches!(e.provenance, Provenance::Measured | Provenance::Derived) {
        FeatureState::Available
    } else {
        FeatureState::Unavailable
    };
    let rate = e
        .sources
        .iter()
        .filter_map(|id| reg.entries.get(id)?.descriptor.nominal_rate_hz)
        .reduce(f32::min);
    InputAvailability {
        class,
        target: target.into(),
        state,
        methods: if state == FeatureState::Available {
            vec![e.method]
        } else {
            vec![]
        },
        rate_hz: rate,
        reasons: if state == FeatureState::Available {
            vec![]
        } else {
            vec![reason(
                if class == InputClass::W {
                    "not_implemented"
                } else if class == InputClass::R {
                    "twist_not_observed"
                } else {
                    "segment_not_observed"
                },
                Some(target.into()),
                "The required input is not currently observed.",
            )]
        },
    }
}
pub fn report(
    sk: &Skeleton,
    reg: &Registry,
    store: &Store,
    solved: &Solved,
    config: &Config,
    now: u64,
) -> CapabilityReport {
    let devices = reg.status(now, &config.quality, store);
    let table = features(&devices, solved);
    let body = sample(sk, reg, solved, 0, now, vec![]).body;
    let measured = measured_keypoints(solved.landmarks_3d.as_ref());
    let mut inputs = vec![];
    for (i, name) in BODY_KEYPOINT_NAMES.iter().enumerate() {
        let e = if let Some(p) = &measured[i] {
            evidence(
                Provenance::Measured,
                MethodId::Triangulated,
                Some(p.confidence),
                None,
                p.used_views.clone(),
            )
        } else {
            body.as_ref()
                .and_then(|b| b.keypoints.get(i)?.as_ref())
                .map(|p| p.evidence.clone())
                .unwrap_or_else(|| {
                    evidence(Provenance::Unavailable, MethodId::Fk, None, None, vec![])
                })
        };
        inputs.push(availability(InputClass::P, name, &e, reg));
    }
    for (i, def) in sk.segments.iter().enumerate() {
        let absent = evidence(
            Provenance::Unavailable,
            MethodId::RestFill,
            None,
            None,
            vec![],
        );
        let s = body.as_ref().and_then(|b| b.segments.get(i));
        for (class, e) in [
            (InputClass::A, s.map_or(&absent, |s| &s.axis)),
            (InputClass::R, s.map_or(&absent, |s| &s.twist)),
            (InputClass::W, s.map_or(&absent, |s| &s.angular_velocity)),
        ] {
            inputs.push(availability(class, def.id.as_str(), e, reg));
        }
    }
    let mut image_available = false;
    for entry in reg.entries.values() {
        if !entry.connected {
            continue;
        }
        if let Some(o) = entry
            .latest
            .get(&ObservationKind::Landmarks2d)
            .filter(|o| o.t_us <= now && now - o.t_us <= config.engine.stale_after_ms * 1000)
            && let ObservationData::Landmarks2d { landmarks } = &o.data
        {
            let names = entry
                .descriptor
                .camera
                .as_ref()
                .and_then(|c| c.landmark_schema.as_ref())
                .map(|s| &s.names);
            for (i, p) in landmarks.points.iter().enumerate() {
                let Some(p) = p.as_ref().filter(|p| p.confidence > 0.0) else {
                    continue;
                };
                let target = if landmarks.schema_id
                    == crate::skeleton::proportions::LANDMARK_SCHEMA_ID
                    || landmarks.schema_id == BODY_KEYPOINT_SCHEMA_ID
                {
                    BODY_KEYPOINT_NAMES.get(i).map(|s| s.to_string())
                } else {
                    names.and_then(|n| n.get(i)).cloned()
                };
                if let Some(target) = target {
                    image_available = true;
                    let e = evidence(
                        Provenance::Derived,
                        MethodId::ImagePlane,
                        Some(p.confidence),
                        Some(o.t_us),
                        vec![entry.descriptor.id.clone()],
                    );
                    inputs.push(availability(InputClass::I, &target, &e, reg));
                }
            }
        }
    }
    let limb_count = [6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 18]
        .into_iter()
        .filter(|i| measured[*i].is_some())
        .count();
    let multi = measured[0].is_some() && limb_count >= 8;
    let imu = inputs.iter().any(|i| {
        i.class == InputClass::R
            && i.state == FeatureState::Available
            && i.methods.contains(&MethodId::ImuMounted)
    });
    let automatic = if multi && imu {
        CapabilityProfileId::Hybrid
    } else if multi {
        CapabilityProfileId::MultiView
    } else if measured.iter().all(Option::is_none) && imu {
        CapabilityProfileId::Inertial
    } else if measured.iter().all(Option::is_none) && image_available {
        CapabilityProfileId::Basic
    } else {
        CapabilityProfileId::None
    };
    let profile = config
        .tracking
        .profile_override
        .filter(|p| rank(*p) <= rank(automatic))
        .unwrap_or(automatic);
    let measurements = super::joints::measure(sk, &solved.rotations, &solved.observed);
    let mut observables: Vec<_> = sk
        .definition
        .joints
        .iter()
        .zip(measurements)
        .enumerate()
        .map(|(i, (j, m))| {
            let method = body.as_ref().and_then(|b| {
                let a = &b.segments[sk.joints[i].a].axis;
                let z = &b.segments[sk.joints[i].b].axis;
                (m.value.is_some()).then_some(if a.method == z.method {
                    a.method
                } else {
                    MethodId::SkeletonFitAxis
                })
            });
            ObservableAvailability {
                id: j.id.clone(),
                state: if m.value.is_some() {
                    FeatureState::Available
                } else {
                    FeatureState::Unavailable
                },
                method,
                reasons: if m.value.is_none() && m.reasons.is_empty() {
                    table
                        .iter()
                        .find(|f| f.id == FeatureId::JointAngles)
                        .map_or_else(Vec::new, |f| f.reasons.clone())
                } else {
                    m.reasons
                },
            }
        })
        .collect();
    // Reason-only entries for declared streams without a consumer; no new observable is computed.
    for d in &devices {
        for capability in &d.descriptor.capabilities {
            if let Some(mut why) = unconsumed(*capability) {
                why.subject = Some(d.descriptor.id.to_string());
                let name = serde_json::to_value(capability).expect("capability enum");
                observables.push(ObservableAvailability {
                    id: format!(
                        "input.{}.{}",
                        d.descriptor.id,
                        name.as_str().unwrap_or_default()
                    ),
                    state: FeatureState::Unavailable,
                    method: None,
                    reasons: vec![why],
                });
            }
        }
    }
    let (tracking_body, calibration_body) = identity_bodies(&devices, reg, store, config);
    let tracking_config_id = canonical_id(&tracking_body);
    CapabilityReport {
        profile,
        profile_override: config.tracking.profile_override,
        tracking_config_id,
        calibration_set_id: canonical_id(&calibration_body),
        inputs,
        observables,
    }
}

/// The bodies whose hashes are `tracking_config_id` and `calibration_set_id` (12 §4, 32 §3).
pub fn identity_bodies(
    devices: &[DeviceStatus],
    reg: &Registry,
    store: &Store,
    config: &Config,
) -> (serde_json::Value, serde_json::Value) {
    let device_config:Vec<_>=devices.iter().map(|d|json!({"id":d.descriptor.id,"kind":d.descriptor.kind,"model":d.descriptor.model,"resolution":d.descriptor.camera.as_ref().map(|c|c.resolution_px),"nominal_rate":d.descriptor.nominal_rate_hz})).collect();
    let detectors:Vec<_>=devices.iter().filter_map(|d|d.descriptor.camera.as_ref().map(|c|json!({"device":d.descriptor.id,"schema":c.landmark_schema.as_ref().map(|s|&s.id),"detector":detector_id(config,&d.descriptor)}))).collect();
    let camera_processing:Vec<_>=config.sources.iter().filter_map(|s|match s {crate::config::SourceConfig::Camera(c) if c.enabled=>Some(json!({"id":c.id,"model":crate::sources::camera::pose::DETECTOR_NAME,"execution":c.execution,"min_person_score":c.min_person_score,"min_keypoint_score":c.min_keypoint_score,"smoothing":{"filter":"none"},"latencies":c.cameras.iter().map(|c|json!({"id":c.id,"latency_ms":c.latency_ms})).collect::<Vec<_>>()})),_=>None}).collect();
    let version = env!("CARGO_PKG_VERSION")
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    let tracking = json!({"devices":device_config,"detectors":detectors,"camera_processing":camera_processing,"assignments":reg.assignments,"triangulation":config.triangulation,"gates":null,"fusion":{"method":crate::pipeline::fusion::METHOD_VERSION,"mode":config.tracking.mode,"time_alignment":config.time_alignment,"settings":config.fusion},"analysis":config.analysis(),"recognition":config.recognition(),"observable_catalogue":"bansayo.observables.v1","engine_version":version});
    let profiles:Vec<_>=store.profiles.values().filter(|p|valid_profile(p)&&!store.stale.contains(&p.id)&&p.subject.device_id.as_ref().is_some_and(|id|reg.entries.contains_key(id)&& (p.kind!=CalibrationKind::Mounting||p.subject.segment_id.as_ref()==reg.assignments.get(id)))).map(|p|json!({"id":p.id,"hash":sha256(serde_json::to_value(p).expect("profile JSON").to_string().as_bytes())})).collect();
    (tracking, json!(profiles))
}

/// Pose-model identity of a camera device (12 §5.3 `detector_id`).
pub fn detector_id(config: &Config, d: &DeviceDescriptor) -> String {
    if d.origin != Origin::Simulated {
        return crate::sources::camera::pose::DETECTOR_NAME.into();
    }
    config
        .sources
        .iter()
        .find_map(|s| match s {
            crate::config::SourceConfig::Simulated(s)
                if s.cameras.iter().any(|c| c.id == d.id.as_str()) =>
            {
                Some(
                    s.detector_id
                        .clone()
                        .unwrap_or_else(|| SIMULATED_DETECTOR.into()),
                )
            }
            _ => None,
        })
        .unwrap_or_else(|| SIMULATED_DETECTOR.into())
}

pub const SIMULATED_DETECTOR: &str = "simulated-projection";
