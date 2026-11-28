//! OpenXR head and controller-grip poses from the VR client (`GET /api/v1/ingest/xr`).
//!
//! The three devices are fixed and announced at start when `xr.enabled`; the client never names
//! devices, frames or origins. Each frame is stamped with one server receive time (`receive_time`):
//! the client's monotonic clock is kept as `device_time_us` and is never treated as session time.
//! Poses stay in raw XR stage space; nothing here aligns them to world or to the skeleton.
use crate::config::XrConfig;
use bansayo_contracts::*;

pub const SOURCE_ID: &str = "xr-openxr";
/// Device ids in `XrRole::ORDER`.
pub const DEVICE_IDS: [&str; 3] = ["XR-HMD", "XR-LEFT-GRIP", "XR-RIGHT-GRIP"];
/// Ceilings for the configurable bounds; the pipeline validator applies these to every
/// `rigid_pose` (live and recorded), the adapter the tighter configured values.
pub const MAX_POSITION_NORM_M: f32 = 50.0;
pub const MAX_QUATERNION_NORM_ERROR: f32 = 0.02;

fn index(role: XrRole) -> usize {
    match role {
        XrRole::Head => 0,
        XrRole::LeftGrip => 1,
        XrRole::RightGrip => 2,
    }
}

#[allow(clippy::expect_used)]
pub fn device_id(role: XrRole) -> DeviceId {
    DeviceId::new(DEVICE_IDS[index(role)]).expect("reserved XR device ids are valid")
}

#[allow(clippy::expect_used)]
fn source_id() -> SourceId {
    SourceId::new(SOURCE_ID).expect("reserved XR source id is valid")
}

/// The three XR descriptors, or none when XR is off.
pub fn descriptors(config: &XrConfig) -> Vec<DeviceDescriptor> {
    if !config.enabled {
        return vec![];
    }
    let models = [
        "OpenXR HMD",
        "OpenXR left controller grip",
        "OpenXR right controller grip",
    ];
    XrRole::ORDER
        .iter()
        .zip(models)
        .map(|(role, model)| DeviceDescriptor {
            id: device_id(*role),
            source_id: source_id(),
            kind: DeviceKind::XrTrackedDevice,
            origin: Origin::Live,
            model: Some(model.into()),
            firmware: None,
            transport: Transport::Websocket,
            capabilities: vec![SensorCapability::RigidPose],
            nominal_rate_hz: Some(config.nominal_rate_hz as f32),
            camera: None,
        })
        .collect()
}

/// Why a frame was refused. `code` is sent to the client and stable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XrValidationError {
    #[error("samples must be head, left_grip, right_grip in that order")]
    RoleOrder,
    #[error("{0:?}: a tracked sample needs position and rotation, an untracked one neither")]
    TrackedMismatch(XrRole),
    #[error("{0:?}: position is non-finite or farther than the configured bound")]
    Position(XrRole),
    #[error("{0:?}: rotation is non-finite or not a unit quaternion")]
    Rotation(XrRole),
    #[error("seq must strictly increase on a connection")]
    Sequence,
    #[error("client_time_us went backwards on a connection")]
    ClientTime,
    #[error("frames arrive faster than xr.max_ingest_rate_hz")]
    OverRate,
}
impl XrValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RoleOrder => "xr_role_order",
            Self::TrackedMismatch(_) => "xr_tracked_mismatch",
            Self::Position(_) => "xr_position_invalid",
            Self::Rotation(_) => "xr_rotation_invalid",
            Self::Sequence => "xr_sequence",
            Self::ClientTime => "xr_client_time",
            Self::OverRate => "xr_over_rate",
        }
    }
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// Whether a rigid pose is finite, within `max_norm_m` and has a unit quaternion within `tol`.
pub fn pose_ok(position_m: &Vec3, rotation: &Quat, max_norm_m: f32, tol: f32) -> (bool, bool) {
    let p = position_m.iter().all(|c| c.is_finite()) && norm(position_m) <= max_norm_m;
    let q = rotation.iter().all(|c| c.is_finite()) && (norm(rotation) - 1.0).abs() <= tol;
    (p, q)
}

