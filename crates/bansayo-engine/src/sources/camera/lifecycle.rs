//! Per-camera lifecycle on its capture thread: identify the hardware, choose a capture mode,
//! open, measure, and on loss reconnect with the same mode and calibration. State is published to
//! the shared runtime map about once a second, never per frame.
use super::{capture, identity, modes, timing::FrameStats};
use crate::{config::LiveCameraSpec, registry::reason, sources::RuntimeMap};
use bansayo_contracts::*;
use std::path::Path;

/// Runtime publication period.
const PUBLISH_US: u64 = 1_000_000;

pub fn describe_selector(s: &identity::Selector) -> String {
    match (&s.serial, &s.interface_id, &s.hardware_id, s.index) {
        (Some(serial), _, _, _) => format!("System camera, serial {serial}"),
        (_, Some(endpoint), _, _) => format!(
            "System camera {}",
            identity::identity_from_endpoint("", endpoint).fingerprint
        ),
        (_, _, Some(h), _) => format!("System camera {h}"),
        (_, _, _, Some(i)) => format!("System camera {i}"),
        _ => "System camera".into(),
    }
}

pub struct Lifecycle<'a> {
    id: DeviceId,
    runtime: &'a RuntimeMap,
    data_dir: &'a Path,
    profile: Option<identity::DeviceProfile>,
    stats: FrameStats,
    status: CameraRuntimeStatus,
    ever_active: bool,
    settings_problem: bool,
    rate_problem: Option<Reason>,
    mode_note: Option<Reason>,
    identity_problem: Option<Reason>,
    published_us: u64,
}

