//! Session clock and device-clock offset estimation (docs/03 §2).

use std::collections::VecDeque;
use std::time::{Instant, SystemTime};

use bansayo_contracts::TimeUs;

/// Monotonic microseconds since engine start, anchored once to UTC.
#[derive(Debug, Clone)]
pub struct SessionClock {
    start: Instant,
    epoch_utc: String,
}

impl SessionClock {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
            epoch_utc: humantime::format_rfc3339_millis(SystemTime::now()).to_string(),
        }
    }

    /// Session time now.
    pub fn now_us(&self) -> TimeUs {
        u64::try_from(self.start.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    /// RFC 3339 UTC instant of session time zero.
    pub fn epoch_utc(&self) -> &str {
        &self.epoch_utc
    }

    /// Instant at which `t_us` falls.
    pub fn instant_at(&self, t_us: TimeUs) -> Instant {
        self.start + std::time::Duration::from_micros(t_us)
    }
}

/// Current UTC time as RFC 3339 with second precision.
pub fn utc_now_rfc3339() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

/// Number of recent samples the offset minimum is taken over.
pub const OFFSET_WINDOW: usize = 256;
/// Samples needed before an offset estimate is considered settled.
pub const OFFSET_SETTLE_SAMPLES: usize = 10;
/// A device clock that jumps backwards (reboot) by more than this resets the estimator.
pub const DEVICE_CLOCK_JUMP_US: i64 = 1_000_000;

/// Drift is estimated from per-second minima over at least this span of device time.
pub const DRIFT_BLOCK_US: u64 = 1_000_000;
pub const DRIFT_MIN_SPAN_US: u64 = 20_000_000;
const DRIFT_BLOCKS: usize = 120;

/// One-way offset estimate: `offset = min(receive_time − device_time)` over a sliding window.
/// Jitter only adds delay, so the minimum tracks the true offset plus the smallest latency. The
/// smallest latency itself cannot be observed one-way; it is the configured `latency_ms` or it
/// remains a bias the status does not hide.
#[derive(Debug, Clone, Default)]
pub struct OffsetEstimator {
    window: VecDeque<i64>,
    last_device_us: Option<u64>,
    resets: u32,
    /// `(device time at block start, minimum offset in block)` for the drift estimate.
    blocks: VecDeque<(u64, i64)>,
}

impl OffsetEstimator {
    /// Adds a sample and returns the session-clock capture time of `device_us`.
    pub fn observe(&mut self, device_us: u64, receive_us: TimeUs) -> TimeUs {
        if self
            .last_device_us
            .is_some_and(|last| (device_us as i64) < last as i64 - DEVICE_CLOCK_JUMP_US)
        {
            // Reboot or reset: earlier samples describe another clock.
            self.window.clear();
            self.blocks.clear();
            self.resets += 1;
        }
        self.last_device_us = Some(device_us);
        if self.window.len() == OFFSET_WINDOW {
            self.window.pop_front();
        }
        let raw = receive_us as i64 - device_us as i64;
        self.window.push_back(raw);
        match self.blocks.back_mut() {
            Some((start, min)) if device_us < start.saturating_add(DRIFT_BLOCK_US) => {
                *min = (*min).min(raw)
            }
            _ => {
                if self.blocks.len() == DRIFT_BLOCKS {
                    self.blocks.pop_front();
                }
                self.blocks.push_back((device_us, raw));
            }
        }
        u64::try_from(device_us as i64 + self.offset())
            .unwrap_or(0)
            .min(receive_us)
    }

    fn offset(&self) -> i64 {
        self.window.iter().copied().min().unwrap_or(0)
    }

    /// Whether enough samples have been seen for the estimate to be trusted.
    pub fn settled(&self) -> bool {
        self.window.len() >= OFFSET_SETTLE_SAMPLES
    }

    /// Spread of the fastest tenth of transport delays above the minimum: how firmly the window
    /// pins the offset. `None` until settled.
    pub fn uncertainty_us(&self) -> Option<u32> {
        if !self.settled() {
            return None;
        }
        let mut sorted: Vec<i64> = self.window.iter().copied().collect();
        sorted.sort_unstable();
        let p10 = sorted[sorted.len() / 10];
        u32::try_from(p10 - sorted[0]).ok()
    }

    /// Device clock rate error from a least-squares line through per-second offset minima.
    /// Positive: the device clock runs slow. `None` until `DRIFT_MIN_SPAN_US` has been seen.
    pub fn drift_ppm(&self) -> Option<f32> {
        let (first, last) = (self.blocks.front()?, self.blocks.back()?);
        if last.0.saturating_sub(first.0) < DRIFT_MIN_SPAN_US || self.blocks.len() < 3 {
            return None;
        }
        let n = self.blocks.len() as f64;
        let x0 = first.0 as f64;
        let (mx, my) = self.blocks.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| {
            (sx + (*x as f64 - x0) / n, sy + *y as f64 / n)
        });
        let (sxy, sxx) = self.blocks.iter().fold((0.0, 0.0), |(a, b), (x, y)| {
            let dx = *x as f64 - x0 - mx;
            (a + dx * (*y as f64 - my), b + dx * dx)
        });
        (sxx > 0.0).then(|| (sxy / sxx * 1e6) as f32)
    }

    pub fn resets(&self) -> u32 {
        self.resets
    }

    pub fn status(&self) -> bansayo_contracts::TimeSyncStatus {
        bansayo_contracts::TimeSyncStatus {
            time_quality: bansayo_contracts::TimeQuality::Estimated,
            offset_us: self.settled().then(|| self.offset()),
            uncertainty_us: self.uncertainty_us(),
            drift_ppm: self.drift_ppm(),
            settled: self.settled(),
            resets: self.resets,
        }
    }
}

