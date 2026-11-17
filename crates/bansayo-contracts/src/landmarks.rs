//! Camera landmarks: 2D image points, model-relative 3D and triangulated world 3D, kept apart.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DeviceId, FrameRef, Origin, ProfileId, TimeQuality, TimeUs, Vec3};

/// Names and connectivity of a landmark set; points are index-aligned with `names`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LandmarkSchema {
    /// e.g. `bansayo.body.v1.joints`.
    pub id: String,
    pub names: Vec<String>,
    /// Pairs of indices into `names` drawn as bones.
    pub edges: Vec<[u32; 2]>,
}

/// Timing and size of the camera frame landmarks were detected in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CameraFrameMetadata {
    pub device_id: DeviceId,
    pub frame_seq: u64,
    /// Capture instant, session clock.
    pub t_us: TimeUs,
    /// Device clock, if the camera has one.
    pub device_time_us: Option<u64>,
    pub time_quality: TimeQuality,
    /// Image width and height, pixels.
    pub image_size_px: [u32; 2],
}

/// One detected 2D landmark, pixels in the image frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Landmark2D {
    pub x_px: f32,
    pub y_px: f32,
    /// 0..1.
    pub confidence: f32,
}

/// Single-camera 2D landmarks in the `image` frame. Not metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PoseLandmarks2D {
    pub frame_meta: CameraFrameMetadata,
    /// Always `{ type: "image", id }`.
    pub frame: FrameRef,
    pub origin: Origin,
    /// Matches the camera's `LandmarkSchema.id`.
    pub schema_id: String,
    /// e.g. `simulated-projection`.
    pub detector: String,
    /// Index-aligned with the schema; `null` means not detected.
    pub points: Vec<Option<Landmark2D>>,
}

/// One model-inferred 3D landmark, model units in the `model` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ModelLandmark3D {
    pub p: Vec3,
    pub confidence: f32,
}

/// One triangulated landmark, metres in the world frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TriangulatedLandmark3D {
    pub m: Vec3,
    /// Propagated position uncertainty in metres; not a validated accuracy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub sigma_m: Option<f32>,
    /// Number of camera views used; equals `used_views.len()`.
    pub views: u32,
    /// RMS reprojection error over the used views, pixels.
    pub reprojection_error_px: f32,
    pub confidence: f32,
    /// Cameras whose observation contributed to `m`.
    #[serde(default)]
    pub used_views: Vec<DeviceId>,
    /// Cameras that observed the landmark but were rejected as outliers.
    #[serde(default)]
    pub rejected_views: Vec<DeviceId>,
}

/// 3D landmarks. The two kinds have different fields and are never converted into each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PoseLandmarks3D {
    /// Model-inferred, root-relative, unknown scale. Never placed in world.
    ModelRelative {
        frame_meta: CameraFrameMetadata,
        frame: FrameRef,
        origin: Origin,
        schema_id: String,
        detector: String,
        points: Vec<Option<ModelLandmark3D>>,
    },
    /// Multi-camera triangulation; metric, world frame.
    Triangulated {
        t_us: TimeUs,
        frame: FrameRef,
        origin: Origin,
        schema_id: String,
        calibration_refs: Vec<ProfileId>,
        points: Vec<Option<TriangulatedLandmark3D>>,
    },
}
