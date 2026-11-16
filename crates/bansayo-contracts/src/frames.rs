//! Spatial primitives and frame references.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DeviceId, SegmentId};

/// A 3-vector `[x, y, z]`. Frame and unit are named by the field that holds it.
pub type Vec3 = [f32; 3];

/// A unit Hamilton quaternion `[x, y, z, w]`. `q_a_from_b` maps coordinates in `b` into `a`.
pub type Quat = [f32; 4];

/// The frame a spatial value is expressed in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrameRef {
    /// World: right-handed, +Y up, +Z forward, metres, floor at `y = 0`.
    World,
    /// The XR runtime's stage space; world axes and units, related to world by yaw and translation.
    XrStage,
    /// A device's body frame, right-handed.
    Sensor { id: DeviceId },
    /// Gravity-aligned frame of an IMU's orientation output: +Y up, heading arbitrary.
    SensorReference { id: DeviceId },
    /// Camera optical frame: +X right, +Y down, +Z along the optical axis, metres.
    Camera { id: DeviceId },
    /// Image pixels: origin top-left, +x right, +y down.
    Image { id: DeviceId },
    /// A detector's non-metric model space; never convertible to world.
    Model { id: DeviceId },
    /// A body segment frame; aligned with world axes in the rest pose.
    Segment { id: SegmentId },
}
