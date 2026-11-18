//! Physical cameras: capture, 2D pose inference and `landmarks_2d` observations.
//!
//! One capture thread per camera keeps only its newest frame (a bounded latest-frame buffer), so
//! a slow consumer drops frames instead of adding latency. One inference thread serves every
//! camera round-robin. A camera that fails or is unplugged reports a link loss and is reopened
//! every two seconds; the others keep running. Observations have the same shape as the
//! simulator's, so triangulation and the skeleton fit cannot tell them apart.
pub mod capture;
pub mod controls;
pub mod fiducial;
pub mod pose;

use crate::{
    config::{CameraSourceSpec, LiveCameraSpec},
    pipeline::skeleton_fit::{COCO17_SCHEMA_ID, coco17_schema},
    sources::{Ingest, Source, SourceContext, SourceError},
};
use bansayo_contracts::*;
use capture::Backend;
use image::RgbImage;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Newest JPEG preview per camera, for the API.
pub type Previews = Arc<RwLock<BTreeMap<DeviceId, bytes::Bytes>>>;
pub type FiducialPreviews =
    Arc<RwLock<BTreeMap<DeviceId, (std::time::Instant, fiducial::Snapshot)>>>;
const PREVIEW_MAX_WIDTH: u32 = 640;
const PREVIEW_PERIOD_US: u64 = 100_000;
const REOPEN_AFTER: Duration = Duration::from_secs(2);

/// Manual capture settings requested per camera; `PUT /cameras/{id}/settings` changes them and
/// bumps `generation`, and the capture thread applies them on its next frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RequestedSettings {
    pub generation: u64,
    pub exposure_s: Option<f64>,
    pub gain: Option<f64>,
}
pub type CameraSettings = Arc<RwLock<BTreeMap<DeviceId, RequestedSettings>>>;

/// Applies requested settings and says what is in effect (08 §3.2, ADR-020). A setting the device
/// or backend cannot apply leaves the camera running and adds a reason naming the property.
pub fn apply_settings(backend: &Backend, r: &RequestedSettings) -> CameraSettingsStatus {
    let problem = |property: &str, message: String| Reason {
        code: "camera_setting_not_applied".into(),
        subject: Some(property.into()),
        message,
    };
    match backend {
        // Set on the phone; validation refuses exposure_s and gain for network cameras.
        Backend::Network(_) => CameraSettingsStatus::default(),
        Backend::Frames(_) => CameraSettingsStatus {
            exposure_s: None,
            gain: None,
            problems: [("exposure_s", r.exposure_s), ("gain", r.gain)]
                .into_iter()
                .filter(|(_, v)| v.is_some())
                .map(|(p, _)| problem(p, format!("{p}: an image folder has no capture settings.")))
                .collect(),
        },
        Backend::Device(index) => {
            let mut problems = vec![];
            if r.exposure_s.is_none() {
                problems.push(Reason {
                    code: "exposure_automatic".into(),
                    subject: Some("exposure_s".into()),
                    message:
                        "exposure_s is not set: automatic exposure blurs fast limbs (08 §3.2)."
                            .into(),
                });
            }
            let requested = r.exposure_s.is_some() || r.gain.is_some();
            let result = if requested {
                controls::apply(*index, r.exposure_s, r.gain)
            } else {
                Ok(Default::default())
            };
            let applied = match result {
                Ok(a) => a,
                Err(e) => {
                    let property = if e.starts_with("gain") {
                        "gain"
                    } else {
                        "exposure_s"
                    };
                    problems.push(problem(property, e));
                    // Exposure may have been set before gain failed; read what the device says.
                    controls::apply(*index, r.exposure_s, None).unwrap_or_default()
                }
            };
            CameraSettingsStatus {
                exposure_s: applied.exposure_s.map(|v| v as f32),
                gain: applied.gain.map(|v| v as f32),
                problems,
            }
        }
    }
}

