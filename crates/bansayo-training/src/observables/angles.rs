//! Model-axis angles, radians. Keep arithmetic order stable for the existing frame contract.
use nalgebra::{UnitQuaternion as Rot, Vector3 as V3};
fn angle_between(a: &V3<f64>, b: &V3<f64>) -> f64 {
    let n = a.norm() * b.norm();
    if n == 0.0 {
        0.0
    } else {
        (a.dot(b) / n).clamp(-1.0, 1.0).acos()
    }
}
pub fn axis_angle(a: &Rot<f64>, a_tail: &V3<f64>, b: &Rot<f64>, b_tail: &V3<f64>) -> f64 {
    angle_between(&(a * a_tail), &(b * b_tail))
}
pub fn from_down_axis(a: &Rot<f64>, b: &Rot<f64>, b_tail: &V3<f64>) -> f64 {
    angle_between(&(a * (-V3::y())), &(b * b_tail))
}
pub fn hip_flexion(pelvis: &Rot<f64>, thigh: &Rot<f64>) -> f64 {
    let d = pelvis.inverse() * (thigh * (-V3::y()));
    d.z.atan2(-d.y)
}
/// Chest up measured from world up, both projected into the pelvis sagittal plane.
/// Its normal is pelvis left (+X); the signed angle is positive towards pelvis forward.
pub fn trunk_forward_lean(pelvis: &Rot<f64>, chest: &Rot<f64>) -> Option<f64> {
    let left = pelvis * V3::x();
    let chest_up = chest * V3::y();
    let up = V3::y() - left * left.y;
    let projected = chest_up - left * chest_up.dot(&left);
    if up.norm() < 0.35 || projected.norm() < 0.35 {
        return None;
    }
    Some(left.dot(&up.cross(&projected)).atan2(up.dot(&projected)))
}
