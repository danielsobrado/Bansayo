//! Elementary rotations about world axes, in degrees or radians.

use nalgebra::Vector3;

use super::{Rot, V3};

pub fn rot_x(rad: f64) -> Rot {
    Rot::from_axis_angle(&Vector3::x_axis(), rad)
}

pub fn rot_y(rad: f64) -> Rot {
    Rot::from_axis_angle(&Vector3::y_axis(), rad)
}

pub fn rot_z(rad: f64) -> Rot {
    Rot::from_axis_angle(&Vector3::z_axis(), rad)
}

/// Yaw about world +Y by `deg` degrees.
pub fn yaw_deg(deg: f64) -> Rot {
    rot_y(deg.to_radians())
}

/// Unsigned angle between two vectors, radians. Zero if either is zero.
pub fn angle_between(a: &V3, b: &V3) -> f64 {
    let n = a.norm() * b.norm();
    if n == 0.0 { 0.0 } else { (a.dot(b) / n).clamp(-1.0, 1.0).acos() }
}
