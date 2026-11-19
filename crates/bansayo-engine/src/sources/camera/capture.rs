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
    /// JPEG frames sent by a network client to `/api/v1/ingest/camera/{id}` (ADR-042).
    Network(String),
    /// An IP camera's RTSP stream, read through `ffmpeg` (ADR-020 item 7).
    Rtsp(String),
}

/// The URL without `user:password@`, for logs and device descriptions.
pub fn redact(url: &str) -> String {
    match (url.find("://"), url.find('@')) {
        (Some(s), Some(a)) if a > s => format!("{}***@{}", &url[..s + 3], &url[a + 1..]),
        _ => url.to_owned(),
    }
}

/// The host (and port) of a URL, for a camera's model name.
pub fn host_of(url: &str) -> String {
    let rest = redact(url);
    let rest = rest
        .split_once("://")
        .map_or(rest.as_str(), |(_, r)| r)
        .to_owned();
    let rest = rest
        .rsplit_once('@')
        .map_or(rest.as_str(), |(_, r)| r)
        .to_owned();
    rest.split('/').next().unwrap_or_default().to_owned()
}

/// Takes the first complete JPEG (SOI … EOI) from the front of `buf`, dropping bytes before it.
pub fn take_jpeg(buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    let start = buf.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let end = buf[start + 2..]
        .windows(2)
        .position(|w| w == [0xFF, 0xD9])?
        + start
        + 4;
    let jpeg = buf[start..end].to_vec();
    buf.drain(..end);
    Some(jpeg)
}

/// An IP camera through `ffmpeg`: RTSP over TCP, decoded and re-encoded as MJPEG at the declared
/// size and rate on stdout. The camera keeps its own exposure; timing is arrival time
/// (`time_quality: receive_time`), so set `latency_ms` to the stream's delay.
struct Ffmpeg {
    child: std::process::Child,
    out: std::process::ChildStdout,
    buf: Vec<u8>,
    decode_us: u64,
    label: String,
}

impl Ffmpeg {
    fn open(url: &str, [w, h]: [u32; 2], rate_hz: f64) -> Result<Self, String> {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error"]);
        if url.starts_with("rtsp://") {
            // 5 s socket timeout: an offline camera ends ffmpeg, so the camera is reported
            // disconnected and reopened instead of a capture thread waiting forever.
            cmd.args([
                "-rtsp_transport",
                "tcp",
                "-timeout",
                "5000000",
                "-fflags",
                "nobuffer",
                "-flags",
                "low_delay",
            ]);
        } else {
            // Files (tests): play at their own rate, as a camera would.
            cmd.arg("-re");
        }
        cmd.args(["-i", url, "-an", "-vf"])
            .arg(format!("scale={w}:{h}"))
            .args([
                "-r",
                &format!("{rate_hz}"),
                "-f",
                "image2pipe",
                "-c:v",
                "mjpeg",
                "-q:v",
                "5",
                "-",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console window per camera
        }
        let mut child = cmd.spawn().map_err(|e| {
            format!("ffmpeg could not be started ({e}); install ffmpeg and put it on PATH")
        })?;
        let out = child.stdout.take().ok_or("ffmpeg has no output")?;
        Ok(Self {
            child,
            out,
            buf: Vec::with_capacity(1 << 20),
            decode_us: 0,
            label: redact(url),
        })
    }

    /// Stops ffmpeg and anything it started: package managers install `ffmpeg` as a shim that
    /// runs the real binary as a child, which a plain kill would leave running.
    fn stop(&mut self) {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(0x0800_0000)
                .status();
        }
        let _ = self.child.kill();
    }

    /// Why ffmpeg stopped: its last error line, without credentials.
    fn failure(&mut self) -> String {
        self.stop();
        let mut err = String::new();
        if let Some(mut e) = self.child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        let last = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("stream ended");
        format!("{}: {}", self.label, redact(last))
    }
}

