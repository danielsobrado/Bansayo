//! Stage-1 phase machine compiled from a technique definition (05 §6, 07 §3.1–3.2).
use crate::content::compile::{CAtom, CCondition, CompiledTechnique, Obs};
use crate::content::types::{Op, RefPoint};
use crate::observables::ObservableFrame;
use bansayo_contracts::MethodId;
use std::collections::VecDeque;

/// Recent observable values for reference lookups (`ref: rep_start | phase_start | rest`).
#[derive(Debug, Clone, Default)]
pub struct History {
    /// `(t_us, [(value, norm)])` per tick, oldest first.
    frames: VecDeque<(u64, Vec<(Option<f64>, Option<f64>)>)>,
}

const HISTORY_SPAN_US: u64 = 6_000_000;

impl History {
    pub fn push(&mut self, f: &ObservableFrame) {
        if self.frames.back().is_some_and(|b| b.0 >= f.t_us) {
            return;
        }
        self.frames
            .push_back((f.t_us, f.values.iter().map(|v| (v.value, v.norm)).collect()));
        while self
            .frames
            .front()
            .is_some_and(|x| f.t_us.saturating_sub(x.0) > HISTORY_SPAN_US)
        {
            self.frames.pop_front();
        }
    }

    /// The newest stored value at or before `t`.
    pub fn at(&self, t: u64, obs: &Obs) -> Option<f64> {
        let i = obs.index?;
        let (_, row) = self.frames.iter().rev().find(|(ft, _)| *ft <= t)?;
        let (v, n) = row[i];
        if obs.norm { n } else { v }
    }
}

/// Rest values captured while no activity segment is open (05 §6 `ref: rest`).
#[derive(Debug, Clone, Default)]
pub struct Rest {
    pub values: Vec<(Option<f64>, Option<f64>)>,
}

impl Rest {
    pub fn capture(&mut self, f: &ObservableFrame) {
        self.values = f.values.iter().map(|v| (v.value, v.norm)).collect();
    }

    fn get(&self, obs: &Obs) -> Option<f64> {
        let (v, n) = *self.values.get(obs.index?)?;
        if obs.norm { n } else { v }
    }
}

