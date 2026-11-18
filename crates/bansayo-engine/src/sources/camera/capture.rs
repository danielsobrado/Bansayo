//! Blocking frame producers: a physical camera (Windows Media Foundation) or a folder of images.
use image::RgbImage;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Where a camera's frames come from.
#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    /// System camera index, as listed by `bansayo-engine --list-cameras`.
    Device(u32),
    /// Images (`.jpg`, `.jpeg`, `.png`) in name order, played at the camera rate and looped.
    Frames(PathBuf),
}

/// A blocking frame producer. `next` returns as soon as a frame is available. Opened, used and
/// dropped on its camera's capture thread.
pub trait Capture {
    fn next(&mut self) -> Result<RgbImage, String>;
    /// Negotiated format, for the log.
    fn describe(&self) -> String;
}

pub fn open(
    backend: &Backend,
    resolution_px: [u32; 2],
    rate_hz: f64,
) -> Result<Box<dyn Capture>, String> {
    match backend {
        Backend::Frames(dir) => Ok(Box::new(Frames::open(dir, rate_hz)?)),
        Backend::Device(index) => device::open(*index, resolution_px, rate_hz),
    }
}

/// Image files of a frames directory, in name order.
pub fn frame_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["jpg", "jpeg", "png"].contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("{} contains no .jpg or .png images", dir.display()));
    }
    Ok(files)
}

struct Frames {
    files: Vec<PathBuf>,
    index: usize,
    period: Duration,
    due: Instant,
}
impl Frames {
    fn open(dir: &Path, rate_hz: f64) -> Result<Self, String> {
        Ok(Self {
            files: frame_files(dir)?,
            index: 0,
            period: Duration::from_secs_f64(1.0 / rate_hz),
            due: Instant::now(),
        })
    }
}
impl Capture for Frames {
    fn next(&mut self) -> Result<RgbImage, String> {
        let now = Instant::now();
        if self.due > now {
            std::thread::sleep(self.due - now);
        }
        // Scheduled from the previous deadline so sleep jitter does not lower the rate.
        self.due = (self.due + self.period).max(now);
        let path = &self.files[self.index % self.files.len()];
        self.index += 1;
        Ok(image::open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .to_rgb8())
    }
    fn describe(&self) -> String {
        format!(
            "{} images at {:.1} Hz",
            self.files.len(),
            1.0 / self.period.as_secs_f64()
        )
    }
}

/// System cameras `(index, name)`.
pub fn list_devices() -> Result<Vec<(u32, String)>, String> {
    device::list()
}

/// Formats a system camera offers, one line per pixel format and resolution.
pub fn device_formats(index: u32) -> Result<Vec<String>, String> {
    device::formats(index)
}

#[cfg(windows)]
mod device {
    use super::Capture;
    use image::RgbImage;
    use nokhwa::{
        Camera,
        pixel_format::RgbFormat,
        utils::{
            ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat,
            RequestedFormatType, Resolution,
        },
    };