impl Capture for Ffmpeg {
    fn next(&mut self) -> Result<RgbImage, String> {
        let jpeg = loop {
            if let Some(j) = take_jpeg(&mut self.buf) {
                break j;
            }
            let mut chunk = [0u8; 65536];
            match std::io::Read::read(&mut self.out, &mut chunk) {
                Ok(0) | Err(_) => return Err(self.failure()),
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
            }
        };
        let started = Instant::now();
        let image = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
            .map_err(|e| format!("{}: frame: {e}", self.label))?
            .to_rgb8();
        self.decode_us = started.elapsed().as_micros() as u64;
        Ok(image)
    }
    fn decode_us(&self) -> u64 {
        self.decode_us
    }
    fn describe(&self) -> String {
        format!("RTSP {} through ffmpeg", self.label)
    }
}

impl Drop for Ffmpeg {
    fn drop(&mut self) {
        self.stop();
        let _ = self.child.wait();
    }
}

/// The newest JPEG frame a network client sent for one camera: a one-slot buffer, so a frame not
/// yet taken is replaced and counted as dropped (08 §3).
#[derive(Default)]
pub struct NetworkFeed {
    frame: std::sync::Mutex<Option<bytes::Bytes>>,
    wake: std::sync::Condvar,
    pub dropped: std::sync::atomic::AtomicU64,
}

