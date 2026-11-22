//! Validation at live-ingest and persisted-recording boundaries. Internal solver stages trust it.
use crate::math::rot_from_wire;
use bansayo_contracts::*;
pub fn descriptor(d: &DeviceDescriptor) -> Result<(), String> {
    if d.nominal_rate_hz
        .is_some_and(|r| !r.is_finite() || r <= 0.0 || r > 1000.0)
    {
        return Err("Device nominal rate is invalid.".into());
    }
    if let Some(camera) = &d.camera {
        if camera.resolution_px.contains(&0) {
            return Err("Camera resolution is invalid.".into());
        }
        if let Some(schema) = &camera.landmark_schema
            && (schema.names.len() > 1024
                || schema
                    .edges
                    .iter()
                    .flatten()
                    .any(|i| *i as usize >= schema.names.len()))
        {
            return Err("Camera landmark schema is invalid.".into());
        }
    }
    if (d.kind == DeviceKind::Camera) != d.camera.is_some() {
        return Err("Device camera descriptor does not match its kind.".into());
    }
    Ok(())
}
pub fn observation(
    o: &SensorObservation,
    d: &DeviceDescriptor,
    accepted_us: u64,
) -> Result<(), String> {
    if o.device_id != d.id || o.source_id != d.source_id || o.origin != d.origin {
        return Err("Observation identity does not match its device.".into());
    }
    if o.t_us > accepted_us
        || o.confidence
            .is_some_and(|q| !q.is_finite() || !(0.0..=1.0).contains(&q))
    {
        return Err("Observation timestamp or confidence is invalid.".into());
    }
    let capability = match &o.data {
        ObservationData::Orientation { frame, q } => {
            if rot_from_wire(*q).is_none()
                || !matches!(frame,FrameRef::SensorReference{id} if id==&o.device_id)
            {
                return Err("Orientation quaternion or reference frame is invalid.".into());
            }
            SensorCapability::Orientation
        }
        ObservationData::Landmarks2d { landmarks } => {
            let Some(camera) = &d.camera else {
                return Err("Landmarks require a camera descriptor.".into());
            };
            if landmarks.frame_meta.device_id != o.device_id
                || landmarks.frame_meta.t_us != o.t_us
                || landmarks.frame_meta.image_size_px != camera.resolution_px
                || landmarks.origin != o.origin
                || !matches!(&landmarks.frame,FrameRef::Image{id} if id==&o.device_id)
            {
                return Err("Landmark metadata is inconsistent.".into());
            }
            if let Some(schema) = &camera.landmark_schema
                && (landmarks.schema_id != schema.id
                    || landmarks.points.len() != schema.names.len())
            {
                return Err("Landmarks do not match the declared schema.".into());
            }
            if landmarks.points.len() > 1024
                || landmarks.points.iter().flatten().any(|p| {
                    !p.x_px.is_finite()
                        || !p.y_px.is_finite()
                        || !p.confidence.is_finite()
                        || !(0.0..=1.0).contains(&p.confidence)
                })
            {
                return Err("Landmark coordinates or confidence are invalid.".into());
            }
            SensorCapability::Landmarks2d
        }
        ObservationData::AngularVelocity { rad_s, .. } => {
            finite(rad_s)?;
            SensorCapability::AngularVelocity
        }
        ObservationData::Acceleration { m_s2, .. } => {
            finite(m_s2)?;
            SensorCapability::Acceleration
        }
        ObservationData::Position { m, .. } => {
            finite(m)?;
            SensorCapability::Position
        }
        ObservationData::Distance { m } => {
            if !m.is_finite() || *m < 0.0 {
                return Err("Distance is invalid.".into());
            }
            SensorCapability::Distance
        }
        ObservationData::Contact { force_n, .. } => {
            if force_n.is_some_and(|v| !v.is_finite() || v < 0.0) {
                return Err("Contact force is invalid.".into());
            }
            SensorCapability::Contact
        }
        ObservationData::Diagnostics {
            battery_level,
            quality,
            ..
        } => {
            if battery_level
                .iter()
                .chain(quality.iter())
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err("Device diagnostic quality is invalid.".into());
            }
            return Ok(());
        }
        ObservationData::RigidPose {
            frame,
            position_m,
            rotation,
        } => {
            // Only OpenXR devices report rigid poses, and only in raw stage space (Phase 01).
            let (p, q) = crate::sources::xr::pose_ok(
                position_m,
                rotation,
                crate::sources::xr::MAX_POSITION_NORM_M,
                crate::sources::xr::MAX_QUATERNION_NORM_ERROR,
            );
            if d.kind != DeviceKind::XrTrackedDevice || *frame != FrameRef::XrStage || !p || !q {
                return Err("Rigid pose frame, device kind or values are invalid.".into());
            }
            SensorCapability::RigidPose
        }
        ObservationData::Landmarks3d { .. } => {
            return Err("3D landmarks are not supported by this engine build.".into());
        }
    };
    if !d.capabilities.contains(&capability) {
        return Err("Observation capability was not declared by the device.".into());
    }
    Ok(())
}
fn finite(values: &[f32]) -> Result<(), String> {
    if values.iter().any(|v| !v.is_finite()) {
        Err("Observation contains a non-finite value.".into())
    } else {
        Ok(())
    }
}
