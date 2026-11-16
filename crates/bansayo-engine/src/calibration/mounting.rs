use crate::math::Rot;
use nalgebra::{Quaternion, Vector4};
pub const CAPTURE_WINDOW_US: u64 = 1_000_000;
pub const MAX_SPREAD_RAD: f64 = 0.035;
/// Quaternion mean after hemisphere alignment, then the largest deviation from that mean.
pub fn mean_and_spread(readings: &[Rot]) -> Option<(Rot, f64)> {
    let first = readings.first()?.quaternion().coords;
    let mut sum = Vector4::zeros();
    for q in readings {
        let v = q.quaternion().coords;
        sum += if first.dot(&v) < 0.0 { -v } else { v };
    }
    if sum.norm() < 1e-9 {
        return None;
    }
    let mean = Rot::new_normalize(Quaternion::from_vector(sum));
    let spread = readings
        .iter()
        .map(|q| q.angle_to(&mean))
        .fold(0.0, f64::max);
    Some((mean, spread))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inverse_mean_recovers_mounting_and_motion() {
        let mounting = Rot::from_euler_angles(0.7, -0.2, 1.2);
        let raw = mounting.inverse();
        let (mean, spread) = mean_and_spread(&[raw, raw]).unwrap();
        assert!(spread < 1e-9);
        let world = Rot::from_euler_angles(0.1, 0.4, -0.3);
        assert!((world * mounting.inverse() * mean.inverse()).angle_to(&world) < 1e-9);
    }
    #[test]
    fn spread_detects_movement() {
        let (_, spread) =
            mean_and_spread(&[Rot::identity(), Rot::from_euler_angles(0.3, 0.0, 0.0)]).unwrap();
        assert!(spread > MAX_SPREAD_RAD);
    }
}