impl NetworkFeed {
    pub fn push(&self, jpeg: bytes::Bytes) {
        if let Ok(mut f) = self.frame.lock() {
            if f.is_some() {
                self.dropped
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            *f = Some(jpeg);
            self.wake.notify_one();
        }
    }
    fn take(&self, timeout: Duration) -> Option<bytes::Bytes> {
        let guard = self.frame.lock().ok()?;
        let (mut guard, _) = self
            .wake
            .wait_timeout_while(guard, timeout, |f| f.is_none())
            .ok()?;
        guard.take()
    }
}

/// Network feeds by camera id, shared by the API (producer) and the capture threads.
pub type NetworkFeeds =
    std::sync::Arc<std::collections::BTreeMap<String, std::sync::Arc<NetworkFeed>>>;

/// No frame for this long: the network camera is disconnected.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(2);

struct Network {
    feed: std::sync::Arc<NetworkFeed>,
    decode_us: u64,
}
impl Capture for Network {
    fn next(&mut self) -> Result<RgbImage, String> {
        let jpeg = self
            .feed
            .take(NETWORK_TIMEOUT)
            .ok_or("no frame from the network client for 2 s")?;
        let started = Instant::now();
        let image = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
            .map_err(|e| format!("network frame: {e}"))?
            .to_rgb8();
        self.decode_us = started.elapsed().as_micros() as u64;
        Ok(image)
    }
    fn decode_us(&self) -> u64 {
        self.decode_us
    }
    fn describe(&self) -> String {
        "JPEG frames over the network".into()
    }
}

/// A blocking frame producer. `next` returns as soon as a frame is available. Opened, used and
/// dropped on its camera's capture thread.
pub trait Capture {
    fn next(&mut self) -> Result<RgbImage, String>;
    /// How long decoding the frame `next` just returned took, microseconds (15 §4.1).
    fn decode_us(&self) -> u64 {
        0
    }
    /// Recorded landmarks of the frame `next` just returned, when the source carries them.
    fn recorded_landmarks(&self) -> Option<Vec<Option<bansayo_contracts::Landmark2D>>> {
        None
    }
    /// Negotiated format, for the log.
    fn describe(&self) -> String;
}

pub fn open(
    backend: &Backend,
    resolution_px: [u32; 2],
    rate_hz: f64,
) -> Result<Box<dyn Capture>, String> {
    open_with(backend, resolution_px, rate_hz, &NetworkFeeds::default())
}

/// As [`open`], with the network feeds a `Network` backend reads from. Opening a network camera
/// waits for its first frame, so an absent phone is reported as unavailable, not as a stream.
pub fn open_with(
    backend: &Backend,
    resolution_px: [u32; 2],
    rate_hz: f64,
    feeds: &NetworkFeeds,
) -> Result<Box<dyn Capture>, String> {
    match backend {
        Backend::Frames(dir) => Ok(Box::new(Frames::open(dir, rate_hz)?)),
        Backend::Device(index) => device::open(*index, resolution_px, rate_hz),
        Backend::Rtsp(url) => Ok(Box::new(Ffmpeg::open(url, resolution_px, rate_hz)?)),
        Backend::Network(id) => {
            let feed = feeds
                .get(id)
                .cloned()
                .ok_or_else(|| format!("{id}: no network feed"))?;
            {
                let guard = feed.frame.lock().map_err(|e| e.to_string())?;
                let (guard, _) = feed
                    .wake
                    .wait_timeout_while(guard, NETWORK_TIMEOUT, |f| f.is_none())
                    .map_err(|e| e.to_string())?;
                if guard.is_none() {
                    return Err("waiting for the phone to connect and send frames".into());
                }
            }
            Ok(Box::new(Network { feed, decode_us: 0 }))
        }
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

/// File in an image folder holding recorded COCO-17 landmarks, one JSON line per image in name
/// order: an array of 17 entries, each `null` or `[x_px, y_px, confidence]`. With it the camera
/// replays those landmarks instead of running pose inference, so footage annotated or recorded
/// elsewhere, and the tests, need no model weights.
pub const RECORDED_LANDMARKS: &str = "landmarks.jsonl";

type Points = Vec<Option<bansayo_contracts::Landmark2D>>;

/// Rows of an image folder's `landmarks.jsonl`, or `None` when the folder has none.
pub fn recorded_landmarks(dir: &Path) -> Result<Option<Vec<Points>>, String> {
    let path = dir.join(RECORDED_LANDMARKS);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            let row: Vec<Option<[f32; 3]>> = serde_json::from_str(line)
                .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
            if row.len() != 17 {
                return Err(format!(
                    "{} line {}: 17 COCO-17 entries expected, found {}",
                    path.display(),
                    i + 1,
                    row.len()
                ));
            }
            Ok(row
                .into_iter()
                .map(|p| {
                    p.map(|[x, y, c]| bansayo_contracts::Landmark2D {
                        x_px: x,
                        y_px: y,
                        confidence: c,
                    })
                })
                .collect())
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

struct Frames {
    files: Vec<PathBuf>,
    decode_us: u64,
    landmarks: Option<Vec<Points>>,
    index: usize,
    period: Duration,
    due: Instant,
}
impl Frames {
    fn open(dir: &Path, rate_hz: f64) -> Result<Self, String> {
        let files = frame_files(dir)?;
        let landmarks = recorded_landmarks(dir)?;
        if let Some(rows) = &landmarks
            && rows.len() != files.len()
        {
            return Err(format!(
                "{}: {} has {} rows for {} images",
                dir.display(),
                RECORDED_LANDMARKS,
                rows.len(),
                files.len()
            ));
        }
        Ok(Self {
            files,
            decode_us: 0,
            landmarks,
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
        let started = Instant::now();
        let image = image::open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .to_rgb8();
        self.decode_us = started.elapsed().as_micros() as u64;
        Ok(image)
    }
    fn decode_us(&self) -> u64 {
        self.decode_us
    }
    fn recorded_landmarks(&self) -> Option<Points> {
        let rows = self.landmarks.as_ref()?;
        rows.get((self.index + rows.len() - 1) % rows.len())
            .cloned()
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

    struct Device(Camera, u64);

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
        Ok(Box::new(Device(camera, 0)))
    }

    impl Capture for Device {
        fn next(&mut self) -> Result<RgbImage, String> {
            let buffer = self.0.frame().map_err(|e| e.to_string())?;
            let started = std::time::Instant::now();
            let image = if buffer.source_frame_format() == FrameFormat::MJPEG {
                image::load_from_memory_with_format(buffer.buffer(), image::ImageFormat::Jpeg)
                    .map(|i| i.to_rgb8())
                    .map_err(|e| format!("MJPEG decode: {e}"))
            } else {
                buffer
                    .decode_image::<RgbFormat>()
                    .map_err(|e| e.to_string())
            };
            self.1 = started.elapsed().as_micros() as u64;
            image
        }
        fn decode_us(&self) -> u64 {
            self.1
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
    fn network_frames_decode_replace_and_time_out() {
        let feed = std::sync::Arc::new(NetworkFeed::default());
        let feeds: NetworkFeeds = std::sync::Arc::new([("PHONE".to_owned(), feed.clone())].into());
        let backend = Backend::Network("PHONE".into());
        // Nobody sending: opening waits 2 s, then says so.
        assert!(open_with(&backend, [8, 4], 30.0, &feeds).is_err());
        let jpeg = |v: u8| {
            let mut out = vec![];
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95)
                .encode_image(&RgbImage::from_pixel(8, 4, image::Rgb([v, v, v])))
                .unwrap();
            bytes::Bytes::from(out)
        };
        feed.push(jpeg(10));
        let mut c = open_with(&backend, [8, 4], 30.0, &feeds).unwrap();
        // The older frame is replaced by the newer one and counted as dropped.
        feed.push(jpeg(200));
        let img = c.next().unwrap();
        assert_eq!(img.dimensions(), (8, 4));
        assert!(img.get_pixel(0, 0)[0] > 150, "{:?}", img.get_pixel(0, 0));
        assert_eq!(feed.dropped.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(c.next().is_err(), "no frame for 2 s");
        assert!(open_with(&Backend::Network("OTHER".into()), [8, 4], 30.0, &feeds).is_err());
    }

    #[test]
    fn jpeg_frames_are_split_and_credentials_redacted() {
        let mut buf = vec![0x00, 0xFF, 0xD8, 1, 2, 0xFF, 0xD9, 0xFF, 0xD8, 3];
        assert_eq!(take_jpeg(&mut buf).unwrap(), [0xFF, 0xD8, 1, 2, 0xFF, 0xD9]);
        assert_eq!(buf, [0xFF, 0xD8, 3], "the next, incomplete frame stays");
        assert!(take_jpeg(&mut buf).is_none());
        let url = "rtsp://admin:s3cret@192.168.1.40:554/h264Preview_01_main";
        assert_eq!(
            redact(url),
            "rtsp://***@192.168.1.40:554/h264Preview_01_main"
        );
        assert_eq!(host_of(url), "192.168.1.40:554");
        assert_eq!(host_of("rtsp://10.0.0.5/live"), "10.0.0.5");
        assert_eq!(redact("rtsp://10.0.0.5/live"), "rtsp://10.0.0.5/live");
    }

    /// The ffmpeg path on a generated clip (skipped without ffmpeg on PATH): frames arrive at the
    /// declared size; a stream that cannot be opened fails without showing the password.
    #[test]
    fn rtsp_backend_reads_frames_through_ffmpeg() {
        let ok = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output();
        if !ok.is_ok_and(|o| o.status.success()) {
            eprintln!("ffmpeg not on PATH; skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=25:duration=1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success());
        let backend = Backend::Rtsp(clip.display().to_string());
        let mut c = open(&backend, [160, 120], 25.0).unwrap();
        let mut n = 0;
        while let Ok(img) = c.next() {
            assert_eq!(img.dimensions(), (160, 120));
            n += 1;
        }
        assert!(
            (20..=30).contains(&n),
            "{n} frames from a 1 s clip at 25 fps"
        );
        let mut bad = open(
            &Backend::Rtsp("rtsp://admin:s3cret@127.0.0.1:9/none".into()),
            [160, 120],
            25.0,
        )
        .unwrap();
        let err = bad.next().unwrap_err();
        assert!(!err.contains("s3cret") && err.contains("***@"), "{err}");
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
