//! Per-stage latency instrumentation (martial-arts 15 §4.1): always compiled, off unless
//! `diagnostics.timing` is on. Stages record durations on the session clock; every 10 s the
//! percentiles go to the `diagnostics` topic, and a stage whose p95 stays over its budget for 5 s
//! raises a notice naming it. The summary is logged at shutdown.
use bansayo_contracts::{DeviceId, StageTiming};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub const WINDOW_US: u64 = 10_000_000;
pub const OVER_BUDGET_US: u64 = 5_000_000;

/// The stages of the camera path (15 §2.1) and their budgets at 60 fps, microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Frame arrival → decoded image (MJPEG decode, or image file read for image folders).
    Decode,
    /// Decoded image → landmarks (subject box and pose inference).
    Inference,
    Fiducial,
    /// Landmarks ready → the solver tick that uses them.
    TickWait,
    /// Solver stages, `MotionSample` and planar bodies.
    Solve,
    /// Training runner: observables → recognition → metrics → exercise → coaching.
    Training,
    /// Serialising the frame for every client.
    Publish,
}

impl Stage {
    pub const ALL: [Stage; 7] = [
        Stage::Decode,
        Stage::Inference,
        Stage::Fiducial,
        Stage::TickWait,
        Stage::Solve,
        Stage::Training,
        Stage::Publish,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Stage::Decode => "decode",
            Stage::Inference => "inference",
            Stage::Fiducial => "fiducial",
            Stage::TickWait => "tick_wait",
            Stage::Solve => "solve",
            Stage::Training => "training",
            Stage::Publish => "publish",
        }
    }
    /// 15 §2.1; the tick wait is one solver period at the configured rate.
    pub fn budget_us(self, tick_period_us: u64) -> u64 {
        match self {
            Stage::Decode => 4_000,
            Stage::Inference => 12_000,
            Stage::Fiducial => 8_000,
            Stage::TickWait => tick_period_us,
            Stage::Solve => 2_000,
            Stage::Training => 1_000,
            Stage::Publish => 2_000,
        }
    }
}

#[derive(Default)]
struct Inner {
    samples: BTreeMap<Stage, VecDeque<(u64, u64)>>,
    /// When each camera's newest landmarks were ready, for the tick-wait stage.
    ready: BTreeMap<DeviceId, u64>,
    /// Stages currently reported over budget, so a notice is raised once per episode.
    noticed: Vec<Stage>,
}

pub struct Timing {
    on: AtomicBool,
    tick_period_us: u64,
    inner: Mutex<Inner>,
}

pub type Timings = Arc<Timing>;

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

impl Timing {
    pub fn new(on: bool, tick_period_us: u64) -> Timings {
        Arc::new(Self {
            on: AtomicBool::new(on),
            tick_period_us,
            inner: Mutex::default(),
        })
    }

    /// Whether stages should measure at all; checked before taking any timestamp.
    pub fn on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    pub fn record(&self, stage: Stage, t_us: u64, duration_us: u64) {
        if !self.on() {
            return;
        }
        if let Ok(mut i) = self.inner.lock() {
            let q = i.samples.entry(stage).or_default();
            q.push_back((t_us, duration_us));
            while q
                .front()
                .is_some_and(|(t, _)| t_us.saturating_sub(*t) > WINDOW_US)
            {
                q.pop_front();
            }
        }
    }

    /// A camera's landmarks became ready at `t_us`.
    pub fn ready(&self, camera: &DeviceId, t_us: u64) {
        if self.on()
            && let Ok(mut i) = self.inner.lock()
        {
            i.ready.insert(camera.clone(), t_us);
        }
    }

    /// At a solver tick: the wait of every camera's landmarks that became ready since the last.
    pub fn tick(&self, now_us: u64) {
        if !self.on() {
            return;
        }
        let ready: Vec<u64> = match self.inner.lock() {
            Ok(mut i) => std::mem::take(&mut i.ready).into_values().collect(),
            Err(_) => return,
        };
        for t in ready {
            self.record(Stage::TickWait, now_us, now_us.saturating_sub(t));
        }
    }

