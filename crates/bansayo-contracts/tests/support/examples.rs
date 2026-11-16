//! Every golden example in `contracts/examples/`, built from Rust values.

use std::f32::consts::FRAC_1_SQRT_2;

use bansayo_contracts::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::skeleton::{self, ASSIGNED, HEIGHT_M};
use super::state::{self, CAMERA, LANDMARK_SCHEMA};
use super::{dev, profile, reason, seg, session_id, src};

/// One golden file: its name, its exact text, and a round trip through its Rust type.
pub struct Example {
    pub file: String,
    /// Pretty JSON written straight from the Rust value, so fields keep declaration order.
    pub text: String,
    pub round_trip: fn(Value) -> serde_json::Result<Value>,
}

impl Example {
    pub fn value(&self) -> Value {
        serde_json::from_str(&self.text).expect("example parses")
    }
}

/// Converts through JSON text: `serde_json::to_value` widens `f32` to `f64` (0.9275 becomes
/// 0.9275000095367432), while the wire format writes the shortest `f32` representation.
fn to_json<T: Serialize>(value: &T) -> serde_json::Result<Value> {
    serde_json::from_str(&serde_json::to_string(value)?)
}

fn ex<T: Serialize + DeserializeOwned>(name: &str, value: T) -> Example {
    Example {
        file: format!("{name}.json"),
        text: serde_json::to_string_pretty(&value).expect("example serialises") + "\n",
        round_trip: |v| to_json(&serde_json::from_value::<T>(v)?),
    }
}

const T_US: TimeUs = 303_833_333;
const IDENTITY: Quat = [0.0, 0.0, 0.0, 1.0];
/// 90° about +Z, normalised to f32 precision.
const QUARTER_TURN_Z: Quat = [0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2];

fn hello() -> ServerMessage {
    ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        engine_version: state::ENGINE_VERSION.into(),
        session_epoch_utc: "2026-11-16T10:17:01Z".into(),
        conventions: Conventions::default(),
        skeleton: skeleton::definition(),
        state: state::system_state(),
    }
}

fn joint(value: Option<f32>, missing: Option<&str>) -> JointMeasurement {
    match (value, missing) {
        (Some(v), _) => JointMeasurement {
            value: Some(v),
            provenance: Provenance::Derived,
            confidence: Some(0.95),
            reasons: vec![],
        },
        (None, subject) => JointMeasurement {
            value: None,
            provenance: Provenance::Unavailable,
            confidence: None,
            reasons: vec![match subject {
                Some(s) => reason(
                    "segment_not_observed",
                    Some(s),
                    &format!("Segment {s} is not observed."),
                ),
                None => reason(
                    "no_orientation_source",
                    None,
                    "No device provides orientation.",
                ),
            }],
        },
    }
}

/// Joint measurements for the assigned trackers in the rest pose, in definition order.
fn rest_joints() -> Vec<JointMeasurement> {
    let observed = |s: &str| ASSIGNED.iter().any(|(_, a)| *a == s);
    skeleton::definition()
        .joints
        .iter()
        .map(
            |j| match j.segments.iter().find(|s| !observed(s.as_str())) {
                None => joint(Some(0.0), None),
                Some(missing) => joint(None, Some(missing.as_str())),
            },
        )
        .collect()
}

fn frame() -> ServerMessage {
    let segments = skeleton::rest_pose_segments();
    ServerMessage::Frame {
        seq: 18_230,
        t_us: T_US,
        origins: vec![Origin::Simulated],
        pose: Some(SkeletonPose {
            skeleton_rev: skeleton::SKELETON_REV,
            root_position_m: segments[0].position_m,
            root_position_provenance: Provenance::Inferred,
            segments,
        }),
        joints: rest_joints(),
        quality: TrackingQuality {
            state: TrackingState::Tracking,
            observed_segments: ASSIGNED.len() as u32,
            total_segments: 19,
            reasons: vec![
                reason(
                    "root_position_inferred",
                    None,
                    "Root position is inferred from floor contact.",
                ),
                reason(
                    "body_proportions_assumed",
                    None,
                    "Body proportions are population averages for the entered height.",
                ),
            ],
        },
    }
}

fn frame_no_pose() -> ServerMessage {
    let joints = skeleton::definition()
        .joints
        .iter()
        .map(|_| joint(None, None))
        .collect();
    ServerMessage::Frame {
        seq: 412,
        t_us: 6_866_666,
        origins: vec![Origin::Simulated],
        pose: None,
        joints,
        quality: TrackingQuality {
            state: TrackingState::Unavailable,
            observed_segments: 0,
            total_segments: 19,
            reasons: vec![reason(
                "no_orientation_source",
                None,
                "No device provides orientation.",
            )],
        },
    }
}

fn frame_meta() -> CameraFrameMetadata {
    CameraFrameMetadata {
        device_id: dev(CAMERA),
        frame_seq: 9_114,
        t_us: 303_800_000,
        device_time_us: Some(303_712_345),
        time_quality: TimeQuality::Estimated,
        image_size_px: [1280, 720],
    }
}

