use bansayo_contracts::*;
use bansayo_engine::{
    calibration::Store,
    registry::persistence::{
        RUNTIME_SCHEMA_VERSION, RuntimeSnapshot, load_runtime, save_runtime_snapshot,
    },
    skeleton::Skeleton,
};
use std::collections::{BTreeMap, BTreeSet};
fn profiles() -> Vec<CalibrationProfile> {
    [
        CalibrationKind::ImuBias,
        CalibrationKind::Mounting,
        CalibrationKind::HeadingAlignment,
        CalibrationKind::BodyProportions,
        CalibrationKind::CameraIntrinsics,
        CalibrationKind::CameraExtrinsics,
        CalibrationKind::FloorPlane,
        CalibrationKind::XrAlignment,
        CalibrationKind::TimeSync,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| CalibrationProfile {
        schema_version: 1,
        id: ProfileId::new(format!("profile-{i}")).unwrap(),
        kind,
        method: "imported".into(),
        origin: Origin::Fixture,
        created_utc: "2026-11-17T00:00:00Z".into(),
        subject: CalibrationSubject {
            device_id: Some(DeviceId::new("DEVICE").unwrap()),
            segment_id: None,
        },
        depends_on: vec![],
        data: serde_json::json!({"unknown_payload":[1,2,3]})
            .as_object()
            .unwrap()
            .clone(),
        quality: Default::default(),
    })
    .collect()
}
fn assert_retained(store: &Store, expected: &[CalibrationProfile]) {
    assert_eq!(store.profiles.len(), expected.len());
    for p in expected {
        assert_eq!(store.profiles.get(&p.id), Some(p));
        let s = store
            .summaries()
            .into_iter()
            .find(|s| s.id == p.id)
            .unwrap();
        assert_eq!(s.kind, p.kind);
        assert_eq!(s.state, CalibrationState::Invalid);
        assert_eq!(s.data, p.data);
    }
    assert!(
        store
            .mounting(
                &DeviceId::new("DEVICE").unwrap(),
                &SegmentId::new("pelvis").unwrap()
            )
            .is_none()
    );
}
#[test]
fn all_calibration_kinds_survive_manifest_load_and_save_as_invalid_when_uninterpreted() {
    let dir = tempfile::tempdir().unwrap();
    let expected = profiles();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let snapshot = RuntimeSnapshot {
        schema_version: RUNTIME_SCHEMA_VERSION,
        assignments: BTreeMap::new(),
        stale_calibrations: BTreeSet::new(),
        calibration_profiles: expected.clone(),
    };
    save_runtime_snapshot(dir.path(), &snapshot).unwrap();
    let (store, assignments) = load_runtime(dir.path(), &BTreeMap::new(), &sk).unwrap();
    assert_retained(&store, &expected);
    save_runtime_snapshot(
        dir.path(),
        &RuntimeSnapshot::from_state(
            &assignments,
            std::iter::empty(),
            &store.stale,
            &store.profiles,
        ),
    )
    .unwrap();
    assert_retained(
        &load_runtime(dir.path(), &BTreeMap::new(), &sk).unwrap().0,
        &expected,
    );
}
#[test]
fn legacy_files_of_all_kinds_are_migrated_without_discarding_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let expected = profiles();
    std::fs::create_dir(dir.path().join("calibration")).unwrap();
    for p in &expected {
        std::fs::write(
            dir.path()
                .join("calibration")
                .join(format!("{}.json", p.id)),
            serde_json::to_vec(p).unwrap(),
        )
        .unwrap();
    }
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    assert_retained(
        &load_runtime(dir.path(), &BTreeMap::new(), &sk).unwrap().0,
        &expected,
    );
    assert_retained(
        &load_runtime(dir.path(), &BTreeMap::new(), &sk).unwrap().0,
        &expected,
    );
}
