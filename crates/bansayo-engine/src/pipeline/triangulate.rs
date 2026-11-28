//! Multi-camera triangulation of 2D landmarks into world points. Pure.
//!
//! Per landmark: undistort every view to a normalised ray, solve a confidence-weighted linear DLT
//! by SVD, validate by reprojection through the full distortion model, drop the worst view while
//! it exceeds the threshold, and publish nothing that cannot be validated.
// shortcut: linear DLT only, no nonlinear (Gauss-Newton) refinement and no temporal filtering;
// upgrade when real cameras show noise or latency the per-frame linear solve cannot absorb.
use crate::{
    calibration::Store,
    config::TriangulationConfig,
    math::{Rot, V3, angle_between, rot_from_wire, v3_from_wire, v3_to_wire},
    registry::Registry,
};
use bansayo_contracts::*;
use nalgebra::DMatrix;

/// A calibrated pinhole camera with Brown-Conrady distortion.
#[derive(Debug, Clone)]
pub struct Camera {
    pub k: CameraIntrinsicsData,
    pub q_world_from_camera: Rot,
    /// Optical centre in world, metres.
    pub centre: V3,
}
impl Camera {
    pub fn new(k: CameraIntrinsicsData, e: &CameraExtrinsicsData) -> Option<Self> {
        let q_world_from_camera = rot_from_wire(e.q_world_from_camera)?;
        (intrinsics_valid(&k) && extrinsics_valid(e)).then(|| Self {
            k,
            q_world_from_camera,
            centre: v3_from_wire(e.t_world_from_camera_m),
        })
    }
    /// World point to pixels through the distortion model; `None` behind the camera.
    pub fn project(&self, p: &V3) -> Option<[f64; 2]> {
        let c = self
            .q_world_from_camera
            .inverse_transform_vector(&(p - self.centre));
        (c.z > 0.0).then(|| self.distort([c.x / c.z, c.y / c.z]))
    }
    fn distort(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let k = &self.k;
        let [k1, k2, p1, p2, k3] = k.distortion;
        let r2 = x * x + y * y;
        let radial = 1.0 + r2 * (k1 + r2 * (k2 + r2 * k3));
        [
            k.fx_px * (x * radial + 2.0 * p1 * x * y + p2 * (r2 + 2.0 * x * x)) + k.cx_px,
            k.fy_px * (y * radial + p1 * (r2 + 2.0 * y * y) + 2.0 * p2 * x * y) + k.cy_px,
        ]
    }
    /// Pixels to normalised undistorted image coordinates by fixed-point iteration. Exact when
    /// every coefficient is zero; converges for the moderate distortion of ordinary lenses.
    pub fn undistort(&self, [u, v]: [f64; 2]) -> [f64; 2] {
        let k = &self.k;
        let [k1, k2, p1, p2, k3] = k.distortion;
        let d = [(u - k.cx_px) / k.fx_px, (v - k.cy_px) / k.fy_px];
        let [mut x, mut y] = d;
        for _ in 0..30 {
            let r2 = x * x + y * y;
            let radial = 1.0 + r2 * (k1 + r2 * (k2 + r2 * k3));
            let dx = 2.0 * p1 * x * y + p2 * (r2 + 2.0 * x * x);
            let dy = p1 * (r2 + 2.0 * y * y) + 2.0 * p2 * x * y;
            [x, y] = [(d[0] - dx) / radial, (d[1] - dy) / radial];
        }
        [x, y]
    }
}
pub fn intrinsics_valid(k: &CameraIntrinsicsData) -> bool {
    k.fx_px > 0.0
        && k.fy_px > 0.0
        && [k.fx_px, k.fy_px, k.cx_px, k.cy_px]
            .iter()
            .chain(&k.distortion)
            .all(|v| v.is_finite())
}
pub fn extrinsics_valid(e: &CameraExtrinsicsData) -> bool {
    rot_from_wire(e.q_world_from_camera).is_some()
        && e.t_world_from_camera_m.iter().all(|v| v.is_finite())
}
/// One camera's detection of one landmark.
struct View<'a> {
    device: &'a DeviceId,
    camera: &'a Camera,
    px: [f64; 2],
    /// Normalised undistorted image coordinates.
    n: [f64; 2],
    w: f64,
}
/// Triangulates one landmark from its detections; `None` unless the result validates.
pub fn point(
    detections: &[(&DeviceId, &Camera, &Landmark2D)],
    config: &TriangulationConfig,
) -> Option<TriangulatedLandmark3D> {
    point_with_noise_floor(detections, config, 2.0)
}
fn point_with_noise_floor(
    detections: &[(&DeviceId, &Camera, &Landmark2D)],
    config: &TriangulationConfig,
    noise_floor_px: f64,
) -> Option<TriangulatedLandmark3D> {
    let max = config.max_reprojection_error_px;
    let min = config.min_views.max(2) as usize;
    let mut used: Vec<View> = detections
        .iter()
        .filter(|(_, _, l)| l.confidence > 0.0)
        .map(|&(device, camera, l)| {
            let px = [l.x_px.into(), l.y_px.into()];
            View {
                device,
                camera,
                px,
                n: camera.undistort(px),
                w: l.confidence.into(),
            }
        })
        .filter(|v| v.n.iter().chain(&v.px).all(|x| x.is_finite()) && v.w.is_finite())
        .collect();
    if used.len() < min {
        return None;
    }
    let mut rejected = vec![];
    let (m, errors) = loop {
        let m = dlt(&used)?;
        // A point behind a camera (cheirality) has no reprojection: it is the worst possible error.
        let errors: Vec<f64> = used
            .iter()
            .map(|v| {
                v.camera
                    .project(&m)
                    .map_or(f64::INFINITY, |[x, y]| (x - v.px[0]).hypot(y - v.px[1]))
            })
            .collect();
        let (worst, &error) = errors
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))?;
        // With two views the outlier cannot be identified; the final check below rejects the point.
        if error <= max || used.len() <= min {
            break (m, errors);
        }
        rejected.push(used.remove(worst));
    };
    let rays: Vec<V3> = used
        .iter()
        .map(|v| v.camera.q_world_from_camera * V3::new(v.n[0], v.n[1], 1.0))
        .collect();
    let spread = rays
        .iter()
        .enumerate()
        .flat_map(|(i, a)| rays[i + 1..].iter().map(move |b| angle_between(a, b)))
        .fold(0.0, f64::max);
    let rms = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    let confidence = used.iter().map(|v| v.w).sum::<f64>() / used.len() as f64
        * (1.0 - rms / max).clamp(0.0, 1.0);
    (errors.iter().all(|e| *e <= max)
        && spread >= config.min_ray_angle_deg.to_radians()
        && m.iter().all(|v| v.is_finite()))
    .then(|| TriangulatedLandmark3D {
        sigma_m: keypoint_sigma(
            rms,
            noise_floor_px,
            used.iter()
                .map(|v| (m - v.camera.centre).norm())
                .sum::<f64>()
                / used.len() as f64,
            used.iter()
                .map(|v| (v.camera.k.fx_px + v.camera.k.fy_px) / 2.0)
                .sum::<f64>()
                / used.len() as f64,
            spread,
            used.len(),
        )
        .map(|s| s as f32),
        m: v3_to_wire(&m),
        views: used.len() as u32,
        reprojection_error_px: rms as f32,
        confidence: confidence as f32,
        used_views: used.iter().map(|v| v.device.clone()).collect(),
        rejected_views: rejected.iter().map(|v| v.device.clone()).collect(),
        view_diagnostics: Some(
            used.iter()
                .map(|v| view_diagnostic(v, &m, true))
                .chain(rejected.iter().map(|v| view_diagnostic(v, &m, false)))
                .collect(),
        ),
    })
}
fn view_diagnostic(v: &View<'_>, m: &V3, used: bool) -> TriangulatedViewDiagnostic {
    let projected_px = v
        .camera
        .project(m)
        .filter(|p| p.iter().all(|x| x.is_finite()));
    let error_px = projected_px.map(|p| (p[0] - v.px[0]).hypot(p[1] - v.px[1]));
    let direction = (v.camera.q_world_from_camera * V3::new(v.n[0], v.n[1], 1.0)).normalize();
    TriangulatedViewDiagnostic {
        device_id: v.device.clone(),
        observed_px: v.px,
        projected_px,
        error_px,
        ray_origin_m: v3_to_wire(&v.camera.centre),
        ray_direction: v3_to_wire(&direction),
        used,
    }
}

