//! Physical cameras: capture, 2D pose inference and `landmarks_2d` observations.
//!
//! One capture thread per camera keeps only its newest frame (a bounded latest-frame buffer), so
//! a slow consumer drops frames instead of adding latency. One inference thread serves every
//! camera round-robin. A camera that fails or is unplugged reports a link loss and is reopened
//! every two seconds; the others keep running. Observations have the same shape as the
//! simulator's, so triangulation and the skeleton fit cannot tell them apart.
pub mod capture;
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
const PREVIEW_MAX_WIDTH: u32 = 640;
const PREVIEW_PERIOD_US: u64 = 100_000;
const REOPEN_AFTER: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct CameraSource {
    pub spec: CameraSourceSpec,
    pub previews: Previews,
}

impl CameraSource {
    pub fn descriptors(&self) -> Vec<DeviceDescriptor> {
        self.spec
            .cameras
            .iter()
            .map(|c| {
                let device = matches!(c.backend, Backend::Device(_));
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
                    }),
                    firmware: None,
                    transport: if device {
                        Transport::Usb
                    } else {
                        Transport::File
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

struct Frame {
    image: RgbImage,
    t_us: u64,
}

/// Newest frame per camera plus a wake-up for the inference thread.
struct Slots {
    frames: Vec<Mutex<Option<Frame>>>,
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
            fresh: Mutex::new(false),
            wake: Condvar::new(),
        });
        for (index, camera) in self.spec.cameras.iter().enumerate() {
            let (camera, slots, stop, context) =
                (camera.clone(), slots.clone(), stop.clone(), context.clone());
            std::thread::Builder::new()
                .name(format!("capture-{}", camera.id))
                .spawn(move || capture_loop(index, &camera, &slots, &stop, &context))
                .map_err(|e| SourceError(e.to_string()))?;
        }
        let (fatal_tx, fatal_rx) = tokio::sync::oneshot::channel();
        {
            let (source, slots, stop, context) =
                (self.clone(), slots, stop.clone(), context.clone());
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

fn capture_loop(
    index: usize,
    camera: &LiveCameraSpec,
    slots: &Slots,
    stop: &AtomicBool,
    context: &SourceContext,
) {
    let mut connected = true;
    let mut time = crate::clock::SourceTime::default();
    while !stop.load(Ordering::Relaxed) {
        let failure = match capture::open(&camera.backend, camera.resolution_px, camera.rate_hz) {
            Ok(mut capture) => {
                tracing::info!(camera=%camera.id, format=%capture.describe(), "Camera opened");
                loop {
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
                            let (t_us, _) =
                                time.map(None, context.clock.now_us(), camera.latency_us);
                            if !connected {
                                connected = true;
                                link(context, &camera.id, true);
                            }
                            if let Ok(mut slot) = slots.frames[index].lock() {
                                *slot = Some(Frame { image, t_us });
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
        tracing::warn!(camera=%camera.id, error=%failure, "Camera unavailable; retrying");
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
    let mut detector = pose::Detector::load(
        &spec.models_dir,
        spec.execution,
        spec.min_person_score,
        spec.min_keypoint_score,
    )?;
    tracing::info!(source=%spec.id, "Pose models loaded");
    let n = spec.cameras.len();
    let mut tracks: Vec<pose::Track> = (0..n).map(|_| pose::Track::default()).collect();
    let mut filters: Vec<Vec<Option<[pose::OneEuro; 2]>>> = vec![vec![None; 17]; n];
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
        let detection = detector.track(&frame.image, &mut tracks[i])?;
        let mut points = detection
            .as_ref()
            .map_or_else(|| vec![None; 17], |d| d.points.clone());
        for (point, filter) in points.iter_mut().zip(filters[i].iter_mut()) {
            match point {
                Some(p) => {
                    let f = filter.get_or_insert_with(|| {
                        [
                            pose::OneEuro::new(p.x_px, frame.t_us),
                            pose::OneEuro::new(p.y_px, frame.t_us),
                        ]
                    });
                    p.x_px = f[0].filter(p.x_px, frame.t_us);
                    p.y_px = f[1].filter(p.y_px, frame.t_us);
                }
                // A lost point restarts its filter so it does not glide in from a stale place.
                None => *filter = None,
            }
        }
        seqs[i] += 1;
        let seen: Vec<f32> = points.iter().flatten().map(|p| p.confidence).collect();
        let origin = if matches!(camera.backend, Backend::Device(_)) {
            Origin::Live
        } else {
            Origin::Fixture
        };
        let observation = SensorObservation {
            device_id: camera.id.clone(),
            source_id: spec.id.clone(),
            origin,
            seq: seqs[i],
            t_us: frame.t_us,
            device_time_us: None,
            time_quality: TimeQuality::ReceiveTime,
            calibration_ref: None,
            // Nobody in view says nothing about the camera's health.
            confidence: (!seen.is_empty()).then(|| seen.iter().sum::<f32>() / seen.len() as f32),
            data: ObservationData::Landmarks2d {
                landmarks: PoseLandmarks2D {
                    frame_meta: CameraFrameMetadata {
                        device_id: camera.id.clone(),
                        frame_seq: seqs[i],
                        t_us: frame.t_us,
                        device_time_us: None,
                        time_quality: TimeQuality::ReceiveTime,
                        image_size_px: [frame.image.width(), frame.image.height()],
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
        };
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
