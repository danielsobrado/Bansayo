use bansayo_contracts::*;
#[test]
fn canonical_points_have_normative_order_and_new_types_round_trip() {
    assert_eq!(BODY_KEYPOINT_SCHEMA_ID,"bansayo.body.v1.points");
    assert_eq!(BODY_KEYPOINT_NAMES.len(),26);
    assert_eq!(&BODY_KEYPOINT_NAMES[19..],&["head_top","left_hand_tip","right_hand_tip","left_toe","right_toe","left_heel","right_heel"]);
    let motion=MotionSample{seq:1,t_us:100,origins:vec![Origin::Simulated],body:None,planar:vec![]};
    let json=serde_json::to_value(&motion).unwrap();assert!(json["body"].is_null());
    assert_eq!(motion,serde_json::from_value(json).unwrap());
    assert_eq!(serde_json::to_value(CapabilityProfileId::MultiView).unwrap(),"multi_view");
    assert_eq!(serde_json::to_value(MethodId::SkeletonFitAxis).unwrap(),"skeleton_fit_axis");
}
#[test]
fn legacy_state_and_landmarks_accept_absent_additions() {
    let state:serde_json::Value=serde_json::from_str(include_str!("../../../contracts/examples/system_state.json")).unwrap();
    let mut legacy=state;legacy.as_object_mut().unwrap().remove("capability_report");
    let parsed:SystemState=serde_json::from_value(legacy).unwrap();assert!(parsed.capability_report.is_none());
    let p:TriangulatedLandmark3D=serde_json::from_value(serde_json::json!({"m":[0,1,0],"views":2,"reprojection_error_px":1,"confidence":1})).unwrap();assert!(p.sigma_m.is_none());
}
