use bansayo_training::observables::angles::{
    axis_angle, from_down_axis, hip_flexion, trunk_forward_lean,
};
use nalgebra::{Matrix3, UnitQuaternion as Q, Vector3 as V};
#[test]
fn forward_trunk_lean_is_positive_and_follows_the_pelvis_sagittal_plane() {
    let pelvis = Q::identity();
    let forward = Q::from_euler_angles(-0.3, 0.0, 0.0);
    assert!((trunk_forward_lean(&pelvis, &forward).unwrap() - 0.3).abs() < 1e-12);
    let heading = Q::from_axis_angle(&V::y_axis(), 1.2);
    assert!((trunk_forward_lean(&heading, &(heading * forward)).unwrap() - 0.3).abs() < 1e-12);
    let tilted_pelvis = Q::from_euler_angles(0.4, 0.0, 0.0);
    assert!(
        (trunk_forward_lean(&tilted_pelvis, &forward).unwrap() - 0.3).abs() < 1e-12,
        "lean is from world up, not from pelvis up"
    );
    assert!(trunk_forward_lean(&pelvis, &Q::from_euler_angles(0.0, 0.0, 1.5)).is_none());
}
#[test]
fn moved_angles_are_rigid_transform_invariant() {
    for i in 0..40 {
        let a = Q::from_euler_angles(i as f64 * 0.031, 0.21, -0.16);
        let b = Q::from_euler_angles(-0.71, i as f64 * 0.027, 0.31);
        let g = Q::from_euler_angles(0.37, -0.58, i as f64 * 0.113);
        let tail = V::new(0.0, -0.4, 0.0);
        assert!(
            (axis_angle(&a, &tail, &b, &tail) - axis_angle(&(g * a), &tail, &(g * b), &tail)).abs()
                < 1e-12
        );
        assert!(
            (from_down_axis(&a, &b, &tail) - from_down_axis(&(g * a), &(g * b), &tail)).abs()
                < 1e-12
        );
        assert!((hip_flexion(&a, &b) - hip_flexion(&(g * a), &(g * b))).abs() < 1e-12);
    }
}
#[test]
fn left_right_reflection_preserves_flexion_elevation_and_forward_lean() {
    let s = Matrix3::from_diagonal(&V::new(-1.0, 1.0, 1.0));
    let mirror = |q: Q<f64>| Q::from_matrix(&(s * q.to_rotation_matrix().matrix() * s));
    for i in 0..40 {
        let a = Q::from_euler_angles(0.0, i as f64 * 0.05, 0.0);
        let b = Q::from_euler_angles(-0.6, 0.3, i as f64 * 0.01);
        let tail = V::new(0.4, 0.0, 0.0);
        let leg = -V::y();
        assert!(
            (axis_angle(&a, &tail, &b, &tail)
                - axis_angle(&mirror(a), &(-tail), &mirror(b), &(-tail)))
            .abs()
                < 1e-12
        );
        assert!(
            (from_down_axis(&a, &b, &tail) - from_down_axis(&mirror(a), &mirror(b), &(-tail)))
                .abs()
                < 1e-12
        );
        assert!((hip_flexion(&a, &b) - hip_flexion(&mirror(a), &mirror(b))).abs() < 1e-12);
        assert!(
            (axis_angle(&a, &leg, &b, &leg) - axis_angle(&mirror(a), &leg, &mirror(b), &leg)).abs()
                < 1e-12
        );
        assert!(
            (trunk_forward_lean(&a, &b).unwrap()
                - trunk_forward_lean(&mirror(a), &mirror(b)).unwrap())
            .abs()
                < 1e-12
        );
    }
}
