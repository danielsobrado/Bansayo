//! Periodic and deadline-based wake-ups from dedicated threads.
//!
//! Tokio's timer on Windows wakes only about every 15.6 ms, which caps a 100 Hz source at
//! 64 Hz. `std::thread::sleep` is high-resolution there, and a channel/watch wakeup resumes the
//! async task at once.

use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    watch,
};

/// Deadlines longer than this wait coarsely via Tokio until [`COARSE_SLEEP_LEAD`] remains,
/// keeping idle sources completely asleep without holding a long thread sleep.
pub const COARSE_SLEEP_THRESHOLD: Duration = Duration::from_millis(35);
/// Remaining window handed to the high-resolution `std::thread::sleep` worker.
pub const COARSE_SLEEP_LEAD: Duration = Duration::from_millis(25);

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

struct DeadlineState {
    armed: Option<(u64, Instant)>,
    closed: bool,
}

/// Reusable high-resolution deadline sleeper backed by one worker thread per source.
pub struct DeadlineTimer {
    state: Arc<(Mutex<DeadlineState>, Condvar)>,
    done_rx: watch::Receiver<u64>,
    generation: u64,
    handle: Option<JoinHandle<()>>,
}

impl Default for DeadlineTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadlineTimer {
    pub fn new() -> Self {
        let state = Arc::new((
            Mutex::new(DeadlineState {
                armed: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let (done_tx, done_rx) = watch::channel(0_u64);
        let worker_state = state.clone();
        let handle = std::thread::Builder::new()
            .name("bansayo-sim-deadline".into())
            .spawn(move || {
                loop {
                    let (generation, target) = {
                        let (lock, cvar) = &*worker_state;
                        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if guard.closed {
                                return;
                            }
                            if let Some(pair) = guard.armed {
                                break pair;
                            }
                            guard = cvar.wait(guard).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let dur = target.saturating_duration_since(Instant::now());
                    if !dur.is_zero() {
                        std::thread::sleep(dur);
                    }
                    let (lock, _) = &*worker_state;
                    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.armed.is_some_and(|(g, _)| g == generation) {
                        guard.armed = None;
                        let _ = done_tx.send(generation);
                    }
                }
            })
            .ok();
        Self {
            state,
            done_rx,
            generation: 0,
            handle,
        }
    }

    fn signal_close(&mut self) {
        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        guard.closed = true;
        guard.armed = None;
        cvar.notify_one();
    }

    /// Waits until `target`, using Tokio for long idle gaps and `std::thread::sleep` for the
    /// final high-resolution interval. Cancel-safe inside `tokio::select!`.
    pub async fn wait_until(&mut self, target: Instant) {
        loop {
            let remaining = target.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return;
            }
            if remaining > COARSE_SLEEP_THRESHOLD || self.handle.is_none() {
                let slice = if self.handle.is_none() {
                    remaining
                } else {
                    remaining - COARSE_SLEEP_LEAD
                };
                tokio::time::sleep(slice).await;
                continue;
            }
            self.generation += 1;
            let generation = self.generation;
            {
                let (lock, cvar) = &*self.state;
                let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                guard.armed = Some((generation, target));
                cvar.notify_one();
            }
            struct Disarm<'a> {
                state: &'a (Mutex<DeadlineState>, Condvar),
                generation: u64,
            }
            impl Drop for Disarm<'_> {
                fn drop(&mut self) {
                    let (lock, _) = self.state;
                    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.armed.is_some_and(|(g, _)| g == self.generation) {
                        guard.armed = None;
                    }
                }
            }
            let _disarm = Disarm {
                state: &self.state,
                generation,
            };
            let _ = self.done_rx.wait_for(|&done| done >= generation).await;
            return;
        }
    }

    #[cfg(test)]
    pub fn stop_and_join(mut self) {
        self.signal_close();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DeadlineTimer {
    fn drop(&mut self) {
        self.signal_close();
        let _ = self.handle.take();
    }
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

    #[tokio::test]
    async fn deadline_timer_wakes_near_5ms_deadlines_and_stops_cleanly() {
        let mut timer = DeadlineTimer::new();
        let started = Instant::now();
        let mut next = started + Duration::from_millis(5);
        for _ in 0..40 {
            timer.wait_until(next).await;
            next = (next + Duration::from_millis(5)).max(Instant::now());
        }
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(190), "{elapsed:?}");
        assert!(elapsed < Duration::from_millis(400), "{elapsed:?}");
        timer.stop_and_join();
    }

    #[tokio::test]
    async fn deadline_timer_handles_coarse_plus_fine_deadlines() {
        let mut timer = DeadlineTimer::new();
        let started = Instant::now();
        timer.wait_until(started + Duration::from_millis(70)).await;
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(65), "{elapsed:?}");
        assert!(elapsed < Duration::from_millis(200), "{elapsed:?}");
        timer.stop_and_join();
    }
}
