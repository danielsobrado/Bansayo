use super::trackers::sample;
use crate::{config::CameraSpec, math::V3, skeleton::proportions::LANDMARK_SCHEMA_ID};
use kinexa_contracts::*;
pub fn project(
    c: &CameraSpec,
    seed: u64,
    seq: u64,
    t_us: u64,
    positions: &[V3],
) -> PoseLandmarks2D {
    let position = V3::from(c.position_m);
    let forward = (V3::from(c.look_at_m) - position).normalize();
    let right = forward.cross(&V3::y()).normalize();
    let up = right.cross(&forward);
    let [width, height] = c.resolution_px;
    let focal = height as f64 / 2.0 / (c.fov_y_deg.to_radians() / 2.0).tan();
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
        schema_id: LANDMARK_SCHEMA_ID.into(),
        detector: "simulated-projection".into(),
        points,
    }
}