impl<'a> Lifecycle<'a> {
    pub fn new(camera: &LiveCameraSpec, runtime: &'a RuntimeMap, data_dir: &'a Path) -> Self {
        let mut identity_problem = None;
        let profile = match identity::load_profile(data_dir, &camera.id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(camera=%camera.id, error=%e, "Camera profile unreadable");
                identity_problem = Some(reason(
                    "camera_profile_invalid",
                    Some(camera.id.to_string()),
                    &format!("{e}. The camera runs; its calibration stays unconfirmed."),
                ));
                None
            }
        };
        let mut me = Self {
            id: camera.id.clone(),
            runtime,
            data_dir,
            profile,
            stats: FrameStats::default(),
            status: CameraRuntimeStatus {
                lifecycle: CameraLifecycle::Discovered,
                identity: None,
                identity_confirmed: identity_problem.is_none(),
                requested: Some(CaptureMode {
                    resolution_px: camera.resolution_px,
                    pixel_format: "MJPEG".into(),
                    fps: Some(camera.rate_hz as f32),
                    verified: false,
                }),
                negotiated: None,
                timing: None,
                replaced_frames: 0,
                inference_hz: None,
                decode_ms: None,
                reconnects: 0,
                reasons: vec![],
            },
            ever_active: false,
            settings_problem: false,
            rate_problem: None,
            mode_note: None,
            identity_problem,
            published_us: 0,
        };
        me.publish();
        me
    }

    fn set(&mut self, lifecycle: CameraLifecycle) {
        self.status.lifecycle = lifecycle;
        self.publish();
    }

    fn publish(&mut self) {
        self.status.reasons = [&self.identity_problem, &self.mode_note, &self.rate_problem]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        if let Ok(mut all) = self.runtime.write() {
            let entry = all.entry(self.id.clone()).or_default();
            // Inference rate is written by the inference thread.
            let inference_hz = entry.camera.as_ref().and_then(|c| c.inference_hz);
            let mut status = self.status.clone();
            status.inference_hz = inference_hz;
            entry.camera = Some(status);
        }
    }

    /// Finds the hardware, checks it against the profile, chooses a mode and opens it.
    pub fn open_device(
        &mut self,
        selector: &identity::Selector,
        camera: &LiveCameraSpec,
        fallback_fps: &[f64],
    ) -> Result<(Box<dyn capture::Capture>, Option<u32>), String> {
        let present = capture::resolve(selector)?;
        let advertised = capture::device_modes(present.index).unwrap_or_default();
        let chosen = self.identified(&present, &advertised, camera, fallback_fps);
        let fps = chosen.fps.map_or(camera.rate_hz, f64::from);
        let capture = capture::open_device(present.index, chosen.resolution_px, fps)?;
        Ok((capture, Some(present.index)))
    }

    /// Checks the found hardware against the profile and chooses the mode to request.
    pub fn identified(
        &mut self,
        present: &identity::Present,
        advertised: &[CaptureMode],
        camera: &LiveCameraSpec,
        fallback_fps: &[f64],
    ) -> CaptureMode {
        let found = present.identity.clone();
        // Re-read on every open: the user may have confirmed a new identity meanwhile.
        if let Ok(profile) = identity::load_profile(self.data_dir, &self.id) {
            self.profile = profile;
            if self
                .identity_problem
                .as_ref()
                .is_some_and(|r| r.code == "camera_profile_invalid")
            {
                self.identity_problem = None;
            }
        }
        let changed = |r: &Reason| r.code == "camera_identity_changed";
        if identity::confirmed(self.profile.as_ref(), &found) {
            if self.identity_problem.as_ref().is_some_and(changed) {
                self.identity_problem = None;
            }
        } else if self.identity_problem.is_none() {
            let was = self
                .profile
                .as_ref()
                .map_or_else(String::new, |p| p.identity.fingerprint.clone());
            tracing::warn!(camera=%self.id, %was, now=%found.fingerprint, "Camera hardware differs from its profile");
            self.identity_problem = Some(reason(
                "camera_identity_changed",
                Some(self.id.to_string()),
                &format!(
                    "This camera's calibration was made with {was}; {} is connected now. Its 3D calibration is not used until you confirm the identity or recalibrate.",
                    found.fingerprint
                ),
            ));
        }
        let confirmed = self.identity_problem.is_none();
        if confirmed && self.profile.is_none() {
            self.save_profile(&found, None);
        }
        self.status.identity = Some(found);
        self.status.identity_confirmed = confirmed;
        self.set(CameraLifecycle::Identified);
        let chosen = modes::select(
            advertised,
            camera.resolution_px,
            camera.rate_hz,
            fallback_fps,
        );
        self.mode_note = chosen
            .note
            .as_ref()
            .map(|n| reason("camera_mode_not_as_requested", Some(self.id.to_string()), n));
        self.status.requested = Some(chosen.mode.clone());
        self.set(CameraLifecycle::Configuring);
        chosen.mode
    }

    fn save_profile(&mut self, found: &CameraIdentity, mode: Option<CaptureMode>) {
        let mut profile = self.profile.clone().unwrap_or(identity::DeviceProfile {
            schema_version: identity::PROFILE_SCHEMA_VERSION,
            logical_id: self.id.clone(),
            identity: found.clone(),
            role: None,
            last_mode: None,
            last_opened_utc: String::new(),
            identity_confirmed_utc: None,
        });
        profile.last_mode = mode.or(profile.last_mode);
        profile.last_opened_utc = crate::clock::utc_now_rfc3339();
        match identity::save_profile(self.data_dir, &profile) {
            Ok(()) => self.profile = Some(profile),
            Err(e) => tracing::warn!(camera=%self.id, error=%e, "Camera profile not saved"),
        }
    }

    pub fn opened(&mut self, negotiated: Option<CaptureMode>) {
        self.stats.restart();
        self.rate_problem = None;
        if self.status.identity_confirmed
            && let Some(found) = self.status.identity.clone()
            && negotiated.as_ref().is_some_and(|m| m.verified)
        {
            self.save_profile(&found, negotiated.clone());
        }
        self.status.negotiated = negotiated;
        self.ever_active = true;
        self.set(CameraLifecycle::Active);
    }

    pub fn settings_problems(&mut self, any: bool) {
        self.settings_problem = any;
    }

    pub fn replaced(&mut self) {
        self.status.replaced_frames += 1;
    }

    /// Records one frame arrival; republishes timing and degradation once a second.
    pub fn frame(&mut self, now_us: u64, decode_us: u64, warn_fps_ratio: f64) {
        self.stats.frame(now_us);
        self.status.decode_ms = Some(decode_us as f32 / 1000.0);
        if now_us.saturating_sub(self.published_us) < PUBLISH_US {
            return;
        }
        self.published_us = now_us;
        self.status.timing = self.stats.summary();
        let requested = self.status.requested.as_ref().and_then(|m| m.fps);
        self.rate_problem = match (&self.status.timing, requested) {
            (Some(t), Some(r))
                if t.window_frames >= 10 && (t.measured_fps as f64) < r as f64 * warn_fps_ratio =>
            {
                Some(reason(
                    "camera_rate_below_requested",
                    Some(self.id.to_string()),
                    &format!(
                        "Requested {r:.0} fps, measured {:.1} fps. Check the mode, the USB link and the lighting (automatic exposure lowers the rate in dim light).",
                        t.measured_fps
                    ),
                ))
            }
            _ => None,
        };
        let lifecycle = if self.rate_problem.is_some() || self.settings_problem {
            CameraLifecycle::Degraded
        } else {
            CameraLifecycle::Active
        };
        self.set(lifecycle);
    }

    pub fn lost(&mut self, failure: &str) {
        let next = if self.ever_active {
            CameraLifecycle::Reconnecting
        } else {
            CameraLifecycle::Disconnected
        };
        if self.status.lifecycle != next || self.ever_active {
            if matches!(
                self.status.lifecycle,
                CameraLifecycle::Active | CameraLifecycle::Degraded
            ) {
                self.status.reconnects += 1;
            }
            self.rate_problem = Some(reason(
                if failure.contains("identical cameras") {
                    "camera_identity_ambiguous"
                } else {
                    "camera_unavailable"
                },
                Some(self.id.to_string()),
                failure,
            ));
            self.set(next);
        }
    }

    /// The user confirms the connected hardware as this camera: the profile is rewritten.
    pub fn confirm(data_dir: &Path, runtime: &RuntimeMap, id: &DeviceId) -> Result<(), String> {
        let found = runtime
            .read()
            .ok()
            .and_then(|r| r.get(id)?.camera.as_ref()?.identity.clone())
            .ok_or("the camera has not been identified yet")?;
        let mut profile = identity::load_profile(data_dir, id)
            .ok()
            .flatten()
            .unwrap_or(identity::DeviceProfile {
                schema_version: identity::PROFILE_SCHEMA_VERSION,
                logical_id: id.clone(),
                identity: found.clone(),
                role: None,
                last_mode: None,
                last_opened_utc: crate::clock::utc_now_rfc3339(),
                identity_confirmed_utc: None,
            });
        profile.identity = found;
        profile.identity_confirmed_utc = Some(crate::clock::utc_now_rfc3339());
        identity::save_profile(data_dir, &profile).map_err(|e| e.to_string())?;
        if let Ok(mut all) = runtime.write()
            && let Some(c) = all.get_mut(id).and_then(|e| e.camera.as_mut())
        {
            c.identity_confirmed = true;
            c.reasons.retain(|r| r.code != "camera_identity_changed");
        }
        Ok(())
    }
}