#[derive(Clone)]
pub struct CameraSource {
    pub spec: CameraSourceSpec,
    pub previews: Previews,
    pub fiducials: FiducialPreviews,
    pub settings: CameraSettings,
    pub timing: crate::diagnostics::Timings,
    pub feeds: capture::NetworkFeeds,
}

impl CameraSource {
    pub fn descriptors(&self) -> Vec<DeviceDescriptor> {
        self.spec
            .cameras
            .iter()
            .map(|c| {
                let device = matches!(c.backend, Backend::Device(_) | Backend::Network(_));
                DeviceDescriptor {
                    id: c.id.clone(),
                    source_id: self.spec.id.clone(),
                    kind: DeviceKind::Camera,
                    origin: if device {
                        Origin::Live
                    } else {
                        Origin::Fixture
                    },
                    model: Some(match &c.backend {
                        Backend::Device(i) => format!("System camera {i}"),
                        Backend::Frames(dir) => format!("Image folder {}", dir.display()),
                        Backend::Network(_) => "Network camera (phone)".into(),
                    }),
                    firmware: None,
                    transport: match c.backend {
                        Backend::Device(_) => Transport::Usb,
                        Backend::Frames(_) => Transport::File,
                        Backend::Network(_) => Transport::Websocket,
                    },
                    capabilities: vec![SensorCapability::Landmarks2d],
                    nominal_rate_hz: Some(c.rate_hz as f32),
                    camera: Some(CameraInfo {
                        resolution_px: c.resolution_px,
                        landmark_schema: Some(coco17_schema()),
                    }),
                }
            })
            .collect()
    }
}

#[derive(Clone)]
struct Frame {
    image: Arc<RgbImage>,
    t_us: u64,
    settings: Option<CameraSettingsStatus>,
    recorded: Option<Vec<Option<Landmark2D>>>,
}

/// Newest frame per camera plus a wake-up for the inference thread.
struct Slots {
    frames: Vec<Mutex<Option<Frame>>>,
    marker_frames: Vec<Mutex<Option<Frame>>>,
    marker_fresh: Mutex<bool>,
    marker_wake: Condvar,
    fresh: Mutex<bool>,
    wake: Condvar,
}

/// Stops the threads when the source task ends, including when it is aborted.
struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl Source for CameraSource {
    type Context = SourceContext;
    async fn run(self, context: SourceContext) -> Result<(), SourceError> {
        let stop = Arc::new(AtomicBool::new(false));
        let _guard = StopOnDrop(stop.clone());
        let slots = Arc::new(Slots {
            frames: self.spec.cameras.iter().map(|_| Mutex::new(None)).collect(),
            marker_frames: self.spec.cameras.iter().map(|_| Mutex::new(None)).collect(),
            marker_fresh: Mutex::new(false),
            marker_wake: Condvar::new(),
            fresh: Mutex::new(false),
            wake: Condvar::new(),
        });
        for (index, camera) in self.spec.cameras.iter().enumerate() {
            if let Ok(mut s) = self.settings.write() {
                s.entry(camera.id.clone()).or_insert(RequestedSettings {
                    generation: 0,
                    exposure_s: camera.exposure_s,
                    gain: camera.gain,
                });
            }
            let (camera, slots, stop, context, settings, timing, feeds) = (
                camera.clone(),
                slots.clone(),
                stop.clone(),
                context.clone(),
                self.settings.clone(),
                self.timing.clone(),
                self.feeds.clone(),
            );
            std::thread::Builder::new()
                .name(format!("capture-{}", camera.id))
                .spawn(move || {
                    let shared = Shared {
                        settings: &settings,
                        timing: &timing,
                        feeds: &feeds,
                    };
                    capture_loop(index, &camera, &slots, &stop, &context, &shared)
                })
                .map_err(|e| SourceError(e.to_string()))?;
        }
        let (fatal_tx, fatal_rx) = tokio::sync::oneshot::channel();
        {
            let (source, slots, stop, context) =
                (self.clone(), slots.clone(), stop.clone(), context.clone());
            std::thread::Builder::new()
                .name(format!("pose-{}", self.spec.id))
                .spawn(move || {
                    // ONNX Runtime loading panics when its library is missing; report it instead.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        inference_loop(&source, &slots, &stop, &context)
                    }))
                    .unwrap_or_else(|panic| {
                        Err(panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                            .map(|m| format!("{m} (see models/README.md)"))
                            .unwrap_or_else(|| "pose inference panicked".into()))
                    });
                    if let Err(e) = result {
                        let _ = fatal_tx.send(e);
                    }
                })
                .map_err(|e| SourceError(e.to_string()))?;
        }
        if self.spec.fiducials.enabled && self.spec.cameras.iter().any(|c| c.fiducials) {
            let (source, slots, stop, context) =
                (self.clone(), slots.clone(), stop.clone(), context.clone());
            std::thread::Builder::new()
                .name(format!("apriltag-{}", self.spec.id))
                .spawn(move || {
                    if let Err(message) = marker_loop(&source, &slots, &stop, &context) {
                        tracing::error!(source=%source.spec.id, %message, "AprilTag unavailable");
                        let _ = context.events.try_send(Ingest::Notice {
                            code: "apriltag_unavailable".into(),
                            message,
                        });
                    }
                })
                .map_err(|e| SourceError(e.to_string()))?;
        }
        tokio::select! {
            _ = context.cancel.cancelled() => Ok(()),
            Ok(message) = fatal_rx => {
                tracing::error!(source=%self.spec.id, %message, "Pose inference failed");
                Err(SourceError(message))
            }
        }
    }
}