    pub fn list() -> Result<Vec<(u32, String)>, String> {
        Ok(nokhwa::query(ApiBackend::MediaFoundation)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|c| Some((c.index().as_index().ok()?, c.human_name())))
            .collect())
    }

    /// Formats a device offers, `"MJPEG 1280x720 @ 30, 15 fps"` per format and resolution.
    pub fn formats(index: u32) -> Result<Vec<String>, String> {
        let any = RequestedFormat::new::<RgbFormat>(RequestedFormatType::None);
        let mut camera = Camera::new(CameraIndex::Index(index), any).map_err(|e| e.to_string())?;
        let mut rates: std::collections::BTreeMap<(String, u32, u32), Vec<u32>> =
            Default::default();
        for f in camera
            .compatible_camera_formats()
            .map_err(|e| e.to_string())?
        {
            let r = f.resolution();
            rates
                .entry((f.format().to_string(), r.width(), r.height()))
                .or_default()
                .push(f.frame_rate());
        }
        Ok(rates
            .into_iter()
            .map(|((format, w, h), mut fps)| {
                fps.sort_unstable_by(|a, b| b.cmp(a));
                fps.dedup();
                let fps: Vec<String> = fps.iter().map(u32::to_string).collect();
                format!("{format} {w}x{h} @ {} fps", fps.join(", "))
            })
            .collect())
    }

    struct Device(Camera);

    pub fn open(index: u32, [w, h]: [u32; 2], rate_hz: f64) -> Result<Box<dyn Capture>, String> {
        // MJPEG keeps 720p/1080p at full rate over USB 2; decoded here, not by nokhwa.
        let wanted = CameraFormat::new(Resolution::new(w, h), FrameFormat::MJPEG, rate_hz as u32);
        // `Closest` alone picked 1 fps on a UGREEN camera offering 1280x720 at 30 (18 Nov 2026).
        let mut camera = Camera::new(
            CameraIndex::Index(index),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(wanted)),
        )
        .or_else(|_| {
            Camera::new(
                CameraIndex::Index(index),
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(wanted)),
            )
        })
        .map_err(|e| e.to_string())?;
        camera.open_stream().map_err(|e| e.to_string())?;
        Ok(Box::new(Device(camera)))
    }

    impl Capture for Device {
        fn next(&mut self) -> Result<RgbImage, String> {
            let buffer = self.0.frame().map_err(|e| e.to_string())?;
            if buffer.source_frame_format() == FrameFormat::MJPEG {
                return image::load_from_memory_with_format(
                    buffer.buffer(),
                    image::ImageFormat::Jpeg,
                )
                .map(|i| i.to_rgb8())
                .map_err(|e| format!("MJPEG decode: {e}"));
            }
            buffer
                .decode_image::<RgbFormat>()
                .map_err(|e| e.to_string())
        }
        fn describe(&self) -> String {
            // nokhwa 0.10 reports the rate's denominator ("@1FPS") for Media Foundation formats.
            let f = self.0.camera_format();
            format!(
                "{} {} ({})",
                f.format(),
                f.resolution(),
                self.0.info().human_name()
            )
        }
    }
    impl Drop for Device {
        fn drop(&mut self) {
            let _ = self.0.stop_stream();
        }
    }
}

#[cfg(not(windows))]
mod device {
    use super::Capture;
    const UNSUPPORTED: &str =
        "camera devices are supported on Windows only in this build; use frames_dir";
    pub fn list() -> Result<Vec<(u32, String)>, String> {
        Err(UNSUPPORTED.into())
    }
    pub fn open(_: u32, _: [u32; 2], _: f64) -> Result<Box<dyn Capture>, String> {
        Err(UNSUPPORTED.into())
    }
    pub fn formats(_: u32) -> Result<Vec<String>, String> {
        Err(UNSUPPORTED.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Capture rate of a physical camera without inference. Needs a camera:
    /// `BANSAYO_CAMERA=0 cargo test -p bansayo-engine -- --ignored --nocapture capture_rate`.
    #[test]
    #[ignore]
    fn capture_rate_of_a_physical_camera() {
        let index = std::env::var("BANSAYO_CAMERA")
            .expect("BANSAYO_CAMERA")
            .parse()
            .unwrap();
        let mut c = open(&Backend::Device(index), [1280, 720], 30.0).unwrap();
        eprintln!("format {}", c.describe());
        c.next().unwrap();
        let t = Instant::now();
        for _ in 0..60 {
            c.next().unwrap();
        }
        eprintln!("{:.1} frames/s", 60.0 / t.elapsed().as_secs_f64());
    }

    #[test]
    fn frames_play_in_name_order_at_rate_and_loop() {
        let dir = tempfile::tempdir().unwrap();
        for (name, v) in [("b.png", 20u8), ("a.png", 10), ("note.txt", 0)] {
            let path = dir.path().join(name);
            if name.ends_with(".png") {
                RgbImage::from_pixel(4, 2, image::Rgb([v, 0, 0]))
                    .save(&path)
                    .unwrap();
            } else {
                std::fs::write(&path, "x").unwrap();
            }
        }
        let mut c = open(&Backend::Frames(dir.path().into()), [4, 2], 50.0).unwrap();
        let t = Instant::now();
        let reds: Vec<u8> = (0..5)
            .map(|_| c.next().unwrap().get_pixel(0, 0)[0])
            .collect();
        assert_eq!(reds, [10, 20, 10, 20, 10]);
        // Five frames at 50 Hz: the first is immediate, four periods follow.
        assert!(
            t.elapsed() >= Duration::from_millis(75),
            "{:?}",
            t.elapsed()
        );
        assert!(frame_files(&dir.path().join("missing")).is_err());
    }
}
