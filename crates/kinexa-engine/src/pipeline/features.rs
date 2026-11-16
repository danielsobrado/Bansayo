use crate::registry::reason;
use kinexa_contracts::*;
pub fn features(trackers: bool, cameras: bool, observed: u32, all: u32) -> Vec<FeatureStatus> {
    use FeatureId::*;
    use FeatureState::*;
    [
        SegmentOrientation,
        SkeletonPose,
        JointAngles,
        RootPosition,
        Landmarks2d,
        Landmarks3dTriangulated,
        CameraImuFusion,
        GroundContact,
        Recording,
        Playback,
    ]
    .into_iter()
    .map(|id| {
        let (state, code, message) = match id {
            SegmentOrientation | SkeletonPose | JointAngles if trackers && observed > 0 => (
                if observed == all { Available } else { Degraded },
                "partial_observation",
                "Untracked segments are inferred; dependent joint values remain unavailable.",
            ),
            SegmentOrientation | SkeletonPose | JointAngles if trackers => (
                Degraded,
                "mounting_calibration_missing",
                "Capture the reference pose to derive segment rotations.",
            ),
            SegmentOrientation | SkeletonPose | JointAngles => (
                Unavailable,
                "no_orientation_devices",
                "No orientation trackers are configured.",
            ),
            RootPosition if trackers => (
                Degraded,
                "root_position_inferred",
                "Root placement uses assumed body proportions and floor contact.",
            ),
            Landmarks2d if cameras => (Available, "", ""),
            Landmarks2d => (Unavailable, "no_cameras", "No camera is configured."),
            RootPosition | GroundContact => (
                Unavailable,
                "no_positional_reference",
                "No calibrated positional reference is available.",
            ),
            _ => (
                Unavailable,
                "not_implemented",
                "This feature is not implemented in Stage 1.",
            ),
        };
        FeatureStatus {
            id,
            state,
            reasons: if state == Available {
                vec![]
            } else {
                vec![reason(code, None, message)]
            },
        }
    })
    .collect()
}
