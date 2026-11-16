//! Ground truth of the simulated body: segment world rotations and positions as a pure function
//! of (motion, time). Feet are placed on the floor by the same floor-contact rule as the solver.

use bansayo_contracts::SegmentId;

use super::motions::{Motion, ScriptedAngles};
use crate::math::{Rot, V3, rot_x, rot_z};
use crate::skeleton::{Skeleton, floor_contact_root, forward_kinematics};

/// The body at one instant, in skeleton segment order.
#[derive(Debug, Clone)]
pub struct BodyPose {
    pub rotations: Vec<Rot>,
    pub positions: Vec<V3>,
    pub root: V3,
}

fn set(sk: &Skeleton, world: &mut [Option<Rot>], id: &str, r: Rot) {
    if let Some(i) = SegmentId::new(id).ok().and_then(|s| sk.index_of(&s)) {
        world[i] = Some(r);
    }
}

/// Arm rotations relative to the chest: `elevation` 0 = hanging, 90° = T-pose.
fn arm(left: bool, elevation: f64) -> Rot {
    let hang = std::f64::consts::FRAC_PI_2;
    if left {
        rot_z(elevation - hang)
    } else {
        rot_z(hang - elevation)
    }
}

/// World rotations of the explicitly animated segments; the rest follow their parent.
fn animate(sk: &Skeleton, motion: Motion, a: &ScriptedAngles) -> Vec<Rot> {
    let mut w: Vec<Option<Rot>> = vec![None; sk.len()];
    let pelvis = rot_x(a.pelvis_tilt);
    let chest = rot_x(a.trunk_lean);
    set(sk, &mut w, "pelvis", pelvis);
    set(sk, &mut w, "spine", pelvis.slerp(&chest, 0.5));
    set(sk, &mut w, "chest", chest);
    set(
        sk,
        &mut w,
        "left_upper_arm",
        chest * arm(true, a.left_shoulder),
    );
    set(
        sk,
        &mut w,
        "right_upper_arm",
        chest * arm(false, a.right_shoulder),
    );
    for (side, hip, knee) in [
        ("left", a.left_hip, a.left_knee),
        ("right", a.right_hip, a.right_knee),
    ] {
        // Thigh forward angle from vertical in world, and shin = thigh flexed back by the knee.
        let thigh_fwd = hip - a.pelvis_tilt;
        set(sk, &mut w, &format!("{side}_thigh"), rot_x(-thigh_fwd));
        set(sk, &mut w, &format!("{side}_shin"), rot_x(knee - thigh_fwd));
        if motion == Motion::Squat {
            set(sk, &mut w, &format!("{side}_foot"), Rot::identity());
        }
    }
    let mut out: Vec<Rot> = Vec::with_capacity(sk.len());
    for (i, s) in sk.segments.iter().enumerate() {
        let r = w[i].unwrap_or_else(|| s.parent.map_or(Rot::identity(), |p| out[p]));
        out.push(r);
    }
    out
}

/// Ground-truth pose of `motion` at simulation time `t_s`.
pub fn body_pose(sk: &Skeleton, motion: Motion, t_s: f64) -> BodyPose {
    let rotations = animate(sk, motion, &motion.angles(t_s));
    let root = floor_contact_root(sk, &rotations);
    let positions = forward_kinematics(sk, &rotations, root);
    BodyPose {
        rotations,
        positions,
        root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::proportions::MODEL_ID;

    fn sk() -> Skeleton {
        Skeleton::builtin(MODEL_ID, 1.75).expect("builtin")
    }

    #[test]
    fn reference_pose_is_identity_t_pose() {
        let sk = sk();
        let pose = body_pose(&sk, Motion::ReferencePose, 1.234);
        assert!(pose.rotations.iter().all(|r| r.angle() < 1e-12));
        assert!((pose.root.y - 0.530 * 1.75).abs() < 1e-9);
    }

    #[test]
    fn feet_stay_on_the_floor_and_under_the_hips_in_a_squat() {
        let sk = sk();
        for i in 0..40 {
            let pose = body_pose(&sk, Motion::Squat, i as f64 * 0.1);
            let foot = sk
                .index_of(&SegmentId::new("left_foot").expect("id"))
                .expect("foot");
            let heel =
                pose.positions[foot] + pose.rotations[foot] * sk.segments[foot].contact_points[0];
            assert!(heel.y.abs() < 1e-9, "heel at {}", heel.y);
            assert!(
                pose.positions[foot].z.abs() < 0.01,
                "ankle drift {}",
                pose.positions[foot].z
            );
        }
        let bottom = body_pose(&sk, Motion::Squat, 2.0);
        assert!(
            bottom.root.y < 0.7,
            "pelvis lowers in a squat: {}",
            bottom.root.y
        );
    }

    #[test]
    fn kicking_foot_leaves_the_floor() {
        let sk = sk();
        let pose = body_pose(&sk, Motion::FrontKick, 1.25);
        let right = sk
            .index_of(&SegmentId::new("right_foot").expect("id"))
            .expect("foot");
        assert!(pose.positions[right].y > 0.3);
    }
}