/// Pinhole projection of the rest pose from the simulated camera at (0, 1.2, 3.2) looking at -Z,
/// 50° vertical field of view. Hands are reported as not detected.
fn landmarks_2d() -> PoseLandmarks2D {
    let (w, h, cam_y, cam_z) = (1280.0, 720.0, 1.2, 3.2);
    let f = (h / 2.0) / (25.0_f64).to_radians().tan();
    let segs = skeleton::segments();
    let points = skeleton::rest_positions()
        .iter()
        .zip(&segs)
        .map(|(p, s)| {
            let depth = cam_z - p[2];
            let px = |v: f64| ((v * 10.0).round() / 10.0) as f32;
            (!s.id.ends_with("_hand")).then(|| Landmark2D {
                x_px: px(w / 2.0 + f * p[0] / depth),
                y_px: px(h / 2.0 - f * (p[1] - cam_y) / depth),
                confidence: 0.98,
            })
        })
        .collect();
    PoseLandmarks2D {
        frame_meta: frame_meta(),
        frame: FrameRef::Image { id: dev(CAMERA) },
        origin: Origin::Simulated,
        schema_id: LANDMARK_SCHEMA.into(),
        detector: "simulated-projection".into(),
        points,
    }
}

fn observation(device: &str, seq: u64, data: ObservationData) -> SensorObservation {
    SensorObservation {
        device_id: dev(device),
        source_id: src("sim"),
        origin: Origin::Simulated,
        seq,
        t_us: 303_829_000,
        device_time_us: Some(303_741_210),
        time_quality: TimeQuality::Estimated,
        calibration_ref: None,
        confidence: Some(0.97),
        data,
    }
}

fn sensor_ref(d: &str) -> FrameRef {
    FrameRef::SensorReference { id: dev(d) }
}

fn sensor(d: &str) -> FrameRef {
    FrameRef::Sensor { id: dev(d) }
}

fn triangulated_landmarks() -> PoseLandmarks3D {
    let points = (0..2)
        .map(|i| {
            Some(TriangulatedLandmark3D {
                m: [0.0, 0.9275 + 0.105 * i as f32, 0.0],
                views: 2,
                reprojection_error_px: 1.4,
                confidence: 0.9,
                used_views: vec![dev("SIM-CAM1"), dev("SIM-CAM2")],
                rejected_views: vec![],
            })
        })
        .chain([None])
        .collect();
    PoseLandmarks3D::Triangulated {
        t_us: 303_800_000,
        frame: FrameRef::World,
        origin: Origin::Simulated,
        schema_id: LANDMARK_SCHEMA.into(),
        calibration_refs: vec![
            profile("camera-extrinsics-SIM-CAM1"),
            profile("camera-extrinsics-SIM-CAM2"),
        ],
        points,
    }
}

fn observations() -> Vec<Example> {
    use ObservationData as D;
    let schema_len = state::landmark_schema().names.len();
    let model_points = (0..schema_len)
        .map(|i| {
            (i != 3).then_some(ModelLandmark3D {
                p: [0.0, 0.1 * i as f32, 0.0],
                confidence: 0.8,
            })
        })
        .collect();
    let obs = |name: &str, device: &str, data| {
        ex(
            &format!("sensor_observation_{name}"),
            observation(device, 30_311, data),
        )
    };
    vec![
        obs(
            "orientation",
            "SIM-T01",
            D::Orientation {
                frame: sensor_ref("SIM-T01"),
                q: QUARTER_TURN_Z,
            },
        ),
        obs(
            "angular_velocity",
            "SIM-T01",
            D::AngularVelocity {
                frame: sensor("SIM-T01"),
                rad_s: [0.01, -0.02, 0.5],
            },
        ),
        obs(
            "acceleration",
            "SIM-T01",
            D::Acceleration {
                frame: sensor("SIM-T01"),
                m_s2: [0.1, 9.81, -0.2],
            },
        ),
        obs(
            "position",
            "SIM-T01",
            D::Position {
                frame: FrameRef::World,
                m: [0.0, 0.9275, 0.0],
            },
        ),
        obs("distance", "SIM-T01", D::Distance { m: 0.42 }),
        obs(
            "contact",
            "SIM-T01",
            D::Contact {
                in_contact: true,
                force_n: None,
            },
        ),
        obs(
            "landmarks_2d",
            CAMERA,
            D::Landmarks2d {
                landmarks: landmarks_2d(),
            },
        ),
        obs(
            "landmarks_3d_model_relative",
            CAMERA,
            D::Landmarks3d {
                landmarks: PoseLandmarks3D::ModelRelative {
                    frame_meta: frame_meta(),
                    frame: FrameRef::Model { id: dev(CAMERA) },
                    origin: Origin::Simulated,
                    schema_id: LANDMARK_SCHEMA.into(),
                    detector: "simulated-lifting".into(),
                    points: model_points,
                },
            },
        ),
        obs(
            "landmarks_3d_triangulated",
            CAMERA,
            D::Landmarks3d {
                landmarks: triangulated_landmarks(),
            },
        ),
        obs(
            "diagnostics",
            "SIM-T01",
            D::Diagnostics {
                battery_level: Some(0.82),
                charging: Some(false),
                rssi_dbm: Some(-54),
                quality: Some(0.97),
            },
        ),
    ]
}