fn link(context: &SourceContext, id: &DeviceId, connected: bool) {
    let _ = context
        .events
        .try_send(Ingest::Event(SourceEvent::DeviceLink {
            device_id: id.clone(),
            connected,
            t_us: context.clock.now_us(),
        }));
}

/// What a capture thread shares with the rest of the engine.
struct Shared<'a> {
    settings: &'a CameraSettings,
    timing: &'a crate::diagnostics::Timing,
    feeds: &'a capture::NetworkFeeds,
}

fn capture_loop(
    index: usize,
    camera: &LiveCameraSpec,
    slots: &Slots,
    stop: &AtomicBool,
    context: &SourceContext,
    shared: &Shared,
) {
    let Shared {
        settings,
        timing,
        feeds,
    } = *shared;
    let mut last_failure = String::new();
    let mut connected = true;
    let mut time = crate::clock::SourceTime::default();
    let requested = || {
        settings
            .read()
            .ok()
            .and_then(|s| s.get(&camera.id).copied())
            .unwrap_or_default()
    };
    while !stop.load(Ordering::Relaxed) {
        let opened =
            capture::open_with(&camera.backend, camera.resolution_px, camera.rate_hz, feeds);
        let failure = match opened {
            Ok(mut capture) => {
                tracing::info!(camera=%camera.id, format=%capture.describe(), "Camera opened");
                last_failure.clear();
                // Applied after the stream is open: the device then holds them while streaming.
                let mut applied: Option<(u64, CameraSettingsStatus)> = None;
                loop {
                    let r = requested();
                    if applied.as_ref().is_none_or(|(g, _)| *g != r.generation) {
                        let status = apply_settings(&camera.backend, &r);
                        let problems: Vec<&str> =
                            status.problems.iter().map(|p| p.message.as_str()).collect();
                        tracing::info!(camera=%camera.id, exposure_s=?status.exposure_s, gain=?status.gain, ?problems, "Camera settings");
                        applied = Some((r.generation, status));
                    }
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    match capture.next() {
                        // Landmarks are validated against the declared size, as intrinsics are.
                        Ok(image)
                            if image.dimensions()
                                != (camera.resolution_px[0], camera.resolution_px[1]) =>
                        {
                            break format!(
                                "delivers {}x{} frames; set resolution_px: [{}, {}] to match",
                                image.width(),
                                image.height(),
                                image.width(),
                                image.height()
                            );
                        }
                        Ok(image) => {
                            // Arrival is the closest instant to acquisition this API exposes.
                            let now = context.clock.now_us();
                            timing.record(
                                crate::diagnostics::Stage::Decode,
                                now,
                                capture.decode_us(),
                            );
                            let (t_us, _) = time.map(None, now, camera.latency_us);
                            if !connected {
                                connected = true;
                                link(context, &camera.id, true);
                            }
                            let frame = Frame {
                                image: Arc::new(image),
                                t_us,
                                settings: applied.as_ref().map(|(_, s)| s.clone()),
                                recorded: capture.recorded_landmarks(),
                            };
                            if camera.fiducials {
                                if let Ok(mut target) = slots.marker_frames[index].lock() {
                                    *target = Some(frame.clone());
                                }
                                if let Ok(mut fresh) = slots.marker_fresh.lock() {
                                    *fresh = true;
                                    slots.marker_wake.notify_one();
                                }
                            }
                            if let Ok(mut slot) = slots.frames[index].lock() {
                                if slot.is_some()
                                    && let Some(counter) = context.dropped.get(&camera.id)
                                {
                                    counter.fetch_add(1, Ordering::Relaxed);
                                }
                                *slot = Some(frame);
                            }
                            if let Ok(mut fresh) = slots.fresh.lock() {
                                *fresh = true;
                                slots.wake.notify_one();
                            }
                        }
                        Err(e) => break e,
                    }
                }
            }
            Err(e) => e,
        };
        // Said once per distinct failure: a phone that is not connected yet is not news every 2 s.
        if failure != last_failure {
            tracing::warn!(camera=%camera.id, error=%failure, "Camera unavailable; retrying");
            last_failure = failure;
        }
        if connected {
            connected = false;
            link(context, &camera.id, false);
        }
        std::thread::sleep(REOPEN_AFTER);
    }
}