    fn stats(&self, now_us: u64, window_us: u64) -> Vec<StageTiming> {
        let Ok(i) = self.inner.lock() else {
            return vec![];
        };
        Stage::ALL
            .iter()
            .filter_map(|s| {
                let mut d: Vec<u64> = i
                    .samples
                    .get(s)?
                    .iter()
                    .filter(|(t, _)| now_us.saturating_sub(*t) <= window_us)
                    .map(|(_, d)| *d)
                    .collect();
                if d.is_empty() {
                    return None;
                }
                d.sort_unstable();
                Some(StageTiming {
                    stage: s.name().into(),
                    p50_us: percentile(&d, 0.50),
                    p95_us: percentile(&d, 0.95),
                    p99_us: percentile(&d, 0.99),
                    max_us: *d.last().unwrap_or(&0),
                    count: d.len() as u64,
                    budget_us: s.budget_us(self.tick_period_us),
                })
            })
            .collect()
    }

    /// Percentiles over the last 10 s.
    pub fn summary(&self, now_us: u64) -> Vec<StageTiming> {
        self.stats(now_us, WINDOW_US)
    }

    /// Stages whose p95 over the last 5 s exceeds their budget and were not reported yet; a
    /// stage back under budget can be reported again later.
    pub fn newly_over_budget(&self, now_us: u64) -> Vec<StageTiming> {
        let over: Vec<StageTiming> = self
            .stats(now_us, OVER_BUDGET_US)
            .into_iter()
            .filter(|s| s.p95_us > s.budget_us)
            .collect();
        let Ok(mut i) = self.inner.lock() else {
            return vec![];
        };
        let names: Vec<&str> = over.iter().map(|s| s.stage.as_str()).collect();
        i.noticed.retain(|s| names.contains(&s.name()));
        let mut out = vec![];
        for s in over {
            let stage = Stage::ALL.into_iter().find(|x| x.name() == s.stage);
            if let Some(stage) = stage
                && !i.noticed.contains(&stage)
            {
                i.noticed.push(stage);
                out.push(s);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_window_and_budget_notices() {
        let t = Timing::new(true, 16_667);
        for k in 0..100u64 {
            t.record(Stage::Decode, k * 100_000, 1_000 + k);
            t.record(Stage::Solve, k * 100_000, 5_000);
        }
        let s = t.summary(9_900_000);
        let decode = s.iter().find(|x| x.stage == "decode").unwrap();
        assert_eq!(
            (decode.count, decode.max_us, decode.budget_us),
            (100, 1_099, 4_000)
        );
        assert_eq!(decode.p50_us, 1_050);
        // Solve is at 5 ms against a 2 ms budget: one notice, then quiet while it stays over.
        let over = t.newly_over_budget(9_900_000);
        assert_eq!(
            over.iter().map(|x| x.stage.as_str()).collect::<Vec<_>>(),
            ["solve"]
        );
        assert!(t.newly_over_budget(9_900_000).is_empty());
        // Old samples leave the window.
        t.record(Stage::Decode, 30_000_000, 2_000);
        let s = t.summary(30_000_000);
        assert_eq!(s.iter().find(|x| x.stage == "decode").unwrap().count, 1);
        // Tick wait from ready instants.
        t.ready(&DeviceId::new("CAM").unwrap(), 29_990_000);
        t.tick(30_000_000);
        let wait = t.summary(30_000_000);
        assert_eq!(
            wait.iter().find(|x| x.stage == "tick_wait").unwrap().max_us,
            10_000
        );
        // Off: nothing is recorded.
        let off = Timing::new(false, 16_667);
        off.record(Stage::Decode, 1, 1);
        assert!(off.summary(1).is_empty());
    }
}
