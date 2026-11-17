//! A realistic `SystemState`: simulated trackers in mixed states, one camera, mixed calibration.

use bansayo_contracts::*;

use super::skeleton::{self, ASSIGNED};
use super::{dev, profile, reason, seg, session_id, src};

pub const ENGINE_VERSION: &str = "0.1.0";
pub const CAMERA: &str = "SIM-CAM1";
pub const LANDMARK_SCHEMA: &str = "bansayo.body.v1.joints";

/// Landmarks at every segment origin, edges along the parent links.
pub fn landmark_schema() -> LandmarkSchema {
    let segs = skeleton::segments();
    let index = |id: &str| segs.iter().position(|s| s.id == id).expect("segment") as u32;
    LandmarkSchema {
        id: LANDMARK_SCHEMA.into(),
        names: segs.iter().map(|s| s.id.clone()).collect(),
        edges: segs
            .iter()
            .filter_map(|s| s.parent.as_deref().map(|p| [index(p), index(&s.id)]))
            .collect(),
    }
}

pub fn tracker_descriptor(id: &str) -> DeviceDescriptor {
    DeviceDescriptor {
        id: dev(id),
        source_id: src("sim"),
        kind: DeviceKind::ImuTracker,
        origin: Origin::Simulated,
        model: Some("Simulated IMU".into()),
        firmware: None,
        transport: Transport::Simulated,
        capabilities: vec![
            SensorCapability::Orientation,
            SensorCapability::Battery,
            SensorCapability::SignalStrength,
            SensorCapability::Quality,
        ],
        nominal_rate_hz: Some(100.0),
        camera: None,
    }
}

pub fn camera_descriptor() -> DeviceDescriptor {
    DeviceDescriptor {
        id: dev(CAMERA),
        source_id: src("sim"),
        kind: DeviceKind::Camera,
        origin: Origin::Simulated,
        model: Some("Simulated camera".into()),
        firmware: None,
        transport: Transport::Simulated,
        capabilities: vec![SensorCapability::Image, SensorCapability::Landmarks2d],
        nominal_rate_hz: Some(30.0),
        camera: Some(CameraInfo {
            resolution_px: [1280, 720],
            landmark_schema: Some(landmark_schema()),
        }),
    }
}

fn mounting_id(device: &str, segment: &str) -> String {
    format!("mounting-{device}-{segment}")
}

fn cal(kind: CalibrationKind, state: CalibrationState, profile_id: Option<&str>) -> CalibrationRef {
    CalibrationRef {
        kind,
        state,
        profile_id: profile_id.map(profile),
    }
}

fn heading_assumed(id: &str) -> Reason {
    reason(
        "heading_alignment_assumed",
        Some(id),
        "Heading alignment has not been calibrated; identity is assumed.",
    )
}

fn tracker(index: usize, state: TrackingState, mounting: CalibrationState) -> DeviceStatus {
    let (id, segment) = ASSIGNED[index];
    let connected = state != TrackingState::Disconnected;
    let degraded = state == TrackingState::Degraded;
    let mut reasons = vec![heading_assumed(id)];
    match state {
        TrackingState::Disconnected => reasons.insert(
            0,
            reason(
                "device_disconnected",
                Some(id),
                &format!("Tracker {id} is disconnected."),
            ),
        ),
        TrackingState::Degraded => reasons.insert(
            0,
            reason(
                "device_degraded",
                Some(id),
                &format!("Tracker {id} reports low quality."),
            ),
        ),
        _ => {}
    }
    if mounting == CalibrationState::Stale {
        reasons.push(reason(
            "mounting_calibration_stale",
            Some(id),
            "Mounting calibration is out of date; capture the reference pose again.",
        ));
    }
    let profile_id = mounting_id(id, segment);
    let profile_ref = (mounting != CalibrationState::Missing).then_some(profile_id.as_str());
    DeviceStatus {
        descriptor: tracker_descriptor(id),
        state,
        reasons,
        assignment: Some(seg(segment)),
        stream: StreamStatus {
            measured_rate_hz: if connected { 99.8 } else { 0.0 },
            last_seen_t_us: Some(if connected { 303_829_000 } else { 281_400_000 }),
            received: 30_311 - index as u64 * 1_207,
            dropped: if connected { index as u64 } else { 12 },
            time_quality: TimeQuality::Estimated,
        },
        battery: connected.then_some(BatteryStatus {
            level: 0.82,
            charging: Some(false),
        }),
        signal: connected.then_some(SignalStatus { rssi_dbm: -54 }),
        quality: connected.then_some(if degraded { 0.3 } else { 0.97 }),
        calibration: vec![
            cal(CalibrationKind::Mounting, mounting, profile_ref),
            cal(
                CalibrationKind::HeadingAlignment,
                CalibrationState::Assumed,
                None,
            ),
        ],
    }
}

