//! Periodic ticks from a plain thread.
//!
//! Tokio's timer on Windows wakes only about every 15.6 ms, which caps a 100 Hz source at
//! 64 Hz. `std::thread::sleep` is high-resolution there, and a channel send wakes the
//! receiving task at once.

use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, error::TrySendError};

/// Ticks every `period`. A tick the receiver has not taken yet is skipped, not queued.
/// The thread ends when the receiver is dropped.
pub fn ticker(period: Duration) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel(1);
    std::thread::spawn(move || {
        let mut next = Instant::now() + period;
        loop {
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
            if let Err(TrySendError::Closed(())) = tx.try_send(()) {
                break;
            }
            // Keep the average rate; after a stall resume from now instead of bursting.
            next = (next + period).max(Instant::now());
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ticks_near_the_nominal_rate_and_stops_when_dropped() {
        let mut ticks = ticker(Duration::from_millis(5));
        let started = Instant::now();
        for _ in 0..40 {
            ticks.recv().await;
        }
        // 40 ticks are nominally 200 ms; a 15.6 ms timer would need over 600 ms.
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(190), "{elapsed:?}");
        assert!(elapsed < Duration::from_millis(400), "{elapsed:?}");
    }
}
