use bansayo_engine::{config, pipeline::triangulate::keypoint_sigma};
#[test]
fn keypoint_uncertainty_uses_noise_floor_geometry_and_view_count() {
    let sigma = keypoint_sigma(0.0, 2.0, 3.0, 1000.0, std::f64::consts::FRAC_PI_2, 2).unwrap();
    assert!((sigma - 0.006).abs() < 1e-12);
    assert!(
        (keypoint_sigma(4.0, 2.0, 3.0, 1000.0, std::f64::consts::FRAC_PI_6, 5).unwrap() - 0.012)
            .abs()
            < 1e-12
    );
    assert!(keypoint_sigma(0.0, 2.0, 3.0, 1000.0, 0.0, 2).is_none());
    assert!(keypoint_sigma(0.0, 2.0, 3.0, 1000.0, 1.0, 1).is_none());
}
#[test]
fn detector_noise_floor_defaults_and_validates() {
    assert_eq!(
        config::parse("schema_version: 1")
            .unwrap()
            .config
            .evidence
            .detector_noise_floor_px,
        2.0
    );
    assert_eq!(
        config::parse("schema_version: 1\nevidence: {detector_noise_floor_px: 3.0}")
            .unwrap()
            .config
            .evidence
            .detector_noise_floor_px,
        3.0
    );
    for value in ["0", "-1", ".nan", ".inf"] {
        assert!(
            config::parse(&format!(
                "schema_version: 1\nevidence: {{detector_noise_floor_px: {value}}}"
            ))
            .is_err()
        );
    }
}
