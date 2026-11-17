use super::trackers::sample;
use crate::{
    config::CameraSpec,
    math::{Rot, V3, rot_to_wire, v3_to_wire},
    skeleton::proportions::LANDMARK_SCHEMA_ID,
};
use bansayo_contracts::*;
use nalgebra::{Matrix3, Rotation3};
/// `method` of the camera profiles the simulator declares.
pub const DECLARED_BY_SIMULATOR: &str = "declared_by_simulator";
/// Camera position, world `[right, up, forward]` axes and focal length in pixels.
fn basis(c: &CameraSpec) -> (V3, [V3; 3], f64) {
    let position = V3::from(c.position_m);
    let forward = (V3::from(c.look_at_m) - position).normalize();
    let right = forward.cross(&V3::y()).normalize();
    let up = right.cross(&forward);
    let focal = c.resolution_px[1] as f64 / 2.0 / (c.fov_y_deg.to_radians() / 2.0).tan();
    (position, [right, up, forward], focal)
}
pub fn project(
    c: &CameraSpec,
    seed: u64,
    seq: u64,
    t_us: u64,
    positions: &[V3],
) -> PoseLandmarks2D {
    let (position, [right, up, forward], focal) = basis(c);
    let [width, height] = c.resolution_px;
    let points = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d = p - position;
            let z = d.dot(&forward);
            if z <= 0.0 {
                return None;
            }
            let x = width as f64 / 2.0
                + focal * d.dot(&right) / z
                + sample(&c.id, seed, seq, i as u64 * 2) * c.noise_px;
            let y = height as f64 / 2.0 - focal * d.dot(&up) / z
                + sample(&c.id, seed, seq, i as u64 * 2 + 1) * c.noise_px;
            (x >= 0.0 && y >= 0.0 && x < width as f64 && y < height as f64).then_some(Landmark2D {
                x_px: x as f32,
                y_px: y as f32,
                confidence: 1.0,
            })
        })
        .collect();
    PoseLandmarks2D {
        frame_meta: CameraFrameMetadata {
            device_id: c.id.clone(),
            frame_seq: seq,
            t_us,
            device_time_us: Some(t_us),
            time_quality: TimeQuality::Synchronized,
            image_size_px: c.resolution_px,
        },
        frame: FrameRef::Image { id: c.id.clone() },
        origin: Origin::Simulated,
        schema_id: if positions.len() == 26 {
            BODY_KEYPOINT_SCHEMA_ID.into()
        } else {
            LANDMARK_SCHEMA_ID.into()
        },
        detector: "simulated-projection".into(),
        points,
    }
}
/// The ideal pinhole model `project` uses, as `camera_intrinsics` and `camera_extrinsics`
/// profiles. Camera frame: +X right, +Y down (image y), +Z forward.
pub fn profiles(c: &CameraSpec, created_utc: &str) -> Vec<CalibrationProfile> {
    let (position, [right, up, forward], focal) = basis(c);
    let [width, height] = c.resolution_px;
    let q = Rot::from_rotation_matrix(&Rotation3::from_matrix_unchecked(Matrix3::from_columns(&[
        right, -up, forward,
    ])));
    let intrinsics = serde_json::to_value(CameraIntrinsicsData {
        image_size_px: c.resolution_px,
        fx_px: focal,
        fy_px: focal,
        cx_px: width as f64 / 2.0,
        cy_px: height as f64 / 2.0,
        distortion: [0.0; 5],
    });
    let extrinsics = serde_json::to_value(CameraExtrinsicsData {
        q_world_from_camera: rot_to_wire(&q),
        t_world_from_camera_m: v3_to_wire(&position),
    });
    let intrinsics_dependency = CalibrationDependency {
        kind: CalibrationKind::CameraIntrinsics,
        device_id: Some(c.id.clone()),
    };
    [
        (
            CalibrationKind::CameraIntrinsics,
            "intrinsics",
            intrinsics,
            vec![],
        ),
        (
            CalibrationKind::CameraExtrinsics,
            "extrinsics",
            extrinsics,
            vec![intrinsics_dependency],
        ),
    ]
    .into_iter()
    .filter_map(|(kind, name, data, depends_on)| {
        Some(CalibrationProfile {
            schema_version: CALIBRATION_PROFILE_SCHEMA_VERSION,
            id: ProfileId::new(format!("camera-{name}-{}", c.id)).ok()?,
            kind,
            method: DECLARED_BY_SIMULATOR.into(),
            origin: Origin::Simulated,
            created_utc: created_utc.into(),
            subject: CalibrationSubject {
                device_id: Some(c.id.clone()),
                segment_id: None,
            },
            depends_on,
            data: data.ok()?.as_object()?.clone(),
            quality: Default::default(),
        })
    })
    .collect()
}