/// Inference rate per camera, smoothed over about ten frames.
pub fn inferred(runtime: &RuntimeMap, id: &DeviceId, t_us: u64) {
    if let Ok(mut all) = runtime.write() {
        let entry = all.entry(id.clone()).or_default();
        if let Some(last) = entry.last_inference_us
            && t_us > last
            && let Some(c) = entry.camera.as_mut()
        {
            let hz = 1e6 / (t_us - last) as f32;
            c.inference_hz = Some(c.inference_hz.map_or(hz, |h| h + (hz - h) * 0.1));
        }
        entry.last_inference_us = Some(t_us);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::camera::capture::Backend;

    fn spec(rate_hz: f64) -> LiveCameraSpec {
        LiveCameraSpec {
            id: DeviceId::new("CAM1").unwrap(),
            backend: Backend::Device(identity::Selector {
                hardware_id: Some("vid_0c45&pid_6369".into()),
                ..Default::default()
            }),
            rate_hz,
            resolution_px: [1280, 720],
            latency_us: 0,
            exposure_s: None,
            gain: None,
            fiducials: false,
        }
    }
    fn present(index: u32, instance: &str) -> identity::Present {
        identity::Present {
            index,
            identity: identity::identity_from_endpoint(
                "USB Camera",
                &format!(r"\\?\usb#vid_0c45&pid_6369&mi_00#{instance}#{{e5323777}}\global"),
            ),
        }
    }
    fn mode(fps: f32) -> CaptureMode {
        CaptureMode {
            resolution_px: [1280, 720],
            pixel_format: "MJPEG".into(),
            fps: Some(fps),
            verified: true,
        }
    }
    fn status(runtime: &RuntimeMap) -> CameraRuntimeStatus {
        runtime.read().unwrap()[&DeviceId::new("CAM1").unwrap()]
            .camera
            .clone()
            .unwrap()
    }

    /// A fake device through the whole lifecycle: first sighting, a 120 fps request on a 30 fps
    /// camera, a slow camera, unplug and replug on another index, then a different unit.
    #[test]
    fn lifecycle_with_a_fake_device() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = RuntimeMap::default();
        let camera = spec(120.0);
        let mut life = Lifecycle::new(&camera, &runtime, dir.path());
        assert_eq!(status(&runtime).lifecycle, CameraLifecycle::Discovered);
        let a = present(0, "6&aaa&0&0000");
        let chosen = life.identified(&a, &[mode(30.0)], &camera, &[60.0, 30.0]);
        assert_eq!(chosen.fps, Some(30.0), "120 is never invented");
        let s = status(&runtime);
        assert_eq!(s.lifecycle, CameraLifecycle::Configuring);
        assert!(s.identity_confirmed);
        assert!(
            s.reasons
                .iter()
                .any(|r| r.code == "camera_mode_not_as_requested")
        );
        assert!(
            identity::load_profile(dir.path(), &camera.id)
                .unwrap()
                .is_some()
        );
        life.opened(Some(chosen));
        for i in 0..40u64 {
            life.frame(i * 33_333, 4_000, 0.85);
        }
        assert_eq!(status(&runtime).lifecycle, CameraLifecycle::Active);
        // Asked for 30, delivering 15: degraded, with a reason naming both rates.
        let slow = spec(30.0);
        let mut life2 = Lifecycle::new(&slow, &runtime, dir.path());
        let m = life2.identified(&present(1, "6&aaa&0&0000"), &[mode(30.0)], &slow, &[]);
        life2.opened(Some(m));
        for i in 0..40u64 {
            life2.frame(2_000_000 + i * 66_666, 4_000, 0.85);
        }
        let s = status(&runtime);
        assert_eq!(s.lifecycle, CameraLifecycle::Degraded);
        assert!(
            s.reasons
                .iter()
                .any(|r| r.code == "camera_rate_below_requested"
                    && r.message.contains("Requested 30"))
        );
        // Unplugged during training, back on another index: same profile, no recalibration.
        life2.lost("camera disconnected");
        let s = status(&runtime);
        assert_eq!(
            (s.lifecycle, s.reconnects),
            (CameraLifecycle::Reconnecting, 1)
        );
        life2.identified(&present(4, "6&aaa&0&0000"), &[mode(30.0)], &slow, &[]);
        life2.opened(Some(mode(30.0)));
        let s = status(&runtime);
        assert_eq!(s.lifecycle, CameraLifecycle::Active);
        assert!(s.identity_confirmed);
        // A different unit of the same model under the same logical id is not accepted silently.
        let mut life3 = Lifecycle::new(&slow, &runtime, dir.path());
        let b = present(0, "6&bbb&0&0000");
        life3.identified(&b, &[mode(30.0)], &slow, &[]);
        let s = status(&runtime);
        assert!(!s.identity_confirmed);
        assert!(
            s.reasons
                .iter()
                .any(|r| r.code == "camera_identity_changed")
        );
        Lifecycle::confirm(dir.path(), &runtime, &slow.id).unwrap();
        assert!(status(&runtime).identity_confirmed);
        life3.identified(&b, &[mode(30.0)], &slow, &[]);
        assert!(
            status(&runtime).identity_confirmed,
            "the confirmed unit is accepted on reopen"
        );
    }
}
