use super::solve::Solved;
use crate::registry::reason;
use bansayo_contracts::*;
pub fn features(devices: &[DeviceStatus], solved: &Solved) -> Vec<FeatureStatus> {
    use FeatureId::*;
    use FeatureState::*;
    let trackers = devices
        .iter()
        .any(|d| d.descriptor.kind == DeviceKind::ImuTracker);
    let cameras = devices
        .iter()
        .filter(|d| d.descriptor.kind == DeviceKind::Camera);
    let calibrated = cameras
        .clone()
        .filter(|d| {
            d.calibration
                .iter()
                .all(|c| c.state == CalibrationState::Valid)
        })
        .count();
    let (cameras, observed, all) = (
        cameras.count(),
        solved.quality.observed_segments,
        solved.quality.total_segments,
    );
    let measured_root = solved
        .pose
        .as_ref()
        .is_some_and(|p| p.root_position_provenance == Provenance::Measured);
    let reconstructed = match &solved.landmarks_3d {
        Some(PoseLandmarks3D::Triangulated { points, .. }) => {
            Some(points.iter().all(Option::is_some))
        }
        _ => None,
    };
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
            SegmentOrientation | SkeletonPose | JointAngles if !trackers && observed > 0 => (
                Degraded,
                "camera_only_skeleton",
                "Fitted to triangulated landmarks: axial twist and unseen segments are inferred.",
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
            RootPosition if measured_root => (Available, "", ""),
            RootPosition if trackers => (
                Degraded,
                "root_position_inferred",
                "Root placement uses assumed body proportions and floor contact.",
            ),
            Landmarks2d if cameras > 0 => (Available, "", ""),
            Landmarks2d => (Unavailable, "no_cameras", "No camera is configured."),
            RootPosition | GroundContact => (
                Unavailable,
                "no_positional_reference",
                "No calibrated positional reference is available.",
            ),
            Landmarks3dTriangulated => match reconstructed {
                Some(true) => (Available, "", ""),
                Some(false) => (
                    Degraded,
                    "landmarks_partially_reconstructed",
                    "Some landmarks are seen by too few cameras or failed the reprojection check.",
                ),
                None if cameras < 2 => (
                    Unavailable,
                    "insufficient_cameras",
                    "Triangulation needs at least two cameras.",
                ),
                None if calibrated < 2 => (
                    Unavailable,
                    "camera_calibration_missing",
                    "Fewer than two cameras have valid intrinsics and extrinsics.",
                ),
                None => (
                    Unavailable,
                    "insufficient_synchronized_views",
                    "Too few calibrated cameras delivered fresh, synchronised views.",
                ),
            },
            CameraImuFusion => (
                Unavailable,
                "not_implemented",
                "Camera and IMU fusion is not implemented; camera and tracker data are combined only for the root position.",
            ),
            _ => (
                Unavailable,
                "not_implemented",
                "This feature is not implemented in this build.",
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
