//! Rung-1 fusion state (ADR-025): per-tracker heading correction from camera evidence, and the
//! last camera anchor of the root. Deterministic, replayable, no filter.
//!
//! Heading. Mounting is captured in the reference pose with heading alignment assumed identity
//! (docs/05 §3): `M = R̄⁻¹`. If the tracker's reference frame is in truth yawed by `A` from the
//! world, the true segment rotation is `S = A·(R·M)·A⁻¹`: a conjugation, not a pre-rotation. So
//! pure turns about the vertical are already right, and `A` shows only in the direction of a
//! tilted segment. The correction therefore compares, for segments whose rest axis is vertical
//! (thighs, shins, spine), the horizontal direction of the camera-observed axis with the IMU's,
//! and moves `A` towards it slowly and with a rate limit. Upright segments carry no heading
//! information; then the correction holds.
use crate::config::FusionConfig;
use crate::math::{Rot, V3};
use bansayo_contracts::DeviceId;
use std::collections::BTreeMap;

/// Recorded in the tracking configuration identity: a change of method is a new comparability
/// class (ADR-025 consequences).
pub const METHOD_VERSION: &str = "bansayo.fusion.anchored_fk.v1";
/// Errors below this mark a heading correction as converged.
pub const CONVERGED_DEG: f64 = 5.0;
/// Longest step between two corrections, seconds; a longer gap does not allow a bigger jump.
const MAX_STEP_S: f64 = 0.1;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeadingState {
    /// `A` as a yaw angle about world +Y, radians.
    pub yaw_rad: f64,
    pub converged: bool,
    pub last_us: Option<u64>,
    pub accepted: u32,
    pub rejected: u32,
}

/// The last camera-measured root position.
#[derive(Debug, Clone, PartialEq)]
pub struct Anchor {
    pub root: V3,
    pub t_us: u64,
    pub sigma_m: Option<f32>,
    pub confidence: Option<f32>,
    pub sources: Vec<DeviceId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FusionState {
    pub heading: BTreeMap<DeviceId, HeadingState>,
    pub anchor: Option<Anchor>,
}

/// Why a heading correction was not applied this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suppressed {
    /// The segment's rest axis is not vertical, so yaw does not map one to one.
    Unobservable,
    /// The segment is too close to vertical in the camera or IMU view.
    Degenerate,
    /// Larger than the outlier limit after convergence.
    Outlier,
}

pub fn yaw(angle: f64) -> Rot {
    Rot::from_axis_angle(&V3::y_axis(), angle)
}

/// `A·R·A⁻¹` for a mounted IMU rotation `R·M`.
pub fn corrected(mounted: &Rot, heading: f64) -> Rot {
    yaw(heading) * mounted * yaw(-heading)
}

fn wrap(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    (a + std::f64::consts::PI).rem_euclid(t) - std::f64::consts::PI
}

/// Heading error, camera minus corrected IMU, of one segment with rest axis `rest_tail`.
pub fn heading_error(
    rest_tail: &V3,
    mounted: &Rot,
    heading: f64,
    camera: &Rot,
    min_tilt_deg: f64,
) -> Result<f64, Suppressed> {
    let axis = rest_tail
        .try_normalize(1e-9)
        .ok_or(Suppressed::Unobservable)?;
    if axis.y.abs() < 0.9 {
        return Err(Suppressed::Unobservable);
    }
    let cam = camera * axis;
    let imu = corrected(mounted, heading) * axis;
    let min = min_tilt_deg.to_radians().sin();
    let horizontal = |v: &V3| v.x.hypot(v.z);
    if horizontal(&cam) < min || horizontal(&imu) < min {
        return Err(Suppressed::Degenerate);
    }
    Ok(wrap(cam.x.atan2(cam.z) - imu.x.atan2(imu.z)))
}

