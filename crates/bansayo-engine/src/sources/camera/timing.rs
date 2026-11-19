//! Measured frame arrival statistics per camera: actual rate, interval percentiles and jitter.
//! Arrival times are session-clock instants; exposure start is not inferred from them.
use bansayo_contracts::FrameTiming;
use std::collections::VecDeque;

/// Frames in the statistics window (10 s at 30 fps).
pub const WINDOW_FRAMES: usize = 300;

#[derive(Debug, Default)]
pub struct FrameStats {
    last_us: Option<u64>,
    intervals_us: VecDeque<u64>,
}

impl FrameStats {
    pub fn frame(&mut self, t_us: u64) {
        if let Some(last) = self.last_us
            && t_us > last
        {
            if self.intervals_us.len() == WINDOW_FRAMES {
                self.intervals_us.pop_front();
            }
            self.intervals_us.push_back(t_us - last);
        }
        self.last_us = Some(t_us);
    }

    /// After a reconnect the gap to the first frame is not a frame interval.
    pub fn restart(&mut self) {
        self.last_us = None;
    }

    pub fn summary(&self) -> Option<FrameTiming> {
        if self.intervals_us.len() < 2 {
            return None;
        }
        let n = self.intervals_us.len() as f64;
        let mean = self.intervals_us.iter().sum::<u64>() as f64 / n;
        let var = self
            .intervals_us
            .iter()
            .map(|v| (*v as f64 - mean).powi(2))
            .sum::<f64>()
            / n;
        let mut sorted: Vec<u64> = self.intervals_us.iter().copied().collect();
        sorted.sort_unstable();
        let pct = |p: f64| sorted[((n * p).ceil() as usize).clamp(1, sorted.len()) - 1] as f64;
        Some(FrameTiming {
            window_frames: sorted.len() as u32,
            measured_fps: (1e6 / mean) as f32,
            mean_interval_ms: (mean / 1000.0) as f32,
            p95_interval_ms: (pct(0.95) / 1000.0) as f32,
            p99_interval_ms: (pct(0.99) / 1000.0) as f32,
            jitter_ms: (var.sqrt() / 1000.0) as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_30_fps_and_a_stall() {
        let mut s = FrameStats::default();
        assert!(s.summary().is_none());
        for i in 0..50u64 {
            s.frame(i * 33_333);
        }
        let t = s.summary().unwrap();
        assert!((t.measured_fps - 30.0).abs() < 0.05, "{t:?}");
        assert!(t.jitter_ms < 0.01);
        // One 200 ms stall shows in p99 and jitter, barely in the mean.
        s.frame(49 * 33_333 + 200_000);
        let t = s.summary().unwrap();
        assert!(
            t.p99_interval_ms > 150.0 && t.p95_interval_ms < 34.0,
            "{t:?}"
        );
        assert!(t.jitter_ms > 10.0);
    }

    #[test]
    fn a_camera_asked_for_120_that_delivers_30_is_measured_at_30() {
        let mut s = FrameStats::default();
        let mut t = 0;
        for i in 0..60u64 {
            t += 33_000 + (i % 3) * 400;
            s.frame(t);
        }
        let m = s.summary().unwrap();
        assert!((29.5..30.5).contains(&m.measured_fps), "{m:?}");
        s.restart();
        s.frame(t + 5_000_000);
        assert_eq!(
            s.summary().unwrap().window_frames,
            59,
            "reconnect gap is not an interval"
        );
    }
}
