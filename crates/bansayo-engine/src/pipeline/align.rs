//! Time alignment of asynchronous orientation observations (ADR-023, ADR-025 rung 1).
//!
//! The solver reconstructs at one instant `t_ref`. Each orientation stream is evaluated at
//! `t_ref` by spherical linear interpolation between the two samples around it. Nothing is
//! extrapolated: `t_ref` is chosen no later than the newest sample of every stream it uses, and a
//! stream whose samples around `t_ref` are further apart than the gap limit is not used.
use crate::math::{Rot, rot_from_wire};
use bansayo_contracts::{ObservationData, SensorObservation};

/// Orientation of one stream at the reconstruction instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Aligned {
    pub rotation: Rot,
    /// Spacing of the samples interpolated between; 0 for an exact sample.
    pub bracket_us: u64,
    pub confidence: Option<f32>,
}

fn rotation(o: &SensorObservation) -> Option<Rot> {
    match o.data {
        ObservationData::Orientation { q, .. } => rot_from_wire(q),
        _ => None,
    }
}

/// SLERP along the shorter arc. Quaternions `q` and `-q` are the same rotation, so the sign of
/// `b` is chosen to make the dot product non-negative first.
pub fn slerp(a: &Rot, b: &Rot, f: f64) -> Rot {
    let b = if a.coords.dot(&b.coords) < 0.0 {
        Rot::new_unchecked(-b.into_inner())
    } else {
        *b
    };
    a.try_slerp(&b, f, 1e-9).unwrap_or_else(|| a.nlerp(&b, f))
}

/// The orientation at `t` from a time-ordered history (oldest first), or `None` when `t` is
/// outside the history (no extrapolation) or the samples around it are more than `max_gap_us`
/// apart.
pub fn orientation_at<'a>(
    history: impl DoubleEndedIterator<Item = &'a SensorObservation> + Clone,
    t: u64,
    max_gap_us: u64,
) -> Option<Aligned> {
    let before = history.clone().rev().find(|o| o.t_us <= t)?;
    if before.t_us == t {
        return Some(Aligned {
            rotation: rotation(before)?,
            bracket_us: 0,
            confidence: before.confidence,
        });
    }
    let after = history.into_iter().find(|o| o.t_us > t)?;
    let gap = after.t_us - before.t_us;
    if gap > max_gap_us {
        return None;
    }
    let f = (t - before.t_us) as f64 / gap as f64;
    Some(Aligned {
        rotation: slerp(&rotation(before)?, &rotation(after)?, f),
        bracket_us: gap,
        confidence: match (before.confidence, after.confidence) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        },
    })
}

/// The reconstruction instant: the oldest of the streams' newest sample times, among streams
/// fresh at `now`. Every fresh stream then has a sample at or after it, so none is extrapolated.
pub fn reference_time(
    newest: impl IntoIterator<Item = u64>,
    now: u64,
    stale_us: u64,
) -> Option<u64> {
    newest
        .into_iter()
        .filter(|t| *t <= now && now - t <= stale_us)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::rot_to_wire;
    use bansayo_contracts::*;
    use nalgebra::Vector3;

    fn sample(seq: u64, t_us: u64, r: &Rot) -> SensorObservation {
        let id = DeviceId::new("T1").unwrap();
        SensorObservation {
            device_id: id.clone(),
            source_id: SourceId::new("s").unwrap(),
            origin: Origin::Simulated,
            seq,
            t_us,
            device_time_us: None,
            time_quality: TimeQuality::Estimated,
            calibration_ref: None,
            confidence: Some(1.0),
            data: ObservationData::Orientation {
                frame: FrameRef::SensorReference { id },
                q: rot_to_wire(r),
            },
        }
    }
    /// A shin swinging at 900 °/s about a tilted axis: a fast kick.
    fn truth(t_us: u64) -> Rot {
        let axis = nalgebra::Unit::new_normalize(Vector3::new(1.0, 0.3, 0.1));
        Rot::from_axis_angle(&axis, (900.0f64).to_radians() * t_us as f64 * 1e-6)
    }

    #[test]
    fn slerp_between_100_and_200_hz_samples_matches_constant_rate_rotation() {
        for period in [10_000u64, 5_000] {
            let history: Vec<_> = (0..20)
                .map(|i| sample(i, i * period, &truth(i * period)))
                .collect();
            // Camera instants of a 30 fps and a 60 fps camera, offset by 3.7 ms.
            for t in [3_700u64, 37_033, 20_366, 53_700, 87_033] {
                let a = orientation_at(history.iter(), t, 50_000).unwrap();
                let err = a.rotation.angle_to(&truth(t)).to_degrees();
                // Constant angular velocity about a fixed axis: SLERP is exact up to f32 wire.
                assert!(err < 0.01, "period {period} t {t}: {err}°");
                assert_eq!(a.bracket_us, period);
            }
        }
    }

    #[test]
    fn nothing_is_extrapolated_and_gaps_are_not_bridged() {
        let history: Vec<_> = [0u64, 10_000, 90_000]
            .iter()
            .enumerate()
            .map(|(i, t)| sample(i as u64, *t, &truth(*t)))
            .collect();
        assert!(
            orientation_at(history.iter(), 95_000, 50_000).is_none(),
            "after newest"
        );
        assert!(
            orientation_at(history.iter(), 50_000, 50_000).is_none(),
            "80 ms gap"
        );
        assert!(orientation_at(history.iter(), 50_000, 100_000).is_some());
        assert_eq!(
            orientation_at(history.iter(), 90_000, 50_000)
                .unwrap()
                .bracket_us,
            0,
            "exact sample"
        );
        let empty: Vec<SensorObservation> = vec![];
        assert!(orientation_at(empty.iter(), 0, 50_000).is_none());
    }

    #[test]
    fn opposite_quaternion_signs_take_the_short_way() {
        let a = Rot::from_axis_angle(&Vector3::y_axis(), 0.1);
        let b = Rot::from_axis_angle(&Vector3::y_axis(), 0.3);
        let flipped = Rot::new_unchecked(-b.into_inner());
        let m = slerp(&a, &flipped, 0.5);
        assert!(m.angle_to(&Rot::from_axis_angle(&Vector3::y_axis(), 0.2)) < 1e-9);
        // Linear interpolation of the raw components would go through the long way round.
        assert!(slerp(&a, &a, 0.3).angle_to(&a) < 1e-12);
    }

    #[test]
    fn reference_time_is_the_oldest_fresh_newest_sample() {
        assert_eq!(reference_time([100, 80, 95], 100, 50), Some(80));
        assert_eq!(
            reference_time([100, 10], 100, 50),
            Some(100),
            "stale stream ignored"
        );
        assert_eq!(reference_time([], 100, 50), None);
        assert_eq!(
            reference_time([150], 100, 50),
            None,
            "future sample ignored"
        );
    }
}