/// Structure and value checks of one frame on its own (ordering is per connection, see `XrLink`).
pub fn validate_frame(frame: &XrFrame, config: &XrConfig) -> Result<(), XrValidationError> {
    if frame.samples.iter().map(|s| s.role).ne(XrRole::ORDER) {
        return Err(XrValidationError::RoleOrder);
    }
    for s in &frame.samples {
        match (s.tracked, &s.position_m, &s.rotation_xyzw) {
            (true, Some(p), Some(q)) => {
                let (p_ok, q_ok) = pose_ok(
                    p,
                    q,
                    config.max_position_norm_m,
                    config.max_quaternion_norm_error,
                );
                if !p_ok {
                    return Err(XrValidationError::Position(s.role));
                }
                if !q_ok {
                    return Err(XrValidationError::Rotation(s.role));
                }
            }
            (false, None, None) => {}
            _ => return Err(XrValidationError::TrackedMismatch(s.role)),
        }
    }
    Ok(())
}

/// The observation for one tracked sample of a validated frame; `None` when untracked.
pub fn observation(
    frame: &XrFrame,
    sample: &XrDeviceSample,
    received_us: TimeUs,
) -> Option<SensorObservation> {
    let (true, Some(position_m), Some(rotation)) =
        (sample.tracked, sample.position_m, sample.rotation_xyzw)
    else {
        return None;
    };
    Some(SensorObservation {
        device_id: device_id(sample.role),
        source_id: source_id(),
        origin: Origin::Live,
        seq: frame.seq,
        t_us: received_us,
        device_time_us: Some(frame.client_time_us),
        time_quality: TimeQuality::ReceiveTime,
        calibration_ref: None,
        // Tracking validity says nothing about accuracy.
        confidence: None,
        data: ObservationData::RigidPose {
            frame: FrameRef::XrStage,
            position_m,
            rotation,
        },
    })
}

/// Per-connection ingest state: ordering, rate and each device's link. A new connection starts
/// fresh, so its first tracked sample re-links the device and the registry drops old seq history.
#[derive(Debug, Default)]
pub struct XrLink {
    last: Option<(u64, u64)>,
    /// `None` until this connection has said anything about the device.
    linked: [Option<bool>; 3],
    next_allowed_us: Option<u64>,
}

impl XrLink {
    /// Validates and rate-limits a frame, then emits link changes and observations through
    /// `send`, which returns `false` when the event could not be queued. A device's state only
    /// advances on a queued link event, and its pose is only sent after its link is up.
    /// Returns the number of observations queued.
    pub fn frame(
        &mut self,
        frame: &XrFrame,
        config: &XrConfig,
        received_us: TimeUs,
        mut send: impl FnMut(SourceEvent) -> bool,
    ) -> Result<usize, XrValidationError> {
        validate_frame(frame, config)?;
        if let Some((seq, client)) = self.last {
            if frame.seq <= seq {
                return Err(XrValidationError::Sequence);
            }
            if frame.client_time_us < client {
                return Err(XrValidationError::ClientTime);
            }
        } else if frame.seq != 0 {
            return Err(XrValidationError::Sequence);
        }
        self.last = Some((frame.seq, frame.client_time_us));
        // Average-rate gate: up to half an interval of arrival jitter, no lasting burst credit.
        let interval = (1e6 / config.max_ingest_rate_hz) as u64;
        let jitter = interval / 2;
        let next = self.next_allowed_us.unwrap_or(received_us);
        if received_us + jitter < next {
            return Err(XrValidationError::OverRate);
        }
        self.next_allowed_us = Some(next.max(received_us.saturating_sub(jitter)) + interval);
        let mut queued = 0;
        for sample in &frame.samples {
            let i = index(sample.role);
            if self.linked[i] != Some(sample.tracked) {
                let event = SourceEvent::DeviceLink {
                    device_id: device_id(sample.role),
                    connected: sample.tracked,
                    t_us: received_us,
                };
                if !send(event) {
                    continue;
                }
                self.linked[i] = Some(sample.tracked);
            }
            if let Some(observation) = observation(frame, sample, received_us)
                && send(SourceEvent::Observation { observation })
            {
                queued += 1;
            }
        }
        Ok(queued)
    }