pub fn camera_status() -> DeviceStatus {
    DeviceStatus {
        descriptor: camera_descriptor(),
        state: TrackingState::Tracking,
        reasons: vec![],
        assignment: None,
        stream: StreamStatus {
            measured_rate_hz: 30.0,
            last_seen_t_us: Some(303_800_000),
            received: 9_114,
            dropped: 0,
            time_quality: TimeQuality::Estimated,
        },
        battery: None,
        signal: None,
        quality: None,
        calibration: vec![
            cal(
                CalibrationKind::CameraIntrinsics,
                CalibrationState::Valid,
                Some("camera-intrinsics-SIM-CAM1"),
            ),
            cal(
                CalibrationKind::CameraExtrinsics,
                CalibrationState::Valid,
                Some("camera-extrinsics-SIM-CAM1"),
            ),
        ],
    }
}

pub fn devices() -> Vec<DeviceStatus> {
    vec![
        tracker(0, TrackingState::Tracking, CalibrationState::Valid),
        tracker(1, TrackingState::Tracking, CalibrationState::Valid),
        tracker(2, TrackingState::Degraded, CalibrationState::Stale),
        tracker(3, TrackingState::Disconnected, CalibrationState::Missing),
        camera_status(),
    ]
}

fn summary(
    id: &str,
    kind: CalibrationKind,
    state: CalibrationState,
    device: Option<&str>,
    segment: Option<&str>,
    method: &str,
    created_utc: Option<&str>,
) -> CalibrationProfileSummary {
    CalibrationProfileSummary {
        id: profile(id),
        kind,
        state,
        device_id: device.map(dev),
        segment_id: segment.map(seg),
        method: method.into(),
        created_utc: created_utc.map(Into::into),
        data: Default::default(),
    }
}

pub fn calibration() -> Vec<CalibrationProfileSummary> {
    use CalibrationKind as K;
    use CalibrationState as S;
    vec![
        summary(
            "mounting-SIM-T01-pelvis",
            K::Mounting,
            S::Valid,
            Some("SIM-T01"),
            Some("pelvis"),
            "reference_pose",
            Some("2026-11-16T10:22:05Z"),
        ),
        summary(
            "mounting-SIM-T02-chest",
            K::Mounting,
            S::Valid,
            Some("SIM-T02"),
            Some("chest"),
            "reference_pose",
            Some("2026-11-16T10:22:05Z"),
        ),
        summary(
            "mounting-SIM-T03-left_thigh",
            K::Mounting,
            S::Stale,
            Some("SIM-T03"),
            Some("left_thigh"),
            "reference_pose",
            Some("2026-11-16T10:22:05Z"),
        ),
        summary(
            "heading-alignment-default",
            K::HeadingAlignment,
            S::Assumed,
            None,
            None,
            "default",
            None,
        ),
        summary(
            "body-proportions-default",
            K::BodyProportions,
            S::Assumed,
            None,
            None,
            "default",
            None,
        ),
        summary(
            "camera-intrinsics-SIM-CAM1",
            K::CameraIntrinsics,
            S::Valid,
            Some(CAMERA),
            None,
            "declared_by_simulator",
            Some("2026-11-16T10:17:01Z"),
        ),
        summary(
            "camera-extrinsics-SIM-CAM1",
            K::CameraExtrinsics,
            S::Valid,
            Some(CAMERA),
            None,
            "declared_by_simulator",
            Some("2026-11-16T10:17:01Z"),
        ),
    ]
}

