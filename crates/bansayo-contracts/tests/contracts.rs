//! Generated-output staleness, golden round trips, tolerance and literal wire shapes.

mod support;

use bansayo_contracts::*;
use serde_json::{Value, json};

use support::{dev, examples, generated};

#[test]
fn generated_contracts_are_current() {
    generated::check_or_bless();
}

#[test]
fn generation_is_deterministic() {
    assert_eq!(generated::typescript(), generated::typescript());
    assert_eq!(generated::examples(), generated::examples());
}

#[test]
fn golden_examples_round_trip() {
    let dir = generated::contracts_dir().join("examples");
    // While blessing, the files are being rewritten concurrently; check the fresh output instead.
    let blessing = generated::blessing();
    let fresh = generated::examples();
    for e in examples::all() {
        let path = dir.join(&e.file);
        let text = match blessing {
            true => fresh[&e.file].clone(),
            false => std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("{}: {err}", path.display())),
        };
        let golden: Value =
            serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", e.file));
        let back = (e.round_trip)(golden.clone()).unwrap_or_else(|err| panic!("{}: {err}", e.file));
        assert_eq!(back, golden, "{} does not round-trip", e.file);
    }
}

#[test]
fn unknown_fields_are_ignored() {
    let mut frame = examples::all()
        .into_iter()
        .find(|e| e.file == "frame.json")
        .expect("frame")
        .value();
    frame["future_field"] = json!({ "anything": [1, 2] });
    frame["pose"]["segments"][0]["observed_position_m"] = json!([0.0, 1.0, 0.0]);
    frame["quality"]["reasons"][0]["severity"] = json!("low");
    assert!(serde_json::from_value::<ServerMessage>(frame).is_ok());

    let mut profile = examples::all()
        .into_iter()
        .find(|e| e.file == "calibration_profile_mounting.json")
        .expect("profile")
        .value();
    profile["future_field"] = json!(true);
    assert!(serde_json::from_value::<CalibrationProfile>(profile).is_ok());

    let body = json!({ "segment": "pelvis", "client_note": "x" });
    assert!(serde_json::from_value::<AssignmentRequest>(body).is_ok());
}

#[test]
fn invalid_identifiers_are_rejected_on_input() {
    let bad_session = json!({ "session_id": "../../etc/passwd" });
    assert!(serde_json::from_value::<PlaybackStartRequest>(bad_session).is_err());
    let bad_segment = json!({ "segment": "Left Thigh" });
    assert!(serde_json::from_value::<AssignmentRequest>(bad_segment).is_err());
}

#[test]
fn frame_ref_shape() {
    assert_eq!(
        serde_json::to_value(FrameRef::World).ok(),
        Some(json!({ "type": "world" }))
    );
    assert_eq!(
        serde_json::to_value(FrameRef::XrStage).ok(),
        Some(json!({ "type": "xr_stage" }))
    );
    assert_eq!(
        serde_json::to_value(FrameRef::Sensor { id: dev("T01") }).ok(),
        Some(json!({ "type": "sensor", "id": "T01" }))
    );
    assert_eq!(
        serde_json::to_value(FrameRef::SensorReference { id: dev("T01") }).ok(),
        Some(json!({ "type": "sensor_reference", "id": "T01" }))
    );
}

#[test]
fn observation_data_shape() {
    let data = ObservationData::Orientation {
        frame: FrameRef::SensorReference { id: dev("SIM-T01") },
        q: [0.0, 0.0, 0.0, 1.0],
    };
    assert_eq!(
        serde_json::to_value(data).ok(),
        Some(json!({
            "kind": "orientation",
            "frame": { "type": "sensor_reference", "id": "SIM-T01" },
            "q": [0.0, 0.0, 0.0, 1.0]
        }))
    );
    let contact = ObservationData::Contact {
        in_contact: true,
        force_n: None,
    };
    assert_eq!(
        serde_json::to_value(contact).ok(),
        Some(json!({ "kind": "contact", "in_contact": true, "force_n": null }))
    );
}