fn inference_loop(
    source: &CameraSource,
    slots: &Slots,
    stop: &AtomicBool,
    context: &SourceContext,
) -> Result<(), String> {
    let spec = &source.spec;
    // Loaded on the first frame without recorded landmarks.
    let mut detector: Option<pose::Detector> = None;
    let n = spec.cameras.len();
    let mut tracks: Vec<pose::Track> = (0..n).map(|_| pose::Track::default()).collect();
    let mut seqs = vec![0u64; n];
    let mut last_preview = vec![0u64; n];
    let mut next = 0;
    while !stop.load(Ordering::Relaxed) {
        // Round-robin: the first camera after the last one served that has a new frame.
        let taken = (0..n).find_map(|k| {
            let i = (next + k) % n;
            let frame = slots.frames[i].lock().ok()?.take()?;
            Some((i, frame))
        });
        let Some((i, frame)) = taken else {
            let mut fresh = slots.fresh.lock().map_err(|e| e.to_string())?;
            if !*fresh {
                fresh = slots
                    .wake
                    .wait_timeout(fresh, Duration::from_millis(50))
                    .map_err(|e| e.to_string())?
                    .0;
            }
            *fresh = false;
            continue;
        };
        next = i + 1;
        let camera = &spec.cameras[i];
        let started = source.timing.on().then(std::time::Instant::now);
        let detection = match frame.recorded.clone() {
            Some(points) => Some(pose::Detection {
                bbox: [
                    0.0,
                    0.0,
                    frame.image.width() as f32,
                    frame.image.height() as f32,
                ],
                score: 1.0,
                points,
            }),
            None => {
                if detector.is_none() {
                    detector = Some(pose::Detector::load(
                        &spec.models_dir,
                        spec.execution,
                        spec.min_person_score,
                        spec.min_keypoint_score,
                    )?);
                    tracing::info!(source=%spec.id, "Pose models loaded");
                }
                let was = tracks[i].ambiguous;
                let found = match detector.as_mut() {
                    Some(d) => d.track(&frame.image, &mut tracks[i])?,
                    None => None,
                };
                if tracks[i].ambiguous && !was {
                    let _ = context.events.try_send(Ingest::Notice {
                        code: "multiple_people".into(),
                        message: format!(
                            "{}: two people are equally likely to be the athlete; no pose until one leaves the view.",
                            spec.cameras[i].id
                        ),
                    });
                }
                found
            }
        };
        if let Some(s) = started {
            let now = context.clock.now_us();
            let d = s.elapsed().as_micros() as u64;
            source
                .timing
                .record(crate::diagnostics::Stage::Inference, now, d);
            source.timing.ready(&camera.id, now);
        }
        seqs[i] += 1;
        let observation = landmarks_observation(
            camera,
            &spec.id,
            seqs[i],
            frame.t_us,
            [frame.image.width(), frame.image.height()],
            detection.as_ref(),
            frame.settings.clone(),
        );
        if context
            .events
            .try_send(Ingest::Event(SourceEvent::Observation { observation }))
            .is_err()
            && let Some(c) = context.dropped.get(&camera.id)
        {
            c.fetch_add(1, Ordering::Relaxed);
        }
        if frame.t_us.saturating_sub(last_preview[i]) >= PREVIEW_PERIOD_US {
            last_preview[i] = frame.t_us;
            if let Some(jpeg) = preview_jpeg(&frame.image)
                && let Ok(mut previews) = source.previews.write()
            {
                previews.insert(camera.id.clone(), jpeg);
            }
        }
    }
    Ok(())
}