fn feature(id: FeatureId, state: FeatureState, reasons: Vec<Reason>) -> FeatureStatus {
    FeatureStatus { id, state, reasons }
}

fn not_implemented(id: FeatureId, subject: &str, what: &str) -> FeatureStatus {
    feature(
        id,
        FeatureState::Unavailable,
        vec![reason(
            "not_implemented",
            Some(subject),
            &format!("{what} is not implemented in this build."),
        )],
    )
}

pub fn features() -> Vec<FeatureStatus> {
    use FeatureId as F;
    use FeatureState as S;
    vec![
        feature(
            F::SegmentOrientation,
            S::Degraded,
            vec![
                reason(
                    "mounting_calibration_stale",
                    Some("SIM-T03"),
                    "Mounting calibration of SIM-T03 is out of date.",
                ),
                reason(
                    "device_disconnected",
                    Some("SIM-T04"),
                    "Tracker SIM-T04 is disconnected.",
                ),
            ],
        ),
        feature(F::SkeletonPose, S::Available, vec![]),
        feature(
            F::JointAngles,
            S::Degraded,
            vec![reason(
                "segment_not_observed",
                Some("left_shin"),
                "Left shin is not observed.",
            )],
        ),
        feature(
            F::RootPosition,
            S::Degraded,
            vec![reason(
                "root_position_inferred",
                None,
                "Root position is inferred from floor contact.",
            )],
        ),
        feature(F::Landmarks2d, S::Available, vec![]),
        feature(
            F::Landmarks3dTriangulated,
            S::Unavailable,
            vec![reason(
                "insufficient_cameras",
                None,
                "Triangulation needs at least two cameras.",
            )],
        ),
        not_implemented(
            F::CameraImuFusion,
            "camera_imu_fusion",
            "Camera and IMU fusion",
        ),
        not_implemented(F::GroundContact, "ground_contact", "Ground contact sensing"),
        feature(F::Recording, S::Available, vec![]),
        feature(F::Playback, S::Available, vec![]),
    ]
}

pub fn simulation() -> SimulationState {
    let motions = [
        ("reference_pose", "Reference pose"),
        ("standing", "Standing"),
        ("squat", "Squat"),
        ("knee_flexion", "Knee flexion"),
        ("arm_raise", "Arm raise"),
        ("front_kick", "Front kick"),
    ];
    SimulationState {
        source_id: src("sim"),
        motion: "standing".into(),
        available_motions: motions
            .iter()
            .map(|(id, label)| MotionOption {
                id: (*id).into(),
                label: (*label).into(),
            })
            .collect(),
        speed: 1.0,
        paused: false,
    }
}

pub fn recording_session() -> RecordingSession {
    RecordingSession {
        id: session_id("20261116T102205Z-a3f9"),
        name: "Squat set 1".into(),
        created_utc: "2026-11-16T10:22:05Z".into(),
        duration_us: 42_500_000,
        size_bytes: 18_734_112,
        origins: vec![Origin::Simulated],
        device_count: 5,
        engine_version: ENGINE_VERSION.into(),
        protocol_version: PROTOCOL_VERSION,
    }
}

pub fn system_state() -> SystemState {
    SystemState {
        capability_report: None,
        rev: 412,
        engine: EngineInfo {
            version: ENGINE_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
            mode: EngineMode::Live,
            state: TrackingState::Degraded,
            uptime_s: 303.8,
        },
        devices: devices(),
        calibration: calibration(),
        features: features(),
        recording: None,
        playback: None,
        simulation: Some(simulation()),
    }
}

pub fn system_state_replay() -> SystemState {
    SystemState {
        capability_report: None,
        rev: 431,
        engine: EngineInfo {
            mode: EngineMode::Replay,
            uptime_s: 341.2,
            ..system_state().engine
        },
        playback: Some(PlaybackState {
            session: recording_session(),
            paused: false,
            speed: 1.0,
            position_us: 12_250_000,
            looping: true,
        }),
        simulation: None,
        ..system_state()
    }
}