/// Moves the heading by a bounded fraction of `error`; returns the change applied.
pub fn step(
    state: &mut HeadingState,
    error: f64,
    t_us: u64,
    cfg: &FusionConfig,
) -> Result<f64, Suppressed> {
    if !error.is_finite() || state.last_us.is_some_and(|last| t_us <= last) {
        return Err(Suppressed::Degenerate);
    }
    let dt = state
        .last_us
        .map_or(0.0, |l| t_us.saturating_sub(l) as f64 * 1e-6)
        .min(MAX_STEP_S);
    state.last_us = Some(t_us);
    if state.converged && error.abs() > cfg.heading_outlier_deg.to_radians() {
        state.rejected = state.rejected.saturating_add(1);
        return Err(Suppressed::Outlier);
    }
    let limit = cfg.max_heading_correction_deg_s.to_radians() * dt;
    let applied = (error * (dt / cfg.heading_time_constant_s).min(1.0)).clamp(-limit, limit);
    state.yaw_rad = wrap(state.yaw_rad + applied);
    state.accepted = state.accepted.saturating_add(1);
    if (error - applied).abs() < CONVERGED_DEG.to_radians() {
        state.converged = true;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shin_down() -> V3 {
        V3::new(0.0, -0.4, 0.0)
    }

    #[test]
    fn repeated_and_reversed_timestamps_do_not_change_heading() {
        let cfg = FusionConfig::default();
        let mut state = HeadingState::default();
        step(&mut state, 0.2, 1000, &cfg).unwrap();
        let before = state.clone();
        assert_eq!(
            step(&mut state, 0.2, 1000, &cfg),
            Err(Suppressed::Degenerate)
        );
        assert_eq!(
            step(&mut state, 0.2, 999, &cfg),
            Err(Suppressed::Degenerate)
        );
        assert_eq!(
            step(&mut state, f64::NAN, 2000, &cfg),
            Err(Suppressed::Degenerate)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn conjugation_leaves_turns_alone_and_rotates_tilt_axes() {
        let turn = yaw(0.7);
        assert!(corrected(&turn, 1.2).angle_to(&turn) < 1e-12);
        // A thigh raised forward (about +X) seen by a tracker whose frame is yawed 90°.
        let truth = Rot::from_axis_angle(&V3::x_axis(), -1.2);
        let a = 90f64.to_radians();
        let mounted = yaw(-a) * truth * yaw(a);
        assert!(corrected(&mounted, a).angle_to(&truth) < 1e-12);
    }

    #[test]
    fn heading_is_observed_only_on_tilted_vertical_segments() {
        let a = 30f64.to_radians();
        let truth = Rot::from_axis_angle(&V3::x_axis(), -1.0); // front kick, shin forward
        let mounted = yaw(-a) * truth * yaw(a);
        let e = heading_error(&shin_down(), &mounted, 0.0, &truth, 20.0).unwrap();
        assert!((e - a).abs() < 1e-9, "{}", e.to_degrees());
        // Standing: the shin is vertical, nothing to learn.
        let standing = Rot::identity();
        assert_eq!(
            heading_error(&shin_down(), &standing, 0.0, &standing, 20.0),
            Err(Suppressed::Degenerate)
        );
        // An arm's rest axis is horizontal (T-pose): not used.
        assert_eq!(
            heading_error(&V3::new(0.3, 0.0, 0.0), &truth, 0.0, &truth, 20.0),
            Err(Suppressed::Unobservable)
        );
    }

    #[test]
    fn correction_is_slow_bounded_and_rejects_outliers_once_converged() {
        let cfg = FusionConfig::default();
        let mut s = HeadingState::default();
        let target = 60f64.to_radians();
        let mut t = 0;
        let mut max_step: f64 = 0.0;
        for _ in 0..1_200 {
            t += 16_667; // 60 Hz ticks, 20 s
            let error = wrap(target - s.yaw_rad);
            let applied = step(&mut s, error, t, &cfg).unwrap();
            max_step = max_step.max(applied.abs());
        }
        // Never faster than 15 °/s: no snap.
        assert!(max_step <= 15f64.to_radians() * 0.016_668 + 1e-12);
        assert!(s.converged && (s.yaw_rad - target).abs() < 1f64.to_radians());
        let before = s.yaw_rad;
        t += 16_667;
        assert_eq!(step(&mut s, 2.0, t, &cfg), Err(Suppressed::Outlier));
        assert_eq!((s.yaw_rad, s.rejected), (before, 1));
    }
}
