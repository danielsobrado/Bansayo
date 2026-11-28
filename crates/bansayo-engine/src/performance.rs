//! Low-rate, opt-in resource sampling outside the tracking pipeline.
use crate::{clock::SessionClock, diagnostics::Timings, process_resources};
use bansayo_contracts::StageTiming;
use serde::Serialize;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const SAMPLE_PERIOD: Duration = Duration::from_secs(2);
const MAX_SAMPLES: usize = 300;

#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    t_us: u64,
    cpu_percent_one_core: Option<f64>,
    resident_bytes: Option<u64>,
}

#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub timing_enabled: bool,
    pub epoch: String,
    pub samples: VecDeque<Sample>,
    pub stages: Vec<StageTiming>,
}

fn cpu_percent(previous: f64, current: f64, seconds: f64) -> Option<f64> {
    let value = (current - previous) / seconds * 100.0;
    (seconds > 0.0 && value.is_finite() && value >= 0.0).then_some(value)
}

type Worker = tokio::task::JoinHandle<Result<(), crate::app::EngineError>>;

pub fn spawn(
    enabled: bool,
    timing: Timings,
    clock: SessionClock,
    cancel: CancellationToken,
) -> (watch::Receiver<Arc<Snapshot>>, Worker) {
    let mut snapshot = Snapshot {
        enabled,
        timing_enabled: timing.on(),
        epoch: clock.epoch_utc().into(),
        samples: VecDeque::new(),
        stages: vec![],
    };
    let (tx, rx) = watch::channel(Arc::new(snapshot.clone()));
    let task = tokio::spawn(async move {
        if !enabled {
            return Ok(());
        }
        let mut interval = tokio::time::interval(SAMPLE_PERIOD);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut prior: Option<(u64, f64)> = None;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {}
            }
            let counters = tokio::task::spawn_blocking(process_resources::read).await?;
            let now = clock.now_us();
            let cpu = prior.and_then(|(t, value)| {
                cpu_percent(
                    value,
                    counters.cpu_seconds?,
                    now.saturating_sub(t) as f64 / 1e6,
                )
            });
            prior = counters.cpu_seconds.map(|v| (now, v));
            if snapshot.samples.len() == MAX_SAMPLES {
                snapshot.samples.pop_front();
            }
            snapshot.samples.push_back(Sample {
                t_us: now,
                cpu_percent_one_core: cpu,
                resident_bytes: counters.resident_bytes,
            });
            snapshot.stages = timing.summary(now);
            tx.send_replace(Arc::new(snapshot.clone()));
        }
        Ok(())
    });
    (rx, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_usage_can_exceed_one_core_and_rejects_invalid_deltas() {
        assert_eq!(cpu_percent(1.0, 5.0, 2.0), Some(200.0));
        assert_eq!(cpu_percent(5.0, 1.0, 2.0), None);
        assert_eq!(cpu_percent(1.0, 2.0, 0.0), None);
    }
}