#[test]
fn server_message_shape() {
    let notice = ServerMessage::Notice {
        level: NoticeLevel::Warning,
        code: "device_disconnected".into(),
        message: "Tracker SIM-T04 is disconnected.".into(),
    };
    assert_eq!(
        serde_json::to_value(notice).ok(),
        Some(json!({
            "type": "notice",
            "level": "warning",
            "code": "device_disconnected",
            "message": "Tracker SIM-T04 is disconnected."
        }))
    );
    let landmarks = examples::all()
        .into_iter()
        .find(|e| e.file == "landmarks_2d.json")
        .expect("landmarks");
    let landmarks = landmarks.value();
    assert_eq!(landmarks["type"], json!("landmarks_2d"));
    assert_eq!(
        landmarks["landmarks"]["frame"],
        json!({ "type": "image", "id": "SIM-CAM1" })
    );
}

#[test]
fn enum_strings() {
    let caps = [
        SensorCapability::Landmarks2d,
        SensorCapability::Landmarks3dModel,
        SensorCapability::SignalStrength,
    ];
    assert_eq!(
        serde_json::to_value(caps).ok(),
        Some(json!([
            "landmarks_2d",
            "landmarks_3d_model",
            "signal_strength"
        ]))
    );
    assert_eq!(
        serde_json::to_value([FeatureId::Landmarks2d, FeatureId::Landmarks3dTriangulated]).ok(),
        Some(json!(["landmarks_2d", "landmarks_3d_triangulated"]))
    );
    assert_eq!(
        serde_json::to_value(TimeQuality::ReceiveTime).ok(),
        Some(json!("receive_time"))
    );
    assert_eq!(
        serde_json::to_value(Conventions::default()).ok(),
        Some(json!({
            "handedness": "right", "up": "+y", "forward": "+z", "length_unit": "m",
            "angle_unit": "rad", "quaternion_order": "xyzw", "time_unit": "us"
        }))
    );
}

#[test]
fn api_envelope_shape() {
    assert_eq!(
        serde_json::to_value(ApiResponse::ok(HealthResponse::default())).ok(),
        Some(json!({ "ok": true, "data": { "status": "ok" } }))
    );
    assert_eq!(
        serde_json::to_value(ApiResponse::ok(DeletedResponse::default())).ok(),
        Some(json!({ "ok": true, "data": { "deleted": true } }))
    );
    let mismatched =
        json!({ "ok": true, "error": { "code": "internal", "message": "x", "details": null } });
    assert!(serde_json::from_value::<ApiResponse<HealthResponse>>(mismatched).is_err());
    // Optional request fields are omitted when absent and accepted when missing.
    assert_eq!(
        serde_json::to_value(PlaybackUpdateRequest::default()).ok(),
        Some(json!({}))
    );
    assert!(serde_json::from_value::<SimulationUpdateRequest>(json!({})).is_ok());
}

#[test]
fn xr_client_hello_round_trip() {
    let wire = json!({ "type": "hello", "protocol_version": 1 });
    let hello: XrClientMessage = serde_json::from_value(wire.clone()).expect("hello");
    assert_eq!(
        hello,
        XrClientMessage::Hello(XrHello {
            protocol_version: XR_PROTOCOL_VERSION
        })
    );
    assert_eq!(serde_json::to_value(&hello).ok(), Some(wire));
    let ready = XrServerMessage::Ready {
        protocol_version: 1,
        time_quality: TimeQuality::ReceiveTime,
    };
    assert_eq!(
        serde_json::to_value(&ready).ok(),
        Some(json!({ "type": "ready", "protocol_version": 1, "time_quality": "receive_time" }))
    );
    let extra = json!({ "type": "hello", "protocol_version": 1, "device_id": "XR-HMD" });
    assert!(serde_json::from_value::<XrClientMessage>(extra).is_err());
}

fn xr_frame_json(samples: Value) -> Value {
    json!({ "type": "frame", "seq": 42, "client_time_us": 123_456_789u64, "samples": samples })
}
fn xr_sample(role: &str) -> Value {
    json!({ "role": role, "tracked": true, "position_m": [0.0, 1.7, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0] })
}

