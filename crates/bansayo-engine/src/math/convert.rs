//! Contract arrays (`[x, y, z, w]`, `[x, y, z]`, `f32`) to and from nalgebra (`f64`).

use bansayo_contracts::{Quat, Vec3};
use nalgebra::{Quaternion, UnitQuaternion, Vector3};

/// Rotation used inside the engine.
pub type Rot = UnitQuaternion<f64>;
/// Vector used inside the engine.
pub type V3 = Vector3<f64>;

/// A received quaternion whose norm differs from 1 by more than this is invalid.
pub const UNIT_QUAT_TOLERANCE: f64 = 1e-3;

/// Parses a wire quaternion, rejecting non-finite or non-unit values.
pub fn rot_from_wire(q: Quat) -> Option<Rot> {
    let [x, y, z, w] = q.map(f64::from);
    let raw = Quaternion::new(w, x, y, z);
    let norm = raw.norm();
    (raw.coords.iter().all(|c| c.is_finite()) && (norm - 1.0).abs() <= UNIT_QUAT_TOLERANCE)
        .then(|| UnitQuaternion::new_normalize(raw))
}

pub fn rot_to_wire(r: &Rot) -> Quat {
    let q = r.quaternion();
    [q.i as f32, q.j as f32, q.k as f32, q.w as f32]
}

pub fn v3_from_wire(v: Vec3) -> V3 {
    V3::new(v[0].into(), v[1].into(), v[2].into())
}

pub fn v3_to_wire(v: &V3) -> Vec3 {
    [v.x as f32, v.y as f32, v.z as f32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_order_is_xyzw() {
        let r = Rot::from_axis_angle(&V3::z_axis(), std::f64::consts::FRAC_PI_2);
        let q = rot_to_wire(&r);
        assert!(q[0].abs() < 1e-6 && q[1].abs() < 1e-6);
        assert!((q[2] - q[3]).abs() < 1e-6 && q[3] > 0.7);
        let back = rot_from_wire(q).map(|b| b.angle_to(&r));
        assert!(back.is_some_and(|a| a < 1e-6));
    }

    #[test]
    fn rejects_invalid_quaternions() {
        assert!(rot_from_wire([0.0, 0.0, 0.0, 0.9]).is_none());
        assert!(rot_from_wire([f32::NAN, 0.0, 0.0, 1.0]).is_none());
        assert!(rot_from_wire([0.0, 0.0, 0.0, 1.0]).is_some());
    }

    #[test]
    fn rotation_maps_b_into_a() {
        // q_a_from_b rotates +X of frame b into +Y of frame a for a 90° turn about +Z.
        let r = Rot::from_axis_angle(&V3::z_axis(), std::f64::consts::FRAC_PI_2);
        assert!((r * V3::x() - V3::y()).norm() < 1e-12);
    }
}