/// Confidence-weighted linear triangulation in normalised camera coordinates, solved by SVD.
fn dlt(views: &[View]) -> Option<V3> {
    let mut rows: Vec<f64> = Vec::with_capacity(views.len() * 8);
    for v in views {
        let r = v
            .camera
            .q_world_from_camera
            .inverse()
            .to_rotation_matrix()
            .into_inner();
        let mut p = r.insert_column(3, 0.0);
        p.set_column(3, &-(r * v.camera.centre));
        for (k, s) in v.n.iter().enumerate() {
            rows.extend(((p.row(2) * *s - p.row(k)) * v.w).iter());
        }
    }
    let svd = DMatrix::from_row_slice(rows.len() / 4, 4, &rows).svd(false, true);
    let h = svd.v_t?.row(svd.singular_values.imin()).clone_owned();
    (h[3].abs() > 1e-12).then(|| V3::new(h[0], h[1], h[2]) / h[3])
}
/// Two frames of one camera further apart than this are not interpolated between.
pub const MAX_INTERPOLATION_GAP_US: u64 = 100_000;

/// Triangulates every calibrated camera aligned to one instant, `None` when fewer than
/// `min_views` cameras are usable.
///
/// Cameras whose newest frame is within `max_skew_ms` of the newest overall take part. The
/// instant is the oldest of their newest frames, so each has a frame at or after it; a camera
/// with a frame before it too is linearly interpolated between the two, otherwise its first
/// frame after the instant is used if within `max_skew_ms`. Free-running cameras therefore
/// contribute the same moment instead of frames up to one period apart.
pub fn triangulate(
    registry: &Registry,
    store: &Store,
    now: u64,
    stale_us: u64,
    config: &TriangulationConfig,
) -> Option<PoseLandmarks3D> {
    triangulate_with_noise_floor(registry, store, now, stale_us, config, 2.0)
}
pub fn triangulate_with_noise_floor(
    registry: &Registry,
    store: &Store,
    now: u64,
    stale_us: u64,
    config: &TriangulationConfig,
    noise_floor_px: f64,
) -> Option<PoseLandmarks3D> {
    let skew_us = config.max_skew_ms * 1000;
    let mut views: Vec<_> = registry
        .entries
        .iter()
        .filter_map(|(id, e)| {
            let frames: Vec<&PoseLandmarks2D> = e
                .landmarks
                .iter()
                .filter(|o| now.saturating_sub(o.t_us) <= stale_us)
                .filter_map(|o| match &o.data {
                    ObservationData::Landmarks2d { landmarks } => Some(landmarks),
                    _ => None,
                })
                .collect();
            let latest = *frames.last()?;
            let (intrinsics, k) = store.camera(CalibrationKind::CameraIntrinsics, id)?;
            let (extrinsics, e) = store.camera(CalibrationKind::CameraExtrinsics, id)?;
            let camera = Camera::new(k, &e)
                .filter(|c| c.k.image_size_px == latest.frame_meta.image_size_px)?;
            Some((id, camera, frames, [intrinsics, extrinsics]))
        })
        .collect();
    let newest = views
        .iter()
        .map(|v| *v.2.last().unwrap_or_else(|| unreachable!()))
        .max_by_key(|l| l.frame_meta.t_us)?;
    let (newest_us, schema, len) = (
        newest.frame_meta.t_us,
        newest.schema_id.clone(),
        newest.points.len(),
    );
    views.retain(|(_, _, frames, _)| {
        frames.last().is_some_and(|l| {
            newest_us - l.frame_meta.t_us <= skew_us
                && l.schema_id == schema
                && l.points.len() == len
        })
    });
    if views.len() < config.min_views as usize {
        return None;
    }
    let t_us = views
        .iter()
        .filter_map(|v| v.2.last().map(|l| l.frame_meta.t_us))
        .min()?;
    let aligned: Vec<_> = views
        .iter()
        .filter_map(|(id, camera, frames, refs)| {
            let compatible = |f: &&&PoseLandmarks2D| {
                f.schema_id == schema
                    && f.points.len() == len
                    && f.frame_meta.image_size_px == camera.k.image_size_px
            };
            let after = frames
                .iter()
                .filter(compatible)
                .find(|f| f.frame_meta.t_us >= t_us)?;
            let before = frames
                .iter()
                .rev()
                .filter(compatible)
                .find(|f| f.frame_meta.t_us <= t_us);
            let points = match before {
                Some(b) if b.frame_meta.t_us == t_us => b.points.clone(),
                Some(b)
                    if after.frame_meta.t_us - b.frame_meta.t_us <= MAX_INTERPOLATION_GAP_US =>
                {
                    interpolate(b, after, t_us)
                }
                _ if after.frame_meta.t_us - t_us <= skew_us => after.points.clone(),
                _ => return None,
            };
            Some((*id, camera, points, after.origin, refs))
        })
        .collect();
    if aligned.len() < config.min_views as usize {
        return None;
    }
    let points = (0..len)
        .map(|i| {
            let detections: Vec<_> = aligned
                .iter()
                .filter_map(|(id, c, points, _, _)| Some((*id, *c, points[i].as_ref()?)))
                .collect();
            point_with_noise_floor(&detections, config, noise_floor_px)
        })
        .collect();
    Some(PoseLandmarks3D::Triangulated {
        t_us,
        frame: FrameRef::World,
        origin: aligned
            .iter()
            .map(|v| v.3)
            .reduce(|a, b| if a == b { a } else { Origin::Live })?,
        schema_id: schema,
        calibration_refs: aligned.iter().flat_map(|v| v.4.clone()).collect(),
        points,
    })
}