/// The `landmarks_2d` observation of one frame: the detector's points exactly as returned, with no
/// smoothing, so recordings hold raw inputs and re-analysis sees what live analysis saw (O-20).
pub fn landmarks_observation(
    camera: &LiveCameraSpec,
    source_id: &SourceId,
    seq: u64,
    t_us: u64,
    image_size_px: [u32; 2],
    detection: Option<&pose::Detection>,
    settings: Option<CameraSettingsStatus>,
) -> SensorObservation {
    let points = detection.map_or_else(|| vec![None; 17], |d| d.points.clone());
    let seen: Vec<f32> = points.iter().flatten().map(|p| p.confidence).collect();
    let origin = if matches!(camera.backend, Backend::Device(_) | Backend::Network(_)) {
        Origin::Live
    } else {
        Origin::Fixture
    };
    SensorObservation {
        device_id: camera.id.clone(),
        source_id: source_id.clone(),
        origin,
        seq,
        t_us,
        device_time_us: None,
        time_quality: TimeQuality::ReceiveTime,
        calibration_ref: None,
        // Nobody in view says nothing about the camera's health.
        confidence: (!seen.is_empty()).then(|| seen.iter().sum::<f32>() / seen.len() as f32),
        data: ObservationData::Landmarks2d {
            landmarks: PoseLandmarks2D {
                frame_meta: CameraFrameMetadata {
                    device_id: camera.id.clone(),
                    frame_seq: seq,
                    t_us,
                    device_time_us: None,
                    time_quality: TimeQuality::ReceiveTime,
                    image_size_px,
                    settings,
                },
                frame: FrameRef::Image {
                    id: camera.id.clone(),
                },
                origin,
                schema_id: COCO17_SCHEMA_ID.into(),
                detector: pose::DETECTOR_NAME.into(),
                points,
            },
        },
    }
}

