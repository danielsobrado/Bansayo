//! Forward kinematics and floor-contact root placement.

use super::Skeleton;
use crate::math::{Rot, V3};

/// Segment origins in world from world rotations and a root position. Parents precede children.
pub fn forward_kinematics(skeleton: &Skeleton, rotations: &[Rot], root: V3) -> Vec<V3> {
    let mut out: Vec<V3> = Vec::with_capacity(skeleton.len());
    for s in &skeleton.segments {
        let p = match s.parent {
            None => root,
            Some(p) => out[p] + rotations[p] * s.offset,
        };
        out.push(p);
    }
    out
}

/// Root position that puts the lowest contact point on `y = 0`, horizontally at the origin.
/// Falls back to the rest root height when the skeleton has no contact points.
pub fn floor_contact_root(skeleton: &Skeleton, rotations: &[Rot]) -> V3 {
    let rest_root = skeleton.segments.first().map_or(V3::zeros(), |s| s.offset);
    let positions = forward_kinematics(skeleton, rotations, V3::zeros());
    let lowest = skeleton
        .segments
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let position = positions[i];
            s.contact_points
                .iter()
                .map(move |c| (position + rotations[i] * c).y)
        })
        .reduce(f64::min);
    match lowest {
        Some(y) => V3::new(0.0, -y, 0.0),
        None => V3::new(0.0, rest_root.y, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::proportions::MODEL_ID;

    #[test]
    fn rest_pose_stands_on_the_floor_with_pelvis_at_rest_height() {
        let sk = Skeleton::builtin(MODEL_ID, 1.75).expect("builtin");
        let rot = vec![Rot::identity(); sk.len()];
        let root = floor_contact_root(&sk, &rot);
        assert!((root.y - 0.530 * 1.75).abs() < 1e-9);
        let pos = forward_kinematics(&sk, &rot, root);
        let head = sk
            .segments
            .iter()
            .position(|s| s.id.as_str() == "head")
            .expect("head");
        assert!((pos[head].y + sk.segments[head].tail.y - 1.75).abs() < 1e-9);
    }

    #[test]
    fn child_follows_parent_rotation() {
        let sk = Skeleton::builtin(MODEL_ID, 1.75).expect("builtin");
        let mut rot = vec![Rot::identity(); sk.len()];
        let thigh = sk
            .segments
            .iter()
            .position(|s| s.id.as_str() == "left_thigh")
            .expect("thigh");
        let shin = thigh + 1;
        rot[thigh] = crate::math::rot_x(-std::f64::consts::FRAC_PI_2);
        let pos = forward_kinematics(&sk, &rot, V3::zeros());
        // Thigh flexed 90° forward: the knee is in front of the hip at hip height.
        let d = pos[shin] - pos[thigh];
        assert!(d.y.abs() < 1e-9 && (d.z - 0.245 * 1.75).abs() < 1e-6);
    }
}
