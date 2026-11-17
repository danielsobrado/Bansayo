//! 2D body pose: YOLOX person detection, then RTMPose (SimCC) on the chosen person's box.
//!
//! Pre/post-processing follows the models' bundled mmdeploy `pipeline.json` (see models/README.md):
//! YOLOX takes a keep-ratio 640x640 letterbox padded with 114, BGR, unnormalised, NMS in-graph;
//! RTMPose takes a 192x256 crop of the box padded by 1.25 at 3:4, RGB, ImageNet mean/std, and
//! decodes SimCC with split ratio 2 and score = min(max_x, max_y).
use bansayo_contracts::Landmark2D;
use image::RgbImage;
use ort::{session::Session, value::Tensor};
use std::{path::Path, sync::OnceLock};

pub const DETECTOR_MODEL: &str = "yolox-m-humanart.onnx";
pub const POSE_MODEL: &str = "rtmpose-m.onnx";
pub const DETECTOR_NAME: &str = "yolox-m-humanart+rtmpose-m";
const DET_SIZE: usize = 640;
const POSE_W: usize = 192;
const POSE_H: usize = 256;
const SIMCC_SPLIT: f32 = 2.0;
const KEYPOINTS: usize = 17;
/// The detector runs at least this often; between runs the box follows the previous keypoints.
const REDETECT_EVERY: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Execution {
    /// DirectML (Windows GPU) when available, otherwise CPU.
    #[default]
    Auto,
    Cpu,
}

static ENV: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();

/// ONNX Runtime shared library under `models_dir`, loaded at run time (see models/README.md).
pub fn runtime_library(models_dir: &Path) -> std::path::PathBuf {
    use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
    models_dir
        .join("onnxruntime")
        .join(format!("{DLL_PREFIX}onnxruntime{DLL_SUFFIX}"))
}

