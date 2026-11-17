//! Pure measurements. Inferred segment rotations never produce a numeric joint value.
use crate::{
    math::Rot,
    registry::reason,
    skeleton::{Skeleton, proportions::JointKind},
};
use bansayo_contracts::*;
use bansayo_training::observables::angles;
pub fn measure(sk: &Skeleton, rotations: &[Rot], observed: &[bool]) -> Vec<JointMeasurement> {
    sk.joints
        .iter()
        .map(|j| {
            if !observed[j.a] || !observed[j.b] {
                return JointMeasurement {
                    value: None,
                    provenance: Provenance::Unavailable,
                    confidence: None,
                    reasons: [j.a, j.b]
                        .into_iter()
                        .filter(|i| !observed[*i])
                        .map(|i| {
                            reason(
                                "segment_not_observed",
                                Some(sk.segments[i].id.to_string()),
                                "Joint measurement requires an observed, calibrated segment.",
                            )
                        })
                        .collect(),
                };
            }
            let a = rotations[j.a];
            let b = rotations[j.b];
            let value = match j.kind {
                JointKind::LongAxes => Some(angles::axis_angle(
                    &a,
                    &sk.segments[j.a].tail,
                    &b,
                    &sk.segments[j.b].tail,
                )),
                JointKind::FromDownAxis => {
                    Some(angles::from_down_axis(&a, &b, &sk.segments[j.b].tail))
                }
                JointKind::SagittalFromDown => Some(angles::hip_flexion(&a, &b)),
                JointKind::TrunkLean => angles::trunk_forward_lean(&a, &b),
            };
            JointMeasurement {
                value: value.map(|v| v as f32),
                provenance: if value.is_some() {
                    Provenance::Derived
                } else {
                    Provenance::Unavailable
                },
                confidence: None,
                reasons: if value.is_some() {
                    vec![]
                } else {
                    vec![reason(
                        "out_of_plane",
                        None,
                        "Chest axis has too little projection in the pelvis sagittal plane.",
                    )]
                },
            }
        })
        .collect()
}
/// A derived measurement inherits the weaker input confidence; an unknown input stays unknown.
pub fn confidence(sk: &Skeleton, joints: &mut [JointMeasurement], pose: Option<&SkeletonPose>) {
    let Some(pose) = pose else {
        return;
    };
    for (definition, measurement) in sk.joints.iter().zip(joints.iter_mut()) {
        if measurement.value.is_some() {
            measurement.confidence = pose.segments[definition.a]
                .confidence
                .zip(pose.segments[definition.b].confidence)
                .map(|(a, b)| a.min(b));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::simulated::{body::body_pose, motions::Motion};
    #[test]
    fn squat_and_front_kick_angles_match_script_to_one_microradian() {
        let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
        for motion in [Motion::Squat, Motion::FrontKick] {
            for i in 0..80 {
                let t = i as f64 * 0.05;
                let body = body_pose(&sk, motion, t);
                let m = measure(&sk, &body.rotations, &vec![true; sk.len()]);
                let a = motion.angles(t);
                for (joint, expected) in [
                    (0, a.left_knee),
                    (1, a.right_knee),
                    (4, a.left_hip),
                    (5, a.right_hip),
                ] {
                    assert!(
                        (m[joint].value.unwrap() as f64 - expected).abs() < 1e-6,
                        "{motion:?} {t} {joint} {:?} {expected}",
                        m[joint]
                    );
                }
            }
        }
    }
    #[test]
    fn inferred_input_yields_null_with_reason() {
        let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
        let m = measure(
            &sk,
            &vec![Rot::identity(); sk.len()],
            &vec![false; sk.len()],
        );
        assert!(m.iter().all(|m| m.value.is_none() && !m.reasons.is_empty()));
    }
}