fn source_events() -> Vec<Example> {
    vec![
        ex(
            "source_event_device_announced",
            SourceEvent::DeviceAnnounced {
                descriptor: state::tracker_descriptor("SIM-T01"),
            },
        ),
        ex(
            "source_event_device_link",
            SourceEvent::DeviceLink {
                device_id: dev("SIM-T04"),
                connected: false,
                t_us: 282_400_000,
            },
        ),
        ex(
            "source_event_observation",
            SourceEvent::Observation {
                observation: observation(
                    "SIM-T01",
                    30_311,
                    ObservationData::Orientation {
                        frame: sensor_ref("SIM-T01"),
                        q: QUARTER_TURN_Z,
                    },
                ),
            },
        ),
    ]
}

fn calibration_profile() -> CalibrationProfile {
    let object = |v: Value| match v {
        Value::Object(map) => map,
        other => panic!("expected an object, got {other}"),
    };
    let data = MountingData {
        q_sensor_from_segment: [0.0, 0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    };
    let quality = MountingQuality {
        capture_window_ms: 1000,
        angular_spread_rad: 0.002,
    };
    CalibrationProfile {
        schema_version: CALIBRATION_PROFILE_SCHEMA_VERSION,
        id: profile("mounting-SIM-T01-pelvis"),
        kind: CalibrationKind::Mounting,
        method: "reference_pose".into(),
        origin: Origin::Simulated,
        created_utc: "2026-11-16T10:22:05Z".into(),
        subject: CalibrationSubject {
            device_id: Some(dev("SIM-T01")),
            segment_id: Some(seg("pelvis")),
        },
        depends_on: vec![CalibrationDependency {
            kind: CalibrationKind::HeadingAlignment,
            device_id: Some(dev("SIM-T01")),
        }],
        data: object(to_json(&data).expect("mounting data")),
        quality: object(to_json(&quality).expect("mounting quality")),
    }
}

fn api() -> Vec<Example> {
    vec![
        ex("api_ok", ApiResponse::ok(state::simulation())),
        ex(
            "api_error",
            ApiResponse::<RecordingState>::err(ApiError {
                code: ApiErrorCode::Conflict,
                message: "Recording cannot start while a session is being replayed.".into(),
                details: None,
            }),
        ),
        ex(
            "api_request_assignment",
            AssignmentRequest {
                segment: Some(seg("left_shin")),
            },
        ),
        ex(
            "api_request_reference_pose",
            ReferencePoseRequest {
                device_ids: Some(vec![dev("SIM-T01"), dev("SIM-T02")]),
            },
        ),
        ex(
            "api_request_simulation",
            SimulationUpdateRequest {
                motion: Some("squat".into()),
                speed: Some(0.5),
                paused: Some(false),
            },
        ),
        ex(
            "api_request_fault",
            FaultRequest {
                action: FaultAction::Degrade,
                quality: Some(0.3),
            },
        ),
        ex(
            "api_request_recording_start",
            RecordingStartRequest {
                name: Some("Squat set 1".into()),
            },
        ),
        ex(
            "api_request_playback_start",
            PlaybackStartRequest {
                session_id: session_id("20261116T102205Z-a3f9"),
                speed: Some(1.0),
                looping: Some(true),
            },
        ),
        ex(
            "api_request_playback_update",
            PlaybackUpdateRequest {
                paused: Some(true),
                speed: Some(0.25),
                position_us: Some(12_250_000),
            },
        ),
    ]
}

pub fn all() -> Vec<Example> {
    let skeleton = skeleton::definition();
    debug_assert_eq!(skeleton.height_m, HEIGHT_M as f32);
    let mut out = vec![
        ex("hello", hello()),
        ex("skeleton", ServerMessage::Skeleton { skeleton }),
        ex(
            "state",
            ServerMessage::State {
                state: state::system_state(),
            },
        ),
        ex("frame", frame()),
        ex("frame_no_pose", frame_no_pose()),
        ex(
            "landmarks_2d",
            ServerMessage::Landmarks2d {
                landmarks: landmarks_2d(),
            },
        ),
        ex(
            "landmarks_3d",
            ServerMessage::Landmarks3d {
                landmarks: triangulated_landmarks(),
            },
        ),
        ex(
            "notice",
            ServerMessage::Notice {
                level: NoticeLevel::Warning,
                code: "device_disconnected".into(),
                message: "Tracker SIM-T04 is disconnected.".into(),
            },
        ),
        ex("system_state", state::system_state()),
        ex("system_state_replay", state::system_state_replay()),
        ex("calibration_profile_mounting", calibration_profile()),
        ex("recording_session", state::recording_session()),
    ];
    out.extend(observations());
    out.extend(source_events());
    out.extend(api());
    out
}