/// Landmarks at `t_us` between two frames; a point missing from either is missing.
fn interpolate(a: &PoseLandmarks2D, b: &PoseLandmarks2D, t_us: u64) -> Vec<Option<Landmark2D>> {
    let span = (b.frame_meta.t_us - a.frame_meta.t_us) as f32;
    let s = (t_us - a.frame_meta.t_us) as f32 / span;
    a.points
        .iter()
        .zip(&b.points)
        .map(|(p, q)| {
            let (p, q) = (p.as_ref()?, q.as_ref()?);
            Some(Landmark2D {
                x_px: p.x_px + (q.x_px - p.x_px) * s,
                y_px: p.y_px + (q.y_px - p.y_px) * s,
                confidence: p.confidence.min(q.confidence),
            })
        })
        .collect()
}

/// 04 §8: propagation estimate from the accepted views, with a detector noise floor.
pub fn keypoint_sigma(
    rms_px: f64,
    floor_px: f64,
    distance_m: f64,
    focal_px: f64,
    ray_angle: f64,
    views: usize,
) -> Option<f64> {
    if views < 2 || focal_px <= 0.0 || ray_angle.sin().abs() <= f64::EPSILON {
        return None;
    }
    let sigma = rms_px.max(floor_px) * distance_m
        / focal_px
        / ray_angle.sin().abs()
        / ((views - 1) as f64).sqrt();
    (sigma.is_finite() && sigma >= 0.0).then_some(sigma)
}