fn marker_loop(
    source: &CameraSource,
    slots: &Slots,
    stop: &AtomicBool,
    context: &SourceContext,
) -> Result<(), String> {
    let mut native = fiducial::Native::open(&source.spec.fiducials)?;
    let n = source.spec.cameras.len();
    let mut next = 0;
    let mut last = vec![None::<u64>; n];
    let period = (1_000_000.0 / source.spec.fiducials.max_detection_hz) as u64;
    tracing::info!(source=%source.spec.id, "AprilTag detector initialized");
    while !stop.load(Ordering::Relaxed) {
        let taken = (0..n).find_map(|k| {
            let i = (next + k) % n;
            if !source.spec.cameras[i].fiducials {
                return None;
            }
            slots.marker_frames[i]
                .lock()
                .ok()?
                .take()
                .map(|frame| (i, frame))
        });
        let Some((i, frame)) = taken else {
            let mut ready = slots.marker_fresh.lock().map_err(|e| e.to_string())?;
            if !*ready {
                ready = slots
                    .marker_wake
                    .wait_timeout(ready, Duration::from_millis(50))
                    .map_err(|e| e.to_string())?
                    .0;
            }
            *ready = false;
            continue;
        };
        next = (i + 1) % n;
        if last[i].is_some_and(|at| frame.t_us.saturating_sub(at) < period) {
            continue;
        }
        last[i] = Some(frame.t_us);
        let started = source.timing.on().then(std::time::Instant::now);
        let tags = native.detect(&frame.image)?;
        if let Some(started) = started {
            source.timing.record(
                crate::diagnostics::Stage::Fiducial,
                context.clock.now_us(),
                started.elapsed().as_micros() as u64,
            );
        }
        let camera = &source.spec.cameras[i];
        let snapshot = fiducial::Snapshot {
            camera_id: camera.id.to_string(),
            t_us: frame.t_us,
            image_size_px: [frame.image.width(), frame.image.height()],
            tags,
        };
        if let Ok(mut latest) = source.fiducials.write() {
            latest.insert(camera.id.clone(), (std::time::Instant::now(), snapshot));
        }
    }
    Ok(())
}

fn preview_jpeg(image: &RgbImage) -> Option<bytes::Bytes> {
    let scaled;
    let image = if image.width() > PREVIEW_MAX_WIDTH {
        let h = image.height() * PREVIEW_MAX_WIDTH / image.width();
        scaled = image::imageops::thumbnail(image, PREVIEW_MAX_WIDTH, h);
        &scaled
    } else {
        image
    };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70)
        .encode_image(image)
        .ok()?;
    Some(out.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O-20: what is recorded and solved is the detector's output, bit for bit, frame after frame.
    #[test]
    fn recorded_landmarks_equal_raw_detector_output() {
        let camera = LiveCameraSpec {
            id: DeviceId::new("CAM1").unwrap(),
            backend: Backend::Device(0),
            rate_hz: 30.0,
            resolution_px: [640, 480],
            latency_us: 0,
            exposure_s: None,
            gain: None,
            fiducials: false,
        };
        let source = SourceId::new("cams").unwrap();
        for (k, jitter) in [0.0f32, 3.5, -2.25, 7.0, 0.125].into_iter().enumerate() {
            let points: Vec<Option<Landmark2D>> = (0..17)
                .map(|i| {
                    (i % 5 != 3).then_some(Landmark2D {
                        x_px: 100.0 + i as f32 * 11.0 + jitter,
                        y_px: 50.0 + i as f32 * 7.0 - jitter,
                        confidence: 0.3 + i as f32 * 0.03,
                    })
                })
                .collect();
            let detection = pose::Detection {
                bbox: [0.0, 0.0, 640.0, 480.0],
                score: 0.9,
                points: points.clone(),
            };
            let o = landmarks_observation(
                &camera,
                &source,
                k as u64 + 1,
                k as u64 * 33_333,
                [640, 480],
                Some(&detection),
                None,
            );
            let ObservationData::Landmarks2d { landmarks } = o.data else {
                panic!("landmarks");
            };
            let bits = |p: &[Option<Landmark2D>]| -> Vec<Option<[u32; 3]>> {
                p.iter()
                    .map(|p| {
                        p.as_ref()
                            .map(|p| [p.x_px.to_bits(), p.y_px.to_bits(), p.confidence.to_bits()])
                    })
                    .collect()
            };
            assert_eq!(bits(&landmarks.points), bits(&points), "frame {k}");
        }
    }
}