    pub fn has_linked_devices(&self) -> bool {
        self.linked.contains(&Some(true))
    }

    /// Marks every device this connection has linked as disconnected (idle, close, shutdown).
    pub fn unlink_all(&mut self, t_us: TimeUs, mut send: impl FnMut(SourceEvent) -> bool) {
        for role in XrRole::ORDER {
            let i = index(role);
            if self.linked[i] == Some(true)
                && send(SourceEvent::DeviceLink {
                    device_id: device_id(role),
                    connected: false,
                    t_us,
                })
            {
                self.linked[i] = Some(false);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn on() -> XrConfig {
        XrConfig {
            enabled: true,
            ..Default::default()
        }
    }
    fn sample(role: XrRole, pose: Option<(Vec3, Quat)>) -> XrDeviceSample {
        XrDeviceSample {
            role,
            tracked: pose.is_some(),
            position_m: pose.map(|p| p.0),
            rotation_xyzw: pose.map(|p| p.1),
        }
    }
    const I: Quat = [0.0, 0.0, 0.0, 1.0];
    fn frame(seq: u64, t: u64, right: bool) -> XrFrame {
        XrFrame {
            seq,
            client_time_us: t,
            samples: [
                sample(XrRole::Head, Some(([0.0, 1.72, 0.0], I))),
                sample(XrRole::LeftGrip, Some(([-0.35, 1.25, -0.22], I))),
                sample(XrRole::RightGrip, right.then_some(([0.35, 1.25, -0.22], I))),
            ],
        }
    }
    fn run(
        link: &mut XrLink,
        f: &XrFrame,
        at: u64,
    ) -> (Result<usize, XrValidationError>, Vec<SourceEvent>) {
        let mut out = vec![];
        let r = link.frame(f, &on(), at, |e| {
            out.push(e);
            true
        });
        (r, out)
    }

    #[test]
    fn xr_descriptors_are_stable_and_disabled_by_default() {
        assert!(descriptors(&XrConfig::default()).is_empty());
        let d = descriptors(&on());
        let ids: Vec<_> = d.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, DEVICE_IDS);
        for d in &d {
            assert_eq!(d.source_id.as_str(), SOURCE_ID);
            assert_eq!(d.kind, DeviceKind::XrTrackedDevice);
            assert_eq!(d.origin, Origin::Live);
            assert_eq!(d.transport, Transport::Websocket);
            assert_eq!(d.capabilities, vec![SensorCapability::RigidPose]);
            assert_eq!(d.nominal_rate_hz, Some(60.0));
            assert!(crate::sources::validation::descriptor(d).is_ok());
        }
    }

    #[test]
    fn xr_adapter_accepts_valid_stage_pose() {
        let f = frame(42, 123_456_789, true);
        let o = observation(&f, &f.samples[0], 4_567_890).unwrap();
        assert_eq!(
            serde_json::to_value(&o).unwrap(),
            serde_json::json!({
                "device_id": "XR-HMD", "source_id": "xr-openxr", "origin": "live", "seq": 42,
                "t_us": 4_567_890, "device_time_us": 123_456_789u64, "time_quality": "receive_time",
                "calibration_ref": null, "confidence": null,
                "data": {"kind": "rigid_pose", "frame": {"type": "xr_stage"},
                         "position_m": [0.0, 1.72f32, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0]}
            })
        );
        let (r, events) = run(&mut XrLink::default(), &frame(0, 10, true), 1_000);
        assert_eq!(r, Ok(3));
        // Link before pose for every device; one shared server time.
        let kinds: Vec<_> = events
            .iter()
            .map(|e| match e {
                SourceEvent::DeviceLink {
                    device_id,
                    connected: true,
                    t_us: 1_000,
                } => format!("link {device_id}"),
                SourceEvent::Observation { observation: o } if o.t_us == 1_000 => {
                    format!("pose {}", o.device_id)
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "link XR-HMD",
                "pose XR-HMD",
                "link XR-LEFT-GRIP",
                "pose XR-LEFT-GRIP",
                "link XR-RIGHT-GRIP",
                "pose XR-RIGHT-GRIP"
            ]
        );
    }

    #[test]
    fn xr_adapter_rejects_nonfinite_and_nonunit_quaternion() {
        let with_q = |q: Quat| {
            let mut f = frame(0, 0, true);
            f.samples[1].rotation_xyzw = Some(q);
            validate_frame(&f, &on())
        };
        let bad = XrValidationError::Rotation(XrRole::LeftGrip);
        assert_eq!(with_q([f32::NAN, 0.0, 0.0, 1.0]), Err(bad.clone()));
        assert_eq!(with_q([0.0, 0.0, 0.0, f32::INFINITY]), Err(bad.clone()));
        assert_eq!(with_q([0.0; 4]), Err(bad.clone()));
        assert_eq!(with_q([0.0, 0.0, 0.0, 1.006]), Err(bad.clone()));
        assert_eq!(with_q([0.0, 0.0, 0.0, 0.994]), Err(bad));
        assert_eq!(with_q([0.0, 0.0, 0.0, 1.004]), Ok(()));
        // Raw values are kept: an accepted near-unit quaternion is not silently renormalised.
        let mut f = frame(0, 0, true);
        f.samples[0].rotation_xyzw = Some([0.0, 0.0, 0.0, 1.004]);
        let o = observation(&f, &f.samples[0], 1).unwrap();
        assert!(
            matches!(o.data, ObservationData::RigidPose { rotation, .. } if rotation[3] == 1.004)
        );
        let mut f = frame(0, 0, true);
        f.samples[2].position_m = Some([0.0, 20.5, 0.0]);
        assert_eq!(
            validate_frame(&f, &on()),
            Err(XrValidationError::Position(XrRole::RightGrip))
        );
        f.samples[2].position_m = Some([f32::NAN, 0.0, 0.0]);
        assert_eq!(
            validate_frame(&f, &on()),
            Err(XrValidationError::Position(XrRole::RightGrip))
        );
    }

    #[test]
    fn xr_adapter_rejects_misordered_or_duplicate_roles() {
        let mut swapped = frame(0, 0, true);
        swapped.samples.swap(1, 2);
        let mut duplicate = frame(0, 0, true);
        duplicate.samples[2].role = XrRole::LeftGrip;
        for f in [swapped, duplicate] {
            let (r, events) = run(&mut XrLink::default(), &f, 0);
            assert_eq!(r, Err(XrValidationError::RoleOrder));
            assert!(events.is_empty(), "no partial frame");
        }
    }

    #[test]
    fn xr_adapter_rejects_pose_when_untracked() {
        let mut f = frame(0, 0, false);
        assert!(observation(&f, &f.samples[2], 0).is_none());
        f.samples[2].position_m = Some([0.0; 3]);
        assert_eq!(
            validate_frame(&f, &on()),
            Err(XrValidationError::TrackedMismatch(XrRole::RightGrip))
        );
        let mut f = frame(0, 0, true);
        f.samples[0].rotation_xyzw = None;
        assert_eq!(
            validate_frame(&f, &on()),
            Err(XrValidationError::TrackedMismatch(XrRole::Head))
        );
        // An untracked device is reported disconnected and gets no pose; the others continue.
        let (r, events) = run(&mut XrLink::default(), &frame(0, 0, false), 5);
        assert_eq!(r, Ok(2));
        assert!(events.iter().any(|e| matches!(e, SourceEvent::DeviceLink { device_id, connected: false, .. } if device_id.as_str() == "XR-RIGHT-GRIP")));
        assert!(!events.iter().any(|e| matches!(e, SourceEvent::Observation { observation } if observation.device_id.as_str() == "XR-RIGHT-GRIP")));
    }

    #[test]
    fn xr_link_orders_rates_and_relinks() {
        let mut link = XrLink::default();
        assert_eq!(
            run(&mut link, &frame(1, 0, true), 0).0,
            Err(XrValidationError::Sequence),
            "a connection starts at 0"
        );
        assert_eq!(run(&mut link, &frame(0, 100, true), 0).0, Ok(3));
        assert_eq!(
            run(&mut link, &frame(0, 200, true), 20_000).0,
            Err(XrValidationError::Sequence),
            "duplicate"
        );
        assert_eq!(
            run(&mut link, &frame(1, 50, true), 20_000).0,
            Err(XrValidationError::ClientTime)
        );
        // Steady state: no link events repeat.
        let (r, events) = run(&mut link, &frame(1, 200, true), 20_000);
        assert_eq!((r, events.len()), (Ok(3), 3));
        // 120 Hz ceiling = 8333 us. A late frame earns at most half an interval of credit, so
        // one early follower passes and the next is dropped; a flood stays at the ceiling.
        assert_eq!(run(&mut link, &frame(2, 201, true), 20_100).0, Ok(3));
        assert_eq!(
            run(&mut link, &frame(3, 202, true), 20_200).0,
            Err(XrValidationError::OverRate)
        );
        assert_eq!(run(&mut link, &frame(4, 203, true), 28_400).0, Ok(3));
        let mut accepted = 0;
        for i in 0..1000u64 {
            if run(&mut link, &frame(5 + i, 300 + i, true), 40_000 + i * 1_000)
                .0
                .is_ok()
            {
                accepted += 1;
            }
        }
        assert!(
            (119..=122).contains(&accepted),
            "1 s of 1 kHz frames, {accepted} accepted"
        );
        // Tracking loss and reacquisition: one false, then true before the next pose.
        let mut link = XrLink::default();
        assert_eq!(run(&mut link, &frame(0, 0, true), 0).0, Ok(3));
        let (_, lost) = run(&mut link, &frame(1, 1, false), 20_000);
        assert!(matches!(
            &lost[2],
            SourceEvent::DeviceLink {
                connected: false,
                ..
            }
        ));
        let (_, still) = run(&mut link, &frame(2, 2, false), 40_000);
        assert_eq!(still.len(), 2, "loss reported once");
        let (_, back) = run(&mut link, &frame(3, 3, true), 60_000);
        assert!(matches!(
            &back[2],
            SourceEvent::DeviceLink {
                connected: true,
                ..
            }
        ));
        assert!(matches!(&back[3], SourceEvent::Observation { .. }));
        let mut unlinked = vec![];
        link.unlink_all(70_000, |e| {
            unlinked.push(e);
            true
        });
        assert_eq!(unlinked.len(), 3);
        link.unlink_all(80_000, |_| panic!("already unlinked"));
    }

    #[test]
    fn xr_link_keeps_link_before_pose_when_queue_is_full() {
        let mut link = XrLink::default();
        let mut sent = vec![];
        // The queue refuses the first event (head's link): head's pose must not go out unlinked.
        let mut first = true;
        let r = link.frame(&frame(0, 0, true), &on(), 0, |e| {
            if std::mem::take(&mut first) {
                return false;
            }
            sent.push(e);
            true
        });
        assert_eq!(r, Ok(2));
        assert!(!sent.iter().any(|e| matches!(e, SourceEvent::Observation { observation } if observation.device_id.as_str() == "XR-HMD")));
        // The head links on the next frame.
        let (_, next) = run(&mut link, &frame(1, 1, true), 20_000);
        assert!(matches!(
            &next[0],
            SourceEvent::DeviceLink {
                connected: true,
                ..
            }
        ));
    }
}