fn raw(frame: &ObservableFrame, obs: &Obs) -> Option<(f64, u64, bool)> {
    let v = frame.get(obs.index?);
    let x = if obs.norm { v.norm } else { v.value }?;
    Some((
        x * obs.sign,
        v.evidence.t_obs_us.unwrap_or(frame.t_us),
        v.fresh,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tri {
    True(u64),
    False,
    Unknown,
}

#[derive(Debug, Clone, Default)]
struct Peak {
    best: Option<(u64, f64)>,
    before: Option<(u64, f64)>,
    after: Option<(u64, f64)>,
    lower: u32,
    floor: Option<f64>,
}

impl Peak {
    /// Feeds a sample of `x` (already sign-adjusted so that a maximum is sought).
    fn feed(&mut self, t: u64, x: f64, last: Option<(u64, f64)>) {
        self.floor = Some(self.floor.map_or(x, |f| f.min(x)));
        match self.best {
            Some((_, b)) if x <= b => {
                self.lower += 1;
                if self.lower == 1 {
                    self.after = Some((t, x));
                }
            }
            _ => {
                self.best = Some((t, x));
                self.before = last;
                self.after = None;
                self.lower = 0;
            }
        }
    }

    /// `(vertex time, vertex value)` once two lower samples follow a rise of `prominence`.
    fn confirmed(&self, prominence: f64, floor_before: f64) -> Option<(u64, f64)> {
        let (t1, x1) = self.best?;
        if self.lower < 2 || x1 - floor_before < prominence {
            return None;
        }
        Some(match (self.before, self.after) {
            (Some(p), Some(n)) => vertex(p, (t1, x1), n),
            _ => (t1, x1),
        })
    }
}

/// Vertex of the parabola through three samples (04 §7), clamped to the outer two.
pub fn vertex(a: (u64, f64), b: (u64, f64), c: (u64, f64)) -> (u64, f64) {
    let (x0, x1, x2) = (a.0 as f64, b.0 as f64, c.0 as f64);
    let (y0, y1, y2) = (a.1, b.1, c.1);
    let d = (x0 - x1) * (x0 - x2) * (x1 - x2);
    if d.abs() < 1e-9 {
        return b;
    }
    let pa = (x2 * (y1 - y0) + x1 * (y0 - y2) + x0 * (y2 - y1)) / d;
    let pb = (x2 * x2 * (y0 - y1) + x1 * x1 * (y2 - y0) + x0 * x0 * (y1 - y2)) / d;
    if pa.abs() < 1e-18 {
        return b;
    }
    let tv = (-pb / (2.0 * pa)).clamp(x0, x2);
    let pc = y0 - pa * x0 * x0 - pb * x0;
    (tv.round() as u64, pa * tv * tv + pb * tv + pc)
}

fn cross(a: (u64, f64), b: (u64, f64), level: f64) -> u64 {
    if (b.1 - a.1).abs() < 1e-12 {
        return b.0;
    }
    let u = ((level - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
    a.0 + ((b.0 - a.0) as f64 * u).round() as u64
}

#[derive(Debug, Clone, Default)]
struct AtomState {
    prev: Option<(u64, f64)>,
    since: Option<u64>,
    latch: Option<u64>,
    peak: Peak,
    known: bool,
}

/// What a reference value is subtracted from.
pub struct Refs<'a> {
    pub history: &'a History,
    pub rest: &'a Rest,
    pub rep_start: Option<u64>,
    pub phase_start: Option<u64>,
}

impl Refs<'_> {
    fn value(&self, a: &CAtom) -> Option<f64> {
        let at = |t: Option<u64>| self.history.at(t?, &a.obs).map(|v| v * a.obs.sign);
        match a.reference {
            RefPoint::Absolute => Some(0.0),
            RefPoint::RepStart => at(self.rep_start),
            RefPoint::PhaseStart => at(self.phase_start),
            RefPoint::Rest => self.rest.get(&a.obs).map(|v| v * a.obs.sign),
        }
    }
}

impl AtomState {
    fn reset_latch(&mut self) {
        self.latch = None;
        self.peak = Peak::default();
    }

    fn update(&mut self, a: &CAtom, frame: &ObservableFrame, refs: &Refs) {
        let Some((x, t, fresh)) = raw(frame, &a.obs) else {
            self.known = false;
            self.since = None;
            self.prev = None;
            return;
        };
        let Some(r) = refs.value(a) else {
            self.known = false;
            return;
        };
        self.known = true;
        if !fresh && self.prev.is_some_and(|p| p.0 >= t) {
            return;
        }
        let x = x - r;
        let prev = self.prev;
        let p = prev.unwrap_or((t, x));
        match a.op {
            Op::Gt | Op::Lt | Op::Between | Op::ChangedBy => {
                let truth = |v: f64| match a.op {
                    Op::Gt => v > a.lo,
                    Op::Lt => v < a.lo,
                    Op::ChangedBy => v.abs() >= a.lo,
                    _ => v >= a.lo && v <= a.hi,
                };
                if truth(x) {
                    if self.since.is_none() {
                        let level = match a.op {
                            Op::Between if p.1 >= a.lo => a.hi,
                            Op::ChangedBy => a.lo * x.signum(),
                            _ => a.lo,
                        };
                        self.since = Some(if prev.is_some() && !truth(p.1) {
                            cross(p, (t, x), level)
                        } else {
                            t
                        });
                    }
                } else {
                    self.since = None;
                }
            }
            Op::RisesAbove | Op::FallsBelow => {
                let (now, before, level) = match a.op {
                    Op::RisesAbove => (x >= a.lo, p.1 >= a.lo, a.lo),
                    _ => (x <= a.lo, p.1 <= a.lo, a.lo),
                };
                if self.latch.is_none() && now && prev.is_some() && !before {
                    self.latch = Some(cross(p, (t, x), level));
                }
                if let Some(l) = self.latch
                    && !now
                    && t.saturating_sub(l) < a.hold_us
                {
                    self.latch = None;
                }
            }
            Op::LocalMax | Op::LocalMin => {
                let s = if a.op == Op::LocalMax { x } else { -x };
                let last = prev.map(|(pt, px)| (pt, if a.op == Op::LocalMax { px } else { -px }));
                self.peak.feed(t, s, last);
                if self.latch.is_none() {
                    let floor = self.peak.floor.unwrap_or(s);
                    if let Some((tv, _)) = self.peak.confirmed(a.prominence, floor) {
                        self.latch = Some(tv);
                    }
                }
            }
        }
        self.prev = Some((t, x));
    }

    fn eval(&self, a: &CAtom, now: u64) -> Tri {
        if !self.known {
            return Tri::Unknown;
        }
        match a.op {
            Op::Gt | Op::Lt | Op::Between | Op::ChangedBy => match self.since {
                Some(s) if now.saturating_sub(s) >= a.hold_us => Tri::True(s),
                _ => Tri::False,
            },
            _ => match self.latch {
                Some(l)
                    if now.saturating_sub(l) >= a.hold_us
                        || matches!(a.op, Op::LocalMax | Op::LocalMin) =>
                {
                    Tri::True(l)
                }
                _ => Tri::False,
            },
        }
    }
}

fn eval(c: &CCondition, atoms: &[CAtom], st: &[AtomState], now: u64) -> Tri {
    match c {
        CCondition::Atom(i) => st[*i].eval(&atoms[*i], now),
        CCondition::All(list) => {
            let mut t = 0;
            let mut unknown = false;
            for c in list {
                match eval(c, atoms, st, now) {
                    Tri::False => return Tri::False,
                    Tri::Unknown => unknown = true,
                    Tri::True(x) => t = t.max(x),
                }
            }
            if unknown { Tri::Unknown } else { Tri::True(t) }
        }
        CCondition::Any(list) => {
            let mut best: Option<u64> = None;
            let mut unknown = false;
            for c in list {
                match eval(c, atoms, st, now) {
                    Tri::True(x) => best = Some(best.map_or(x, |b| b.min(x))),
                    Tri::Unknown => unknown = true,
                    Tri::False => {}
                }
            }
            match (best, unknown) {
                (Some(t), _) => Tri::True(t),
                (None, true) => Tri::Unknown,
                (None, false) => Tri::False,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abort {
    PhaseTimeout,
    TooLong,
    SegmentEnded,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MachineEvent {
    Started { rep_start: u64 },
    PhaseEnter { phase: usize, t_us: u64 },
    Strike { t_us: u64, value: f64 },
    Completed { t_us: u64 },
    Aborted { why: Abort, reached: Option<usize> },
}

/// One machine per (definition, side).
#[derive(Debug, Clone)]
pub struct Machine {
    pub tech: std::sync::Arc<CompiledTechnique>,
    atoms: Vec<AtomState>,
    /// Entered phases `(index, t_us)`; empty while idle.
    pub entered: Vec<(usize, u64)>,
    pub rep_start: Option<u64>,
    pub completed: Option<u64>,
    pub aborted: Option<Abort>,
    pub strike: Option<(u64, f64)>,
    strike_peak: Peak,
    strike_floor: Option<f64>,
    strike_open: bool,
    last_strike: Option<(u64, f64)>,
    strike_reported: bool,
    pub min_confidence: f64,
    pub image_plane: bool,
    pub optional_known_skipped: bool,
    pub quality_gap: bool,
    gaps: Vec<Option<u64>>,
    /// Phase-time limits are divided by this (stage `slow`: the technique's `slow_scale`).
    pub tempo_scale: f64,
    last_mandatory: usize,
}

impl Machine {
    pub fn new(tech: std::sync::Arc<CompiledTechnique>, tempo_scale: f64) -> Self {
        let last_mandatory = tech.phases.iter().rposition(|p| !p.optional).unwrap_or(0);
        Self {
            atoms: vec![AtomState::default(); tech.atoms.len()],
            gaps: vec![None; tech.required.len()],
            tech,
            entered: vec![],
            rep_start: None,
            completed: None,
            aborted: None,
            strike: None,
            strike_peak: Peak::default(),
            strike_floor: None,
            strike_open: false,
            last_strike: None,
            strike_reported: false,
            min_confidence: 1.0,
            image_plane: false,
            optional_known_skipped: false,
            quality_gap: false,
            tempo_scale,
            last_mandatory,
        }
    }

    pub fn idle(&self) -> bool {
        self.entered.is_empty() && self.completed.is_none() && self.aborted.is_none()
    }

    pub fn finished(&self) -> bool {
        self.completed.is_some() || self.aborted.is_some()
    }

    pub fn current(&self) -> Option<usize> {
        self.entered.last().map(|e| e.0)
    }

    /// Back to idle for the next activity segment.
    pub fn reset(&mut self) {
        let fresh = Self::new(self.tech.clone(), self.tempo_scale);
        let atoms = std::mem::take(&mut self.atoms);
        *self = fresh;
        self.atoms = atoms;
        self.atoms.iter_mut().for_each(AtomState::reset_latch);
    }

    fn limit(&self, us: u64) -> u64 {
        (us as f64 / self.tempo_scale) as u64
    }

    pub fn step(
        &mut self,
        frame: &ObservableFrame,
        history: &History,
        rest: &Rest,
        segment_open: Option<u64>,
    ) -> Vec<MachineEvent> {
        let tech = self.tech.clone();
        let refs = Refs {
            history,
            rest,
            rep_start: self.rep_start.or(segment_open),
            phase_start: self.entered.last().map(|e| e.1).or(segment_open),
        };
        for (st, a) in self.atoms.iter_mut().zip(&tech.atoms) {
            st.update(a, frame, &refs);
        }
        let now = frame
            .values
            .iter()
            .filter_map(|v| v.evidence.t_obs_us)
            .max()
            .unwrap_or(frame.t_us);
        if self.finished() {
            self.track_strike(frame);
            return self.strike_event();
        }
        let mut out = vec![];
        let Some(n) = self.current() else {
            let Some(open) = segment_open else {
                self.atoms.iter_mut().for_each(AtomState::reset_latch);
                return out;
            };
            let driver = raw(frame, &tech.driver).map(|(x, ..)| x);
            if driver.is_some_and(|d| d >= tech.start_above)
                && let Tri::True(t) = eval(&tech.phases[0].enter, &tech.atoms, &self.atoms, now)
            {
                self.rep_start = Some(open.min(t));
                self.entered.push((0, t.max(open)));
                self.atoms.iter_mut().for_each(AtomState::reset_latch);
                self.strike_open = tech.strike.as_ref().is_some_and(|s| s.in_phase == 0);
                out.push(MachineEvent::Started {
                    rep_start: open.min(t),
                });
                out.push(MachineEvent::PhaseEnter {
                    phase: 0,
                    t_us: t.max(open),
                });
                self.observe(frame, now);
                self.track_strike(frame);
            }
            return out;
        };
        self.observe(frame, now);
        self.track_strike(frame);
        out.extend(self.strike_event());
        let entered_at = self.entered.last().map_or(now, |e| e.1);
        let rep_start = self.rep_start.unwrap_or(entered_at);
        if now.saturating_sub(entered_at) > self.limit(tech.phases[n].max_us) {
            return self.abort(Abort::PhaseTimeout, out);
        }
        if now.saturating_sub(rep_start) > self.limit(tech.max_us) {
            return self.abort(Abort::TooLong, out);
        }
        if now.saturating_sub(entered_at) >= tech.phases[n].min_us
            && let Some(next) = tech.phases.get(n + 1)
        {
            let first = eval(&next.enter, &tech.atoms, &self.atoms, now);
            let target = match first {
                Tri::True(t) => Some((n + 1, t)),
                _ if next.optional => match tech
                    .phases
                    .get(n + 2)
                    .map(|p| eval(&p.enter, &tech.atoms, &self.atoms, now))
                {
                    Some(Tri::True(t)) => {
                        if first == Tri::False {
                            self.optional_known_skipped = true;
                        }
                        Some((n + 2, t))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some((p, t)) = target {
                let t = t.max(entered_at);
                self.entered.push((p, t));
                self.atoms.iter_mut().for_each(AtomState::reset_latch);
                out.push(MachineEvent::PhaseEnter { phase: p, t_us: t });
                if p == tech.strike.as_ref().map_or(usize::MAX, |s| s.in_phase) {
                    self.strike_open = true;
                }
                return out;
            }
        }
        if n >= self.last_mandatory
            && let Tri::True(t) = eval(&tech.complete, &tech.atoms, &self.atoms, now)
        {
            if tech.phases.get(n + 1).is_some_and(|p| p.optional) {
                self.optional_known_skipped |=
                    eval(&tech.phases[n + 1].enter, &tech.atoms, &self.atoms, now) == Tri::False;
            }
            let t = t.max(entered_at);
            self.completed = Some(t);
            out.push(MachineEvent::Completed { t_us: t });
        }
        out
    }

    fn abort(&mut self, why: Abort, mut out: Vec<MachineEvent>) -> Vec<MachineEvent> {
        self.aborted = Some(why);
        out.push(MachineEvent::Aborted {
            why,
            reached: self.current(),
        });
        out
    }

    /// The activity segment closed at `end`. An unfinished repetition is incomplete.
    pub fn segment_ended(&mut self) -> Vec<MachineEvent> {
        if self.idle() || self.finished() {
            return vec![];
        }
        self.abort(Abort::SegmentEnded, vec![])
    }

    fn observe(&mut self, frame: &ObservableFrame, now: u64) {
        for (k, obs) in self.tech.required.iter().enumerate() {
            let v = obs.index.map(|i| frame.get(i));
            match v.filter(|v| v.value.is_some()) {
                Some(v) => {
                    self.gaps[k] = None;
                    if let Some(c) = v.evidence.confidence {
                        self.min_confidence = self.min_confidence.min(f64::from(c));
                    }
                    self.image_plane |= v.evidence.method == MethodId::ImagePlane;
                }
                None => {
                    let since = *self.gaps[k].get_or_insert(now);
                    if now.saturating_sub(since) > self.tech.max_gap_us {
                        self.quality_gap = true;
                    }
                }
            }
        }
    }

    fn track_strike(&mut self, frame: &ObservableFrame) {
        let Some(s) = self.tech.strike.as_ref() else {
            return;
        };
        if !self.strike_open || self.strike.is_some() {
            return;
        }
        let Some((x, t, fresh)) = raw(frame, &s.obs) else {
            return;
        };
        if !fresh {
            return;
        }
        let x = if s.max { x } else { -x };
        self.strike_floor = Some(self.strike_floor.map_or(x, |f| f.min(x)));
        let prev = self.last_strike;
        self.strike_peak.feed(t, x, prev);
        self.last_strike = Some((t, x));
        if let Some((tv, xv)) = self
            .strike_peak
            .confirmed(s.prominence, self.strike_floor.unwrap_or(x))
        {
            self.strike = Some((tv, if s.max { xv } else { -xv }));
            self.strike_reported = false;
        }
    }

    fn strike_event(&mut self) -> Vec<MachineEvent> {
        match self.strike {
            Some((t, v)) if !self.strike_reported => {
                self.strike_reported = true;
                vec![MachineEvent::Strike { t_us: t, value: v }]
            }
            _ => vec![],
        }
    }

    /// Recognition confidence of a completed repetition (07 §3.2).
    pub fn confidence(&self, rep_end: u64) -> f64 {
        let mut c = self.min_confidence;
        if self.image_plane {
            c *= 0.8;
        }
        if self.optional_known_skipped {
            c *= 0.85;
        }
        let d = rep_end.saturating_sub(self.rep_start.unwrap_or(rep_end)) as f64;
        let (lo, hi) = (self.tech.min_us as f64, self.tech.max_us as f64);
        if (d - lo).abs() <= 0.1 * lo || (d - hi).abs() <= 0.1 * hi {
            c *= 0.8;
        }
        c.clamp(0.0, 1.0)
    }

    pub fn mandatory_phases(&self) -> usize {
        self.tech.phases.iter().filter(|p| !p.optional).count()
    }
}
