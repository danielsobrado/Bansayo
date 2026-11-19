//! Capture-mode presets and selection among the modes a device advertises. Presets are
//! preferences: the result is always a mode the device listed, or the request unchanged with
//! `verified: false` when the backend lists nothing.
use bansayo_contracts::CaptureMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// 1280x720 at 30 fps.
    Basic,
    /// 1920x1080 at 60 fps.
    Balanced,
    /// 1280x720 at 120 fps.
    FastMotion,
    /// 1920x1080 at 120 fps.
    Quality,
}

impl Preset {
    pub fn request(self) -> ([u32; 2], f64) {
        match self {
            Self::Basic => ([1280, 720], 30.0),
            Self::Balanced => ([1920, 1080], 60.0),
            Self::FastMotion => ([1280, 720], 120.0),
            Self::Quality => ([1920, 1080], 120.0),
        }
    }
}

/// The chosen mode and, when it is not what was asked for, why.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub mode: CaptureMode,
    pub note: Option<String>,
}

/// Compressed formats keep 720p and 1080p at full rate over USB 2.
const PREFERRED_FORMATS: [&str; 2] = ["MJPEG", "MJPG"];

/// Picks the advertised mode at the requested size with the highest rate not above the request;
/// then each fallback rate; then the highest rate below the request. The size never changes
/// silently: intrinsics and landmarks are bound to it.
pub fn select(
    advertised: &[CaptureMode],
    resolution_px: [u32; 2],
    fps: f64,
    fallback_fps: &[f64],
) -> Selection {
    let requested = CaptureMode {
        resolution_px,
        pixel_format: "MJPEG".into(),
        fps: Some(fps as f32),
        verified: false,
    };
    let at_size: Vec<&CaptureMode> = advertised
        .iter()
        .filter(|m| m.resolution_px == resolution_px && m.fps.is_some_and(|f| f > 0.0))
        .collect();
    if at_size.is_empty() {
        let note = if advertised.is_empty() {
            "the backend lists no modes; requesting the configured mode unverified".to_owned()
        } else {
            format!(
                "{}x{} is not advertised; requesting it unverified",
                resolution_px[0], resolution_px[1]
            )
        };
        return Selection {
            mode: requested,
            note: Some(note),
        };
    }
    let rank = |m: &&&CaptureMode| {
        (
            PREFERRED_FORMATS.contains(&m.pixel_format.as_str()),
            (m.fps.unwrap_or(0.0) * 1000.0) as i64,
        )
    };
    let best_at = |limit: f64| {
        at_size
            .iter()
            .filter(|m| m.fps.unwrap_or(0.0) as f64 <= limit + 0.5)
            .max_by_key(rank)
            .map(|m| (*m).clone())
    };
    if let Some(mode) = best_at(fps).filter(|m| (m.fps.unwrap_or(0.0) as f64 - fps).abs() < 0.5) {
        return Selection { mode, note: None };
    }
    for f in fallback_fps {
        if let Some(mode) = best_at(*f).filter(|m| (m.fps.unwrap_or(0.0) as f64 - f).abs() < 0.5) {
            return Selection {
                note: Some(format!("{fps} fps is not offered; using fallback {f} fps")),
                mode,
            };
        }
    }
    let mode = best_at(fps)
        .or_else(|| {
            at_size
                .iter()
                .min_by_key(|m| rank(m).1)
                .map(|m| (*m).clone())
        })
        .unwrap_or(requested);
    Selection {
        note: Some(format!(
            "{fps} fps is not offered at {}x{}; the nearest is {} fps",
            resolution_px[0],
            resolution_px[1],
            mode.fps.unwrap_or(0.0)
        )),
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(format: &str, w: u32, h: u32, fps: f32) -> CaptureMode {
        CaptureMode {
            resolution_px: [w, h],
            pixel_format: format.into(),
            fps: Some(fps),
            verified: true,
        }
    }
    /// The UGREEN webcam on the owner's PC: MJPEG up to 1080p at 30, YUY2 slower.
    fn ugreen() -> Vec<CaptureMode> {
        vec![
            m("MJPEG", 1920, 1080, 30.0),
            m("MJPEG", 1280, 720, 30.0),
            m("MJPEG", 1280, 720, 15.0),
            m("YUYV", 1280, 720, 10.0),
            m("YUYV", 640, 480, 30.0),
        ]
    }

    #[test]
    fn exact_mode_is_chosen_and_mjpeg_preferred() {
        let s = select(&ugreen(), [1280, 720], 30.0, &[]);
        assert_eq!(s.mode, m("MJPEG", 1280, 720, 30.0));
        assert_eq!(s.note, None);
    }

    #[test]
    fn unsupported_rate_is_never_invented() {
        let (size, fps) = Preset::FastMotion.request();
        let s = select(&ugreen(), size, fps, &[60.0, 30.0]);
        assert_eq!(s.mode.fps, Some(30.0), "120 and 60 are not offered");
        assert!(s.note.unwrap().contains("fallback 30"));
        let s = select(&ugreen(), size, fps, &[]);
        assert_eq!(s.mode.fps, Some(30.0));
        assert!(s.note.unwrap().contains("nearest is 30"));
    }

    #[test]
    fn unadvertised_size_is_requested_unverified() {
        let s = select(&ugreen(), [800, 600], 30.0, &[]);
        assert!(!s.mode.verified);
        assert_eq!(s.mode.resolution_px, [800, 600]);
        assert!(s.note.is_some());
        let s = select(&[], [1280, 720], 60.0, &[]);
        assert!(!s.mode.verified && s.note.unwrap().contains("no modes"));
    }

    #[test]
    fn a_fast_camera_gets_its_fast_mode() {
        let cams = vec![m("MJPEG", 1280, 720, 120.0), m("MJPEG", 1280, 720, 60.0)];
        assert_eq!(select(&cams, [1280, 720], 120.0, &[]).mode.fps, Some(120.0));
        assert_eq!(select(&cams, [1280, 720], 60.0, &[]).mode.fps, Some(60.0));
        assert_eq!(select(&cams, [1280, 720], 90.0, &[]).mode.fps, Some(60.0));
    }
}
