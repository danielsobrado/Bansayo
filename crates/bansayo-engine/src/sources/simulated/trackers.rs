use crate::math::{Rot, rot_to_wire, yaw_deg};
use bansayo_contracts::{DeviceId, Quat};
/// Stable SplitMix-style samples keyed by identity/sequence; no scheduler-dependent RNG state.
pub(super) fn sample(id: &DeviceId, seed: u64, seq: u64, axis: u64) -> f64 {
    let mut h = seed ^ seq.wrapping_mul(0x9e3779b97f4a7c15) ^ axis;
    for b in id.as_str().bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ u64::from(b);
    }
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((h ^ (h >> 31)) >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0
}
pub fn reading(
    id: &DeviceId,
    seed: u64,
    seq: u64,
    noise_deg: f64,
    heading: f64,
    world: Rot,
) -> Quat {
    let mount = mounting(id, seed);
    let n = noise_deg.to_radians();
    let noise = Rot::from_euler_angles(
        sample(id, seed, seq, 4) * n,
        sample(id, seed, seq, 5) * n,
        sample(id, seed, seq, 6) * n,
    );
    rot_to_wire(&(noise * yaw_deg(-heading) * world * mount.inverse()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::rot_from_wire;
    #[test]
    fn calibrated_zero_noise_reading_matches_ground_truth_to_one_microradian() {
        let id = DeviceId::new("tracker").unwrap();
        let reference = rot_from_wire(reading(&id, 42, 1, 0.0, 0.0, Rot::identity())).unwrap();
        let mounting = reference.inverse();
        for i in 0..100 {
            let world = Rot::from_euler_angles(i as f64 * 0.03, 0.2, -0.1);
            let raw = rot_from_wire(reading(&id, 42, i, 0.0, 0.0, world)).unwrap();
            assert!((raw * mounting).angle_to(&world) < 1e-6);
        }
    }
    #[test]
    fn noise_is_deterministic_and_seed_changes_mounting() {
        let id = DeviceId::new("tracker").unwrap();
        assert_eq!(
            reading(&id, 42, 10, 0.2, 0.0, Rot::identity()),
            reading(&id, 42, 10, 0.2, 0.0, Rot::identity())
        );
        assert_ne!(
            reading(&id, 42, 10, 0.2, 0.0, Rot::identity()),
            reading(&id, 43, 10, 0.2, 0.0, Rot::identity())
        );
    }
}

pub(super) fn mounting(id: &DeviceId, seed: u64) -> Rot {
    Rot::from_euler_angles(
        sample(id, seed, 0, 1) * 2.0,
        sample(id, seed, 0, 2) * 2.0,
        sample(id, seed, 0, 3) * 2.0,
    )
}