#[test]
fn xr_frame_requires_three_ordered_roles() {
    let three = xr_frame_json(json!([
        xr_sample("head"),
        xr_sample("left_grip"),
        xr_sample("right_grip")
    ]));
    let Ok(XrClientMessage::Frame(frame)) = serde_json::from_value::<XrClientMessage>(three) else {
        panic!("a three-sample frame decodes");
    };
    assert_eq!(frame.samples.map(|s| s.role), XrRole::ORDER);
    // Count, unknown roles and client-chosen identity are refused at decode; order and
    // duplicates are refused by the engine's frame validation (sources/xr.rs).
    for bad in [
        xr_frame_json(json!([xr_sample("head"), xr_sample("left_grip")])),
        xr_frame_json(json!([
            xr_sample("head"),
            xr_sample("left_grip"),
            xr_sample("right_grip"),
            xr_sample("right_grip")
        ])),
        xr_frame_json(json!([
            xr_sample("head"),
            xr_sample("left_hand"),
            xr_sample("right_grip")
        ])),
        {
            let mut f = xr_frame_json(json!([
                xr_sample("head"),
                xr_sample("left_grip"),
                xr_sample("right_grip")
            ]));
            f["frame"] = json!({ "type": "world" });
            f
        },
    ] {
        assert!(serde_json::from_value::<XrClientMessage>(bad).is_err());
    }
}

#[test]
fn xr_untracked_requires_absent_pose() {
    let untracked = json!({ "role": "right_grip", "tracked": false, "position_m": null, "rotation_xyzw": null });
    let s: XrDeviceSample = serde_json::from_value(untracked).expect("untracked");
    assert!(!s.tracked && s.position_m.is_none() && s.rotation_xyzw.is_none());
    // Non-numbers never become a pose: JSON has no NaN, strings are refused.
    let nan = json!({ "role": "head", "tracked": true, "position_m": ["NaN", 0.0, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0] });
    assert!(serde_json::from_value::<XrDeviceSample>(nan).is_err());
    let short = json!({ "role": "head", "tracked": true, "position_m": [0.0, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0] });
    assert!(serde_json::from_value::<XrDeviceSample>(short).is_err());
}

#[test]
fn rigid_pose_contract_round_trip() {
    let wire = json!({
        "device_id": "XR-HMD", "source_id": "xr-openxr", "origin": "live", "seq": 42,
        "t_us": 4_567_890, "device_time_us": 123_456_789u64, "time_quality": "receive_time",
        "calibration_ref": null, "confidence": null,
        "data": { "kind": "rigid_pose", "frame": { "type": "xr_stage" },
                  "position_m": [0.0, 1.72, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0] }
    });
    let o: SensorObservation = serde_json::from_value(wire.clone()).expect("rigid pose");
    assert!(matches!(
        o.data,
        ObservationData::RigidPose {
            frame: FrameRef::XrStage,
            ..
        }
    ));
    let back: Value =
        serde_json::from_str(&serde_json::to_string(&o).expect("serialise")).expect("parse");
    assert_eq!(back, wire);
    assert_eq!(
        serde_json::to_value(DeviceKind::XrTrackedDevice).ok(),
        Some(json!("xr_tracked_device"))
    );
    assert_eq!(
        serde_json::to_value(SensorCapability::RigidPose).ok(),
        Some(json!("rigid_pose"))
    );
}

#[test]
fn legacy_observation_examples_unchanged() {
    // Shapes recorded before RigidPose existed still decode to the same variants.
    for (file, kind) in [
        ("sensor_observation_orientation.json", "orientation"),
        ("sensor_observation_position.json", "position"),
        ("sensor_observation_landmarks_2d.json", "landmarks_2d"),
    ] {
        let e = examples::all()
            .into_iter()
            .find(|e| e.file == file)
            .expect("legacy example");
        let o: SensorObservation = serde_json::from_value(e.value()).expect(file);
        assert_eq!(
            serde_json::to_value(&o.data)
                .ok()
                .and_then(|v| v["kind"].as_str().map(String::from)),
            Some(kind.to_owned())
        );
    }
}