/// Extends a wrapping 32-bit device clock (e.g. microseconds since boot on a microcontroller,
/// which wraps every 71.6 minutes) to 64 bits. A step back of more than half the range is a
/// wrap; a smaller step back is passed through, so the offset estimator sees it as a reset.
#[derive(Debug, Clone, Default)]
pub struct Unwrap32 {
    last: Option<u32>,
    high: u64,
}
impl Unwrap32 {
    pub fn extend(&mut self, t: u32) -> u64 {
        if let Some(last) = self.last
            && t < last
            && last - t > u32::MAX / 2
        {
            self.high += 1 << 32;
        }
        self.last = Some(t);
        self.high + t as u64
    }
}

/// Adapter-owned timestamp mapper. Replay already has session timestamps and never remaps them.
#[derive(Debug, Default)]
pub struct SourceTime {
    offset: OffsetEstimator,
}
impl SourceTime {
    pub fn reset(&mut self) {
        self.offset = OffsetEstimator {
            resets: self.offset.resets.saturating_add(1),
            ..Default::default()
        };
    }

    pub fn status(&self) -> bansayo_contracts::TimeSyncStatus {
        self.offset.status()
    }
    pub fn map(
        &mut self,
        device_us: Option<u64>,
        arrival_us: TimeUs,
        latency_us: u64,
    ) -> (TimeUs, bansayo_contracts::TimeQuality) {
        match device_us {
            Some(t) => (
                self.offset.observe(t, arrival_us),
                bansayo_contracts::TimeQuality::Estimated,
            ),
            None => (
                arrival_us.saturating_sub(latency_us),
                bansayo_contracts::TimeQuality::ReceiveTime,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_latency_recovers_the_offset() {
        let mut est = OffsetEstimator::default();
        let offset = 5_000_000_i64;
        let latencies = [3_000, 800, 1_500, 200, 4_000, 900];
        let mut last = 0;
        for (i, lat) in latencies.iter().enumerate() {
            let device = 1_000_000 + i as u64 * 10_000;
            let receive = (device as i64 + offset + lat) as u64;
            last = est.observe(device, receive);
        }
        let device = 1_000_000 + 5 * 10_000;
        assert_eq!(last, (device as i64 + offset + 200) as u64);
    }

    #[test]
    fn capture_time_never_exceeds_receive_time() {
        let mut est = OffsetEstimator::default();
        assert_eq!(est.observe(100, 50), 50);
    }

    #[test]
    fn drift_and_uncertainty_are_estimated_from_evidence() {
        let mut est = OffsetEstimator::default();
        assert_eq!(est.uncertainty_us(), None);
        // A device clock 50 ppm slow, 100 Hz for 30 s, delays 0.3-2.3 ms.
        let mut last = None;
        for i in 0..3_000u64 {
            let session = 1_000_000 + i * 10_000;
            let device = (session as f64 * (1.0 - 50e-6)) as u64;
            let delay = 300 + (i * 7_919) % 2_000;
            last = Some(est.observe(device, session + delay));
            if i == 1_000 {
                assert_eq!(est.drift_ppm(), None, "10 s is not enough evidence");
            }
        }
        let drift = est.drift_ppm().unwrap();
        assert!((drift - 50.0).abs() < 5.0, "{drift} ppm");
        let u = est.uncertainty_us().unwrap();
        assert!(u < 400, "{u} us");
        // Mapped time is never later than arrival and within the minimum delay plus drift.
        let t = last.unwrap();
        assert!(t <= 1_000_000 + 2_999 * 10_000 + 2_300);
        let status = est.status();
        assert!(status.settled && status.offset_us.is_some() && status.resets == 0);
    }

    #[test]
    fn a_32_bit_device_clock_wraps_without_a_reset() {
        let mut w = Unwrap32::default();
        let mut est = OffsetEstimator::default();
        let start = u32::MAX - 50_000;
        let mut mapped = vec![];
        for i in 0..1_000u32 {
            let device = w.extend(start.wrapping_add(i * 10_000));
            mapped.push(est.observe(device, 10_000_000 + i as u64 * 10_000 + 500));
        }
        assert_eq!(est.resets(), 0);
        assert!(
            mapped.windows(2).all(|m| m[1] - m[0] == 10_000),
            "{mapped:?}"
        );
        // A reboot is a step back of seconds, not a wrap: the estimator resets.
        let device = w.extend(1_000);
        est.observe(device, 20_000_000);
        assert_eq!(est.resets(), 1);
    }

    #[test]
    fn device_reboot_resets_the_window() {
        let mut est = OffsetEstimator::default();
        for i in 0..20 {
            est.observe(10_000_000 + i * 10_000, 20_000_000 + i * 10_000);
        }
        assert!(est.settled());
        est.observe(1_000, 30_000_000);
        assert!(!est.settled());
    }
}
