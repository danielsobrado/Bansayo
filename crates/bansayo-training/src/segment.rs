//! Activity segmenter (07 §2), independent of any technique.
use crate::observables::{ObservableFrame, index};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmenterConfig {
    pub start_above: f64,
    pub start_hold_us: u64,
    pub end_below: f64,
    pub end_hold_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    /// Implied by `end_hold_us ≥ merge_gap_us`: a segment that rises again before its end is
    /// confirmed is kept open, which merges segments closer than the gap.
    pub merge_gap_us: u64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            start_above: 1.0,
            start_hold_us: 40_000,
            end_below: 0.4,
            end_hold_us: 150_000,
            min_us: 150_000,
            max_us: 4_000_000,
            merge_gap_us: 80_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentEvent {
    Start {
        t_us: u64,
    },
    End {
        start_us: u64,
        end_us: u64,
        too_long: bool,
        too_short: bool,
        peak: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    Rising {
        start: u64,
    },
    Active {
        start: u64,
    },
    Falling {
        start: u64,
        end: u64,
    },
    /// After a too-long segment, until activity drops below `end_below`.
    Cooling,
}

const DRIVERS: &[&str] = &[
    "ankle_speed.left",
    "ankle_speed.right",
    "knee_speed.left",
    "knee_speed.right",
    "wrist_speed.left",
    "wrist_speed.right",
    "elbow_speed.left",
    "elbow_speed.right",
    "root_speed",
];

/// Activity level: the maximum normalised speed of the available drivers, and its input time.
pub fn activity(frame: &ObservableFrame) -> Option<(f64, u64)> {
    DRIVERS
        .iter()
        .filter_map(|id| {
            let v = frame.get(index(id)?);
            Some((v.norm?, v.evidence.t_obs_us.unwrap_or(frame.t_us)))
        })
        .reduce(|a, b| (a.0.max(b.0), a.1.max(b.1)))
}

#[derive(Debug, Clone)]
pub struct Segmenter {
    cfg: SegmenterConfig,
    state: State,
    last: Option<(u64, f64)>,
    peak: f64,
}

fn cross(a: (u64, f64), b: (u64, f64), level: f64) -> u64 {
    if (b.1 - a.1).abs() < 1e-12 {
        return b.0;
    }
    let u = ((level - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
    a.0 + ((b.0 - a.0) as f64 * u).round() as u64
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            state: State::Idle,
            last: None,
            peak: 0.0,
        }
    }

    /// Start of the segment being opened or open, if any.
    pub fn open_since(&self) -> Option<u64> {
        match self.state {
            State::Rising { start } | State::Active { start } | State::Falling { start, .. } => {
                Some(start)
            }
            _ => None,
        }
    }

    pub fn active(&self) -> bool {
        matches!(self.state, State::Active { .. } | State::Falling { .. })
    }

    pub fn step(&mut self, frame: &ObservableFrame) -> Vec<SegmentEvent> {
        let (a, t) = match activity(frame) {
            Some(x) => x,
            // No input: nothing opens; an open segment sees zero activity.
            None => (0.0, frame.t_us),
        };
        if self.last.is_some_and(|l| t <= l.0) {
            return vec![];
        }
        let prev = self.last.unwrap_or((t, a));
        self.last = Some((t, a));
        let c = self.cfg;
        let mut out = vec![];
        self.state = match self.state {
            State::Idle | State::Rising { .. } if a < c.start_above => State::Idle,
            State::Idle => {
                self.peak = a;
                State::Rising {
                    start: cross(prev, (t, a), c.start_above),
                }
            }
            State::Rising { start } if t - start >= c.start_hold_us => {
                out.push(SegmentEvent::Start { t_us: start });
                State::Active { start }
            }
            s @ State::Rising { .. } => s,
            State::Active { start } | State::Falling { start, .. } if t - start > c.max_us => {
                out.push(SegmentEvent::End {
                    start_us: start,
                    end_us: t,
                    too_long: true,
                    too_short: false,
                    peak: self.peak,
                });
                State::Cooling
            }
            State::Active { start } if a < c.end_below => State::Falling {
                start,
                end: cross(prev, (t, a), c.end_below),
            },
            s @ State::Active { .. } => s,
            State::Falling { start, .. } if a >= c.end_below => State::Active { start },
            State::Falling { start, end } if t - end >= c.end_hold_us => {
                out.push(SegmentEvent::End {
                    start_us: start,
                    end_us: end,
                    too_long: false,
                    too_short: end - start < c.min_us,
                    peak: self.peak,
                });
                State::Idle
            }
            s @ State::Falling { .. } => s,
            State::Cooling if a < c.end_below => State::Idle,
            State::Cooling => State::Cooling,
        };
        self.peak = self.peak.max(a);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observables::ids;
    use bansayo_contracts::ObservableValue;

    fn frame(t: u64, a: Option<f64>) -> ObservableFrame {
        ObservableFrame {
            t_us: t,
            values: ids()
                .iter()
                .map(|(id, _)| ObservableValue {
                    id: id.clone(),
                    value: a.filter(|_| id == "ankle_speed.left"),
                    norm: a.filter(|_| id == "ankle_speed.left"),
                    evidence: bansayo_contracts::Evidence {
                        t_obs_us: Some(t),
                        ..crate::evidence::unavailable()
                    },
                    fresh: true,
                    reasons: vec![],
                })
                .collect(),
        }
    }

    fn run(levels: &[(u64, f64)]) -> Vec<SegmentEvent> {
        let mut s = Segmenter::new(SegmenterConfig::default());
        levels
            .iter()
            .flat_map(|(t, a)| s.step(&frame(*t, Some(*a))))
            .collect()
    }

    fn profile(on_ms: u64, total_ms: u64) -> Vec<(u64, f64)> {
        (0..total_ms / 10)
            .map(|k| {
                (
                    k * 10_000,
                    if k * 10 >= 100 && k * 10 < 100 + on_ms {
                        2.0
                    } else {
                        0.0
                    },
                )
            })
            .collect()
    }

    #[test]
    fn opens_after_the_start_hold_and_closes_after_the_end_hold() {
        let ev = run(&profile(500, 1500));
        let start = 95_000; // interpolated between 90 ms (0) and 100 ms (2.0) at level 1.0
        assert_eq!(ev[0], SegmentEvent::Start { t_us: start });
        match ev[1] {
            SegmentEvent::End {
                start_us,
                end_us,
                too_long,
                too_short,
                ..
            } => {
                assert_eq!(start_us, start);
                assert_eq!(end_us, 600_000 - 10_000 + 8_000);
                assert!(!too_long && !too_short);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn short_blips_long_segments_and_missing_inputs() {
        assert!(run(&profile(30, 500)).is_empty());
        let ev = run(&profile(100, 600));
        assert!(matches!(
            ev[1],
            SegmentEvent::End {
                too_short: true,
                ..
            }
        ));
        let ev = run(&profile(5000, 6000));
        assert!(matches!(ev[1], SegmentEvent::End { too_long: true, .. }));
        assert_eq!(ev.len(), 2, "no reopening while cooling");
        let mut s = Segmenter::new(SegmenterConfig::default());
        assert!(
            (0..100)
                .flat_map(|k| s.step(&frame(k * 10_000, None)))
                .next()
                .is_none()
        );
    }

    #[test]
    fn a_brief_dip_does_not_split_a_segment() {
        let mut levels = profile(600, 1500);
        for l in levels
            .iter_mut()
            .filter(|l| (300_000..360_000).contains(&l.0))
        {
            l.1 = 0.0;
        }
        let ev = run(&levels);
        assert_eq!(ev.len(), 2);
    }
}
