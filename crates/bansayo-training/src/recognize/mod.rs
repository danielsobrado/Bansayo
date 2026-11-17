//! Recognition: segmenter + bank of phase machines + closed-set arbiter (07 §1–3).
pub mod machine;

use crate::content::compile::CompiledTechnique;
use crate::observables::ObservableFrame;
use crate::segment::{SegmentEvent, Segmenter, SegmenterConfig};
use machine::{History, Machine, MachineEvent, Rest};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The expected technique on the expected side.
    Expected,
    /// The expected technique on the other side (`wrong_side`).
    Mirror,
    /// A `confusable_with` definition run as a distractor.
    Distractor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Incomplete,
    RejectedQuality,
    WrongSide,
    /// No accepted candidate; `unknown` of 07 §3.3, counted as an extra movement.
    Unknown,
    Ambiguous,
    /// Activity with no machine leaving idle.
    Extra,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub reference: String,
    pub side: bansayo_contracts::Side,
    pub confidence: f64,
}

/// A recognised (or rejected) movement, after its activity segment closed.
#[derive(Debug, Clone)]
pub struct Finalized {
    pub outcome: Outcome,
    pub tech: Arc<CompiledTechnique>,
    pub rep_start: u64,
    pub rep_end: u64,
    /// Entered phases `(index, start)`; each ends where the next starts, the last at `rep_end`.
    pub phases: Vec<(usize, u64)>,
    pub strike: Option<(u64, f64)>,
    pub confidence: f64,
    pub reached: Option<usize>,
    pub candidates: Vec<Candidate>,
    pub segment: (u64, u64),
    pub peak_activity: f64,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecognitionEvent {
    ActivityStart { t_us: u64 },
    ActivityEnd { t_us: u64 },
    RepStarted { t_us: u64 },
    PhaseEnter { phase: String, t_us: u64 },
    Strike { t_us: u64, value: f64 },
}

#[derive(Debug, Clone)]
pub enum RecognitionOutput {
    Event(RecognitionEvent),
    Result(Box<Finalized>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecognizerConfig {
    pub segmenter: SegmenterConfig,
    /// `recognition.accept_confidence.guided` (default 0.5).
    pub accept_confidence: f64,
    pub ambiguity_margin: f64,
    pub tempo_scale: f64,
}

impl Default for RecognizerConfig {
    fn default() -> Self {
        Self {
            segmenter: SegmenterConfig::default(),
            accept_confidence: 0.5,
            ambiguity_margin: 0.10,
            tempo_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Recognizer {
    cfg: RecognizerConfig,
    segmenter: Segmenter,
    bank: Vec<(Role, Machine)>,
    history: History,
    rest: Rest,
}

impl Recognizer {
    /// The bank: expected first, then its mirror side, then distractors.
    pub fn new(cfg: RecognizerConfig, bank: Vec<(Role, Arc<CompiledTechnique>)>) -> Self {
        Self {
            segmenter: Segmenter::new(cfg.segmenter),
            bank: bank
                .into_iter()
                .map(|(r, t)| (r, Machine::new(t, cfg.tempo_scale)))
                .collect(),
            cfg,
            history: History::default(),
            rest: Rest::default(),
        }
    }

    pub fn rest(&self) -> &Rest {
        &self.rest
    }

    /// The phase the expected machine is in, if any.
    pub fn expected_phase(&self) -> Option<String> {
        let (_, m) = self.bank.iter().find(|(r, _)| *r == Role::Expected)?;
        if m.finished() {
            return None;
        }
        m.current().map(|i| m.tech.phases[i].id.clone())
    }

    pub fn in_repetition(&self) -> bool {
        self.bank
            .iter()
            .any(|(r, m)| *r == Role::Expected && !m.idle() && !m.finished())
    }

    pub fn step(&mut self, frame: &ObservableFrame) -> Vec<RecognitionOutput> {
        self.history.push(frame);
        let events = self.segmenter.step(frame);
        if self.segmenter.open_since().is_none() && self.bank.iter().all(|(_, m)| m.idle()) {
            self.rest.capture(frame);
        }
        let open = self.segmenter.open_since();
        let mut out = vec![];
        for e in &events {
            if let SegmentEvent::Start { t_us } = e {
                out.push(RecognitionOutput::Event(RecognitionEvent::ActivityStart {
                    t_us: *t_us,
                }));
            }
        }
        for (role, m) in &mut self.bank {
            for e in m.step(frame, &self.history, &self.rest, open) {
                if *role != Role::Expected {
                    continue;
                }
                let ev = match e {
                    MachineEvent::Started { rep_start } => {
                        RecognitionEvent::RepStarted { t_us: rep_start }
                    }
                    MachineEvent::PhaseEnter { phase, t_us } => RecognitionEvent::PhaseEnter {
                        phase: m.tech.phases[phase].id.clone(),
                        t_us,
                    },
                    MachineEvent::Strike { t_us, value } => {
                        RecognitionEvent::Strike { t_us, value }
                    }
                    _ => continue,
                };
                out.push(RecognitionOutput::Event(ev));
            }
        }
        for e in events {
            if let SegmentEvent::End {
                start_us,
                end_us,
                too_long,
                too_short,
                peak,
            } = e
            {
                out.push(RecognitionOutput::Event(RecognitionEvent::ActivityEnd {
                    t_us: end_us,
                }));
                if let Some(f) = self.arbitrate((start_us, end_us), too_long, too_short, peak) {
                    out.push(RecognitionOutput::Result(Box::new(f)));
                }
                for (_, m) in &mut self.bank {
                    m.reset();
                }
            }
        }
        out
    }

    fn arbitrate(
        &mut self,
        segment: (u64, u64),
        too_long: bool,
        too_short: bool,
        peak: f64,
    ) -> Option<Finalized> {
        for (_, m) in &mut self.bank {
            m.segment_ended();
        }
        let expected = self
            .bank
            .iter()
            .find(|(r, _)| *r == Role::Expected)?
            .1
            .clone();
        let finalize = |m: &Machine, outcome: Outcome| {
            let rep_start = m.rep_start.unwrap_or(segment.0);
            let rep_end = m
                .completed
                .map_or(segment.1, |c| c.max(segment.1))
                .min(rep_start + m.tech.max_us);
            Finalized {
                outcome,
                tech: m.tech.clone(),
                rep_start,
                rep_end,
                phases: m.entered.clone(),
                strike: m.strike,
                confidence: if m.completed.is_some() {
                    m.confidence(rep_end)
                } else {
                    0.0
                },
                reached: m.current(),
                candidates: vec![],
                segment,
                peak_activity: peak,
                reason: None,
            }
        };
        let mut done: Vec<(Role, &Machine, f64)> = self
            .bank
            .iter()
            .filter(|(_, m)| m.completed.is_some())
            .map(|(r, m)| (*r, m, finalize(m, Outcome::Completed).confidence))
            .collect();
        done.sort_by(|a, b| {
            b.1.mandatory_phases()
                .cmp(&a.1.mandatory_phases())
                .then(b.2.total_cmp(&a.2))
        });
        let candidates: Vec<Candidate> = done
            .iter()
            .map(|(_, m, c)| Candidate {
                reference: m.tech.reference.clone(),
                side: m.tech.side,
                confidence: *c,
            })
            .collect();
        let with = |mut f: Finalized, reason: Option<&'static str>| {
            f.candidates = candidates.clone();
            f.reason = reason;
            Some(f)
        };
        if too_long {
            return with(finalize(&expected, Outcome::Unknown), Some("too_long"));
        }
        let Some((role, best, conf)) = done.first().copied() else {
            let started = self.bank.iter().any(|(_, m)| !m.idle());
            return if !expected.idle() {
                with(finalize(&expected, Outcome::Incomplete), None)
            } else if started || !too_short {
                with(finalize(&expected, Outcome::Extra), None)
            } else {
                None
            };
        };
        if let Some((_, _, second)) = done.get(1)
            && conf - second < self.cfg.ambiguity_margin
        {
            return with(finalize(best, Outcome::Ambiguous), None);
        }
        if conf < self.cfg.accept_confidence {
            return with(finalize(best, Outcome::Unknown), Some("low_confidence"));
        }
        let outcome = match role {
            Role::Expected if best.quality_gap => Outcome::RejectedQuality,
            Role::Expected => Outcome::Completed,
            Role::Mirror => Outcome::WrongSide,
            Role::Distractor => Outcome::Unknown,
        };
        with(finalize(best, outcome), None)
    }
}

/// Re-runs a machine over re-filtered frames (zero-phase, 04 §6.3) so stored phase boundaries
/// and anchors carry no filter lag. `None` when the re-run does not complete.
pub fn rerun(
    tech: Arc<CompiledTechnique>,
    frames: &[ObservableFrame],
    rest: &Rest,
    segment: (u64, u64),
    tempo_scale: f64,
) -> Option<Finalized> {
    let mut m = Machine::new(tech, tempo_scale);
    let mut history = History::default();
    for f in frames {
        history.push(f);
        let open = (f.t_us >= segment.0).then_some(segment.0);
        m.step(f, &history, rest, open);
        if m.completed.is_some() && m.strike.is_some() {
            break;
        }
    }
    let complete = m.completed?;
    let rep_start = m.rep_start?;
    let rep_end = complete.max(segment.1).min(rep_start + m.tech.max_us);
    Some(Finalized {
        outcome: Outcome::Completed,
        confidence: m.confidence(rep_end),
        tech: m.tech.clone(),
        rep_start,
        rep_end,
        phases: m.entered.clone(),
        strike: m.strike,
        reached: m.current(),
        candidates: vec![],
        segment,
        peak_activity: 0.0,
        reason: None,
    })
}

#[cfg(test)]
mod tests;
