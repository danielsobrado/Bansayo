//! `PlanarBody` per camera (martial-arts 04 §4, 08 §4.2): canonical keypoints of a camera's
//! newest landmarks, normalised by a reference torso length captured while the athlete holds
//! still. The capture window opens when a training session is created and the reference is held
//! for that session; until it is captured `scale_px` is `null` and no keypoint is given.
//!
//! The capture depends only on observations and the session's creation time, so a re-analysis
//! that opens the window at the same time captures the same reference (12 §7.2).
use crate::registry::{ObservationKind, Registry};
use bansayo_contracts::*;
use std::collections::{BTreeMap, VecDeque};

/// Stillness needed to capture the reference (06 §3: 500 ms).
pub const STILL_US: u64 = 500_000;
/// A keypoint counts as still while it stays within this fraction of the torso length of its mean.
pub const STILL_TOLERANCE: f32 = 0.05;
/// Keypoints whose confidence is below this are not used for the capture.
const MIN_CONFIDENCE: f32 = 0.3;
/// Pelvis, neck, shoulders, hips, knees and ankles must be seen throughout the capture.
const REQUIRED: [usize; 10] = [0, 3, 6, 10, 13, 16, 14, 17, 15, 18];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reference {
    pub scale_px: f32,
    pub origin_px: [f32; 2],
    pub view: PlanarView,
}

type Points = Vec<Option<Landmark2D>>;

#[derive(Debug, Clone, Default)]
struct Camera {
    last_seq: Option<u64>,
    history: VecDeque<(u64, Points)>,
    reference: Option<Reference>,
}

#[derive(Debug, Clone, Default)]
pub struct PlanarState {
    from_us: Option<u64>,
    cameras: BTreeMap<DeviceId, Camera>,
}

