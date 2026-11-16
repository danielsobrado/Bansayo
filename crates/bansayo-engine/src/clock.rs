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

/// One-way offset estimate: `offset = min(receive_time − device_time)` over a sliding window.
/// Jitter only adds delay, so the minimum tracks the true offset plus the smallest latency.
#[derive(Debug, Clone, Default)]
pub struct OffsetEstimator {
    window: VecDeque<i64>,
    last_device_us: Option<u64>,
}

impl OffsetEstimator {
    /// Adds a sample and returns the session-clock capture time of `device_us`.
    pub fn observe(&mut self, device_us: u64, receive_us: TimeUs) -> TimeUs {
        if self
            .last_device_us
            .is_some_and(|last| (device_us as i64) < last as i64 - DEVICE_CLOCK_JUMP_US)
        {
            self.window.clear();
        }
        self.last_device_us = Some(device_us);
        if self.window.len() == OFFSET_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(receive_us as i64 - device_us as i64);
        let offset = self.window.iter().copied().min().unwrap_or(0);
        u64::try_from(device_us as i64 + offset)
            .unwrap_or(0)
            .min(receive_us)
    }

    /// Whether enough samples have been seen for the estimate to be trusted.
    pub fn settled(&self) -> bool {
        self.window.len() >= OFFSET_SETTLE_SAMPLES
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