fn session(models_dir: &Path, model: &str, execution: Execution) -> Result<Session, String> {
    let path = models_dir.join(model);
    let env = ENV
        .get_or_init(|| {
            let library = runtime_library(models_dir);
            let builder = if library.exists() {
                ort::init_from(&library).map_err(|e| format!("{}: {e}", library.display()))?
            } else {
                // ORT_DYLIB_PATH, or the system library search path.
                ort::init()
            };
            builder
                .with_name("bansayo")
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let mut builder = Session::builder(env).map_err(|e| e.to_string())?;
    if execution == Execution::Auto {
        #[cfg(windows)]
        {
            builder = builder
                .with_execution_providers([ort::ep::DirectML::default().build()])
                .map_err(|e| e.to_string())?;
        }
    }
    builder
        .commit_from_file(&path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// One person's keypoints in image pixels, COCO-17 order; `None` below the keypoint threshold.
#[derive(Debug, Clone)]
pub struct Detection {
    /// `[x1, y1, x2, y2]`, pixels.
    pub bbox: [f32; 4],
    pub score: f32,
    pub points: Vec<Option<Landmark2D>>,
}

pub struct Detector {
    det: Session,
    pose: Session,
    pub min_person_score: f32,
    pub min_keypoint_score: f32,
}

/// Per-camera tracking state: the followed subject's box and how long since a full detection.
#[derive(Default)]
pub struct Track {
    bbox: Option<[f32; 4]>,
    since_detect: u32,
}

impl Detector {
    pub fn load(
        models_dir: &Path,
        execution: Execution,
        min_person_score: f32,
        min_keypoint_score: f32,
    ) -> Result<Self, String> {
        Ok(Self {
            det: session(models_dir, DETECTOR_MODEL, execution)?,
            pose: session(models_dir, POSE_MODEL, execution)?,
            min_person_score,
            min_keypoint_score,
        })
    }

    /// Person boxes `[x1, y1, x2, y2, score]` in image pixels, best first.
    pub fn people(&mut self, img: &RgbImage) -> Result<Vec<[f32; 5]>, String> {
        let (w, h) = img.dimensions();
        let s = (DET_SIZE as f32 / w as f32).min(DET_SIZE as f32 / h as f32);
        // Half-pixel centres, as a resize does; outside the scaled image is the 114 pad.
        let input = sample(
            img,
            DET_SIZE,
            DET_SIZE,
            [1.0 / s, 0.5 / s - 0.5, 1.0 / s, 0.5 / s - 0.5],
            114.0,
            [2, 1, 0], // BGR, unnormalised
            |_, v| v,
        );
        let tensor = Tensor::from_array(([1usize, 3, DET_SIZE, DET_SIZE], input))
            .map_err(|e| e.to_string())?;
        let out = self
            .det
            .run(ort::inputs!["input" => tensor])
            .map_err(|e| e.to_string())?;
        let (_, dets) = out["dets"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let (_, labels) = out["labels"]
            .try_extract_tensor::<i64>()
            .map_err(|e| e.to_string())?;
        let mut people: Vec<[f32; 5]> = dets
            .chunks_exact(5)
            .zip(labels)
            .filter(|(d, l)| **l == 0 && d[4] >= self.min_person_score)
            .map(|(d, _)| [d[0] / s, d[1] / s, d[2] / s, d[3] / s, d[4]])
            .collect();
        people.sort_by(|a, b| b[4].total_cmp(&a[4]));
        Ok(people)
    }

    /// Keypoints of the person in `bbox`.
    pub fn keypoints(&mut self, img: &RgbImage, bbox: [f32; 4]) -> Result<Detection, String> {
        let [cx, cy] = [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0];
        let mut bw = (bbox[2] - bbox[0]) * 1.25;
        let mut bh = (bbox[3] - bbox[1]) * 1.25;
        let aspect = POSE_W as f32 / POSE_H as f32;
        if bw > bh * aspect {
            bh = bw / aspect;
        } else {
            bw = bh * aspect;
        }
        let (sx, sy) = (bw / POSE_W as f32, bh / POSE_H as f32);
        const MEAN: [f32; 3] = [123.675, 116.28, 103.53];
        const STD: [f32; 3] = [58.395, 57.12, 57.375];
        let input = sample(
            img,
            POSE_W,
            POSE_H,
            [sx, cx - bw / 2.0, sy, cy - bh / 2.0],
            0.0,
            [0, 1, 2],
            |c, v| (v - MEAN[c]) / STD[c],
        );
        let tensor =
            Tensor::from_array(([1usize, 3, POSE_H, POSE_W], input)).map_err(|e| e.to_string())?;
        let out = self
            .pose
            .run(ort::inputs!["input" => tensor])
            .map_err(|e| e.to_string())?;
        let (_, x) = out["simcc_x"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let (_, y) = out["simcc_y"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let (nx, ny) = (x.len() / KEYPOINTS, y.len() / KEYPOINTS);
        let argmax = |row: &[f32]| {
            row.iter().enumerate().fold(
                (0, f32::MIN),
                |b, (i, v)| if *v > b.1 { (i, *v) } else { b },
            )
        };
        let points = (0..KEYPOINTS)
            .map(|k| {
                let (ix, vx) = argmax(&x[k * nx..(k + 1) * nx]);
                let (iy, vy) = argmax(&y[k * ny..(k + 1) * ny]);
                let confidence = vx.min(vy).clamp(0.0, 1.0);
                (confidence >= self.min_keypoint_score).then(|| Landmark2D {
                    x_px: ix as f32 / SIMCC_SPLIT * sx + cx - bw / 2.0,
                    y_px: iy as f32 / SIMCC_SPLIT * sy + cy - bh / 2.0,
                    confidence,
                })
            })
            .collect();
        Ok(Detection {
            bbox,
            score: 1.0,
            points,
        })
    }

    /// Follows one subject: re-detects periodically or when lost, preferring the box that overlaps
    /// the followed one so the subject does not jump between people.
    pub fn track(
        &mut self,
        img: &RgbImage,
        track: &mut Track,
    ) -> Result<Option<Detection>, String> {
        let box_from_points = track.bbox.filter(|_| track.since_detect < REDETECT_EVERY);
        let (bbox, score) = match box_from_points {
            Some(b) => {
                track.since_detect += 1;
                (b, None)
            }
            None => {
                track.since_detect = 0;
                let people = self.people(img)?;
                let chosen = track
                    .bbox
                    .and_then(|prev| {
                        people
                            .iter()
                            .map(|p| (iou(prev, [p[0], p[1], p[2], p[3]]), p))
                            .filter(|(o, _)| *o > 0.3)
                            .max_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, p)| *p)
                    })
                    .or_else(|| people.first().copied());
                match chosen {
                    Some(p) => ([p[0], p[1], p[2], p[3]], Some(p[4])),
                    None => {
                        track.bbox = None;
                        return Ok(None);
                    }
                }
            }
        };
        let mut detection = self.keypoints(img, bbox)?;
        detection.score = score.unwrap_or(1.0);
        let seen: Vec<&Landmark2D> = detection.points.iter().flatten().collect();
        // Too few confident points: the subject is lost; detect again next frame.
        track.bbox = (seen.len() >= 6).then(|| {
            let (x0, x1) = seen.iter().fold((f32::MAX, f32::MIN), |a, p| {
                (a.0.min(p.x_px), a.1.max(p.x_px))
            });
            let (y0, y1) = seen.iter().fold((f32::MAX, f32::MIN), |a, p| {
                (a.0.min(p.y_px), a.1.max(p.y_px))
            });
            // Keypoints sit inside the body outline; grow the box back towards a person box.
            let (mx, my) = ((x1 - x0) * 0.1, (y1 - y0) * 0.1);
            [x0 - mx, y0 - my, x1 + mx, y1 + my]
        });
        Ok(Some(detection))
    }
}

fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    let inter = w * h;
    inter / (area(a) + area(b) - inter).max(1e-6)
}

/// Bilinear resample into planar NCHW floats. Output pixel `(x, y)` samples the source at
/// `(m[0] * x + m[1], m[2] * y + m[3])`; samples outside the image take `fill`. Output channel `c`
/// reads source channel `order[c]` and stores `norm(c, value)`.
fn sample(
    img: &RgbImage,
    out_w: usize,
    out_h: usize,
    m: [f32; 4],
    fill: f32,
    order: [usize; 3],
    norm: impl Fn(usize, f32) -> f32,
) -> Vec<f32> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let raw = img.as_raw();
    let mut out = vec![0.0f32; 3 * out_w * out_h];
    for y in 0..out_h {
        let sy = m[2] * y as f32 + m[3];
        for x in 0..out_w {
            let sx = m[0] * x as f32 + m[1];
            let inside = sx >= -0.5 && sy >= -0.5 && sx <= w as f32 - 0.5 && sy <= h as f32 - 0.5;
            let mut rgb = [fill; 3];
            if inside {
                let (fx, fy) = (sx.clamp(0.0, (w - 1) as f32), sy.clamp(0.0, (h - 1) as f32));
                let (x0, y0) = (fx as usize, fy as usize);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
                let px = |xx: usize, yy: usize, c: usize| raw[(yy * w + xx) * 3 + c] as f32;
                for (c, v) in rgb.iter_mut().enumerate() {
                    let top = px(x0, y0, c) * (1.0 - ax) + px(x1, y0, c) * ax;
                    let bottom = px(x0, y1, c) * (1.0 - ax) + px(x1, y1, c) * ax;
                    *v = top * (1.0 - ay) + bottom * ay;
                }
            }
            for (c, source) in order.iter().enumerate() {
                out[c * out_w * out_h + y * out_w + x] = norm(c, rgb[*source]);
            }
        }
    }
    out
}

/// One-Euro filter (Casiez et al. 2012) on one scalar: smooths jitter at rest, follows fast motion.
// shortcut: fixed parameters tuned on nothing; tune min_cutoff/beta on recorded real footage.
#[derive(Clone, Copy)]
pub struct OneEuro {
    x: f32,
    dx: f32,
    t_us: u64,
}
pub const SMOOTHING_IDENTITY: &str = "one_euro";
pub const SMOOTHING_PARAMETERS: [f32; 3] = [MIN_CUTOFF_HZ, BETA, D_CUTOFF_HZ];
const MIN_CUTOFF_HZ: f32 = 1.5;
const BETA: f32 = 0.02;
const D_CUTOFF_HZ: f32 = 1.0;
impl OneEuro {
    pub fn new(x: f32, t_us: u64) -> Self {
        Self { x, dx: 0.0, t_us }
    }
    pub fn filter(&mut self, x: f32, t_us: u64) -> f32 {
        let dt = (t_us.saturating_sub(self.t_us) as f32 / 1e6).max(1e-4);
        let alpha = |cutoff: f32| {
            let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
            1.0 / (1.0 + tau / dt)
        };
        let dx = (x - self.x) / dt;
        self.dx += alpha(D_CUTOFF_HZ) * (dx - self.dx);
        let cutoff = MIN_CUTOFF_HZ + BETA * self.dx.abs();
        self.x += alpha(cutoff) * (x - self.x);
        self.t_us = t_us;
        self.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn letterbox_sampling_matches_pixels_and_pads() {
        let img = RgbImage::from_fn(4, 2, |x, y| image::Rgb([x as u8 * 10, y as u8 * 100, 7]));
        // Identity mapping into a larger canvas: in-image pixels copied, outside filled.
        let out = sample(&img, 6, 3, [1.0, 0.0, 1.0, 0.0], 114.0, [0, 1, 2], |_, v| v);
        let at = |c: usize, x: usize, y: usize| out[c * 18 + y * 6 + x];
        assert_eq!(at(0, 3, 1), 30.0);
        assert_eq!(at(1, 3, 1), 100.0);
        assert_eq!(at(2, 0, 0), 7.0);
        assert_eq!(at(0, 5, 0), 114.0);
        assert_eq!(at(1, 0, 2), 114.0);
        // BGR swap.
        let bgr = sample(&img, 4, 2, [1.0, 0.0, 1.0, 0.0], 0.0, [2, 1, 0], |_, v| v);
        assert_eq!(bgr[0], 7.0);
    }
    #[test]
    fn one_euro_holds_still_and_follows_steps() {
        let mut f = OneEuro::new(0.0, 0);
        for i in 1..30 {
            let v = f.filter(if i % 2 == 0 { 1.0 } else { -1.0 }, i * 33_333);
            assert!(v.abs() < 0.6, "jitter {v}");
        }
        let mut f = OneEuro::new(0.0, 0);
        let mut v = 0.0;
        for i in 1..30 {
            v = f.filter(100.0, i * 33_333);
        }
        assert!(v > 99.0, "lag {v}");
    }
    /// Needs the local model files and a photo of one person; run with
    /// `BANSAYO_POSE_IMAGE=photo.jpg cargo test -p bansayo-engine -- --ignored detects`.
    #[test]
    #[ignore]
    fn detects_a_person_in_a_photo() {
        let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        let path = std::env::var("BANSAYO_POSE_IMAGE").expect("BANSAYO_POSE_IMAGE");
        let img = image::open(path).unwrap().to_rgb8();
        let exec = if std::env::var("BANSAYO_POSE_CPU").is_ok() {
            Execution::Cpu
        } else {
            Execution::Auto
        };
        let mut d = Detector::load(&models, exec, 0.5, 0.3).unwrap();
        let mut track = Track::default();
        let t = std::time::Instant::now();
        let det = d.track(&img, &mut track).unwrap().expect("a person");
        eprintln!("first frame {:?}", t.elapsed());
        let t = std::time::Instant::now();
        for _ in 0..20 {
            d.track(&img, &mut track).unwrap();
        }
        eprintln!("per tracked frame {:?}", t.elapsed() / 20);
        for (i, p) in det.points.iter().enumerate() {
            eprintln!("{i} {p:?}");
        }
        eprintln!("bbox {:?} score {}", det.bbox, det.score);
        assert!(det.points.iter().filter(|p| p.is_some()).count() >= 12);
    }
}