fn dist(a: &Landmark2D, b: &Landmark2D) -> f32 {
    ((a.x_px - b.x_px).powi(2) + (a.y_px - b.y_px).powi(2)).sqrt()
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// The reference if the frames (oldest first, spanning `STILL_US`) show the athlete still.
pub fn still_reference(frames: &[&Points]) -> Option<Reference> {
    if frames.len() < 3 {
        return None;
    }
    let get = |f: &Points, k: usize| {
        f.get(k)
            .and_then(Option::as_ref)
            .filter(|p| p.confidence >= MIN_CONFIDENCE)
            .cloned()
    };
    let mut torso = vec![];
    for f in frames {
        for k in REQUIRED {
            get(f, k)?;
        }
        torso.push(dist(&get(f, 0)?, &get(f, 3)?));
    }
    let scale = median(torso);
    if scale < 4.0 {
        return None;
    }
    for k in REQUIRED {
        let pts: Vec<Landmark2D> = frames.iter().filter_map(|f| get(f, k)).collect();
        let n = pts.len() as f32;
        let mean = Landmark2D {
            x_px: pts.iter().map(|p| p.x_px).sum::<f32>() / n,
            y_px: pts.iter().map(|p| p.y_px).sum::<f32>() / n,
            confidence: 1.0,
        };
        if pts.iter().any(|p| dist(p, &mean) > STILL_TOLERANCE * scale) {
            return None;
        }
    }
    let pelvis: Vec<Landmark2D> = frames.iter().filter_map(|f| get(f, 0)).collect();
    let n = pelvis.len() as f32;
    let shoulders = median(
        frames
            .iter()
            .filter_map(|f| Some(dist(&get(f, 6)?, &get(f, 10)?)))
            .collect(),
    );
    let ratio = shoulders / scale;
    Some(Reference {
        scale_px: scale,
        origin_px: [
            pelvis.iter().map(|p| p.x_px).sum::<f32>() / n,
            pelvis.iter().map(|p| p.y_px).sum::<f32>() / n,
        ],
        view: if ratio >= 0.55 {
            PlanarView::Frontal
        } else if ratio <= 0.25 {
            PlanarView::Lateral
        } else {
            PlanarView::Oblique
        },
    })
}

impl Camera {
    /// Tries a capture ending at history entry `i`.
    fn try_capture(&mut self, i: usize, from: u64) {
        let (t, _) = self.history[i];
        if t < from + STILL_US {
            return;
        }
        let frames: Vec<&Points> = self
            .history
            .iter()
            .take(i + 1)
            .filter(|(ft, _)| *ft + STILL_US >= t && *ft >= from)
            .map(|(_, p)| p)
            .collect();
        self.reference = still_reference(&frames);
    }
}

impl PlanarState {
    /// Opens a new capture window at `from_us` (a training session's creation), or closes it.
    /// Frames already received from that time on are scanned, so the result does not depend on
    /// when the caller learns of the session.
    pub fn capture_from(&mut self, from_us: Option<u64>) {
        if self.from_us == from_us {
            return;
        }
        self.from_us = from_us;
        for c in self.cameras.values_mut() {
            c.reference = None;
            if let Some(from) = from_us {
                for i in 0..c.history.len() {
                    if c.reference.is_some() {
                        break;
                    }
                    c.try_capture(i, from);
                }
            }
        }
    }

    pub fn reference(&self, camera: &DeviceId) -> Option<Reference> {
        self.cameras.get(camera).and_then(|c| c.reference)
    }

    /// One `PlanarBody` per camera whose newest landmarks are not older than `stale_us`.
    pub fn update(&mut self, registry: &Registry, now: u64, stale_us: u64) -> Vec<PlanarBody> {
        let mut out = vec![];
        for (id, entry) in &registry.entries {
            let Some(o) = entry.latest.get(&ObservationKind::Landmarks2d) else {
                continue;
            };
            let ObservationData::Landmarks2d { landmarks } = &o.data else {
                continue;
            };
            let Some(points) =
                super::schema_map::to_canonical(&landmarks.schema_id, &landmarks.points)
            else {
                continue;
            };
            let from = self.from_us;
            let cam = self.cameras.entry(id.clone()).or_default();
            if cam.last_seq != Some(o.seq) {
                cam.last_seq = Some(o.seq);
                cam.history.push_back((o.t_us, points.clone()));
                while cam
                    .history
                    .front()
                    .is_some_and(|(t, _)| o.t_us.saturating_sub(*t) > 2 * STILL_US)
                {
                    cam.history.pop_front();
                }
                if cam.reference.is_none()
                    && let Some(from) = from
                {
                    cam.try_capture(cam.history.len() - 1, from);
                }
            }
            if !entry.connected || now.saturating_sub(o.t_us) > stale_us {
                continue;
            }
            out.push(planar_body(id, o.t_us, &points, cam.reference));
        }
        out
    }
}

/// Normalised body: torso lengths, origin at the captured pelvis, +x image right, +y up.
pub fn planar_body(
    camera: &DeviceId,
    t_obs_us: u64,
    points: &Points,
    reference: Option<Reference>,
) -> PlanarBody {
    PlanarBody {
        camera_id: camera.clone(),
        t_obs_us,
        scale_px: reference.map(|r| r.scale_px),
        keypoints: points
            .iter()
            .map(|p| {
                let (r, p) = (reference?, p.as_ref()?);
                Some(PlanarKeypoint {
                    x: (p.x_px - r.origin_px[0]) / r.scale_px,
                    y: -(p.y_px - r.origin_px[1]) / r.scale_px,
                    confidence: p.confidence,
                })
            })
            .collect(),
        view: reference.map_or(PlanarView::Unknown, |r| r.view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(dx: f32, shoulder_half: f32) -> Points {
        let mut v = vec![None; 26];
        let mut set = |k: usize, x: f32, y: f32| {
            v[k] = Some(Landmark2D {
                x_px: x + dx,
                y_px: y,
                confidence: 0.9,
            })
        };
        set(0, 100.0, 200.0);
        set(3, 100.0, 100.0);
        set(6, 100.0 + shoulder_half, 100.0);
        set(10, 100.0 - shoulder_half, 100.0);
        for (k, x) in [(13, 110.0), (16, 90.0)] {
            set(k, x, 200.0);
        }
        for (k, x) in [(14, 110.0), (17, 90.0)] {
            set(k, x, 290.0);
        }
        for (k, x) in [(15, 110.0), (18, 90.0)] {
            set(k, x, 380.0);
        }
        v
    }

    #[test]
    fn stillness_captures_scale_origin_and_view() {
        let frames = [body(0.0, 30.0), body(1.0, 30.0), body(-1.0, 30.0)];
        let r = still_reference(&frames.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(r.scale_px, 100.0);
        assert_eq!(r.view, PlanarView::Frontal);
        assert!((r.origin_px[0] - 100.0).abs() < 1e-4 && r.origin_px[1] == 200.0);
        let lateral = [body(0.0, 10.0), body(0.0, 10.0), body(0.0, 10.0)];
        let r = still_reference(&lateral.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(r.view, PlanarView::Lateral);
        let oblique = [body(0.0, 20.0), body(0.0, 20.0), body(0.0, 20.0)];
        let r = still_reference(&oblique.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(r.view, PlanarView::Oblique);
        // Moving by more than 5 % of the torso length is not still.
        let moving = [body(0.0, 30.0), body(8.0, 30.0), body(-8.0, 30.0)];
        assert!(still_reference(&moving.iter().collect::<Vec<_>>()).is_none());
        // A missing ankle prevents the capture.
        let mut missing = body(0.0, 30.0);
        missing[15] = None;
        assert!(still_reference(&[&missing, &missing, &missing]).is_none());
    }

    #[test]
    fn normalisation_is_in_torso_lengths_with_y_up() {
        let r = Reference {
            scale_px: 100.0,
            origin_px: [100.0, 200.0],
            view: PlanarView::Lateral,
        };
        let id = DeviceId::new("CAM").unwrap();
        let b = planar_body(&id, 5, &body(0.0, 10.0), Some(r));
        let neck = b.keypoints[3].as_ref().unwrap();
        assert_eq!((neck.x, neck.y), (0.0, 1.0));
        let ankle = b.keypoints[15].as_ref().unwrap();
        assert!((ankle.x - 0.1).abs() < 1e-6 && (ankle.y + 1.8).abs() < 1e-6);
        let none = planar_body(&id, 5, &body(0.0, 10.0), None);
        assert!(none.scale_px.is_none() && none.keypoints.iter().all(Option::is_none));
        assert_eq!(none.view, PlanarView::Unknown);
    }

    #[test]
    fn capture_from_rescans_buffered_frames() {
        let mut c = Camera::default();
        for k in 0..20u64 {
            c.history.push_back((k * 50_000, body(0.0, 30.0)));
        }
        let mut s = PlanarState::default();
        s.cameras.insert(DeviceId::new("CAM").unwrap(), c);
        s.capture_from(Some(100_000));
        // Captured at the first frame 500 ms after the window opened, not at the last one.
        assert!(s.reference(&DeviceId::new("CAM").unwrap()).is_some());
    }
}
