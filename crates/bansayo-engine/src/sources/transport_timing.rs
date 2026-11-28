//! Socket-level BIMU packet timing, separate from downstream observation counters.
use serde::Serialize;
use std::collections::VecDeque;

const WINDOW: usize = 256;

#[derive(Debug, Clone, Serialize)]
pub struct TransportSnapshot {
    pub generation: u64,
    pub received: u64,
    pub missing: u64,
    pub rejected_order: u64,
    pub last_receive_us: u64,
    pub intervals_ms: Vec<[f64; 2]>,
}

#[derive(Debug, Default)]
pub struct TransportTiming {
    generation: u64,
    received: u64,
    missing: u64,
    rejected_order: u64,
    last: Option<(u64, u64, u64)>,
    intervals: VecDeque<[f64; 2]>,
}

impl TransportTiming {
    pub fn reset(&mut self, now: u64) {
        *self = Self {
            generation: now,
            ..Self::default()
        };
    }

    pub fn reject(&mut self) {
        self.rejected_order = self.rejected_order.saturating_add(1);
    }

    pub fn record(&mut self, seq: u64, device: u64, arrival: u64) {
        self.received = self.received.saturating_add(1);
        if let Some((previous, clock, receive)) = self.last {
            let delta = seq.saturating_sub(previous);
            self.missing = self.missing.saturating_add(delta.saturating_sub(1));
            if delta == 1 && device > clock && arrival >= receive {
                if self.intervals.len() == WINDOW {
                    self.intervals.pop_front();
                }
                self.intervals.push_back([
                    (device - clock) as f64 / 1000.0,
                    (arrival - receive) as f64 / 1000.0,
                ]);
            }
        }
        self.last = Some((seq, device, arrival));
    }

    pub fn snapshot(&self) -> TransportSnapshot {
        TransportSnapshot {
            generation: self.generation,
            received: self.received,
            missing: self.missing,
            rejected_order: self.rejected_order,
            last_receive_us: self.last.map_or(0, |(_, _, arrival)| arrival),
            intervals_ms: self.intervals.iter().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_datagrams_and_gaps_without_mixing_reference_epochs() {
        let mut timing = TransportTiming::default();
        timing.reset(100);
        timing.record(1, 1000, 2000);
        timing.record(2, 6000, 8000);
        timing.record(5, 21000, 23000);
        timing.reject();
        let s = timing.snapshot();
        assert_eq!(s.received, 3);
        assert_eq!(s.missing, 2);
        assert_eq!(s.rejected_order, 1);
        assert_eq!(s.intervals_ms, vec![[5.0, 6.0]]);
        timing.reset(30000);
        let s = timing.snapshot();
        assert_eq!(s.generation, 30000);
        assert_eq!(s.received, 0);
        assert_eq!(s.missing, 0);
        assert!(s.intervals_ms.is_empty());
    }
}
